//! The SDK compiled from source in the lowering (P5c/P5d, δ; docs/NATIVO-PLANO.md §7).
//!
//! The seven libraries of
//! `sdk_modulo::SOURCE_LIBRARIES` are Dart code compiled like that of the
//! program, each one in its own module (an object in the cache), and the program calls them
//! through the stable symbols. Three things change relative to the closed world:
//!
//! 1. **Call by selector.** A public instance member of an SDK
//!    class may be overridden by a program class, which the SDK does not
//!    know when it is compiled. The call goes through the selector (`c:m`, `g:x`,
//!    `s:x`; the private name carries `@<library>`), which the method table of the
//!    dynamic class resolves (`llvm/selectors.rs`, `runtime/selectors.rs`).
//!    A call that only the library of the class can override stays direct (or through the
//!    closed-world `switch` of `members.rs`): a private
//!    member, or a private class whose subtypes in the library are all
//!    private.
//! 2. **Adapters.** Each instance member gets the uniform entries
//!    that the method table points to: `<symbol>$c` (call), `$g` (read: the
//!    getter, the field, or the tear-off of a method) and `$s` (write), with the
//!    closure convention (`i64 (i64 receiver, ptr args, ptr desc)`).
//! 3. **`external`.** The patch member (`patched_by`) is the body; a native
//!    (`vm:external-name`) is the runtime function of the `natives.rs` table; whatever
//!    has no implementation (a pending native, a VM intrinsic) is a
//!    diagnostic — the member that calls it is **rejected** in the SDK module
//!    (`sdk_modulo`), with the reason, and never emitted wrong.

use super::fn_builder::FnBuilder;
use super::members::{Evaluated, linearization, subclass_of};
use crate::context::Context;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, FunctionKind, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::ParameterKind;

/// What the selector does with the member.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AccessKind {
    /// `o.m(…)`.
    Call,
    /// `o.x`.
    Read,
    /// `o.x = v`.
    Write,
}

/// The text of the selector: `c:m`, `g:x`, `s:x`; a private name carries
/// `@<library>` (two `_m`s of different libraries are different
/// members). The `_=` of the key of a setter in the element tables is dropped
/// (`==` and `[]=` are operators, not setters).
pub fn selector_string(ctx: &Context, ty: AccessKind, name_str: &str, lib: LibraryId) -> String {
    let p = match ty {
        AccessKind::Call => "c",
        AccessKind::Read => "g",
        AccessKind::Write => "s",
    };
    let name_str = name_str.strip_suffix("_=").unwrap_or(name_str);
    if name_str.starts_with('_') {
        format!("{p}:{name_str}@{}", ctx.library_name(lib))
    } else {
        format!("{p}:{name_str}")
    }
}

/// No class outside the library of `cid` can be a subtype of it: it and
/// all its subtypes in the library are private.
pub fn closed_class(ctx: &Context, cid: ClassId) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !ctx.symbol_name(class.name).starts_with('_') {
        return false;
    }
    ctx.subtypes(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library != class.library || ctx.symbol_name(c.name).starts_with('_')
    })
}

/// The member `name` of `cid` can only be overridden inside its
/// library (the closed world of `members.rs` holds for it).
pub fn closed_member(ctx: &Context, cid: ClassId, name_str: &str) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !ctx.program.library(class.library).is_sdk {
        // Program class: the program knows all its subtypes.
        return true;
    }
    name_str.starts_with('_') || closed_class(ctx, cid) || class_closed_by_modifier(ctx, cid)
}

