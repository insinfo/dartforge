//! `async`/`await` (P6, docs/NATIVO-PLANO.md §7.7): the body as a state
//! machine, modeled on dart2js (`pkg/compiler/lib/src/js/rewrite_async.dart`)
//! and on the supports of `native_sdk/async/async_patch.dart`.
//!
//! A function `async f(p…) { … }` becomes two:
//!
//! * **the stub `f`** (the usual symbol): creates the **frame** — a heap
//!   object —, the `Completer` (`_makeAsyncAwaitCompleter`), the closure of the body
//!   (registered in the zone by `_wrapBody`) and starts the body
//!   synchronously (`_asyncStartSync`), returning `completer.future`;
//! * **the body `f$async(env, code, result)`**, a closure with the
//!   frame in its environment. The entry block reads the frame and jumps on the
//!   `state` kept in it: 0 is the start; each `await` is a state.
//!
//! **`await e`.** Writes state `k` into the frame, calls `_asyncAwait(e,
//! body)` and **returns**; the resumption block `R_k` is only reached by the
//! `switch` of the entry, when the `Future` completes and calls the body again.
//! With `code == 1` (`_ERROR`) the `result` is an `_DfAsyncError`: the error is
//! thrown there, at the point of the `await`, with its stack trace, and follows the usual
//! pending exception path — the `try`/`catch`/`finally` that surround the
//! `await` are those of the lowering (the same `exception_target` as before the
//! suspension). An exception that reaches the top of the body goes to `_asyncRethrow`
//! (`completer.completeError`); `return v` goes to `_asyncReturn`.
//!
//! **What crosses an `await` lives in the frame** (contract G: the frame is
//! reached through heap edges — the environment of the body closure, the
//! `_FutureListener` that waits —, not through the shadow stack, which is torn down at
//! each suspension). Two passes over the HIR of the finished body, which do not
//! depend on how each construct was lowered:
//!
//! 1. every `alloca` (the locals, R6) becomes a position of the frame, and its
//!    `load`/`store`, a read/write of the position;
//! 2. every SSA value **live at the entry of a resumption** (liveness with the
//!    virtual suspension → resumption edge) is written into a position of the frame
//!    right after it is defined and reread before each use — what LLVM
//!    does in the *coroutine frame* (`CoroSplit`), without needing to rebuild the
//!    SSA. The values of the entry block (frame, `this`, outer environment)
//!    are recomputed at each entry and stay out.
//!
//! Frame layout: `[state, completer, body, this, outer environment,
//! type tuple, parameters…, locals…, temporaries…]`.

use super::fn_builder::FnBuilder;
use super::locals::{Local, Mode};
use crate::hir::*;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{self, FunctionBody};
use dartforge_intern::SymbolId;
use std::collections::{HashMap, HashSet};

/// Class id of the frame of an `async` function (below the record
/// shapes, `context::SHAPE_BASE_ID`).
pub const ASYNC_FRAME_ID: u32 = 0x3FFF_FF00;

const Q_STATE: usize = 0;
const Q_COMPLETER: usize = 1;
const Q_BODY: usize = 2;
const Q_THIS: usize = 3;
const Q_ENV: usize = 4;
const Q_TUPLE: usize = 5;
const Q_PARAMS: usize = 6;

/// Where the return type of an `async`/`sync*`/`async*` body comes from: the
/// type (declared, or the static one of a closure) or the annotation of a
/// local function.
#[derive(Clone, Copy)]
pub enum AsyncReturn {
    Typed(dartforge_types::table::TypeId),
    Annotation(ast::TypeId),
}

/// The kind of suspended body: `async`, `sync*` or `async*`.
///
/// All three use the same state machine (frame in the heap, `switch` on the
/// state at the entry, the two passes over the HIR); they change the stub, the
/// suspension points and what `return` and the uncaught exception do:
///
/// | | stub returns | suspension | `return` | exception at the top |
/// | --- | --- | --- | --- | --- |
/// | `async` | `completer.future` | `await` | `_asyncReturn` | `_asyncRethrow` |
/// | `sync*` | `_SyncStarIterable` | `yield`, `yield*` (returns `true`) | returns `false` | leaves the body |
/// | `async*` | the stream of the controller | `await`, `yield`, `yield*` | `_asyncStarReturn` | `_asyncStarError` |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Async,
    SyncStar,
    AsyncStar,
}

/// The body kind of a suspending modifier (`async`, `sync*`,
/// `async*`).
///
/// # Panics
/// With `AsyncModifier::None`, which is not a suspended body.
pub fn body_type(m: ast::AsyncModifier) -> BodyType {
    match m {
        ast::AsyncModifier::Async => BodyType::Async,
        ast::AsyncModifier::SyncStar => BodyType::SyncStar,
        ast::AsyncModifier::AsyncStar => BodyType::AsyncStar,
        ast::AsyncModifier::None => panic!("synchronous body is not suspended"),
    }
}

