//! A emissão das listas tipadas e dos valores SIMD (P4,
//! docs/NATIVO-ESPACO-UNIFICADO.md §3.5 e §4.6): a caixa e a descaixa de um vetor
//! SIMD e os ajudantes `@df.tipada_*`, `@df.nucleo_*` e `@df.simd_caixa` do
//! prelúdio.
//!
//! Os ajudantes leem o bloco pelos deslocamentos do contrato
//! (`dartforge_runtime::layout`; a palavra em `b + k` está em `h + k − 2`), sem
//! chamar o runtime:
//!
//! * lista tipada (interna, externa ou visão): o comprimento em `h+14` e o
//!   endereço dos bytes em `h+22`, nos três formatos; nenhum dos dois muda
//!   enquanto a lista vive (a lista tipada não muda de tamanho e o coletor não
//!   move), então as cargas levam `!invariant.load` e o LLVM as tira dos laços
//!   como tirava as funções puras `dartforge_typed_len`/`_ptr` de antes; o cid
//!   (`h+2`) também é invariante (§2.1, princípio 4);
//! * lista do núcleo (`_List`, `_ImmutableList`, `_GrowableList`): o comprimento
//!   em `h+14`, o armazenamento da expansível em `h+22`, a forma no `flags` do
//!   armazenamento (`a−1`) e os elementos em `a+22`. Mutáveis: cargas comuns;
//! * SIMD: as pistas em `h+14`.
//!
//! O lowering (`lower/tipados.rs`) os chama como `CallRuntime` com o nome sem o
//! `@`; os efeitos estão em [`EFEITOS_DOS_AJUDANTES`], que `externs::efeitos_de`
//! consulta (P0).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// Os ajudantes de listas tipadas, listas do núcleo e SIMD (§3.5), impressos no
/// prelúdio de cada módulo por `emit_runtime_decls`. As constantes numéricas são
/// as de `layout` (conferidas no teste `deslocamentos_do_contrato`).
pub const AJUDANTES: &str = "\
define internal i64 @df.tipada_len(i64 %t) alwaysinline {
  %a = add i64 %t, 14
  %p = inttoptr i64 %a to ptr
  %n = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %n
}
define internal i64 @df.tipada_dados(i64 %t) alwaysinline {
  %a = add i64 %t, 22
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal i64 @df.tipada_cid(i64 %t) alwaysinline {
  %a = add i64 %t, 2
  %p = inttoptr i64 %a to ptr
  %c = load i32, ptr %p, align 4, !invariant.load !{}
  %r = sext i32 %c to i64
  ret i64 %r
}
define internal i64 @df.tipada_len_gravavel(i64 %t, i64 %imutavel) alwaysinline {
  %c = call i64 @df.tipada_cid(i64 %t)
  %n = call i64 @df.tipada_len(i64 %t)
  %i = icmp eq i64 %c, %imutavel
  %r = select i1 %i, i64 0, i64 %n
  ret i64 %r
}
define internal i64 @df.tipada_bytes(i64 %t) alwaysinline {
  %c = call i64 @df.tipada_cid(i64 %t)
  %n = call i64 @df.tipada_len(i64 %t)
  %d = sub i64 %c, 22
  %interna = icmp ult i64 %d, 14
  %v = sub i64 %c, 36
  %vt = urem i64 %v, 14
  %bd = icmp sge i64 %c, 64
  %tv = select i1 %bd, i64 14, i64 %vt
  %tipo = select i1 %interna, i64 %d, i64 %tv
  %s = mul i64 %tipo, 4
  %tab = lshr i64 19214116860268544, %s
  %lg = and i64 %tab, 15
  %r = shl i64 %n, %lg
  ret i64 %r
}
define internal i64 @df.tipada_base(i64 %t) alwaysinline {
  %a = add i64 %t, 30
  %p = inttoptr i64 %a to ptr
  %b = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %b
}
define internal i64 @df.tipada_deslocamento(i64 %t) alwaysinline {
  %a = add i64 %t, 38
  %p = inttoptr i64 %a to ptr
  %d = load i64, ptr %p, align 8, !invariant.load !{}
  ret i64 %d
}
define internal i64 @df.nucleo_len(i64 %l) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
  %d = sub i64 %c, 8
  %e = icmp ult i64 %d, 3
  br i1 %e, label %lista, label %fim
lista:
  %a = add i64 %l, 14
  %p = inttoptr i64 %a to ptr
  %n = load i64, ptr %p, align 8
  br label %fim
fim:
  %r = phi i64 [ %n, %lista ], [ 0, %entrada ]
  ret i64 %r
}
define internal i64 @df.nucleo_armazenamento(i64 %l) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
  %e = icmp eq i64 %c, 10
  br i1 %e, label %expansivel, label %fim
