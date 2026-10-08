//! A emissão das caixas, células, contextos, closures e records (P2,
//! docs/NATIVO-ESPACO-UNIFICADO.md §2.5, §3.5 e §4.4): `Box`/`Unbox` escalares,
//! `AllocCell`/`CellGet`/`CellSet`, `AllocEnv`/`EnvGet`, `AllocClosure`,
//! `AllocClosureTipada`, `TearOff`, `AllocRecord` e os ajudantes
//! `@df.caixa_*`/`@df.desencaixa_*`/`@df.identico` do prelúdio.
//!
//! Tudo é bloco do espaço de objetos, alocado em linha por `@df.alocar` (P0) e
//! lido/gravado por deslocamento a partir do handle (`h = bloco + 2`, então a
//! palavra `b + k` está em `h + k − 2`):
//!
//! * `_Mint`/`_Double` (`BRUTO`, w = 1): o valor em `h+14`;
//! * `bool`: as estáticas do runtime ativo, lidas do `Contexto` (deslocamentos
//!   360 e 368, `layout::contexto::VERDADEIRO`/`FALSO`; §4.10 item 9);
//! * `_Celula` (`INSTANCIA`, 1 campo): o valor em `h+14`, o bit do mapa (`h+6`)
//!   pela representação gravada;
//! * `_Contexto` (`INSTANCIA`, k campos): capturas em `h+14+8i`, cada uma na
//!   representação do tipo da variável (a mesma com que o `EnvGet` a lê);
//! * `_Closure` (`INSTANCIA`, 4 campos): código `h+14`, contexto `h+22` (`Ref`,
//!   o único bit do mapa), corpo tipado `h+30`, ABI `h+38`;
//! * `_Record` posicional (`REFS`, w = 1 + k): k em `h+14`, campos em `h+22+8i`.
//!
//! Um objeto recém-alocado é jovem: as gravações da criação não têm barreira.

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, estado, flags};
use std::fmt::Write;

#[cfg(test)]
/// O cabeçalho (palavra 0) de um `_Mint`: `JOVEM | BRUTO << 8 | 1 << 16 | 3 << 32`.
const CAB_MINT: u64 = layout::palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, 1, cid::MINT);
/// O de um `_Double`.
#[cfg(test)]
const CAB_DOUBLE: u64 = layout::palavra_do_cabecalho(estado::JOVEM, flags::BRUTO, 1, cid::DOUBLE);
/// O de uma `_Celula` (1 campo).
const CAB_CELULA: u64 = layout::palavra_do_cabecalho(estado::JOVEM, flags::INSTANCIA, 1, cid::CELULA);
/// O de uma `_Closure` (4 campos).
const CAB_CLOSURE: u64 = layout::palavra_do_cabecalho(estado::JOVEM, flags::INSTANCIA, 4, cid::CLOSURE);