/// The lowering state of the body of an `async`, `sync*` or `async*` function.
#[derive(Debug, Clone)]
pub struct AsyncState {
    pub ty: BodyType,
    /// `sync*`: the iterator that calls the body (a parameter; null for the others).
    pub iterator: Operand,
    pub frame: Operand,
    /// The body closure registered in the zone (the one `_asyncAwait` receives).
    pub body: Operand,
    /// The `Completer` (`async`) or the controller of the stream (`async*`).
    pub completer: Operand,
    pub code: Operand,
    pub result: Operand,
    /// `(k, R_k)`: the resumption block of each state.
    pub resumptions: Vec<(i64, BlockId)>,
    /// `(A, R_k)`: the block that suspends and its resumption.
    pub suspensions: Vec<(BlockId, BlockId)>,
    /// The next `Return` is raw (a suspension or the end of the top handler):
    /// it does not go through `_asyncReturn`.
    pub raw_return: bool,
    /// The symbolic stack trace (§13.14): the position of each `await`, by state.
    pub waits: Vec<(i64, (u32, u32))>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Top-level function `name` of `dart:async` (the supports of `async_patch`).
    pub fn async_support(&self, name_str: &str) -> Option<usize> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup(name_str)?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        match b.getter? {
            dartforge_elements::model::Element::Function(f) => Some(f.0 as usize),
            _ => None,
        }
    }

    /// Calls the `dart:async` support `name` with the already evaluated arguments.
    pub(super) fn call_support(&mut self, name_str: &str, args: Vec<Operand>, span: Span) -> Operand {
        let Some(fid) = self.async_support(name_str) else {
            return self.unsupported(&format!("missing dart:async helper `{name_str}`"), span);
        };
        let evaluated: Vec<super::members::Evaluated> = args.into_iter().map(|a| (None, a)).collect();
        let args = self.match_args(fid, &evaluated);
        self.call_direct(fid, None, args)
    }

    /// [`FnBuilder::call_support`] on the `return` path: its exception
    /// branch, without a handler, is a raw return — otherwise it would go back to
    /// `async_return`, which would call the support again.
    fn call_support_raw(&mut self, name_str: &str, args: Vec<Operand>) -> Operand {
        let before = self.async_state.as_ref().is_some_and(|e| e.raw_return);
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = true;
        }
        let r = self.call_support(name_str, args, Span { start: 0, end: 0 });
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = before;
        }
        r
    }

    /// The reference stays explicit in the HIR for the ownership preparation.
    /// Scalar bits keep the conversion of representation `ty`.
    fn read_position(&mut self, obj: Operand, idx: usize, ty: Type) -> Operand {
        if ty == Type::Ref {
            return self.emit(Instruction::GetField { object: obj, index: idx }, Type::Ref);
        }
        let bits = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_get".to_string(),
                args: vec![(obj, Type::Ref), (Operand::Constant(Constant::Int(idx as i64)), Type::I64)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        self.bits_to(bits, ty)
    }

    /// `dartforge_object_set` with the tag of the representation of the value (E1).
    fn store_position(&mut self, obj: Operand, idx: usize, val: Operand) {
        let (bits, is_ref) = self.to_bits(val);
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_object_set".to_string(),
                args: vec![
                    (obj, Type::Ref),
                    (Operand::Constant(Constant::Int(idx as i64)), Type::I64),
                    (bits, Type::I64),
                    (Operand::Constant(Constant::Int(i64::from(is_ref))), Type::I8),
                ],
                ret_ty: Type::Void,
            },
            Type::Void,
        );
    }

    /// The locals visible in the stub, in deterministic order: those bound to the
    /// environment of the closure (by position) and the others — the parameters — by
    /// the offset of the declaration.
    fn stub_locals(&self) -> (Vec<(SymbolId, Local)>, Vec<(SymbolId, Local)>) {
        let mut env = Vec::new();
        let mut others = Vec::new();
        let mut seen_set = HashSet::new();
        for scope in self.scopes.iter().rev() {
            let mut names: Vec<(&SymbolId, &Local)> = scope.iter().collect();
            names.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(**s).to_string()));
            for (s, l) in names {
                if !seen_set.insert(*s) {
                    continue;
                }
                match l.mode {
                    Mode::Environment { .. } => env.push((*s, l.clone())),
                    _ => others.push((*s, l.clone())),
                }
            }
        }
        env.sort_by_key(|(_, l)| match l.mode {
            Mode::Environment { index: idx_value, .. } => idx_value,
            _ => 0,
        });
        others.sort_by_key(|(s, l)| (l.offset, self.ctx.symbol_name(*s).to_string()));
        (env, others)
    }

    /// The body of an `async`, `sync*` or `async*` function: this builder is the
    /// stub (parameters and captures already declared); the body becomes
    /// `<symbol>$async` and its uniform entry. Finishes the stub.
    /// The type of the value/element from the return annotation of a local function
    /// (`Future<T> f() async`, `Iterable<T> g() sync*`, `FutureOr<T>`).
    fn async_annotation_value(&self, ast: &ast::Ast, a: ast::TypeId, ty: BodyType) -> Option<super::rti::Recipe> {
        let ast::TypeKind::Named { name, args } = &ast.ty(a).kind else { return None };
        let name_str = self.ctx.symbol_name(name.last()?.sym);
        let expected = match ty {
            BodyType::Async => name_str == "Future" || name_str == "FutureOr",
            BodyType::SyncStar => name_str == "Iterable",
            BodyType::AsyncStar => name_str == "Stream",
        };
        if !expected {
            return None;
        }
        self.annotation_recipe(ast.ty(*args.first()?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn lower_async_body(
        &mut self,
        ast: &ast::Ast,
        params: &[ast::Parameter],
        fn_body: &FunctionBody,
        span: Span,
        ret: Option<AsyncReturn>,
        ty: BodyType,
    ) {
        let (captures, parameters) = self.stub_locals();
        let body_symbol = format!("{}$async", self.func.symbol);
        let readable = self.func.name.clone();

        // --- corpo ------------------------------------------------------
        let mut b = FnBuilder::new(self.ctx, self.unit_id, body_symbol.clone(), readable.clone(), Type::Ref);
        let env = Operand::Val(b.add_param("env".to_string(), Type::Ref));
        let iterator = if ty == BodyType::SyncStar {
            Operand::Val(b.add_param("iterador".to_string(), Type::Ref))
        } else {
            Operand::Constant(Constant::Null)
        };
        let code = Operand::Val(b.add_param("codigo".to_string(), Type::Ref));
        let result = Operand::Val(b.add_param("resultado".to_string(), Type::Ref));
        let frame = b.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
        let state = b.read_position(frame.clone(), Q_STATE, Type::I64);
        let completer = b.read_position(frame.clone(), Q_COMPLETER, Type::Ref);
        let closer = b.read_position(frame.clone(), Q_BODY, Type::Ref);
        if self.this_param.is_some() {
            let t = b.read_position(frame.clone(), Q_THIS, Type::Ref);
            b.this_param = Some(t);
            b.finalizable_this = self.finalizable_this;
            b.enclosing_class = self.enclosing_class;
        }
        if !captures.is_empty() {
            let from_outside = b.read_position(frame.clone(), Q_ENV, Type::Ref);
            for (sym, l) in &captures {
                if let Mode::Environment { index: idx_value, cell, .. } = l.mode {
                    b.bind_env(*sym, from_outside.clone(), idx_value, cell, l.ty, l);
                }
            }
        }
        // RTI: the type variables of the stub (the tuple lives in the frame).
        b.function_type_params = self.function_type_params.clone();
        b.local_params = self.local_params.clone();
        b.this_extension = self.this_extension;
        b.this_ext_type = self.this_ext_type;
        b.member_class = self.member_class;
        b.class_by_tuple = self.class_by_tuple;
        if self.type_tuple.is_some() {
            let t = b.read_position(frame.clone(), Q_TUPLE, Type::I64);
            b.type_tuple = Some(t);
        }
        b.async_state = Some(Box::new(AsyncState {
            ty,
            iterator,
            frame: frame.clone(),
            body: closer,
            completer: completer.clone(),
            code,
            result,
            resumptions: Vec::new(),
            suspensions: Vec::new(),
            raw_return: false,
            waits: Vec::new(),
        }));
        // The symbolic stack trace (§13.14): the stub stays out of the stack trace (the VM has
        // a single frame for the function), and the resumed `async` body starts the
        // chain of awaiters through the frame, which it pushes on the stack trace
        // stack.
        if self.ctx.stack_trace {
            self.stack_trace.marks |= crate::hir::stack_trace_marks::HIDDEN;
            b.stack_trace.token = self.stack_trace.token;
            if matches!(ty, BodyType::Async | BodyType::AsyncStar) {
                b.stack_trace.marks |= crate::hir::stack_trace_marks::ASYNC_BODY;
                if ty == BodyType::AsyncStar {
                    b.stack_trace.marks |= crate::hir::stack_trace_marks::ASYNC_GENERATOR;
                }
                b.stack_trace.stack = Some((0, frame.clone()));
            }
        }
        let entry = b.current_block;
        let start = b.new_block();
        b.set_block(start);
        // The `alloca`s of the body become positions of the frame: no direct local
        // functions (the address of a local would not survive the swap).
        b.without_directs = true;
        b.prepare_captures(
            ast,
            super::capture::Root {
                parameters: params,
                body: Some(fn_body),
                initializers: &[],
            },
        );
        b.open_scope();
        for (k, (sym, l)) in parameters.iter().enumerate() {
            let v = b.read_position(frame.clone(), Q_PARAMS + k, l.ty);
            match l.offset {
                Some(off) => b.declare_variable(*sym, off, l.ty, v),
                None => b.declare_local_with_value(*sym, l.ty, v),
            }
            // The representation of the frame does not replace the type of the declaration.
            b.assign_semantic_type(*sym, l.static_type);
        }
        // The top handler: an exception nobody in the body caught completes the
        // `Future` with an error (`async`) or goes to the stream (`async*`). In
        // `sync*` it leaves the body, and `moveNext` handles it.
        let top = b.new_block();
        if ty != BodyType::SyncStar {
            b.exception_targets.push(top);
        }
        // `async*`: the body only starts when someone listens, and starts
        // cancelled if the subscription was already cancelled (`asyncStarBody(true)`).
        if ty == BodyType::AsyncStar {
            b.exit_if_cancelled();
        }
        match fn_body {
            FunctionBody::Block(s) => b.lower_stmt(ast, *s),
            FunctionBody::Expression(e) => {
                let r = b.lower_expr(ast, *e);
                b.route_return(Some(r));
            }
            _ => {}
        }
        if !b.is_terminated() {
            b.terminate(Terminator::Return(None));
        }
        if ty != BodyType::SyncStar {
            b.exception_targets.pop();
        }
        // The top handler has no handler: nothing in it goes back to
        // `_asyncReturn` (an exception return here is raw).
        if let Some(e) = b.async_state.as_mut() {
            e.raw_return = true;
        }
        b.set_block(top);
        let error = b.pending_exception();
        let stack_trace = b.pending_stack_trace();
        b.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        match ty {
            BodyType::Async => {
                b.call_support("_asyncRethrow", vec![error, stack_trace, completer], span);
            }
            BodyType::AsyncStar => {
                b.call_support("_asyncStarError", vec![completer, error, stack_trace], span);
            }
            // Without a top handler: the block is not reached.
            BodyType::SyncStar => {}
        }
        b.terminate_raw(Operand::Constant(Constant::Null));

        // The jump on the state, at the end of the entry block.
        let st = b.async_state.take().expect("estado async");
        let idx0 = b.func.blocks.iter().position(|x| x.id == entry).expect("entry");
        b.func.blocks[idx0].terminator = Terminator::Switch {
            val: state,
            default: start,
            cases: st.resumptions.clone(),
        };
        b.terminated_blocks.insert(entry);

        // The two passes: locals and temporaries that cross an `await`
        // go into the frame.
        let mut next = b.next_value;
        let locals_base = Q_PARAMS + parameters.len();
        let n_locals = convert_allocas(&mut b.func, &frame, locals_base, &mut next);
        let n_temp = save_live(&mut b.func, &frame, locals_base + n_locals, &st.suspensions, entry, &mut next);
        b.next_value = next;
        let size = locals_base + n_locals + n_temp;
        // The name of the body carries its fingerprint: a frame suspended
        // at an `await` keeps the state and the positions of this body, and on a
        // JIT reload it can only resume in an identical body. A body that
        // changed has another name, and the live frames continue in the code they
        // started in (like the active frames in the VM).
        let body_symbol = format!(
            "{body_symbol}$q{:08x}",
            (super::closures::hash_name(&format!("{size}|{:?}", b.func.blocks)) as u64) as u32
        );
        b.func.symbol = body_symbol.clone();
        if self.ctx.stack_trace && matches!(ty, BodyType::Async | BodyType::AsyncStar) {
            let n = st.resumptions.iter().map(|(k, _)| *k).max().unwrap_or(0).max(0) as usize;
            let mut waits = vec![(0u32, 0u32); n];
            for (k, p) in &st.waits {
                if let Some(x) = usize::try_from(*k).ok().and_then(|k| k.checked_sub(1)).and_then(|i| waits.get_mut(i)) {
                    *x = *p;
                }
            }
            b.stack_trace.async_body = Some(crate::hir::TraceAsyncBody { entry: format!("{body_symbol}$ent"), waits });
        }
        if self.ctx.arc_memory {
            // A Dart body called by the entry: the Ref return is Owned.
            b.dart_ref_returns.insert(body_symbol.clone());
        }
        self.absorb(b);

        // --- uniform entry of the body: ([iterator,] code, result) --
        let mut infos = Vec::new();
        if ty == BodyType::SyncStar {
            infos.push(super::closures::EntryParam {
                name: Some("iterador".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            });
        }
        infos.extend([
            super::closures::EntryParam {
                name: Some("codigo".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            },
            super::closures::EntryParam {
                name: Some("resultado".to_string()),
                kind: ast::ParameterKind::Required,
                required: true,
                pattern: super::closures::DefaultValue::Absent,
            },
        ]);
        let symbol_ent = format!("{body_symbol}$ent");
        let mut e = FnBuilder::new(self.ctx, self.unit_id, symbol_ent.clone(), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env_e = e.closure_context(clo);
        if let Some(vals) = e.unpack(&infos, args, desc) {
            let mut all = vec![env_e];
            all.extend(vals);
            let r = e.emit_call_with_check(
                Instruction::CallStatic {
                    symbol: body_symbol,
                    args: all,
                    ret_ty: Type::Ref,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(r)));
        }
        self.absorb(e);

        // --- the stub ---------------------------------------------------
        let mut fields: Vec<Operand> = vec![Operand::Constant(Constant::Int(0)); size];
        fields[Q_COMPLETER] = Operand::Constant(Constant::Null);
        fields[Q_BODY] = Operand::Constant(Constant::Null);
        fields[Q_THIS] = self.this_param.clone().map_or(Operand::Constant(Constant::Null), |t| self.coerce(t, Type::Ref));
        fields[Q_ENV] = captures
            .iter()
            .find_map(|(_, l)| match &l.mode {
                Mode::Environment { env, .. } => Some(env.clone()),
                _ => None,
            })
            .unwrap_or(Operand::Constant(Constant::Null));
        if let Some(t) = self.type_tuple.clone() {
            fields[Q_TUPLE] = t;
        }
        for (k, (_, l)) in parameters.iter().enumerate() {
            let v = self.read_local(l);
            fields[Q_PARAMS + k] = v;
        }
        let frame_s = self.emit(
            Instruction::AllocObject {
                class_id: ASYNC_FRAME_ID,
                fields,
            },
            Type::Ref,
        );
        if let Operand::Val(site) = frame_s {
            self.async_frame_layouts.push((self.func.symbol.clone(), site, symbol_ent.clone(), size));
        }
        // The `T` of the `Completer<T>`, of the `Iterable<T>` or of the `Stream<T>`: for
        // `async`, the value type of the declared `Future` (specification §9
        // "future value type": `Future<T>`/`FutureOr<T>` give `T`); for the
        // generators, the element type (§9 "element type"). The rest,
        // `dynamic`.
        let expected_class = match ty {
            BodyType::Async => self.ctx.core.future_class,
            BodyType::SyncStar => self.ctx.core.iterable_class,
            BodyType::AsyncStar => self.ctx.core.stream_class,
        };
        let val_ref: Option<super::rti::Recipe> = match ret {
            Some(AsyncReturn::Typed(t)) => match self.ctx.table.get(t) {
                dartforge_types::table::Type::Interface { class, args, .. } if Some(*class) == expected_class => {
                    args.first().map(|a| self.type_recipe(*a))
                }
                dartforge_types::table::Type::FutureOr { arg, .. } if ty == BodyType::Async => {
                    Some(self.type_recipe(*arg))
                }
                _ => None,
            },
            Some(AsyncReturn::Annotation(a)) => self.async_annotation_value(ast, a, ty),
            None => None,
        };
        let arm_tuple = |b: &mut Self| {
            let saved = b.armed_tuple.take();
            if let Some(v) = &val_ref {
                let t = b.recipe_rti(&super::rti::Recipe { text: format!("L<{}>", v.text), variables: v.variables });
                b.armed_tuple = Some(t);
            }
            saved
        };
        match ty {
            BodyType::Async => {
                let saved = arm_tuple(self);
                let completer_s = self.call_support("_makeAsyncAwaitCompleter", Vec::new(), span);
                self.armed_tuple = saved;
                let completer_s = self.coerce(completer_s, Type::Ref);
                self.store_position(frame_s.clone(), Q_COMPLETER, completer_s.clone());
                let body_s = self.close_body(frame_s.clone(), symbol_ent, span);
                let r = self.call_support("_asyncStartSync", vec![body_s, completer_s], span);
                self.terminate(Terminator::Return(Some(r)));
            }
            BodyType::AsyncStar => {
                let saved = arm_tuple(self);
                let controller = self.call_support("_makeAsyncStarController", Vec::new(), span);
                self.armed_tuple = saved;
                let controller = self.coerce(controller, Type::Ref);
                self.store_position(frame_s.clone(), Q_COMPLETER, controller.clone());
                let body_s = self.close_body(frame_s.clone(), symbol_ent, span);
                let r = self.call_support("_asyncStarStart", vec![controller, body_s], span);
                self.terminate(Terminator::Return(Some(r)));
            }
            BodyType::SyncStar => {
                // The frame of the stub is the template: each `iterator` starts the body
                // in a new frame, with its parameters (`$new`).
                let new_symbol = format!("{symbol_ent}$new");
                self.new_frame_function(&new_symbol, &symbol_ent, &parameters, size);
                let env_s = self.emit(Instruction::AllocEnv { values: vec![frame_s] }, Type::Ref);
                let new = self.emit(
                    Instruction::AllocClosure {
                        code_symbol: format!("{new_symbol}$ent"),
                        env: env_s,
                    },
                    Type::Ref,
                );
                let saved = arm_tuple(self);
                let r = self.call_support("_makeSyncStarIterable", vec![new], span);
                self.armed_tuple = saved;
                self.terminate(Terminator::Return(Some(r)));
            }
        }
    }

    /// The body closure over the frame, registered in the zone
    /// (`_wrapBody`) and kept in the frame.
    fn close_body(&mut self, frame: Operand, symbol_ent: String, span: Span) -> Operand {
        let env = self.emit(Instruction::AllocEnv { values: vec![frame.clone()] }, Type::Ref);
        let clo = self.emit(
            Instruction::AllocClosure {
                code_symbol: symbol_ent,
                env,
            },
            Type::Ref,
        );
        let fn_body = self.call_support("_wrapBody", vec![clo], span);
        let fn_body = self.coerce(fn_body, Type::Ref);
        self.store_position(frame, Q_BODY, fn_body.clone());
        fn_body
    }

    /// `sync*`: the argument-less closure `new_symbol` (the uniform entry
    /// `<new_symbol>$ent`), which copies from the template frame (its environment)
    /// `this`, the captures, the type tuple and the parameters into a new
    /// frame and returns the body closure over it — the VM's
    /// `_stateAtStart._clone()`.
    fn new_frame_function(
        &mut self,
        new_symbol: &str,
        body_symbol_ent: &str,
        parameters: &[(SymbolId, Local)],
        size: usize,
    ) {
        let readable = self.func.name.clone();
        let mut e = FnBuilder::new(self.ctx, self.unit_id, format!("{new_symbol}$ent"), readable, Type::Ref);
        let clo = Operand::Val(e.add_param("closure".to_string(), Type::Ref));
        let args = Operand::Val(e.add_param_vector(DartEntryVector::Arguments));
        let desc = Operand::Val(e.add_param_vector(DartEntryVector::Descriptor));
        e.declare_arc_uniform_return();
        let env = e.closure_context(clo);
        if e.unpack(&[], args, desc).is_some() {
            let model = e.emit(Instruction::EnvGet { env, index: 0 }, Type::Ref);
            let mut fields: Vec<Operand> = vec![Operand::Constant(Constant::Int(0)); size];
            fields[Q_COMPLETER] = Operand::Constant(Constant::Null);
            fields[Q_BODY] = Operand::Constant(Constant::Null);
            fields[Q_THIS] = e.read_position(model.clone(), Q_THIS, Type::Ref);
            fields[Q_ENV] = e.read_position(model.clone(), Q_ENV, Type::Ref);
            if self.type_tuple.is_some() {
                fields[Q_TUPLE] = e.read_position(model.clone(), Q_TUPLE, Type::I64);
            }
            for (k, (_, l)) in parameters.iter().enumerate() {
                fields[Q_PARAMS + k] = e.read_position(model.clone(), Q_PARAMS + k, l.ty);
            }
            let frame = e.emit(
                Instruction::AllocObject {
                    class_id: ASYNC_FRAME_ID,
                    fields,
                },
                Type::Ref,
            );
            if let Operand::Val(site) = frame {
                e.async_frame_layouts.push((e.func.symbol.clone(), site, body_symbol_ent.to_string(), size));
            }
            let env_q = e.emit(Instruction::AllocEnv { values: vec![frame] }, Type::Ref);
            let fn_body = e.emit(
                Instruction::AllocClosure {
                    code_symbol: body_symbol_ent.to_string(),
                    env: env_q,
                },
                Type::Ref,
            );
            e.terminate(Terminator::Return(Some(fn_body)));
        }
        self.absorb(e);
    }

    /// `async*`: ends the body (through the `finally`s) if the resumption value says
    /// that the subscription was cancelled.
    fn exit_if_cancelled(&mut self) {
        let result = self.async_state.as_ref().expect("async* body").result.clone();
        let cancelled = self.coerce(result, Type::I1);
        let exit = self.new_block();
        let follow = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: cancelled,
            then_block: exit,
            else_block: follow,
        });
        self.set_block(exit);
        self.route_return(None);
        self.set_block(follow);
    }

    /// `yield e` and `yield* e` (specification §18.16, §18.17) in a
    /// `sync*` or `async*` body.
    pub fn lower_yield(&mut self, ast: &ast::Ast, star: bool, e: ast::ExprId, span: Span) {
        let ty = self.async_state.as_ref().map(|e| e.ty);
        if !matches!(ty, Some(BodyType::SyncStar | BodyType::AsyncStar)) {
            self.unsupported("yield outside a generator", span);
            return;
        }
        let v = self.lower_expr(ast, e);
        let v = if matches!(self.operand_type(&v), Type::Void) {
            Operand::Constant(Constant::Null)
        } else {
            self.coerce(v, Type::Ref)
        };
        if self.is_terminated() {
            return;
        }
        let st = self.async_state.as_ref().expect("generator body");
        let (frame, iterator, controller) = (st.frame.clone(), st.iterator.clone(), st.completer.clone());
        let k = st.resumptions.len() as i64 + 1;
        if ty == Some(BodyType::SyncStar) {
            let support = if star { "_syncStarYieldStar" } else { "_syncStarYield" };
            self.call_support(support, vec![iterator, v], span);
            if self.is_terminated() {
                return;
            }
            self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
            let suspends = self.current_block;
            self.terminate_raw(Operand::Constant(Constant::Bool(true)));
            let resumption = self.register_resumption(k, suspends);
            // The exception of the iterator of a `yield*` comes back thrown here.
            let st = self.async_state.as_ref().expect("sync* body");
            let (code, result) = (st.code.clone(), st.result.clone());
            self.set_block(resumption);
            self.throw_if_error(code, result, span);
        } else {
            let support = if star { "_asyncStarAddStream" } else { "_asyncStarAdd" };
            let stop = self.call_support(support, vec![controller, v], span);
            if self.is_terminated() {
                return;
            }
            let stop = self.coerce(stop, Type::I1);
            let exit = self.new_block();
            let suspend = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: stop,
                then_block: exit,
                else_block: suspend,
            });
            self.set_block(exit);
            self.route_return(None);
            self.set_block(suspend);
            // The position of the `yield` (that of the statement): that of the frame of the generator
            // suspended here, in the stack trace.
            let position = self.position.unwrap_or((0, 0));
            if let Some(e) = self.async_state.as_mut() {
                e.waits.push((k, position));
            }
            self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
            let suspends = self.current_block;
            self.terminate_raw(Operand::Constant(Constant::Null));
            let resumption = self.register_resumption(k, suspends);
            self.set_block(resumption);
            self.exit_if_cancelled();
        }
    }

    /// `await for (target in s) body` (specification §18.6.3), lowered the way the
    /// VM kernel desugars it (`native_sdk/async/async_patch.dart`):
    /// `StreamIterator`, `while (await it.moveNext())`, and the `finally` that
    /// cancels the subscription if it still exists — through `break`, `return` or an
    /// exception in the body.
    pub fn lower_await_for(
        &mut self,
        ast: &ast::Ast,
        target: &ast::ForInTarget,
        iterable: ast::ExprId,
        body: ast::StmtId,
        span: Span,
    ) {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            self.unsupported("await for outside an async function", span);
            return;
        }
        let s = self.lower_expr(ast, iterable);
        let s = self.coerce(s, Type::Ref);
        let it = self.call_support("_awaitForIterator", vec![s], span);
        let it = self.coerce(it, Type::Ref);
        if self.is_terminated() {
            return;
        }
        let labels = std::mem::take(&mut self.pending_labels);
        let it_c = it.clone();
        let mut loop_fn = |b: &mut Self| {
            let head = b.new_block();
            let fn_body = b.new_block();
            let end = b.new_block();
            for &r in &labels {
                b.labeled_break_targets.insert(r, end);
                b.labeled_continue_targets.insert(r, head);
            }
            b.terminate(Terminator::Branch(head));
            b.set_block(head);
            let next_item = b.call_by_selector(it_c.clone(), "c:moveNext".to_string(), &[]);
            let ok = b.await_value(next_item, span);
            let ok = b.coerce(ok, Type::I1);
            b.terminate(Terminator::CondBranch { cond: ok, then_block: fn_body, else_block: end });
            b.set_block(fn_body);
            b.emit_safepoint();
            b.break_targets.push(end);
            b.continue_targets.push(head);
            b.open_scope();
            let x = b.call_by_selector(it_c.clone(), "g:current".to_string(), &[]);
            b.bind_for_in_target(ast, target, x, iterable, span);
            b.lower_stmt(ast, body);
            b.close_scope();
            b.terminate(Terminator::Branch(head));
            b.break_targets.pop();
            b.continue_targets.pop();
            for &r in &labels {
                b.labeled_break_targets.remove(&r);
                b.labeled_continue_targets.remove(&r);
            }
            b.set_block(end);
        };
        let mut cancel = |b: &mut Self| {
            let active = b.call_support("_awaitForActive", vec![it.clone()], span);
            let active = b.coerce(active, Type::I1);
            let yes = b.new_block();
            let after = b.new_block();
            b.terminate(Terminator::CondBranch { cond: active, then_block: yes, else_block: after });
            b.set_block(yes);
            let f = b.call_by_selector(it.clone(), "c:cancel".to_string(), &[]);
            b.await_value(f, span);
            b.terminate(Terminator::Branch(after));
            b.set_block(after);
        };
        self.lower_try_with(ast, &mut loop_fn, &[], Some(&mut cancel));
    }

    /// Creates the resumption block of state `k`, suspended in `suspends`.
    fn register_resumption(&mut self, k: i64, suspends: BlockId) -> BlockId {
        let resumption = self.new_block();
        let st = self.async_state.as_mut().expect("suspended body");
        st.resumptions.push((k, resumption));
        st.suspensions.push((suspends, resumption));
        resumption
    }

    /// On resumption: with `code == _ERROR`, throws the `_DfAsyncError` of
    /// `result` there; otherwise continues. Returns the block that branches to the
    /// error-free path (its single predecessor).
    fn throw_if_error(&mut self, code: Operand, result: Operand, span: Span) -> BlockId {
        // `code` arrives as an `int` in a `Ref` position (Smi).
        let c = self.coerce(code, Type::I64);
        let is_error = self.emit(
            Instruction::ICmp(ICmpOp::Eq, c, Operand::Constant(Constant::Int(1))),
            Type::I1,
        );
        let b_error = self.new_block();
        let b_ok = self.new_block();
        let origin = self.current_block;
        self.terminate(Terminator::CondBranch {
            cond: is_error,
            then_block: b_error,
            else_block: b_ok,
        });
        self.set_block(b_error);
        self.throw_async_error(result, span);
        self.set_block(b_ok);
        origin
    }

    /// A `Return` that does not go through `_asyncReturn`.
    fn terminate_raw(&mut self, v: Operand) {
        let before = self.async_state.as_ref().is_some_and(|e| e.raw_return);
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = true;
        }
        self.terminate(Terminator::Return(Some(v)));
        if let Some(e) = self.async_state.as_mut() {
            e.raw_return = before;
        }
    }

    /// The `return` of an `async` body (called by `terminate`): `_asyncReturn(v,
    /// completer)` and the raw ret. An exception from `_asyncReturn` (the `as T`
    /// of the value) goes to the top handler.
    pub fn async_return(&mut self, v: Option<Operand>) {
        let st = self.async_state.as_ref().expect("async body");
        let completer = st.completer.clone();
        match st.ty {
            BodyType::Async => {}
            BodyType::SyncStar => {
                self.terminate_raw(Operand::Constant(Constant::Bool(false)));
                return;
            }
            BodyType::AsyncStar => {
                self.call_support_raw("_asyncStarReturn", vec![completer]);
                self.terminate_raw(Operand::Constant(Constant::Null));
                return;
            }
        }
        let v = match v {
            Some(v) if !matches!(self.operand_type(&v), Type::Void) => self.coerce(v, Type::Ref),
            _ => Operand::Constant(Constant::Null),
        };
        self.call_support_raw("_asyncReturn", vec![v, completer]);
        self.terminate_raw(Operand::Constant(Constant::Null));
    }

    /// `await e` (specification §17.34): suspends until the `Future` completes.
    pub fn lower_await(&mut self, ast: &ast::Ast, e: ast::ExprId, span: Span) -> Operand {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            return self.unsupported("await outside an async function", span);
        }
        let v = self.lower_expr(ast, e);
        self.await_value(v, span)
    }

    /// `await` of an already evaluated value (that of `lower_await`, and those of
    /// `await for`).
    pub fn await_value(&mut self, v: Operand, span: Span) -> Operand {
        if !self.async_state.as_ref().is_some_and(|e| e.ty != BodyType::SyncStar) {
            return self.unsupported("await outside an async function", span);
        }
        let v = if matches!(self.operand_type(&v), Type::Void) {
            Operand::Constant(Constant::Null)
        } else {
            self.coerce(v, Type::Ref)
        };
        if self.is_terminated() {
            return Operand::Constant(Constant::Null);
        }
        let st = self.async_state.as_ref().expect("async body");
        let (frame, fn_body) = (st.frame.clone(), st.body.clone());
        let k = st.resumptions.len() as i64 + 1;
        // The position of the `await` (its `fileOffset`): that of the frame of whoever
        // waits here, in the stack trace.
        let position = self.position.unwrap_or((0, 0));
        if let Some(e) = self.async_state.as_mut() {
            e.waits.push((k, position));
        }
        self.store_position(frame, Q_STATE, Operand::Constant(Constant::Int(k)));
        self.call_support("_asyncAwait", vec![v, fn_body], span);
        let suspends = self.current_block;
        self.terminate_raw(Operand::Constant(Constant::Null));
        let resumption = self.register_resumption(k, suspends);
        let st = self.async_state.as_ref().expect("async body");
        let (code, result) = (st.code.clone(), st.result.clone());
        self.set_block(resumption);
        let origin = self.throw_if_error(code, result.clone(), span);
        // The resumed value is the `result` parameter, which each entry of the
        // body replaces: an SSA definition of its own comes out of it, here, so that
        // pass 2 (`save_live`) takes it into the frame when it crosses
        // another suspension point (`[await a, await b]`, `f(await a, await
        // b)`, `await a + await b`, `'${await a}${await b}'`…). The
        // parameters of the body stay out of that pass.
        self.emit(
            Instruction::Phi {
                incoming: vec![(origin, result)],
                ty: Type::Ref,
            },
            Type::Ref,
        )
    }

    /// Throws, at the point of the `await`, the error of an `_DfAsyncError` with
    /// its stack trace.
    fn throw_async_error(&mut self, val_ref: Operand, span: Span) {
        let fields = self.async_error_fields();
        let Some((ve, vr)) = fields else {
            self.unsupported("missing async_patch class `_DfAsyncError`", span);
            return;
        };
        let error = self.read_field(val_ref.clone(), ve, span);
        let stack_trace = self.read_field(val_ref, vr, span);
        self.emit_throw_value(error, Some(stack_trace));
        self.branch_to_handler();
    }

    /// The `error` and `stack_trace` fields of `_DfAsyncError`.
    fn async_error_fields(
        &self,
    ) -> Option<(dartforge_elements::model::VariableId, dartforge_elements::model::VariableId)> {
        let lib = self.ctx.program.libraries.iter().position(|l| l.uri == "dart:async")?;
        let sym = self.ctx.interner.lookup("_DfAsyncError")?;
        let b = self.ctx.program.lookup(dartforge_elements::model::LibraryId(lib as u32), sym)?;
        let dartforge_elements::model::Element::Class(c) = b.getter? else {
            return None;
        };
        let class = &self.ctx.program.classes[c.0 as usize];
        let field = |n: &str| {
            class
                .fields
                .iter()
                .copied()
                .find(|v| self.ctx.symbol_name(self.ctx.program.variables[v.0 as usize].name) == n)
        };
        Some((field("err")?, field("trace")?))
    }

    /// After leaving a pending exception: the branch of `emit_throw_op`
    /// (current handler, `finally` or exit).
    fn branch_to_handler(&mut self) {
        let curr_b = self.current_block;
        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::Branch(exc_target));
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        } else {
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
        }
        let dead = self.new_block();
        self.set_block(dead);
    }
}

