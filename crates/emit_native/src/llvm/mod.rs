//! Gerador de LLVM IR a partir da HIR nativa.

pub mod abi_c;
pub mod externs;
mod raizes;

/// Maior índice de campo lido em linha (`CAMPOS_EM_LINHA` do runtime).
const CAMPOS_EM_LINHA: usize = 4096;
mod seletores;
#[cfg(test)]
mod testes;

use crate::hir::*;
use std::fmt::Write;

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    out: String,
    /// As constantes de string, na ordem de emissão (determinística); o
    /// índice de cada uma é a posição.
    string_constants: Vec<Vec<u8>>,
    /// O índice de cada constante pelo conteúdo: coletar e consultar sem
    /// percorrer a tabela (antes, quadrático no número de literais).
    indice_de_string: std::collections::HashMap<Vec<u8>, usize>,
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
    /// A função abriu um quadro de raízes (`%gcq`).
    tem_frame: bool,
    // --- P1 (closures, α) ---
    /// Vetores constantes de `i64` (`@df.arr.<k>`): assinaturas e descritores.
    vetores: Vec<Vec<i64>>,
    vetor_de: std::collections::HashMap<Vec<i64>, usize>,
    /// Os nomes dos argumentos nomeados dos descritores do módulo: o
    /// descritor só guarda o hash, e o `Invocation` de um `noSuchMethod`
    /// precisa do nome (`dartforge_registrar_nome_de_argumento`).
    nomes_de_argumento: std::collections::BTreeSet<String>,
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
    /// O hash do nome de cada slot, na ordem (o descritor da área): 0 nos
    /// caches de seletor.
    hashes_de_slot: Vec<i64>,
    /// O layout da área da geração em execução (hot reload): os slots que
    /// continuam mantêm o índice, os novos vêm depois (`emit_globais`).
    area_anterior: Option<Vec<i64>>,
}

impl<'a> LlvmEmitter<'a> {
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            string_constants: Vec::new(),
            indice_de_string: std::collections::HashMap::new(),
            tipos: std::collections::HashMap::new(),
            prox_coercao: 0,
            conv_phi: Vec::new(),
            apontado: std::collections::HashMap::new(),
            slots: std::collections::HashMap::new(),
            tem_frame: false,

