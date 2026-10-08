//! A emissão das listas do núcleo (P3, docs/NATIVO-ESPACO-UNIFICADO.md §3.5 e
//! §4.5): o literal de lista (`AllocList`) e os ajudantes `@df.lista_*` do
//! prelúdio.
//!
//! O literal vai ao runtime como palavras (`dartforge_lista_nova`), na forma que
//! os elementos pedem: todos `Ref` → geral; todos `int`, todos `double` ou todos
//! `bool` sem caixa → a forma compacta desse escalar (o `E` reificado, gravado
//! logo depois pelo `rti_definir`, a confirma ou a desfaz: `Heap::lista_ajustar_forma`);
//! misturados → geral, com um vetor de tags que diz ao runtime quais palavras
//! encaixotar. O código gerado não encaixota: uma caixa feita antes da lista
//! ficaria sem raiz na alocação seguinte.
//!
//! Os ajudantes leem o bloco pelos deslocamentos de `layout::desl` (conferidos
//! abaixo, em tempo de compilação, contra os números do texto do IR). Nenhum
//! aloca nem lança ([`EFEITOS_DOS_AJUDANTES`]).

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, desl, flags};
use std::fmt::Write;

/// Os ajudantes `@df.lista_*` (§3.5), impressos no prelúdio de cada módulo por
/// `emit_runtime_decls`. `%l` é uma lista do núcleo (`_List`, `_ImmutableList` ou
/// `_GrowableList`); `%a`, um armazenamento (`_List`/`_ImmutableList`), o que
/// `@df.lista_armazenamento` devolve.
///
/// * `@df.lista_len(l)`: o comprimento, `[l+14]` nas três classes;
/// * `@df.lista_armazenamento(l)`: cid 10 → `[l+22]`, senão `l`;
/// * `@df.lista_elemento(a, i)`: o endereço `a + 22 + 8i`;
/// * `@df.lista_forma(a)`: `[a−1] & 0x66` (`FORMA` e `ELEMENTO`: 0x04 geral,
///   0x22 `int`, 0x42 `double`, 0x62 `bool`);
/// * `@df.lista_capacidade(l)`: o comprimento do armazenamento;
/// * `@df.lista_ler(a, i)`: a palavra `i` (um `Ref` na forma geral);
/// * `@df.lista_gravar_ref(a, i, v)`: grava o `Ref` e aplica a barreira de
///   elemento (`@df.barreira_elemento`, com cartões);
/// * `@df.lista_gravar_len(l, n)`: o comprimento de uma `_GrowableList`, sem
///   barreira (não é referência; o `_setLength` da VM);
/// * `@df.lista_gravar_dados(l, d)`: troca o armazenamento de uma `_GrowableList`
///   com a barreira quando `d` tem a forma do atual (devolve 1); senão não grava
///   e devolve 0 (o native devolve a forma ao novo).
pub const AJUDANTES: &str = "define internal i64 @df.lista_len(i64 %l) alwaysinline {\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  %n = load i64, ptr %q, align 8\n\
  ret i64 %n\n\
}\n\
define internal i64 @df.lista_armazenamento(i64 %l) alwaysinline {\n\
entrada:\n\
  %p = inttoptr i64 %l to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 2\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %e = icmp eq i32 %c, 10\n\
  br i1 %e, label %exp, label %fim\n\
exp:\n\
  %dp = getelementptr inbounds i8, ptr %p, i64 22\n\
  %d = load i64, ptr %dp, align 8\n\
  br label %fim\n\
fim:\n\
  %r = phi i64 [ %d, %exp ], [ %l, %entrada ]\n\
  ret i64 %r\n\
}\n\
define internal ptr @df.lista_elemento(i64 %a, i64 %i) alwaysinline {\n\
  %p = inttoptr i64 %a to ptr\n\
  %b = getelementptr inbounds i8, ptr %p, i64 22\n\
  %e = getelementptr inbounds i64, ptr %b, i64 %i\n\
  ret ptr %e\n\
}\n\
define internal i8 @df.lista_forma(i64 %a) alwaysinline {\n\
  %p = inttoptr i64 %a to ptr\n\
  %fp = getelementptr inbounds i8, ptr %p, i64 -1\n\
  %f = load i8, ptr %fp, align 1\n\
  %r = and i8 %f, 102\n\
  ret i8 %r\n\
}\n\
define internal i64 @df.lista_capacidade(i64 %l) alwaysinline {\n\
  %a = call i64 @df.lista_armazenamento(i64 %l)\n\
  %n = call i64 @df.lista_len(i64 %a)\n\
  ret i64 %n\n\
}\n\
define internal i64 @df.lista_ler(i64 %a, i64 %i) alwaysinline {\n\
  %e = call ptr @df.lista_elemento(i64 %a, i64 %i)\n\
  %v = load i64, ptr %e, align 8\n\
  ret i64 %v\n\
}\n\
define internal void @df.lista_gravar_ref(i64 %a, i64 %i, i64 %v) alwaysinline {\n\
  %e = call ptr @df.lista_elemento(i64 %a, i64 %i)\n\
  store i64 %v, ptr %e, align 8\n\
  call void @df.barreira_elemento(i64 %a, i64 %i, i64 %v)\n\
  ret void\n\
}\n\
define internal void @df.lista_gravar_len(i64 %l, i64 %n) alwaysinline {\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 14\n\
  store i64 %n, ptr %q, align 8\n\
  ret void\n\
}\n\
define internal i64 @df.lista_gravar_dados(i64 %l, i64 %d) alwaysinline {\n\
entrada:\n\
  %a = call i64 @df.lista_armazenamento(i64 %l)\n\
  %fa = call i8 @df.lista_forma(i64 %a)\n\
  %fd = call i8 @df.lista_forma(i64 %d)\n\
  %igual = icmp eq i8 %fa, %fd\n\
  br i1 %igual, label %grava, label %fim\n\
grava:\n\
  %p = inttoptr i64 %l to ptr\n\
  %q = getelementptr inbounds i8, ptr %p, i64 22\n\
  store i64 %d, ptr %q, align 8\n\
  call void @df.barreira(i64 %l, i64 %d)\n\
  br label %fim\n\
fim:\n\
  %r = phi i64 [ 1, %grava ], [ 0, %entrada ]\n\
  ret i64 %r\n\
}\n";