/// The Dart 3 class modifiers close `cid` in its library:
/// it is `final` or `sealed`, and every subtype of it in the program is from the same
/// library and `final`, `sealed` or private — no outside code can
/// extend it, implement it or override a member of it. That is the case of
/// `String`, `int`, `double`, `num` and `bool`: without this, `s.codeUnitAt(i)`
/// with `s` statically a `String` went through the selector and the uniform entry, which
/// checks the arguments in the RTI (writing the headers of `dart:_http`
/// made hundreds of those calls per request).
pub fn class_closed_by_modifier(ctx: &Context, cid: ClassId) -> bool {
    let class = &ctx.program.classes[cid.0 as usize];
    if !(class.modifiers.final_ || class.modifiers.sealed) {
        return false;
    }
    ctx.subtypes(cid).iter().all(|&k| {
        let c = &ctx.program.classes[k.0 as usize];
        c.library == class.library
            && (c.modifiers.final_ || c.modifiers.sealed || ctx.symbol_name(c.name).starts_with('_'))
    })
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `recv.<selector>(evaluated)` through the uniform convention; the result is
    /// `Ref`.
    pub fn call_by_selector(&mut self, recv: Operand, selector: String, evaluated: &[Evaluated]) -> Operand {
        self.call_by_selector_with_tuple(recv, selector, evaluated, Operand::Constant(Constant::Int(0)))
    }

    pub(super) fn call_by_selector_with_tuple(
        &mut self,
        recv: Operand,
        selector: String,
        evaluated: &[Evaluated],
        tuple_types: Operand,
    ) -> Operand {
        let recv = self.coerce(recv, Type::Ref);
        let mut args = Vec::with_capacity(evaluated.len());
        for (n, v) in evaluated {
            if n.is_none() {
                let v = self.coerce(v.clone(), Type::Ref);
                args.push(v);
            }
        }
        let mut named: Vec<(String, Operand)> = evaluated
            .iter()
            .filter_map(|(n, v)| n.map(|n| (self.ctx.symbol_name(n).to_string(), v.clone())))
            .collect();
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut names = Vec::with_capacity(named.len());
        for (n, v) in named {
            let v = self.coerce(v, Type::Ref);
            args.push(v);
            names.push(n);
        }
        self.emit_call_with_check(Instruction::CallSelector { selector, recv, args, names, tuple_types }, Type::Ref)
    }

    /// `recv.name(evaluated)` through the selector, with the private name of the library
    /// of the current unit.
    pub fn call_by_name(&mut self, recv: Operand, ty: AccessKind, name_str: &str, evaluated: &[Evaluated]) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = selector_string(self.ctx, ty, name_str, lib);
        self.call_by_selector(recv, s, evaluated)
    }

    /// [`Self::call_by_name`] with the receiver `alvo` of a known static
    /// type: if all the arguments have a guaranteed type, through the typed
    /// selector (`typed_entry.rs`), which only checks the covariant ones.
    pub fn call_by_name_typed(
        &mut self,
        recv: Operand,
        alvo: dartforge_frontend::ast::ExprId,
        ty: AccessKind,
        name_str: &str,
        evaluated: &[Evaluated],
    ) -> Operand {
        let typed = self.ctx.get_type(self.unit_id, alvo).is_some_and(|t| t != self.ctx.core.dynamic_);
        self.call_by_name_with_typed_receiver(recv, typed, ty, name_str, evaluated)
    }

    /// [`Self::call_by_name_typed`] with the decision about the receiver already
    /// taken (`typed_receiver`: its static type is known).
    pub fn call_by_name_with_typed_receiver(
        &mut self,
        recv: Operand,
        typed_receiver: bool,
        ty: AccessKind,
        name_str: &str,
        evaluated: &[Evaluated],
    ) -> Operand {
        let lib = self.ctx.program.unit(self.unit_id).library;
        let s = selector_string(self.ctx, ty, name_str, lib);
        let s = if typed_receiver && matches!(ty, AccessKind::Call | AccessKind::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        self.call_by_selector(recv, s, evaluated)
    }

    /// Hook of `call_member` (source SDK): the instance member of an
    /// SDK class goes through the selector, unless only its library
    /// can override it and it has **one** implementation, which is a method
    /// (a direct call). An interface getter of the program also uses the
    /// selector when some concrete implementation is a field: the dispatch
    /// through functions of `members.rs` cannot represent that implicit getter.
    pub fn call_source_member(&mut self, recv: Operand, decl_fid: usize, evaluated: &[Evaluated]) -> Option<Operand> {
        if self.in_adapter {
            return None;
        }
        let f = &self.ctx.program.functions[decl_fid];
        let cid = f.class?;
        if f.static_ {
            return None;
        }
        let name_str = self.ctx.symbol_name(f.name).to_string();
        let key = if f.kind == FunctionKind::Setter { format!("{name_str}_=") } else { name_str.clone() };
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !is_sdk && (!matches!(f.kind, FunctionKind::Getter | FunctionKind::ImplicitAccessor)
            || !implementations(self.ctx, cid, &key).iter().any(|i| matches!(i, Implementation::Field(_))))
        {
            return None;
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && let [Implementation::Function(alvo)] = implementations(self.ctx, cid, &key)[..]
        {
            // The `this` of an SDK member is always `Ref`: a scalar `int`
            // becomes a `Smi` or a `_Mint` here (the `int` closed by a modifier
            // arrives with the receiver in `i64`; without the box, an even value or
            // one outside the `Smi` range would be read as a handle).
            let recv = self.coerce(recv, Type::Ref);
            let args = self.match_args(alvo, evaluated);
            let r = self.call_direct(alvo, Some(recv), args);
            let ret = self.return_repr(decl_fid);
            return Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) });
        }
        let ty = match f.kind {
            FunctionKind::Getter => AccessKind::Read,
            FunctionKind::Setter => AccessKind::Write,
            FunctionKind::ImplicitAccessor if evaluated.len() == 1 => AccessKind::Write,
            FunctionKind::ImplicitAccessor => AccessKind::Read,
            _ => AccessKind::Call,
        };
        if is_sdk && closed_member(self.ctx, cid, &name_str) {
            let ret = match self.return_repr(decl_fid) {
                Type::Void => Type::Ref,
                t => t,
            };
            if let Some(r) = self.class_dispatch(&recv, cid, &key, ty, f.library, evaluated, ret) {
                let void = matches!(self.return_repr(decl_fid), Type::Void);
                return Some(if void { Operand::Constant(Constant::Null) } else { r });
            }
        }
        let s = selector_string(self.ctx, ty, &name_str, f.library);
        // Typed receiver and guaranteed arguments: the entry that only checks
        // the covariant ones (`typed_entry.rs`).
        let s = if matches!(ty, AccessKind::Call | AccessKind::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        let tuple = if self.generic_function(decl_fid) {
            self.armed_tuple.clone().unwrap_or(Operand::Constant(Constant::Int(0)))
        } else {
            Operand::Constant(Constant::Int(0))
        };
        let r = self.call_by_selector_with_tuple(recv, s, evaluated, tuple);
        let ret = self.return_repr(decl_fid);
        Some(if matches!(ret, Type::Void) { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) })
    }

    /// Few-target dispatch (P2) of a closed member (only its library
    /// implements it, and the whole program of that library is compiled) with 2 to
    /// [`FEW_TARGETS`] implementations: a `switch` on the class of the receiver
    /// chooses the direct call or the field access of each one — the same
    /// as the direct call of the single-implementation case, repeated per
    /// class —, and the selector stays in the `default` (a class that the module does not
    /// know). Without this, each access went through the selector and the uniform
    /// entry, which checks the arguments (the `_index`, `_data`,
    /// `_usedData` of maps and sets: two hierarchies in
    /// `compact_hash.dart`). `None`: no dispatch by class (the caller
    /// goes on through the selector).
    #[allow(clippy::too_many_arguments)]
    fn class_dispatch(
        &mut self,
        recv: &Operand,
        cid: ClassId,
        key: &str,
        ty: AccessKind,
        library: dartforge_elements::model::LibraryId,
        evaluated: &[Evaluated],
        ret: Type,
    ) -> Option<Operand> {
        let by_class = implementations_by_class(self.ctx, cid, key)?;
        let mut distinct: Vec<Implementation> = Vec::new();
        for (_, i) in &by_class {
            if !distinct.contains(i) {
                distinct.push(*i);
            }
        }
        if distinct.len() < 2 || distinct.len() > FEW_TARGETS {
            return None;
        }
        // Calling a field (a stored closure) stays with the selector.
        if ty == AccessKind::Call && distinct.iter().any(|i| matches!(i, Implementation::Field(_))) {
            return None;
        }
        let recv = self.coerce(recv.clone(), Type::Ref);
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(recv.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let blocks: Vec<BlockId> = distinct.iter().map(|_| self.new_block()).collect();
        let pattern = self.new_block();
        let join = self.new_block();
        let cases = by_class
            .iter()
            .map(|(id, i)| (*id, blocks[distinct.iter().position(|d| d == i).expect("listed implementation")]))
            .collect();
        self.terminate(Terminator::Switch { val: cls, default: pattern, cases });
        let span = dartforge_diagnostics::Span { start: 0, end: 0 };
        let mut entries = Vec::new();
        for (i, &b) in distinct.iter().zip(&blocks) {
            self.set_block(b);
            let r = match (*i, ty) {
                (Implementation::Field(vid), AccessKind::Write) => {
                    let v = evaluated.first().map(|(_, v)| v.clone()).unwrap_or(Operand::Constant(Constant::Null));
                    let v = self.coerce(v, self.field_repr(vid));
                    self.store_field(recv.clone(), vid, v, span);
                    Operand::Constant(Constant::Null)
                }
                (Implementation::Field(vid), _) => self.read_field_with_late_direct(recv.clone(), vid, span),
                (Implementation::Function(alvo), _) => {
                    let args = self.match_args(alvo, evaluated);
                    let r = self.call_direct(alvo, Some(recv.clone()), args);
                    if matches!(self.return_repr(alvo), Type::Void) { Operand::Constant(Constant::Null) } else { r }
                }
            };
            if self.is_terminated() {
                continue;
            }
            let r = self.coerce(r, ret);
            entries.push((self.current_block, r));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(pattern);
        let name_str = key.strip_suffix("_=").unwrap_or(key);
        let s = selector_string(self.ctx, ty, name_str, library);
        let s = if matches!(ty, AccessKind::Call | AccessKind::Write) && self.all_checked(evaluated) {
            super::typed_entry::typed_selector(&s)
        } else {
            s
        };
        let r = self.call_by_selector(recv, s, evaluated);
        if !self.is_terminated() {
            let r = self.coerce(r, ret);
            entries.push((self.current_block, r));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        if entries.is_empty() {
            // Every path throws: the join is unreachable.
            return Some(Self::zero_value(ret));
        }
        Some(self.emit(Instruction::Phi { incoming: entries, ty: ret }, ret))
    }

    /// Hook of the field read: a subclass getter may override
    /// even the implicit getter of a field of the base class.
    pub fn read_source_field(&mut self, obj: Operand, vid: VariableId) -> Option<Operand> {
        if self.in_adapter {
            return None;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let cid = v.class?;
        let name_str = self.ctx.symbol_name(v.name).to_string();
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        if !is_sdk {
            // The direct read of the field only holds if every concrete class of the
            // type has THIS field: a getter that overrides it, or another
            // field with the same name in a class that only implements the interface
            // (`class ScalarToken implements Token { final FileSpan span; }`
            // of `package:yaml`, at another position of the object), go through the selector.
            let only_this_field =
                implementations(self.ctx, cid, &name_str).iter().all(|i| *i == Implementation::Field(vid));
            if only_this_field {
                return None;
            }
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && implementations(self.ctx, cid, &name_str).iter().all(|i| *i == Implementation::Field(vid))
        {
            return None;
        }
        let repr = self.field_repr(vid);
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && let Some(r) = self.class_dispatch(&obj, cid, &name_str, AccessKind::Read, v.library, &[], repr)
        {
            return Some(r);
        }
        let s = selector_string(self.ctx, AccessKind::Read, &name_str, v.library);
        let r = self.call_by_selector(obj, s, &[]);
        let repr = self.field_repr(vid);
        Some(self.coerce(r, repr))
    }

    /// Hook of the field write by assignment (source SDK): the field of
    /// an SDK class that a setter may override goes through the selector
    /// `s:x`. `false`: the usual direct write holds.
    pub fn store_source_field(&mut self, obj: Operand, vid: VariableId, value: Operand) -> bool {
        if self.in_adapter {
            return false;
        }
        let v = &self.ctx.program.variables[vid.0 as usize];
        let Some(cid) = v.class else { return false };
        let is_sdk = self.ctx.program.library(self.ctx.program.classes[cid.0 as usize].library).is_sdk;
        let name_str = self.ctx.symbol_name(v.name).to_string();
        // As in the read: direct only if every concrete class writes THIS
        // field (in the SDK, also if the member is closed).
        let only_this_field =
            implementations(self.ctx, cid, &format!("{name_str}_=")).iter().all(|i| *i == Implementation::Field(vid));
        if only_this_field && (!is_sdk || closed_member(self.ctx, cid, &name_str)) {
            return false;
        }
        if is_sdk && closed_member(self.ctx, cid, &name_str)
            && self
                .class_dispatch(&obj, cid, &format!("{name_str}_="), AccessKind::Write, v.library, &[(None, value.clone())], Type::Ref)
                .is_some()
        {
            return true;
        }
        let s = selector_string(self.ctx, AccessKind::Write, &name_str, v.library);
        self.call_by_selector(obj, s, &[(None, value)]);
        true
    }

    /// `a == b` with the source SDK (§17.26 "Equality"): with a null side,
    /// `identical(a, b)`; otherwise `a.==(b)` by the dynamic class of `a`.
    pub fn source_equality(&mut self, a: Operand, b: Operand) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let na = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), zero.clone()), Type::I1);
        let nb = self.emit(Instruction::ICmp(ICmpOp::Eq, b.clone(), zero), Type::I1);
        let any = self.emit(Instruction::Or(na, nb), Type::I1);
        let any = self.emit(Instruction::ICmp(ICmpOp::Ne, any, Operand::Constant(Constant::Int(0))), Type::I1);
        let b_id = self.new_block();
        let b_din = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: any, then_block: b_id, else_block: b_din });
        self.set_block(b_id);
        let r1 = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b.clone()), Type::I1);
        let end1 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(b_din);
        let r = self.call_by_selector(a, "c:==".to_string(), &[(None, b)]);
        let r2 = self.coerce(r, Type::I1);
        let end2 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(Instruction::Phi { incoming: vec![(end1, r1), (end2, r2)], ty: Type::I1 }, Type::I1)
    }

    /// `a == b` (or `a != b`, with `negate`) when the static type of `a` is
    /// a class whose `==` is closed ([`closed_member`]: `String`, `int`,
    /// `double`, `bool`…): the null rule of [`Self::source_equality`] and,
    /// with both sides non-null, the `==` through the dispatch of
    /// [`Self::call_source_member`] — the direct call, or the `switch` on the
    /// class with the selector in the `default` —, without the uniform entry and the
    /// argument check of the `c:==` selector. It is the `char == " "` of the
    /// `dart:_http` parsers. `None`: the general path.
    pub fn closed_class_equality(
        &mut self,
        type_of_a: Option<dartforge_types::table::TypeId>,
        a: &Operand,
        b: &Operand,
        negate: bool,
    ) -> Option<Operand> {
        if self.in_adapter || self.operand_type(a) != Type::Ref {
            return None;
        }
        let dartforge_types::table::Type::Interface { class, .. } = self.ctx.table.get(type_of_a?) else {
            return None;
        };
        let fid = self.member_in_class(*class, "==")?;
        let owner = self.ctx.program.functions[fid].class?;
        if !self.ctx.program.library(self.ctx.program.classes[owner.0 as usize].library).is_sdk
            || !closed_member(self.ctx, owner, "==")
        {
            return None;
        }
        let a = a.clone();
        let b = self.coerce(b.clone(), Type::Ref);
        // `String`: the `==` of the two concrete classes is the same native
        // (`_StringBase._igualA`, which already handles `null` and the other side
        // not being a `String`): one call, without the null rule or the `switch`
        // on the class — the VM's `String_equality` intrinsic.
        if self.ctx.core.string_class == Some(*class) {
            let r = self.emit(
                Instruction::CallRuntime {
                    name: crate::natives::symbol("DartForge_string_igual_a"),
                    args: vec![(a, Type::Ref), (b, Type::Ref)],
                    ret_ty: Type::I8,
                },
                Type::I8,
            );
            let e = self.coerce(r, Type::I1);
            return Some(if negate { self.emit(Instruction::LNot(e), Type::I1) } else { e });
        }
        let zero = Operand::Constant(Constant::Int(0));
        let na = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), zero.clone()), Type::I1);
        let nb = self.emit(Instruction::ICmp(ICmpOp::Eq, b.clone(), zero), Type::I1);
        let any = self.emit(Instruction::Or(na, nb), Type::I1);
        let any = self.emit(Instruction::ICmp(ICmpOp::Ne, any, Operand::Constant(Constant::Int(0))), Type::I1);
        let b_id = self.new_block();
        let b_din = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch { cond: any, then_block: b_id, else_block: b_din });
        self.set_block(b_id);
        let r1 = self.emit(Instruction::ICmp(ICmpOp::Eq, a.clone(), b.clone()), Type::I1);
        let end1 = self.current_block;
        self.terminate(Terminator::Branch(join));
        self.set_block(b_din);
        // The conditions of `call_source_member` already hold (an instance
        // member of an SDK class, outside an adapter); the selector stays only
        // as a safeguard, without leaving half-built blocks.
        let r = match self.call_source_member(a.clone(), fid, &[(None, b.clone())]) {
            Some(r) => r,
            None => self.call_by_selector(a, "c:==".to_string(), &[(None, b)]),
        };
        let r2 = self.coerce(r, Type::I1);
        let mut entries = vec![(end1, r1)];
        if !self.is_terminated() {
            entries.push((self.current_block, r2));
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        let e = self.emit(Instruction::Phi { incoming: entries, ty: Type::I1 }, Type::I1);
        Some(if negate { self.emit(Instruction::LNot(e), Type::I1) } else { e })
    }

    /// `op is T` with the source SDK (hook of `test_type`): every class
    /// (`int`, `num`, `String`, `Object`…) is a compiled class with an id, and
    /// the answer is that of the subtype graph that the libraries register; null
    /// is only a `T` when `T` is `Null` or nullable (§20.3). Type parameters
    /// are resolved through the RTI of the instance or through the tuple of the method.
    pub fn test_source_type(
        &mut self,
        ast_ty: &dartforge_frontend::ast::TypeAnnotation,
        op: Operand,
    ) -> Option<Operand> {
        // In SDK code, the tests with type arguments (`is List<E>`)
        // choose a fast path equivalent for a correct program
        // (`ListBase.setRange`, `List.from`, `ListQueue.addAll`…): until the RTI,
        // they check the class. In the program, only the cast (`as`) checks the
        // class; `is` with arguments stays a diagnostic.
        let in_sdk = self.ctx.program.library(self.ctx.program.unit(self.unit_id).library).is_sdk;
        let only_class = self.cast_by_class_only || in_sdk;
        // Types without a class that have no resolved recipe stay rejected.
        let without_class = |b: &mut Self| -> Option<Operand> {
            if b.cast_by_class_only {
                Some(Operand::Constant(Constant::Bool(true)))
            } else if in_sdk {
                Some(b.unsupported("teste de tipo sem classe no código do SDK (RTI pelo seletor)", ast_ty.span))
            } else {
                None
            }
        };
        let dartforge_frontend::ast::TypeKind::Named { name, args } = &ast_ty.kind else {
            // `Iterable.generate<E>` tests `_id is E Function(int)` before
            // accepting the implicit generator. The RTI represents the signature and
            // replaces `E` with the tuple of the SDK factory.
            if let Some(recipe) = self.annotation_recipe(ast_ty) {
                let ty = self.recipe_rti(&recipe);
                return Some(self.test_rti(op, ty));
            }
            return without_class(self);
        };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let trivial = |t: &dartforge_frontend::ast::TypeAnnotation| match &t.kind {
            dartforge_frontend::ast::TypeKind::Named { name, args } if args.is_empty() => name.last().is_some_and(|n| {
                let s = self.ctx.symbol_name(n.sym);
                s == "dynamic" || (s == "Object" && t.nullable)
            }),
            _ => false,
        };
        let arguments = !args.is_empty() && !args.iter().all(|a| trivial(unit_ast.ty(*a)));
        if arguments && !only_class {
            return None;
        }
        // In the SDK, `is C<…>` with arguments checks the whole RTI (the
        // `value is Future<T>` of `_Future._asyncComplete` decides between
        // chaining and completing with the `Future` itself); `as` goes by the
        // class.
        if arguments
            && in_sdk
            && !self.cast_by_class_only
            && let Some(recipe) = self.annotation_recipe(ast_ty)
        {
            let ty = self.recipe_rti(&recipe);
            return Some(self.test_rti(op, ty));
        }
        let last_one = name.last()?;
        let name_str = self.ctx.symbol_name(last_one.sym).to_string();
        // Class parameters come from the RTI of `this` (`P<i>`). Those of a method
        // come from the hidden tuple passed on by the selector adapter (`M<i>`).
        if in_sdk
            && !self.cast_by_class_only
            && args.is_empty()
            && (self.function_type_params.contains(&last_one.sym)
                || self.local_type_index(last_one.sym).is_some()
                || self.class_params().contains(&last_one.sym))
        {
            let recipe = self.annotation_recipe(ast_ty)?;
            let ty = self.recipe_rti(&recipe);
            return Some(self.test_rti(op, ty));
        }
        let mut op = op;
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            // Scalar: true already at compile time for the obvious supertypes;
            // the rest (`_Smi`, `_IntegerImplementation`, `Pattern`…) by the
            // class of the box — an `int` is a `_Smi` or a `_Mint` depending on the value.
            let correct = matches!(
                (name_str.as_str(), repr),
                ("int", Type::I64)
                    | ("double", Type::F64)
                    | ("bool", Type::I1 | Type::I8)
                    | ("num", Type::I64 | Type::F64)
                    | ("Object" | "dynamic" | "Comparable", _)
            );
            if correct {
                return Some(Operand::Constant(Constant::Bool(true)));
            }
            op = self.coerce(op, Type::Ref);
        }
        if name_str == "dynamic" {
            return Some(Operand::Constant(Constant::Bool(true)));
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let binding = match &name[..] {
            [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
            _ => self.ctx.program.lookup(lib, last_one.sym),
        };
        let cid = binding.and_then(|b| match b.getter {
            Some(dartforge_elements::model::Element::Class(c)) => Some(c),
            _ => None,
        });
        let Some(id) = cid.and_then(|c| self.ctx.class_id_of(c)) else {
            return without_class(self);
        };
        let _ = only_class;
        let zero = Operand::Constant(Constant::Int(0));
        let null = self.emit(Instruction::ICmp(ICmpOp::Eq, op.clone(), zero.clone()), Type::I1);
        if name_str == "Null"
            && cid.is_some_and(|c| self.ctx.program.library(self.ctx.program.classes[c.0 as usize].library).uri == "dart:core")
        {
            return Some(null);
        }
        let cls = self.emit(
            Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(op, Type::Ref)], ret_ty: Type::I64 },
            Type::I64,
        );
        let sub = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_is_subclass".to_string(),
                args: vec![(cls, Type::I64), (Operand::Constant(Constant::Int(i64::from(id))), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let sub = self.emit(Instruction::ICmp(ICmpOp::Ne, sub, zero), Type::I1);
        // null is not a non-nullable `T`, even though `Null` is a subclass of
        // `Object` in the class graph.
        let non_null = self.emit(Instruction::LNot(null.clone()), Type::I1);
        let base = self.emit(Instruction::And(sub, non_null), Type::I64);
        let base = self.emit(Instruction::ICmp(ICmpOp::Ne, base, Operand::Constant(Constant::Int(0))), Type::I1);
        if !ast_ty.nullable {
            return Some(base);
        }
        let r = self.emit(Instruction::Or(base, null), Type::I64);
        Some(self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1))
    }

    /// `toString()` of a value through the selector (interpolation with the source
    /// SDK): the text of null is `"null"` (the `toString` of `Null`).
    pub fn string_by_selector(&mut self, op: Operand) -> Operand {
        let r = self.call_by_selector(op, "c:toString".to_string(), &[]);
        r
    }

    /// Hook of `call_direct`: the call to an `external` of the source SDK.
    /// `None` when the function has a body (the usual direct call).
    pub fn call_external(&mut self, fid: usize, this: Option<Operand>, args: &[Operand]) -> Option<Operand> {
        let f = &self.ctx.program.functions[fid];
        if !f.external || f.variable.is_some() {
            return None;
        }
        if let Some(p) = f.patched_by
            && p.0 as usize != fid
        {
            return Some(self.call_direct(p.0 as usize, this, args.to_vec()));
        }
        let (native, recognized) = crate::natives::pragmas(self.ctx.program, self.ctx.interner, f);
        let span = Span { start: 0, end: 0 };
        let owner = f
            .class
            .map(|c| format!("{}.", self.ctx.symbol_name(self.ctx.program.classes[c.0 as usize].name)))
            .unwrap_or_default();
        let member = format!("{owner}{}", self.ctx.symbol_name(f.name));
        if let Some(r) = self.inline_string(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        // The hooks of each package of the unified space
        // (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, step 4): core lists
        // (P3), typed lists and SIMD (P4), boxes, closures and records (P2).
        if let Some(r) = self.inline_list(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.typed_inline(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        if let Some(r) = self.inline_box(&member, native.as_deref(), this.as_ref(), args) {
            return Some(r);
        }
        let name_str = match native {
            Some(n) => {
                match crate::natives::native(&n).map(|x| x.state) {
                    Some(crate::natives::State::Runtime) => {}
                    Some(crate::natives::State::Builtin) => return Some(self.embedded_native(&n, fid, args)),
                    Some(crate::natives::State::Pending) => {
                        return Some(self.unsupported(&format!("native pendente `{n}` ({member})"), span));
                    }
                    None => return Some(self.unsupported(&format!("native desconhecido `{n}` ({member})"), span)),
                }
                n
            }
            None if recognized && let Some(op) = self.typed_factory(&member, args) => return Some(op),
            None if recognized => match crate::natives::intrinsic(&member) {
                Some(n) => n.to_string(),
                None => return Some(self.unsupported(&format!("intrínseco da VM `{member}`"), span)),
            },
            None => {
                if let Some(r) = self.call_annotated_native(fid, args, span) {
                    return Some(r);
                }
                return Some(self.unsupported(&format!("external sem implementação `{member}`"), span));
            }
        };
        // The signature of the native is the representation of the declared types;
        // `bool` crosses the boundary as `i8` (natives.rs).
        let mut a = Vec::with_capacity(args.len() + 1);
        if let Some(t) = this {
            // The receiver of a native of `int`/`double`/`bool` is the value (the
            // runtime functions receive `i64`/`double`, like the VM, which
            // unboxes the `Smi`); that of any other class, the `Ref`.
            let r = f.class.map_or(Type::Ref, |c| self.receiver_repr(c));
            let t = self.coerce(t, r);
            if r == Type::I1 {
                let b = self.emit(Instruction::ZExt { op: t, from: Type::I1, to: Type::I8 }, Type::I8);
                a.push((b, Type::I8));
            } else {
                a.push((t, r));
            }
        }
        // The parameters and the return in the value representation of the declared
        // types: `_Smi`/`_Double` (the implementations of `int`/`double`)
        // also cross as `i64`/`double`.
        let types: Vec<Type> = self
            .ctx
            .outline
            .functions
            .get(fid)
            .map(|d| d.parameters.iter().map(|p| self.native_repr(p.ty)).collect())
            .unwrap_or_default();
        for (i, v) in args.iter().enumerate() {
            let t = types.get(i).copied().unwrap_or_else(|| self.operand_type(v));
            let t = if t == Type::Void { Type::Ref } else { t };
            let v = self.coerce(v.clone(), t);
            if t == Type::I1 {
                let b = self.emit(Instruction::ZExt { op: v, from: Type::I1, to: Type::I8 }, Type::I8);
                a.push((b, Type::I8));
            } else {
                a.push((v, t));
            }
        }
        let ret = self.return_repr(fid);
        let ret_value = if super::generative_constructor(self.ctx, fid) {
            Type::Void
        } else {
            self.ctx.outline.functions.get(fid).map_or(ret, |d| self.native_repr(d.return_type))
        };
        // `RETURN_REF`: the runtime returns the box, and the declared value comes out
        // of the coercion (natives.rs).
        let ret_value = if crate::natives::RETURN_REF.contains(&name_str.as_str()) && ret_value != Type::Void {
            Type::Ref
        } else {
            ret_value
        };
        let native_ret = if ret_value == Type::I1 { Type::I8 } else { ret_value };
        let r = self.emit_call_with_check(
            Instruction::CallRuntime { name: crate::natives::symbol(&name_str), args: a, ret_ty: native_ret },
            native_ret,
        );
        let r = match ret_value {
            Type::Void => return Some(Operand::Constant(Constant::Null)),
            Type::I1 => self.emit(Instruction::ICmp(ICmpOp::Ne, r, Operand::Constant(Constant::Int(0))), Type::I1),
            _ => r,
        };
        Some(if ret == Type::Void { Operand::Constant(Constant::Null) } else { self.coerce(r, ret) })
    }

    /// The `vm:recognized` factories of the VM's `typed_data_patch.dart`: the
    /// public list (`Uint8List(n)`) allocates the internal list of the
    /// corresponding class (`_Uint8List`), and `_XArrayView._(base, shift, n)` the
    /// view (`typed_data.rs` of the runtime). The allocation registers the method
    /// table of the class, like that of a common object (LLVM emitter).
    fn typed_factory(&mut self, member: &str, args: &[Operand]) -> Option<Operand> {
        const LISTS: &[(&str, &str, i64)] = &[
            ("Int8List.", "_Int8List", 0),
            ("Uint8List.", "_Uint8List", 1),
            ("Uint8ClampedList.", "_Uint8ClampedList", 2),
            ("Int16List.", "_Int16List", 3),
            ("Uint16List.", "_Uint16List", 4),
            ("Int32List.", "_Int32List", 5),
            ("Uint32List.", "_Uint32List", 6),
            ("Int64List.", "_Int64List", 7),
            ("Uint64List.", "_Uint64List", 8),
            ("Float32List.", "_Float32List", 9),
            ("Float64List.", "_Float64List", 10),
            ("Float32x4List.", "_Float32x4List", 11),
            ("Int32x4List.", "_Int32x4List", 12),
            ("Float64x2List.", "_Float64x2List", 13),
        ];
        const VIEWS: &[(&str, i64)] = &[
            ("_Int8ArrayView", 0),
            ("_Uint8ArrayView", 1),
            ("_Uint8ClampedArrayView", 2),
            ("_Int16ArrayView", 3),
            ("_Uint16ArrayView", 4),
            ("_Int32ArrayView", 5),
            ("_Uint32ArrayView", 6),
            ("_Int64ArrayView", 7),
            ("_Uint64ArrayView", 8),
            ("_Float32ArrayView", 9),
            ("_Float64ArrayView", 10),
            ("_Float32x4ArrayView", 11),
            ("_Int32x4ArrayView", 12),
            ("_Float64x2ArrayView", 13),
            ("_ByteDataView", 14),
        ];
        let id_of = |b: &Self, name: &str| -> Option<i64> {
            b.ctx.sdk_class("typed_data", name).and_then(|c| b.ctx.class_id_of(c)).map(i64::from)
        };
        if let Some(&(_, internal, ty)) = LISTS.iter().find(|(m, _, _)| *m == member) {
            let cid = id_of(self, internal)?;
            let n = self.coerce(args.first()?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_novo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(ty)), Type::I64),
                        (n, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        // `Pointer<X>.asTypedList(n)` (`_asExternalTypedDataX` of
        // `ffi_patch.dart`): the internal list of the class, over the native memory
        // of the pointer (the VM's `ExternalTypedData`).
        const EXTERNALS: &[(&str, &str, i64)] = &[
            ("_asExternalTypedDataInt8", "_Int8List", 0),
            ("_asExternalTypedDataUint8", "_Uint8List", 1),
            ("_asExternalTypedDataInt16", "_Int16List", 3),
            ("_asExternalTypedDataUint16", "_Uint16List", 4),
            ("_asExternalTypedDataInt32", "_Int32List", 5),
            ("_asExternalTypedDataUint32", "_Uint32List", 6),
            ("_asExternalTypedDataInt64", "_Int64List", 7),
            ("_asExternalTypedDataUint64", "_Uint64List", 8),
            ("_asExternalTypedDataFloat", "_Float32List", 9),
            ("_asExternalTypedDataDouble", "_Float64List", 10),
        ];
        if let Some(&(_, internal, ty)) = EXTERNALS.iter().find(|(m, _, _)| *m == member) {
            let cid = id_of(self, internal)?;
            let pointer = self.coerce(args.first()?.clone(), Type::Ref);
            let n = self.coerce(args.get(1)?.clone(), Type::I64);
            return Some(self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "dartforge_typed_externo".to_string(),
                    args: vec![
                        (Operand::Constant(Constant::Int(cid)), Type::I64),
                        (Operand::Constant(Constant::Int(ty)), Type::I64),
                        (pointer, Type::Ref),
                        (n, Type::I64),
                    ],
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            ));
        }
        let (class, factory) = member.split_once('.')?;
        if factory != "_" {
            return None;
        }
        // `_UnmodifiableXArrayView` extends `_XArrayView`: the same type.
        let modifiable = class.strip_prefix("_Unmodifiable").map(|r| format!("_{r}"));
        let searched = modifiable.as_deref().unwrap_or(class);
        let &(_, ty) = VIEWS.iter().find(|(c, _)| *c == searched)?;
        // The `VISAO_IMUTAVEL` bit of the runtime: the unmodifiable view rejects
        // the fast path of `[]=` (its `[]=` throws, as in the VM).
        let ty = if modifiable.is_some() { ty | 0x100 } else { ty };
        let cid = id_of(self, class)?;
        let base = self.coerce(args.first()?.clone(), Type::Ref);
        let shift = self.coerce(args.get(1)?.clone(), Type::I64);
        let n = self.coerce(args.get(2)?.clone(), Type::I64);
        Some(self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_view_nova".to_string(),
                args: vec![
                    (Operand::Constant(Constant::Int(cid)), Type::I64),
                    (Operand::Constant(Constant::Int(ty)), Type::I64),
                    (base, Type::Ref),
                    (shift, Type::I64),
                    (n, Type::I64),
                ],
                ret_ty: Type::Ref,
            },
            Type::Ref,
        ))
    }

    /// The representation of a type at the boundary with a native: the value one
    /// (`i64`, `double`, `i1`) for the non-nullable classes of `int`, `double` and `bool`
    /// (including `_Smi`, `_Mint`, `_Double`), otherwise that of the HIR.
    fn native_repr(&self, ty: dartforge_types::table::TypeId) -> Type {
        if self.ctx.is_void(ty) {
            return Type::Void;
        }
        if let dartforge_types::table::Type::Interface { class, nullable: false, .. } = self.ctx.table.get(ty) {
            let r = self.receiver_repr(*class);
            if r != Type::Ref {
                return r;
            }
        }
        self.repr(ty)
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// The representation of the receiver of a member of `cid`: the value for the
    /// classes of `int`, `double` and `bool` (and their implementations,
    /// `_Smi`, `_Mint`, `_Double`), otherwise `Ref`.
    fn receiver_repr(&self, cid: ClassId) -> Type {
        let core = self.ctx.core;
        let de = |c: Option<ClassId>| c.is_some_and(|c| subclass_of(self.ctx, cid, c));
        if de(core.int_class) {
            Type::I64
        } else if de(core.double_class) {
            Type::F64
        } else if de(core.bool_class) {
            Type::I1
        } else {
            Type::Ref
        }
    }

    /// A native that the lowering generates in place of the call
    /// (`natives::State::Builtin`).
    fn embedded_native(&mut self, name_str: &str, fid: usize, args: &[Operand]) -> Operand {
        let ret = self.return_repr(fid);
        match name_str {
            // `unsafeCast<T>(v)`: the value itself, without a check.
            "Internal_unsafeCast" => {
                let v = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                if ret == Type::Void { v } else { self.coerce(v, ret) }
            }
            // `ClassID.getID(o)`: the class of the value inline (`@df.classe`,
            // docs/NATIVO-ESPACO-UNIFICADO.md §3.8), without calling the runtime.
            "ClassID_getID" => {
                let o = args.first().cloned().unwrap_or(Operand::Constant(Constant::Null));
                let o = self.coerce(o, Type::Ref);
                let c = self.emit(
                    Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(o, Type::Ref)], ret_ty: Type::I64 },
                    Type::I64,
                );
                if ret == Type::Void { c } else { self.coerce(c, ret) }
            }
            _ => self.unsupported(&format!("native embutido `{name_str}`"), Span { start: 0, end: 0 }),
        }
    }
}

