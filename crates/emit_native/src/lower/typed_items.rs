//! The fast path of `[]`, `[]=` and `length` when the static type of the
//! receiver is a numeric typed list (`Int8List` … `Float64List` of
//! `dart:typed_data`) or a core `List<E>`, and the external members of
//! typed lists done inline (`typed_inline`).
//!
//! These `dart:typed_data` classes are `final` in the SDK: the value is always the
//! internal list (`_Int32List`, also over external memory), or a view
//! (`_Int32ArrayView`, and the unmodifiable one) of `typed_data_patch.dart`. In the
//! unified space (docs/NATIVO-ESPACO-UNIFICADO.md §2.5) the three forms have
//! the length at `h+14` and the address of the bytes at `h+22`, invariant
//! while the list lives: `@df.typed_len`/`@df.typed_data`
//! (`llvm/ir_typed.rs`) read them inline with `!invariant.load`, and LLVM hoists
//! the loads out of loops (and vectorizes). On writes, the unmodifiable view
//! (checked by the cid, also invariant) gives length 0. An index out of
//! bounds falls into the usual dispatch, with the same errors as the VM.
//!
//! `List` is not `final`: a user class may implement it. The class of the
//! receiver (`@df.class`) says whether it is a runtime list (`_List`,
//! `_ImmutableList`, `_GrowableList`, cids 8–10); any other gets length 0
//! and keeps the dispatch. The length is at `h+14` in all three; the elements,
//! in the storage (the list itself, or the `_List` at `h+22` of the growable one),
//! one word each starting at `a+22`, with the shape in its `flags` (§2.16):
//! `REFS` (each word a `Ref`) or compact of `int`, `double` or `bool`
//! (the unboxed bits). The read checks the shape (except in the N13 iterations,
//! which check it before); a direct write only with `E` equal to `int`,
//! `double` or `bool` (which no class extends; with another `E`, the list may
//! be of a subtype and the SDK `[]=` checks the value), into a `_List` or
//! `_GrowableList` (the `_ImmutableList` does not accept it) of the compact shape of `E`
//! (`@df.core_len_writable`): a scalar without a barrier.

use super::fn_builder::FnBuilder;
use crate::hir::*;
use dartforge_types::TypeId;
use dartforge_types::table::Type as T;

impl FnBuilder<'_, '_> {
    /// The fixed list of the iterations being emitted that `target` (a local) names (N13).
    pub(super) fn fixed_list_of(
        &self,
        ast: &dartforge_frontend::ast::Ast,
        target: dartforge_frontend::ast::ExprId,
    ) -> Option<super::fn_builder::FixedList> {
        if self.fixed_lists.is_empty() {
            return None;
        }
        let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(target).kind else {
            return None;
        };
        let super::locals::Mode::Memory(key) = self.lookup_local(n.sym)?.mode else {
            return None;
        };
        self.fixed_lists
            .iter()
            .rev()
            .find(|f| f.key == key)
            .cloned()
    }

    /// The proven typed list of the fast iterations being emitted
    /// (`statements::Counted::TypedSet`) that `target` (a local) names; with
    /// `index`, only if it is the loop counter (`target[i]`). The local is
    /// identified by its `alloca`, so a same-named local declared in the body does not
    /// match.
    pub(super) fn proven_of(
        &self,
        ast: &dartforge_frontend::ast::Ast,
        target: dartforge_frontend::ast::ExprId,
        index: Option<dartforge_frontend::ast::ExprId>,
    ) -> Option<super::fn_builder::ProvenTyped> {
        if self.proven_typed.is_empty() {
            return None;
        }
        let address = |e: dartforge_frontend::ast::ExprId| {
            let dartforge_frontend::ast::ExprKind::Identifier(n) = &ast.expr(e).kind else {
                return None;
            };
            match self.lookup_local(n.sym)?.mode {
                super::locals::Mode::Memory(p) => Some(p),
                _ => None,
            }
        };
        let key = address(target)?;
        let counter = match index {
            Some(i) => Some(address(i)?),
            None => None,
        };
        self.proven_typed
            .iter()
            .rev()
            .find(|p| p.key == key && counter.as_ref().is_none_or(|c| *c == p.counter))
            .cloned()
    }

    /// The address of the elements and the length of a typed or SIMD list
    /// (`list`, of static type `ix`), read before the iterations of a
    /// versioned loop; `write`: the length of an unmodifiable view is 0.
    pub(super) fn proven_data(
        &mut self,
        list: &Operand,
        ix: Indexable,
        write: bool,
    ) -> Option<(Operand, Operand)> {
        let list = self.coerce(list.clone(), Type::Ref);
        match ix {
            Indexable::Typed(TypedList { ty: typ, .. }) | Indexable::Simd { ty: typ, .. } => {
                Some(self.typed_data_and_length(&list, typ, write))
            }
            Indexable::Core { .. } => None,
        }
    }
}

/// A receiver with a fast index path.
#[derive(Debug, Clone, Copy)]
pub enum Indexable {
    Typed(TypedList),
    /// `List<E>`: the representation of the element that is written directly (`E`
    /// equal to `int` or `double`), or `None` (direct read only).
    Core {
        write_op: Option<Type>,
    },
    /// `Float32x4List`, `Int32x4List`, `Float64x2List`: the type in the runtime
    /// and the unboxed vector of the element (16 bytes, read and written as an
    /// LLVM vector).
    Simd {
        ty: i64,
        k: Type,
    },
}

/// A numeric typed list: the element type in the runtime (`TIPO_*` of
/// `runtime/src/typed_data.rs`) and the C type of the element.
#[derive(Debug, Clone, Copy)]
pub struct TypedList {
    pub ty: i64,
    pub element: CType,
}

impl TypedList {
    /// The Dart representation of the element.
    fn repr(self) -> Type {
        if matches!(self.element, CType::F32 | CType::F64) {
            Type::F64
        } else {
            Type::I64
        }
    }

