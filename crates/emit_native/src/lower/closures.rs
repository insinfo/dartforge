//! Closures (P1): function expressions, local functions, tear-offs and the call
//! of a function value.
//!
//! **Uniform convention** (that of `dartdevc`/the VM for dynamic calls: a
//! vector of arguments and an *arguments descriptor*): every function value is
//! called through an **entry** `i64 @<body>$ent(i64 closure, ptr args, ptr
//! desc)`. `args` has the arguments (`Ref`), the positional ones first and the
//! named ones after, in the order of the descriptor; `desc` is
//! `[n_positional, n_named, hash(name)…]` with the names sorted. The
//! entry checks the arity against the signature of the function
//! (`dartforge_args_match`, which returns 0 when it does not match — then it throws
//! `NoSuchMethodError`, like the VM), fills in the defaults of the missing optional
//! ones and calls the **body** with the parameters in declaration order.
//!
//! The body of a closure is `i64 @<symbol>(i64 env, i64 p0, …)`, with all
//! the parameters and the return `Ref` (the uniform convention; the body is inferred
//! by `crates/types`, and [`FnBuilder::resolve_by_name`] covers a name without a
//! stored resolution). The environment keeps, in this order, `this`
//! (when the closure is born in an instance member) and the free variables —
//! the cell handle of those that live in one (`capture.rs`), or a copy of the value.
//!
//! The closure is `AllocClosure { code, env }`; the code is the address of the
//! uniform entry (`ptrtoint`, in the emitter), valid between the SDK module and
//! the program's. The tear-off of a top-level or
//! static function is canonical (`TearOff`: always the same handle, `identical(f, f)`);
//! that of an instance method is a new closure with the receiver in the environment, and
//! its entry calls the member with the dispatch of the receiver.

use super::capture::{self, Root};
use super::fn_builder::FnBuilder;
use super::locals::Mode;
use super::members::Evaluated;
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, AsyncModifier, ExprId, FunctionBody, FunctionId, ParameterKind};
use dartforge_intern::SymbolId;
use dartforge_types::table::TypeId;

/// Stable hash of the name of a named argument (64-bit FNV-1a over the
/// UTF-8): the same in any module, which the descriptor needs so that a
/// separately compiled library can call another.
pub fn hash_name(name: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h as i64
}

/// Where the default value of a missing optional parameter comes from.
#[derive(Clone, Copy)]
pub enum DefaultValue {
    /// No default: null.
    Absent,
    /// The expression written in the declaration of the closure.
    Expr(UnitId, ExprId),
    /// The default of parameter `i` of the function `fid` (including that of `super.x`).
    OfFunction(usize, usize),
}

/// A parameter as the uniform entry sees it.
#[derive(Clone)]
pub struct EntryParam {
    pub name: Option<String>,
    pub kind: ParameterKind,
    pub required: bool,
    pub pattern: DefaultValue,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Pre-pass of the cells (before declaring the parameters).
    /// With the direct local functions (`direct_functions.rs`), except in an
    /// `async`/generator body (`without_directs`).
    pub fn prepare_captures(&mut self, ast: &ast::Ast, root: Root) {
        // The outer `late`s visible here (captures already bound): a function
        // that reads them cannot be direct (the initialization mark does not go
        // through the parameter).
        let outside_lates: std::collections::HashSet<SymbolId> = self
            .scopes
            .iter()
            .flat_map(|e| e.iter())
            .filter(|(_, l)| l.late.is_some())
            .map(|(s, _)| *s)
            .collect();
        let c = capture::analyze_with(self.ctx, self.unit_id, ast, root, !self.without_directs, &outside_lates);
        self.cells = c.cells;
        self.assigned = c.assigned;
        self.allowed_directs = c.directs;
    }

    /// Name of the body of a closure inside this function: `$clo<k>` for the
    /// anonymous ones (k counts only the anonymous ones, in text order) and `$<name>` for
    /// the local functions (with `$<k>` if the name repeats).
    fn closure_name(&mut self, name: Option<SymbolId>) -> String {
        match name {
            None => {
                let k = self.n_closures;
                self.n_closures += 1;
                format!("{}$clo{k}", self.func.symbol)
            }
            Some(s) => {
                let base = format!(
                    "{}${}",
                    self.func.symbol,
                    super::sanitize_symbol(self.ctx.symbol_name(s))
                );
                let mut name = base.clone();
                let mut k = 1;
                while !self.local_names.insert(name.clone()) {
                    name = format!("{base}${k}");
                    k += 1;
                }
                name
            }
        }
    }

    /// The parameters of a function expression, for the uniform entry.
    fn closure_params(&self, f: &ast::Function) -> Vec<EntryParam> {
        let unit = self.unit_id;
        f.parameters
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|p| EntryParam {
                name: p.name.map(|n| self.ctx.symbol_name(n.sym).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                pattern: p.default_value.map_or(DefaultValue::Absent, |e| DefaultValue::Expr(unit, e)),
            })
            .collect()
    }

    /// The typed ABI of a function type: the representations of the positional ones
    /// and of the return (`void` comes back as `Ref`, the null), and their code. Only
    /// for types without type, optional or named parameters, up to 12
    /// positional ones.
    pub(super) fn type_abi(&self, t: TypeId) -> Option<(Vec<Type>, Type, i64)> {
        let dartforge_types::table::Type::Function { type_params, ret, positional, optional, named, nullable: false } =
            self.ctx.table.get(t)
        else {
            return None;
        };
        if !type_params.is_empty() || !optional.is_empty() || !named.is_empty() || positional.len() > 12 {
            return None;
        }
        let repr = |t: TypeId| match self.ctx.to_hir_type(t) {
            r @ (Type::I64 | Type::F64 | Type::I1) => r,
            _ => Type::Ref,
        };
        let params: Vec<Type> = positional.iter().map(|&p| repr(p)).collect();
        let ret = repr(*ret);
        let digit = |t: Type| match t {
            Type::I64 => 2,
            Type::F64 => 3,
            Type::I1 => 4,
            _ => 1,
        };
        let mut abi: i64 = 1;
        for &t in params.iter().chain(std::iter::once(&ret)) {
            abi = abi * 5 + digit(t);
        }
        Some((params, ret, abi))
    }

    /// The typed ABI of a synchronous closure with only required positional ones,
    /// by its static type (that of the context, already inferred).
    fn closure_abi(&self, f: &ast::Function, typ: Option<TypeId>) -> Option<(Vec<Type>, Type, i64)> {
        let params = f.parameters.as_deref().unwrap_or(&[]);
        if f.modifier != AsyncModifier::None
            || !f.type_params.is_empty()
            || params.iter().any(|p| p.kind != ParameterKind::Required)
        {
            return None;
        }
        let abi = self.type_abi(typ?)?;
        (abi.0.len() == params.len()).then_some(abi)
    }