/// A concrete implementation of an instance member.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Implementation {
    /// Method, getter, setter or operator (with a body, a patch or a native).
    Function(usize),
    /// Field (the implicit getter/setter).
    Field(VariableId),
}

/// The distinct implementations of `key` (the name; `x_=` for the setter)
/// in the compiled concrete classes that are subtypes of `cid`, each one
/// found by the linearization of the class (the first that implements it).
pub fn implementations(ctx: &Context, cid: ClassId, key: &str) -> Vec<Implementation> {
    let Some(sym) = ctx.interner.lookup(key) else { return Vec::new() };
    if let Some(r) = ctx.implementations_memo.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().clone();
    }
    let r = implementations_without_memo(ctx, cid, sym);
    ctx.implementations_memo.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), std::sync::Arc::new(r.clone()));
    r
}

fn implementations_without_memo(ctx: &Context, cid: ClassId, sym: dartforge_intern::SymbolId) -> Vec<Implementation> {
    let mut exit = Vec::new();
    for &kid in ctx.subtypes(cid).iter() {
        let class = &ctx.program.classes[kid.0 as usize];
        if !ctx.compiled_library(class.library)
            || class.modifiers.abstract_
            || super::members::is_mixin(ctx, kid)
        {
            continue;
        }
        for c in linearization(ctx, kid) {
            let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&sym) else { continue };
            let f = f.0 as usize;
            if !implemented(ctx, f) {
                continue;
            }
            let i = match ctx.program.functions[f].variable {
                Some(v) => Implementation::Field(v),
                None => Implementation::Function(f),
            };
            if !exit.contains(&i) {
                exit.push(i);
            }
            break;
        }
    }
    exit
}