// ---------------------------------------------------------------------------
// The passes over the HIR of the body.

/// The type the emitter gives each value (that of `LlvmEmitter`).
fn function_types(func: &Function) -> HashMap<ValueId, Type> {
    let mut t = HashMap::new();
    for (v, _, ty) in &func.params {
        t.insert(*v, *ty);
    }
    for b in &func.blocks {
        for (v, inst, ty) in &b.instructions {
            t.insert(*v, crate::llvm::LlvmEmitter::result_type(inst, *ty));
        }
    }
    t
}

fn new_value(next: &mut u32) -> ValueId {
    let v = ValueId(*next);
    *next += 1;
    v
}

/// Instructions that read position `slot` of the frame into value `dest`
/// in representation `ty`.
fn read_from_frame(frame: &Operand, slot: usize, ty: Type, dest: ValueId, next: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let get = |ret: Type| Instruction::CallRuntime {
        name: "dartforge_object_get".to_string(),
        args: vec![(frame.clone(), Type::Ref), (Operand::Constant(Constant::Int(slot as i64)), Type::I64)],
        ret_ty: ret,
    };
    match ty {
        // Restoring locals and SSA values uses the same explicit representation
        // as the header reads. Only the ARC preparation chooses the audited
        // borrow; tracing keeps its physical access through the emitter.
        Type::Ref => vec![(dest, Instruction::GetField { object: frame.clone(), index: slot }, Type::Ref)],
        Type::I64 => vec![(dest, get(ty), ty)],
        Type::F64 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::Bitcast { op: Operand::Val(t), to: Type::F64 }, Type::F64),
            ]
        }
        Type::I1 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::ICmp(ICmpOp::Ne, Operand::Val(t), Operand::Constant(Constant::Int(0))), Type::I1),
            ]
        }
        Type::I8 => {
            let t = new_value(next);
            vec![
                (t, get(Type::I64), Type::I64),
                (dest, Instruction::Trunc { op: Operand::Val(t), from: Type::I64, to: Type::I8 }, Type::I8),
            ]
        }
        Type::Void | Type::Ptr => Vec::new(),
        // Unboxed SIMD vectors only exist in synchronous functions
        // (`lower/simd.rs` does not produce them in an asynchronous body).
        Type::V4F32 | Type::V4I32 | Type::V2F64 => unreachable!("unboxed SIMD vector in an asynchronous frame"),
    }
}

