//! Emission of typed lists and SIMD values (P4,
//! docs/NATIVO-ESPACO-UNIFICADO.md §3.5 and §4.6): boxing and unboxing of a SIMD
//! vector and the `@df.tipada_*`, `@df.nucleo_*` and `@df.simd_caixa` helpers of the
//! prelude.
//!
//! The helpers read the block through the offsets of the contract
//! (`dartforge_runtime::layout`; the word at `b + k` is at `h + k − 2`), without
//! calling the runtime:
//!
//! * typed list (internal, external or view): the length at `h+14` and the
//!   address of the bytes at `h+22`, in all three formats; neither changes
//!   while the list lives (a typed list does not change size and the collector does not
//!   move), so the loads carry `!invariant.load` and LLVM hoists them out of loops
//!   as it did with the former pure functions `dartforge_typed_len`/`_ptr`; the cid
//!   (`h+2`) is invariant too (§2.1, principle 4);
//! * core list (`_List`, `_ImmutableList`, `_GrowableList`): the length
//!   at `h+14`, the storage of the growable one at `h+22`, the shape in the `flags` of the
//!   storage (`a−1`) and the elements at `a+22`. Mutable: ordinary loads;
//! * SIMD: the lanes at `h+14`.
//!
//! The lowering (`lower/typed_items.rs`) calls them as `CallRuntime` with the name without the
//! `@`; the effects are in [`HELPER_EFFECTS`], which `externs::effects_of`
//! consults (P0).

use super::LlvmEmitter;
use crate::hir::*;
use std::fmt::Write;

/// The helpers for typed lists, core lists and SIMD (§3.5), printed in the
/// prelude of each module by `emit_runtime_decls`. The numeric constants are
/// those of `layout` (checked in the `contract_offsets` test).
pub const HELPERS: &str = "\
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
define internal void @df.manter_vivo(i64 %l) alwaysinline {
  ret void
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

/// The effects of the helpers called by the lowering as `CallRuntime` (the name without
/// `@`): `(name, allocates, throws)`. `externs::effects_of` (P0) consults them instead of
/// treating them as unknown (conservative) externs. None calls Dart.
pub const HELPER_EFFECTS: &[(&str, bool, bool)] = &[
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
    ("df.manter_vivo", false, false),
    ("df.simd_caixa", true, false),
];

/// The per-module emission state of typed lists.
#[derive(Default)]
pub(super) struct TypedState {}

impl LlvmEmitter<'_> {
    /// The SIMD vector `op` (of type `from`) boxed: `@df.simd_caixa`, an inline
    /// allocation with the lanes at `h+14` (cid 19 `_Float32x4`, 20 `_Int32x4`, 21
    /// `_Float64x2`).
    pub(super) fn emit_simd_box(&mut self, v: u32, op: &Operand, from: Type) {
        let only = self.coerce(op, from);
        let cid = match from {
            Type::V4F32 => dartforge_runtime::layout::cid::FLOAT32X4,
            Type::V4I32 => dartforge_runtime::layout::cid::INT32X4,
            _ => dartforge_runtime::layout::cid::FLOAT64X2,
        };
        writeln!(self.out, "  %bx{v} = bitcast {} {only} to <2 x i64>", from.llvm_ir()).unwrap();
        if self.module.arc_memory {
            self.externals.insert("dartforge_arc_box_simd_owned_v1".into(),
                "declare i64 @dartforge_arc_box_simd_owned_v1(i64, i64, i64) nounwind".into());
            writeln!(self.out, "  %bl{v} = extractelement <2 x i64> %bx{v}, i32 0").unwrap();
            writeln!(self.out, "  %bh{v} = extractelement <2 x i64> %bx{v}, i32 1").unwrap();
            writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_box_simd_owned_v1(i64 {cid}, i64 %bl{v}, i64 %bh{v})").unwrap();
        } else {
            writeln!(self.out, "  %v{v} = call i64 @df.simd_caixa(<2 x i64> %bx{v}, i64 {cid})").unwrap();
        }
    }

    /// The SIMD vector (of type `to`) of a box: the lanes at `h+14`. Without
    /// `!invariant.load`: the box may have been written in the same function
    /// (`@df.simd_caixa`), and the load cannot be hoisted above the write.
    pub(super) fn emit_simd_unbox(&mut self, v: u32, op: &Operand, to: Type) {
        let only = self.coerce(op, Type::Ref);
        writeln!(self.out, "  %ux{v} = add i64 {only}, 14").unwrap();
        writeln!(self.out, "  %up{v} = inttoptr i64 %ux{v} to ptr").unwrap();
        writeln!(self.out, "  %v{v} = load {}, ptr %up{v}, align 8", to.llvm_ir()).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use dartforge_runtime::layout::{self, desl, flags};

    /// The constants written by hand in [`super::HELPERS`] are those of the contract.
    #[test]
    fn contract_offsets() {
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
        // Word 0 of the header of a SIMD value, without the cid.
        assert_eq!(layout::palavra_do_cabecalho(layout::estado::JOVEM, flags::BRUTO, 2, 0), 131_585);
        // The `log2` table of the element size per type (4 bits per type).
        let mut table = 0u64;
        for t in 0..=14u8 {
            let lg = dartforge_runtime::tipadas::tamanho_do_elemento(t).trailing_zeros() as u64;
            table |= lg << (4 * t);
        }
        assert_eq!(table, 19_214_116_860_268_544);
        for (name, _, _) in super::HELPER_EFFECTS {
            assert!(super::HELPERS.contains(&format!("@{name}(")), "{name} without a definition");
        }
    }
}