/// Each compiled concrete class, a subtype of `cid`, with the implementation
/// of `key` that its linearization finds (as in [`implementations`]), by the
/// runtime class id; `None` if some of them has no id.
pub fn implementations_by_class(ctx: &Context, cid: ClassId, key: &str) -> Option<Vec<(i64, Implementation)>> {
    let sym = ctx.interner.lookup(key)?;
    if let Some(r) = ctx.implementations_by_class_memo.read().unwrap_or_else(|e| e.into_inner()).get(&(cid, sym)) {
        return r.as_ref().map(|v| v.as_ref().clone());
    }
    let r = implementations_by_class_without_memo(ctx, cid, sym);
    ctx.implementations_by_class_memo.write().unwrap_or_else(|e| e.into_inner()).insert((cid, sym), r.clone().map(std::sync::Arc::new));
    r
}

fn implementations_by_class_without_memo(
    ctx: &Context,
    cid: ClassId,
    sym: dartforge_intern::SymbolId,
) -> Option<Vec<(i64, Implementation)>> {
    let mut exit = Vec::new();
    for &kid in ctx.subtypes(cid).iter() {
        let class = &ctx.program.classes[kid.0 as usize];
        if !ctx.compiled_library(class.library)
            || class.modifiers.abstract_
            || super::members::is_mixin(ctx, kid)
        {
            continue;
        }
        for c in linearization(ctx, kid) {
            let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&sym) else { continue };
            let f = f.0 as usize;
            if !implemented(ctx, f) {
                continue;
            }
            let i = match ctx.program.functions[f].variable {
                Some(v) => Implementation::Field(v),
                None => Implementation::Function(f),
            };
            exit.push((i64::from(ctx.class_id_of(kid)?), i));
            break;
        }
    }
    Some(exit)
}

/// How many distinct implementations the dispatch by class
/// (`class_dispatch`) accepts; above that, the selector.
const FEW_TARGETS: usize = 4;

/// What an entry of the method table does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Adapter {
    /// Calls the method; reads and calls the value of a getter.
    Call,
    /// Reads the getter; takes the tear-off of a method.
    Read,
    /// Calls the setter.
    Write,
}

impl Adapter {
    fn suffix(self) -> &'static str {
        match self {
            Adapter::Call => "$c",
            Adapter::Read => "$g",
            Adapter::Write => "$s",
        }
    }
}

/// The base symbol of an instance field: `df.<lib>.<Class>.<field>`.
fn field_symbol(ctx: &Context, vid: VariableId) -> String {
    let v = &ctx.program.variables[vid.0 as usize];
    let owner = v.class.map(|c| ctx.symbol_name(ctx.program.classes[c.0 as usize].name).to_string()).unwrap_or_default();
    format!(
        "df.{}.{}.{}",
        crate::context::escape_str(&ctx.library_name(v.library)),
        crate::context::escape_str(&owner),
        crate::context::escape_str(ctx.symbol_name(v.name))
    )
}

/// The implemented instance member (it has a body, is a field, or is `external`
/// with a patch or a native).
fn implemented(ctx: &Context, fid: usize) -> bool {
    let f = &ctx.program.functions[fid];
    if f.variable.is_some() {
        return !f.abstract_;
    }
    if f.external {
        return true;
    }
    super::members::has_body(ctx, fid)
}

/// The method table of a concrete class: for each selector that it
/// answers (its members and the inherited ones, through the linearization, the first
/// that implements it), the uniform entry.
pub fn method_table(ctx: &Context, cid: ClassId) -> Vec<(String, String)> {
    let mut seen_set = std::collections::HashSet::new();
    let mut exit = Vec::new();
    let mut chain = linearization(ctx, cid);
    // The hierarchy of certain synthetic elements (notably enums) does not
    // include `Object` in `supertype_class`. Their members keep inheriting
    // `Object.==`, `hashCode` and the other accessors in Dart semantics.
    if let Some(object) = ctx.core.object_class && !chain.contains(&object) {
        chain.push(object);
    }
    for c in chain {
        let class = &ctx.program.classes[c.0 as usize];
        let mut members: Vec<(&str, usize)> = class
            .instance_members
            .iter()
            .map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize))
            .collect();
        members.sort();
        for (key, fid) in members {
            if !implemented(ctx, fid) {
                continue;
            }
            let f = &ctx.program.functions[fid];
            let is_setter = key.ends_with("_=");
            let name_str = key.strip_suffix("_=").unwrap_or(key);
            // The typed entry of the method (`typed_entry.rs`), when it
            // has one, in the `t…` selector — only if its `c:`/`s:` won here.
            let typed = f.variable.is_none()
                && !matches!(f.kind, FunctionKind::Getter)
                && super::typed_entry::needs_typed_entry(ctx, fid);
            // Outside the closed world of the program (C7, `native_world.rs`): the
            // selector stays taken (a superclass does not fill the place with
            // another member), but without an entry or adapter — the dynamic
            // call to it falls into `noSuchMethod`, like the one the world says
            // does not happen.
            let live = !ctx.pruned_member(fid);
            let mut by = |ty: AccessKind, a: Adapter, base: &str| {
                let s = selector_string(ctx, ty, name_str, f.library);
                if seen_set.insert(s.clone()) && live {
                    if typed && a != Adapter::Read {
                        exit.push((super::typed_entry::typed_selector(&s), format!("{base}$t{}", &a.suffix()[1..])));
                    }
                    exit.push((s, format!("{base}{}", a.suffix())));
                }
            };
            if let Some(v) = f.variable {
                let base = field_symbol(ctx, v);
                if is_setter {
                    by(AccessKind::Write, Adapter::Write, &base);
                } else {
                    by(AccessKind::Read, Adapter::Read, &base);
                    by(AccessKind::Call, Adapter::Call, &base);
                }
                continue;
            }
            let base = super::symbol_of(ctx, fid);
            match f.kind {
                FunctionKind::Setter => by(AccessKind::Write, Adapter::Write, &base),
                FunctionKind::Getter => {
                    by(AccessKind::Read, Adapter::Read, &base);
                    by(AccessKind::Call, Adapter::Call, &base);
                }
                FunctionKind::Constructor | FunctionKind::SyntheticConstructor => {}
                _ => {
                    by(AccessKind::Call, Adapter::Call, &base);
                    by(AccessKind::Read, Adapter::Read, &base);
                }
            }
        }
    }
    exit
}

/// The class (or a superclass of the program) declares `noSuchMethod`: that of
/// `Object` does not count.
fn has_own_no_such_method(ctx: &Context, cid: ClassId) -> bool {
    let Some(nsm) = ctx.interner.lookup("noSuchMethod") else { return false };
    for c in linearization(ctx, cid) {
        if Some(c) == ctx.core.object_class {
            return false;
        }
        if let Some(&f) = ctx.program.classes[c.0 as usize].instance_members.get(&nsm)
            && implemented(ctx, f.0 as usize)
        {
            return true;
        }
    }
    false
}