#[cfg(test)]
mod ref_restoration_tests {
    use super::*;
    use crate::optimize::arc::*;

    #[test]
    fn ref_restored_from_frame_gets_keepalive_before_mutating_child() {
        let mut next = 2;
        let mut instructions = read_from_frame(&Operand::Val(ValueId(0)), 40, Type::Ref, ValueId(1), &mut next);
        instructions.push((ValueId(2), Instruction::SetField {
            object: Operand::Val(ValueId(1)), index: 2, value: Operand::Constant(Constant::Int(42)),
        }, Type::Void));
        let f = Function { symbol: "restaurar".into(), name: "restaurar".into(), debug_info: None,
            params: vec![(ValueId(0), "quadro".into(), Type::Ref)], return_ty: Type::Ref,
            blocks: vec![BasicBlock { id: BlockId(0), instructions,
                terminator: Terminator::Return(Some(Operand::Val(ValueId(1)))) }] };
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions.push(f.clone());
        m.arc_field_layouts.insert(123, vec![Type::Ref, Type::I64, Type::I64]);
        m.classes.push(ClassDef { id: 123, name: "Filho".into(), field_count: 3,
            vtable: vec![], to_string_symbol: None });
        let mut p = DartFunctionPlan::default();
        p.tokens.ret = TokenReturn::Owned;
        let mut plans = HashMap::from([("restaurar".into(), p)]);
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (1, 0));
        let reader = m.functions[0].blocks.iter().flat_map(|b| &b.instructions).find_map(|(v, i, _)|
            matches!(i, Instruction::CallRuntime { name, .. } if name == "dartforge_arc_read_field_ref_v1").then_some(*v)).unwrap();
        assert_eq!(plans["restaurar"].classes[&reader], Ownership::Borrowed {
            owner: OwnerOrigin::Value(ValueId(0)), scope: 0,
        });
        assert_eq!(plans["restaurar"].classes[&ValueId(1)], Ownership::Owned);
        let ir = crate::llvm::LlvmEmitter::new(&m).emit_all();
        assert!(ir.contains("i64 %v0, i64 40)"));
        assert!(ir.find("call void @dartforge_arc_retain").unwrap()
            < ir.find("call void @dartforge_arc_write_field_scalar_checked_v1").unwrap());
        assert_eq!(prepare_arc_dart_module(&mut m, &mut plans).unwrap(), (0, 0));
        // The same typed access can be emitted under tracing; the ARC tokens appear
        // only in the explicitly selected preparation above.
        let mut tracing = Module::new();
        tracing.functions.push(f);
        let mut without_plans = HashMap::new();
        let before = format!("{tracing:?}");
        assert_eq!(prepare_arc_dart_module(&mut tracing, &mut without_plans).unwrap(), (0, 0));
        assert_eq!(format!("{tracing:?}"), before);
        let ir = crate::llvm::LlvmEmitter::new(&tracing).emit_all();
        assert!(!ir.contains("call i64 @dartforge_arc_read_field_ref_v1"));
    }
}

