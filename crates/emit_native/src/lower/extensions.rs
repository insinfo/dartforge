//! Extension members (P4): the receiver becomes the first parameter.
//!
//! The specification (§13 "Extensions") resolves `r.m(…)` statically to the
//! member of the applicable extension; the call is direct, without dispatch, with `r`
//! as `this`. Here the instance member of an extension is a function
//! `df.<library>.ext:<E>.<m>(this, …)` with `this` in the representation of the
//! `on` type (an `int` goes as `i64`).

use super::fn_builder::FnBuilder;
use super::members::Evaluated;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ExtensionId, FunctionKind};
use dartforge_frontend::ast::{self, ParameterKind};
use dartforge_types::table::TypeId;

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Representation of the receiver (`this`) of an extension.
    pub fn extension_receiver_repr(&self, e: ExtensionId) -> Type {
        let on = self.ctx.outline.extensions[e.0 as usize].on;
        self.repr(on)
    }

    /// `r.m(args)` resolved to member `fid` of an extension. `receiver`
    /// is the static type of `r` and `call` the expression with the arguments
    /// (the two arm the tuple of a generic extension or member).
    pub fn call_extension(
        &mut self,
        recv: Operand,
        fid: usize,
        evaluated: &[Evaluated],
        receiver: Option<TypeId>,
        call: Option<(ast::ExprId, &ast::Arguments)>,
        span: Span,
    ) -> Operand {
        // The tuple of this call does not leak into the outer one (an extension
        // getter may be an argument of an already armed generic call).
        let saved = self.armed_tuple.take();
        let is_call = call.is_some();
        self.arm_tuple_with_receiver(fid, receiver, call);
        let r = self.call_armed_extension(recv, fid, evaluated, is_call, span);
        self.armed_tuple = saved;
        r
    }

    /// `is_call`: the expression is a call (`x.g()`); for a getter, the value
    /// it returns is called with the arguments (even with none).
    fn call_armed_extension(&mut self, recv: Operand, fid: usize, evaluated: &[Evaluated], is_call: bool, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        if !super::user_function(self.ctx, fid) || !super::members::has_body(self.ctx, fid) {
            return self.unsupported("membro de extensão do SDK", span);
        }
        if f.static_ {
            let args = self.match_args(fid, evaluated);
            return self.call_direct(fid, None, args);
        }
        // The receiver in the representation of the `on` (extension) or of the
        // representation (extension type, `extension_types.rs`).
        let Some(r) = self.static_this_repr(fid) else {
            return self.unsupported("membro de extensão sem extensão", span);
        };
        let recv = self.coerce(recv, r);
        if f.kind == FunctionKind::Getter && (is_call || !evaluated.is_empty()) {
            let v = self.call_direct(fid, Some(recv), Vec::new());
            return self.call_function_value(v, evaluated);
        }
        let args = self.match_args(fid, evaluated);
        self.call_direct(fid, Some(recv), args)
    }

    /// The extension instance operator that inference resolved for the
    /// expression (`p + 1`, `p[i]`), if it is one.
    pub fn extension_operator(&self, expr: ast::ExprId) -> Option<usize> {
        // That of an extension type is also called this way.
        if let Some(f) = self.function_te(expr) {
            return Some(f);
        }
        match self.ctx.get_resolved(self.unit_id, expr) {
            Some(dartforge_types::resolved::Resolved::ExtensionMember { member, .. })
                if !self.ctx.program.functions[member.0 as usize].static_
                    && super::user_function(self.ctx, member.0 as usize) =>
            {
                Some(member.0 as usize)
            }
            _ => None,
        }
    }

    /// `r.x` resolved to an extension getter (or method tear-off).
    pub fn read_extension(&mut self, recv: Operand, fid: usize, receiver: Option<TypeId>, span: Span) -> Operand {
        let f = &self.ctx.program.functions[fid];
        // A STATIC method of the extension cited without qualification in its
        // body (`codeUnits.any(_maiuscula)`) is a static tear-off, like
        // `Ext._maiuscula`; static getters and fields are read.
        if f.static_ && f.kind == FunctionKind::Function && f.variable.is_none() {
            let el = dartforge_elements::model::Element::Function(dartforge_elements::model::FunctionElementId(fid as u32));
            return self.read_element(el, span);
        }
        if f.kind == FunctionKind::Getter || f.static_ {
            return self.call_extension(recv, fid, &[], receiver, None, span);
        }
        self.extension_tearoff(recv, fid, receiver, span)
    }

    /// Tear-off of an extension instance method (`21.double_of`,
    /// `completer.completeErrorIfPending`): a closure with the receiver in the
    /// environment, whose entry calls the member like `call_extension`. The
    /// type arguments of a generic extension come from the static type of the
    /// receiver, so there is one entry per (member, receiver type).
    fn extension_tearoff(&mut self, recv: Operand, fid: usize, receiver: Option<TypeId>, span: Span) -> Operand {
        let alvo = super::symbol_of(self.ctx, fid);
        let symbol_ent = match receiver {
            Some(t) if self.generic_extension(fid) => format!("{alvo}$tearx{}", t.0),
            _ => format!("{alvo}$tearx"),
        };
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            let env = e.closure_context(clo);
            let receiver_op = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.call_extension(receiver_op, fid, &evaluated, receiver, None, span);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let recv = self.coerce(recv, Type::Ref);
        let env = self.emit(Instruction::AllocEnv { values: vec![recv.clone()] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        // The signature with the owner's type arguments taken from the static
        // type of the receiver (the tuple of the call the entry would make).
        let tuple = if self.generic_extension(fid) {
            let saved = self.armed_tuple.take();
            self.arm_tuple_with_receiver(fid, receiver, None);
            std::mem::replace(&mut self.armed_tuple, saved)
        } else {
            None
        };
        self.define_tearoff_rti_with_tuple(c.clone(), fid, Some(recv), tuple);
        c
    }

    /// The member belongs to an extension (or extension type) with type
    /// parameters.
    fn generic_extension(&self, fid: usize) -> bool {
        self.receiver_pattern(fid).is_some_and(|(n, _)| n > 0)
    }
}