/// The instance members of the superinterfaces of `cid` (superclasses,
/// mixins, `implements`, `on`, transitively) whose selector the table does not
/// have: (selector, use, declared member).
fn members_to_forward(ctx: &Context, cid: ClassId, table: &[(String, String)]) -> Vec<(String, AccessKind, usize)> {
    let mut seen_set: std::collections::HashSet<String> = table.iter().map(|(s, _)| s.clone()).collect();
    let mut exit = Vec::new();
    let mut stack = vec![cid];
    let mut classes = std::collections::HashSet::new();
    while let Some(c) = stack.pop() {
        if !classes.insert(c) || Some(c) == ctx.core.object_class {
            continue;
        }
        let class = &ctx.program.classes[c.0 as usize];
        stack.extend(class.supertype_class);
        stack.extend(class.mixin_classes.iter().copied());
        stack.extend(class.interface_classes.iter().copied());
        stack.extend(class.on_classes.iter().copied());
        let mut members: Vec<(&str, usize)> =
            class.instance_members.iter().map(|(k, f)| (ctx.symbol_name(*k), f.0 as usize)).collect();
        members.sort();
        for (key, fid) in members {
            let f = &ctx.program.functions[fid];
            if f.static_ || matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) {
                continue;
            }
            let name_str = key.strip_suffix("_=").unwrap_or(key);
            let uses: &[AccessKind] = match f.kind {
                FunctionKind::Setter => &[AccessKind::Write],
                FunctionKind::Getter => &[AccessKind::Read],
                FunctionKind::ImplicitAccessor if key.ends_with("_=") => &[AccessKind::Write],
                FunctionKind::ImplicitAccessor => &[AccessKind::Read],
                _ => &[AccessKind::Call],
            };
            for &use_site in uses {
                let s = selector_string(ctx, use_site, name_str, f.library);
                if seen_set.insert(s.clone()) {
                    exit.push((s, use_site, fid));
                }
            }
        }
    }
    exit
}

/// The `noSuchMethod` forwarder of the member `fid` (uniform entry):
/// unpacks the arguments as the member declares them (the defaults of the
/// omitted optional ones), and the runtime builds the `Invocation` in declaration
/// order and calls the `noSuchMethod` of the receiver.
fn generate_nsm_forwarder(ctx: &Context, module: &mut Module, unit: UnitId, symbol: &str, fid: usize, ty: AccessKind) {
    let f = &ctx.program.functions[fid];
    let name_str = ctx.symbol_name(f.name).to_string();
    let mut b = FnBuilder::new(ctx, unit, symbol.to_string(), name_str.clone(), Type::Ref);
    let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
    let args = Operand::Val(b.add_param_vector(DartEntryVector::Arguments));
    let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
    b.declare_arc_uniform_return();
    let (code, values, names, tuple) = match ty {
        AccessKind::Call => {
            let infos = b.function_params(fid);
            let Some(vals) = b.unpack(&infos, args.clone(), desc.clone()) else {
                b.finish(module);
                return;
            };
            let names: Vec<String> = infos
                .iter()
                .filter(|p| p.kind == ParameterKind::Named)
                .map(|p| p.name.clone().unwrap_or_default())
                .collect();
            // The tuple of the generic method comes after the arguments.
            let tuple = if b.generic_function(fid) {
                let npos = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
                let nnom = b.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
                let i = b.emit(Instruction::Add(npos, nnom), Type::I64);
                b.emit(Instruction::LoadIndexed { base: args.clone(), index: i }, Type::I64)
            } else {
                Operand::Constant(Constant::Int(0))
            };
            (0, vals, names, tuple)
        }
        AccessKind::Read => (1, Vec::new(), Vec::new(), Operand::Constant(Constant::Int(0))),
        AccessKind::Write => {
            let v = b.emit(Instruction::LoadIndexed { base: args.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
            (2, vec![v], Vec::new(), Operand::Constant(Constant::Int(0)))
        }
    };
    let npos = values.len() - names.len();
    let nnom = names.len();
    // `dartforge_encaminhar_nsm` reads all the captures as `Ref`: the values
    // go boxed (docs/NATIVO-ESPACO-UNIFICADO.md §4.10, item 48).
    let mut fields: Vec<Operand> = values.into_iter().map(|v| b.coerce(v, Type::Ref)).collect();
    for n in names {
        fields.push(b.emit(Instruction::Const(Constant::String(n)), Type::Ref));
    }
    let env = b.emit(Instruction::AllocEnv { values: fields }, Type::Ref);
    let text = b.emit(Instruction::Const(Constant::String(name_str)), Type::Ref);
    let r = b.emit_call_with_check(
        Instruction::CallRuntime {
            name: "dartforge_encaminhar_nsm".to_string(),
            args: vec![
                (this, Type::Ref),
                (Operand::Constant(Constant::Int(code)), Type::I64),
                (text, Type::Ref),
                (env, Type::Ref),
                (Operand::Constant(Constant::Int(npos as i64)), Type::I64),
                (Operand::Constant(Constant::Int(nnom as i64)), Type::I64),
                (tuple, Type::I64),
            ],
            ret_ty: Type::Ref,
        },
        Type::Ref,
    );
    b.terminate(Terminator::Return(Some(r)));
    b.finish(module);
}

/// The uniform entries of an instance member (or field) of the module.
pub fn lower_function_adapters(ctx: &Context, module: &mut Module, fid: usize) {
    let f = &ctx.program.functions[fid];
    if f.static_ || f.class.is_none() || !implemented(ctx, fid) {
        return;
    }
    if matches!(f.kind, FunctionKind::Constructor | FunctionKind::SyntheticConstructor) || f.factory {
        return;
    }
    if f.variable.is_some() {
        return;
    }
    let base = super::symbol_of(ctx, fid);
    let adapters: &[Adapter] = match f.kind {
        FunctionKind::Setter => &[Adapter::Write],
        FunctionKind::Getter => &[Adapter::Read, Adapter::Call],
        _ => &[Adapter::Call, Adapter::Read],
    };
    // The typed entry (`$tc`, `$ts`): only checks the covariant ones
    // (`typed_entry.rs`), and only exists when it skips some check.
    let mut list: Vec<(Adapter, bool)> = adapters.iter().map(|&a| (a, false)).collect();
    if !matches!(f.kind, FunctionKind::Getter) && super::typed_entry::needs_typed_entry(ctx, fid) {
        list.extend(adapters.iter().filter(|a| **a != Adapter::Read).map(|&a| (a, true)));
    }
    for (a, typed) in list {
        let symbol = if typed { format!("{base}$t{}", &a.suffix()[1..]) } else { format!("{base}{}", a.suffix()) };
        let unit = unit_of(ctx, f.class.expect("class member"));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, symbol, ctx.symbol_name(f.name).to_string(), Type::Ref);
        b.in_adapter = true;
        b.guaranteed_arity = typed;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Arguments));
        let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
        b.declare_arc_uniform_return();
        let span = Span { start: 0, end: 0 };
        let r = match (a, f.kind) {
            (Adapter::Read, FunctionKind::Getter) => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finish(module);
                    continue;
                }
                b.call_direct(fid, Some(recv), Vec::new())
            }
            (Adapter::Call, FunctionKind::Getter) => {
                let v = b.call_direct(fid, Some(recv), Vec::new());
                let v = b.coerce(v, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureForward { arc_owned: false, closure: v, args, desc }, Type::Ref)
            }
            (Adapter::Read, _) => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finish(module);
                    continue;
                }
                b.method_tearoff(recv, fid, span)
            }
            _ => {
                let infos = b.function_params(fid);
                let Some(vals) = b.unpack(&infos, args.clone(), desc.clone()) else {
                    b.finish(module);
                    continue;
                };
                // The VM checks the arguments at the entry of the method, before the
                // body (and the native, in the SDK: `String.+(String)` cannot
                // receive a Smi as a String handle via `dynamic`): the
                // class covariant ones and those of a nominal type, with the message
                // " of 'name'" — before converting each one to the representation.
                b.this_param = Some(recv.clone());
                b.finalizable_this = ctx.classify_this(fid);
                b.enclosing_class = f.class;
                b.check_entry_arguments_with(fid, &vals, typed);
                let reprs: Vec<Type> = ctx.outline.functions[fid].parameters.iter().map(|p| b.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| b.coerce(v, r)).collect();
                if b.generic_function(fid) {
                    let npos = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) },
                        Type::I64,
                    );
                    let nnom = b.emit(
                        Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) },
                        Type::I64,
                    );
                    let index = b.emit(Instruction::Add(npos, nnom), Type::I64);
                    b.armed_tuple = Some(b.emit(Instruction::LoadIndexed { base: args.clone(), index: index }, Type::I64));
                }
                b.call_direct(fid, Some(recv), vals)
            }
        };
        let r = if matches!(b.operand_type(&r), Type::Void) { Operand::Constant(Constant::Null) } else { b.coerce(r, Type::Ref) };
        b.terminate(Terminator::Return(Some(r)));
        b.finish(module);
    }
}

/// The uniform entries of an instance field: `$g`, `$c` and, if it has a
/// setter, `$s`.
pub fn lower_field_adapters(ctx: &Context, module: &mut Module, vid: VariableId) {
    let v = &ctx.program.variables[vid.0 as usize];
    let Some(cid) = v.class else { return };
    if v.static_ {
        return;
    }
    let Some(unit) = unit_of(ctx, cid) else { return };
    let base = field_symbol(ctx, vid);
    let span = Span { start: 0, end: 0 };
    let mut types = vec![Adapter::Read, Adapter::Call];
    if v.setter.is_some() {
        types.push(Adapter::Write);
    }
    for a in types {
        let mut b = FnBuilder::new(ctx, unit, format!("{base}{}", a.suffix()), ctx.symbol_name(v.name).to_string(), Type::Ref);
        b.in_adapter = true;
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Arguments));
        let desc = Operand::Val(b.add_param_vector(DartEntryVector::Descriptor));
        b.declare_arc_uniform_return();
        let r = match a {
            Adapter::Read => {
                if b.unpack(&[], args, desc).is_none() {
                    b.finish(module);
                    continue;
                }
                b.read_field_with_late(recv, vid, span)
            }
            Adapter::Call => {
                let x = b.read_field_with_late(recv, vid, span);
                let x = b.coerce(x, Type::Ref);
                b.emit_call_with_check(Instruction::CallClosureForward { arc_owned: false, closure: x, args, desc }, Type::Ref)
            }
            Adapter::Write => {
                let one = [super::closures::EntryParam {
                    name: None,
                    kind: ParameterKind::Required,
                    required: true,
                    pattern: super::closures::DefaultValue::Absent,
                }];
                let Some(vals) = b.unpack(&one, args, desc) else {
                    b.finish(module);
                    continue;
                };
                b.store_field(recv, vid, vals[0].clone(), span);
                Operand::Constant(Constant::Null)
            }
        };
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finish(module);
    }
}

/// The unit that declares the class (where its adapters are lowered).
fn unit_of(ctx: &Context, cid: ClassId) -> Option<dartforge_elements::model::UnitId> {
    let c = &ctx.program.classes[cid.0 as usize];
    c.decl.map(|d| d.unit).or_else(|| ctx.program.library(c.library).units.first().copied())
}

