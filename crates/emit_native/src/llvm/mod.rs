//! Gerador de LLVM IR a partir da HIR nativa.

pub mod externs;
#[cfg(test)]
mod testes;

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    out: String,
    string_constants: Vec<(String, usize)>,
    /// Tipo de cada valor da funcao sendo emitida, para coercao de operandos.
    tipos: std::collections::HashMap<ValueId, Type>,
    /// Contador dos temporarios de coercao (%c0, %c1, ...), por funcao.
    prox_coercao: u32,
    /// Conversoes que uma entrada de phi exige, atribuidas ao bloco de ORIGEM:
    /// (bloco, nome do temporario, tipo de origem, valor, tipo do phi).
    conv_phi: Vec<(u32, String, Type, ValueId, Type)>,
    /// Tipo guardado por cada `alloca` da função (para o `store`).
    apontado: std::collections::HashMap<ValueId, Type>,
    /// G: slot de raiz de cada valor `Ref` da função (SSA ou `alloca`).
    slots: std::collections::HashMap<ValueId, usize>,
    /// A função abriu um frame de raízes (`%gcf`).
    tem_frame: bool,
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            string_constants: Vec::new(),
            tipos: std::collections::HashMap::new(),
            prox_coercao: 0,
            conv_phi: Vec::new(),
            apontado: std::collections::HashMap::new(),
            slots: std::collections::HashMap::new(),
            tem_frame: false,
        }
    }

    pub fn emit_all(mut self) -> String {
        if !self.module.erros.is_empty() {
            return self.emit_modulo_de_erro();
        }
        // Coleta literais de strings do módulo para declaração como constantes globais
        self.collect_string_constants();

        // 1. Cabeçalho de target
        self.emit_header();

        // 2. Declarações do runtime
        self.emit_runtime_decls();

        // 3. Constantes de strings globais
        self.emit_string_constants();

        // 4. Classes e vtables
        self.emit_vtables();

        // 4b. Globais do usuário (N6)
        self.emit_globais();

        // 5. Funções compiladas
        for func in &self.module.functions {
            self.emit_function(func);
        }

        // 6. Funções de despacho polimórfico
        self.emit_dispatch_functions();

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.out
    }

    fn collect_string_constants(&mut self) {
        for func in &self.module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    if let Instruction::Const(Constant::String(s)) = inst {
                        if !self.string_constants.iter().any(|(existing, _)| existing == s) {
                            let idx = self.string_constants.len();
                            self.string_constants.push((s.clone(), idx));
                        }
                    }
                }
            }
        }
        for class in &self.module.classes {
            if !self.string_constants.iter().any(|(existing, _)| existing == &class.name) {
                let idx = self.string_constants.len();
                self.string_constants.push((class.name.clone(), idx));
            }
        }
    }

    fn emit_header(&mut self) {
        self.out.push_str("target datalayout = \"e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"\n");
        self.out.push_str("target triple = \"x86_64-pc-windows-msvc\"\n\n");
    }

    fn emit_runtime_decls(&mut self) {
        self.out.push_str("; Declarações do runtime nativo Rust (tabela em llvm/externs.rs)\n");
        for e in externs::EXTERNS {
            self.out.push_str(e.decl);
            self.out.push('\n');
        }
        self.out.push('\n');
    }

    fn emit_string_constants(&mut self) {
        if self.string_constants.is_empty() {
            return;
        }
        self.out.push_str("; Constantes de string UTF-8\n");
        for (s, idx) in &self.string_constants {
            let bytes = s.as_bytes();
            let len = bytes.len();
            let mut escaped = String::new();
            for &b in bytes {
                if (b as char).is_ascii_alphanumeric() || b == b' ' || b == b'_' || b == b'.' {
                    escaped.push(b as char);
                } else {
                    write!(escaped, "\\{:02X}", b).unwrap();
                }
            }
            writeln!(
                self.out,
                "@.str.{idx} = private unnamed_addr constant [{len} x i8] c\"{escaped}\""
            ).unwrap();
        }
        self.out.push('\n');
    }

    fn string_const_index(&self, s: &str) -> Option<usize> {
        self.string_constants.iter().find(|(existing, _)| existing == s).map(|(_, idx)| *idx)
    }

    fn emit_vtables(&mut self) {
        // vtables globais se houver classes
        for class in &self.module.classes {
            writeln!(self.out, "; VTable da classe {} (id {})", class.name, class.id).unwrap();
        }
    }

    fn emit_function(&mut self, func: &Function) {
        // Tabela de tipos da funcao: sem ela o emissor nao sabe se %v8 e um
        // i1 (resultado de icmp) ou um i64, e imprime "ret i64 %v8" para um
        // valor i1 — modulo inteiro recusado pelo Clang.
        self.tipos.clear();
        self.apontado.clear();
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                if let Instruction::Alloca(t) = inst {
                    self.apontado.insert(*vid, *t);
                }
            }
        }
        self.prox_coercao = 0;
        for (vid, _, ty) in &func.params {
            self.tipos.insert(*vid, *ty);
        }
        for block in &func.blocks {
            for (vid, inst, ty) in &block.instructions {
                self.tipos.insert(*vid, Self::tipo_do_resultado(inst, *ty));
            }
        }

        // Uma entrada de phi nao pode ser convertida onde o phi esta: phi tem de
        // ser a primeira instrucao do bloco. A conversao pertence ao bloco de
        // ORIGEM daquela entrada, emitida logo antes do terminador dele. Aqui
        // so planejamos; a emissao acontece bloco a bloco, mais abaixo.
        self.conv_phi.clear();
        let blocos_existentes: std::collections::HashSet<u32> =
            func.blocks.iter().map(|b| b.id.0).collect();
        for block in &func.blocks {
            for (_, inst, _) in &block.instructions {
                let Instruction::Phi { incoming, ty } = inst else { continue };
                for (origem, op) in incoming {
                    let Operand::Val(v) = op else { continue };
                    if !blocos_existentes.contains(&origem.0) {
                        continue;
                    }
                    let de = self.tipos.get(v).copied().unwrap_or(Type::I64);
                    let igual = de.llvm_ir() == ty.llvm_ir() && (de == Type::F64) == (*ty == Type::F64);
                    if igual {
                        continue;
                    }
                    let ja = self.conv_phi.iter().any(|(b, _, _, vv, para)| {
                        *b == origem.0 && vv == v && para.llvm_ir() == ty.llvm_ir()
                    });
                    if ja {
                        continue;
                    }
                    let nome = format!("%p{}", self.prox_coercao);
                    self.prox_coercao += 1;
                    self.conv_phi.push((origem.0, nome, de, *v, *ty));
                }
            }
        }

        let ret_ty = func.return_ty.llvm_ir();
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(vid, _, ty)| format!("{} %v{}", ty.llvm_ir(), vid.0))
            .collect();
        let params_str = params.join(", ");

        writeln!(self.out, "define {ret_ty} @{}({}) {{", func.symbol, params_str).unwrap();

        // G1/G2 (docs/NATIVO-PLANO.md §6.5): um slot fixo por `alloca` de
        // tipo `Ref` e por valor SSA `Ref` (parâmetro, resultado de chamada,
        // `Load`, `phi`, caixa, alocação). A ordem é a das instruções, para
        // o IR ser determinístico.
        self.slots.clear();
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                if matches!(inst, Instruction::Alloca(Type::Ref)) {
                    let n = self.slots.len();
                    self.slots.insert(*vid, n);
                }
            }
        }
        for (vid, _, ty) in &func.params {
            if *ty == Type::Ref {
                let n = self.slots.len();
                self.slots.insert(*vid, n);
            }
        }
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                let define_ref = self.tipos.get(vid) == Some(&Type::Ref)
                    && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_));
                if define_ref {
                    let n = self.slots.len();
                    self.slots.insert(*vid, n);
                }
            }
        }
        self.tem_frame = !self.slots.is_empty();

        for block in &func.blocks {
            writeln!(self.out, "b{}:", block.id.0).unwrap();
            if block.id.0 == 0 && self.tem_frame {
                writeln!(self.out, "  %gcf = call i64 @dartforge_gc_push_frame(i64 {})", self.slots.len()).unwrap();
                for (vid, _, ty) in &func.params {
                    if *ty == Type::Ref {
                        let slot = self.slots[vid];
                        writeln!(self.out, "  call void @dartforge_gc_set_root(i64 %gcf, i64 {slot}, i64 %v{})", vid.0).unwrap();
                    }
                }
            }
            // `phi` tem de ser a primeira instrução do bloco: as raízes dos
            // `phi` saem todas depois do último deles.
            let mut raizes_de_phi: Vec<(usize, u32)> = Vec::new();

            for (vid, inst, ty) in &block.instructions {
                let v = vid.0;
                if !matches!(inst, Instruction::Phi { .. }) && !raizes_de_phi.is_empty() {
                    for (slot, pv) in std::mem::take(&mut raizes_de_phi) {
                        writeln!(self.out, "  call void @dartforge_gc_set_root(i64 %gcf, i64 {slot}, i64 %v{pv})").unwrap();
                    }
                }
                match inst {
                    Instruction::Const(Constant::Int(n)) => {
                        writeln!(self.out, "  %v{v} = add i64 0, {n}").unwrap();
                    }
                    Instruction::Const(Constant::Double(d)) => {
                        let bits = d.to_bits();
                        writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap();
                    }
                    Instruction::Const(Constant::Bool(b)) => {
                        let bit = if *b { 1 } else { 0 };
                        writeln!(self.out, "  %v{v} = add i1 0, {bit}").unwrap();
                    }
                    Instruction::Const(Constant::Null) => {
                        writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
                    }
                    Instruction::Const(Constant::String(s)) => {
                        let idx = self.string_const_index(s).unwrap_or(0);
                        let len = s.as_bytes().len();
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_string_new(ptr @.str.{idx}, i64 {len})"
                        ).unwrap();
                    }
                    Instruction::Add(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = add i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Sub(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Mul(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = mul i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SDiv(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sdiv i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SRem(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = srem i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Shl(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = shl i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::AShr(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = ashr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::And(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = and i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Or(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = or i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Xor(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Neg(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 0, {sa}").unwrap();
                    }
                    Instruction::Not(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, -1").unwrap();
                    }
                    Instruction::FAdd(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fadd double {sa}, {sb}").unwrap();
                    }
                    Instruction::FSub(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fsub double {sa}, {sb}").unwrap();
                    }
                    Instruction::FMul(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fmul double {sa}, {sb}").unwrap();
                    }
                    Instruction::FDiv(a, b) => {
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fdiv double {sa}, {sb}").unwrap();
                    }
                    Instruction::FNeg(a) => {
                        let sa = self.coagir(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fneg double {sa}").unwrap();
                    }
                    Instruction::ICmp(op, a, b) => {
                        let op_str = match op {
                            ICmpOp::Eq => "eq",
                            ICmpOp::Ne => "ne",
                            ICmpOp::Slt => "slt",
                            ICmpOp::Sle => "sle",
                            ICmpOp::Sgt => "sgt",
                            ICmpOp::Sge => "sge",
                        };
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = icmp {op_str} i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::FCmp(op, a, b) => {
                        let op_str = match op {
                            FCmpOp::Eq => "oeq",
                            FCmpOp::Ne => "one",
                            FCmpOp::Lt => "olt",
                            FCmpOp::Le => "ole",
                            FCmpOp::Gt => "ogt",
                            FCmpOp::Ge => "oge",
                        };
                        let sa = self.coagir(a, Type::F64);
                        let sb = self.coagir(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fcmp {op_str} double {sa}, {sb}").unwrap();
                    }
                    Instruction::LNot(a) => {
                        let sa = self.coagir(a, Type::I1);
                        writeln!(self.out, "  %v{v} = xor i1 {sa}, true").unwrap();
                    }
                    Instruction::IntToDouble(a) => {
                        let sa = self.coagir(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sitofp i64 {sa} to double").unwrap();
                    }
                    Instruction::DoubleToInt(a) => {
                        let sa = self.coagir(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fptosi double {sa} to i64").unwrap();
                    }
                    Instruction::ZExt { op, from, to } => {
                        let sop = self.coagir(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = zext {f} {sop} to {t}").unwrap();
                    }
                    Instruction::Trunc { op, from, to } => {
                        let sop = self.coagir(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = trunc {f} {sop} to {t}").unwrap();
                    }
                    Instruction::AllocObject { class_id, fields } => {
                        let count = fields.len();
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_new(i64 {class_id}, i64 {count})"
                        ).unwrap();
                        for (idx, field) in fields.iter().enumerate() {
                            // E1: `is_ref` pela representação do valor.
                            let is_ref = u8::from(self.tipo_de(field) == Type::Ref);
                            let sf = self.coagir(field, Type::I64);
                            writeln!(
                                self.out,
                                "  call void @dartforge_object_set(i64 %v{v}, i64 {idx}, i64 {sf}, i8 {is_ref})"
                            ).unwrap();
                        }
                    }
                    Instruction::GetField { object, index } => {
                        let so = self.coagir(object, Type::I64);
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_get(i64 {so}, i64 {index})"
                        ).unwrap();
                    }
                    Instruction::SetField { object, index, value } => {
                        let is_ref = u8::from(self.tipo_de(value) == Type::Ref);
                        let so = self.coagir(object, Type::I64);
                        let sv = self.coagir(value, Type::I64);
                        writeln!(
                            self.out,
                            "  call void @dartforge_object_set(i64 {so}, i64 {index}, i64 {sv}, i8 {is_ref})"
                        ).unwrap();
                    }
                    Instruction::CallStatic { symbol, args, ret_ty } => {
                        let modulo = self.module;
                        let target_func = modulo.functions.iter().find(|f| f.symbol == *symbol);
                        let mut args_str: Vec<String> = Vec::with_capacity(args.len());
                        for (idx, a) in args.iter().enumerate() {
                            let alvo = target_func
                                .and_then(|f| f.params.get(idx))
                                .map_or(Type::I64, |p| p.2);
                            let s = self.coagir(a, alvo);
                            args_str.push(format!("{} {s}", alvo.llvm_ir()));
                        }
                        let joined = args_str.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{symbol}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{symbol}({joined})").unwrap();
                        }
                    }
                    Instruction::CallRuntime { name, args, ret_ty } => {
                        let mut args_formatted = Vec::new();
                        for (a, ty) in args {
                            let s = self.coagir(a, *ty);
                            let t = ty.llvm_ir();
                            args_formatted.push(format!("{t} {s}"));
                        }
                        let joined = args_formatted.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{name}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{name}({joined})").unwrap();
                        }
                    }
                    Instruction::AllocList { elements } => {
                        // Alloca temporário para pares (bits, tag)
                        let count = elements.len();
                        let alloca_id = format!("list_buf_{v}");
                        writeln!(self.out, "  %{alloca_id} = alloca [{} x i64]", count * 2).unwrap();
                        for (idx, (elem, tag)) in elements.iter().enumerate() {
                            let se = self.coagir(elem, Type::I64);
                            let off_bits = idx * 2;
                            let off_tag = idx * 2 + 1;
                            writeln!(self.out, "  %ptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{alloca_id}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {se}, ptr %ptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %ptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{alloca_id}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {tag}, ptr %ptr_{v}_{off_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_list_new(ptr %{alloca_id}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::AllocMap { entries } => {
                        let count = entries.len();
                        let k_buf = format!("map_k_{v}");
                        let v_buf = format!("map_v_{v}");
                        writeln!(self.out, "  %{k_buf} = alloca [{} x i64]", count * 2).unwrap();
                        writeln!(self.out, "  %{v_buf} = alloca [{} x i64]", count * 2).unwrap();
                        for (idx, ((k, k_tag), (val, v_tag))) in entries.iter().enumerate() {
                            let sk = self.coagir(k, Type::I64);
                            let sv = self.coagir(val, Type::I64);
                            let off_bits = idx * 2;
                            let off_tag = idx * 2 + 1;
                            writeln!(self.out, "  %kptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{k_buf}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {sk}, ptr %kptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %kptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{k_buf}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {k_tag}, ptr %kptr_{v}_{off_tag}").unwrap();

                            writeln!(self.out, "  %vptr_{v}_{off_bits} = getelementptr [{} x i64], ptr %{v_buf}, i64 0, i64 {off_bits}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {sv}, ptr %vptr_{v}_{off_bits}").unwrap();
                            writeln!(self.out, "  %vptr_{v}_{off_tag} = getelementptr [{} x i64], ptr %{v_buf}, i64 0, i64 {off_tag}", count * 2).unwrap();
                            writeln!(self.out, "  store i64 {v_tag}, ptr %vptr_{v}_{off_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_map_new(ptr %{k_buf}, ptr %{v_buf}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::AllocRecord { elements } => {
                        let count = elements.len();
                        let total_i64 = count * 2;
                        let buf_name = format!("rec_buf_{v}");
                        writeln!(self.out, "  %{buf_name} = alloca [{total_i64} x i64]").unwrap();
                        for (i, (elem, tag)) in elements.iter().enumerate() {
                            let sop = self.coagir(elem, Type::I64);
                            let ptr_bits = format!("ptr_rec_{v}_{i}_bits");
                            let ptr_tag = format!("ptr_rec_{v}_{i}_tag");
                            let off_bits = i * 2;
                            let off_tag = i * 2 + 1;
                            writeln!(self.out, "  %{ptr_bits} = getelementptr [{total_i64} x i64], ptr %{buf_name}, i64 0, i64 {off_bits}").unwrap();
                            writeln!(self.out, "  store i64 {sop}, ptr %{ptr_bits}").unwrap();
                            writeln!(self.out, "  %{ptr_tag} = getelementptr [{total_i64} x i64], ptr %{buf_name}, i64 0, i64 {off_tag}").unwrap();
                            writeln!(self.out, "  store i64 {tag}, ptr %{ptr_tag}").unwrap();
                        }
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_record_new(ptr %{buf_name}, i64 {count})"
                        ).unwrap();
                    }
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{v} = alloca {}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Load { ptr, ty } => {
                        let sp = self.operand_str(ptr);
                        writeln!(self.out, "  %v{v} = load {}, ptr {sp}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Store { ptr, val } => {
                        // O local guarda a representação do seu tipo (R6); o
                        // valor chega já coagido pelo lowering, e aqui só se
                        // acerta a largura.
                        let t = match ptr {
                            Operand::Val(p) => self.apontado.get(p).copied().unwrap_or(Type::I64),
                            _ => Type::I64,
                        };
                        let sp = self.operand_str(ptr);
                        let sv = self.coagir(val, t);
                        writeln!(self.out, "  store {} {sv}, ptr {sp}", t.llvm_ir()).unwrap();
                        // G2: o local `Ref` tem slot próprio, atualizado a
                        // cada gravação — ele vive mais que o SSA que o gravou.
                        if let Operand::Val(pv) = ptr
                            && let Some(&slot) = self.slots.get(pv)
                        {
                            writeln!(self.out, "  call void @dartforge_gc_set_root(i64 %gcf, i64 {slot}, i64 {sv})").unwrap();
                        }
                    }
                    Instruction::Box { op, from } => {
                        match from {
                            Type::F64 => {
                                let so = self.coagir(op, Type::F64);
                                writeln!(self.out, "  %v{v} = call i64 @dartforge_box_double(double {so})").unwrap();
                            }
                            Type::I1 | Type::I8 => {
                                let so = self.coagir(op, Type::I8);
                                writeln!(self.out, "  %v{v} = call i64 @dartforge_box_bool(i8 {so})").unwrap();
                            }
                            _ => {
                                let so = self.coagir(op, Type::I64);
                                writeln!(self.out, "  %v{v} = call i64 @dartforge_box_int(i64 {so})").unwrap();
                            }
                        }
                    }
                    Instruction::Unbox { op, to } => {
                        let so = self.coagir(op, Type::Ref);
                        match to {
                            Type::F64 => {
                                writeln!(self.out, "  %v{v} = call double @dartforge_unbox_double(i64 {so})").unwrap();
                            }
                            Type::I1 => {
                                writeln!(self.out, "  %u{v} = call i8 @dartforge_unbox_bool(i64 {so})").unwrap();
                                writeln!(self.out, "  %v{v} = trunc i8 %u{v} to i1").unwrap();
                            }
                            _ => {
                                writeln!(self.out, "  %v{v} = call i64 @dartforge_unbox_int(i64 {so})").unwrap();
                            }
                        }
                    }
                    Instruction::LShr(a, b) => {
                        let sa = self.coagir(a, Type::I64);
                        let sb = self.coagir(b, Type::I64);
                        writeln!(self.out, "  %v{v} = lshr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Bitcast { op, to } => {
                        if *to == Type::F64 {
                            let so = self.coagir(op, Type::I64);
                            writeln!(self.out, "  %v{v} = bitcast i64 {so} to double").unwrap();
                        } else {
                            let so = self.coagir(op, Type::F64);
                            writeln!(self.out, "  %v{v} = bitcast double {so} to i64").unwrap();
                        }
                    }
                    Instruction::LoadGlobal { simbolo, ty } => {
                        writeln!(self.out, "  %v{v} = load {}, ptr @{simbolo}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::StoreGlobal { simbolo, val, ty, raiz } => {
                        let sv = self.coagir(val, *ty);
                        writeln!(self.out, "  store {} {sv}, ptr @{simbolo}", ty.llvm_ir()).unwrap();
                        if let Some(id) = raiz {
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 {id}, i64 {sv})").unwrap();
                        }
                    }
                    Instruction::Phi { incoming, ty } => {
                        let t = ty.llvm_ir();
                        // A coercao de uma entrada de `phi` NAO pode ser emitida
                        // aqui: `phi` tem de ser a primeira instrucao do bloco e a
                        // conversao pertence ao bloco de origem. Entao so constantes
                        // sao reescritas no tipo do `phi`; valores que chegam com
                        // largura diferente sao corrigidos na origem, ao terminar
                        // aquele bloco.
                        let in_strs: Vec<String> = incoming
                            .iter()
                            .map(|(b, op)| {
                                let sop = self
                                    .constante_no_tipo(op, *ty)
                                    .or_else(|| match op {
                                        Operand::Val(v) => self
                                            .conv_phi
                                            .iter()
                                            .find(|(bb, _, _, vv, para)| {
                                                *bb == b.0 && vv == v && para.llvm_ir() == ty.llvm_ir()
                                            })
                                            .map(|(_, nome, _, _, _)| nome.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| self.operand_str(op));
                                format!("[ {sop}, %b{} ]", b.0)
                            })
                            .collect();
                        let joined = in_strs.join(", ");
                        writeln!(self.out, "  %v{v} = phi {t} {joined}").unwrap();
                    }
                    _ => {
                        // E3: o verificador da HIR recusa, antes da emissão,
                        // toda instrução sem lowering aqui (o antigo
                        // "; inst pendente" que o Clang aceitava calado).
                        unreachable!("instrução sem emissão passou pelo verificador: {inst:?}");
                    }
                }
                // G1: a raiz logo depois da definição — nada aloca entre
                // o retorno da chamada e este `set_root` (G5).
                if let Some(&slot) = self.slots.get(vid) {
                    if matches!(inst, Instruction::Phi { .. }) {
                        raizes_de_phi.push((slot, v));
                    } else if !matches!(inst, Instruction::Alloca(_)) {
                        writeln!(self.out, "  call void @dartforge_gc_set_root(i64 %gcf, i64 {slot}, i64 %v{v})").unwrap();
                    }
                }
                let _ = ty;
            }
            for (slot, pv) in std::mem::take(&mut raizes_de_phi) {
                writeln!(self.out, "  call void @dartforge_gc_set_root(i64 %gcf, i64 {slot}, i64 %v{pv})").unwrap();
            }

            for (b, nome, de, v, para) in self.conv_phi.clone() {
                if b == block.id.0 {
                    let origem = format!("%v{}", v.0);
                    self.emitir_conversao_nomeada(&nome, de, &origem, para);
                }
            }

            // G3: o frame de raízes fecha antes de TODO `ret`, inclusive o
            // das saídas por exceção (que retornam o valor padrão).
            if self.tem_frame && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  call void @dartforge_gc_pop_frame(i64 %gcf)").unwrap();
            }
            match &block.terminator {
                Terminator::Return(Some(op)) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        let sop = self.coagir(op, func.return_ty);
                        writeln!(self.out, "  ret {} {sop}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Return(None) => {
                    let zero = match func.return_ty {
                        Type::F64 => "0.0",
                        Type::I1 => "false",
                        _ => "0",
                    };
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        writeln!(self.out, "  ret {} {zero}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Branch(target) => {
                    writeln!(self.out, "  br label %b{}", target.0).unwrap();
                }
                Terminator::CondBranch { cond, then_block, else_block } => {
                    let sc = self.coagir(cond, Type::I1);
                    writeln!(self.out, "  br i1 {sc}, label %b{}, label %b{}", then_block.0, else_block.0).unwrap();
                }
                Terminator::Switch { val, default, cases } => {
                    let sv = self.coagir(val, Type::I64);
                    write!(self.out, "  switch i64 {sv}, label %b{} [", default.0).unwrap();
                    for (c, b) in cases {
                        write!(self.out, " i64 {c}, label %b{}", b.0).unwrap();
                    }
                    writeln!(self.out, " ]").unwrap();
                }
                Terminator::Throw(op) => {
                    let tag = match self.tipo_de(op) {
                        Type::I64 => 1,
                        Type::I1 | Type::I8 => 2,
                        Type::F64 => 4,
                        _ => 3,
                    };
                    let sop = self.coagir(op, Type::I64);
                    writeln!(self.out, "  call void @dartforge_exception_throw(i64 {sop}, i8 {tag})").unwrap();
                    writeln!(self.out, "  unreachable").unwrap();
                }
                Terminator::Unreachable => {
                    writeln!(self.out, "  unreachable").unwrap();
                }
            }
        }

        writeln!(self.out, "}}\n").unwrap();
    }

    /// Programa com construto não suportado (N1): o lowering não gera
    /// código; o executável só relata os diagnósticos e sai com 254.
    ///
    /// O lugar certo deste erro é `compilar` devolver `Err` — o que exige
    /// mexer em `lib.rs`, congelado nesta sessão por outro trabalho (a
    /// separação da emissão de IR e o cache de objeto). Até lá o diagnóstico
    /// chega ao placar pelo executável, com a mesma primeira linha.
    fn emit_modulo_de_erro(mut self) -> String {
        self.emit_header();
        // Primeira linha sem a posição: é a chave de agrupamento do harness,
        // e o mesmo construto em programas diferentes tem de cair no mesmo
        // grupo. As posições vêm nas linhas seguintes.
        let primeiro = &self.module.erros[0];
        let resumo = primeiro.rsplit_once(" (").map_or(primeiro.as_str(), |(a, _)| a);
        let mut texto = format!("erro de compilação: {resumo}\n");
        for e in &self.module.erros {
            texto.push_str("  ");
            texto.push_str(e);
            texto.push('\n');
        }
        let bytes = texto.as_bytes();
        let mut escapado = String::new();
        for &b in bytes {
            if (b as char).is_ascii_alphanumeric() || b == b' ' {
                escapado.push(b as char);
            } else {
                write!(escapado, "\\{:02X}", b).unwrap();
            }
        }
        writeln!(self.out, "@.erros = private unnamed_addr constant [{} x i8] c\"{escapado}\"", bytes.len()).unwrap();
        self.out.push_str("declare void @dartforge_erro_de_compilacao(ptr, i64)\n\n");
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        writeln!(self.out, "  call void @dartforge_erro_de_compilacao(ptr @.erros, i64 {})", bytes.len()).unwrap();
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}").unwrap();
        self.out
    }

    /// `@dfg_<id>` (valor, no tipo da representação) e `@dfg_<id>_ok`.
    fn emit_globais(&mut self) {
        for (id, ty) in &self.module.globais {
            let (t, zero) = match ty {
                Type::F64 => ("double", "0.0"),
                Type::I1 => ("i1", "false"),
                Type::I8 => ("i8", "0"),
                _ => ("i64", "0"),
            };
            writeln!(self.out, "@dfg_{id} = internal global {t} {zero}").unwrap();
            writeln!(self.out, "@dfg_{id}_ok = internal global i8 0").unwrap();
        }
        self.out.push('\n');
    }

    fn emit_dispatch_functions(&mut self) {
        self.out.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n");
        self.out.push_str("b0:\n");
        self.out.push_str("  %is_null = icmp eq i64 %obj, 0\n");
        self.out.push_str("  br i1 %is_null, label %ret_null, label %check_obj\n");
        self.out.push_str("ret_null:\n");
        self.out.push_str("  %null_s = call i64 @dartforge_to_string_handle(i64 0)\n");
        self.out.push_str("  ret i64 %null_s\n");
        self.out.push_str("check_obj:\n");
        self.out.push_str("  %cls = call i64 @dartforge_value_class(i64 %obj)\n");

        let mut cases = Vec::new();
        for class in &self.module.classes {
            if let Some(sym) = &class.to_string_symbol {
                cases.push((class.id, sym.clone()));
            }
        }

        if cases.is_empty() {
            self.out.push_str("  br label %fallback\n");
        } else {
            write!(self.out, "  switch i64 %cls, label %fallback [").unwrap();
            for (cid, _) in &cases {
                write!(self.out, " i64 {cid}, label %case_{cid}").unwrap();
            }
            writeln!(self.out, " ]").unwrap();
            for (cid, sym) in &cases {
                writeln!(self.out, "case_{cid}:").unwrap();
                writeln!(self.out, "  %res_{cid} = call i64 @{sym}(i64 %obj)").unwrap();
                writeln!(self.out, "  ret i64 %res_{cid}").unwrap();
            }
        }
        self.out.push_str("fallback:\n");
        self.out.push_str("  %fb = call i64 @dartforge_to_string_handle(i64 %obj)\n");
        self.out.push_str("  ret i64 %fb\n");
        self.out.push_str("}\n\n");
    }

    fn emit_entry(&mut self) {
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        // Registra classes
        for class in &self.module.classes {
            let idx = self.string_const_index(&class.name).unwrap_or(0);
            let len = class.name.as_bytes().len();
            writeln!(
                self.out,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{}, i64 {})",
                class.id, idx, len
            ).unwrap();
        }

        // Registra grafo de subtipagem
        for (sub, sup) in &self.module.subtyping_edges {
            writeln!(
                self.out,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            ).unwrap();
        }

        if let Some(entry) = &self.module.entry_symbol {
            writeln!(self.out, "  call void @{entry}()").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    /// Tipo do valor que o emissor de fato imprime para uma instrucao.
    ///
    /// Nao basta acreditar no tipo registrado na HIR: varios arms imprimem um
    /// tipo fixo (todo Add sai como i64, todo ICmp como i1) e um registro
    /// divergente faria a coercao trabalhar com a informacao errada. O tipo
    /// registrado so vale onde o emissor o usa (Load, Phi, Alloca e as
    /// instrucoes ainda nao expandidas).
    fn tipo_do_resultado(inst: &Instruction, registrado: Type) -> Type {
        match inst {
            Instruction::Const(Constant::Bool(_)) => Type::I1,
            Instruction::Const(Constant::Double(_)) => Type::F64,
            Instruction::Const(Constant::Int(_)) => Type::I64,
            Instruction::Const(_) => Type::Ref,
            Instruction::Add(..)
            | Instruction::Sub(..)
            | Instruction::Mul(..)
            | Instruction::SDiv(..)
            | Instruction::SRem(..)
            | Instruction::Shl(..)
            | Instruction::AShr(..)
            | Instruction::And(..)
            | Instruction::Or(..)
            | Instruction::Xor(..)
            | Instruction::Neg(..)
            | Instruction::Not(..)
            | Instruction::LShr(..)
            | Instruction::DoubleToInt(..) => Type::I64,
            Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocMap { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::Box { .. } => Type::Ref,
            Instruction::Unbox { to, .. } => *to,
            Instruction::Alloca(_) => Type::Ptr,
            Instruction::GetField { .. } => Type::I64,
            Instruction::FAdd(..)
            | Instruction::FSub(..)
            | Instruction::FMul(..)
            | Instruction::FDiv(..)
            | Instruction::FNeg(..)
            | Instruction::IntToDouble(..) => Type::F64,
            Instruction::ICmp(..) | Instruction::FCmp(..) | Instruction::LNot(..) => Type::I1,
            Instruction::ZExt { to, .. } | Instruction::Trunc { to, .. } | Instruction::Bitcast { to, .. } => *to,
            Instruction::LoadGlobal { ty, .. } => *ty,
            Instruction::CallStatic { ret_ty, .. } | Instruction::CallRuntime { ret_ty, .. } => *ret_ty,
            Instruction::Load { ty, .. } => *ty,
            Instruction::Phi { ty, .. } => *ty,
            _ => registrado,
        }
    }

    /// Tipo estatico de um operando dentro da funcao corrente.
    fn tipo_de(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.tipos.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::String(_)) => Type::Ref,
        }
    }

    fn largura(t: Type) -> u32 {
        match t {
            Type::I1 => 1,
            Type::I8 => 8,
            _ => 64,
        }
    }

    /// Literal double na forma hexadecimal do LLVM.
    ///
    /// format!("{d}") imprime 1 para 1.0 e o LLVM recusa "double 1"; a forma
    /// 0x com os 16 digitos do padrao IEEE 754 sempre vale.
    fn double_literal(d: f64) -> String {
        format!("0x{:016X}", d.to_bits())
    }

    /// Reescreve uma constante diretamente no tipo pedido, sem instrucao.
    /// Devolve None quando o operando nao e constante.
    fn constante_no_tipo(&self, op: &Operand, alvo: Type) -> Option<String> {
        let Operand::Constant(c) = op else { return None };
        let s = match (c, alvo) {
            (Constant::String(_), _) => panic!("string deve ser carregada via Instruction::Const"),
            (Constant::Int(n), Type::I1) => if *n != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Bool(b), Type::I1) => if *b { "true".to_string() } else { "false".to_string() },
            (Constant::Null, Type::I1) => "false".to_string(),
            (Constant::Double(d), Type::I1) => if d.to_bits() != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Double(d), Type::F64) => Self::double_literal(*d),
            (Constant::Int(n), Type::F64) => Self::double_literal(f64::from_bits(*n as u64)),
            (Constant::Bool(b), Type::F64) => Self::double_literal(f64::from_bits(u64::from(*b))),
            (Constant::Null, Type::F64) => Self::double_literal(0.0),
            (Constant::Int(n), _) => n.to_string(),
            (Constant::Bool(b), _) => if *b { "1".to_string() } else { "0".to_string() },
            (Constant::Null, _) => "0".to_string(),
            (Constant::Double(d), _) => (d.to_bits() as i64).to_string(),
        };
        Some(s)
    }

    /// Converte um operando para o tipo que a posicao exige.
    ///
    /// O emissor imprime o tipo LLVM em cada posicao ("ret i64", "add i64",
    /// "br i1", o tipo declarado de cada argumento do runtime), mas a HIR
    /// carrega bool ora como i1 (resultado de icmp), ora como i8 (a fronteira
    /// com o Rust), e int/handle como i64. Sem esta conversao o Clang recusa o
    /// modulo inteiro — era a causa de 172 das 214 falhas do corpus nativo.
    ///
    /// Entre i1/i8/i64 a conversao e zext/trunc. Entre double e i64 e bitcast,
    /// nao sitofp/fptosi: a HIR tem nos proprios (IntToDouble/DoubleToInt) para
    /// a conversao numerica, entao um double ocupando um slot i64 so pode ser o
    /// padrao de bits — e assim que Const(Double) e emitido e como os doubles
    /// atravessam o runtime.
    fn coagir(&mut self, op: &Operand, alvo: Type) -> String {
        if alvo == Type::Void {
            return self.operand_str(op);
        }
        if let Some(s) = self.constante_no_tipo(op, alvo) {
            return s;
        }
        let mut atual = self.tipo_de(op);
        let mut texto = self.operand_str(op);
        if atual.llvm_ir() == alvo.llvm_ir() && (atual == Type::F64) == (alvo == Type::F64) {
            return texto;
        }
        // double vira i64 (e vice-versa) sempre pelos bits; larguras menores
        // passam antes por i64 porque bitcast exige tamanho igual.
        if atual == Type::F64 && alvo != Type::F64 {
            texto = self.emitir_conversao("bitcast", Type::F64, &texto, Type::I64);
            atual = Type::I64;
        }
        if alvo == Type::F64 {
            if atual != Type::I64 && atual != Type::Ref {
                texto = self.emitir_conversao("zext", atual, &texto, Type::I64);
                atual = Type::I64;
            }
            if atual != Type::F64 {
                texto = self.emitir_conversao("bitcast", Type::I64, &texto, Type::F64);
            }
            return texto;
        }
        if Self::largura(atual) < Self::largura(alvo) {
            self.emitir_conversao("zext", atual, &texto, alvo)
        } else if Self::largura(atual) > Self::largura(alvo) {
            self.emitir_conversao("trunc", atual, &texto, alvo)
        } else {
            texto
        }
    }

    /// Mesma conversao, mas gravando num nome escolhido por quem chama
    /// (usado pelas entradas de phi, que precisam de um nome estavel).
    fn emitir_conversao_nomeada(&mut self, nome: &str, de: Type, origem: &str, para: Type) {
        let mut atual = de;
        let mut texto = origem.to_string();
        if atual == Type::F64 && para != Type::F64 && Self::largura(para) != 64 {
            texto = self.emitir_conversao("bitcast", Type::F64, &texto, Type::I64);
            atual = Type::I64;
        }
        if para == Type::F64 && atual != Type::F64 && Self::largura(atual) != 64 {
            texto = self.emitir_conversao("zext", atual, &texto, Type::I64);
            atual = Type::I64;
        }
        let op = if atual == Type::F64 || para == Type::F64 {
            "bitcast"
        } else if Self::largura(atual) < Self::largura(para) {
            "zext"
        } else {
            "trunc"
        };
        writeln!(self.out, "  {nome} = {op} {} {texto} to {}", atual.llvm_ir(), para.llvm_ir()).unwrap();
    }

    fn emitir_conversao(&mut self, op: &str, de: Type, texto: &str, para: Type) -> String {
        let c = self.prox_coercao;
        self.prox_coercao += 1;
        writeln!(self.out, "  %c{c} = {op} {} {texto} to {}", de.llvm_ir(), para.llvm_ir()).unwrap();
        format!("%c{c}")
    }

    fn operand_str(&self, op: &Operand) -> String {
        match op {
            Operand::Val(v) => format!("%v{}", v.0),
            Operand::Constant(Constant::Int(n)) => n.to_string(),
            Operand::Constant(Constant::Double(d)) => {
                let _bits = d.to_bits();
                // Em instrução com double imediato, LLVM aceita ponto ou notação científica
                format!("{d}")
            }
            Operand::Constant(Constant::Bool(b)) => if *b { "true" } else { "false" }.to_string(),
            Operand::Constant(Constant::Null) => "0".to_string(),
            Operand::Constant(Constant::String(_)) => {
                panic!("string deve ser carregada via Instruction::Const");
            }
        }
    }
}