/// Instructions that write `value` (in representation `ty`) into position `slot`.
fn store_in_frame(frame: &Operand, slot: usize, val_ref: Operand, ty: Type, next: &mut u32) -> Vec<(ValueId, Instruction, Type)> {
    let mut exit = Vec::new();
    let (bits, is_ref) = match ty {
        Type::Ref => (val_ref, 1),
        Type::I64 => (val_ref, 0),
        Type::F64 => {
            let t = new_value(next);
            exit.push((t, Instruction::Bitcast { op: val_ref, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::I1 | Type::I8 => {
            let t = new_value(next);
            exit.push((t, Instruction::ZExt { op: val_ref, from: ty, to: Type::I64 }, Type::I64));
            (Operand::Val(t), 0)
        }
        Type::Void | Type::Ptr => return exit,
        Type::V4F32 | Type::V4I32 | Type::V2F64 => unreachable!("unboxed SIMD vector in an asynchronous frame"),
    };
    exit.push((
        new_value(next),
        Instruction::CallRuntime {
            name: "dartforge_object_set".to_string(),
            args: vec![
                (frame.clone(), Type::Ref),
                (Operand::Constant(Constant::Int(slot as i64)), Type::I64),
                (bits, Type::I64),
                (Operand::Constant(Constant::Int(is_ref)), Type::I8),
            ],
            ret_ty: Type::Void,
        },
        Type::Void,
    ));
    exit
}

/// Pass 1: each `alloca` becomes a position of the frame starting at `base`.
/// Returns how many positions it used.
fn convert_allocas(func: &mut Function, frame: &Operand, base: usize, next: &mut u32) -> usize {
    let mut position: HashMap<ValueId, (usize, Type)> = HashMap::new();
    for b in &func.blocks {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Alloca(t) = inst {
                let s = base + position.len();
                position.insert(*v, (s, *t));
            }
        }
    }
    if position.is_empty() {
        return 0;
    }
    let types = function_types(func);
    let type_of = |op: &Operand| match op {
        Operand::Val(v) => types.get(v).copied().unwrap_or(Type::I64),
        Operand::Constant(Constant::Int(_)) => Type::I64,
        Operand::Constant(Constant::Double(_)) => Type::F64,
        Operand::Constant(Constant::Bool(_)) => Type::I1,
        Operand::Constant(_) => Type::Ref,
    };
    for b in &mut func.blocks {
        let old_ones = std::mem::take(&mut b.instructions);
        for (v, inst, ty) in old_ones {
            match &inst {
                Instruction::Alloca(_) if position.contains_key(&v) => {}
                Instruction::Load { ptr: Operand::Val(p), ty: t } if position.contains_key(p) => {
                    let (s, _) = position[p];
                    b.instructions.extend(read_from_frame(frame, s, *t, v, next));
                }
                Instruction::Store { ptr: Operand::Val(p), val } if position.contains_key(p) => {
                    let (s, t) = position[p];
                    // The value arrives in the representation of the local (R6, already
                    // coerced by the lowering); only the width may differ.
                    let store_ty = if t == Type::Ref { Type::Ref } else { type_of(val) };
                    b.instructions.extend(store_in_frame(frame, s, val.clone(), store_ty, next));
                }
                _ => b.instructions.push((v, inst, ty)),
            }
        }
    }
    position.len()
}

/// The values read by an instruction.
pub(crate) fn uses_of(inst: &Instruction) -> Vec<ValueId> {
    let mut u = Vec::new();
    let mut op = |o: &Operand| {
        if let Operand::Val(v) = o {
            u.push(*v);
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) => {}
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::SDiv(a, b)
        | Instruction::SRem(a, b)
        | Instruction::Shl(a, b)
        | Instruction::AShr(a, b)
        | Instruction::LShr(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::FAdd(a, b)
        | Instruction::FSub(a, b)
        | Instruction::FMul(a, b)
        | Instruction::FDiv(a, b)
        | Instruction::ICmp(_, a, b)
        | Instruction::FCmp(_, a, b) => {
            op(a);
            op(b);
        }
        Instruction::Neg(a)
        | Instruction::Not(a)
        | Instruction::FNeg(a)
        | Instruction::LNot(a)
        | Instruction::IntToDouble(a)
        | Instruction::DoubleToInt(a)
        | Instruction::CheckNotNull(a) => op(a),
        Instruction::ZExt { op: a, .. }
        | Instruction::ArcCopy { value: a }
        | Instruction::ArcDrop { value: a }
        | Instruction::ArcMove { value: a }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => op(a),
        Instruction::AllocObject { fields, .. } => fields.iter().for_each(&mut op),
        Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
            elements.iter().for_each(|(a, _)| op(a))
        }
        Instruction::ArcLoadStrong { slot: StrongSlot::Frame { frame, .. } } => op(frame),
        Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } } => {},
        Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, value, .. } => op(value),
        Instruction::ArcStoreStrong { slot: StrongSlot::Frame { frame, .. }, value, .. } => {
            op(frame);
            op(value);
        }
        Instruction::AllocCell { value } => op(value),
        Instruction::AllocEnv { values } => values.iter().for_each(&mut op),
        Instruction::ConcatStrings { parts } => parts.iter().for_each(&mut op),
        Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. } => op(env),
        Instruction::TypedCall { target, args, .. } => {
            op(target);
            args.iter().for_each(|(a, _)| op(a));
        }
        Instruction::Load { ptr, .. } => op(ptr),
        Instruction::Store { ptr, val } => {
            op(ptr);
            op(val);
        }
        Instruction::GetField { object, .. } => op(object),
        Instruction::SetField { object, value, .. } => {
            op(object);
            op(value);
        }
        Instruction::GetListElement { list, index } => {
            op(list);
            op(index);
        }
        Instruction::SetListElement { list, index, value } => {
            op(list);
            op(index);
            op(value);
        }
        Instruction::CellGet { cell } => op(cell),
        Instruction::CellSet { cell, value } => {
            op(cell);
            op(value);
        }
        Instruction::EnvGet { env, .. } => op(env),
        Instruction::CallStatic { args, .. } => args.iter().for_each(&mut op),
        Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
            op(receiver);
            args.iter().for_each(&mut op);
        }
        Instruction::CallClosure { closure, args, tuple_types, .. } => {
            op(closure);
            args.iter().for_each(&mut op);
            op(tuple_types);
        }
        Instruction::CallRuntime { args, .. } | Instruction::DeclaredFieldCall { args, .. } => args.iter().for_each(|(a, _)| op(a)),
        Instruction::IsClass { object, .. } => op(object),
        Instruction::StoreGlobal { val, .. } => op(val),
        Instruction::Phi { .. } => {}
        Instruction::LoadIndexed { base, index } => {
            op(base);
            op(index);
        }
        Instruction::CallSelector { recv, args, tuple_types, .. } => {
            op(recv);
            args.iter().for_each(&mut op);
            op(tuple_types);
        }
        Instruction::CallClosureForward { closure, args, desc, .. } => {
            op(closure);
            op(args);
            op(desc);
        }
        Instruction::CallSelectorForward { recv, args, desc, .. } => {
            op(recv);
            op(args);
            op(desc);
        }
        Instruction::NativeCall { target, args, .. } => {
            op(target);
            args.iter().for_each(|(a, _)| op(a));
        }
        Instruction::NativeLoad { address, index: idx_value, .. } => {
            op(address);
            op(idx_value);
        }
        Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
            op(address);
            op(idx_value);
            op(val_ref);
        }
        Instruction::CompositeNativeCall { target, args, dest, .. } => {
            op(target);
            args.iter().for_each(|(a, _)| op(a));
            dest.iter().for_each(&mut op);
        }
        Instruction::Simd { args, .. } => args.iter().for_each(&mut op),
    }
    u
}

