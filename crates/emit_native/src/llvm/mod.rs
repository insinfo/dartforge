//! Gerador de LLVM IR a partir da HIR nativa.

pub mod externs;
mod seletores;
#[cfg(test)]
mod testes;

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    out: String,
    string_constants: Vec<(Vec<u8>, usize)>,
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
    // --- P1 (closures, α) ---
    /// Vetores constantes de `i64` (`@df.arr.<k>`): assinaturas e descritores.
    vetores: Vec<Vec<i64>>,
    vetor_de: std::collections::HashMap<Vec<i64>, usize>,
    // --- P5c (SDK da fonte, δ; `seletores.rs`) ---
    /// Pontos de chamada por seletor já emitidos (um cache cada).
    caches_de_seletor: usize,
    /// Textos dos seletores, na ordem do primeiro uso.
    nomes_de_seletor: Vec<String>,
    /// Assinatura de cada símbolo chamado, para declarar o que o módulo não
    /// define.
    externos: std::collections::BTreeMap<String, String>,
    /// Símbolos definidos em `comdat` (`seletores::ligacao_de`).
    comdats: Vec<String>,
    /// `DARTFORGE_RASTRO=1` na compilação: cada função conta a entrada e a
    /// saída ao runtime, que mostra a pilha de funções Dart numa exceção
    /// (`DARTFORGE_DEPURAR=1`). Só para depurar o SDK da fonte.
    rastro: Option<usize>,
    nomes_do_rastro: Vec<String>,
    // --- Área de globais por isolado ---
    /// O slot (`i64`) de cada global do módulo na área (`@dfg_…` e a
    /// bandeira `$ok`): os estáticos do Dart são por isolado, e cada
    /// isolado é uma thread com a sua área (`dartforge_area_de_globais`).
    slots_de_global: std::collections::HashMap<String, usize>,
    /// Os nomes dos slots, na ordem (para migrar a área numa recarga).
    nomes_de_slot: Vec<String>,
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

            vetores: Vec::new(),
            vetor_de: std::collections::HashMap::new(),
            caches_de_seletor: 0,
            nomes_de_seletor: Vec::new(),
            externos: std::collections::BTreeMap::new(),
            comdats: Vec::new(),
            rastro: std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1").then_some(0),
            nomes_do_rastro: Vec::new(),
            slots_de_global: std::collections::HashMap::new(),
            nomes_de_slot: Vec::new(),
        }
    }

    /// O módulo inteiro. Um módulo com diagnósticos não chega aqui:
    /// `emitir_ir` devolve o erro antes (N1).
    pub fn emit_all(mut self) -> String {
        assert!(self.module.erros.is_empty(), "emit_all com diagnósticos: {:?}", self.module.erros);
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

        // 4c. Tabela de código das closures e vetores constantes (P1)
        self.emit_closures();

        // 5. Funções compiladas
        for func in &self.module.functions {
            self.emit_function(func);
        }

        if self.module.biblioteca_sdk {
            // Uma biblioteca do SDK da fonte (P5c): sem entrada nem despacho
            // de `toString`, que são do programa; a função de registro das
            // classes dela, que a entrada do programa chama.
            let reg = self.module.registro.clone().unwrap_or_else(|| "df.registrar".to_string());
            self.emitir_registro(&reg);
            self.emitir_globais_de_seletores();
            self.emitir_descritor_da_area();
            self.emitir_declaracoes_externas();
            return self.out;
        }

        // 6. Funções de despacho polimórfico
        if self.module.modo_sdk {
            self.emitir_to_string_por_seletor();
        } else {
            self.emit_dispatch_functions();
        }

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.emitir_globais_de_seletores();
        self.emitir_descritor_da_area();
        self.emitir_declaracoes_externas();
        self.out
    }

    fn collect_string_constants(&mut self) {
        for func in &self.module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    if let Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) = inst {
                        let bytes: &[u8] = match inst {
                            Instruction::Const(Constant::String(s)) => s.as_bytes(),
                            Instruction::Const(Constant::StringWtf8(s)) => s,
                            _ => unreachable!(),
                        };
                        if !self.string_constants.iter().any(|(existing, _)| existing == bytes) {
                            let idx = self.string_constants.len();
                            self.string_constants.push((bytes.to_vec(), idx));
                        }
                    }
                }
            }
        }
        for class in &self.module.classes {
            if !self.string_constants.iter().any(|(existing, _)| existing == class.name.as_bytes()) {
                let idx = self.string_constants.len();
                self.string_constants.push((class.name.as_bytes().to_vec(), idx));
            }
        }
    }

    fn emit_header(&mut self) {
        self.out.push_str(crate::alvo::cabecalho_ir());
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
        self.out.push_str("; Constantes de string WTF-8\n");
        for (s, idx) in &self.string_constants {
            let bytes = s.as_slice();
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

    fn string_const_index(&self, s: &[u8]) -> Option<usize> {
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

        let (ligacao, comdat) = self.ligacao_de(&func.symbol);
        writeln!(self.out, "define {ligacao}{ret_ty} @{}({}){comdat} {{", func.symbol, params_str).unwrap();

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
            if block.id.0 == 0 {
                self.emit_buffers_de_closure(func);
                if Self::usa_area(func) {
                    writeln!(self.out, "  %area = call ptr @dartforge_area_de_globais(ptr @df.area)").unwrap();
                }
            }
            if block.id.0 == 0
                && let Some(k) = self.rastro.as_mut()
            {
                let n = *k;
                *k += 1;
                self.nomes_do_rastro.push(func.symbol.clone());
                writeln!(self.out, "  call void @dartforge_rastro_entrada(ptr @df.rastro.{n}, i64 {})", func.symbol.len()).unwrap();
            }
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
                    Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => {
                        let bytes: &[u8] = match inst {
                            Instruction::Const(Constant::String(s)) => s.as_bytes(),
                            Instruction::Const(Constant::StringWtf8(s)) => s,
                            _ => unreachable!(),
                        };
                        let idx = self.string_const_index(bytes).unwrap_or(0);
                        let len = bytes.len();
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
                        let mut tipos_args = Vec::with_capacity(args.len());
                        for (idx, a) in args.iter().enumerate() {
                            // Função de outro módulo (SDK da fonte): o tipo do
                            // operando, que o lowering já coagiu para a
                            // representação do parâmetro.
                            let alvo = target_func
                                .and_then(|f| f.params.get(idx))
                                .map_or_else(|| self.tipo_de(a), |p| p.2);
                            let alvo = if alvo == Type::Void { Type::I64 } else { alvo };
                            let s = self.coagir(a, alvo);
                            args_str.push(format!("{} {s}", alvo.llvm_ir()));
                            tipos_args.push(alvo);
                        }
                        if target_func.is_none() {
                            self.anotar_externo(symbol, *ret_ty, &tipos_args);
                        }
                        let joined = args_str.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{symbol}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{symbol}({joined})").unwrap();
                        }
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if name == "dartforge_object_new"
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.funcoes_de_tabela.contains_key(&(*c as u32))) =>
                    {
                        // SDK da fonte: a primeira alocação registra a tabela
                        // de métodos da classe (`seletores.rs`).
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.funcoes_de_tabela[&(*c as u32)].clone();
                        self.anotar_externo(&f, Type::Ptr, &[]);
                        let n = self.coagir(&args[1].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i64 @dartforge_object_new_t(i64 {c}, i64 {n}, ptr @{f})").unwrap();
                        let _ = ret_ty;
                    }
                    Instruction::CallRuntime { name, args, ret_ty }
                        if (name == "dartforge_typed_novo" || name == "dartforge_view_nova")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.funcoes_de_tabela.contains_key(&(*c as u32))) =>
                    {
                        // A primeira lista tipada (ou visão) de uma classe
                        // registra a tabela de métodos dela, como
                        // `dartforge_object_new_t`.
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.funcoes_de_tabela[&(*c as u32)].clone();
                        self.anotar_externo(&f, Type::Ptr, &[]);
                        let resto: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coagir(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{name}_t({}, ptr @{f})", resto.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    Instruction::CallRuntime { name, args, ret_ty } => {
                        if name.starts_with("dartforge_nativo_") {
                            let tipos: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            self.anotar_externo(name, *ret_ty, &tipos);
                        }
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
                        self.endereco_do_global(v, simbolo);
                        writeln!(self.out, "  %v{v} = load {}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::StoreGlobal { simbolo, val, ty, raiz } => {
                        let sv = self.coagir(val, *ty);
                        self.endereco_do_global(v, simbolo);
                        writeln!(self.out, "  store {} {sv}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                        if raiz.is_some() {
                            // A raiz é identificada pelo endereço do slot:
                            // único entre os módulos e os isolados.
                            writeln!(self.out, "  %gr{v} = ptrtoint ptr %ga{v} to i64").unwrap();
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 %gr{v}, i64 {sv})").unwrap();
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
                    _ if self.emit_closure_inst(v, inst, *ty) => {}
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

            if self.rastro.is_some() && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  call void @dartforge_rastro_saida()").unwrap();
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

    /// `%cf<v>`: o ponteiro da entrada uniforme da closure `c` — o código
    /// dela, ou `@df_clo_invalido` quando o valor não é closure (o runtime
    /// devolve 0 e deixa o `NoSuchMethodError` pendente).
    fn entrada_da_closure(&mut self, v: u32, c: &str) {
        writeln!(self.out, "  %cc{v} = call i64 @dartforge_closure_entry(i64 {c})").unwrap();
        writeln!(self.out, "  %cz{v} = icmp eq i64 %cc{v}, 0").unwrap();
        writeln!(
            self.out,
            "  %cq{v} = select i1 %cz{v}, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc{v}"
        )
        .unwrap();
        writeln!(self.out, "  %cf{v} = inttoptr i64 %cq{v} to ptr").unwrap();
    }

    /// O descritor de uma chamada pela convenção uniforme:
    /// [n_posicionais, n_nomeados, hash(nome)…].
    fn descritor(args: usize, nomes: &[String]) -> Vec<i64> {
        let mut d = vec![(args - nomes.len()) as i64, nomes.len() as i64];
        d.extend(nomes.iter().map(|n| crate::lower::closures::hash_nome(n)));
        d
    }

    fn registrar_vetor(&mut self, v: Vec<i64>) -> usize {
        if let Some(&k) = self.vetor_de.get(&v) {
            return k;
        }
        let k = self.vetores.len();
        self.vetor_de.insert(v.clone(), k);
        self.vetores.push(v);
        k
    }

    /// `@df_clo_invalido` (a entrada que a chamada usa quando o valor não é
    /// closure — o runtime já deixou o `NoSuchMethodError` pendente) e os
    /// vetores constantes.
    fn emit_closures(&mut self) {
        let modulo = self.module;
        for func in &modulo.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    match inst {
                        Instruction::ConstArray(v) => {
                            self.registrar_vetor(v.clone());
                        }
                        Instruction::CallClosure { args, nomes, .. } | Instruction::CallSeletor { args, nomes, .. } => {
                            self.registrar_vetor(Self::descritor(args.len(), nomes));
                        }
                        _ => {}
                    }
                }
            }
        }
        if self.module.modo_sdk {
            // O descritor sem argumentos do `dartforge_dispatch_toString`.
            self.registrar_vetor(vec![0, 0]);
        }
        self.out.push_str("; Closures: entrada inválida e vetores constantes\n");
        self.out.push_str("define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) {\nb0:\n  ret i64 0\n}\n");
        for (k, v) in self.vetores.iter().enumerate() {
            let itens: Vec<String> = v.iter().map(|x| format!("i64 {x}")).collect();
            writeln!(
                self.out,
                "@df.arr.{k} = private unnamed_addr constant [{} x i64] [{}]",
                v.len(),
                itens.join(", ")
            )
            .unwrap();
        }
        self.out.push('\n');
    }

    /// Converte os bits i64 lidos de uma célula/ambiente (%u) para a
    /// representação `ty` do resultado `%v<v>`.
    fn bits_para_repr(&mut self, v: u32, bits: &str, ty: Type) {
        match ty {
            Type::F64 => writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap(),
            Type::I1 => writeln!(self.out, "  %v{v} = icmp ne i64 {bits}, 0").unwrap(),
            Type::I8 => writeln!(self.out, "  %v{v} = trunc i64 {bits} to i8").unwrap(),
            _ => writeln!(self.out, "  %v{v} = add i64 {bits}, 0").unwrap(),
        }
    }

    /// Tag da ABI (bits, tag) de um valor pela representação (E1).
    fn tag_de(&self, op: &Operand) -> u8 {
        match self.tipo_de(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::F64 => 4,
            _ => 3,
        }
    }

    /// Emite uma instrução das closures (P1); `false` se não é uma delas.
    fn emit_closure_inst(&mut self, v: u32, inst: &Instruction, ty: Type) -> bool {
        match inst {
            Instruction::AllocCell { value } => {
                let tag = self.tag_de(value);
                let s = self.coagir(value, Type::I64);
                writeln!(self.out, "  %v{v} = call i64 @dartforge_cell_new(i64 {s}, i8 {tag})").unwrap();
            }
            Instruction::CellGet { cell } => {
                let c = self.coagir(cell, Type::Ref);
                if ty == Type::Ref {
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_cell_get_ref(i64 {c})").unwrap();
                } else {
                    writeln!(self.out, "  %u{v} = call i64 @dartforge_cell_get_bits(i64 {c})").unwrap();
                    self.bits_para_repr(v, &format!("%u{v}"), ty);
                }
            }
            Instruction::CellSet { cell, value } => {
                let tag = self.tag_de(value);
                let c = self.coagir(cell, Type::Ref);
                let s = self.coagir(value, Type::I64);
                writeln!(self.out, "  call void @dartforge_cell_set(i64 {c}, i64 {s}, i8 {tag})").unwrap();
            }
            Instruction::EnvGet { env, index } => {
                let e = self.coagir(env, Type::Ref);
                if ty == Type::Ref {
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_env_get_ref(i64 {e}, i64 {index})").unwrap();
                } else {
                    writeln!(self.out, "  %u{v} = call i64 @dartforge_env_get(i64 {e}, i64 {index})").unwrap();
                    self.bits_para_repr(v, &format!("%u{v}"), ty);
                }
            }
            Instruction::AllocEnv { values } => {
                let n = values.len();
                if n == 0 {
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_env_new(ptr null, i64 0)").unwrap();
                } else {
                    for (i, val) in values.iter().enumerate() {
                        let tag = self.tag_de(val);
                        let s = self.coagir(val, Type::I64);
                        writeln!(self.out, "  %eb{v}_{i} = getelementptr [{} x i64], ptr %envbuf{v}, i64 0, i64 {}", n * 2, i * 2).unwrap();
                        writeln!(self.out, "  store i64 {s}, ptr %eb{v}_{i}").unwrap();
                        writeln!(self.out, "  %et{v}_{i} = getelementptr [{} x i64], ptr %envbuf{v}, i64 0, i64 {}", n * 2, i * 2 + 1).unwrap();
                        writeln!(self.out, "  store i64 {tag}, ptr %et{v}_{i}").unwrap();
                    }
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_env_new(ptr %envbuf{v}, i64 {n})").unwrap();
                }
            }
            // O código de uma closure é o endereço da entrada uniforme: vale
            // entre módulos (uma closure criada no SDK da fonte é chamada no
            // programa) e não depende da ordem de nada.
            Instruction::AllocClosure { code_symbol, env } => {
                self.anotar_externo(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
                let e = self.coagir(env, Type::Ref);
                writeln!(
                    self.out,
                    "  %v{v} = call i64 @dartforge_closure_new(i64 ptrtoint (ptr @{code_symbol} to i64), i64 {e})"
                )
                .unwrap();
            }
            Instruction::TearOff { code_symbol } => {
                self.anotar_externo(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
                writeln!(self.out, "  %v{v} = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @{code_symbol} to i64))").unwrap();
            }
            Instruction::CallClosure { closure, args, nomes, .. } => {
                let k = self.vetor_de[&Self::descritor(args.len(), nomes)];
                let n = args.len().max(1);
                for (i, a) in args.iter().enumerate() {
                    let s = self.coagir(a, Type::I64);
                    writeln!(self.out, "  %ca{v}_{i} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {i}").unwrap();
                    writeln!(self.out, "  store i64 {s}, ptr %ca{v}_{i}").unwrap();
                }
                let c = self.coagir(closure, Type::Ref);
                self.entrada_da_closure(v, &c);
                writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr %cargs{v}, ptr @df.arr.{k})").unwrap();
            }
            Instruction::CallClosureRepasse { closure, args, desc } => {
                let c = self.coagir(closure, Type::Ref);
                let a = self.operand_str(args);
                let d = self.operand_str(desc);
                self.entrada_da_closure(v, &c);
                writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr {a}, ptr {d})").unwrap();
            }
            Instruction::CallSeletor { seletor, recv, args, nomes, tupla_tipos } => {
                self.emitir_chamada_por_seletor(v, seletor, recv, args, nomes, tupla_tipos);
            }
            Instruction::LoadIndexed { base, index } => {
                let b = self.operand_str(base);
                let i = self.coagir(index, Type::I64);
                writeln!(self.out, "  %li{v} = getelementptr i64, ptr {b}, i64 {i}").unwrap();
                writeln!(self.out, "  %v{v} = load i64, ptr %li{v}").unwrap();
            }
            Instruction::ConstArray(vals) => {
                let k = self.vetor_de[vals];
                writeln!(self.out, "  %v{v} = getelementptr i64, ptr @df.arr.{k}, i64 0").unwrap();
            }
            _ => return false,
        }
        true
    }

    /// Os vetores de pilha das closures (argumentos de uma chamada, pares do
    /// ambiente) nascem no bloco de entrada: um `alloca` num laço cresceria a
    /// pilha a cada volta.
    fn emit_buffers_de_closure(&mut self, func: &Function) {
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                match inst {
                    Instruction::CallClosure { args, .. } => {
                        writeln!(self.out, "  %cargs{} = alloca [{} x i64]", vid.0, args.len().max(1)).unwrap();
                    }
                    Instruction::CallSeletor { args, .. } => {
                        writeln!(self.out, "  %sargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::AllocEnv { values } if !values.is_empty() => {
                        writeln!(self.out, "  %envbuf{} = alloca [{} x i64]", vid.0, values.len() * 2).unwrap();
                    }
                    _ => {}
                }
            }
        }
    }

    /// Os globais do módulo (`@dfg_<id>` e a bandeira `$ok`) viram slots da
    /// área de globais do isolado: a VM guarda os estáticos na *field table*
    /// de cada isolado, e aqui cada isolado (uma thread) tem a sua área, que
    /// o runtime cria na primeira vez (`dartforge_area_de_globais`, zerada).
    /// Os caches dos seletores (`seletores.rs`) ganham slots depois destes.
    fn emit_globais(&mut self) {
        for (_, _, simbolo) in &self.module.globais {
            for nome in [simbolo.clone(), format!("{simbolo}$ok")] {
                let n = self.nomes_de_slot.len();
                self.slots_de_global.insert(nome.clone(), n);
                self.nomes_de_slot.push(nome);
            }
        }
    }

    /// O descritor da área de globais do módulo: `[chave, n, nome_0…]`, com
    /// a chave estável do módulo e o hash do nome de cada slot (0 = não
    /// migra numa recarga: os caches de seletor guardam endereços de código).
    fn emitir_descritor_da_area(&mut self) {
        let chave = self.module.registro.clone().unwrap_or_else(|| "df.programa".to_string());
        let mut valores = vec![hash_de_slot(&chave).to_string(), String::new()];
        for nome in &self.nomes_de_slot {
            valores.push(hash_de_slot(nome).to_string());
        }
        for _ in 0..self.caches_de_seletor * 2 {
            valores.push("0".to_string());
        }
        valores[1] = (valores.len() - 2).to_string();
        let itens: Vec<String> = valores.iter().map(|v| format!("i64 {v}")).collect();
        writeln!(self.out, "@df.area = private unnamed_addr constant [{} x i64] [{}]", itens.len(), itens.join(", ")).unwrap();
    }

    /// O slot de um cache de seletor na área.
    pub(super) fn slot_do_cache(&self, ic: usize) -> usize {
        self.nomes_de_slot.len() + 2 * ic
    }

    /// A função usa a área de globais (global, bandeira ou cache de seletor)?
    fn usa_area(func: &Function) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(i, Instruction::LoadGlobal { .. } | Instruction::StoreGlobal { .. } | Instruction::CallSeletor { .. })
            })
        })
    }

    /// O endereço do slot de um global, em `%ga<v>`.
    fn endereco_do_global(&mut self, v: u32, simbolo: &str) {
        let slot = *self
            .slots_de_global
            .get(simbolo)
            .unwrap_or_else(|| panic!("bug do compilador: global @{simbolo} sem slot na área"));
        writeln!(self.out, "  %ga{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
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
        if self.module.modo_sdk {
            self.emitir_registro("df.registrar.programa");
            let ids: Vec<String> = self.module.cids_do_runtime.iter().map(|c| format!("i64 {c}")).collect();
            writeln!(
                self.out,
                "@df.cids = private unnamed_addr constant [{} x i64] [{}]",
                ids.len(),
                ids.join(", ")
            )
            .unwrap();
            if let Some(v) = &self.module.versao_do_sdk {
                writeln!(
                    self.out,
                    "@df.versao_do_sdk = private unnamed_addr constant [{} x i8] c\"{}\"",
                    v.len(),
                    seletores::bytes_llvm(v)
                )
                .unwrap();
            }
            writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
            writeln!(self.out, "  call void @dartforge_registrar_cids(ptr @df.cids, i64 {})", ids.len()).unwrap();
            if let Some(v) = &self.module.versao_do_sdk {
                writeln!(self.out, "  call void @dartforge_registrar_versao_do_sdk(ptr @df.versao_do_sdk, i64 {})", v.len()).unwrap();
            }
            // As tabelas das classes dos valores do runtime (que não passam
            // por `dartforge_object_new_t`).
            for c in self.module.cids_do_runtime.clone() {
                if let Some(f) = self.module.funcoes_de_tabela.get(&(c as u32)).cloned() {
                    self.anotar_externo(&f, Type::Ptr, &[]);
                    writeln!(self.out, "  call void @dartforge_registrar_tabela(i64 {c}, ptr @{f})").unwrap();
                }
            }
            for r in &self.module.registros_do_sdk {
                writeln!(self.out, "  call void @{r}()").unwrap();
                self.externos.insert(r.clone(), format!("declare void @{r}()"));
            }
            writeln!(self.out, "  call void @df.registrar.programa()").unwrap();
            // RTI e laço de eventos, como na entrada de sempre (abaixo).
            if let Some(iniciar) = &self.module.iniciar_rti {
                writeln!(self.out, "  call void @{iniciar}()").unwrap();
            }
            // O que o embedder da VM prepara antes do `main` (o script de
            // `dart:io`, o `Uri.base`), já com as bibliotecas registradas.
            writeln!(self.out, "  call void @dartforge_preparar_embedder()").unwrap();
            if let Some(entry) = &self.module.entry_symbol {
                writeln!(self.out, "  call void @{entry}()").unwrap();
            }
            if let Some(chamar) = &self.module.chamar_dart {
                writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{chamar})").unwrap();
            }
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            // O runtime e o SDK moram na DLL do SDK da fonte: o `main` do
            // executável é este, e entrega a entrada ao runtime.
            writeln!(
                self.out,
                "define i32 @main() {{\n  %r = call i32 @dartforge_iniciar(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}}\n"
            )
            .unwrap();
            return;
        }
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        // Registra classes
        for class in &self.module.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
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

        // RTI: o universo de tipos (classes citadas e regras de supertipo).
        if let Some(iniciar) = &self.module.iniciar_rti {
            writeln!(self.out, "  call void @{iniciar}()").unwrap();
        }
        if let Some(entry) = &self.module.entry_symbol {
            writeln!(self.out, "  call void @{entry}()").unwrap();
        }
        // P6: microtarefas e timers depois do `main` (runtime, `eventos.rs`).
        if let Some(chamar) = &self.module.chamar_dart {
            writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{chamar})").unwrap();
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
    pub(crate) fn tipo_do_resultado(inst: &Instruction, registrado: Type) -> Type {
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
            | Instruction::Box { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::TearOff { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallSeletor { .. }
            | Instruction::CallClosureRepasse { .. } => Type::Ref,
            Instruction::CellSet { .. } => Type::Void,
            Instruction::ConstArray(_) => Type::Ptr,
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
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
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
            (Constant::String(_) | Constant::StringWtf8(_), _) => panic!("string deve ser carregada via Instruction::Const"),
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
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => {
                panic!("string deve ser carregada via Instruction::Const");
            }
        }
    }
}

/// O hash (FNV-1a de 64 bits) do nome de um slot ou da chave do módulo na
/// área de globais; nunca 0, que marca o slot que não migra.
fn hash_de_slot(nome: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in nome.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (if h == 0 { 1 } else { h }) as i64
}