    /// `(params) => e`, `(params) { … }` or the value of a local function.
    /// `ty`: the static type of the closure expression (the context one), from
    /// which the value type of an `async`/generator body comes.
    pub fn lower_closure(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span, typ: Option<TypeId>) -> Operand {
        let f = ast.function(fid);
        // Free variables that are locals visible here.
        let (free_set, _) = capture::free_set(self.ctx, self.unit_id, ast, fid);
        let captures: Vec<(SymbolId, super::locals::Local)> = free_set
            .into_iter()
            .filter_map(|s| self.lookup_local(s).map(|l| (s, l)))
            .collect();
        let with_this = self.this_param.is_some();
        let base = usize::from(with_this);

        // The name carries the fingerprint of what the body expects from the environment
        // (the captures, in order, with the mode and the type; the `this`; the type
        // tuple) and of the parameters: on a JIT reload, a live closure only
        // starts running the new body of the same name if it reads its
        // environment the same way. A new closure inserted before, or captures
        // that change, give another name, and the live closures stay in the body in
        // which they were created.
        let mut shape = format!("{with_this}|{}|{}|", self.type_tuple.is_some(), f.type_params.len());
        // Generic closure (local `T f<T>()`, `<T>(x) => …`): its type
        // arguments come in the hidden slot of the call and join, in the tuple of the
        // body, after those of its creator (`M<n_outside + i>`).
        let n_outside = self.tuple_size();
        let own_ids: Vec<dartforge_types::table::TypeParamId> = match typ.map(|t| self.ctx.table.get(t)) {
            Some(dartforge_types::table::Type::Function { type_params, .. }) if type_params.len() == f.type_params.len() => {
                type_params.to_vec()
            }
            _ => Vec::new(),
        };
        let own_set: Vec<(SymbolId, dartforge_types::table::TypeParamId, usize)> = f
            .type_params
            .iter()
            .enumerate()
            .map(|(i, tp)| {
                let id = own_ids.get(i).copied().unwrap_or(dartforge_types::table::TypeParamId(u32::MAX));
                (tp.name.sym, id, n_outside + i)
            })
            .collect();
        let generic = !own_set.is_empty();
        // Direct environment: with a single value in the environment — `this` without
        // captures, or a `Ref` capture (or the handle of its cell) without
        // `this` —, the closure keeps the value itself in place of the environment, and
        // the body receives it as the `env` parameter: without the environment object
        // (one allocation and one indirection less; the VM keeps the capture in the
        // `Context` only when there is more than one). Excluded: an `async`/generator body
        // (the frame keeps the environment), generic, type tuple, `late`.
        let direct = f.modifier == AsyncModifier::None
            && !generic
            && self.type_tuple.is_none()
            && match (with_this, &captures[..]) {
                (true, []) => true,
                (false, [(_, l)]) => {
                    l.late.is_none()
                        && (matches!(l.mode, Mode::Cell(_) | Mode::Environment { cell: true, .. }) || l.ty == Type::Ref)
                }
                _ => false,
            };
        if direct {
            shape.push_str("direto;");
        }
        for (sym, l) in &captures {
            let cell = matches!(l.mode, Mode::Cell(_) | Mode::Environment { cell: true, .. });
            shape.push_str(&format!("{}:{cell}:{:?};", self.ctx.symbol_name(*sym), l.ty));
        }
        for p in f.parameters.as_deref().unwrap_or(&[]) {
            let name = p.name.map_or("", |n| self.ctx.symbol_name(n.sym));
            shape.push_str(&format!("{name}:{:?}:{};", p.kind, p.required));
        }
        // The typed ABI of the body: parameters and return in the representations of the
        // closure type; it enters the fingerprint, and a live closure
        // stays in the body of the ABI in which it was born.
        let abi = self.closure_abi(f, typ);
        if let Some((ps, r, code)) = &abi {
            shape.push_str(&format!("abi:{code}:{ps:?}:{r:?};"));
        }
        let (reprs, ret_repr) = match &abi {
            Some((ps, r, _)) => (Some(ps.clone()), *r),
            None => (None, Type::Ref),
        };
        let symbol = format!(
            "{}$e{:08x}",
            self.closure_name(f.name.map(|n| n.sym)),
            (hash_name(&shape) as u64) as u32
        );
        let readable = f
            .name
            .map_or_else(|| "<closure>".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());

        // --- corpo ---------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, symbol.clone(), readable.clone(), ret_repr);
        // The symbolic stack trace (§13.14): the token of the closure, the entry through
        // which the runtime finds it, the link to the awaiter and the inherited hidden one.
        if self.ctx.stack_trace {
            b.stack_trace.token = self.ctx.line_and_column(self.unit_id, f.name.map_or(f.span.start, |n| n.span.start));
            b.stack_trace.marks |= self.stack_trace.marks & crate::hir::stack_trace_marks::HIDDEN;
            b.stack_trace.closure_entry = Some(format!("{symbol}$ent"));
            b.stack_trace.chain_link = captures
                .iter()
                .enumerate()
                .find(|(_, (_, l))| l.offset.is_some_and(|o| self.wait_link_at(ast, o)))
                .map(|(i, (_, l))| crate::hir::WaitLink {
                    index: if direct { 0 } else { base + i },
                    direct,
                    cell: matches!(l.mode, Mode::Cell(_) | Mode::Environment { cell: true, .. }),
                });
        }
        let env_b = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        if direct && with_this {
            b.this_param = Some(env_b.clone());
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        } else if direct {
            let (sym, l) = &captures[0];
            let cell = matches!(l.mode, Mode::Cell(_) | Mode::Environment { cell: true, .. });
            let mode = if cell {
                let ptr = b.entry_alloca(Type::Ref);
                b.emit(Instruction::Store { ptr: ptr.clone(), val: env_b.clone() }, Type::Void);
                Mode::Cell(ptr)
            } else {
                Mode::Value(env_b.clone())
            };
            b.bind_local_as(*sym, super::locals::Local { mode, ty: l.ty, static_type: l.static_type, finalizable: l.finalizable, scope: 0, offset: None, late: None });
        } else if with_this {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: 0,
                },
                Type::Ref,
            );
            b.this_param = Some(t);
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        for (i, (sym, l)) in captures.iter().enumerate().filter(|_| !direct) {
            let cell = matches!(
                l.mode,
                Mode::Cell(_) | Mode::Environment { cell: true, .. }
            );
            // A captured SIMD vector goes into the environment boxed (the position
            // has 64 bits).
            let ty = if l.ty.is_vector() { Type::Ref } else { l.ty };
            b.bind_env(*sym, env_b.clone(), base + i, cell, ty, l);
        }
        // RTI: the closure sees the type variables of its creator (`T` of the
        // enclosing generic function: the tuple goes at the end of the environment).
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.local_params.extend(own_set.iter().copied());
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.class_by_tuple {
            // In a factory there is no `this`, but `T` is the class's.
            b.enclosing_class = self.enclosing_class;
        }
        if self.type_tuple.is_some() && !generic {
            let t = b.emit(
                Instruction::EnvGet {
                    env: env_b.clone(),
                    index: base + captures.len(),
                },
                Type::I64,
            );
            b.type_tuple = Some(t);
        }
        let params = f.parameters.as_deref().unwrap_or(&[]);
        b.prepare_captures(
            ast,
            Root {
                parameters: params,
                body: Some(&f.body),
                initializers: &[],
            },
        );
        b.open_scope();
        for (i, p) in params.iter().enumerate() {
            let name = p
                .name
                .map_or_else(|| "arg".to_string(), |n| self.ctx.symbol_name(n.sym).to_string());
            let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
            let vid = b.add_param_dart(name, ty);
            if let Some(n) = p.name {
                b.declare_variable(n.sym, n.span.start as usize, ty, Operand::Val(vid));
            }
        }
        // The joined tuple (the outer one and the call's) reaches the body of the
        // generic closure as the last parameter.
        if generic {
            let t = b.add_param_rti("$types".to_string());
            b.type_tuple = Some(Operand::Val(t));
        }
        if f.modifier != AsyncModifier::None {
            // P6: an `async`, `sync*` or `async*` closure — the body becomes a state
            // machine. The value/element type comes from the return annotation
            // or from the static type of the closure; without either, `dynamic`.
            use super::async_sm::AsyncReturn;
            let ret_val = f.return_type.map(AsyncReturn::Annotation).or_else(|| {
                typ.and_then(|t| match self.ctx.table.get(t) {
                    dartforge_types::table::Type::Function { ret, .. } => Some(AsyncReturn::Typed(*ret)),
                    _ => None,
                })
            });
            b.lower_async_body(ast, params, &f.body, span, ret_val, super::async_sm::body_type(f.modifier));
        } else {
            match &f.body {
                FunctionBody::Block(s) => b.lower_stmt(ast, *s),
                FunctionBody::Expression(e) => {
                    let r = b.lower_expr(ast, *e);
                    if !b.is_terminated() {
                        let r = b.coerce(r, ret_repr);
                        b.terminate(Terminator::Return(Some(r)));
                    }
                }
                _ => {}
            }
        }
        // The body of the closure is a Dart body: the Ref return is Owned, like
        // that of a function (`lower_function`).
        if self.ctx.arc_memory && b.func.return_ty == Type::Ref {
            b.dart_ref_returns.insert(symbol.clone());
        }
        b.record_local_inline_policy(ast, fid);
        self.absorb(b);