/// Replaces each use of `of` with `to` in an instruction (without the `phi`s).
fn replace_uses(inst: &mut Instruction, swap: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = swap(*v)
        {
            *v = n;
        }
    };
    match inst {
        Instruction::Const(_) | Instruction::Alloca(_) | Instruction::LoadGlobal { .. } | Instruction::TearOff { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) | Instruction::Phi { .. } => {}
        Instruction::Add(a, b)
        | Instruction::Sub(a, b)
        | Instruction::Mul(a, b)
        | Instruction::SDiv(a, b)
        | Instruction::SRem(a, b)
        | Instruction::Shl(a, b)
        | Instruction::AShr(a, b)
        | Instruction::LShr(a, b)
        | Instruction::And(a, b)
        | Instruction::Or(a, b)
        | Instruction::Xor(a, b)
        | Instruction::FAdd(a, b)
        | Instruction::FSub(a, b)
        | Instruction::FMul(a, b)
        | Instruction::FDiv(a, b)
        | Instruction::ICmp(_, a, b)
        | Instruction::FCmp(_, a, b) => {
            t(a);
            t(b);
        }
        Instruction::Neg(a)
        | Instruction::Not(a)
        | Instruction::FNeg(a)
        | Instruction::LNot(a)
        | Instruction::IntToDouble(a)
        | Instruction::DoubleToInt(a)
        | Instruction::CheckNotNull(a) => t(a),
        Instruction::ZExt { op: a, .. }
        | Instruction::ArcCopy { value: a }
        | Instruction::ArcDrop { value: a }
        | Instruction::ArcMove { value: a }
        | Instruction::Trunc { op: a, .. }
        | Instruction::Bitcast { op: a, .. }
        | Instruction::Box { op: a, .. }
        | Instruction::Unbox { op: a, .. } => t(a),
        Instruction::AllocObject { fields, .. } => fields.iter_mut().for_each(t),
        Instruction::AllocList { elements } | Instruction::AllocRecord { elements } => {
            elements.iter_mut().for_each(|(a, _)| t(a))
        }
        Instruction::ArcLoadStrong { slot: StrongSlot::Frame { frame, .. } } => t(frame),
        Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } } => {},
        Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, value, .. } => t(value),
        Instruction::ArcStoreStrong { slot: StrongSlot::Frame { frame, .. }, value, .. } => {
            t(frame);
            t(value);
        }
        Instruction::AllocCell { value } => t(value),
        Instruction::AllocEnv { values } => values.iter_mut().for_each(t),
        Instruction::ConcatStrings { parts } => parts.iter_mut().for_each(t),
        Instruction::AllocClosure { env, .. } | Instruction::AllocClosureTyped { env, .. } => t(env),
        Instruction::TypedCall { target, args, .. } => {
            t(target);
            args.iter_mut().for_each(|(a, _)| t(a));
        }
        Instruction::Load { ptr, .. } => t(ptr),
        Instruction::Store { ptr, val } => {
            t(ptr);
            t(val);
        }
        Instruction::GetField { object, .. } => t(object),
        Instruction::SetField { object, value, .. } => {
            t(object);
            t(value);
        }
        Instruction::GetListElement { list, index } => {
            t(list);
            t(index);
        }
        Instruction::SetListElement { list, index, value } => {
            t(list);
            t(index);
            t(value);
        }
        Instruction::CellGet { cell } => t(cell),
        Instruction::CellSet { cell, value } => {
            t(cell);
            t(value);
        }
        Instruction::EnvGet { env, .. } => t(env),
        Instruction::CallStatic { args, .. } => args.iter_mut().for_each(t),
        Instruction::CallInterface { receiver, args, .. } | Instruction::CallDynamic { receiver, args, .. } => {
            t(receiver);
            args.iter_mut().for_each(t);
        }
        Instruction::CallClosure { closure, args, tuple_types, .. } => {
            t(closure);
            args.iter_mut().for_each(t);
            t(tuple_types);
        }
        Instruction::CallRuntime { args, .. } | Instruction::DeclaredFieldCall { args, .. } => args.iter_mut().for_each(|(a, _)| t(a)),
        Instruction::IsClass { object, .. } => t(object),
        Instruction::StoreGlobal { val, .. } => t(val),
        Instruction::LoadIndexed { base, index } => {
            t(base);
            t(index);
        }
        Instruction::CallSelector { recv, args, tuple_types, .. } => {
            t(recv);
            args.iter_mut().for_each(t);
            t(tuple_types);
        }
        Instruction::CallClosureForward { closure, args, desc, .. } => {
            t(closure);
            t(args);
            t(desc);
        }
        Instruction::CallSelectorForward { recv, args, desc, .. } => {
            t(recv);
            t(args);
            t(desc);
        }
        Instruction::NativeCall { target, args, .. } => {
            t(target);
            args.iter_mut().for_each(|(a, _)| t(a));
        }
        Instruction::NativeLoad { address, index: idx_value, .. } => {
            t(address);
            t(idx_value);
        }
        Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
            t(address);
            t(idx_value);
            t(val_ref);
        }
        Instruction::CompositeNativeCall { target, args, dest, .. } => {
            t(target);
            args.iter_mut().for_each(|(a, _)| t(a));
            if let Some(d) = dest { t(d) }
        }
        Instruction::Simd { args, .. } => args.iter_mut().for_each(t),
    }
}