expansivel:
  %a = add i64 %l, 22
  %p = inttoptr i64 %a to ptr
  %s = load i64, ptr %p, align 8
  br label %fim
fim:
  %r = phi i64 [ %s, %expansivel ], [ %l, %entrada ]
  ret i64 %r
}
define internal i64 @df.nucleo_forma(i64 %a) alwaysinline {
  %f = add i64 %a, -1
  %p = inttoptr i64 %f to ptr
  %b = load i8, ptr %p, align 1
  %m = and i8 %b, 102
  %r = zext i8 %m to i64
  ret i64 %r
}
define internal i64 @df.nucleo_len_gravavel(i64 %l, i64 %forma) alwaysinline {
entrada:
  %c = call i64 @df.classe(i64 %l)
  %fixa = icmp eq i64 %c, 8
  %exp = icmp eq i64 %c, 10
  %mod = or i1 %fixa, %exp
  br i1 %mod, label %forma.conf, label %fim
forma.conf:
  %a = call i64 @df.nucleo_armazenamento(i64 %l)
  %f = call i64 @df.nucleo_forma(i64 %a)
  %ok = icmp eq i64 %f, %forma
  %la = add i64 %l, 14
  %lp = inttoptr i64 %la to ptr
  %n = load i64, ptr %lp, align 8
  %g = select i1 %ok, i64 %n, i64 0
  br label %fim
fim:
  %r = phi i64 [ %g, %forma.conf ], [ 0, %entrada ]
  ret i64 %r
}
define internal i64 @df.palavra_ref(i64 %e, i64 %i) alwaysinline {
  %o = shl i64 %i, 3
  %a = add i64 %e, %o
  %p = inttoptr i64 %a to ptr
  %r = load i64, ptr %p, align 8
  ret i64 %r
}
define internal i64 @df.simd_caixa(<2 x i64> %v, i64 %cid) alwaysinline {
  %c = shl i64 %cid, 32
  %cab = or i64 %c, 131585
  %h = call i64 @df.alocar(i64 %cab, i64 2)
  %a = add i64 %h, 14
  %p = inttoptr i64 %a to ptr
  store <2 x i64> %v, ptr %p, align 8
  ret i64 %h
}
";

/// Os efeitos dos ajudantes chamados pelo lowering como `CallRuntime` (o nome sem
/// `@`): `(nome, aloca, lança)`. `externs::efeitos_de` (P0) os consulta em vez de
/// tratá-los como externs desconhecidas (conservadoras). Nenhum chama o Dart.
pub const EFEITOS_DOS_AJUDANTES: &[(&str, bool, bool)] = &[
    ("df.tipada_len", false, false),
    ("df.tipada_dados", false, false),
    ("df.tipada_cid", false, false),
    ("df.tipada_len_gravavel", false, false),
    ("df.tipada_bytes", false, false),
    ("df.tipada_base", false, false),
    ("df.tipada_deslocamento", false, false),
    ("df.nucleo_len", false, false),
    ("df.nucleo_armazenamento", false, false),
    ("df.nucleo_forma", false, false),
    ("df.nucleo_len_gravavel", false, false),
    ("df.palavra_ref", false, false),
    ("df.simd_caixa", true, false),
];

/// O estado da emissão de listas tipadas por módulo.
#[derive(Default)]
pub(super) struct EstadoDeTipados {}