/// Os ajudantes da P2 (§3.5), impressos no prelúdio de cada módulo por
/// `emit_runtime_decls`. As constantes dos cabeçalhos estão escritas por
/// extenso (o texto é `const`); o teste `ajudantes_com_os_cabecalhos_do_layout`
/// confere que são as de [`layout::palavra_do_cabecalho`].
///
/// * `@df.caixa_int`: `Smi` na faixa de 63 bits; senão `_Mint` novo.
/// * `@df.desencaixa_int`: `Smi` → `ashr 1`; `_Mint` → o valor; senão o
///   runtime, que lança `TypeError` (exceção pendente).
/// * `@df.caixa_double`/`@df.desencaixa_double`: idem com `_Double`.
/// * `@df.num_para_double`: um `num` em `Ref` como `double` (`Smi` e `_Mint`
///   convertidos; o resto pelo `@df.desencaixa_double`): o operando direito
///   de `d op= n` com `d` `double` e `n` `num`.
/// * `@df.caixa_bool`: `select` entre as estáticas; `@df.desencaixa_bool`
///   compara com elas, e o resto vai ao runtime.
/// * `@df.identico`: `identical` (§2.10) — o mesmo handle, ou dois `_Mint`/
///   `_Double` com os mesmos bits.
pub const AJUDANTES: &str = "define internal i64 @df.caixa_int(i64 %v) alwaysinline {\n\
  %a = add i64 %v, 4611686018427387904\n\
  %ok = icmp ult i64 %a, -9223372036854775808\n\
  br i1 %ok, label %smi, label %heap\n\
smi:\n\
  %s = shl i64 %v, 1\n\
  %r = or i64 %s, 1\n\
  ret i64 %r\n\
heap:\n\
  %h = call i64 @df.alocar(i64 12884967937, i64 1)\n\
  %p = inttoptr i64 %h to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store i64 %v, ptr %q, align 8\n\
  ret i64 %h\n\
}\n\
define internal i64 @df.desencaixa_int(i64 %r) alwaysinline {\n\
  %b = and i64 %r, 1\n\
  %e = icmp ne i64 %b, 0\n\
  br i1 %e, label %smi, label %obj\n\
smi:\n\
  %v = ashr i64 %r, 1\n\
  ret i64 %v\n\
obj:\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %lento\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %mint = icmp eq i32 %c, 3\n\
  br i1 %mint, label %ler, label %lento\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load i64, ptr %vp, align 8\n\
  ret i64 %x\n\
lento:\n\
  %h = call i64 @dartforge_unbox_int(i64 %r)\n\
  ret i64 %h\n\
}\n\
define internal i64 @df.caixa_double(double %d) alwaysinline {\n\
  %h = call i64 @df.alocar(i64 17179935233, i64 1)\n\
  %p = inttoptr i64 %h to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store double %d, ptr %q, align 8\n\
  ret i64 %h\n\
}\n\
define internal double @df.desencaixa_double(i64 %r) alwaysinline {\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %lento\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %dbl = icmp eq i32 %c, 4\n\
  br i1 %dbl, label %ler, label %lento\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load double, ptr %vp, align 8\n\
  ret double %x\n\
lento:\n\
  %h = call double @dartforge_unbox_double(i64 %r)\n\
  ret double %h\n\
}\n\
define internal double @df.num_para_double(i64 %r) alwaysinline {\n\
  %b = and i64 %r, 1\n\
  %e = icmp ne i64 %b, 0\n\
  br i1 %e, label %smi, label %obj\n\
smi:\n\
  %v = ashr i64 %r, 1\n\
  %dv = sitofp i64 %v to double\n\
  ret double %dv\n\
obj:\n\
  %m = and i64 %r, -9223372036854775801\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %cls, label %dbl\n\
cls:\n\
  %p = inttoptr i64 %r to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %mint = icmp eq i32 %c, 3\n\
  br i1 %mint, label %ler, label %dbl\n\
ler:\n\
  %vp = getelementptr inbounds i8, ptr %p, i64 14\n\
  %x = load i64, ptr %vp, align 8\n\
  %dx = sitofp i64 %x to double\n\
  ret double %dx\n\
dbl:\n\
  %d = call double @df.desencaixa_double(i64 %r)\n\
  ret double %d\n\
}\n\
define internal i64 @df.caixa_bool(i1 %b) alwaysinline {\n\
  %ctx = call ptr @dartforge_contexto()\n\
  %o = select i1 %b, i64 360, i64 368\n\
  %p = getelementptr inbounds i8, ptr %ctx, i64 %o\n\
  %r = load i64, ptr %p, align 8, !invariant.load !{}\n\
  ret i64 %r\n\
}\n\
define internal i1 @df.desencaixa_bool(i64 %r) alwaysinline {\n\
  %ctx = call ptr @dartforge_contexto()\n\
  %vp = getelementptr inbounds i8, ptr %ctx, i64 360\n\
  %vh = load i64, ptr %vp, align 8, !invariant.load !{}\n\
  %v = icmp eq i64 %r, %vh\n\
  br i1 %v, label %sim, label %t\n\
sim:\n\
  ret i1 true\n\
t:\n\
  %fp = getelementptr inbounds i8, ptr %ctx, i64 368\n\
  %fh = load i64, ptr %fp, align 8, !invariant.load !{}\n\
  %f = icmp eq i64 %r, %fh\n\
  br i1 %f, label %nao, label %lento\n\
nao:\n\
  ret i1 false\n\
lento:\n\
  %u = call i8 @dartforge_unbox_bool(i64 %r)\n\
  %x = icmp ne i8 %u, 0\n\
  ret i1 %x\n\
}\n\
define internal i1 @df.identico(i64 %a, i64 %b) alwaysinline {\n\
  %eq = icmp eq i64 %a, %b\n\
  br i1 %eq, label %sim, label %t1\n\
t1:\n\
  %ma = and i64 %a, -9223372036854775801\n\
  %oa = icmp eq i64 %ma, 2\n\
  %mb = and i64 %b, -9223372036854775801\n\
  %ob = icmp eq i64 %mb, 2\n\
  %ambos = and i1 %oa, %ob\n\
  br i1 %ambos, label %t2, label %nao\n\
t2:\n\
  %pa = inttoptr i64 %a to ptr\n\
  %pb = inttoptr i64 %b to ptr\n\
  %cpa = getelementptr inbounds i8, ptr %pa, i64 2\n\
  %cpb = getelementptr inbounds i8, ptr %pb, i64 2\n\
  %ca = load i32, ptr %cpa, align 4, !invariant.load !{}\n\
  %cb = load i32, ptr %cpb, align 4, !invariant.load !{}\n\
  %mesma = icmp eq i32 %ca, %cb\n\
  %c3 = sub i32 %ca, 3\n\
  %num = icmp ult i32 %c3, 2\n\
  %cmp = and i1 %mesma, %num\n\
  br i1 %cmp, label %t3, label %nao\n\
t3:\n\
  %va = getelementptr inbounds i8, ptr %pa, i64 14\n\
  %vb = getelementptr inbounds i8, ptr %pb, i64 14\n\
  %xa = load i64, ptr %va, align 8\n\
  %xb = load i64, ptr %vb, align 8\n\
  %r = icmp eq i64 %xa, %xb\n\
  ret i1 %r\n\
sim:\n\
  ret i1 true\n\
nao:\n\
  ret i1 false\n\
}\n";