pub(crate) fn terminator_uses(term: &Terminator) -> Vec<ValueId> {
    let op = |o: &Operand| match o {
        Operand::Val(v) => Some(*v),
        _ => None,
    };
    match term {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => op(o).into_iter().collect(),
        Terminator::CondBranch { cond, .. } => op(cond).into_iter().collect(),
        Terminator::Switch { val, .. } => op(val).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn replace_in_terminator(term: &mut Terminator, swap: &dyn Fn(ValueId) -> Option<ValueId>) {
    let t = |o: &mut Operand| {
        if let Operand::Val(v) = o
            && let Some(n) = swap(*v)
        {
            *v = n;
        }
    };
    match term {
        Terminator::Return(Some(o)) | Terminator::Throw(o) => t(o),
        Terminator::CondBranch { cond, .. } => t(cond),
        Terminator::Switch { val, .. } => t(val),
        _ => {}
    }
}

pub(crate) fn successors(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Branch(b) => vec![*b],
        Terminator::CondBranch { then_block, else_block, .. } => vec![*then_block, *else_block],
        Terminator::Switch { default, cases, .. } => {
            let mut v = vec![*default];
            v.extend(cases.iter().map(|(_, b)| *b));
            v
        }
        _ => Vec::new(),
    }
}

/// Pass 2: the SSA values live at the entry of a resumption go into the
/// frame starting at `base` (written after the definition, reread before
/// each use). Returns how many positions it used.
fn save_live(
    func: &mut Function,
    frame: &Operand,
    base: usize,
    suspensions: &[(BlockId, BlockId)],
    entry: BlockId,
    next: &mut u32,
) -> usize {
    if suspensions.is_empty() {
        return 0;
    }
    let idx: HashMap<BlockId, usize> = func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let n = func.blocks.len();
    let mut defs: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut uses: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    // Uses in a `phi` are attributed to the end of the origin block.
    let mut end_uses: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    for (i, b) in func.blocks.iter().enumerate() {
        for (v, inst, _) in &b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origin, o) in incoming {
                    if let (Operand::Val(x), Some(&j)) = (o, idx.get(origin)) {
                        end_uses[j].insert(*x);
                    }
                }
            } else {
                for u in uses_of(inst) {
                    if !defs[i].contains(&u) {
                        uses[i].insert(u);
                    }
                }
            }
            defs[i].insert(*v);
        }
        for u in terminator_uses(&b.terminator) {
            if !defs[i].contains(&u) {
                uses[i].insert(u);
            }
        }
    }
    let mut succ: Vec<Vec<usize>> = func
        .blocks
        .iter()
        .map(|b| successors(&b.terminator).iter().filter_map(|s| idx.get(s).copied()).collect())
        .collect();
    for (a, r) in suspensions {
        if let (Some(&ia), Some(&ir)) = (idx.get(a), idx.get(r)) {
            succ[ia].push(ir);
        }
    }
    let mut live_in: Vec<HashSet<ValueId>> = vec![HashSet::new(); n];
    let mut changed = true;
    while changed {
        changed = false;
        for i in (0..n).rev() {
            let mut out: HashSet<ValueId> = end_uses[i].clone();
            for &s in &succ[i] {
                out.extend(live_in[s].iter().copied());
            }
            let mut inn: HashSet<ValueId> = uses[i].clone();
            for x in out {
                if !defs[i].contains(&x) {
                    inn.insert(x);
                }
            }
            if inn.len() != live_in[i].len() {
                live_in[i] = inn;
                changed = true;
            }
        }
    }
    let params: HashSet<ValueId> = func.params.iter().map(|p| p.0).collect();
    let from_entry = idx.get(&entry).map(|&i| defs[i].clone()).unwrap_or_default();
    let mut save: Vec<ValueId> = suspensions
        .iter()
        .filter_map(|(_, r)| idx.get(r))
        .flat_map(|&i| live_in[i].iter().copied())
        .filter(|v| !params.contains(v) && !from_entry.contains(v))
        .collect();
    let types = function_types(func);
    save.retain(|v| !matches!(types.get(v), Some(Type::Void | Type::Ptr) | None));
    save.sort_by_key(|v| v.0);
    save.dedup();
    if save.is_empty() {
        return 0;
    }
    let position: HashMap<ValueId, (usize, Type)> = save
        .iter()
        .enumerate()
        .map(|(k, v)| (*v, (base + k, types[v])))
        .collect();

    // The pending guard at the end of a block (call, read of the
    // pending state, comparison and branch): the shape the ARC preparation
    // recognizes. Nothing goes between the call and the branch.
    fn guard(b: &BasicBlock) -> Option<(usize, BlockId)> {
        let n = b.instructions.len();
        let Terminator::CondBranch { cond: Operand::Val(c), else_block, .. } = &b.terminator else { return None };
        let [(p, Instruction::CallRuntime { name, .. }, _), (ic, Instruction::ICmp(ICmpOp::Ne, Operand::Val(pv), _), _)] =
            b.instructions.get(n.checked_sub(2)?..)? else { return None };
        (name == "dartforge_exception_pending" && pv == p && ic == c && n >= 3).then(|| (n - 3, *else_block))
    }
    // Writes of the result of a guarded call: they go to the success
    // edge (a new block), after the guard.
    let mut on_edges: Vec<(BlockId, BlockId, Vec<(ValueId, Instruction, Type)>)> = Vec::new();
    for b in &mut func.blocks {
        let saved = guard(b);
        // Rereads at the end of the block for the `phi` entries of the successors
        // are done in a second round (below).
        let old_ones = std::mem::take(&mut b.instructions);
        let n_phis = old_ones.iter().take_while(|(_, i, _)| matches!(i, Instruction::Phi { .. })).count();
        let mut new_ones: Vec<(ValueId, Instruction, Type)> = Vec::with_capacity(old_ones.len());
        let mut after_phis: Vec<(ValueId, Instruction, Type)> = Vec::new();
        for (k, (v, mut inst, ty)) in old_ones.into_iter().enumerate() {
            if !matches!(inst, Instruction::Phi { .. }) {
                // Reread of each saved value the instruction uses.
                let mut swaps: HashMap<ValueId, ValueId> = HashMap::new();
                for u in uses_of(&inst) {
                    if let Some(&(s, t)) = position.get(&u)
                        && !swaps.contains_key(&u)
                    {
                        let nv = new_value(next);
                        new_ones.extend(read_from_frame(frame, s, t, nv, next));
                        swaps.insert(u, nv);
                    }
                }
                if !swaps.is_empty() {
                    replace_uses(&mut inst, &|x| swaps.get(&x).copied());
                }
            }
            let is_phi = matches!(inst, Instruction::Phi { .. });
            new_ones.push((v, inst, ty));
            if let Some(&(s, t)) = position.get(&v) {
                let writes = store_in_frame(frame, s, Operand::Val(v), t, next);
                if is_phi {
                    after_phis.extend(writes);
                } else if let Some((idx_value, success)) = saved
                    && idx_value == k
                {
                    on_edges.push((b.id, success, writes));
                } else {
                    new_ones.extend(writes);
                }
            }
            if k + 1 == n_phis {
                new_ones.append(&mut after_phis);
            }
        }
        new_ones.append(&mut after_phis);
        // The terminator.
        let mut swaps: HashMap<ValueId, ValueId> = HashMap::new();
        for u in terminator_uses(&b.terminator) {
            if let Some(&(s, t)) = position.get(&u)
                && !swaps.contains_key(&u)
            {
                let nv = new_value(next);
                new_ones.extend(read_from_frame(frame, s, t, nv, next));
                swaps.insert(u, nv);
            }
        }
        if !swaps.is_empty() {
            replace_in_terminator(&mut b.terminator, &|x| swaps.get(&x).copied());
        }
        b.instructions = new_ones;
    }
    let mut next_block = func.blocks.iter().map(|b| b.id.0).max().map_or(0, |m| m + 1);
    for (origin, success, writes) in on_edges {
        let edge = BlockId(next_block);
        next_block += 1;
        for b in &mut func.blocks {
            if b.id == origin
                && let Terminator::CondBranch { else_block, .. } = &mut b.terminator
            {
                *else_block = edge;
            }
            if b.id == success {
                for (_, inst, _) in &mut b.instructions {
                    if let Instruction::Phi { incoming, .. } = inst {
                        for (of, _) in incoming.iter_mut().filter(|(of, _)| *of == origin) {
                            *of = edge;
                        }
                    }
                }
            }
        }
        func.blocks.push(BasicBlock { id: edge, instructions: writes, terminator: Terminator::Branch(success) });
    }
    // `phi` entries with a saved value: reread at the end of the origin
    // block (before its terminator; before the guarded call, which does not
    // write into the frame).
    let mut rereads: Vec<(BlockId, ValueId, ValueId)> = Vec::new(); // (origem, antigo, novo)
    for b in &mut func.blocks {
        for (_, inst, _) in &mut b.instructions {
            if let Instruction::Phi { incoming, .. } = inst {
                for (origin, o) in incoming.iter_mut() {
                    if let Operand::Val(x) = o
                        && position.contains_key(x)
                    {
                        let nv = new_value(next);
                        rereads.push((*origin, *x, nv));
                        *x = nv;
                    }
                }
            }
        }
    }
    for (origin, old, new) in rereads {
        if let Some(b) = func.blocks.iter_mut().find(|b| b.id == origin) {
            let (s, t) = position[&old];
            let read = read_from_frame(frame, s, t, new, next);
            match guard(b) {
                Some((idx_value, _)) => {
                    b.instructions.splice(idx_value..idx_value, read);
                }
                None => b.instructions.extend(read),
            }
        }
    }
    save.len()
}

