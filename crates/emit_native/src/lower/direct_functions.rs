//! Local functions that do not escape, called directly (N17,
//! docs/NATIVO-PLANO.md §9).
//!
//! **The problem.** A local function (`void skipWS() { … }` inside a
//! method) was always a value: on each execution of the declaration, an environment
//! with the captures, the closure and — because the name counts as assigned —
//! a cell for the name; each call went through the uniform entry of the
//! closure. `_HeaderValue._parse` of `dart:_http` declares six (eight with
//! those inside `parseParameters`) per call, and the HTTP server made
//! 103 closures and 103 environments per request.
//!
//! **The rule.** What the VM does in the flow graph (closures that do not
//! escape are called statically and, when possible, inlined) and what the
//! CFE guarantees: a local function whose name only appears as a call target
//! `f(…)` — never read as a value, passed, stored or returned — and only from
//! inside the function that declares it or from other direct functions cannot be
//! called after the outer function returns. The analysis is in
//! `capture.rs` (`analyze_with`): the references, the shape (no type
//! parameters of its own, synchronous, only required positionals, not reading an
//! outer `late`) and the fixpoint, because a function that stops being
//! direct becomes a closure and what it calls also starts escaping through it.
//! A generic local function stays a closure: the tuple of its type arguments
//! (`$types`) comes from the uniform entry.
//!
//! **The code.** The direct function becomes a module function,
//! `<outer>$<name>$d<fingerprint>`, with the parameters
//! `[this] captures… [type tuple of the outer function] parameters…`, and the call is a
//! static `call` — no closure, environment nor cell for the name. Each
//! capture goes in one of three ways, decided at the declaration:
//!
//! * **value** — the variable is never assigned (the parameter, the `final`, the
//!   `var` only initialized): the value read at the time of the call;
//! * **address** — assigned, and no escaping function captures it: the
//!   address of the declarer's `alloca`, and the function reads and writes through it (the
//!   `index` of `_parse`). The direct function only runs while the frame of its
//!   declarer exists. The `alloca` of a `Ref` whose address leaves this way lives
//!   in its own slot of the roots frame (`llvm/mod.rs`,
//!   `frame_allocas`): what the direct function writes stays rooted, without a
//!   cell (the `listenerValueOrError` and the `source` of
//!   `_Future._propagateToListeners`);
//! * **cell** — also captured by an escaping closure: the handle of the
//!   usual cell.
//!
//! A direct function that calls another one declared outside it also receives its
//! captures (and forwards them): the call never looks a capture up by
//! name, which an inner block may have shadowed.

use super::fn_builder::FnBuilder;
use super::locals::{Local, Mode};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody, FunctionId};
use dartforge_intern::SymbolId;

/// How a capture reaches the direct function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    Value,
    Address,
    Cell,
}

/// A capture of a direct function: the name, how it is passed, the representation
/// of the variable and where it lives in the builder that registered the function.
#[derive(Clone, Debug)]
pub struct Capture {
    pub sym: SymbolId,
    pub passing: Passing,
    pub ty: Type,
    pub local: Local,
}

/// A direct function visible in the current builder.
#[derive(Clone, Debug)]
pub struct Direct {
    pub symbol: String,
    pub with_this: bool,
    pub with_tuple: bool,
    pub captures: Vec<Capture>,
    pub reprs: Vec<Type>,
    pub ret: Type,
}