/// The function in place of a rejected SDK member: the same signature, and the
/// body warns at runtime which member and why
/// (`dartforge_membro_recusado`) — never a wrong output.
pub fn rejected_function(ctx: &Context, fid: usize, reason: &str) -> Function {
    let f = &ctx.program.functions[fid];
    let unit = match f.node {
        dartforge_elements::model::FunctionRef::Function { unit, .. }
        | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => Some(unit),
        dartforge_elements::model::FunctionRef::None => f.class.and_then(|c| unit_of(ctx, c)),
    }
    .or_else(|| ctx.program.library(f.library).units.first().copied())
    .expect("library with a unit");
    let generative = super::generative_constructor(ctx, fid);
    let ret = if generative {
        Type::Void
    } else {
        ctx.hir_return(fid)
    };
    let symbol = super::symbol_of(ctx, fid);
    let mut b = FnBuilder::new(ctx, unit, symbol.clone(), ctx.symbol_name(f.name).to_string(), ret);
    let with_this = (f.class.is_some() && !f.static_ && !f.factory) || generative;
    if with_this {
        b.add_param("this".to_string(), Type::Ref);
    }
    if let Some(d) = ctx.outline.functions.get(fid) {
        for p in &d.parameters {
            let t = b.repr(p.ty);
            b.add_param("p".to_string(), t);
        }
    }
    let text = format!("{symbol} ({reason})");
    let t = b.emit(Instruction::Const(Constant::String(text)), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// The ARC facts of a rejection body (which never returns, but keeps the
/// signature of the member): the Ref return follows the Owned convention and the
/// non-Ref parameters, in the representation of the Dart type, are scalars.
fn register_rejection_facts(ctx: &Context, module: &mut Module, f: &Function) {
    if !ctx.arc_memory {
        return;
    }
    if f.return_ty == Type::Ref {
        module.dart_ref_returns.insert(f.symbol.clone());
    }
    let scalars: std::collections::HashSet<ValueId> = f
        .params
        .iter()
        .filter(|(_, _, t)| matches!(t, Type::I64 | Type::F64 | Type::I1 | Type::I8))
        .map(|(v, _, _)| *v)
        .collect();
    if !scalars.is_empty() {
        module.dart_scalar_parameters.entry(f.symbol.clone()).or_default().extend(scalars);
    }
}

/// Lowers a source SDK function in a separate module; if it does not lower
/// (a construct diagnostic, a pending native, an intrinsic, a panic of the
/// lowering, a verifier problem), the module receives in its place the
/// rejected function (`rejected_function`), and the reason goes to
/// `Module::rejected`.
pub fn lower_function_or_reject(ctx: &Context, module: &mut Module, fid: usize) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        super::lower_function(ctx, &mut m, fid);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    let reason = match &r {
        Ok(m) if m.errors.is_empty() => None,
        Ok(m) => Some(
            m.errors[0]
                .strip_prefix(crate::UNSUPPORTED_PREFIX)
                .unwrap_or(&m.errors[0])
                .to_string(),
        ),
        Err(p) => Some(format!(
            "pânico do lowering: {}",
            p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
        )),
    };
    match (r, reason) {
        (Ok(mut m), None) => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
        }
        (_, Some(reason)) => {
            if !super::members::has_body(ctx, fid) {
                return;
            }
            let short = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
            let symbol = super::symbol_of(ctx, fid);
            let f = rejected_function(ctx, fid, &short);
            register_rejection_facts(ctx, module, &f);
            module.functions.push(f);
            // The whole diagnostic goes into the summary, with the position.
            module.rejected.push((symbol, reason));
        }
        (Err(_), None) => unreachable!(),
    }
}

/// The functions that return the non-literal default values of the parameters
/// of the functions of the module (`members::pattern_symbol`): a caller from outside
/// the module (the program) does not have the resolutions of the SDK units and calls
/// this function instead of lowering the expression.
pub fn lower_sdk_patterns(ctx: &Context, module: &mut Module) {
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        if !ctx.library_in_module(f.library) {
            continue;
        }
        let unit = match f.node {
            dartforge_elements::model::FunctionRef::Function { unit, .. }
            | dartforge_elements::model::FunctionRef::Constructor { unit, .. } => unit,
            dartforge_elements::model::FunctionRef::None => continue,
        };
        let base_symbol = super::symbol_of(ctx, fid);
        let b = FnBuilder::new(ctx, unit, String::new(), String::new(), Type::Ref);
        let Some((unit_ref, params)) = b.parameters_of(fid) else { continue };
        let exported: Vec<usize> = params
            .iter()
            .enumerate()
            .filter(|(_, p)| p.default_value.is_some_and(|e| !super::members::literal_pattern(&ctx.program.unit(unit_ref).ast, e)))
            .map(|(i, _)| i)
            .collect();
        drop(b);
        for i in exported {
            let symbol = format!("{base_symbol}.padrao.{i}");
            let mut b = FnBuilder::new(ctx, unit, symbol.clone(), symbol.clone(), Type::Ref);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let v = b.default_value(fid, i).unwrap_or(Operand::Constant(Constant::Null));
                let v = b.coerce(v, Type::Ref);
                b.terminate(Terminator::Return(Some(v)));
                b
            }));
            match r {
                Ok(mut b) if b.errors.is_empty() => {
                    // Dart body called by the module of the parameter: the Ref
                    // return is Owned.
                    if ctx.arc_memory {
                        b.dart_ref_returns.insert(symbol.clone());
                    }
                    b.finish(module);
                }
                Ok(b) => {
                    let reason = b.errors[0].clone();
                    module.functions.push(rejected_pattern(ctx, unit, &symbol, &reason));
                    module.rejected.push((symbol, reason));
                }
                Err(_) => {
                    let reason = "pânico ao baixar o valor padrão".to_string();
                    module.functions.push(rejected_pattern(ctx, unit, &symbol, &reason));
                    module.rejected.push((symbol, reason));
                }
            }
        }
    }
}

/// The default function that does not lower: warns at runtime, like the
/// rejected functions.
fn rejected_pattern(ctx: &Context, unit: dartforge_elements::model::UnitId, symbol: &str, reason: &str) -> Function {
    let mut b = FnBuilder::new(ctx, unit, symbol.to_string(), symbol.to_string(), Type::Ref);
    let short = reason.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(reason);
    let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({short})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    b.func
}

/// The adapters of the instance members of the classes of the module and the
/// method tables of its concrete classes (source SDK).
pub fn lower_adapters_and_tables(ctx: &Context, module: &mut Module) {
    lower_sdk_patterns(ctx, module);
    for (fid, f) in ctx.program.functions.iter().enumerate() {
        let name_str = ctx.symbol_name(f.name);
        if f.class.is_none() && name_str.starts_with("_dartforge") && ctx.library_in_module(f.library) {
            module.helpers.push((name_str.to_string(), super::symbol_of(ctx, fid)));
        }
    }
    for (k, class) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.library_in_module(class.library) {
            continue;
        }
        let mut fids: Vec<usize> = class.instance_members.values().map(|f| f.0 as usize).collect();
        fids.sort_unstable();
        fids.dedup();
        for fid in fids {
            // Outside the closed world (C7): no entry in the table, no adapter.
            if ctx.program.functions[fid].class != Some(cid) || ctx.pruned_member(fid) {
                continue;
            }
            adapters_or_reject(ctx, module, |m| lower_function_adapters(ctx, m, fid));
        }
        for &vid in &class.fields {
            if ctx.program.variables[vid.0 as usize].static_ || ctx.pruned_field(vid) {
                continue;
            }
            adapters_or_reject(ctx, module, |m| lower_field_adapters(ctx, m, vid));
        }
        let concrete = !class.modifiers.abstract_ && !super::members::is_mixin(ctx, cid);
        if concrete && let Some(id) = ctx.class_id_of(cid) {
            let mut table = method_table(ctx, cid);
            // The VM implements `_StackTrace.toString` in C++, without a Dart
            // declaration in the class. Our object keeps the string in field zero, or
            // the addresses of the `throw` that the runtime symbolizes on the first
            // request (the symbolic stack trace, §13.14).
            if ctx.symbol_name(class.name) == "_StackTrace"
                && ctx.program.library(class.library).uri == "dart:core"
                && let Some(u) = unit_of(ctx, cid)
            {
                let symbol = "df.$stackTrace.toString$c".to_string();
                let mut b = FnBuilder::new(ctx, u, symbol.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Arguments);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let text = b.emit(Instruction::CallRuntime {
                    name: "dartforge_rastro_texto".to_string(),
                    args: vec![(this, Type::Ref)],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(text)));
                b.finish(module);
                table.retain(|(s, _)| s != "c:toString");
                table.push(("c:toString".to_string(), symbol));
            }
            // The layout generated for enums keeps `_Enum._name` in field 1.
            // SDK extensions (byName/asNameMap) read that field through the
            // private selector of dart:core, including in user enums.
            if super::enums::is_enum(ctx, cid)
                && let Some(decl) = class.decl
            {
                let symbol = format!(
                    "df.{}.{}.$enumName$g",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "_name".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Arguments);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let name_str = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(1)), Type::I64),
                    ],
                    ret_ty: Type::Ref,
                }, Type::Ref);
                b.terminate(Terminator::Return(Some(name_str)));
                b.finish(module);
                table.retain(|(s, _)| s != "g:_name@dart:core");
                table.push(("g:_name@dart:core".to_string(), symbol));
                // `index` (field 0) through the selector: `x.index` with `x` of type
                // `Enum` or `T extends Enum` (the analyzer's `EnumSet.updated`)
                // arrives through the table, not through the static type.
                let symbol = format!(
                    "df.{}.{}.$enumIndex$g",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "index".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Arguments);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let index = b.emit(Instruction::CallRuntime {
                    name: "dartforge_object_get".to_string(),
                    args: vec![
                        (this, Type::Ref),
                        (Operand::Constant(Constant::Int(0)), Type::I64),
                    ],
                    ret_ty: Type::I64,
                }, Type::I64);
                let index = b.coerce(index, Type::Ref);
                b.terminate(Terminator::Return(Some(index)));
                b.finish(module);
                table.retain(|(s, _)| s != "g:index");
                table.push(("g:index".to_string(), symbol));
            }
            // An enum without an override gets `Enum.name` from the emitter. The
            // selector table must use that same body, including when the
            // value is printed by the source `List.toString`.
            if super::enums::is_enum(ctx, cid)
                && module.classes.iter().any(|c| c.id == id && c.to_string_symbol.is_none())
                && let Some(decl) = class.decl
            {
                let base = format!(
                    "df.{}.{}.toString",
                    crate::context::escape_str(&ctx.library_name(class.library)),
                    crate::context::escape_str(ctx.symbol_name(class.name))
                );
                let symbol = format!("{base}$c");
                let mut b = FnBuilder::new(ctx, decl.unit, symbol.clone(), "toString".to_string(), Type::Ref);
                let this = Operand::Val(b.add_param("this".to_string(), Type::Ref));
                b.add_param_vector(DartEntryVector::Arguments);
                b.add_param_vector(DartEntryVector::Descriptor);
                b.declare_arc_uniform_return();
                let text = b.emit_call_with_check(
                    Instruction::CallStatic { symbol: base, args: vec![this], ret_ty: Type::Ref },
                    Type::Ref,
                );
                b.terminate(Terminator::Return(Some(text)));
                b.finish(module);
                table.retain(|(s, _)| s != "c:toString");
                table.push(("c:toString".to_string(), symbol));
            }
            // `noSuchMethod` forwarders: the class with its own `noSuchMethod`
            // answers every member of its interfaces that it does not
            // implement (the VM generates the forwarder in the kernel).
            if let Some(decl) = class.decl
                && has_own_no_such_method(ctx, cid)
            {
                for (selector, ty, fid) in members_to_forward(ctx, cid, &table) {
                    let symbol = format!(
                        "df.{}.{}.$nsm.{}",
                        crate::context::escape_str(&ctx.library_name(class.library)),
                        crate::context::escape_str(ctx.symbol_name(class.name)),
                        crate::context::escape_str(&selector)
                    );
                    generate_nsm_forwarder(ctx, module, decl.unit, &symbol, fid, ty);
                    table.push((selector, symbol));
                }
            }
            module.method_tables.push((id, table_symbol(ctx, cid), table));
        }
    }
    // The table function of every compiled concrete class (the allocation, in
    // any module, registers the table of the class).
    for (k, class) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(k as u32);
        if !ctx.compiled_library(class.library) || class.modifiers.abstract_ || super::members::is_mixin(ctx, cid) {
            continue;
        }
        if let Some(id) = ctx.class_id_of(cid) {
            module.table_functions.insert(id, table_symbol(ctx, cid));
        }
    }
}

/// The function that returns the method table of a class:
/// `df.mt.<library>.<Class>`.
pub fn table_symbol(ctx: &Context, cid: ClassId) -> String {
    let c = &ctx.program.classes[cid.0 as usize];
    format!(
        "df.mt.{}.{}",
        crate::context::escape_str(&ctx.library_name(c.library)),
        crate::context::escape_str(ctx.symbol_name(c.name))
    )
}