#[cfg(test)]
mod frame_abi_tests {
    use super::*;

    #[test]
    fn actual_async_and_generator_allocators_publish_per_resume_body_layouts() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let directory = tempfile::tempdir().unwrap();
            let entry = directory.path().join("main.dart");
            std::fs::write(&entry, r#"
import 'dart:async';
Future<int> one(int x) async { final saved = x + 1; await Future<void>.value(); return saved; }
Iterable<int> two(int x) sync* { yield x; yield x + 1; }
Stream<int> three(int x) async* { yield x; await Future<void>.value(); yield x + 1; }
void main() { one(1); two(2).toList(); three(3).listen(print); }
"#).unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = dartforge_intern::Interner::new();
            let (program, diagnostics) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let mut table = dartforge_types::TypeTable::new();
            let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
            let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
            let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk)
                .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
            let (bodies, _) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
            let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
            let module = crate::lower::lower_program(&context);
            assert!(module.errors.is_empty(), "{:?}", module.errors);
            assert_eq!(module.async_frame_layouts.len(), 4, "async, async*, sync* template and iterator clone");
            for (owner, site, body, count) in &module.async_frame_layouts {
                let allocator = module.functions.iter().find(|f| &f.symbol == owner).expect("defined allocator");
                assert!(module.functions.iter().any(|f| &f.symbol == body), "defined resume entry {body}");
                assert!(allocator.blocks.iter().flat_map(|b| &b.instructions).any(|(id, instruction, _)| {
                    id == site && matches!(instruction, Instruction::AllocObject {class_id, fields} if *class_id == ASYNC_FRAME_ID && fields.len() == *count)
                }), "actual allocator {owner}");
            }
            let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
            for (owner, site, body, count) in &module.async_frame_layouts {
                assert!(ir.contains(&format!("; df.async-frame {owner} v{} {body} {count}\n", site.0)));
            }
        }).unwrap().join().unwrap();
    }
}