    /// Can `[]=` write directly? `Uint8ClampedList` saturates the value; it
    /// keeps the SDK `[]=`.
    fn direct_write(self) -> bool {
        self.ty != 2
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The fast index path of static type `ty` (non-nullable), if
    /// any.
    pub(super) fn indexable(&self, ty: Option<TypeId>) -> Option<Indexable> {
        let T::Interface {
            class,
            args,
            nullable: false,
        } = self.ctx.table.get(ty?)
        else {
            return None;
        };
        let c = self.ctx.program.classes.get(class.0 as usize)?;
        let library = self.ctx.program.library(c.library).uri.as_str();
        let name_str = self.ctx.symbol_name(c.name);
        if library == "dart:core" && name_str == "List" {
            let write_op = match args.first().map(|&e| self.ctx.table.get(e)) {
                Some(T::Interface {
                    class: e,
                    nullable: false,
                    ..
                }) => {
                    let e = self.ctx.program.classes.get(e.0 as usize)?;
                    let core = self.ctx.program.library(e.library).uri == "dart:core";
                    match self.ctx.symbol_name(e.name) {
                        "int" if core => Some(Type::I64),
                        "double" if core => Some(Type::F64),
                        "bool" if core => Some(Type::I1),
                        _ => None,
                    }
                }
                _ => None,
            };
            return Some(Indexable::Core { write_op });
        }
        // Under ARC, each access through the address of the bytes gets
        // `@df.keep_alive(list)` in the preparation (`optimize/arc/derived_set.rs`).
        if library != "dart:typed_data" {
            return None;
        }
        let (typ, element) = match name_str {
            "Int8List" => (0, CType::I8),
            "Uint8List" => (1, CType::U8),
            "Uint8ClampedList" => (2, CType::U8),
            "Int16List" => (3, CType::I16),
            "Uint16List" => (4, CType::U16),
            "Int32List" => (5, CType::I32),
            "Uint32List" => (6, CType::U32),
            "Int64List" => (7, CType::I64),
            "Uint64List" => (8, CType::U64),
            "Float32List" => (9, CType::F32),
            "Float64List" => (10, CType::F64),
            "Float32x4List" => return self.simd_list(11, Type::V4F32),
            "Int32x4List" => return self.simd_list(12, Type::V4I32),
            "Float64x2List" => return self.simd_list(13, Type::V2F64),
            _ => return None,
        };
        Some(Indexable::Typed(TypedList { ty: typ, element }))
    }

    /// `list.fillRange(start, end, value)` of a numeric typed list
    /// (static type `Int8List` … `Float64List`, except
    /// `Uint8ClampedList`), with the three arguments non-nullable `int`/`int`/element:
    /// the runtime fills the range at once
    /// (`dartforge_typed_fill_int`/`_double`); an invalid range, an empty list
    /// or an unmodifiable view go back to the SDK `fillRange` with the same
    /// values, which throws the VM error. The `dart:typed_data` classes are
    /// `final`: the member is always the SDK one.
    pub(super) fn fill_typed(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        e: dartforge_frontend::ast::ExprId,
    ) -> Option<Operand> {
        use dartforge_frontend::ast::{ExprId, ExprKind};
        let ExprKind::Call { target, arguments } = &ast.expr(e).kind else {
            return None;
        };
        if arguments.args.len() != 3
            || arguments.args.iter().any(|a| a.name.is_some())
            || !arguments.type_args.is_empty()
        {
            return None;
        }
        let ExprKind::Property {
            target: recv,
            name,
            null_aware: false,
        } = &ast.expr(*target).kind
        else {
            return None;
        };
        if self.ctx.symbol_name(name.sym) != "fillRange" {
            return None;
        }
        let Some(Indexable::Typed(l)) = self.indexable(self.ctx.get_type(self.unit_id, *recv))
        else {
            return None;
        };
        if !l.direct_write() {
            return None;
        }
        let repr = l.repr();
        let class_of =
            |s: &Self, x: ExprId| match s.ctx.get_type(s.unit_id, x).map(|t| s.ctx.table.get(t)) {
                Some(T::Interface {
                    class,
                    nullable: false,
                    ..
                }) => Some(*class),
                _ => None,
            };
        let int = self.ctx.core.int_class;
        let element = if repr == Type::F64 {
            self.ctx.core.double_class
        } else {
            int
        };
        let [a, b, v] = [
            arguments.args[0].value,
            arguments.args[1].value,
            arguments.args[2].value,
        ];
        if int.is_none()
            || class_of(self, a) != int
            || class_of(self, b) != int
            || class_of(self, v) != element
        {
            return None;
        }
        // Dart evaluation order: receiver, then the arguments.
        let mut ops = Vec::with_capacity(4);
        for (x, t) in [
            (*recv, Type::Ref),
            (a, Type::I64),
            (b, Type::I64),
            (v, repr),
        ] {
            let op = self.lower_expr(ast, x);
            if self.is_terminated() {
                return Some(Operand::Constant(Constant::Null));
            }
            ops.push(self.coerce(op, t));
        }
        let name_str = if repr == Type::F64 {
            "dartforge_typed_fill_double"
        } else {
            "dartforge_typed_fill_int"
        };
        let done = self.emit(
            Instruction::CallRuntime {
                name: name_str.to_string(),
                args: vec![
                    (ops[0].clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(l.ty)), Type::I64),
                    (ops[1].clone(), Type::I64),
                    (ops[2].clone(), Type::I64),
                    (ops[3].clone(), repr),
                ],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let ok = self.emit(
            Instruction::ICmp(ICmpOp::Ne, done, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let slow = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: join,
            else_block: slow,
        });
        self.set_block(slow);
        let args: Vec<super::members::Evaluated> =
            ops[1..].iter().map(|o| (None, o.clone())).collect();
        self.call_by_name(
            ops[0].clone(),
            super::sdk_source::AccessKind::Call,
            "fillRange",
            &args,
        );
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        Some(Operand::Constant(Constant::Null))
    }

    /// A SIMD list: the unboxed vector does not cross asynchronous frames
    /// (there the element keeps the dispatch).
    fn simd_list(&self, typ: i64, k: Type) -> Option<Indexable> {
        self.async_state
            .is_none()
            .then_some(Indexable::Simd { ty: typ, k })
    }

    /// A prelude helper (`llvm/ir_typed.rs`): an inline read of the
    /// block, without effects (`HELPER_EFFECTS`).
    fn typed_helper(&mut self, name_str: &str, args: Vec<(Operand, Type)>, ret: Type) -> Operand {
        self.emit(
            Instruction::CallRuntime {
                name: name_str.to_string(),
                args,
                ret_ty: ret,
            },
            ret,
        )
    }

    /// The length for the fast path, or 0 if it does not apply: that of the typed
    /// list (0 for an unmodifiable view, on writes), or that of the runtime
    /// list (0 for whatever is not one; on direct writes, 0 also for the
    /// immutable one or one of another shape).
    fn fast_length(&mut self, list: &Operand, ix: Indexable, write: bool) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let write_op = match ix {
            Indexable::Typed(TypedList { ty: typ, .. }) | Indexable::Simd { ty: typ, .. } => {
                let (data, n) = self.typed_data_and_length(list, typ, write);
                self.typed_data = Some((list.clone(), data));
                return n;
            }
            Indexable::Core { write_op } => write_op,
        };
        let Some(t) = write_op.filter(|_| write) else {
            // The logical length, inline; in a fixed list of the iterations
            // (N13), the one read before them.
            return match &self.access_fixed {
                Some(f) => f.length.clone(),
                None => {
                    self.typed_helper("df.core_len", vec![(list.clone(), Type::Ref)], Type::I64)
                }
            };
        };
        // N14: in the N13 iterations, the direct write was checked before them.
        if let Some(n) = self
            .access_fixed
            .as_ref()
            .and_then(|f| f.writable_length.clone())
        {
            return n;
        }
        // Direct write: `_List` or `_GrowableList` (the class says it is
        // modifiable) with the storage in the compact shape of `E` (which only
        // exists with the reified `E` exactly `int`, `double` or `bool`:
        // covariance is satisfied).
        self.typed_helper(
            "df.core_len_writable",
            vec![(list.clone(), Type::Ref), (int(shape_code(t)), Type::I64)],
            Type::I64,
        )
    }

    /// `list.length`.
    pub(super) fn indexed_length(&mut self, list: Operand, ix: Indexable) -> Operand {
        // A proven typed list of the fast iterations: the length read before.
        if let Some(p) = self.access_proven.take()
            && let Indexable::Typed(_) | Indexable::Simd { .. } = ix
        {
            return p.length;
        }
        let list = self.coerce(list, Type::Ref);
        // N13: the runtime list of the iterations (non-null, fixed length).
        if let (Indexable::Core { .. }, Some(f)) = (ix, &self.access_fixed) {
            return f.length.clone();
        }
        let n = self.fast_length(&list, ix, false);
        self.typed_data = None;
        if let Indexable::Typed(_) | Indexable::Simd { .. } = ix
            && !crate::target::is_wasm()
        {
            // The static type guarantees the typed list of the type: the length
            // is that one. On WebAssembly the list may be a JavaScript-backed one
            // (`dart:_js_types`), whose `@df.typed_len` is 0: the getter answers.
            return n;
        }
        // 0: empty, or not a runtime list — the getter answers.
        let empty = self.emit(
            Instruction::ICmp(ICmpOp::Eq, n.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let fast_block = self.new_block();
        let slow_block = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: empty,
            then_block: slow_block,
            else_block: fast_block,
        });
        self.set_block(fast_block);
        self.terminate(Terminator::Branch(join));
        self.set_block(slow_block);
        let s = self.call_by_name(list, super::sdk_source::AccessKind::Read, "length", &[]);
        let s = self.coerce(s, Type::I64);
        let slow_end = self.current_block;
        if self.is_terminated() {
            self.set_block(join);
            return n;
        }
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(
            Instruction::Phi {
                incoming: vec![(fast_block, n), (slow_end, s)],
                ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// Branches to the fast path when `index` is within the bounds of the
    /// suitable list, otherwise to `slow`; joins both results in
    /// representation `repr`.
    #[allow(clippy::too_many_arguments)]
    fn branch_indexed(
        &mut self,
        list: &Operand,
        index: &Operand,
        ix: Indexable,
        write: bool,
        repr: Type,
        fast: &mut dyn FnMut(&mut Self) -> Operand,
        slow: &mut dyn FnMut(&mut Self) -> Operand,
    ) -> Operand {
        self.typed_data = None;
        let n = self.fast_length(list, ix, write);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, index.clone(), n), Type::I1);
        let fast_block = self.new_block();
        let slow_block = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: fast_block,
            else_block: slow_block,
        });

        self.set_block(fast_block);
        let r = fast(self);
        self.typed_data = None;
        let r = self.coerce(r, repr);
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));

        self.set_block(slow_block);
        let s = slow(self);
        let s = self.coerce(s, repr);
        let slow_end = self.current_block;
        let slow_arrives = !self.is_terminated();
        if slow_arrives {
            self.terminate(Terminator::Branch(join));
        }

        self.set_block(join);
        if slow_arrives {
            self.emit(
                Instruction::Phi {
                    incoming: vec![(fast_end, r), (slow_end, s)],
                    ty: repr,
                },
                repr,
            )
        } else {
            r
        }
    }

    /// The address of the elements of a suitable typed list: what
    /// `fast_length` read for the bounds test that guards this
    /// access, or `@df.typed_data`.
    fn typed_addresses(&mut self, list: &Operand) -> Operand {
        if let Some((l, d)) = &self.typed_data
            && l == list
        {
            return d.clone();
        }
        self.typed_helper("df.typed_data", vec![(list.clone(), Type::Ref)], Type::I64)
    }

    /// The address of the elements and the length of a typed list of type
    /// `ty` (internal, external or a view: the same offsets, §2.5), inline
    /// and invariant (`@df.typed_data`/`@df.typed_len`); `write`: 0
    /// for the unmodifiable view of the type (`@df.typed_len_writable`, by the cid).
    /// Before, a fixed-address header per pure runtime call and a
    /// cache per access point.
    fn typed_data_and_length(
        &mut self,
        list: &Operand,
        typ: i64,
        write: bool,
    ) -> (Operand, Operand) {
        let data = self.typed_helper("df.typed_data", vec![(list.clone(), Type::Ref)], Type::I64);
        let n = if write {
            let immutable = i64::from(dartforge_runtime::layout::cid::view(typ as u8, true));
            self.typed_helper(
                "df.typed_len_writable",
                vec![
                    (list.clone(), Type::Ref),
                    (Operand::Constant(Constant::Int(immutable)), Type::I64),
                ],
                Type::I64,
            )
        } else {
            self.typed_helper("df.typed_len", vec![(list.clone(), Type::Ref)], Type::I64)
        };
        (data, n)
    }

    /// `list[index]`, in result `repr`. `None` when the index is not
    /// an unboxed `int` (the dispatch stays).
    pub(super) fn read_indexed(
        &mut self,
        list: Operand,
        index: Operand,
        ix: Indexable,
        repr: Type,
    ) -> Option<Operand> {
        if self.operand_type(&index) != Type::I64 {
            return None;
        }
        // `list[i]` in the fast iterations of a versioned loop: `i` is
        // proven to be in `0..length` — the load, without a bounds test nor a
        // slow path (which would call the runtime and prevent LLVM from
        // vectorizing the loop).
        if let Some(p) = self.access_proven.take() {
            match ix {
                Indexable::Typed(l) => {
                    let v = self.emit(
                        Instruction::NativeLoad {
                            address: p.data,
                            index,
                            ty: l.element,
                        },
                        l.repr(),
                    );
                    return Some(self.coerce(v, repr));
                }
                Indexable::Simd { k, .. } => {
                    let v = self.emit(
                        Instruction::Simd {
                            op: OpSimd::Loads,
                            args: vec![p.data, index],
                        },
                        k,
                    );
                    return Some(self.coerce(v, repr));
                }
                Indexable::Core { .. } => {}
            }
        }
        let list = self.coerce(list, Type::Ref);
        let (list2, index2) = (list.clone(), index.clone());
        let mut fast = |s: &mut Self| match ix {
            Indexable::Typed(l) => {
                let address = s.typed_addresses(&list2);
                s.emit(
                    Instruction::NativeLoad {
                        address,
                        index: index2.clone(),
                        ty: l.element,
                    },
                    l.repr(),
                )
            }
            Indexable::Core { .. } => s.read_list_element(&list2, &index2, repr),
            Indexable::Simd { k, .. } => {
                let address = s.typed_addresses(&list2);
                s.emit(
                    Instruction::Simd {
                        op: OpSimd::Loads,
                        args: vec![address, index2.clone()],
                    },
                    k,
                )
            }
        };
        let mut slow = |s: &mut Self| {
            // `String.codeUnits` (the header and body bytes of `dart:_http`):
            // the code unit read inline, before the dispatch.
            if repr == Type::I64
                && matches!(ix, Indexable::Core { .. })
                && let Some(layout) = s.code_units_layout()
            {
                return s.read_code_units_or(layout, &list2, &index2, |s| {
                    s.call_by_name_with_typed_receiver(
                        list2.clone(),
                        true,
                        super::sdk_source::AccessKind::Call,
                        "[]",
                        &[(None, index2.clone())],
                    )
                });
            }
            // The receiver has a static type (the indexable list): the typed
            // selector (`typed_entry.rs`).
            s.call_by_name_with_typed_receiver(
                list2.clone(),
                true,
                super::sdk_source::AccessKind::Call,
                "[]",
                &[(None, index2.clone())],
            )
        };
        Some(self.branch_indexed(&list, &index, ix, false, repr, &mut fast, &mut slow))
    }

    /// The class id of `CodeUnits` (`dart:_internal`) and the position of its
    /// `_string` field, when the program has the class with a fixed layout.
    fn code_units_layout(&self) -> Option<(u32, usize)> {
        if self.ctx.arc_memory {
            return None;
        }
        *self.ctx.code_units.get_or_init(|| {
            let (k, class) = self.ctx.program.classes.iter().enumerate().find(|(_, c)| {
                self.ctx.symbol_name(c.name) == "CodeUnits"
                    && self.ctx.program.library(c.library).uri == "dart:_internal"
            })?;
            let id = self
                .ctx
                .class_id_of(dartforge_elements::model::ClassId(k as u32))?;
            let field = class.fields.iter().copied().find(|v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                !var.static_ && self.ctx.symbol_name(var.name) == "_string"
            })?;
            Some((id, self.field_index(field)?))
        })
    }

    /// `list[index]` of an `int` list that may be a `CodeUnits`: when it is
    /// one and the index is within its string, the string's code unit;
    /// otherwise `other` (the dispatch), whose result comes as `I64`.
    fn read_code_units_or(
        &mut self,
        (class_id, field): (u32, usize),
        list: &Operand,
        index: &Operand,
        other: impl FnOnce(&mut Self) -> Operand,
    ) -> Operand {
        let class = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(list.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let is = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                class,
                Operand::Constant(Constant::Int(i64::from(class_id))),
            ),
            Type::I1,
        );
        let units = self.new_block();
        let read = self.new_block();
        let dispatch = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: is,
            then_block: units,
            else_block: dispatch,
        });
        self.set_block(units);
        let string = self.emit(
            Instruction::GetField {
                object: list.clone(),
                index: field,
            },
            Type::Ref,
        );
        let length = self.emit(
            Instruction::CallRuntime {
                name: "df.string_len".to_string(),
                args: vec![(string.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let inside = self.emit(
            Instruction::ICmp(ICmpOp::Ult, index.clone(), length),
            Type::I1,
        );
        self.terminate(Terminator::CondBranch {
            cond: inside,
            then_block: read,
            else_block: dispatch,
        });
        self.set_block(read);
        let unit = self.emit(
            Instruction::CallRuntime {
                name: "df.string_code_unit".to_string(),
                args: vec![(string, Type::Ref), (index.clone(), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.terminate(Terminator::Branch(join));
        self.set_block(dispatch);
        let r = other(self);
        let r = self.coerce(r, Type::I64);
        let dispatch_end = self.current_block;
        let arrives = !self.is_terminated();
        if arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if !arrives {
            return unit;
        }
        self.emit(
            Instruction::Phi {
                incoming: vec![(read, unit), (dispatch_end, r)],
                ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// `list[index] = value`. `false` when there is no fast path (the
    /// caller does the dispatch).
    pub(super) fn store_indexed(
        &mut self,
        list: Operand,
        index: Operand,
        value: Operand,
        ix: Indexable,
    ) -> bool {
        let direct = match ix {
            Indexable::Typed(l) => l.direct_write(),
            Indexable::Core { write_op } => write_op.is_some(),
            Indexable::Simd { .. } => true,
        };
        let proven = self.access_proven.take();
        if !direct || self.operand_type(&index) != Type::I64 {
            return false;
        }
        // `list[i] = v` in the fast iterations of a versioned loop, with the
        // list also proven for the write: the write, without a bounds
        // test nor a slow path. `Int32List` etc. keep the low bits
        // (the `trunc` of `NativeWrite`), like the VM's `[]=`.
        if let Some(p) = proven.filter(|p| p.write) {
            match ix {
                Indexable::Typed(l) => {
                    let v = self.coerce(value, l.repr());
                    self.emit(
                        Instruction::NativeWrite {
                            address: p.data,
                            index,
                            ty: l.element,
                            value: v,
                        },
                        Type::Void,
                    );
                    return true;
                }
                Indexable::Simd { k, .. } => {
                    let v = self.coerce(value, k);
                    self.emit(
                        Instruction::Simd {
                            op: OpSimd::Writes,
                            args: vec![p.data, index, v],
                        },
                        Type::Void,
                    );
                    return true;
                }
                Indexable::Core { .. } => {}
            }
        }
        let list = self.coerce(list, Type::Ref);
        let (list2, index2, value2) = (list.clone(), index.clone(), value.clone());
        let mut fast = |s: &mut Self| {
            match ix {
                Indexable::Typed(l) => {
                    let address = s.typed_addresses(&list2);
                    let v = s.coerce(value2.clone(), l.repr());
                    s.emit(
                        Instruction::NativeWrite {
                            address,
                            index: index2.clone(),
                            ty: l.element,
                            value: v,
                        },
                        Type::Void,
                    );
                }
                Indexable::Core { write_op: Some(t) } => {
                    let v = s.coerce(value2.clone(), t);
                    s.store_list_element(&list2, &index2, v, t);
                }
                Indexable::Core { write_op: None } => unreachable!("direct store checked above"),
                Indexable::Simd { k, .. } => {
                    let address = s.typed_addresses(&list2);
                    let v = s.coerce(value2.clone(), k);
                    s.emit(
                        Instruction::Simd {
                            op: OpSimd::Writes,
                            args: vec![address, index2.clone(), v],
                        },
                        Type::Void,
                    );
                }
            }
            Operand::Constant(Constant::Int(0))
        };
        let mut slow = |s: &mut Self| {
            // An unboxed SIMD vector is only boxed here, on the slow path.
            let value = if s.operand_type(&value).is_vector() {
                s.coerce(value.clone(), Type::Ref)
            } else {
                value.clone()
            };
            s.call_by_name_with_typed_receiver(
                list2.clone(),
                true,
                super::sdk_source::AccessKind::Call,
                "[]=",
                &[(None, index2.clone()), (None, value)],
            );
            Operand::Constant(Constant::Int(0))
        };
        self.branch_indexed(&list, &index, ix, true, Type::I64, &mut fast, &mut slow);
        true
    }

    /// The storage of a runtime list (the `_List` or
    /// `_ImmutableList` itself, or the data of the `_GrowableList`): the address of
    /// element 0 (`a+22`) and its shape (`flags & (FORMAT | ELEMENT)`, the
    /// [`shape_code`]). The caller knows that `list` is a runtime list.
    ///
    /// Under ARC, the address is derived from the list: the preparation holds the list
    /// after each access, which stays in the block of the read, without a call between
    /// the two (`optimize/arc/derived_set.rs`; growth replaces the storage).
    pub(super) fn list_storage(&mut self, list: &Operand) -> (Operand, Operand) {
        let a = self.typed_helper(
            "df.core_storage",
            vec![(list.clone(), Type::Ref)],
            Type::I64,
        );
        let shape = self.typed_helper("df.core_shape", vec![(a.clone(), Type::I64)], Type::I64);
        let data = self.emit(
            Instruction::Add(a, Operand::Constant(Constant::Int(STORAGE_ELEMENTS))),
            Type::I64,
        );
        (data, shape)
    }

    /// The logical length of `list` if it is a runtime list, otherwise 0
    /// (`@df.core_len`).
    pub(super) fn core_length(&mut self, list: &Operand) -> Operand {
        self.typed_helper("df.core_len", vec![(list.clone(), Type::Ref)], Type::I64)
    }

    /// The length of `list` for direct writes in shape `shape`
    /// (`@df.core_len_writable`): 0 if it is not a `_List`/`_GrowableList`
    /// of that shape.
    pub(super) fn core_writable_length(&mut self, list: &Operand, shape: i64) -> Operand {
        self.typed_helper(
            "df.core_len_writable",
            vec![
                (list.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(shape)), Type::I64),
            ],
            Type::I64,
        )
    }

    /// The address of element 0 and the shape: those checked before the
    /// N13 iterations (shape known at emission), or read inline.
    ///
    /// With the ARC preparation, the storage is reread at each access: the
    /// slow path of `[]` (inside the iterations) is a call, and a
    /// storage read before them would have nobody holding it
    /// (`optimize/arc/derived_set.rs`). The shape checked before still holds.
    fn elements_and_shape(&mut self, list: &Operand) -> (Operand, Result<i64, Operand>) {
        if let Some(f) = &self.access_fixed {
            let shape = f.shape;
            if self.ctx.arc_memory && crate::target::arc_prepared().unwrap_or(false) {
                let (data, _) = self.list_storage(list);
                return (data, Ok(shape));
            }
            return (f.data.clone(), Ok(shape));
        }
        let (data, shape) = self.list_storage(list);
        (data, Err(shape))
    }

    /// Element `index` of a storage of shape `k`, in its natural
    /// representation: the `Ref` (`@df.word_ref`, which the
    /// emitter roots like any `Ref`), or the bits of the scalar. Under ARC,
    /// the general list goes through the checked read that produces an owner.
    fn load_in_shape(
        &mut self,
        list: &Operand,
        data: &Operand,
        index: &Operand,
        k: i64,
    ) -> Operand {
        match k {
            SHAPE_INT => self.emit(
                Instruction::NativeLoad {
                    address: data.clone(),
                    index: index.clone(),
                    ty: CType::I64,
                },
                Type::I64,
            ),
            SHAPE_DOUBLE => self.emit(
                Instruction::NativeLoad {
                    address: data.clone(),
                    index: index.clone(),
                    ty: CType::F64,
                },
                Type::F64,
            ),
            SHAPE_BOOL => {
                let b = self.emit(
                    Instruction::NativeLoad {
                        address: data.clone(),
                        index: index.clone(),
                        ty: CType::I64,
                    },
                    Type::I64,
                );
                self.emit(
                    Instruction::ICmp(ICmpOp::Ne, b, Operand::Constant(Constant::Int(0))),
                    Type::I1,
                )
            }
            _ if self.ctx.arc_memory => self.emit(
                Instruction::CallRuntime {
                    name: arc_list_ref_read().into(),
                    args: vec![(list.clone(), Type::Ref), (index.clone(), Type::I64)],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ),
            _ => self.typed_helper(
                "df.word_ref",
                vec![(data.clone(), Type::I64), (index.clone(), Type::I64)],
                Type::Ref,
            ),
        }
    }

    /// Element `index` through the SDK `[]` (the shape that does not serve the
    /// requested representation), in `repr`.
    fn element_by_sdk(&mut self, list: &Operand, index: &Operand, repr: Type) -> Operand {
        let r = self.call_by_name_with_typed_receiver(
            list.clone(),
            true,
            super::sdk_source::AccessKind::Call,
            "[]",
            &[(None, index.clone())],
        );
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        self.coerce(r, repr)
    }

    /// Element `index` (already checked) of a runtime list, in
    /// representation `repr`.
    ///
    /// In the compact shape of `repr` (`int`, `double`, `bool`), the 8 bytes of the
    /// bits at `data + 8·index`; in the general list, the `Ref` (unboxed if
    /// `repr` is scalar). Read as a `Ref`, a compact element comes out
    /// boxed. What is left (a compact shape that is not that of the scalar
    /// `repr`) goes to the SDK `[]`. The shape known at emission (the N13
    /// iterations) spares the check.
    pub(super) fn read_list_element(
        &mut self,
        list: &Operand,
        index: &Operand,
        repr: Type,
    ) -> Operand {
        let scalar = matches!(repr, Type::I64 | Type::F64 | Type::I1);
        let shapes: Vec<i64> = if scalar {
            vec![shape_code(repr), SHAPE_GENERAL]
        } else {
            vec![SHAPE_GENERAL, SHAPE_INT, SHAPE_DOUBLE, SHAPE_BOOL]
        };
        let (data, shape) = self.elements_and_shape(list);
        let f = match shape {
            Ok(k) if shapes.contains(&k) => {
                let v = self.load_in_shape(list, &data, index, k);
                return self.coerce(v, repr);
            }
            Ok(_) => return self.element_by_sdk(list, index, repr),
            Err(f) => f,
        };
        let join = self.new_block();
        let mut entries = Vec::with_capacity(shapes.len() + 1);
        for k in shapes {
            let ok = self.emit(
                Instruction::ICmp(ICmpOp::Eq, f.clone(), Operand::Constant(Constant::Int(k))),
                Type::I1,
            );
            let yes = self.new_block();
            let node = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: ok,
                then_block: yes,
                else_block: node,
            });
            self.set_block(yes);
            let v = self.load_in_shape(list, &data, index, k);
            let v = self.coerce(v, repr);
            if !self.is_terminated() {
                entries.push((self.current_block, v));
                self.terminate(Terminator::Branch(join));
            }
            self.set_block(node);
        }
        let v = self.element_by_sdk(list, index, repr);
        if !self.is_terminated() {
            entries.push((self.current_block, v));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match entries.len() {
            0 => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
            1 => entries.pop().expect("one entry").1,
            _ => self.emit(
                Instruction::Phi {
                    incoming: entries,
                    ty: repr,
                },
                repr,
            ),
        }
    }

    /// Writes `value` (`int`, `double` or `bool`, unboxed) into element
    /// `index` (already checked) of a runtime list whose direct write
    /// was checked (`@df.core_len_writable`): the storage is compact
    /// in the shape of `repr`, and the element is the 8 bytes of the bits (scalar:
    /// without a barrier).
    fn store_list_element(&mut self, list: &Operand, index: &Operand, value: Operand, repr: Type) {
        let (typ, value) = match repr {
            Type::F64 => (CType::F64, value),
            Type::I1 => {
                let b = self.emit(
                    Instruction::ZExt {
                        op: value,
                        from: Type::I1,
                        to: Type::I64,
                    },
                    Type::I64,
                );
                (CType::I64, b)
            }
            _ => (CType::I64, value),
        };
        let (data, _) = self.elements_and_shape(list);
        self.emit(
            Instruction::NativeWrite {
                address: data,
                index: index.clone(),
                ty: typ,
                value,
            },
            Type::Void,
        );
    }
}

/// The ARC read of a `Ref` element of a general list: with the preparation
/// published, an owner that it consumes; without it (legacy ARC, in which
/// nobody releases tokens of the code), the borrow of the list.
pub(super) fn arc_list_ref_read() -> &'static str {
    if crate::target::arc_prepared().unwrap_or(false) {
        "dartforge_arc_list_ref_owned_v1"
    } else {
        "dartforge_arc_list_ref_v1"
    }
}

/// The shape of a runtime list storage: the `FORMAT` and
/// `ELEMENT` bits of the `flags` (`layout::flags`, §2.3 and §2.16).
pub(super) const SHAPE_GENERAL: i64 = dartforge_runtime::layout::flags::REFS as i64;
const SHAPE_INT: i64 =
    (dartforge_runtime::layout::flags::RAW | dartforge_runtime::layout::flags::ELEMENT_INT) as i64;
const SHAPE_DOUBLE: i64 = (dartforge_runtime::layout::flags::RAW
    | dartforge_runtime::layout::flags::ELEMENT_DOUBLE) as i64;
const SHAPE_BOOL: i64 =
    (dartforge_runtime::layout::flags::RAW | dartforge_runtime::layout::flags::ELEMENT_BOOL) as i64;

/// Element 0 of a storage, from the handle (`b+24` = `h+22`).
const STORAGE_ELEMENTS: i64 =
    dartforge_runtime::layout::offsets::ELEMENTS as i64 - dartforge_runtime::layout::HANDLE_OFFSET;

/// The code of the compact shape of the elements of representation `t` (`int`,
/// `double`, `bool`), what `@df.core_shape` reads from the storage; the general
/// shape for the others.
pub(super) fn shape_code(t: Type) -> i64 {
    match t {
        Type::I64 => SHAPE_INT,
        Type::F64 => SHAPE_DOUBLE,
        Type::I1 => SHAPE_BOOL,
        _ => SHAPE_GENERAL,
    }
}

/// The element type and the C type of a numeric typed list class of
/// `typed_data_patch.dart` (`_Int8List`, `_Int8ArrayView`,
/// `_UnmodifiableInt8ArrayView`…); `None` for SIMD and the rest.
fn class_element(class: &str) -> Option<(i64, CType)> {
    let base = class
        .strip_prefix("_Unmodifiable")
        .or_else(|| class.strip_prefix('_'))?;
    let base = base
        .strip_suffix("ArrayView")
        .or_else(|| base.strip_suffix("List"))?;
    element_of_name(base)
}

/// `Int8` … `Float64` → (the `TIPO_*`, the C type).
fn element_of_name(name_str: &str) -> Option<(i64, CType)> {
    Some(match name_str {
        "Int8" => (0, CType::I8),
        "Uint8" => (1, CType::U8),
        "Uint8Clamped" => (2, CType::U8),
        "Int16" => (3, CType::I16),
        "Uint16" => (4, CType::U16),
        "Int32" => (5, CType::I32),
        "Uint32" => (6, CType::U32),
        "Int64" => (7, CType::I64),
        "Uint64" => (8, CType::U64),
        "Float32" => (9, CType::F32),
        "Float64" => (10, CType::F64),
        _ => return None,
    })
}

impl FnBuilder<'_, '_> {
    /// The external member of a typed list or view done inline (P4,
    /// docs/NATIVO-ESPACO-UNIFICADO.md §3.8), or `None` (the usual call):
    ///
    /// * `TypedDataBase_length`, `TypedDataView_offsetInBytes`,
    ///   `TypedDataView_typedData`: the load from the block;
    /// * the recognized `[]` of each numeric list and view
    ///   (`DartForge_typed_index_*`): the bounds test and the load, with the
    ///   runtime as the slow path (which throws the VM's `RangeError`);
    /// * the numeric `_TypedList._getX`/`_setX` (the byte accesses of
    ///   `ByteData` and of the patch's copies): the test `0 <= off`, `off + N <=
    ///   bytes` and the load or write, with the runtime as the slow path.
    ///
    /// The results come out in the natural representation (`int` `I64`, `double`
    /// `F64`), like `inline_string`. SIMD and `_memMove*` stay in the runtime.
    pub(super) fn typed_inline(
        &mut self,
        member: &str,
        native: Option<&str>,
        this: Option<&Operand>,
        args: &[Operand],
    ) -> Option<Operand> {
        let this = this?.clone();
        if self.operand_type(&this) != Type::Ref {
            return None;
        }
        let helper_name = match (native, args.len()) {
            (Some("TypedDataBase_length"), 0) => Some(("df.typed_len", Type::I64)),
            (Some("TypedDataView_offsetInBytes"), 0) => Some(("df.typed_offset", Type::I64)),
            (Some("TypedDataView_typedData"), 0) => Some(("df.typed_base", Type::Ref)),
            (Some(_), _) => return None,
            (None, _) => None,
        };
        if let Some((name_str, ret)) = helper_name {
            return Some(self.typed_helper(name_str, vec![(this, Type::Ref)], ret));
        }
        let slow = crate::natives::intrinsic(member)?;
        if let Some(class) = member.strip_suffix(".[]") {
            let (_, tc) = class_element(class)?;
            if args.len() != 1 {
                return None;
            }
            return Some(self.index_typed_inline(this, args[0].clone(), tc, slow));
        }
        let (writes, name_str) = match member.strip_prefix("_TypedList._get") {
            Some(n) => (false, n),
            None => (true, member.strip_prefix("_TypedList._set")?),
        };
        let (_, tc) = element_of_name(name_str)?;
        if args.len() != usize::from(writes) + 1 {
            return None;
        }
        Some(self.inline_typed_bytes(this, args, tc, writes, slow))
    }

    /// `this[i]` of a numeric list or view (element `tc`): inline
    /// when `0 <= i < length`, otherwise the runtime intrinsic `slow`.
    fn index_typed_inline(&mut self, this: Operand, i: Operand, tc: CType, slow: &str) -> Operand {
        let repr = tc.hir_type();
        let i = self.coerce(i, Type::I64);
        let n = self.typed_helper("df.typed_len", vec![(this.clone(), Type::Ref)], Type::I64);
        let ok = self.emit(Instruction::ICmp(ICmpOp::Ult, i.clone(), n), Type::I1);
        let fast = self.new_block();
        let slowly = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: fast,
            else_block: slowly,
        });
        self.set_block(fast);
        let data = self.typed_helper("df.typed_data", vec![(this.clone(), Type::Ref)], Type::I64);
        let v = self.emit(
            Instruction::NativeLoad {
                address: data,
                index: i.clone(),
                ty: tc,
            },
            repr,
        );
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(slowly);
        let s = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(slow),
                args: vec![(this, Type::Ref), (i, Type::I64)],
                ret_ty: repr,
            },
            repr,
        );
        let slow_end = self.current_block;
        let arrives = !self.is_terminated();
        if arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if arrives {
            self.emit(
                Instruction::Phi {
                    incoming: vec![(fast_end, v), (slow_end, s)],
                    ty: repr,
                },
                repr,
            )
        } else {
            v
        }
    }

    /// `this._getX(off)` / `this._setX(off, v)` (element `tc` of `N`
    /// bytes at `off`, unaligned): inline when `off u< bytes` and
    /// `off + N <= bytes`, otherwise the runtime intrinsic `slow` (which throws).
    fn inline_typed_bytes(
        &mut self,
        this: Operand,
        args: &[Operand],
        tc: CType,
        writes: bool,
        slow: &str,
    ) -> Operand {
        let int = |x: i64| Operand::Constant(Constant::Int(x));
        let repr = tc.hir_type();
        let off = self.coerce(args[0].clone(), Type::I64);
        let value = if writes {
            Some(self.coerce(args[1].clone(), repr))
        } else {
            None
        };
        let bytes = self.typed_helper("df.typed_bytes", vec![(this.clone(), Type::Ref)], Type::I64);
        let inside = self.emit(
            Instruction::ICmp(ICmpOp::Ult, off.clone(), bytes.clone()),
            Type::I1,
        );
        let block_fits = self.new_block();
        let fast = self.new_block();
        let slowly = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: inside,
            then_block: block_fits,
            else_block: slowly,
        });
        self.set_block(block_fits);
        let end = self.emit(
            Instruction::Add(off.clone(), int(tc.c_size() as i64)),
            Type::I64,
        );
        let fits = self.emit(Instruction::ICmp(ICmpOp::Sle, end, bytes), Type::I1);
        self.terminate(Terminator::CondBranch {
            cond: fits,
            then_block: fast,
            else_block: slowly,
        });

        self.set_block(fast);
        let data = self.typed_helper("df.typed_data", vec![(this.clone(), Type::Ref)], Type::I64);
        let address = self.emit(Instruction::Add(data, off.clone()), Type::I64);
        let v = match &value {
            Some(x) => {
                self.emit(
                    Instruction::NativeWrite {
                        address,
                        index: int(0),
                        ty: tc,
                        value: x.clone(),
                    },
                    Type::Void,
                );
                None
            }
            None => Some(self.emit(
                Instruction::NativeLoad {
                    address,
                    index: int(0),
                    ty: tc,
                },
                repr,
            )),
        };
        let fast_end = self.current_block;
        self.terminate(Terminator::Branch(join));

        self.set_block(slowly);
        let mut a = vec![(this, Type::Ref), (off, Type::I64)];
        if let Some(x) = value {
            a.push((x, repr));
        }
        let ret = if writes { Type::Void } else { repr };
        let s = self.emit_call_with_check(
            Instruction::CallRuntime {
                name: crate::natives::symbol(slow),
                args: a,
                ret_ty: ret,
            },
            ret,
        );
        let slow_end = self.current_block;
        let arrives = !self.is_terminated();
        if arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match v {
            None => Operand::Constant(Constant::Null),
            Some(v) if arrives => self.emit(
                Instruction::Phi {
                    incoming: vec![(fast_end, v), (slow_end, s)],
                    ty: repr,
                },
                repr,
            ),
            Some(v) => v,
        }
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    use crate::context::Context;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn source_read_produces_owner_only_with_arc() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "Object ler(List<Object> valores, int i) => valores[i]; void main() { ler([1], 0); }\n").unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, diags) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diags.is_empty(), "{diags:?}");
            let mut table = TypeTable::new();
            let core = CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let (bodies, body_diags) = dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
            for d in diags.iter().chain(&body_diags) {
                assert!(!dartforge_types::codes::is_compile_error(&d.message), "{d:?}");
            }
            let te = crate::erasure::compute(&program, &outline, &mut table);
            let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
            ctx.te = te;
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                let mut owned = 0; let mut raw_items = 0;
                for (_, inst, ty) in m.functions.iter().filter(|f| f.name == "ler").flat_map(|f| &f.blocks).flat_map(|b| &b.instructions) {
                    if let Instruction::CallRuntime { name, args, ret_ty } = inst {
                        if name == "dartforge_arc_list_ref_owned_v1" || name == "dartforge_arc_list_ref_v1" {
                            owned += 1;
                            assert_eq!((*ret_ty, *ty), (Type::Ref, Type::Ref));
                            assert_eq!(args.iter().map(|a| a.1).collect::<Vec<_>>(), vec![Type::Ref, Type::I64]);
                        }
                        if name == "df.word_ref" { raw_items += 1; }
                    }
                }
                if arc { assert!(owned > 0); assert_eq!(raw_items, 0); }
                else { assert_eq!(owned, 0); assert!(raw_items > 0); }
            }
        }).unwrap().join().unwrap();
    }
}
