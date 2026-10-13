//! Boxes, closures and records in the lowering (P2, docs/NATIVO-ESPACO-UNIFICADO.md
//! §3.8 and §4.4): the external members done inline and the read of the fields of
//! a `_Closure` (`INSTANCIA`: code, context, typed body, ABI — §2.5).
//!
//! The closure fields are read by `dartforge_object_get` with a constant
//! index, which the emitter expands inline (the word `b+16+8i`; null and
//! `Smi` read zeros): no runtime call.

use super::fn_builder::FnBuilder;
use crate::hir::*;

/// The context field (or the value of the direct environment) of a `_Closure`;
/// field 0 is the code (the uniform entry).
pub(super) const CLOSURE_CONTEXTO: i64 = 1;
/// The field of the typed body.
pub(super) const CLOSURE_TIPADO: i64 = 2;
/// The field of the ABI of the typed body.
pub(super) const CLOSURE_ABI: i64 = 3;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The word `field` of the body of object `obj`, in representation `ty` (the
    /// inline read of `dartforge_object_get`). Only for callers that already know the
    /// class of the object (or that it has the field).
    pub(super) fn object_word(&mut self, obj: Operand, field: i64, ty: Type) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(field)), Type::I64)],
                ret_ty: ty,
            },
            ty,
        )
    }

    /// The context of closure `clo` (the `env` the body receives): the `_Contexto`
    /// of the captures, the value of the direct environment, or null.
    pub(super) fn closure_context(&mut self, clo: Operand) -> Operand {
        if self.ctx.arc_memory {
            // The ABI checks class/map and returns a borrow of the closure,
            // including for the scalar null of canonical tear-offs.
            self.emit(Instruction::CallRuntime {
                name: "dartforge_arc_contexto_closure_v1".to_string(),
                args: vec![(clo, Type::Ref)],
                ret_ty: Type::Ref,
            }, Type::Ref)
        } else {
            self.object_word(clo, CLOSURE_CONTEXTO, Type::Ref)
        }
    }

    /// The external box/closure/record member done inline, or `None` (the
    /// usual call). `member` is `Class.name`; `native`, the name of the native,
    /// if any.
    pub(super) fn inline_box(
        &mut self,
        member: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let _ = member;
        match (native?, this, args) {
            // `identical(a, b)` (§2.10): `@df.identico`, without a call.
            ("Identical_comparison", None, [a, b]) => {
                let a = self.coerce(a.clone(), Type::Ref);
                let b = self.coerce(b.clone(), Type::Ref);
                Some(self.emit(
                    Instruction::CallRuntime {
                        name: "df.identico".to_string(),
                        args: vec![(a, Type::Ref), (b, Type::Ref)],
                        ret_ty: Type::I1,
                    },
                    Type::I1,
                ))
            }
            // `_Smi.hashCode`/`_Mint.hashCode`: the VM's `HashIntegerOp`
            // (`il_x64.cc`): the 96-bit product `v · 0x2d51` (v unsigned),
            // its three 32-bit words combined by xor, 30 bits.
            ("DartForge_int_hashCode", Some(t), []) => {
                let v = self.coerce(t.clone(), Type::I64);
                let k = |c: i64| Operand::Constant(Constant::Int(c));
                let lo32 = self.emit(Instruction::And(v.clone(), k(0xffff_ffff)), Type::I64);
                let hi32 = self.emit(Instruction::LShr(v, k(32)), Type::I64);
                let p_lo = self.emit(Instruction::Mul(lo32, k(0x2d51)), Type::I64);
                let p_hi = self.emit(Instruction::Mul(hi32, k(0x2d51)), Type::I64);
                let goes = self.emit(Instruction::LShr(p_lo.clone(), k(32)), Type::I64);
                let p_hi = self.emit(Instruction::Add(p_hi, goes), Type::I64);
                // c0 ^ c1 ^ c2: p_lo (low), p_hi (low) and p_hi >> 32.
                let c2 = self.emit(Instruction::LShr(p_hi.clone(), k(32)), Type::I64);
                let x = self.emit(Instruction::Xor(p_lo, p_hi), Type::I64);
                let x = self.emit(Instruction::Xor(x, c2), Type::I64);
                Some(self.emit(Instruction::And(x, k(0x3fff_ffff)), Type::I64))
            }
            // `_Double._bitsDe(d)`: the bits of the `double`.
            ("DartForge_double_bits", None, [d]) | ("DartForge_double_bits", Some(d), []) => {
                let d = self.coerce(d.clone(), Type::F64);
                Some(self.emit(Instruction::Bitcast { op: d, to: Type::I64 }, Type::I64))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod context_tests {
    use crate::{context::Context, hir::*};
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn source_uses_borrowed_context_only_in_arc_mode() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry,
                "int topo() => 7; int Function() criar(int valor) => () => valor; const List<int> valores = [1, 2]; void main() { valores; criar(42)(); topo(); }\n").unwrap();
            let sdk = crate::sdk_modulo::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, body_diags) = dartforge_types::infer_program_bodies(
                &program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&body_diags) {
                assert!(!dartforge_types::codes::e_erro_de_compilacao(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                if arc {
                    assert!(!m.dart_vector_parameters.is_empty());
                    assert!(!m.arc_uniform_returns.is_empty());
                    assert!(!m.arc_rti_caches.is_empty());
                    assert!(!m.arc_shared_functions.is_empty());
                    assert!(m.functions.iter().any(|f| f.name == "const" && m.arc_shared_functions.contains(&f.symbol)));
                    for cache in &m.arc_rti_caches {
                        assert!(m.globals.iter().any(|g| g.0 == 0 && g.1 == Type::I64 && &g.2 == cache));
                    }
                    for (symbol, roles) in &m.dart_vector_parameters {
                        let f = m.functions.iter().find(|f| &f.symbol == symbol).unwrap();
                        assert_eq!(roles.len(), 2);
                        assert!(m.arc_uniform_returns.contains(symbol));
                        for (id, role) in roles {
                            let p = f.params.iter().find(|(v, _, _)| v == id).unwrap();
                            assert_eq!(p.2, Type::Ptr);
                            assert_eq!(p.1, match role { DartEntryVector::Arguments => "args", DartEntryVector::Descriptor => "desc" });
                        }
                    }
                } else { assert!(m.dart_vector_parameters.is_empty()); assert!(m.arc_uniform_returns.is_empty()); assert!(m.arc_rti_caches.is_empty()); assert!(m.arc_shared_functions.is_empty()); }

                let mut borrows = 0;
                let mut raw_reads = 0;
                for (_, inst, ty) in m.functions.iter().flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
                    if let Instruction::CallRuntime { name, args, ret_ty } = inst {
                        if name == "dartforge_arc_contexto_closure_v1" {
                            assert_eq!((*ret_ty, *ty), (Type::Ref, Type::Ref));
                            assert_eq!(args.len(), 1);
                            assert_eq!(args[0].1, Type::Ref);
                            borrows += 1;
                        }
                        if name == "dartforge_object_get" && *ret_ty == Type::Ref
                            && args.get(1).is_some_and(|a| a.0 == Operand::Constant(Constant::Int(1))) {
                            raw_reads += 1;
                        }
                    }
                }
                let mut errors = 0;
                for b in m.functions.iter().flat_map(|f| &f.blocks) {
                    for (v, i, _) in &b.instructions {
                        if matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_nsm_chamada") {
                            errors += 1;
                            if arc {
                                let Terminator::CondBranch { cond, then_block, else_block } = &b.terminator else {
                                    panic!("NSM without a pending exit under ARC");
                                };
                                assert_ne!(then_block, else_block);
                                let last_ones = &b.instructions[b.instructions.len() - 3..];
                                assert_eq!(last_ones[0].0, *v);
                                assert!(matches!(&last_ones[1].1, Instruction::CallRuntime { name, args, ret_ty }
                                    if name == "dartforge_exception_pending" && args.is_empty() && *ret_ty == Type::I8));
                                assert!(matches!(&last_ones[2].1, Instruction::ICmp(ICmpOp::Ne, Operand::Val(p), Operand::Constant(Constant::Int(0)))
                                    if *p == last_ones[1].0));
                                assert_eq!(*cond, Operand::Val(last_ones[2].0));
                            } else {
                                assert!(matches!(b.terminator, Terminator::Return(_)));
                            }
                        }
                    }
                }
                assert!(errors > 0);
                if arc { assert!(borrows > 0); assert_eq!(raw_reads, 0); }
                else { assert_eq!(borrows, 0); assert!(raw_reads > 0); }
            }
        }).unwrap().join().unwrap();
    }
}