// Os números do texto de `AJUDANTES` são os do contrato de layout (relativos ao
// handle, bloco + 2).
const _: () = {
    let h = layout::DESLOCAMENTO_DO_HANDLE as usize;
    assert!(desl::COMPRIMENTO - h == 14);
    assert!(desl::EXPANSIVEL_DADOS - h == 22);
    assert!(desl::ELEMENTOS - h == 22);
    assert!(desl::CLASSE - h == 2);
    assert!(desl::FLAGS as i64 - h as i64 == -1);
    assert!(cid::GROWABLE_LIST == 10);
    assert!(flags::FORMA | flags::ELEMENTO == 102);
};

/// Os ajudantes do módulo: no ARC (`Module::memoria_arc`), a gravação do
/// elemento e a troca do armazenamento da `_GrowableList` passam pelo
/// runtime, que conta a troca e aplica a barreira: o elemento pela palavra
/// do corpo `REFS` (`dartforge_arc_gravar_ref`), o armazenamento pelo campo
/// 1 da `INSTANCIA` (`dartforge_object_set`).
pub(super) fn ajudantes(arc: bool) -> std::borrow::Cow<'static, str> {
    if !arc {
        return std::borrow::Cow::Borrowed(AJUDANTES);
    }
    let elemento = "%e = call ptr @df.lista_elemento(i64 %a, i64 %i)\nstore i64 %v, ptr %e, align 8\ncall void @df.barreira_elemento(i64 %a, i64 %i, i64 %v)\n";
    let elemento_arc = "%p = add i64 %i, 1\ncall void @dartforge_arc_gravar_ref(i64 %a, i64 %p, i64 %v)\n";
    let dados = "%p = inttoptr i64 %l to ptr\n%q = getelementptr inbounds i8, ptr %p, i64 22\nstore i64 %d, ptr %q, align 8\ncall void @df.barreira(i64 %l, i64 %d)\n";
    let dados_arc = "call void @dartforge_object_set(i64 %l, i64 1, i64 %d, i8 1)\n";
    assert!(AJUDANTES.matches(elemento).count() == 1 && AJUDANTES.matches(dados).count() == 1);
    std::borrow::Cow::Owned(AJUDANTES.replace(elemento, elemento_arc).replace(dados, dados_arc))
}