/// The adapters generated by `gerar`; those that do not lower become entries that
/// warn at runtime.
fn adapters_or_reject(ctx: &Context, module: &mut Module, gerar: impl FnOnce(&mut Module)) {
    let mut m = Module::new();
    m.sdk_mode = true;
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| gerar(&mut m)));
    if r.is_ok() && m.errors.is_empty() {
        module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
        module.globals.extend(m.globals);
        return;
    }
    let reason = m
        .errors
        .first()
        .map(|e| e.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(e).to_string())
        .unwrap_or_else(|| "pânico do lowering".to_string());
    let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
    // Each built uniform entry and the corresponding typed one
    // (`$tc`/`$ts`, `typed_entry.rs`), which the table may cite even without
    // having been built before the error.
    let mut entries: Vec<(String, String)> = Vec::new();
    for f in &m.functions {
        let s = &f.symbol;
        if !(s.ends_with("$c") || s.ends_with("$g") || s.ends_with("$s") || s.ends_with("$tc") || s.ends_with("$ts")) {
            continue;
        }
        let typed = (s.ends_with("$c") || s.ends_with("$s")) && !s.ends_with("$tc") && !s.ends_with("$ts");
        for symbol in [Some(s.clone()), typed.then(|| format!("{}$t{}", &s[..s.len() - 2], &s[s.len() - 1..]))].into_iter().flatten() {
            if !entries.iter().any(|(x, _)| *x == symbol) {
                entries.push((symbol, f.name.clone()));
            }
        }
    }
    for (symbol, name_str) in entries {
        let unit = ctx.program.units.iter().position(|_| true).map(|u| dartforge_elements::model::UnitId(u as u32));
        let Some(unit) = unit else { continue };
        let mut b = FnBuilder::new(ctx, unit, symbol.clone(), name_str, Type::Ref);
        b.add_param("this".to_string(), Type::Ref);
        b.add_param_vector(DartEntryVector::Arguments);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({reason})"))), Type::Ref);
        b.emit(
            Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
            Type::Void,
        );
        b.terminate(Terminator::Unreachable);
        module.rejected.push((symbol, reason.clone()));
        // With the facts of the uniform ABI (vectors and return) declared above.
        b.finish(module);
    }
}
/// The getter of a `late` field with an initializer of the source SDK, or its
/// rejection: `build` lowers the getter in a separate module; if it has a
/// diagnostic (or the lowering panics), the function that
/// warns at runtime takes its place.
pub fn lower_late_getter_or_reject(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    build: impl Fn(&mut Module),
) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        build(&mut m);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    let symbol = super::late_field_getter_symbol(ctx, vid);
    let reason = match r {
        Ok(mut m) if m.errors.is_empty() => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
            return;
        }
        Ok(m) => m.errors[0].strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(&m.errors[0]).to_string(),
        Err(_) => "pânico do lowering".to_string(),
    };
    let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let mut b = FnBuilder::new(ctx, unit, symbol.clone(), name_str, Type::Ref);
    b.add_param("this".to_string(), Type::Ref);
    let t = b.emit(Instruction::Const(Constant::String(format!("{symbol} ({reason})"))), Type::Ref);
    b.emit(
        Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
        Type::Void,
    );
    b.terminate(Terminator::Unreachable);
    register_rejection_facts(ctx, module, &b.func);
    module.functions.push(b.func);
    module.rejected.push((symbol, reason));
}

/// The lazy getter of a source SDK global (or its rejection) and the
/// setter `<getter>$set`, which another module calls to write the global.
/// The program global outside the closed world (C7, `native_world.rs`): the
/// getter throws (`FnBuilder::pruned_body`); the setter is the usual one.
pub fn lower_pruned_global(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::global_symbol(ctx, vid);
    let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
    b.pruned_body();
    b.finish(module);
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), name_str, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    // The value arrives in the representation of the Dart type of the global: scalar if not Ref.
    if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
        s.dart_scalar_parameters.entry(s.func.symbol.clone()).or_default().insert(v);
    }
    s.store_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finish(module);
}