/// The identity of where a variable lives (to join the captures of
/// different functions that are the same variable).
fn identity(l: &Local) -> String {
    format!("{:?}", l.mode)
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Declaration `f` is a direct local function (`capture.rs`).
    pub fn direct_local_function(&self, fid: FunctionId) -> bool {
        self.allowed_directs.contains(&fid)
    }

    /// How variable `l` is passed to a direct function declared here.
    fn passing_of(&self, l: &Local) -> Passing {
        match &l.mode {
            Mode::Cell(_) | Mode::Environment { cell: true, .. } => Passing::Cell,
            Mode::Memory(_)
                if l.offset
                    .is_some_and(|o| self.assigned.contains(&o) || self.pointers.contains(&o)) =>
            {
                Passing::Address
            }
            _ => Passing::Value,
        }
    }

    /// A local, direct `R f(params) { … }`: lowers the body as a module function
    /// and registers the name (without a value: it is never read).
    pub fn declare_direct_function(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(name) = f.name else { return };
        // The name, visible in the scope (the call finds it by offset); the value
        // is never read.
        self.declare_variable(
            name.sym,
            name.span.start,
            Type::Ref,
            Operand::Constant(Constant::Null),
        );

        // The captures: the free variables visible here, and those of the direct
        // functions the body calls (forwarded).
        let (free_set, _) = super::capture::free_set(self.ctx, self.unit_id, ast, fid);
        let mut captures: Vec<Capture> = Vec::new();
        let mut calls: Vec<(SymbolId, usize)> = Vec::new();
        for s in free_set {
            let Some(l) = self.lookup_local(s) else {
                continue;
            };
            if let Some(o) = l.offset
                && (self.direct_functions.contains_key(&o) || o == name.span.start)
            {
                calls.push((s, o));
                continue;
            }
            let passing = self.passing_of(&l);
            if !captures.iter().any(|c| identity(&c.local) == identity(&l)) {
                captures.push(Capture {
                    sym: s,
                    passing,
                    ty: l.ty,
                    local: l,
                });
            }
        }
        for (_, o) in &calls {
            if let Some(d) = self.direct_functions.get(o) {
                for c in d.captures.clone() {
                    if !captures
                        .iter()
                        .any(|x| identity(&x.local) == identity(&c.local))
                    {
                        captures.push(c);
                    }
                }
            }
        }
        let with_this = self.this_param.is_some();
        let with_tuple = self.type_tuple.is_some();
        let params = f.parameters.as_deref().unwrap_or(&[]);
        let ty = self.literal_function_type(ast, fid, None);
        let (reprs, ret) = match ty.and_then(|t| self.type_abi(t)) {
            Some((ps, r, _)) if ps.len() == params.len() => (ps, r),
            _ => (vec![Type::Ref; params.len()], Type::Ref),
        };
        // The name carries the fingerprint of the signature (like the body of
        // a closure, `closures.rs`): on a JIT reload, a caller of
        // a version with other captures calls another symbol.
        let mut shape = format!("{with_this}|{with_tuple}|");
        for c in &captures {
            shape.push_str(&format!(
                "{}:{:?}:{:?};",
                self.ctx.symbol_name(c.sym),
                c.passing,
                c.ty
            ));
        }
        shape.push_str(&format!("{reprs:?}->{ret:?}"));
        let base = format!(
            "{}${}",
            self.func.symbol,
            super::sanitize_symbol(self.ctx.symbol_name(name.sym))
        );
        let mut symbol = base.clone();
        let mut k = 1;
        while !self.local_names.insert(symbol.clone()) {
            symbol = format!("{base}${k}");
            k += 1;
        }
        let symbol = format!(
            "{symbol}$d{:08x}",
            (super::closures::hash_name(&shape) as u64) as u32
        );
        let direct = Direct {
            symbol: symbol.clone(),
            with_this,
            with_tuple,
            captures,
            reprs,
            ret,
        };
        self.direct_functions
            .insert(name.span.start, direct.clone());

        // --- corpo ---------------------------------------------------------
        let readable = self.ctx.symbol_name(name.sym).to_string();
        let mut b = FnBuilder::new(self.ctx, self.unit_id, symbol, readable, ret);
        b.open_scope();
        if with_this {
            let t = b.add_param("this".to_string(), Type::Ref);
            b.this_param = Some(Operand::Val(t));
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        // Each capture, bound in the body; the function forwards the same place to the
        // direct functions it calls.
        let mut linked_set: Vec<(String, Local)> = Vec::new();
        for c in &direct.captures {
            let name_p = self.ctx.symbol_name(c.sym).to_string();
            let local = match c.passing {
                Passing::Value => {
                    let v = b.add_param_dart(name_p, c.ty);
                    Local {
                        mode: Mode::Value(Operand::Val(v)),
                        ty: c.ty,
                        static_type: c.local.static_type,
                        finalizable: c.local.finalizable,
                        scope: 0,
                        offset: c.local.offset,
                        late: None,
                    }
                }
                Passing::Address => {
                    let v = b.add_param(name_p, Type::Ptr);
                    if let Some(o) = c.local.offset {
                        b.pointers.insert(o);
                    }
                    Local {
                        mode: Mode::Memory(Operand::Val(v)),
                        ty: c.ty,
                        static_type: c.local.static_type,
                        finalizable: c.local.finalizable,
                        scope: 0,
                        offset: c.local.offset,
                        late: None,
                    }
                }
                Passing::Cell => {
                    let v = b.add_param(name_p, Type::Ref);
                    let ptr = b.entry_alloca(Type::Ref);
                    b.emit(
                        Instruction::Store {
                            ptr: ptr.clone(),
                            val: Operand::Val(v),
                        },
                        Type::Void,
                    );
                    Local {
                        mode: Mode::Cell(ptr),
                        ty: c.ty,
                        static_type: c.local.static_type,
                        finalizable: c.local.finalizable,
                        scope: 0,
                        offset: c.local.offset,
                        late: None,
                    }
                }
            };
            linked_set.push((identity(&c.local), local.clone()));
            b.bind_local_as(c.sym, local);
        }
        if with_tuple {
            let t = b.add_param_rti("$outer_types".to_string());
            b.type_tuple = Some(Operand::Val(t));
        }
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.class_by_tuple {
            b.enclosing_class = self.enclosing_class;
        }
        // The direct functions the body calls (itself, in recursion), with the
        // captures replaced by the bindings from there.
        for (s, o) in &calls {
            let Some(d) = self.direct_functions.get(o) else {
                continue;
            };
            let mut d = d.clone();
            for c in d.captures.iter_mut() {
                if let Some((_, l)) = linked_set.iter().find(|(id, _)| *id == identity(&c.local)) {
                    c.local = l.clone();
                }
            }
            b.declare_variable(*s, *o, Type::Ref, Operand::Constant(Constant::Null));
            b.direct_functions.insert(*o, d);
        }
        b.prepare_captures(
            ast,
            super::capture::Root {
                parameters: params,
                body: Some(&f.body),
                initializers: &[],
            },
        );
        b.open_scope();
        for (i, p) in params.iter().enumerate() {
            let name_p = p.name.map_or_else(
                || "arg".to_string(),
                |n| self.ctx.symbol_name(n.sym).to_string(),
            );
            let v = b.add_param_dart(name_p, direct.reprs[i]);
            if let Some(n) = p.name {
                b.declare_variable(n.sym, n.span.start, direct.reprs[i], Operand::Val(v));
            }
        }
        match &f.body {
            FunctionBody::Block(s) => b.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                let r = b.lower_expr(ast, *e);
                if !b.is_terminated() {
                    let r = b.coerce(r, ret);
                    b.terminate(Terminator::Return(Some(r)));
                }
            }
            _ => {
                b.unsupported("direct local function without a body", span);
            }
        }
        if self.ctx.arc_memory {
            self.arc_direct_local_functions
                .insert(b.func.symbol.clone());
            // Dart body: the Ref return is Owned, like that of a function.
            if b.func.return_ty == Type::Ref {
                b.dart_ref_returns.insert(b.func.symbol.clone());
            }
        }
        b.record_local_inline_policy(ast, fid);
        self.absorb(b);
    }

    /// The direct function registered in local `l` (the name of a direct local
    /// function), if it is one.
    pub fn local_direct(&self, l: &Local) -> Option<Direct> {
        self.direct_functions.get(&l.offset?).cloned()
    }

    /// `f(evaluated)` of a direct function: a static `call` with the
    /// captures read now (the value, the address or the cell).
    pub fn call_direct_fn(
        &mut self,
        d: &Direct,
        evaluated: &[super::members::Evaluated],
    ) -> Operand {
        let mut args = Vec::with_capacity(d.captures.len() + evaluated.len() + 2);
        if d.with_this {
            let t = self
                .this_param
                .clone()
                .unwrap_or(Operand::Constant(Constant::Null));
            args.push(self.coerce(t, Type::Ref));
        }
        for c in &d.captures {
            let v = match c.passing {
                Passing::Value => {
                    let v = self.read_local(&c.local);
                    self.coerce(v, c.ty)
                }
                Passing::Address => match &c.local.mode {
                    Mode::Memory(ptr) => ptr.clone(),
                    _ => unreachable!("capture by address outside memory"),
                },
                Passing::Cell => self
                    .local_cell(&c.local)
                    .unwrap_or(Operand::Constant(Constant::Null)),
            };
            args.push(v);
        }
        if d.with_tuple {
            args.push(
                self.type_tuple
                    .clone()
                    .unwrap_or(Operand::Constant(Constant::Int(0))),
            );
        }
        for (i, (_, v)) in evaluated.iter().enumerate() {
            let ty = d.reprs.get(i).copied().unwrap_or(Type::Ref);
            args.push(self.coerce(v.clone(), ty));
        }
        self.emit_call_with_check(
            Instruction::CallStatic {
                symbol: d.symbol.clone(),
                args,
                ret_ty: d.ret,
            },
            d.ret,
        )
    }
}