/// Os efeitos de cada ajudante `@df.*` deste arquivo chamado por
/// `CallRuntime`: `(nome, aloca, lança)`. `externs::efeitos_de` os consulta
/// antes da tabela de externs (um ajudante fora daqui é conservador).
pub(super) const EFEITOS_DOS_AJUDANTES: &[(&str, bool, bool)] = &[
    ("df.lista_len", false, false),
    ("df.lista_armazenamento", false, false),
    ("df.lista_forma", false, false),
    ("df.lista_capacidade", false, false),
    ("df.lista_ler", false, false),
    ("df.lista_gravar_ref", false, false),
    ("df.lista_gravar_len", false, false),
    ("df.lista_gravar_dados", false, false),
];

/// O estado da emissão de listas por módulo.
#[derive(Default)]
pub(super) struct EstadoDeListas {}

/// A tag de ABI de uma palavra do literal misto (a de `LlvmEmitter::tag_de`): 1
/// `int`, 2 `bool`, 3 `Ref`, 4 bits de `double`.
const TAG_REF: u8 = 3;

impl LlvmEmitter<'_> {
    /// Os `alloca`s do bloco de entrada de um `AllocList`: as palavras e as tags
    /// (usadas só no literal misto; o LLVM tira a que sobra).
    pub(super) fn buffer_de_lista(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len().max(1);
        writeln!(self.out, "  %list_buf_{v} = alloca [{n} x i64], align 8").unwrap();
        writeln!(self.out, "  %list_tags_{v} = alloca [{n} x i8], align 1").unwrap();
    }

    /// A lista literal: uma `_GrowableList` de capacidade `n` com as palavras
    /// dos elementos (`dartforge_lista_nova(palavras, n, cid, forma)`).
    pub(super) fn emitir_alloc_list(&mut self, v: u32, elements: &[(Operand, u8)]) {
        let n = elements.len();
        let largura = n.max(1);
        let tags: Vec<u8> = elements.iter().map(|(op, _)| self.tag_de(op)).collect();
        // A forma pelo código de `listas::Elemento` (0 geral, 1 int, 2 double,
        // 3 bool); `None`: misturado.
        let forma: Option<i64> = match tags.first() {
            None => Some(0),
            Some(&t) if tags.iter().all(|&x| x == t) => Some(match t {
                1 => 1,
                4 => 2,
                2 => 3,
                _ => 0,
            }),
            _ => None,
        };
        for (k, (elem, _)) in elements.iter().enumerate() {
            // A palavra na representação do elemento: o `Ref`, o `i64`, os bits
            // do `double` (`bitcast`) ou 0/1 (`zext`).
            let palavra = if tags[k] == TAG_REF { self.coagir(elem, Type::Ref) } else { self.coagir(elem, Type::I64) };
            writeln!(self.out, "  %lp{v}_{k} = getelementptr [{largura} x i64], ptr %list_buf_{v}, i64 0, i64 {k}").unwrap();
            writeln!(self.out, "  store i64 {palavra}, ptr %lp{v}_{k}, align 8").unwrap();
            if forma.is_none() {
                writeln!(self.out, "  %lt{v}_{k} = getelementptr [{largura} x i8], ptr %list_tags_{v}, i64 0, i64 {k}").unwrap();
                writeln!(self.out, "  store i8 {}, ptr %lt{v}_{k}, align 1", tags[k]).unwrap();
            }
        }
        let forma = match forma {
            Some(f) => f.to_string(),
            None => {
                writeln!(self.out, "  %ltp{v} = ptrtoint ptr %list_tags_{v} to i64").unwrap();
                format!("%ltp{v}")
            }
        };
        writeln!(
            self.out,
            "  %v{v} = call i64 @dartforge_lista_nova(ptr %list_buf_{v}, i64 {n}, i64 {}, i64 {forma})",
            cid::GROWABLE_LIST
        )
        .unwrap();
    }
}