            vetores: Vec::new(),
            vetor_de: std::collections::HashMap::new(),
            nomes_de_argumento: std::collections::BTreeSet::new(),
            caches_de_seletor: 0,
            nomes_de_seletor: Vec::new(),
            externos: std::collections::BTreeMap::new(),
            comdats: Vec::new(),
            rastro: std::env::var("DARTFORGE_RASTRO").is_ok_and(|v| v == "1").then_some(0),
            nomes_do_rastro: Vec::new(),
            slots_de_global: std::collections::HashMap::new(),
            hashes_de_slot: Vec::new(),
            area_anterior: None,
        }
    }

    /// O emissor de uma geração nova de um programa em execução (hot reload
    /// do JIT): `anterior` são os nomes dos slots da área da geração viva
    /// ([`crate::area_do_ir`]). Os globais que continuam mantêm o índice e
    /// os novos vêm depois, então o código das duas gerações lê e grava a
    /// mesma área — os quadros `async` suspensos e as closures que ainda
    /// executam código antigo veem os mesmos estáticos que o código novo.
    pub fn com_area_anterior(mut self, anterior: Option<Vec<i64>>) -> Self {
        self.area_anterior = anterior;
        self
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
        // A área deste isolado com o layout desta geração, criada (ou
        // estendida) agora: a publicação de uma recarga do JIT a chama no
        // ponto seguro, antes de o código novo executar.
        self.out.push_str(
            "define void @df.preparar_area() {\n  %a = call ptr @dartforge_area_de_globais(ptr @df.area)\n  ret void\n}\n",
        );
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
                        Self::registrar_string(&mut self.string_constants, &mut self.indice_de_string, bytes);
                    }
                }
            }
        }
        for class in &self.module.classes {
            Self::registrar_string(&mut self.string_constants, &mut self.indice_de_string, class.name.as_bytes());
        }
    }

    /// Acrescenta `bytes` à tabela de constantes, se ainda não estiver.
    fn registrar_string(tabela: &mut Vec<Vec<u8>>, indice: &mut std::collections::HashMap<Vec<u8>, usize>, bytes: &[u8]) {
        if !indice.contains_key(bytes) {
            indice.insert(bytes.to_vec(), tabela.len());
            tabela.push(bytes.to_vec());
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
        // As chamadas nativas com struct por valor copiam os bytes numa
        // temporária da pilha (`llvm/abi_c.rs`).
        let compostas = self.module.functions.iter().any(|f| {
            f.blocks.iter().any(|b| b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::ChamadaNativaComposta { .. })))
        });
        if compostas || !self.module.ffi_callbacks.is_empty() {
            self.out.push_str(
                "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                 declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                 declare ptr @llvm.stacksave.p0()\n\
                 declare void @llvm.stackrestore.p0(ptr)\n",
            );
        }
        self.out.push('\n');
    }

    fn emit_string_constants(&mut self) {
        if self.string_constants.is_empty() {
            return;
        }
        self.out.push_str("; Constantes de string WTF-8\n");
        for (idx, s) in self.string_constants.iter().enumerate() {
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
        self.indice_de_string.get(s).copied()
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

        // G1/G2 (docs/NATIVO-PLANO.md §6.5): um slot por `alloca` de tipo
        // `Ref`, e os valores SSA `Ref` vivos em algum ponto de coleta, com
        // slot compartilhado entre os que nunca estão vivos juntos
        // (`raizes.rs`).
        let blocos_que_convertem: std::collections::HashSet<u32> = self.conv_phi.iter().map(|(b, ..)| *b).collect();
        self.slots = raizes::atribuir_slots(
            func,
            &self.tipos,
            &|inst| self.pode_coletar(inst),
            &|b| blocos_que_convertem.contains(&b.0),
        );
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
                // O quadro de raízes no stack da função (a pilha-sombra,
                // `QuadroDeRaizes` do runtime): anterior, número de slots e
                // os slots, zerados antes de o runtime encadeá-lo. Cada raiz
                // é um `store` no slot dela (slots compartilhados: o tamanho
                // é o do maior).
                let n = self.slots.values().max().map_or(0, |m| m + 1);
                let t = format!("{{ ptr, i64, [{n} x i64] }}");
                writeln!(self.out, "  %gcq = alloca {t}, align 8").unwrap();
                writeln!(self.out, "  store {t} {{ ptr null, i64 {n}, [{n} x i64] zeroinitializer }}, ptr %gcq").unwrap();
                for slot in 0..n {
                    writeln!(self.out, "  %gcs{slot} = getelementptr inbounds {t}, ptr %gcq, i64 0, i32 2, i64 {slot}").unwrap();
                }
                writeln!(self.out, "  call void @dartforge_gc_empilhar(ptr %gcq)").unwrap();
                for (vid, _, _) in &func.params {
                    if let Some(&slot) = self.slots.get(vid) {
                        writeln!(self.out, "  store i64 %v{}, ptr %gcs{slot}", vid.0).unwrap();
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
                        writeln!(self.out, "  store i64 %v{pv}, ptr %gcs{slot}").unwrap();
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
                            ICmpOp::Ult => "ult",
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
                    // O campo em linha: o par `(bits, is_ref)` de 16 bytes no
                    // endereço que `dartforge_object_campos` dá (o layout é
                    // conferido no runtime, `nucleo.rs`).
                    Instruction::GetField { object, index } if (*index as usize) < CAMPOS_EM_LINHA => {
                        let so = self.coagir(object, Type::I64);
                        writeln!(self.out, "  %fc{v} = call i64 @dartforge_object_campos(i64 {so})").unwrap();
                        writeln!(self.out, "  %fp{v} = inttoptr i64 %fc{v} to ptr").unwrap();
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {index}, i32 0").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::GetField { object, index } => {
                        let so = self.coagir(object, Type::I64);
                        writeln!(
                            self.out,
                            "  %v{v} = call i64 @dartforge_object_get(i64 {so}, i64 {index})"
                        ).unwrap();
                    }
                    Instruction::SetField { object, index, value } if (*index as usize) < CAMPOS_EM_LINHA => {
                        let is_ref = u8::from(self.tipo_de(value) == Type::Ref);
                        let so = self.coagir(object, Type::I64);
                        let sv = self.coagir(value, Type::I64);
                        writeln!(self.out, "  %fc{v} = call i64 @dartforge_object_campos(i64 {so})").unwrap();
                        writeln!(self.out, "  %fp{v} = inttoptr i64 %fc{v} to ptr").unwrap();
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {index}, i32 0").unwrap();
                        writeln!(self.out, "  store i64 {sv}, ptr %fg{v}, align 8").unwrap();
                        writeln!(self.out, "  %fr{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {index}, i32 1").unwrap();
                        writeln!(self.out, "  store i8 {is_ref}, ptr %fr{v}, align 8").unwrap();
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
                    // Campo com índice constante: em linha, como `GetField`.
                    Instruction::CallRuntime { name, args, .. }
                        if name == "dartforge_object_get"
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let so = self.coagir(&args[0].0, Type::I64);
                        writeln!(self.out, "  %fc{v} = call i64 @dartforge_object_campos(i64 {so})").unwrap();
                        writeln!(self.out, "  %fp{v} = inttoptr i64 %fc{v} to ptr").unwrap();
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {i}, i32 0").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::CallRuntime { name, args, .. }
                        if name == "dartforge_object_set"
                            && args.len() == 4
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..CAMPOS_EM_LINHA as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let so = self.coagir(&args[0].0, Type::I64);
                        let sv = self.coagir(&args[2].0, Type::I64);
                        // `is_ref` é um `bool` do Rust: só 0 ou 1.
                        let is_ref = match &args[3].0 {
                            Operand::Constant(Constant::Int(n)) => u8::from(*n != 0).to_string(),
                            Operand::Constant(Constant::Bool(b)) => u8::from(*b).to_string(),
                            outro => {
                                let x = self.coagir(outro, Type::I64);
                                writeln!(self.out, "  %fb{v} = icmp ne i64 {x}, 0").unwrap();
                                writeln!(self.out, "  %fz{v} = zext i1 %fb{v} to i8").unwrap();
                                format!("%fz{v}")
                            }
                        };
                        writeln!(self.out, "  %fc{v} = call i64 @dartforge_object_campos(i64 {so})").unwrap();
                        writeln!(self.out, "  %fp{v} = inttoptr i64 %fc{v} to ptr").unwrap();
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {i}, i32 0").unwrap();
                        writeln!(self.out, "  store i64 {sv}, ptr %fg{v}, align 8").unwrap();
                        writeln!(self.out, "  %fr{v} = getelementptr inbounds {{ i64, i8 }}, ptr %fp{v}, i64 {i}, i32 1").unwrap();
                        writeln!(self.out, "  store i8 {is_ref}, ptr %fr{v}, align 8").unwrap();
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
                        if (name == "dartforge_typed_novo" || name == "dartforge_view_nova" || name == "dartforge_typed_externo")
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
                    Instruction::ChamadaNativa { alvo, args, ret } => {
                        // A chamada C: cada argumento convertido ao tipo C
                        // (estreitos com a extensão da ABI), o retorno
                        // estendido de volta explicitamente (o Windows x64
                        // não garante os bits altos de um retorno estreito).
                        let a = self.coagir(alvo, Type::I64);
                        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
                        let mut partes = Vec::with_capacity(args.len());
                        for (i, (op, tc)) in args.iter().enumerate() {
                            partes.push(self.argumento_c(v, i, op, *tc));
                        }
                        let lista = partes.join(", ");
                        let conv = match ret {
                            TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {} %nr{v} to i64", ret.llvm())),
                            TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {} %nr{v} to i64", ret.llvm())),
                            TipoC::F32 => Some(format!("fpext float %nr{v} to double")),
                            TipoC::Ptr | TipoC::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                            TipoC::I64 | TipoC::U64 | TipoC::F64 | TipoC::Bool | TipoC::Void => None,
                        };
                        match (ret, conv) {
                            (TipoC::Void, _) => writeln!(self.out, "  call void %fn{v}({lista})").unwrap(),
                            (_, None) => writeln!(self.out, "  %v{v} = call {} %fn{v}({lista})", ret.llvm()).unwrap(),
                            (_, Some(c)) => {
                                writeln!(self.out, "  %nr{v} = call {} %fn{v}({lista})", ret.llvm()).unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                        }
                    }
                    Instruction::CargaNativa { endereco, indice, tipo } => {
                        // Sem alinhamento suposto (os bytes de uma lista
                        // tipada são de um `Vec<u8>`).
                        let e = self.coagir(endereco, Type::I64);
                        let i = self.coagir(indice, Type::I64);
                        let t = tipo.llvm();
                        writeln!(self.out, "  %cp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %cg{v} = getelementptr {t}, ptr %cp{v}, i64 {i}").unwrap();
                        let conv = match tipo {
                            TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {t} %cl{v} to i64")),
                            TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {t} %cl{v} to i64")),
                            TipoC::F32 => Some(format!("fpext float %cl{v} to double")),
                            _ => None,
                        };
                        match conv {
                            Some(c) => {
                                writeln!(self.out, "  %cl{v} = load {t}, ptr %cg{v}, align 1").unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                            None => writeln!(self.out, "  %v{v} = load {t}, ptr %cg{v}, align 1").unwrap(),
                        }
                    }
                    Instruction::GravacaoNativa { endereco, indice, tipo, valor } => {
                        let e = self.coagir(endereco, Type::I64);
                        let i = self.coagir(indice, Type::I64);
                        let t = tipo.llvm();
                        let x = match tipo {
                            TipoC::F32 => {
                                let d = self.coagir(valor, Type::F64);
                                writeln!(self.out, "  %gx{v} = fptrunc double {d} to float").unwrap();
                                format!("%gx{v}")
                            }
                            TipoC::F64 => self.coagir(valor, Type::F64),
                            TipoC::I64 | TipoC::U64 => self.coagir(valor, Type::I64),
                            _ => {
                                let n = self.coagir(valor, Type::I64);
                                writeln!(self.out, "  %gx{v} = trunc i64 {n} to {t}").unwrap();
                                format!("%gx{v}")
                            }
                        };
                        writeln!(self.out, "  %gp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %gg{v} = getelementptr {t}, ptr %gp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  store {t} {x}, ptr %gg{v}, align 1").unwrap();
                    }
                    Instruction::ChamadaNativaComposta { alvo, args, ret, destino, variadica } => {
                        self.chamada_nativa_composta(v, alvo, args, ret, destino.as_ref(), *variadica);
                    }
                    Instruction::AllocList { elements } => {
                        // Alloca temporário para pares (bits, tag)
                        let count = elements.len();
                        let alloca_id = format!("list_buf_{v}");
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
                    // O `alloca` nasce no bloco de entrada (`emit_buffers_de_closure`).
                    Instruction::Alloca(_) => {}
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
                            writeln!(self.out, "  store i64 {sv}, ptr %gcs{slot}").unwrap();
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
                        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
                    }
                }
                let _ = ty;
            }
            for (slot, pv) in std::mem::take(&mut raizes_de_phi) {
                writeln!(self.out, "  store i64 %v{pv}, ptr %gcs{slot}").unwrap();
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
                writeln!(self.out, "  call void @dartforge_gc_desempilhar(ptr %gcq)").unwrap();
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
                            self.nomes_de_argumento.extend(nomes.iter().cloned());
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

    /// Todo `alloca` da função nasce no bloco de entrada — os vetores das
    /// closures (argumentos, pares do ambiente), os dos literais de lista,
    /// mapa e record, e os locais: um `alloca` fora da entrada reserva pilha
    /// nova a cada execução, e num laço a pilha cresce até estourar.
    fn emit_buffers_de_closure(&mut self, func: &Function) {
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                match inst {
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{} = alloca {}", vid.0, ty.llvm_ir()).unwrap();
                    }
                    Instruction::AllocList { elements } => {
                        writeln!(self.out, "  %list_buf_{} = alloca [{} x i64]", vid.0, elements.len() * 2).unwrap();
                    }
                    Instruction::AllocMap { entries } => {
                        writeln!(self.out, "  %map_k_{} = alloca [{} x i64]", vid.0, entries.len() * 2).unwrap();
                        writeln!(self.out, "  %map_v_{} = alloca [{} x i64]", vid.0, entries.len() * 2).unwrap();
                    }
                    Instruction::AllocRecord { elements } => {
                        writeln!(self.out, "  %rec_buf_{} = alloca [{} x i64]", vid.0, elements.len() * 2).unwrap();
                    }
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
    ///
    /// Numa geração de hot reload ([`LlvmEmitter::com_area_anterior`]), a
    /// área começa com o layout da geração viva inteiro — inclusive os slots
    /// que sumiram e os caches de seletor dela, que o código antigo ainda
    /// usa — e só acrescenta: um global que continua fica no mesmo índice.
    fn emit_globais(&mut self) {
        let anterior: std::collections::HashMap<i64, usize> = match &self.area_anterior {
            Some(a) => {
                self.hashes_de_slot = a.clone();
                a.iter().enumerate().filter(|(_, h)| **h != 0).map(|(i, h)| (*h, i)).collect()
            }
            None => std::collections::HashMap::new(),
        };
        for (_, _, simbolo) in &self.module.globais {
            for nome in [simbolo.clone(), format!("{simbolo}$ok")] {
                let h = hash_de_slot(&nome);
                let n = match anterior.get(&h) {
                    Some(&i) => i,
                    None => {
                        self.hashes_de_slot.push(h);
                        self.hashes_de_slot.len() - 1
                    }
                };
                self.slots_de_global.insert(nome, n);
            }
        }
    }

    /// O descritor da área de globais do módulo: `[chave, n, nome_0…]`, com
    /// a chave estável do módulo e o hash do nome de cada slot (0 = não
    /// migra numa recarga: os caches de seletor guardam endereços de código).
    fn emitir_descritor_da_area(&mut self) {
        let chave = self.module.registro.clone().unwrap_or_else(|| "df.programa".to_string());
        let mut valores = vec![hash_de_slot(&chave).to_string(), String::new()];
        for h in &self.hashes_de_slot {
            valores.push(h.to_string());
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
        self.hashes_de_slot.len() + 2 * ic
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
            // A preparação de um isolado (a principal e a de cada
            // `Isolate.spawn`, que o runtime chama na thread nova): os
            // registros das bibliotecas, a RTI e o embedder.
            writeln!(self.out, "define void @df.preparar_isolado() {{").unwrap();
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
            // `dart:ffi`: os tipos nativos e os trampolins das assinaturas
            // (`lower/ffi.rs`).
            for c in self.module.ffi_compostos.clone() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_composto(i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {})",
                    c.rti, c.classe, c.campos, c.indice_base, c.indice_deslocamento, c.tamanho, c.alinhamento
                )
                .unwrap();
            }
            for (c, letra) in &self.module.ffi_tipos {
                writeln!(self.out, "  call void @dartforge_ffi_registrar_tipo(i64 {c}, i64 {})", u32::from(*letra)).unwrap();
            }
            for (i, (chave, simbolo)) in self.module.ffi_trampolins.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_trampolim(ptr @df.ffi.chave.{i}, i64 {}, ptr @\"{simbolo}\")",
                    chave.len()
                )
                .unwrap();
            }
            for (i, cb) in self.module.ffi_callbacks.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_registrar_callback(ptr @df.ffi.cbchave.{i}, i64 {}, ptr @df.ffi.cbentrada.{i})",
                    cb.chave.len()
                )
                .unwrap();
            }
            writeln!(self.out, "  call void @dartforge_preparar_embedder()").unwrap();
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            for (i, (chave, _)) in self.module.ffi_trampolins.iter().enumerate() {
                writeln!(self.out, "@df.ffi.chave.{i} = private unnamed_addr constant [{} x i8] c\"{chave}\"", chave.len()).unwrap();
            }
            self.emit_callbacks_ffi();
            writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
            let chamar = self.module.chamar_dart.as_ref().map_or("null".to_string(), |c| format!("@{c}"));
            writeln!(self.out, "  call void @dartforge_registrar_isolados(ptr @df.preparar_isolado, ptr {chamar})").unwrap();
            writeln!(self.out, "  call void @df.preparar_isolado()").unwrap();
            self.chamar_main();
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
        self.chamar_main();
        // P6: microtarefas e timers depois do `main` (runtime, `eventos.rs`).
        if let Some(chamar) = &self.module.chamar_dart {
            writeln!(self.out, "  call void @dartforge_laco_de_eventos(ptr @{chamar})").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    /// A chamada do `main` na entrada: com parâmetros, o primeiro é a lista
    /// dos argumentos da linha de comando (`dartforge_argumentos_do_main`) e
    /// o segundo, `null`.
    /// Um argumento primitivo de chamada C: o operando convertido ao tipo C
    /// (estreitos com a extensão da ABI), como `tipo atributos valor`.
    fn argumento_c(&mut self, v: u32, i: usize, op: &Operand, tc: TipoC) -> String {
        let x = match tc {
            TipoC::F32 => {
                let d = self.coagir(op, Type::F64);
                writeln!(self.out, "  %na{v}_{i} = fptrunc double {d} to float").unwrap();
                format!("%na{v}_{i}")
            }
            TipoC::F64 => self.coagir(op, Type::F64),
            TipoC::Bool => self.coagir(op, Type::I1),
            TipoC::Ptr | TipoC::Handle => {
                let n = self.coagir(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                format!("%na{v}_{i}")
            }
            TipoC::I64 | TipoC::U64 => self.coagir(op, Type::I64),
            TipoC::Void => unreachable!("argumento nativo void"),
            estreito => {
                let n = self.coagir(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = trunc i64 {n} to {}", estreito.llvm()).unwrap();
                format!("%na{v}_{i}")
            }
        };
        format!("{} {}{x}", tc.llvm(), tc.extensao())
    }

    /// Uma chamada C com structs/unions por valor (`llvm/abi_c.rs`): cada
    /// composto é copiado dos bytes do operando para uma temporária alinhada
    /// da pilha (liberada por `stackrestore` depois da chamada), e passado
    /// em peças, `byval` ou por ponteiro; um retorno composto vai para
    /// `destino` (direto: pelas peças gravadas numa temporária; `sret`: o
    /// próprio destino).
    fn chamada_nativa_composta(
        &mut self,
        v: u32,
        alvo: &Operand,
        args: &[(Operand, TipoNativo)],
        ret: &TipoNativo,
        destino: Option<&Operand>,
        variadica: Option<usize>,
    ) {
        use abi_c::{PassagemArg, PassagemRet};
        let conv = abi_c::Convencao::do_alvo();
        let mut regs = abi_c::Registradores::novos();
        let a = self.coagir(alvo, Type::I64);
        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
        writeln!(self.out, "  %pilha{v} = call ptr @llvm.stacksave.p0()").unwrap();
        // Retorno primeiro: um `sret` ocupa o primeiro registrador inteiro.
        let passagem_ret = match ret {
            TipoNativo::Composto(l) => Some((abi_c::retorno(conv, l, &mut regs), l.clone())),
            TipoNativo::Prim(_) => None,
        };
        let mut partes = Vec::with_capacity(args.len() + 1);
        let destino_ptr = destino.map(|d| {
            let d = self.coagir(d, Type::I64);
            writeln!(self.out, "  %dst{v} = inttoptr i64 {d} to ptr").unwrap();
            format!("%dst{v}")
        });
        if let (Some((PassagemRet::Sret { alinhamento }, l)), Some(d)) = (&passagem_ret, &destino_ptr) {
            partes.push(format!("ptr sret([{} x i8]) align {alinhamento} {d}", l.tamanho));
        }
        let mut tipos_fixos: Vec<String> = Vec::new();
        if matches!(&passagem_ret, Some((PassagemRet::Sret { .. }, _))) {
            tipos_fixos.push("ptr".to_string());
        }
        for (i, (op, t)) in args.iter().enumerate() {
            let variadico = variadica.is_some_and(|n| i >= n);
            match t {
                // Promoções de argumento padrão do C num argumento variádico:
                // `float` vai como `double`; inteiros menores que `int`,
                // como `int` (com o sinal do tipo).
                TipoNativo::Prim(tc) if variadico => {
                    regs.consumir_primitivo(*tc);
                    let p = match tc {
                        TipoC::F32 => self.argumento_c(v, i, op, TipoC::F64),
                        TipoC::I8 | TipoC::I16 => self.argumento_c(v, i, op, TipoC::I32),
                        TipoC::U8 | TipoC::U16 | TipoC::Bool => {
                            let n = self.coagir(op, if *tc == TipoC::Bool { Type::I1 } else { Type::I64 });
                            let ext = if *tc == TipoC::Bool { format!("zext i1 {n} to i32") } else { format!("trunc i64 {n} to i32") };
                            writeln!(self.out, "  %na{v}_{i} = {ext}").unwrap();
                            format!("i32 %na{v}_{i}")
                        }
                        _ => self.argumento_c(v, i, op, *tc),
                    };
                    partes.push(p);
                }
                TipoNativo::Prim(tc) => {
                    regs.consumir_primitivo(*tc);
                    let p = self.argumento_c(v, i, op, *tc);
                    tipos_fixos.push(p.split(' ').next().unwrap_or("i64").to_string());
                    partes.push(p);
                }
                TipoNativo::Composto(l) => {
                    // A cópia: alinhada, zerada até a palavra, com os bytes.
                    let n = self.coagir(op, Type::I64);
                    let tam = l.tamanho.div_ceil(16) * 16;
                    let al = l.alinhamento.max(16);
                    writeln!(self.out, "  %src{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                    writeln!(self.out, "  %tmp{v}_{i} = alloca [{tam} x i8], align {al}").unwrap();
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %tmp{v}_{i}, i8 0, i64 {tam}, i1 false)").unwrap();
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %tmp{v}_{i}, ptr %src{v}_{i}, i64 {}, i1 false)", l.tamanho).unwrap();
                    match abi_c::argumento(conv, l, &mut regs) {
                        PassagemArg::Direta(pecas) => {
                            for (k, p) in pecas.iter().enumerate() {
                                writeln!(self.out, "  %pp{v}_{i}_{k} = getelementptr i8, ptr %tmp{v}_{i}, i64 {}", p.deslocamento).unwrap();
                                writeln!(self.out, "  %pc{v}_{i}_{k} = load {}, ptr %pp{v}_{i}_{k}, align 1", p.tipo).unwrap();
                                partes.push(format!("{} {}%pc{v}_{i}_{k}", p.tipo, p.atributos));
                                tipos_fixos.push(p.tipo.clone());
                            }
                        }
                        PassagemArg::Byval { alinhamento } => {
                            partes.push(format!("ptr byval([{} x i8]) align {alinhamento} %tmp{v}_{i}", l.tamanho));
                            tipos_fixos.push("ptr".to_string());
                        }
                        PassagemArg::Indireta => {
                            partes.push(format!("ptr %tmp{v}_{i}"));
                            tipos_fixos.push("ptr".to_string());
                        }
                    }
                }
            }
        }
        let lista = partes.join(", ");
        // O tipo da chamada: numa variádica, `ret (fixos, ...)`.
        let tipo_da_chamada = |ret: &str| match variadica {
            Some(_) if tipos_fixos.is_empty() => format!("{ret} (...)"),
            Some(_) => format!("{ret} ({}, ...)", tipos_fixos.join(", ")),
            None => ret.to_string(),
        };
        match (ret, &passagem_ret) {
            (TipoNativo::Prim(TipoC::Void), _) => writeln!(self.out, "  call {} %fn{v}({lista})", tipo_da_chamada("void")).unwrap(),
            (TipoNativo::Prim(tc), _) => {
                let conv_ret = match tc {
                    TipoC::I8 | TipoC::I16 | TipoC::I32 => Some(format!("sext {} %nr{v} to i64", tc.llvm())),
                    TipoC::U8 | TipoC::U16 | TipoC::U32 => Some(format!("zext {} %nr{v} to i64", tc.llvm())),
                    TipoC::F32 => Some(format!("fpext float %nr{v} to double")),
                    TipoC::Ptr | TipoC::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                    _ => None,
                };
                match conv_ret {
                    Some(c) => {
                        writeln!(self.out, "  %nr{v} = call {} %fn{v}({lista})", tipo_da_chamada(tc.llvm())).unwrap();
                        writeln!(self.out, "  %v{v} = {c}").unwrap();
                    }
                    None => writeln!(self.out, "  %v{v} = call {} %fn{v}({lista})", tipo_da_chamada(tc.llvm())).unwrap(),
                }
            }
            (TipoNativo::Composto(_), Some((PassagemRet::Sret { .. }, _))) => {
                writeln!(self.out, "  call {} %fn{v}({lista})", tipo_da_chamada("void")).unwrap();
            }
            (TipoNativo::Composto(_), Some((PassagemRet::Direta(pecas), l))) => {
                let tipo = abi_c::tipo_do_retorno(pecas);
                let tam = l.tamanho.div_ceil(16) * 16;
                writeln!(self.out, "  %rr{v} = call {} %fn{v}({lista})", tipo_da_chamada(&tipo)).unwrap();
                writeln!(self.out, "  %rt{v} = alloca [{tam} x i8], align 16").unwrap();
                for (k, p) in pecas.iter().enumerate() {
                    let val = if pecas.len() == 1 {
                        format!("%rr{v}")
                    } else {
                        writeln!(self.out, "  %re{v}_{k} = extractvalue {tipo} %rr{v}, {k}").unwrap();
                        format!("%re{v}_{k}")
                    };
                    writeln!(self.out, "  %rp{v}_{k} = getelementptr i8, ptr %rt{v}, i64 {}", p.deslocamento).unwrap();
                    writeln!(self.out, "  store {} {val}, ptr %rp{v}_{k}, align 1", p.tipo).unwrap();
                }
                if let Some(d) = &destino_ptr {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr {d}, ptr %rt{v}, i64 {}, i1 false)", l.tamanho).unwrap();
                }
            }
            (TipoNativo::Composto(_), None) => unreachable!("retorno composto sem passagem"),
        }
        writeln!(self.out, "  call void @llvm.stackrestore.p0(ptr %pilha{v})").unwrap();
    }

    /// As entradas C dos callbacks do `dart:ffi` (`ffi_callbacks.rs` do
    /// runtime). Cada entrada tem a ABI C da assinatura e o contexto do
    /// callback no parâmetro `nest` (r10 no x86-64, x15 no AArch64), que o
    /// trampolim escrito pelo runtime carrega: no modo ouvinte copia
    /// os argumentos para a mensagem; senão converte-os para a
    /// representação Dart, chama o corpo HIR e converte o retorno — ou
    /// devolve o retorno excepcional, se a closure lançou.
    fn emit_callbacks_ffi(&mut self) {
        use abi_c::{PassagemArg, PassagemRet};
        if self.module.ffi_callbacks.is_empty() {
            return;
        }
        let conv = abi_c::Convencao::do_alvo();
        for (i, cb) in self.module.ffi_callbacks.clone().iter().enumerate() {
            writeln!(self.out, "@df.ffi.cbchave.{i} = private unnamed_addr constant [{} x i8] c\"{}\"", cb.chave.len(), cb.chave).unwrap();
            let mut regs = abi_c::Registradores::novos();
            // O retorno primeiro: um `sret` ocupa o primeiro registrador.
            let ret = match &cb.ret {
                TipoNativo::Composto(l) => Some((abi_c::retorno(conv, l, &mut regs), l.clone())),
                TipoNativo::Prim(_) => None,
            };
            let mut decls = vec!["ptr nest %ctx".to_string()];
            if let Some((PassagemRet::Sret { alinhamento }, l)) = &ret {
                decls.push(format!("ptr sret([{} x i8]) align {alinhamento} %sret", l.tamanho));
            }
            // Como cada composto chega: em peças (remontadas numa
            // temporária) ou já em memória (`byval`/ponteiro).
            let mut pecas_de: Vec<Option<(Vec<abi_c::Peca>, usize)>> = Vec::with_capacity(cb.params.len());
            for (j, t) in cb.params.iter().enumerate() {
                match t {
                    TipoNativo::Prim(tc) => {
                        regs.consumir_primitivo(*tc);
                        decls.push(format!("{} {}%a{j}", tc.llvm(), tc.extensao()));
                        pecas_de.push(None);
                    }
                    TipoNativo::Composto(l) => match abi_c::argumento(conv, l, &mut regs) {
                        PassagemArg::Direta(pecas) => {
                            for (k, p) in pecas.iter().enumerate() {
                                decls.push(format!("{} {}%a{j}_{k}", p.tipo, p.atributos));
                            }
                            pecas_de.push(Some((pecas, l.tamanho)));
                        }
                        PassagemArg::Byval { alinhamento } => {
                            decls.push(format!("ptr byval([{} x i8]) align {alinhamento} %a{j}", l.tamanho));
                            pecas_de.push(None);
                        }
                        PassagemArg::Indireta => {
                            decls.push(format!("ptr %a{j}"));
                            pecas_de.push(None);
                        }
                    },
                }
            }
            let (tipo_ret, ret_ext) = match (&cb.ret, &ret) {
                (TipoNativo::Prim(tc), _) => (
                    tc.llvm().to_string(),
                    match tc {
                        TipoC::I8 | TipoC::I16 => "signext ",
                        TipoC::U8 | TipoC::U16 | TipoC::Bool => "zeroext ",
                        _ => "",
                    },
                ),
                (_, Some((PassagemRet::Direta(pecas), _))) => (abi_c::tipo_do_retorno(pecas), ""),
                _ => ("void".to_string(), ""),
            };
            let r = tipo_ret.as_str();
            writeln!(self.out, "define internal {ret_ext}{r} @df.ffi.cbentrada.{i}({}) {{", decls.join(", ")).unwrap();
            let n = cb.params.len().max(1);
            writeln!(self.out, "entrada:\n  %saida = alloca i64\n  %buf = alloca [{n} x i64]").unwrap();
            if let Some((_, l)) = &ret {
                writeln!(self.out, "  %rt = alloca [{} x i8], align 16", l.tamanho.div_ceil(16) * 16).unwrap();
            }
            // O endereço de cada composto (`%end{j}`, i64).
            for (j, t) in cb.params.iter().enumerate() {
                let TipoNativo::Composto(_) = t else { continue };
                match &pecas_de[j] {
                    Some((pecas, tamanho)) => {
                        writeln!(self.out, "  %m{j} = alloca [{} x i8], align 16", tamanho.div_ceil(16) * 16).unwrap();
                        for (k, p) in pecas.iter().enumerate() {
                            writeln!(self.out, "  %mp{j}_{k} = getelementptr i8, ptr %m{j}, i64 {}", p.deslocamento).unwrap();
                            writeln!(self.out, "  store {} %a{j}_{k}, ptr %mp{j}_{k}, align 1", p.tipo).unwrap();
                        }
                        writeln!(self.out, "  %end{j} = ptrtoint ptr %m{j} to i64").unwrap();
                    }
                    None => writeln!(self.out, "  %end{j} = ptrtoint ptr %a{j} to i64").unwrap(),
                }
            }
            writeln!(self.out, "  %modo = call i64 @dartforge_ffi_callback_entrar(ptr %ctx)").unwrap();
            writeln!(self.out, "  %e_ouvinte = icmp ne i64 %modo, 0\n  br i1 %e_ouvinte, label %ouvinte, label %local").unwrap();
            // Ouvinte: os bits de cada argumento na mensagem (de um
            // composto, o endereço: o runtime copia os bytes).
            writeln!(self.out, "ouvinte:").unwrap();
            for (j, t) in cb.params.iter().enumerate() {
                let bits = match t {
                    TipoNativo::Composto(_) => format!("add i64 %end{j}, 0"),
                    TipoNativo::Prim(tc) => match tc {
                        TipoC::I8 | TipoC::I16 | TipoC::I32 => format!("sext {} %a{j} to i64", tc.llvm()),
                        TipoC::U8 | TipoC::U16 | TipoC::U32 | TipoC::Bool => format!("zext {} %a{j} to i64", tc.llvm()),
                        TipoC::I64 | TipoC::U64 => format!("add i64 %a{j}, 0"),
                        TipoC::F32 => {
                            writeln!(self.out, "  %of{j} = fpext float %a{j} to double").unwrap();
                            format!("bitcast double %of{j} to i64")
                        }
                        TipoC::F64 => format!("bitcast double %a{j} to i64"),
                        TipoC::Ptr | TipoC::Handle => format!("ptrtoint ptr %a{j} to i64"),
                        TipoC::Void => unreachable!("parâmetro nativo void"),
                    },
                };
                writeln!(self.out, "  %o{j} = {bits}\n  %g{j} = getelementptr [{n} x i64], ptr %buf, i64 0, i64 {j}\n  store i64 %o{j}, ptr %g{j}").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_ffi_callback_postar(ptr %ctx, ptr %buf, i64 {})", cb.params.len()).unwrap();
            match &cb.ret {
                TipoNativo::Prim(TipoC::Void) => writeln!(self.out, "  ret void").unwrap(),
                TipoNativo::Prim(TipoC::Ptr | TipoC::Handle) => writeln!(self.out, "  ret ptr null").unwrap(),
                TipoNativo::Prim(TipoC::F32 | TipoC::F64) => writeln!(self.out, "  ret {r} 0.0").unwrap(),
                TipoNativo::Prim(_) => writeln!(self.out, "  ret {r} 0").unwrap(),
                TipoNativo::Composto(_) if r == "void" => writeln!(self.out, "  ret void").unwrap(),
                TipoNativo::Composto(_) => writeln!(self.out, "  ret {r} zeroinitializer").unwrap(),
            }
            // Local: a chamada ao corpo HIR na representação Dart.
            writeln!(self.out, "local:\n  %c = ptrtoint ptr %ctx to i64").unwrap();
            let mut args = vec!["i64 %c".to_string()];
            for (j, t) in cb.params.iter().enumerate() {
                let tc = match t {
                    TipoNativo::Composto(_) => {
                        args.push(format!("i64 %end{j}"));
                        continue;
                    }
                    TipoNativo::Prim(tc) => *tc,
                };
                let (conv_arg, ty) = match tc {
                    TipoC::I8 | TipoC::I16 | TipoC::I32 => (Some(format!("sext {} %a{j} to i64", tc.llvm())), "i64"),
                    TipoC::U8 | TipoC::U16 | TipoC::U32 => (Some(format!("zext {} %a{j} to i64", tc.llvm())), "i64"),
                    TipoC::F32 => (Some(format!("fpext float %a{j} to double")), "double"),
                    TipoC::Ptr | TipoC::Handle => (Some(format!("ptrtoint ptr %a{j} to i64")), "i64"),
                    TipoC::I64 | TipoC::U64 => (None, "i64"),
                    TipoC::F64 => (None, "double"),
                    TipoC::Bool => (None, "i1"),
                    TipoC::Void => unreachable!("parâmetro nativo void"),
                };
                match conv_arg {
                    Some(c) => {
                        writeln!(self.out, "  %d{j} = {c}").unwrap();
                        args.push(format!("{ty} %d{j}"));
                    }
                    None => args.push(format!("{ty} %a{j}")),
                }
            }
            let hr = cb.ret.tipo_hir().llvm_ir();
            let vazio = matches!(cb.ret, TipoNativo::Prim(TipoC::Void));
            if vazio {
                writeln!(self.out, "  call void @{}({})", cb.corpo, args.join(", ")).unwrap();
            } else {
                writeln!(self.out, "  %r = call {hr} @{}({})", cb.corpo, args.join(", ")).unwrap();
            }
            // Uma struct devolvida: os bytes copiados já (antes de qualquer
            // alocação); com exceção o corpo devolve 0 e o retorno é zerado.
            if let Some((_, l)) = &ret {
                let tam = l.tamanho.div_ceil(16) * 16;
                writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %rt, i8 0, i64 {tam}, i1 false)").unwrap();
                writeln!(self.out, "  call void @dartforge_ffi_copiar_composto(ptr %rt, i64 %r, i64 {})", l.tamanho).unwrap();
            }
            writeln!(self.out, "  %x = call i8 @dartforge_ffi_callback_sair(ptr %ctx, ptr %saida)").unwrap();
            match (&cb.ret, &ret) {
                (TipoNativo::Prim(TipoC::Void), _) => {
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((PassagemRet::Sret { .. }, l))) => {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %rt, i64 {}, i1 false)", l.tamanho).unwrap();
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((PassagemRet::Direta(pecas), _))) => {
                    let mut atual = "undef".to_string();
                    for (k, p) in pecas.iter().enumerate() {
                        writeln!(self.out, "  %rp{k} = getelementptr i8, ptr %rt, i64 {}", p.deslocamento).unwrap();
                        writeln!(self.out, "  %rv{k} = load {}, ptr %rp{k}, align 1", p.tipo).unwrap();
                        if pecas.len() == 1 {
                            atual = format!("%rv{k}");
                        } else {
                            writeln!(self.out, "  %ra{k} = insertvalue {r} {atual}, {} %rv{k}, {k}", p.tipo).unwrap();
                            atual = format!("%ra{k}");
                        }
                    }
                    writeln!(self.out, "  ret {r} {atual}\n}}\n").unwrap();
                    continue;
                }
                _ => {}
            }
            let TipoNativo::Prim(tr) = cb.ret else { unreachable!("retorno composto sem passagem") };
            writeln!(self.out, "  %e_excecao = icmp ne i8 %x, 0\n  br i1 %e_excecao, label %excecao, label %normal").unwrap();
            // O retorno excepcional (bits do tipo C) e o normal (Dart → C).
            writeln!(self.out, "excecao:\n  %e = load i64, ptr %saida").unwrap();
            let exc = match tr {
                TipoC::I64 | TipoC::U64 => "add i64 %e, 0".to_string(),
                TipoC::F64 => "bitcast i64 %e to double".to_string(),
                TipoC::F32 => {
                    writeln!(self.out, "  %ed = bitcast i64 %e to double").unwrap();
                    "fptrunc double %ed to float".to_string()
                }
                TipoC::Ptr | TipoC::Handle => "inttoptr i64 %e to ptr".to_string(),
                estreito => format!("trunc i64 %e to {}", estreito.llvm()),
            };
            writeln!(self.out, "  %ev = {exc}\n  ret {r} %ev").unwrap();
            let normal = match tr {
                TipoC::I64 | TipoC::U64 | TipoC::F64 | TipoC::Bool => None,
                TipoC::F32 => Some("fptrunc double %r to float".to_string()),
                TipoC::Ptr | TipoC::Handle => Some("inttoptr i64 %r to ptr".to_string()),
                estreito => Some(format!("trunc i64 %r to {}", estreito.llvm())),
            };
            match normal {
                Some(c) => writeln!(self.out, "normal:\n  %nv = {c}\n  ret {r} %nv\n}}\n").unwrap(),
                None => writeln!(self.out, "normal:\n  ret {r} %r\n}}\n").unwrap(),
            }
        }
    }

    fn chamar_main(&mut self) {
        let Some(entry) = self.module.entry_symbol.clone() else { return };
        let n = self.module.entry_params.min(2);
        if n == 0 {
            writeln!(self.out, "  call void @{entry}()").unwrap();
            return;
        }
        writeln!(self.out, "  %df.args = call i64 @dartforge_argumentos_do_main()").unwrap();
        let args = if n == 1 { "i64 %df.args".to_string() } else { "i64 %df.args, i64 0".to_string() };
        writeln!(self.out, "  call void @{entry}({args})").unwrap();
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
            Instruction::ChamadaNativa { ret, .. } => ret.tipo_hir(),
            Instruction::CargaNativa { tipo, .. } => tipo.tipo_hir(),
            Instruction::GravacaoNativa { .. } => Type::Void,
            Instruction::ChamadaNativaComposta { ret, destino, .. } => {
                if destino.is_some() { Type::Void } else { ret.tipo_hir() }
            }
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
    /// A instrução pode alocar (e, portanto, coletar)? Conservador: só não
    /// coleta o que comprovadamente emite só aritmética, memória local ou
    /// extern que não aloca — e sem conversão que encaixote (um operando
    /// `Ref` onde se espera escalar pode lançar `TypeError`, que aloca; um
    /// escalar onde se espera `Ref`, ou uma constante de texto, aloca).
    fn pode_coletar(&self, inst: &Instruction) -> bool {
        let escalar = |op: &Operand| {
            self.tipo_de(op) != Type::Ref && !matches!(op, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
        };
        let exato = |op: &Operand, t: Type| match op {
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => false,
            Operand::Constant(Constant::Null) => t == Type::Ref,
            _ => {
                let a = self.tipo_de(op);
                a == t || (a != Type::Ref && t != Type::Ref)
            }
        };
        match inst {
            Instruction::Const(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_) | Constant::Null) => false,
            Instruction::Add(a, b)
            | Instruction::Sub(a, b)
            | Instruction::Mul(a, b)
            | Instruction::SDiv(a, b)
            | Instruction::SRem(a, b)
            | Instruction::Shl(a, b)
            | Instruction::AShr(a, b)
            | Instruction::LShr(a, b)
            | Instruction::And(a, b)
            | Instruction::Or(a, b)
            | Instruction::Xor(a, b)
            | Instruction::FAdd(a, b)
            | Instruction::FSub(a, b)
            | Instruction::FMul(a, b)
            | Instruction::FDiv(a, b)
            | Instruction::ICmp(_, a, b)
            | Instruction::FCmp(_, a, b) => !(escalar(a) && escalar(b)),
            Instruction::Neg(a)
            | Instruction::Not(a)
            | Instruction::FNeg(a)
            | Instruction::LNot(a)
            | Instruction::IntToDouble(a)
            | Instruction::DoubleToInt(a) => !escalar(a),
            Instruction::Alloca(_) | Instruction::Load { .. } | Instruction::Phi { .. } => false,
            // Em linha (load/store no vetor de campos), sem conversão que
            // aloque: um `Ref` já é `i64`, um `double` é `bitcast`.
            Instruction::GetField { index, .. } => (*index as usize) >= CAMPOS_EM_LINHA,
            Instruction::SetField { index, value, .. } => {
                (*index as usize) >= CAMPOS_EM_LINHA
                    || matches!(value, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
            }
            Instruction::Store { ptr, val } => {
                let t = match ptr {
                    Operand::Val(p) => self.apontado.get(p).copied().unwrap_or(Type::I64),
                    _ => return true,
                };
                !exato(val, t)
            }
            Instruction::CargaNativa { endereco, indice, .. } => !(escalar(endereco) && escalar(indice)),
            Instruction::GravacaoNativa { endereco, indice, valor, .. } => {
                !(escalar(endereco) && escalar(indice) && escalar(valor))
            }
            Instruction::CallRuntime { name, args, .. } => {
                let e = externs::efeitos_de(name);
                let nao_aloca = !e.aloca && !e.chama_dart;
                !(nao_aloca && args.iter().all(|(a, t)| exato(a, *t)))
            }
            _ => true,
        }
    }

    fn tipo_de(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.tipos.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Funcao(_)) => Type::I64,
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
            (Constant::Funcao(f), Type::I64) => format!("ptrtoint (ptr @{f} to i64)"),
            (Constant::Funcao(_), _) => panic!("endereço de função só como i64"),
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
            Operand::Constant(Constant::Funcao(f)) => format!("ptrtoint (ptr @{f} to i64)"),
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