/// Os efeitos de cada ajudante `@df.*` deste arquivo chamado por
/// `CallRuntime`: `(nome, aloca, lança)`. `externs::efeitos_de` os consulta
/// antes da tabela de externs (um ajudante fora daqui é conservador). Os
/// desencaixes lançam `TypeError` (que aloca) pelo runtime.
pub(super) const EFEITOS_DOS_AJUDANTES: &[(&str, bool, bool)] = &[
    ("df.caixa_int", true, false),
    ("df.caixa_double", true, false),
    ("df.caixa_bool", false, false),
    ("df.desencaixa_int", true, true),
    ("df.desencaixa_double", true, true),
    ("df.num_para_double", true, true),
    ("df.desencaixa_bool", true, true),
    ("df.identico", false, false),
];

/// O estado da emissão de caixas e closures por módulo.
#[derive(Default)]
pub(super) struct EstadoDeCaixas {
    /// Contador dos nomes locais das instruções sem valor (`CellSet`).
    proximo: u32,
}

impl EstadoDeCaixas {
    fn novo_nome(&mut self) -> u32 {
        let k = self.proximo;
        self.proximo += 1;
        k
    }
}

impl LlvmEmitter<'_> {
    /// `Box` de um escalar (`I64`, `F64`, `I1`/`I8`) em `%v<v>`.
    pub(super) fn emitir_caixa(&mut self, v: u32, op: &Operand, from: Type) {
        match from {
            Type::F64 => {
                let so = self.coagir(op, Type::F64);
                writeln!(self.out, "  %v{v} = call i64 @df.caixa_double(double {so})").unwrap();
            }
            Type::I1 => {
                let so = self.coagir(op, Type::I1);
                writeln!(self.out, "  %v{v} = call i64 @df.caixa_bool(i1 {so})").unwrap();
            }
            Type::I8 => {
                let so = self.coagir(op, Type::I8);
                writeln!(self.out, "  %cb{v} = icmp ne i8 {so}, 0").unwrap();
                writeln!(self.out, "  %v{v} = call i64 @df.caixa_bool(i1 %cb{v})").unwrap();
            }
            _ => {
                let so = self.coagir(op, Type::I64);
                writeln!(self.out, "  %v{v} = call i64 @df.caixa_int(i64 {so})").unwrap();
            }
        }
    }

    /// `Unbox` para um escalar (`I64`, `F64`, `I1`); lança `TypeError` pela
    /// pendência.
    pub(super) fn emitir_descaixa(&mut self, v: u32, op: &Operand, to: Type) {
        let so = self.coagir(op, Type::Ref);
        match to {
            Type::F64 => {
                writeln!(self.out, "  %v{v} = call double @df.desencaixa_double(i64 {so})").unwrap();
            }
            Type::I1 => {
                writeln!(self.out, "  %v{v} = call i1 @df.desencaixa_bool(i64 {so})").unwrap();
            }
            _ => {
                writeln!(self.out, "  %v{v} = call i64 @df.desencaixa_int(i64 {so})").unwrap();
            }
        }
    }

    /// A alocação em linha (`@df.alocar`) de um bloco de `w` palavras com o
    /// cabeçalho `cab`, em `%v<v>`, e o ponteiro dele (o handle) em `%bp<v>`.
    /// Acima de `TLAB_N` palavras, `@df.alocar` vai ao runtime.
    fn alocar_bloco(&mut self, v: u32, cab: u64, w: usize) {
        writeln!(self.out, "  %v{v} = call i64 @df.alocar(i64 {}, i64 {w})", cab as i64).unwrap();
        writeln!(self.out, "  %bp{v} = inttoptr i64 %v{v} to ptr").unwrap();
    }

    /// Grava `valor` (texto do IR, `i64`) na palavra de deslocamento `desl`
    /// (a partir do bloco) do objeto `%bp<v>`; `sufixo` torna o nome único.
    fn gravar_palavra(&mut self, v: u32, sufixo: &str, desl: usize, valor: &str) {
        let d = desl as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        writeln!(self.out, "  %bg{v}_{sufixo} = getelementptr inbounds i8, ptr %bp{v}, i64 {d}").unwrap();
        writeln!(self.out, "  store i64 {valor}, ptr %bg{v}_{sufixo}, align 8").unwrap();
    }

    /// Grava o `mapa` (os 32 primeiros bits de referência) do objeto `%bp<v>`.
    fn gravar_mapa(&mut self, v: u32, mapa: u32) {
        if mapa == 0 {
            // A palavra 1 do cabeçalho vem zerada da alocação.
            return;
        }
        let d = layout::desl::MAPA as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        writeln!(self.out, "  %bm{v} = getelementptr inbounds i8, ptr %bp{v}, i64 {d}").unwrap();
        writeln!(self.out, "  store i32 {}, ptr %bm{v}, align 4", mapa as i32).unwrap();
    }

    /// Todos os elementos do record são `Ref` (o lowering encaixota,
    /// `lower_registro_posicional`)? Senão, o caminho lento com os pares.
    fn record_so_de_refs(&self, elements: &[(Operand, u8)]) -> bool {
        elements.iter().all(|(op, _)| self.tipo_de(op) == Type::Ref)
    }

    /// O `alloca` do bloco de entrada de um `AllocRecord`: os pares
    /// `(bits, tag)` do caminho lento (elemento sem caixa), ou os `Ref` de um
    /// record grande demais para a TLAB.
    pub(super) fn buffer_de_record(&mut self, v: u32, elements: &[(Operand, u8)]) {
        if !self.record_so_de_refs(elements) {
            writeln!(self.out, "  %rec_buf_{v} = alloca [{} x i64]", elements.len() * 2).unwrap();
        } else if 1 + elements.len() > layout::TLAB_N {
            writeln!(self.out, "  %rec_buf_{v} = alloca [{} x i64]", elements.len()).unwrap();
        }
    }

    /// O record posicional literal (`_Record`, `REFS`).
    pub(super) fn emitir_alloc_record(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len();
        if !self.record_so_de_refs(elements) {
            // Caminho lento (um elemento sem caixa, que o lowering ainda não
            // encaixotou): o runtime encaixota com as raízes certas.
            let total = n * 2;
            for (i, (elem, _)) in elements.iter().enumerate() {
                let tag = self.tag_de(elem);
                let sop = self.coagir(elem, Type::I64);
                writeln!(self.out, "  %rb{v}_{i} = getelementptr [{total} x i64], ptr %rec_buf_{v}, i64 0, i64 {}", i * 2).unwrap();
                writeln!(self.out, "  store i64 {sop}, ptr %rb{v}_{i}").unwrap();
                writeln!(self.out, "  %rt{v}_{i} = getelementptr [{total} x i64], ptr %rec_buf_{v}, i64 0, i64 {}", i * 2 + 1).unwrap();
                writeln!(self.out, "  store i64 {tag}, ptr %rt{v}_{i}").unwrap();
            }
            writeln!(self.out, "  %v{v} = call i64 @dartforge_record_new(ptr %rec_buf_{v}, i64 {n})").unwrap();
            return;
        }
        let w = 1 + n;
        if w > layout::TLAB_N {
            for (i, (elem, _)) in elements.iter().enumerate() {
                let s = self.coagir(elem, Type::Ref);
                writeln!(self.out, "  %rb{v}_{i} = getelementptr [{n} x i64], ptr %rec_buf_{v}, i64 0, i64 {i}").unwrap();
                writeln!(self.out, "  store i64 {s}, ptr %rb{v}_{i}").unwrap();
            }
            writeln!(self.out, "  %v{v} = call i64 @dartforge_record_novo(ptr %rec_buf_{v}, i64 {n})").unwrap();
            return;
        }
        let cab = layout::palavra_do_cabecalho(estado::JOVEM, flags::REFS, w, cid::RECORD);
        let valores: Vec<String> = elements.iter().map(|(e, _)| self.coagir(e, Type::Ref)).collect();
        self.alocar_bloco(v, cab, w);
        self.gravar_palavra(v, "n", layout::desl::RECORD_FORMA, &n.to_string());
        for (i, s) in valores.iter().enumerate() {
            self.gravar_palavra(v, &i.to_string(), layout::desl::RECORD_CAMPOS + 8 * i, s);
        }
    }

    /// A célula de uma captura mutável: o valor na representação do local.
    pub(super) fn emitir_alloc_cell(&mut self, v: u32, value: &Operand) {
        let e_ref = self.tipo_de(value) == Type::Ref;
        let s = self.coagir(value, Type::I64);
        self.alocar_bloco(v, CAB_CELULA, layout::palavras_de_instancia(1));
        self.gravar_palavra(v, "c", layout::desl::CORPO, &s);
        self.gravar_mapa(v, u32::from(e_ref));
    }

    /// A leitura de uma célula na representação `ty` (a com que foi gravada).
    pub(super) fn emitir_cell_get(&mut self, v: u32, cell: &Operand, ty: Type) {
        let c = self.coagir(cell, Type::Ref);
        let d = layout::desl::CORPO as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        writeln!(self.out, "  %cgp{v} = inttoptr i64 {c} to ptr").unwrap();
        writeln!(self.out, "  %cgg{v} = getelementptr inbounds i8, ptr %cgp{v}, i64 {d}").unwrap();
        writeln!(self.out, "  %u{v} = load i64, ptr %cgg{v}, align 8").unwrap();
        self.bits_para_repr(v, &format!("%u{v}"), ty);
    }

    /// A gravação numa célula: o valor, o bit do mapa pela representação e,
    /// para um `Ref`, a barreira (`@df.barreira`).
    pub(super) fn emitir_cell_set(&mut self, cell: &Operand, value: &Operand) {
        let k = self.caixas.novo_nome();
        let e_ref = self.tipo_de(value) == Type::Ref;
        let c = self.coagir(cell, Type::Ref);
        let s = self.coagir(value, Type::I64);
        // No ARC a gravação vai ao runtime, que conta a troca (o campo 0 da
        // célula, `INSTANCIA` de um campo).
        if self.module.memoria_arc {
            writeln!(self.out, "  call void @dartforge_object_set(i64 {c}, i64 0, i64 {s}, i8 {})", u8::from(e_ref)).unwrap();
            return;
        }
        let d = layout::desl::CORPO as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        let dm = layout::desl::MAPA as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        writeln!(self.out, "  %csp{k} = inttoptr i64 {c} to ptr").unwrap();
        writeln!(self.out, "  %csg{k} = getelementptr inbounds i8, ptr %csp{k}, i64 {d}").unwrap();
        writeln!(self.out, "  store i64 {s}, ptr %csg{k}, align 8").unwrap();
        writeln!(self.out, "  %csm{k} = getelementptr inbounds i8, ptr %csp{k}, i64 {dm}").unwrap();
        writeln!(self.out, "  store i32 {}, ptr %csm{k}, align 4", u8::from(e_ref)).unwrap();
        let constante = matches!(value, Operand::Constant(Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_)));
        if e_ref && !constante {
            writeln!(self.out, "  call void @df.barreira(i64 {c}, i64 {s})").unwrap();
        }
    }

    /// O `alloca` do bloco de entrada de um `AllocEnv`: nenhum (as capturas
    /// vão direto ao bloco do contexto).
    pub(super) fn buffer_de_ambiente(&mut self, v: u32, values: &[Operand]) {
        let _ = (v, values);
    }

    /// O contexto (`_Contexto`) das capturas de uma closure: cada captura na
    /// representação do seu operando, com o bit do mapa aceso para `Ref`. Sem
    /// capturas, null (quem não captura nada nunca lê o contexto).
    pub(super) fn emitir_alloc_env(&mut self, v: u32, values: &[Operand]) {
        let n = values.len();
        if n == 0 {
            writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
            return;
        }
        let refs: Vec<bool> = values.iter().map(|val| self.tipo_de(val) == Type::Ref).collect();
        let bits: Vec<String> = values.iter().map(|val| self.coagir(val, Type::I64)).collect();
        let w = layout::palavras_de_instancia(n);
        if w <= layout::TLAB_N {
            let cab = layout::palavra_do_cabecalho(estado::JOVEM, flags::INSTANCIA, n, cid::CONTEXTO);
            self.alocar_bloco(v, cab, w);
        } else {
            // Contexto grande: o runtime (o `n` de uma `INSTANCIA` é o número
            // de campos, não as palavras do corpo).
            writeln!(self.out, "  %v{v} = call i64 @dartforge_object_new(i64 {}, i64 {n})", cid::CONTEXTO).unwrap();
            writeln!(self.out, "  %bp{v} = inttoptr i64 %v{v} to ptr").unwrap();
        }
        for (i, s) in bits.iter().enumerate() {
            self.gravar_palavra(v, &i.to_string(), layout::desl::CORPO + 8 * i, s);
        }
        let mapa = refs.iter().take(32).enumerate().fold(0u32, |m, (i, &r)| m | (u32::from(r) << i));
        self.gravar_mapa(v, mapa);
        // A extensão do mapa (campos 32..), nas palavras depois dos campos.
        for j in 0..layout::palavras_do_mapa(n) {
            let palavra = refs
                .iter()
                .enumerate()
                .skip(32 + 64 * j)
                .take(64)
                .fold(0u64, |m, (i, &r)| m | (u64::from(r) << ((i - 32) % 64)));
            if palavra != 0 {
                let desl = layout::desl::CORPO + 8 * (layout::capacidade(n) + j);
                self.gravar_palavra(v, &format!("m{j}"), desl, &(palavra as i64).to_string());
            }
        }
    }

    /// A captura `index` do contexto, na representação `ty` — a mesma com que
    /// foi gravada (o lowering garante; §4.4 e risco 12).
    pub(super) fn emitir_env_get(&mut self, v: u32, env: &Operand, index: usize, ty: Type) {
        let e = self.coagir(env, Type::Ref);
        let d = (layout::desl::CORPO + 8 * index) as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        writeln!(self.out, "  %ep{v} = inttoptr i64 {e} to ptr").unwrap();
        writeln!(self.out, "  %eg{v} = getelementptr inbounds i8, ptr %ep{v}, i64 {d}").unwrap();
        writeln!(self.out, "  %u{v} = load i64, ptr %eg{v}, align 8").unwrap();
        self.bits_para_repr(v, &format!("%u{v}"), ty);
    }

    /// Uma `_Closure` em linha: código, contexto (`Ref`), corpo tipado e ABI.
    fn emitir_closure_em_linha(&mut self, v: u32, codigo: &str, env: &Operand, tipado: &str, abi: i64) {
        let e = self.coagir(env, Type::Ref);
        self.alocar_bloco(v, CAB_CLOSURE, layout::palavras_de_instancia(4));
        self.gravar_palavra(v, "k", layout::desl::CLOSURE_CODIGO, codigo);
        self.gravar_palavra(v, "e", layout::desl::CLOSURE_CONTEXTO, &e);
        self.gravar_palavra(v, "t", layout::desl::CLOSURE_TIPADO, tipado);
        self.gravar_palavra(v, "a", layout::desl::CLOSURE_ABI, &abi.to_string());
        // Só o contexto é referência.
        self.gravar_mapa(v, 0b10);
    }

    /// A closure de código `code_symbol` (a entrada uniforme) sobre `env`.
    pub(super) fn emitir_alloc_closure(&mut self, v: u32, code_symbol: &str, env: &Operand) {
        // O código de uma closure é o endereço da entrada uniforme: vale entre
        // módulos (uma closure criada no SDK da fonte é chamada no programa) e
        // não depende da ordem de nada.
        self.anotar_externo(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        let codigo = format!("ptrtoint (ptr @{code_symbol} to i64)");
        self.emitir_closure_em_linha(v, &codigo, env, "0", 0);
    }

    /// A closure com corpo tipado (`AllocClosureTipada`). Com `direto`, `env`
    /// é o próprio valor capturado (ou o `this`) no lugar do contexto: o mesmo
    /// campo `Ref`.
    pub(super) fn emitir_alloc_closure_tipada(&mut self, v: u32, code_symbol: &str, env: &Operand, tipado: &str, abi: i64, direto: bool) {
        let _ = direto;
        self.anotar_externo(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        let codigo = format!("ptrtoint (ptr @{code_symbol} to i64)");
        let corpo = format!("ptrtoint (ptr @{tipado} to i64)");
        self.emitir_closure_em_linha(v, &codigo, env, &corpo, abi);
    }

    /// O tear-off canônico da função de entrada uniforme `code_symbol` (o
    /// runtime guarda o único handle, `Heap::tearoff`).
    pub(super) fn emitir_tearoff(&mut self, v: u32, code_symbol: &str) {
        self.anotar_externo(code_symbol, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
        writeln!(self.out, "  %v{v} = call i64 @dartforge_tearoff(i64 ptrtoint (ptr @{code_symbol} to i64))").unwrap();
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ajudantes_com_os_cabecalhos_do_layout() {
        assert!(AJUDANTES.contains(&format!("@df.alocar(i64 {}, i64 1)", CAB_MINT as i64)));
        assert!(AJUDANTES.contains(&format!("@df.alocar(i64 {}, i64 1)", CAB_DOUBLE as i64)));
        // A máscara de `e_objeto` (7 | MIN) e os deslocamentos do valor e da classe.
        assert!(AJUDANTES.contains(&format!("and i64 %r, {}", 7 | i64::MIN)));
        assert_eq!(layout::desl::VALOR as i64 - layout::DESLOCAMENTO_DO_HANDLE, 14);
        assert_eq!(layout::desl::CLASSE as i64 - layout::DESLOCAMENTO_DO_HANDLE, 2);
        assert_eq!((cid::MINT, cid::DOUBLE), (3, 4));
        for (nome, _, _) in EFEITOS_DOS_AJUDANTES {
            assert!(AJUDANTES.contains(&format!("@{nome}(")), "{nome} sem definição");
        }
        assert_eq!(CAB_CELULA >> 32, cid::CELULA as u64);
        assert_eq!(CAB_CLOSURE >> 16 & 0xFFFF, 4);
    }
}