        // --- entrada uniforme ----------------------------------------------
        let infos = self.closure_params(f);
        let symbol_ent = format!("{symbol}$ent");
        let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env_e = e.closure_context(clo);
        // The type arguments that the generic closure uses when the call
        // passes none (a dynamic call): the written bounds, like
        // instantiation to bounds; `dynamic` without a bound or when the bound
        // depends on type variables.
        let pattern = generic.then(|| {
            let mut text = String::from("L<");
            for (i, (_, id, _)) in own_set.iter().enumerate() {
                if i > 0 {
                    text.push(',');
                }
                let limit = (id.0 != u32::MAX)
                    .then(|| self.ctx.table.param(*id))
                    .filter(|d| d.explicit)
                    .map(|d| self.type_recipe(d.bound))
                    .filter(|r| !r.variables);
                text.push_str(limit.as_ref().map_or("D", |r| r.text.as_str()));
            }
            text.push('>');
            super::rti::Recipe { text, variables: false }
        });
        if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
            let mut all = vec![env_e.clone()];
            // The typed body receives each argument in its representation (the
            // unboxing checks the type, like the VM's dynamic entry).
            for (i, v) in vals.into_iter().enumerate() {
                let ty = reprs.as_ref().map_or(Type::Ref, |r| r[i]);
                let v = e.coerce(v, ty);
                all.push(v);
            }
            if let Some(pattern) = &pattern {
                // The tuple of the call is in the slot after the arguments.
                let own = e.slot_tuple(&args, &desc);
                let from_outside = if self.type_tuple.is_some() {
                    e.emit(Instruction::EnvGet { env: env_e.clone(), index: base + captures.len() }, Type::I64)
                } else {
                    Operand::Constant(Constant::Int(0))
                };
                let pattern = e.recipe_rti(pattern);
                let joined = e.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_rti_tuple_join".to_string(),
                        args: vec![
                            (from_outside, Type::I64),
                            (Operand::Constant(Constant::Int(n_outside as i64)), Type::I64),
                            (own, Type::I64),
                            (pattern, Type::I64),
                        ],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                all.push(joined);
            }
            if !e.is_terminated() {
                let r = e.emit_call_with_check(
                    Instruction::CallStatic {
                        symbol: symbol.clone(),
                        args: all,
                        ret_ty: ret_repr,
                    },
                    ret_repr,
                );
                let r = e.coerce(r, Type::Ref);
                if !e.is_terminated() {
                    e.terminate(Terminator::Return(Some(r)));
                }
            }
        }
        self.absorb(e);

        // --- creation --------------------------------------------------------
        let mut values = Vec::with_capacity(base + captures.len());
        if let Some(t) = self.this_param.clone() {
            values.push(t);
        }
        for (_, l) in &captures {
            let v = match self.local_cell(l) {
                Some(c) => c,
                None => self.read_local(l),
            };
            // The unboxed SIMD vector goes boxed (`bind_env` with `Ref`).
            let v = if self.operand_type(&v).is_vector() { self.coerce(v, Type::Ref) } else { v };
            values.push(v);
        }
        if let Some(t) = self.type_tuple.clone() {
            // The tuple goes raw into the context (`I64`), as the `EnvGet` of the body reads it.
            let t = self.coerce(t, Type::I64);
            values.push(t);
        }
        let env = if direct {
            let v = values.pop().expect("the value of the direct environment");
            self.coerce(v, Type::Ref)
        } else {
            self.emit(Instruction::AllocEnv { values: values }, Type::Ref)
        };
        match abi {
            Some((_, _, code)) => self.emit(
                Instruction::AllocClosureTyped { code_symbol: symbol_ent, env, typed: symbol, abi: code, direct },
                Type::Ref,
            ),
            // Without a typed ABI, the typed body is not called (`abi` 0 matches
            // no typed call): the entry in its place.
            None if direct => self.emit(
                Instruction::AllocClosureTyped { code_symbol: symbol_ent.clone(), env, typed: symbol_ent, abi: 0, direct },
                Type::Ref,
            ),
            None => self.emit(
                Instruction::AllocClosure {
                    code_symbol: symbol_ent,
                    env,
                },
                Type::Ref,
            ),
        }
    }

    /// The parameter declared at `offset` has `@pragma('vm:awaiter-link')`:
    /// the closure that captures it leads to the frame of the awaiter
    /// (`StackTraceUtils`, `TryGetAwaiterLink`).
    fn wait_link_at(&self, ast: &ast::Ast, offset: usize) -> bool {
        ast.functions.iter().any(|f| {
            f.parameters.iter().flatten().any(|p| {
                p.name.is_some_and(|n| n.span.start == offset)
                    && p.metadata.iter().any(|a| crate::source::pragma_with_string(self.ctx.interner, ast, a, "vm:awaiter-link"))
            })
        })
    }

    /// Applies metadata on a local declaration to its body, preserving separately
    /// declared nested functions and the generated uniform entry.
    pub fn record_local_inline_policy(&mut self, ast: &ast::Ast, fid: FunctionId) {
        let metadata = ast.local_metadata.iter().find_map(|(stmt, metadata)|
            matches!(ast.stmts[stmt.0 as usize].kind, ast::StmtKind::Function(id) if id == fid)
                .then_some(metadata));
        let Some(metadata) = metadata else { return };
        let has = |name| metadata.iter().any(|an|
            crate::source::pragma_with_string(self.ctx.interner, ast, an, name));
        let policy = if has("vm:never-inline") { Some(crate::hir::InlinePolicy::Never) }
            else if has("vm:prefer-inline") { Some(crate::hir::InlinePolicy::Prefer) }
            else { None };
        if let Some(policy) = policy {
            self.inline_policies.insert(self.func.symbol.clone(), policy);
            let prefix = format!("{}$async$q", self.func.symbol);
            for f in &self.extra_functions {
                if f.symbol.strip_prefix(&prefix)
                    .is_some_and(|suffix| suffix.len() == 8 && suffix.bytes().all(|b| b.is_ascii_hexdigit())) {
                    self.inline_policies.insert(f.symbol.clone(), policy);
                }
            }
        }
    }

    /// Hands to the module (through the `extra_functions` of this function) a function
    /// built separately, with its diagnostics.
    pub fn absorb(&mut self, mut b: FnBuilder<'a, 'c>) {
        b.close_stack_trace();
        self.inline_policies.extend(b.inline_policies);
        self.errors.extend(b.errors);
        self.extra_globals.extend(b.extra_globals);
        self.dart_scalar_parameters.extend(b.dart_scalar_parameters);
        self.dart_rti_parameters.extend(b.dart_rti_parameters);
        self.dart_vector_parameters.extend(b.dart_vector_parameters);
        self.arc_uniform_returns.extend(b.arc_uniform_returns);
        self.dart_ref_returns.extend(b.dart_ref_returns);
        self.arc_rti_caches.extend(b.arc_rti_caches);
        self.arc_shared_functions.extend(b.arc_shared_functions);
        self.arc_direct_local_functions.extend(b.arc_direct_local_functions);
        self.async_frame_layouts.extend(b.async_frame_layouts);
        self.extra_functions.push(b.func);
        self.extra_functions.extend(b.extra_functions);
    }

    /// Local `R f(params) { … }`: the name is a variable that keeps the closure
    /// (in a cell when the function calls itself or is captured and bound
    /// later — `capture.rs` marks the name as assigned).
    pub fn declare_local_function(&mut self, ast: &ast::Ast, fid: FunctionId, span: Span) {
        let f = ast.function(fid);
        let Some(name) = f.name else {
            return;
        };
        self.declare_variable(
            name.sym,
            name.span.start as usize,
            Type::Ref,
            Operand::Constant(Constant::Null),
        );
        let typ = self.literal_function_type(ast, fid, None);
        let clo = self.lower_closure(ast, fid, span, typ);
        self.define_closure_rti(clo.clone(), ast, fid, None);
        self.store_local(name.sym, clo);
    }

    /// Unpacks the arguments of the uniform convention into the parameters
    /// `params` (in declaration order), all `Ref`. A wrong arity throws
    /// `NoSuchMethodError` and returns `None` (the block has already returned).
    pub fn unpack(
        &mut self,
        params: &[EntryParam],
        args: Operand,
        desc: Operand,
    ) -> Option<Vec<Operand>> {
        let n_req = params
            .iter()
            .filter(|p| p.kind == ParameterKind::Required)
            .count();
        let n_pos = params
            .iter()
            .filter(|p| p.kind != ParameterKind::Named)
            .count();
        let named: Vec<&EntryParam> = params
            .iter()
            .filter(|p| p.kind == ParameterKind::Named)
            .collect();
        let mut sig = vec![n_req as i64, n_pos as i64, named.len() as i64];
        for p in &named {
            sig.push(hash_name(p.name.as_deref().unwrap_or("")));
        }
        for p in &named {
            sig.push(i64::from(p.required));
        }
        // The typed entry (`typed_entry.rs`): the arity and the names come
        // guaranteed by the static caller.
        if !self.guaranteed_arity {
            let ok = if named.is_empty() {
                // Without named ones in the signature, the check of
                // `dartforge_args_match` is: no named one passed and
                // `n_req <= positionals <= n_pos` — inline, without the call.
                let int = |x: usize| Operand::Constant(Constant::Int(x as i64));
                let npos = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: int(0) }, Type::I64);
                let n_named = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: int(1) }, Type::I64);
                if n_req == n_pos {
                    let x = self.emit(Instruction::Xor(npos, int(n_req)), Type::I64);
                    let y = self.emit(Instruction::Or(x, n_named), Type::I64);
                    self.emit(Instruction::ICmp(ICmpOp::Eq, y, int(0)), Type::I1)
                } else {
                    let lo = self.emit(Instruction::ICmp(ICmpOp::Sge, npos.clone(), int(n_req)), Type::I1);
                    let hi = self.emit(Instruction::ICmp(ICmpOp::Sle, npos, int(n_pos)), Type::I1);
                    let z = self.emit(Instruction::ICmp(ICmpOp::Eq, n_named, int(0)), Type::I1);
                    let a = self.emit(Instruction::And(lo, hi), Type::I64);
                    let a = self.emit(Instruction::And(a, z), Type::I64);
                    self.emit(Instruction::ICmp(ICmpOp::Ne, a, int(0)), Type::I1)
                }
            } else {
                let sig = self.emit(Instruction::ConstArray(sig), Type::Ptr);
                let ok = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_args_match".to_string(),
                        args: vec![(desc.clone(), Type::Ptr), (sig, Type::Ptr)],
                        ret_ty: Type::I8,
                    },
                    Type::I8,
                );
                self.emit(Instruction::ICmp(ICmpOp::Ne, ok, Operand::Constant(Constant::Int(0))), Type::I1)
            };
            let b_ok = self.new_block();
            let b_error = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: ok,
                then_block: b_ok,
                else_block: b_error,
            });
            self.set_block(b_error);
            let call = Instruction::CallRuntime {
                name: "dartforge_nsm_call".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            };
            if self.ctx.arc_memory {
                // The reentry/pending contract needs an explicit error exit
                // to validate the cleanup of the owners of the entry.
                self.emit_call_with_check(call, Type::Void);
            } else {
                self.emit(call, Type::Void);
            }
            self.terminate(Terminator::Return(Some(Operand::Constant(Constant::Null))));
            self.set_block(b_ok);
        }

        let npos = self.emit(
            Instruction::LoadIndexed {
                base: desc.clone(),
                index: Operand::Constant(Constant::Int(0)),
            },
            Type::I64,
        );
        let mut exit = Vec::with_capacity(params.len());
        let mut i_pos = 0i64;
        for p in params {
            if p.kind == ParameterKind::Required {
                let v = self.emit(
                    Instruction::LoadIndexed {
                        base: args.clone(),
                        index: Operand::Constant(Constant::Int(i_pos)),
                    },
                    Type::Ref,
                );
                exit.push(v);
                i_pos += 1;
                continue;
            }
            let (present, index) = if p.kind == ParameterKind::Optional {
                let c = self.emit(
                    Instruction::ICmp(ICmpOp::Sgt, npos.clone(), Operand::Constant(Constant::Int(i_pos))),
                    Type::I1,
                );
                let idx = Operand::Constant(Constant::Int(i_pos));
                i_pos += 1;
                (c, idx)
            } else {
                let h = hash_name(p.name.as_deref().unwrap_or(""));
                let j = self.emit(
                    Instruction::CallRuntime {
                        name: "dartforge_arg_index".to_string(),
                        args: vec![(desc.clone(), Type::Ptr), (Operand::Constant(Constant::Int(h)), Type::I64)],
                        ret_ty: Type::I64,
                    },
                    Type::I64,
                );
                let c = self.emit(
                    Instruction::ICmp(ICmpOp::Sge, j.clone(), Operand::Constant(Constant::Int(0))),
                    Type::I1,
                );
                let idx = self.emit(Instruction::Add(npos.clone(), j), Type::I64);
                (c, idx)
            };
            let b_yes = self.new_block();
            let b_no = self.new_block();
            let b_end = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: present,
                then_block: b_yes,
                else_block: b_no,
            });
            self.set_block(b_yes);
            let v_yes = self.emit(
                Instruction::LoadIndexed {
                    base: args.clone(),
                    index: index,
                },
                Type::Ref,
            );
            let yes_end = self.current_block;
            self.terminate(Terminator::Branch(b_end));
            self.set_block(b_no);
            let v_no = match p.pattern {
                DefaultValue::Absent => Operand::Constant(Constant::Null),
                DefaultValue::Expr(unit, e) => self.lower_expr_of(unit, e),
                DefaultValue::OfFunction(fid, i) => self
                    .default_value(fid, i)
                    .unwrap_or(Operand::Constant(Constant::Null)),
            };
            let v_no = self.coerce(v_no, Type::Ref);
            let mut entries = vec![(yes_end, v_yes)];
            if !self.is_terminated() {
                entries.push((self.current_block, v_no));
                self.terminate(Terminator::Branch(b_end));
            }
            self.set_block(b_end);
            let v = self.emit(
                Instruction::Phi {
                    incoming: entries,
                    ty: Type::Ref,
                },
                Type::Ref,
            );
            exit.push(v);
        }
        Some(exit)
    }

    /// Calls a function value (closure, tear-off) with already evaluated
    /// arguments, through the uniform convention. Returns `Ref`.
    pub fn call_function_value(&mut self, callee: Operand, evaluated: &[Evaluated]) -> Operand {
        // The static type of the called value (`lower_call`): with a typed
        // ABI, the call tries the typed body of the closure first.
        let typ = self.callee_type.take();
        let call = self.current_call.take();
        let callee = self.coerce(callee, Type::Ref);
        if let Some((reprs, ret, abi)) = typ.and_then(|t| self.type_abi(t))
            && reprs.len() == evaluated.len()
            && evaluated.iter().all(|(n, _)| n.is_none())
        {
            return self.call_typed_closure(callee, evaluated, &reprs, ret, abi);
        }
        let tuple = self.value_call_tuple(typ, call);
        self.call_uniform_closure(callee, evaluated, tuple)
    }

    /// The type argument tuple that the call `call` of a function value
    /// passes in the hidden slot: the written ones (`f<int>(…)`); without them, those
    /// deduced by matching the generic static type of the value (`ty`,
    /// `R Function<X…>(…)`) with that of the call and those of the arguments, as
    /// inference did (what cannot be deduced stays `dynamic`). `0` (none) when
    /// the value is not generic: the generic closure called this way uses the bounds.
    fn value_call_tuple(&mut self, typ: Option<TypeId>, call: Option<ExprId>) -> Operand {
        use dartforge_types::table::Type as T;
        let zero = Operand::Constant(Constant::Int(0));
        let Some(expr) = call else { return zero };
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let ast::ExprKind::Call { arguments, .. } = &unit_ast.expr(expr).kind else { return zero };
        if !arguments.type_args.is_empty() {
            let args = arguments.type_args.to_vec();
            let mut r = self.type_argument_recipes(&args);
            r.text = format!("L<{}>", r.text);
            return self.recipe_rti(&r);
        }
        let Some(t) = typ else { return zero };
        let T::Function { type_params, ret, positional, optional, named, .. } = self.ctx.table.get(t).clone() else {
            return zero;
        };
        if type_params.is_empty() {
            return zero;
        }
        let mut found_items: Vec<Option<TypeId>> = vec![None; type_params.len()];
        if let Some(real) = self.ctx.get_type_raw(self.unit_id, expr) {
            self.unify(ret, real, &type_params, &mut found_items);
        }
        let mut positionals = positional.iter().chain(optional.iter());
        for a in arguments.args.iter() {
            let decl = match a.name {
                Some(n) => named.iter().find(|(s, _, _)| *s == n.sym).map(|(_, t, _)| *t),
                None => positionals.next().copied(),
            };
            if let (Some(d), Some(real)) = (decl, self.ctx.get_type_raw(self.unit_id, a.value)) {
                self.unify(d, real, &type_params, &mut found_items);
            }
        }
        let mut r = super::rti::Recipe { text: "L<".to_string(), variables: false };
        for (i, found) in found_items.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            let x = self.type_recipe(found.unwrap_or(self.ctx.core.dynamic_));
            r.text.push_str(&x.text);
            r.variables |= x.variables;
        }
        r.text.push('>');
        self.recipe_rti(&r)
    }

    /// The call through the typed body when the closure has the ABI `abi`, checked
    /// at runtime (another closure, a callable class or a subtype
    /// with another representation go through the uniform entry): without a vector of
    /// arguments, a descriptor, an arity check or boxes.
    fn call_typed_closure(&mut self, callee: Operand, evaluated: &[Evaluated], reprs: &[Type], ret: Type, abi: i64) -> Operand {
        // Returns `Ref`, like the uniform call. The fields of `_Closure`
        // (code, context, typed body, ABI) are read from the block inline,
        // after checking the class (`@df.class` inline): only a `_Closure`
        // block has the four fields; whatever is not a closure goes through the
        // uniform entry.
        debug_assert_ne!(abi, 0);
        let fast = self.new_block();
        let slow = self.new_block();
        let join = self.new_block();
        let check_abi = self.new_block();
        let class = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(callee.clone(), Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let is_closure = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                class,
                Operand::Constant(Constant::Int(i64::from(dartforge_runtime::layout::cid::CLOSURE))),
            ),
            Type::I1,
        );
        self.terminate(Terminator::CondBranch { cond: is_closure, then_block: check_abi, else_block: slow });

        self.set_block(check_abi);
        let closure_abi = self.object_word(callee.clone(), super::boxes::CLOSURE_ABI, Type::I64);
        let has = self.emit(Instruction::ICmp(ICmpOp::Eq, closure_abi, Operand::Constant(Constant::Int(abi))), Type::I1);
        self.terminate(Terminator::CondBranch { cond: has, then_block: fast, else_block: slow });

        self.set_block(fast);
        let target = self.object_word(callee.clone(), super::boxes::CLOSURE_TYPED, Type::I64);
        // The context is a reference (a root while alive).
        let env = self.closure_context(callee.clone());
        let mut args = vec![(env, Type::Ref)];
        for ((_, v), &t) in evaluated.iter().zip(reprs) {
            let v = self.coerce(v.clone(), t);
            args.push((v, t));
        }
        let r = self.emit_call_with_check(Instruction::TypedCall { target, args, ret }, ret);
        // The contract of `call_function_value` is `Ref` (the caller joins it with
        // other paths); the optimizer undoes the box when the user wants
        // the scalar (`Unbox(Box(x))`).
        let r = self.coerce(r, Type::Ref);
        let fast_end = self.current_block;
        let fast_arrives = !self.is_terminated();
        if fast_arrives {
            self.terminate(Terminator::Branch(join));
        }

        self.set_block(slow);
        let s = self.call_uniform_closure(callee, evaluated, Operand::Constant(Constant::Int(0)));
        let slow_end = self.current_block;
        let slow_arrives = !self.is_terminated();
        if slow_arrives {
            self.terminate(Terminator::Branch(join));
        }
        self.set_block(join);
        match (fast_arrives, slow_arrives) {
            (true, true) => self.emit(Instruction::Phi { incoming: vec![(fast_end, r), (slow_end, s)], ty: Type::Ref }, Type::Ref),
            (true, false) => r,
            (false, true) => s,
            (false, false) => {
                self.terminate(Terminator::Unreachable);
                Operand::Constant(Constant::Null)
            }
        }
    }

    /// The call through the uniform convention (the `$ent` entry of the closure).
    fn call_uniform_closure(&mut self, callee: Operand, evaluated: &[Evaluated], tuple_types: Operand) -> Operand {
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
        self.emit_call_with_check(
            Instruction::CallClosure {
                closure: callee,
                args,
                names,
                ret_ty: Type::Ref,
                tuple_types,
            },
            Type::Ref,
        )
    }

    /// The type argument tuple that the call through the uniform convention
    /// passes in the slot after the arguments (`0`: none).
    pub(super) fn slot_tuple(&mut self, args: &Operand, desc: &Operand) -> Operand {
        let npos = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(0)) }, Type::I64);
        let n_named = self.emit(Instruction::LoadIndexed { base: desc.clone(), index: Operand::Constant(Constant::Int(1)) }, Type::I64);
        let index = self.emit(Instruction::Add(npos, n_named), Type::I64);
        self.emit(Instruction::LoadIndexed { base: args.clone(), index: index }, Type::I64)
    }

    /// The parameters of a program function, for the uniform entry.
    pub(super) fn function_params(&self, fid: usize) -> Vec<EntryParam> {
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return Vec::new();
        };
        data
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| EntryParam {
                name: p.name.map(|n| self.ctx.symbol_name(n).to_string()),
                kind: p.kind,
                required: p.kind == ParameterKind::Required || p.required,
                pattern: DefaultValue::OfFunction(fid, i),
            })
            .collect()
    }

    /// Tear-off of a top-level or static function: canonical.
    pub fn function_tearoff(&mut self, fid: usize) -> Operand {
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$tear");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let unit = self.unit_id;
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, unit, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
                // The arguments of a dynamic call, checked as in the
                // VM (" of 'name'") before being converted.
                e.check_entry_arguments(fid, &vals);
                // Generic function: the type arguments come in the hidden slot.
                if e.generic_function(fid) {
                    e.armed_tuple = Some(e.slot_tuple(&args, &desc));
                }
                let reprs: Vec<Type> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .map(|p| e.repr(p.ty))
                    .collect();
                let vals: Vec<Operand> = vals
                    .into_iter()
                    .zip(reprs)
                    .map(|(v, r)| e.coerce(v, r))
                    .collect();
                let r = e.call_direct(fid, None, vals);
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: symbol_ent,
            },
            Type::Ref,
        );
        // RTI: the signature of the function (`f is R Function(P)`).
        self.define_tearoff_rti(t.clone(), fid, None);
        t
    }

    /// Tear-off of an instance method: a new closure with the receiver in the
    /// environment; the entry calls the member with the dispatch through the receiver.
    pub fn method_tearoff(&mut self, recv: Operand, fid: usize, span: Span) -> Operand {
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$tearm");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
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
            let receiver = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            if self.tearoff_by_selector(fid) {
                // The tear-off is that of the implementation that the dispatch chooses
                // (as in the VM): the arguments are checked by its
                // parameters, and the omitted optional ones get its
                // defaults, not those of the static member. `Def<E>` with
                // `hash(Object? e)` implementing `Eq<E>.hash(E e)`, taken
                // from a `Def<Never>`, accepts any argument (the
                // `DefaultEquality<Never>` of `package:collection` in
                // freezed's `SetEquality<String>().equals`, B01). The vector
                // of arguments, the descriptor and the type tuple go as
                // they came to the entry of the implementation through the selector without the
                // `t`, the one that checks everything (`typed_entry.rs`).
                let f = &self.ctx.program.functions[fid];
                let selector = super::sdk_source::selector_string(
                    self.ctx,
                    super::sdk_source::AccessKind::Call,
                    self.ctx.symbol_name(f.name),
                    f.library,
                );
                let r = e.emit_call_with_check(
                    Instruction::CallSelectorForward {
                        arc_owned: false,
                        selector,
                        recv: receiver,
                        args,
                        desc,
                    },
                    Type::Ref,
                );
                e.terminate(Terminator::Return(Some(r)));
            } else if let Some(vals) = e.unpack(&infos, args, desc) {
                // Single implementation: the arguments checked as in the
                // uniform entry (" of 'name'"), with the receiver kept
                // in place of `this`, before the direct call. An SDK member
                // with a single implementation: the check is that of its
                // entry (the parameter names are its own).
                let checked = self.single_implementation(fid).unwrap_or(fid);
                let saved = (e.this_param.replace(receiver.clone()), e.enclosing_class);
                e.enclosing_class = self.ctx.program.functions[checked].class;
                e.check_entry_arguments(checked, &vals);
                (e.this_param, e.enclosing_class) = saved;
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.call_member(receiver, fid, &evaluated, span);
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
        let c = self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol_ent,
                env,
            },
            Type::Ref,
        );
        self.define_tearoff_rti(c.clone(), fid, Some(recv));
        c
    }

    /// The entry of the tear-off of `fid` calls through the selector instead of checking
    /// the arguments by the parameters of `fid` and calling directly: `fid` may
    /// not be the implementation that the receiver runs (an abstract member or
    /// one overridden in the program, or an open SDK member), and an override
    /// may widen the type of a parameter (`Object?` in place of the `E` of the
    /// interface). Every class has the selector table (source SDK).
    fn tearoff_by_selector(&self, fid: usize) -> bool {
        use super::sdk_source::{Implementation, implementations, closed_member};
        let f = &self.ctx.program.functions[fid];
        let Some(cid) = f.class else { return false };
        if f.static_ || self.single_implementation(fid).is_some() {
            return false;
        }
        let name = self.ctx.symbol_name(f.name).to_string();
        !closed_member(self.ctx, cid, &name)
            || implementations(self.ctx, cid, &name)[..] != [Implementation::Function(fid)]
    }

    /// What a name without a resolution (closure body) is through the lexical scope,
    /// after the locals: a member of the enclosing class (going up the
    /// superclasses of the program), otherwise the scope of the library. Returns the
    /// resolution that inference would have stored, for the usual paths.
    pub fn resolve_by_name(&self, sym: SymbolId) -> Option<dartforge_types::resolved::Resolved> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        // In the body of an extension, its members come before those of the
        // `on` type (the lexical scope of the extension encloses the body).
        if let Some((e, _)) = self.this_extension {
            let x = &self.ctx.program.extensions[e.0 as usize];
            if let Some(&f) = x.instance_members.get(&sym).or_else(|| x.static_members.get(&sym)) {
                return Some(Resolved::ExtensionMember { extension: e, member: f });
            }
        }
        // In the body of an extension type, its members (and those of the extension
        // types it implements).
        if let Some(r) = self.member_te_by_name(sym) {
            return Some(r);
        }
        for c in self.enclosing_class.map(|c| crate::lower::members::linearization(self.ctx, c)).unwrap_or_default() {
            let class = &self.ctx.program.classes[c.0 as usize];
            if !self.ctx.compiled_library(class.library) {
                break;
            }
            if let Some(&f) = class.instance_members.get(&sym).or_else(|| class.static_members.get(&sym)) {
                return Some(Resolved::Member {
                    class: c,
                    member: MemberRef::Function(f),
                    via_super: false,
                });
            }
            if let Some(&v) = class
                .fields
                .iter()
                .find(|&&v| self.ctx.program.variables[v.0 as usize].name == sym)
            {
                return Some(Resolved::Member {
                    class: c,
                    member: MemberRef::Variable(v),
                    via_super: false,
                });
            }
            // The constants of an `enum` are its static members, but the
            // model keeps them separately (`enum_constants`): `case forward:` in the
            // body of the enum itself is the constant pattern `Direction.forward`.
            if let Some(v) = enum_constant(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        // In a static member, the statics of the class that declares it.
        if self.enclosing_class.is_none()
            && let Some(c) = self.member_class
        {
            let class = &self.ctx.program.classes[c.0 as usize];
            if let Some(&f) = class.static_members.get(&sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Function(f), via_super: false });
            }
            if let Some(&v) = class.fields.iter().find(|&&v| {
                let var = &self.ctx.program.variables[v.0 as usize];
                var.static_ && var.name == sym
            }) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
            if let Some(v) = enum_constant(self.ctx, c, sym) {
                return Some(Resolved::Member { class: c, member: MemberRef::Variable(v), via_super: false });
            }
        }
        let lib = self.ctx.program.unit(self.unit_id).library;
        let b = self.ctx.program.lookup(lib, sym)?;
        b.getter.or(b.setter).map(Resolved::Element)
    }

    /// Reads a name without a resolution that is not a local (see `resolve_by_name`).
    pub fn read_unresolved_name(&mut self, sym: SymbolId, span: Span) -> Option<Operand> {
        use dartforge_types::resolved::Resolved;
        match self.resolve_by_name(sym)? {
            r @ Resolved::Member { .. } if self.resolved_member_te(Some(&r)).is_some() => {
                let m = self.resolved_member_te(Some(&r))?;
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                Some(self.read_member_te(this, m, receiver, span))
            }
            Resolved::Member { member, .. } => Some(self.read_implicit_member(member, span)),
            Resolved::Element(el) => Some(self.read_element(el, span)),
            Resolved::ExtensionMember { member, .. } => {
                let this = self.this_param.clone().unwrap_or(Operand::Constant(Constant::Null));
                let receiver = self.implicit_receiver();
                Some(self.read_extension(this, member.0 as usize, receiver, span))
            }
            _ => None,
        }
    }

    /// `f<T…>`: the closure of a generic function with the type arguments
    /// fixed (`tuple`, in the environment). The entry builds the tuple and calls the
    /// function; the reified type is that of the instantiation (`ty`, static).
    pub fn instantiated_tearoff(&mut self, fid: usize, tuple: Operand, typ: Option<TypeId>) -> Operand {
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$teari");
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
            let t = e.emit(Instruction::EnvGet { env, index: 0 }, Type::I64);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                // The arguments of a dynamic call, checked as in the
                // VM (" of 'name'") before being converted.
                e.check_entry_arguments(fid, &vals);
                let reprs: Vec<Type> = self.ctx.outline.functions[fid].parameters.iter().map(|p| e.repr(p.ty)).collect();
                let vals: Vec<Operand> = vals.into_iter().zip(reprs).map(|(v, r)| e.coerce(v, r)).collect();
                e.armed_tuple = Some(t);
                let r = e.call_direct(fid, None, vals);
                let r = if matches!(e.operand_type(&r), Type::Void) {
                    Operand::Constant(Constant::Null)
                } else {
                    e.coerce(r, Type::Ref)
                };
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        // The tuple goes raw (`I64`), as the entry reads it (`EnvGet` in `I64`).
        let tuple = self.coerce(tuple, Type::I64);
        let env = self.emit(Instruction::AllocEnv { values: vec![tuple] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        self.define_instantiation_rti(c.clone(), fid, typ);
        c
    }

    /// `C<T…>.new`: like `constructor_tearoff`, with the type of the object
    /// (`object`, `C<T…>`) and the tuple of the class (`tuple`, for a factory)
    /// in the environment.
    pub fn instantiated_constructor_tearoff(
        &mut self,
        fid: usize,
        object: Operand,
        tuple: Operand,
        typ: Option<TypeId>,
        span: Span,
    ) -> Operand {
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$teari");
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
            let obj = e.emit(Instruction::EnvGet { env: env.clone(), index: 0 }, Type::I64);
            let t = e.emit(Instruction::EnvGet { env, index: 1 }, Type::I64);
            if let Some(vals) = e.unpack(&infos, args, desc) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let r = e.instantiate_evaluated_with_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &evaluated,
                    span,
                    Some(obj),
                    Some(t),
                );
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        // The type of the object and the tuple go raw (`I64`), as the entry reads them.
        let object = self.coerce(object, Type::I64);
        let tuple = self.coerce(tuple, Type::I64);
        let env = self.emit(Instruction::AllocEnv { values: vec![object, tuple] }, Type::Ref);
        let c = self.emit(Instruction::AllocClosure { code_symbol: symbol_ent, env }, Type::Ref);
        self.define_instantiation_rti(c.clone(), fid, typ);
        c
    }

    /// The reified type of an instantiated tear-off: the static type of the
    /// expression (the signature with the type arguments applied); without
    /// it, the declared signature.
    fn define_instantiation_rti(&mut self, clo: Operand, fid: usize, typ: Option<TypeId>) {
        match typ {
            Some(t) if matches!(self.ctx.table.get(t), dartforge_types::table::Type::Function { type_params, .. } if type_params.is_empty()) => {
                let r = self.type_rti(t);
                self.define_rti(clo, r);
            }
            _ => self.define_tearoff_rti(clo, fid, None),
        }
    }

    /// The resolution of an expression, or (an identifier without a resolution, as
    /// in a closure body) that of the name through the lexical scope.
    pub fn resolution_or_name(&self, ast: &ast::Ast, e: ast::ExprId) -> Option<dartforge_types::resolved::Resolved> {
        if let Some(r) = self.ctx.get_resolved(self.unit_id, e).cloned() {
            return Some(r);
        }
        match &ast.expr(e).kind {
            ast::ExprKind::Identifier(n) if self.lookup_local(n.sym).is_none() => self.resolve_by_name(n.sym),
            _ => None,
        }
    }

    /// The generic function (top-level or static, of the program) that `target`
    /// names, for `target<T…>`.
    pub fn target_generic_function(&self, ast: &ast::Ast, target: ast::ExprId) -> Option<usize> {
        use dartforge_types::resolved::{MemberRef, Resolved};
        let fid = match self.resolution_or_name(ast, target)? {
            Resolved::Element(dartforge_elements::model::Element::Function(f)) => f.0 as usize,
            Resolved::Member { member: MemberRef::Function(f), .. } => f.0 as usize,
            _ => return None,
        };
        let f = &self.ctx.program.functions[fid];
        let free = f.class.is_none() || f.static_;
        (free && f.variable.is_none() && f.kind == dartforge_elements::model::FunctionKind::Function && self.generic_function(fid))
            .then_some(fid)
    }

    /// The recipes of the written type arguments (`<int, T>`), joined by
    /// commas (`dynamic` for what does not resolve).
    pub fn type_argument_recipes(&self, args: &[ast::TypeId]) -> super::rti::Recipe {
        let unit_ast = &self.ctx.program.unit(self.unit_id).ast;
        let mut r = super::rti::Recipe { text: String::new(), variables: false };
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                r.text.push(',');
            }
            match self.annotation_recipe(unit_ast.ty(*a)) {
                Some(x) => {
                    r.text.push_str(&x.text);
                    r.variables |= x.variables;
                }
                None => r.text.push('D'),
            }
        }
        r
    }

    /// Constructor tear-off (`C.new`, `C.name`): canonical, and the entry
    /// builds the object with the received arguments.
    /// Generic tear-off of the constructor `fid` of the class `c` whose static type
    /// is `C<A…> Function<P…>(…)` (`ty`, the `P`s in `params`, the `A`s in
    /// `args`): the call passes the `P`s in the hidden slot, and the object is born
    /// `C<A…>` with `P` replaced by them (an alias `typedef F<T> = C<int, T>`
    /// gives `C<int, T>`).
    pub fn generic_constructor_tearoff(
        &mut self,
        fid: usize,
        c: dartforge_elements::model::ClassId,
        params: &[dartforge_types::table::TypeParamId],
        args: &[TypeId],
        typ: TypeId,
        span: Span,
    ) -> Operand {
        let vars: Vec<String> = (0..params.len()).map(|i| format!("M{i}")).collect();
        let strings: Vec<String> = args.iter().map(|a| self.type_recipe_with(*a, params, &vars).text).collect();
        let string_object = format!("C{}<{}>", self.ctx.id_rti(c), strings.join(","));
        let tuple_text = format!("L<{}>", strings.join(","));
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$teara{:08x}", (hash_name(&string_object) as u64) as u32);
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let a = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let d = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, a.clone(), d.clone()) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                let slot = e.slot_tuple(&a, &d);
                e.type_tuple = Some(slot);
                let object = e.recipe_rti(&super::rti::Recipe { text: string_object.clone(), variables: true });
                let tuple = e.recipe_rti(&super::rti::Recipe { text: tuple_text.clone(), variables: true });
                let r = e.instantiate_evaluated_with_rti(
                    dartforge_elements::model::FunctionElementId(fid as u32),
                    &evaluated,
                    span,
                    Some(object),
                    Some(tuple),
                );
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(Instruction::TearOff { code_symbol: symbol_ent }, Type::Ref);
        // The signature is the static type (the generic function).
        let r = self.type_recipe(typ);
        let rti = self.recipe_rti(&r);
        self.define_rti(t.clone(), rti);
        t
    }

    /// Generic tear-off without instantiation (`final f = C.new;`): the signature
    /// is the static type, the generic function (`<Y0>(Y0) => C<Y0>` in the VM), not
    /// that of the constructor with the class parameters as `dynamic`.
    pub fn tearoff_generic_signature(&mut self, t: &Operand, e: ast::ExprId) {
        if let Some(typ) = self.ctx.get_type(self.unit_id, e)
            && matches!(self.ctx.table.get(typ), dartforge_types::table::Type::Function { type_params, .. } if !type_params.is_empty())
        {
            let r = self.type_recipe(typ);
            let rti = self.recipe_rti(&r);
            self.define_rti(t.clone(), rti);
        }
    }

    pub fn constructor_tearoff(&mut self, fid: usize, span: Span) -> Operand {
        let target = super::symbol_of(self.ctx, fid);
        let symbol_ent = format!("{target}$tear");
        if self.ctx.arc_memory { self.arc_shared_functions.insert(symbol_ent.clone()); }
        if !self.done_entries.contains(&symbol_ent) {
            self.done_entries.insert(symbol_ent.clone());
            let infos = self.function_params(fid);
            let name = self.ctx.symbol_name(self.ctx.program.functions[fid].name).to_string();
            let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), name, Type::Ref);
            e.add_param("closure".to_string(), Type::Ref);
            let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
            let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
            e.declare_arc_uniform_return();
            if let Some(vals) = e.unpack(&infos, args.clone(), desc.clone()) {
                let evaluated: Vec<Evaluated> = self.ctx.outline.functions[fid]
                    .parameters
                    .iter()
                    .zip(vals)
                    .map(|(p, v)| (if p.kind == ParameterKind::Named { p.name } else { None }, v))
                    .collect();
                // Constructor of a generic class: the tear-off is a generic
                // function in the class parameters (`C<T> Function<T>()`),
                // and the type arguments of the call come in the hidden slot
                // (none: `dynamic`).
                let class = self.ctx.program.functions[fid].class;
                let n = class.and_then(|c| self.ctx.outline.classes.get(c.0 as usize)).map_or(0, |d| d.type_params.len());
                let r = match class.filter(|_| n > 0) {
                    Some(c) => {
                        let tuple = e.slot_tuple(&args, &desc);
                        e.type_tuple = Some(tuple.clone());
                        let vars: Vec<String> = (0..n).map(|i| format!("M{i}")).collect();
                        let object = e.recipe_rti(&super::rti::Recipe {
                            text: format!("C{}<{}>", self.ctx.id_rti(c), vars.join(",")),
                            variables: true,
                        });
                        e.instantiate_evaluated_with_rti(
                            dartforge_elements::model::FunctionElementId(fid as u32),
                            &evaluated,
                            span,
                            Some(object),
                            Some(tuple),
                        )
                    }
                    None => e.instantiate_evaluated(dartforge_elements::model::FunctionElementId(fid as u32), &evaluated, span),
                };
                let r = e.coerce(r, Type::Ref);
                e.terminate(Terminator::Return(Some(r)));
            }
            self.absorb(e);
        }
        let t = self.emit(
            Instruction::TearOff {
                code_symbol: symbol_ent,
            },
            Type::Ref,
        );
        // The constructor is also a reified function. Without the signature, the
        // `current as E` of ListIterator rejects a tear-off kept in a
        // `List<C Function()>` when E starts being propagated by the SDK.
        self.define_tearoff_rti(t.clone(), fid, None);
        t
    }
}

/// The constant `sym` of the `enum` `c`, if `c` is an enum and declares it.
fn enum_constant(
    ctx: &crate::context::Context,
    c: dartforge_elements::model::ClassId,
    sym: SymbolId,
) -> Option<dartforge_elements::model::VariableId> {
    ctx.program.classes[c.0 as usize]
        .enum_constants
        .iter()
        .copied()
        .find(|v| ctx.program.variables[v.0 as usize].name == sym)
}