pub fn lower_global_or_reject(
    ctx: &Context,
    module: &mut Module,
    vid: VariableId,
    unit: dartforge_elements::model::UnitId,
    repr: Type,
) {
    let name_str = ctx.symbol_name(ctx.program.variables[vid.0 as usize].name).to_string();
    let getter = super::global_symbol(ctx, vid);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut m = Module::new();
        m.sdk_mode = true;
        let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
        b.lower_global_getter(vid, repr);
        b.finish(&mut m);
        if m.errors.is_empty() {
            let problems = super::verifier::verify(&m);
            m.errors.extend(problems);
        }
        m
    }));
    match r {
        Ok(mut m) if m.errors.is_empty() => {
            module.functions.extend(std::mem::take(&mut m.functions));
        super::join_arc_facts(module, &mut m);
            module.globals.extend(m.globals);
        }
        r => {
            let reason = match r {
                Ok(m) => m.errors[0].strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(&m.errors[0]).to_string(),
                Err(_) => "pânico do lowering".to_string(),
            };
            let reason = reason.rsplit_once(" (").map_or(reason.as_str(), |(a, _)| a).to_string();
            let mut b = FnBuilder::new(ctx, unit, getter.clone(), name_str.clone(), repr);
            let t = b.emit(Instruction::Const(Constant::String(format!("{getter} ({reason})"))), Type::Ref);
            b.emit(
                Instruction::CallRuntime { name: "dartforge_membro_recusado".to_string(), args: vec![(t, Type::Ref)], ret_ty: Type::Void },
                Type::Void,
            );
            b.terminate(Terminator::Unreachable);
            register_rejection_facts(ctx, module, &b.func);
            module.functions.push(b.func);
            module.rejected.push((getter.clone(), reason));
        }
    }
    let mut s = FnBuilder::new(ctx, unit, format!("{getter}$set"), name_str, Type::Void);
    let v = s.add_param("v".to_string(), repr);
    // The value arrives in the representation of the Dart type of the global: scalar if not Ref.
    if ctx.arc_memory && matches!(repr, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
        s.dart_scalar_parameters.entry(s.func.symbol.clone()).or_default().insert(v);
    }
    s.store_global(vid, Operand::Val(v), Span { start: 0, end: 0 });
    s.terminate(Terminator::Return(None));
    s.finish(module);
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `{}` with the source SDK: a new source `_Map` (or `_Set`), the
    /// default `LinkedHashMap`/`LinkedHashSet` that the literal builds.
    pub fn source_empty_collection(&mut self, map: bool) -> Operand {
        let name_str = if map { "_Map" } else { "_Set" };
        let span = Span { start: 0, end: 0 };
        let Some(cid) = self.ctx.sdk_class("_compact_hash", name_str) else {
            return self.unsupported(&format!("literal de coleção sem `{name_str}`"), span);
        };
        let empty = self.ctx.interner.lookup("");
        let Some(ctor) = empty.and_then(|v| self.ctx.program.classes[cid.0 as usize].constructors.get(&v).copied()) else {
            return self.unsupported(&format!("`{name_str}()` ausente"), span);
        };
        self.instantiate_evaluated(ctor, &[], span)
    }

    /// `for (x in source) f(x)` through the `Iterator` protocol (§17.7.3):
    /// `iterator`, `moveNext()` and `current`, through the selectors.
    pub fn iterate_source(&mut self, source: Operand, f: &mut dyn FnMut(&mut Self, Operand)) {
        let it = self.call_by_selector(source, "g:iterator".to_string(), &[]);
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let ok = self.call_by_selector(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coerce(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
        self.set_block(fn_body);
        self.emit_safepoint();
        let x = self.call_by_selector(it, "g:current".to_string(), &[]);
        f(self, x);
        self.terminate(Terminator::Branch(head));
        self.set_block(end);
    }

    /// Binds the current element of a `for-in` to its target (a declared
    /// variable, an existing local or a pattern), in the representation of the target.
    pub fn bind_for_in_target(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        x: Operand,
        iterable: dartforge_frontend::ast::ExprId,
        span: Span,
    ) {
        use dartforge_frontend::ast::{ExprKind, ForInTarget};
        // A `dynamic` element in a typed target: the implicit cast of each
        // element (the CFE puts it in the assignment to the loop variable), with the
        // message of the cast, before converting to the representation.
        let dynamic_element = match self.ctx.get_type(self.unit_id, iterable) {
            Some(t) if t == self.ctx.core.dynamic_ => true,
            Some(t) => self.element_type(t) == Some(self.ctx.core.dynamic_),
            None => false,
        };
        match target {
            ForInTarget::Declared { name, ty: annotated, .. } => {
                if dynamic_element && let Some(tid) = annotated {
                    self.check_type_or_throw(ast.ty(*tid), x.clone(), super::rti::CastContext::Implicit);
                    if self.is_terminated() {
                        return;
                    }
                }
                let ty = self.local_repr_of(name.span.start as usize);
                let x = self.coerce(x, ty);
                self.declare_variable(name.sym, name.span.start as usize, ty, x);
            }
            ForInTarget::Expression(e) => {
                if let ExprKind::Identifier(id) = &ast.expr(*e).kind {
                    if dynamic_element
                        && let Some(t) = self.ctx.get_type(self.unit_id, *e)
                        && t != self.ctx.core.dynamic_
                        && t != self.ctx.core.object_nullable
                    {
                        let rti = self.type_rti(t);
                        self.cast_rti_in(x.clone(), rti, super::rti::CastContext::Implicit);
                        if self.is_terminated() {
                            return;
                        }
                    }
                    let ty = self.lookup_local(id.sym).map_or(Type::Ref, |l| l.ty);
                    let x = self.coerce(x, ty);
                    self.store_local(id.sym, x);
                } else {
                    self.unsupported("alvo de for-in", span);
                }
            }
            ForInTarget::Pattern { pattern, .. } => {
                self.match_irrefutable(ast, *pattern, x, super::patterns::Linkage::Declare, iterable);
            }
        }
    }

    /// `for (target in iterable) body` with the source SDK: the `Iterator`
    /// protocol, with `break`/`continue` (and the labels) like the usual
    /// `for-in`.
    pub fn lower_source_for_in(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
    ) {
        let source = self.lower_expr(ast, iterable);
        // The labels of this loop (both paths below use them; a loop
        // nested in the body cannot see them as its own).
        let labels = std::mem::take(&mut self.pending_labels);
        // `List<E>`: the runtime list walked by index, with the
        // length check of `ListIterator`; another class that
        // implements `List` goes through its `Iterator` (below).
        let mut direct_end = None;
        if let Some(super::typed_items::Indexable::Core { write_op }) = self.indexable(self.ctx.get_type(self.unit_id, iterable)) {
            let source = self.coerce(source.clone(), Type::Ref);
            if let Some(end) = self.list_for_in(ast, target, iterable, body, span, source, write_op.unwrap_or(Type::Ref), &labels) {
                direct_end = Some(end);
            }
        }
        let it = self.call_by_selector(source, "g:iterator".to_string(), &[]);
        let head = self.new_block();
        let fn_body = self.new_block();
        let end = self.new_block();
        for &r in &labels {
            self.labeled_break_targets.insert(r, end);
            self.labeled_continue_targets.insert(r, head);
        }
        self.terminate(Terminator::Branch(head));
        self.set_block(head);
        let ok = self.call_by_selector(it.clone(), "c:moveNext".to_string(), &[]);
        let ok = self.coerce(ok, Type::I1);
        self.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
        self.set_block(fn_body);
        self.emit_safepoint();
        self.break_targets.push(end);
        self.continue_targets.push(head);
        self.open_scope();
        let x = self.call_by_selector(it, "g:current".to_string(), &[]);
        self.bind_for_in_target(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.close_scope();
        self.terminate(Terminator::Branch(head));
        self.break_targets.pop();
        self.continue_targets.pop();
        for &r in &labels {
            self.labeled_break_targets.remove(&r);
            self.labeled_continue_targets.remove(&r);
        }
        self.set_block(end);
        // The exit of the loop through the `Iterator` meets that of the direct loop.
        if let Some(after) = direct_end {
            self.terminate(Terminator::Branch(after));
            self.set_block(after);
        }
    }

    /// The `for-in` over a runtime list by index. Leaves the current
    /// block on the path of the `Iterator` (a list of another class) and returns the
    /// block after the loop, where both paths arrive.
    ///
    /// It is what `ListIterator` (and `_FixedSizeArrayIterator`, which never sees
    /// the length change) does: on each round trip, a length different from the
    /// initial one throws `ConcurrentModificationError(list)`; an index at the end
    /// terminates; otherwise the element `list[i]`.
    #[allow(clippy::too_many_arguments)]
    fn list_for_in(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        target: &dartforge_frontend::ast::ForInTarget,
        iterable: dartforge_frontend::ast::ExprId,
        body: dartforge_frontend::ast::StmtId,
        span: Span,
        list: Operand,
        repr: Type,
        labels: &[dartforge_intern::SymbolId],
    ) -> Option<BlockId> {
        let class = self.ctx.sdk_class("core", "ConcurrentModificationError")?;
        let empty = self.ctx.interner.lookup("")?;
        let ctor = *self.ctx.program.classes[class.0 as usize].constructors.get(&empty)?;
        // A core list by class (`@df.classe`, fixed cids 8–10,
        // docs/NATIVO-ESPACO-UNIFICADO.md §2.4): `_List`, `_ImmutableList` or
        // `_GrowableList`; the length inline (`@df.lista_len`).
        let cid = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(list.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let since = self.emit(
            Instruction::Sub(cid, Operand::Constant(Constant::Int(i64::from(dartforge_runtime::layout::cid::LIST)))),
            Type::I64,
        );
        let is_list = self.emit(Instruction::ICmp(ICmpOp::Ult, since, Operand::Constant(Constant::Int(3))), Type::I1);
        let direct = self.new_block();
        let iterator = self.new_block();
        let after = self.new_block();
        self.terminate(Terminator::CondBranch { cond: is_list, then_block: direct, else_block: iterator });

        self.set_block(direct);
        let n0 = self.list_len_inline(list.clone());
        let index = self.emit(Instruction::Alloca(Type::I64), Type::Ptr);
        self.emit(Instruction::Store { ptr: index.clone(), val: Operand::Constant(Constant::Int(0)) }, Type::Void);
        let head = self.new_block();
        let changed = self.new_block();
        let checks = self.new_block();
        let fn_body = self.new_block();
        self.terminate(Terminator::Branch(head));

        self.set_block(head);
        // Inside the loop, the length read from the block on each round trip
        // (`@df.lista_len`, the same offset in the three classes).
        let n = self.list_len_inline(list.clone());
        let equal = self.emit(Instruction::ICmp(ICmpOp::Eq, n.clone(), n0), Type::I1);
        self.terminate(Terminator::CondBranch { cond: equal, then_block: checks, else_block: changed });

        self.set_block(changed);
        let error = self.instantiate_evaluated(ctor, &[(None, list.clone())], span);
        if !self.is_terminated() {
            self.emit_throw_op(error);
        }
        if !self.is_terminated() {
            self.terminate(Terminator::Unreachable);
        }

        self.set_block(checks);
        let i = self.emit(Instruction::Load { ptr: index.clone(), ty: Type::I64 }, Type::I64);
        let inside = self.emit(Instruction::ICmp(ICmpOp::Slt, i.clone(), n), Type::I1);
        self.terminate(Terminator::CondBranch { cond: inside, then_block: fn_body, else_block: after });

        self.set_block(fn_body);
        self.emit_safepoint();
        let x = self.read_list_element(&list, &i, repr);
        let next = self.emit(Instruction::Add(i, Operand::Constant(Constant::Int(1))), Type::I64);
        self.emit(Instruction::Store { ptr: index, val: next }, Type::Void);
        for &r in labels {
            self.labeled_break_targets.insert(r, after);
            self.labeled_continue_targets.insert(r, head);
        }
        self.break_targets.push(after);
        self.continue_targets.push(head);
        self.open_scope();
        self.bind_for_in_target(ast, target, x, iterable, span);
        self.lower_stmt(ast, body);
        self.close_scope();
        if !self.is_terminated() {
            self.terminate(Terminator::Branch(head));
        }
        self.break_targets.pop();
        self.continue_targets.pop();
        for r in labels {
            self.labeled_break_targets.remove(r);
            self.labeled_continue_targets.remove(r);
        }
        self.set_block(iterator);
        Some(after)
    }
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// `v is <name>` for a class of `dart:core` (`List`, `Map`,
    /// `Record`), through the class graph of the source SDK; null is not.
    pub fn is_core_instance(&mut self, v: Operand, name_str: &str) -> Operand {
        let zero = Operand::Constant(Constant::Int(0));
        let Some(id) = self.ctx.sdk_class("core", name_str).and_then(|c| self.ctx.class_id_of(c)) else {
            return Operand::Constant(Constant::Bool(false));
        };
        let cls = self.emit(
            Instruction::CallRuntime { name: "dartforge_value_class".to_string(), args: vec![(v.clone(), Type::Ref)], ret_ty: Type::I64 },
            Type::I64,
        );
        let sub = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_is_subclass".to_string(),
                args: vec![(cls, Type::I64), (Operand::Constant(Constant::Int(i64::from(id))), Type::I64)],
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let sub = self.emit(Instruction::ICmp(ICmpOp::Ne, sub, zero.clone()), Type::I1);
        let non_null = self.emit(Instruction::ICmp(ICmpOp::Ne, v, zero.clone()), Type::I1);
        let r = self.emit(Instruction::And(sub, non_null), Type::I64);
        self.emit(Instruction::ICmp(ICmpOp::Ne, r, zero), Type::I1)
    }

    /// Continues if `cond`, otherwise branches to `failure`.
    fn require_source(&mut self, cond: Operand, failure: BlockId) {
        let cond = self.to_bool(cond);
        let ok = self.new_block();
        self.terminate(Terminator::CondBranch { cond, then_block: ok, else_block: failure });
        self.set_block(ok);
    }

    /// List pattern with the source SDK (§19.4.6 "List pattern"): the value is
    /// a `List`, and `length`, `[]` and `sublist` go through the selector.
    #[allow(clippy::too_many_arguments)]
    pub fn match_source_list(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        elements: &[dartforge_frontend::ast::ListPatternElement],
        value: Operand,
        failure: BlockId,
        linkage: super::patterns::Linkage,
        linked: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origin: dartforge_frontend::ast::ExprId,
    ) {
        use dartforge_frontend::ast::ListPatternElement;
        let v = self.coerce(value, Type::Ref);
        let e = self.is_core_instance(v.clone(), "List");
        self.require_source(e, failure);
        let len = self.call_by_selector(v.clone(), "g:length".to_string(), &[]);
        let len = self.coerce(len, Type::I64);
        let rest = elements.iter().position(|e| matches!(e, ListPatternElement::Rest(_)));
        let n_fixed = elements.len() - usize::from(rest.is_some());
        let ok = self.emit(
            Instruction::ICmp(
                if rest.is_some() { ICmpOp::Sge } else { ICmpOp::Eq },
                len.clone(),
                Operand::Constant(Constant::Int(n_fixed as i64)),
            ),
            Type::I1,
        );
        self.require_source(ok, failure);
        for (i, el) in elements.iter().enumerate() {
            match (el, rest) {
                (ListPatternElement::Pattern(sp), r) => {
                    let idx = match r {
                        Some(r) if i > r => {
                            let after = (elements.len() - i) as i64;
                            self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(after))), Type::I64)
                        }
                        _ => Operand::Constant(Constant::Int(i as i64)),
                    };
                    let x = self.call_by_selector(v.clone(), "c:[]".to_string(), &[(None, idx)]);
                    self.match_pattern(ast, *sp, x, failure, linkage, linked, origin);
                }
                (ListPatternElement::Rest(Some(sp)), _) => {
                    let after = (elements.len() - i - 1) as i64;
                    let end = self.emit(Instruction::Sub(len.clone(), Operand::Constant(Constant::Int(after))), Type::I64);
                    let sub = self.call_by_selector(
                        v.clone(),
                        "c:sublist".to_string(),
                        &[(None, Operand::Constant(Constant::Int(i as i64))), (None, end)],
                    );
                    self.match_pattern(ast, *sp, sub, failure, linkage, linked, origin);
                }
                (ListPatternElement::Rest(None), _) => {}
            }
        }
    }

    /// Map pattern with the source SDK (§19.4.7 "Map pattern"): the value is
    /// a `Map`, and each key exists (`containsKey`) and matches (`[]`).
    #[allow(clippy::too_many_arguments)]
    pub fn match_source_map(
        &mut self,
        ast: &dartforge_frontend::ast::Ast,
        entries: &[dartforge_frontend::ast::MapPatternEntry],
        value: Operand,
        failure: BlockId,
        linkage: super::patterns::Linkage,
        linked: &mut std::collections::HashSet<dartforge_intern::SymbolId>,
        origin: dartforge_frontend::ast::ExprId,
    ) {
        let v = self.coerce(value, Type::Ref);
        let e = self.is_core_instance(v.clone(), "Map");
        self.require_source(e, failure);
        for en in entries {
            let k = self.lower_expr(ast, en.key);
            let has = self.call_by_selector(v.clone(), "c:containsKey".to_string(), &[(None, k.clone())]);
            let has = self.coerce(has, Type::I1);
            self.require_source(has, failure);
            let x = self.call_by_selector(v.clone(), "c:[]".to_string(), &[(None, k)]);
            self.match_pattern(ast, en.value, x, failure, linkage, linked, origin);
        }
    }
}
/// The record shapes with a named field of the program (`records.rs`) with the
/// source SDK: each one gets the method table — `toString` and `==`
/// (the generated ones), and the members of `Object` — with the adapters.
pub fn record_shape_tables(ctx: &Context, module: &mut Module) {
    let Some(u) = ctx.entry_lib.and_then(|l| ctx.program.library(l).units.first().copied()) else { return };
    let object = ctx.core.object_class.map(|c| method_table(ctx, c)).unwrap_or_default();
    for k in 0..ctx.record_shapes.len() {
        let id = crate::context::SHAPE_BASE_ID + k as u32;
        let base = format!("df.$registro.{k}");
        // toString$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.toString$c"), "toString".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param_vector(DartEntryVector::Arguments);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.toString"), args: vec![recv], ret_ty: Type::Ref },
            Type::Ref,
        );
        b.terminate(Terminator::Return(Some(r)));
        b.finish(module);
        // ==$c
        let mut b = FnBuilder::new(ctx, u, format!("{base}.$3d$3d$c"), "==".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        let args = Operand::Val(b.add_param_vector(DartEntryVector::Arguments));
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let other = b.emit(Instruction::LoadIndexed { base: args, index: Operand::Constant(Constant::Int(0)) }, Type::Ref);
        let r = b.emit_call_with_check(
            Instruction::CallStatic {
                symbol: super::records::EQUAL_SYMBOL.to_string(),
                args: vec![recv, other],
                ret_ty: Type::I1,
            },
            Type::I1,
        );
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finish(module);
        // hashCode$g: the hash follows the structural equality of the record.
        let mut b = FnBuilder::new(ctx, u, format!("{base}.hashCode$g"), "hashCode".to_string(), Type::Ref);
        let recv = Operand::Val(b.add_param("this".to_string(), Type::Ref));
        b.add_param_vector(DartEntryVector::Arguments);
        b.add_param_vector(DartEntryVector::Descriptor);
        b.declare_arc_uniform_return();
        let r = b.emit_call_with_check(
            Instruction::CallStatic { symbol: format!("{base}.hashCode"), args: vec![recv], ret_ty: Type::I64 },
            Type::I64,
        );
        let r = b.coerce(r, Type::Ref);
        b.terminate(Terminator::Return(Some(r)));
        b.finish(module);
        let mut table: Vec<(String, String)> = vec![
            ("c:toString".to_string(), format!("{base}.toString$c")),
            ("c:==".to_string(), format!("{base}.$3d$3d$c")),
            ("g:hashCode".to_string(), format!("{base}.hashCode$g")),
            ("c:hashCode".to_string(), format!("{base}.hashCode$g")),
        ];
        for (s, f) in &object {
            if !table.iter().any(|(x, _)| x == s) {
                table.push((s.clone(), f.clone()));
            }
        }
        module.method_tables.push((id, format!("df.mt.$registro.{k}"), table));
    }
}

#[cfg(test)]
mod arc_tests {
    use super::*;
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    #[test]
    fn separately_lowered_function_keeps_arc_facts() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "class C { final String s; C(this.s); }\nString de(C c, int n) => n > 0 ? c.s : 'x';\nvoid main() { print(de(C('a'), 1)); }\n").unwrap();
            let sdk = crate::sdk_modulo::load_native_sdk(crate::sdk_tests()).unwrap();
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
            let fid = program.functions.iter().position(|f| interner.resolve(f.name) == "de").unwrap();
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let mut m = Module::new();
                lower_function_or_reject(&ctx, &mut m, fid);
                assert!(m.rejected.is_empty(), "{:?}", m.rejected);
                let symbol = super::super::symbol_of(&ctx, fid);
                assert_eq!(m.dart_ref_returns.contains(&symbol), arc);
                assert_eq!(m.dart_scalar_parameters.get(&symbol).is_some_and(|s| s.len() == 1), arc);
            }
        }).unwrap().join().unwrap();
    }
}