impl LlvmEmitter<'_> {
    /// O vetor SIMD `op` (do tipo `from`) em caixa: `@df.simd_caixa`, alocação em
    /// linha com as pistas em `h+14` (cid 19 `_Float32x4`, 20 `_Int32x4`, 21
    /// `_Float64x2`).
    pub(super) fn emitir_caixa_simd(&mut self, v: u32, op: &Operand, from: Type) {
        let so = self.coagir(op, from);
        let cid = match from {
            Type::V4F32 => dartforge_runtime::layout::cid::FLOAT32X4,
            Type::V4I32 => dartforge_runtime::layout::cid::INT32X4,
            _ => dartforge_runtime::layout::cid::FLOAT64X2,
        };
        writeln!(self.out, "  %bx{v} = bitcast {} {so} to <2 x i64>", from.llvm_ir()).unwrap();
        writeln!(self.out, "  %v{v} = call i64 @df.simd_caixa(<2 x i64> %bx{v}, i64 {cid})").unwrap();
    }

    /// O vetor SIMD (do tipo `to`) de uma caixa: as pistas em `h+14`. Sem
    /// `!invariant.load`: a caixa pode ter sido gravada na mesma função
    /// (`@df.simd_caixa`), e a carga não pode subir acima da gravação.
    pub(super) fn emitir_descaixa_simd(&mut self, v: u32, op: &Operand, to: Type) {
        let so = self.coagir(op, Type::Ref);
        writeln!(self.out, "  %ux{v} = add i64 {so}, 14").unwrap();
        writeln!(self.out, "  %up{v} = inttoptr i64 %ux{v} to ptr").unwrap();
        writeln!(self.out, "  %v{v} = load {}, ptr %up{v}, align 8", to.llvm_ir()).unwrap();
    }
}

#[cfg(test)]
mod testes {
    use dartforge_runtime::layout::{self, desl, flags};

    /// As constantes escritas à mão em [`super::AJUDANTES`] são as do contrato.
    #[test]
    fn deslocamentos_do_contrato() {
        let h = |b: usize| b as i64 - layout::DESLOCAMENTO_DO_HANDLE;
        assert_eq!(h(desl::COMPRIMENTO), 14);
        assert_eq!(h(desl::DADOS), 22);
        assert_eq!(h(desl::CLASSE), 2);
        assert_eq!(h(desl::BASE_DA_VISAO), 30);
        assert_eq!(h(desl::DESLOCAMENTO_DA_VISAO), 38);
        assert_eq!(h(desl::EXPANSIVEL_DADOS), 22);
        assert_eq!(h(desl::ELEMENTOS), 22);
        assert_eq!(h(desl::VALOR), 14);
        assert_eq!(h(desl::FLAGS), -1);
        assert_eq!(flags::FORMA | flags::ELEMENTO, 102);
        assert_eq!(layout::cid::PRIMEIRA_TIPADA, 22);
        assert_eq!(layout::cid::PRIMEIRA_VISAO, 36);
        assert_eq!(layout::cid::BYTE_DATA_VIEW, 64);
        assert_eq!(layout::cid::GROWABLE_LIST, 10);
        assert_eq!(layout::cid::LIST, 8);
        // A palavra 0 do cabeçalho de um SIMD, sem o cid.
        assert_eq!(layout::palavra_do_cabecalho(layout::estado::JOVEM, flags::BRUTO, 2, 0), 131_585);
        // A tabela de `log2` do tamanho do elemento por tipo (4 bits por tipo).
        let mut tabela = 0u64;
        for t in 0..=14u8 {
            let lg = dartforge_runtime::tipadas::tamanho_do_elemento(t).trailing_zeros() as u64;
            tabela |= lg << (4 * t);
        }
        assert_eq!(tabela, 19_214_116_860_268_544);
        for (nome, _, _) in super::EFEITOS_DOS_AJUDANTES {
            assert!(super::AJUDANTES.contains(&format!("@{nome}(")), "{nome} sem definição");
        }
    }
}
