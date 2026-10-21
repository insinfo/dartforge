//! Builder of HIR functions during the lowering.

use crate::context::Context;
use crate::hir::*;
use dartforge_frontend::ast::{self, BinaryOp, ExprId};
use dartforge_intern::SymbolId;
use dartforge_types::resolved::LocalId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FinallyScope {
    pub entry_block: BlockId,
    pub resume_block: BlockId,
    pub reason_phi: ValueId,
    pub ret_val_phi: ValueId,
    pub incoming: Vec<(BlockId, i64, Operand)>,
    /// How many unlabeled `break`/`continue` targets existed when the
    /// `try` started: a jump to one of them crosses this `finally`.
    pub prof_break: usize,
    pub prof_continue: usize,
    /// Labels that already existed when the `try` started (the outer ones).
    pub break_labels: std::collections::HashSet<SymbolId>,
    pub continue_labels: std::collections::HashSet<SymbolId>,
    /// Jumps (`break`/`continue`, with the label) that cross this
    /// `finally`: the one with index `k` enters with reason `5 + k` and, at the end of the
    /// `finally`, continues the jump.
    pub jumps: Vec<(bool, Option<SymbolId>)>,
}

pub struct FnBuilder<'a, 'c> {
    pub ctx: &'c Context<'a>,
    pub unit_id: dartforge_elements::model::UnitId,
    pub func: Function,
    pub current_block: BlockId,
    pub next_value: u32,
    pub next_block: u32,
    pub locals: HashMap<LocalId, Operand>,
    /// Lexical scopes of the locals, from the outermost (parameters) to the current one (R6).
    pub(super) scopes: Vec<super::locals::LocalScope>,
    /// Monotonic identity of the scopes, independent of the depth.
    pub(super) next_scope: u32,
    /// `late` initializers that the lowering is already expanding. A recursive
    /// read must consult the cell in the runtime, not expand the AST
    /// again during compilation.
    pub late_initializers_in_lowering: std::collections::HashSet<usize>,
    /// How many `alloca`s are already at the start of the entry block.
    pub n_allocas: usize,
    pub value_types: HashMap<ValueId, Type>,
    /// Facts of the declared parameters, including absorbed functions.
    pub dart_scalar_parameters: HashMap<String, std::collections::HashSet<ValueId>>,
    /// RTI IDs with an explicit native origin, including absorbed functions.
    pub dart_rti_parameters: HashMap<String, std::collections::HashSet<ValueId>>,
    /// Native vectors with a role declared by the producer's ABI.
    pub dart_vector_parameters: HashMap<String, HashMap<ValueId, DartEntryVector>>,
    pub arc_uniform_returns: std::collections::HashSet<String>,
    /// Dart bodies built here separately (closures) whose Ref return follows
    /// the Owned convention of Dart functions (`Module::dart_ref_returns`).
    pub dart_ref_returns: std::collections::HashSet<String>,
    /// Caches of native RTI IDs declared by the lowering, with an implicit `$ok` flag.
    /// All writes and representations require a check before producing contracts.
    pub arc_rti_caches: std::collections::HashSet<String>,
    /// Shareable definitions declared nominally by the ARC producer.
    /// Does not certify equivalence of the copies nor emission dependencies.
    pub arc_shared_functions: std::collections::HashSet<String>,
    pub arc_direct_local_functions: std::collections::HashSet<String>,
    pub break_targets: Vec<BlockId>,
    pub continue_targets: Vec<BlockId>,
    pub this_param: Option<Operand>,
    /// Obligation of the static receiver. None requires proof; there is only a capture
    /// obligation when this_param represents an effective capture.
    pub finalizable_this: Option<bool>,
    pub enclosing_class: Option<dartforge_elements::model::ClassId>,
    pub exception_targets: Vec<BlockId>,
    pub finally_scopes: Vec<FinallyScope>,
    pub active_catch_stack: Vec<(Operand, u8)>,
    pub terminated_blocks: std::collections::HashSet<BlockId>,
    pub extra_functions: Vec<Function>,
    /// Per-body frame ABI, transported with separately lowered functions.
    pub async_frame_layouts: Vec<(String, ValueId, String, usize)>,
    pub inline_policies: std::collections::HashMap<String, crate::hir::InlinePolicy>,
    pub labeled_break_targets: HashMap<SymbolId, BlockId>,
    pub labeled_continue_targets: HashMap<SymbolId, BlockId>,
    pub pending_labels: Vec<SymbolId>,
    /// The static type of the function value that the current call will call
    /// (`lower_call`, read by `call_function_value`).
    pub callee_type: Option<dartforge_types::TypeId>,
    /// The expression of the current call (`lower_call`), from which
    /// `call_function_value` takes the type arguments it passes to a
    /// generic closure (written ones, or those inferred by the static type).
    pub current_call: Option<ExprId>,
    pub current_cascade_target: Option<Operand>,
    /// An already evaluated receiver: when `lower_expr` reaches the expression, it uses the
    /// value instead of evaluating it again (the assignment `a?.b = v` evaluates `a`
    /// before, to test for null).
    pub receiver_ready: Option<(ExprId, Operand)>,
    /// The `?.`/`?[` assignment whose null test has already been done (the target).
    pub handled_null_aware: Option<ExprId>,
    /// Diagnostics of unsupported constructs (N1).
    pub errors: Vec<String>,
    /// The current `?.` chain: exit block with null and the phi entries
    /// (N3). `None` outside a chain.
    pub null_chain: Option<(BlockId, Vec<(BlockId, Operand)>)>,
    /// The next `lower_expr` is the target of a link of the current chain.
    pub continue_chain: bool,
    /// Value read before a compound assignment (result of `x++`).
    pub old_value: Option<Operand>,
    // --- P1 (closures, α) ---
    /// Offsets of the declarations of this function that live in a cell (capture.rs).
    pub cells: std::collections::HashSet<usize>,
    /// Anonymous closures already created in this function (body name).
    pub n_closures: u32,
    /// Names of local function bodies already used in this function.
    pub local_names: std::collections::HashSet<String>,
    /// Tear-off entries already generated by this function.
    pub done_entries: std::collections::HashSet<String>,
    // --- P3 (canonical const, α) ---
    /// Globals created by this function (the canonical constants).
    pub extra_globals: Vec<(u32, Type, String)>,
    /// The constant expression that the current canonical getter evaluates (it is not
    /// canonicalized again inside it).
    pub constant_in_progress: Option<ExprId>,
    /// Inside a constant context (a `const` initializer, a default value,
    /// arguments of a `const C(…)`): `C(…)`, `[…]` and `{…}` are constants.
    pub in_const_context: bool,
    /// The value key (`constants.rs`) of each visible `const` local.
    pub local_const_keys: HashMap<SymbolId, (String, ExprId)>,
    /// The current pattern is a matching one (`case`, `if-case`): a bare name
    /// in it is a constant pattern, not a new variable.
    pub refutable_pattern: bool,
    /// The current type test is that of an `as` (checks only the class).
    pub cast_by_class_only: bool,
    // --- P5c (source SDK, δ) ---
    /// This function is an adapter of the method table (`sdk_source.rs`): the
    /// member it adapts is called directly, never through the selector again.
    pub in_adapter: bool,
    /// Typed entry (`$tc`/`$ts`, `typed_entry.rs`): the arity and the
    /// names have already been guaranteed by the static caller, and
    /// `unpack` does not check them again.
    pub guaranteed_arity: bool,
    /// The parameter types of the invocation whose arguments are evaluated
    /// next (the argument list by address): `evaluate_args`
    /// checks each `dynamic` argument right after evaluating it, like the
    /// implicit cast that the CFE puts around it (`typed_entry.rs`).
    pub arg_types: Option<(usize, super::typed_entry::InvocationTypes)>,
    /// Argument values whose type the static caller guarantees (a known
    /// static type that is not `dynamic`, or checked by the implicit
    /// cast): only with all of them like this does the call use the typed entry; the
    /// rest stay with the one that checks everything (`c:`).
    pub checked_args: std::collections::HashSet<ValueId>,
    // --- direct local functions (`direct_functions.rs`) ---
    /// The local functions that `capture.rs` decided are direct in this function.
    pub allowed_directs: std::collections::HashSet<dartforge_frontend::ast::FunctionId>,
    /// Offsets of the declarations of this function that are written somewhere.
    pub assigned: std::collections::HashSet<usize>,
    /// Offsets of the outer variables that reach this direct function by
    /// address (passed on by address to another direct one).
    pub pointers: std::collections::HashSet<usize>,
    /// The visible direct functions, by the offset of the name.
    pub direct_functions: HashMap<usize, super::direct_functions::Direct>,
    /// Without direct functions (an `async`/generator body: the `alloca`s become
    /// frame positions, and their address is useless).
    pub without_directs: bool,
    /// The address of the elements of the typed list whose length
    /// `fast_length` has just read (the list and the address): the access
    /// it guards uses the same one, without reading again (`typed_items.rs`).
    pub typed_data: Option<(Operand, Operand)>,
    /// The cache of the header of a typed list read from a field, per element
    /// type: the two locals (handle, header) that all the accesses of the
    /// function share (`typed_items.rs`, the typed header cache).
    pub header_caches: std::collections::HashMap<i64, (Operand, Operand)>,
    /// The values read from a global variable through its getter
    /// (`members.rs`, `read_global`): reread on each access, like a field,
    /// so they also go through the header cache.
    pub read_from_global: std::collections::HashSet<ValueId>,
    // --- P6 (async, `async_sm.rs`) ---
    /// Body of the current `async` function: the frame, the resumptions.
    pub async_state: Option<Box<super::async_sm::AsyncState>>,
    // --- RTI (`rti.rs`) ---
    /// The type parameters of the current function (`M<i>`), by name.
    pub function_type_params: Vec<SymbolId>,
    /// The type parameters of the enclosing generic closures (the local function
    /// `T f<T>()`, the expression `<T>(x) => …`) and the current one: name, parameter and
    /// position in the tuple (`M<i>`), after those of their creators
    /// (`closures.rs`, `lower_closure`). The innermost comes last.
    pub local_params: Vec<(SymbolId, dartforge_types::table::TypeParamId, usize)>,
    /// The type parameters of the class come in the tuple (a factory of a generic
    /// class: there is no `this`).
    pub class_by_tuple: bool,
    /// The type argument tuple of the current function (`I64`), if there is one.
    pub type_tuple: Option<Operand>,
    /// The static type of the creation that `instantiate` will lower (`C<T…>`).
    /// The concrete class of the next creation, when the chosen constructor is
    /// that of the superclass that a mixin application forwards
    /// (`members::constructor_of`).
    pub concrete_class: Option<dartforge_elements::model::ClassId>,
    pub creation_type: Option<dartforge_types::table::TypeId>,
    /// The type argument tuple of the current generic call, which
    /// `call_direct` appends when the target has type parameters.
    pub armed_tuple: Option<Operand>,
    /// The arguments the call site wrote for the function `match_args`
    /// matched last (`lower/js_members.rs`: dart2wasm passes only those to
    /// JavaScript); the next `call_external` of that function takes it.
    pub(crate) call_shape: Option<(usize, super::js_members::CallShape)>,
    /// The static type of `this` in an extension member (the `on`): the
    /// implicit receiver of another extension call.
    pub this_extension: Option<(
        dartforge_elements::model::ExtensionId,
        dartforge_types::table::TypeId,
    )>,
    /// In an instance member of an extension type: the type and the static type
    /// of `this` (`E<T…>`, not erased) — the implicit receiver of its
    /// members (`extension_types.rs`).
    pub this_ext_type: Option<(
        dartforge_elements::model::ClassId,
        dartforge_types::table::TypeId,
    )>,
    /// The value of a `return;` (and of the end of the body): the representation, in the
    /// generative constructor of an extension type.
    pub constructor_return: Option<Operand>,
    /// The tuple of the type arguments written in the object pattern of the current generic
    /// extension type (`patterns.rs`).
    pub pattern_tuple_te: Option<Operand>,
    /// N13: the core lists whose length and data are fixed in the
    /// round trips being emitted (`statements::Counted::CoreList`), by the address of the
    /// local.
    pub fixed_lists: Vec<FixedList>,
    /// The fixed list of the indexed access being emitted (`typed_items.rs`).
    pub access_fixed: Option<FixedList>,
    /// The typed lists of the fast round trips being emitted
    /// (`statements::Counted::TypedSet`): the loop counter is proven
    /// within their bounds, and the data and the length were read
    /// before the round trips.
    pub proven_typed: Vec<ProvenTyped>,
    /// The proven typed list of the indexed access being emitted (`typed_items.rs`).
    pub access_proven: Option<ProvenTyped>,
    /// The class that declares the current function (also in a static
    /// member): its statics are in the lexical scope.
    pub member_class: Option<dartforge_elements::model::ClassId>,
    /// How many `Pointer.fromFunction`s the function has already lowered: with the symbol, the
    /// site of each one (there is one trampoline per site, as in the VM).
    pub callback_sites: u32,
    /// J05: `(line, column)` of the statement being lowered, with
    /// debugging on; each emitted instruction records it.
    pub position: Option<(u32, u32)>,
    /// What the symbolic stack trace needs from the function besides the positions (§13.14).
    pub stack_trace: FunctionStackTrace,
}

/// What the stack trace in the VM's format needs from a function besides the positions
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14), passed to the
/// [`crate::hir::FunctionDebugInfo`] when the function is delivered
/// ([`FnBuilder::close_stack_trace`]).
#[derive(Debug, Clone, Default)]
pub struct FunctionStackTrace {
    /// The position of the function token.
    pub token: Option<(u32, u32)>,
    /// [`crate::hir::stack_trace_marks`].
    pub marks: u8,
    pub async_body: Option<crate::hir::TraceAsyncBody>,
    pub closure_entry: Option<String>,
    pub chain_link: Option<crate::hir::WaitLink>,
    /// The trace stack (`dartforge_stack_trace_enter(ty, value)` at the entry and
    /// `dartforge_stack_trace_exit` at each exit): the frame of an `async` body
    /// (type 0) and the listener of `_FutureListener.handleValue` (type 1), which the
    /// runtime matches with the frames of the machine stack.
    pub stack: Option<(i64, Operand)>,
}

impl<'a, 'c> FnBuilder<'a, 'c> {
    /// Initializer of a variable/field declared in the AST.
    ///
    /// The `VariableElement` does not keep the expression; it lives in the AST of the unit
    /// that declared the variable (`VariableRef`). We only return the expression
    /// when the unit is the same one this builder is lowering, because
    /// `lower_expr` receives the current `ast` — an `ExprId` of another unit
    /// would index the wrong tree and silently produce wrong code.
    /// Converts any operand to i1, to serve as a condition.
    ///
    /// Locals go through alloca/store/load as i64, so a bool kept
    /// in a variable comes back as i64 0/1; comparing with zero recovers the i1 without
    /// assuming anything about the source width.
    pub fn to_bool(&mut self, op: Operand) -> Operand {
        match self.operand_type(&op) {
            Type::I1 => return op,
            // A boxed `bool` is not "different from zero": the handle of the
            // box of `false` is too. It comes back through `Unbox` (R3).
            Type::Ref => return self.coerce(op, Type::I1),
            _ => {}
        }
        self.emit(
            Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))),
            Type::I1,
        )
    }

    /// Representation of the static type of an expression, when it is
    /// known and is not `dynamic`.
    pub fn expression_repr(&self, e: ExprId) -> Option<Type> {
        let t = self.ctx.get_type(self.unit_id, e)?;
        if t == self.ctx.core.dynamic_ || self.ctx.is_void(t) {
            return None;
        }
        Some(self.ctx.to_hir_type(t))
    }

    pub fn new(
        ctx: &'c Context<'a>,
        unit_id: dartforge_elements::model::UnitId,
        symbol: String,
        name: String,
        return_ty: Type,
    ) -> Self {
        let entry_block = BlockId(0);
        let func = Function {
            symbol,
            name,
            debug_info: None,
            params: Vec::new(),
            return_ty,
            blocks: vec![BasicBlock {
                id: entry_block,
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            }],
        };

        Self {
            ctx,
            unit_id,
            func,
            current_block: entry_block,
            next_value: 0,
            next_block: 1,
            locals: HashMap::new(),
            scopes: vec![super::locals::LocalScope::new(0)],
            next_scope: 1,
            late_initializers_in_lowering: std::collections::HashSet::new(),
            n_allocas: 0,
            value_types: HashMap::new(),
            dart_scalar_parameters: HashMap::new(),
            dart_rti_parameters: HashMap::new(),
            dart_vector_parameters: HashMap::new(),
            arc_uniform_returns: Default::default(),
            dart_ref_returns: Default::default(),
            arc_rti_caches: Default::default(),
            arc_shared_functions: Default::default(),
            arc_direct_local_functions: Default::default(),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            this_param: None,
            finalizable_this: None,
            enclosing_class: None,
            exception_targets: Vec::new(),
            finally_scopes: Vec::new(),
            active_catch_stack: Vec::new(),
            terminated_blocks: std::collections::HashSet::new(),
            extra_functions: Vec::new(),
            async_frame_layouts: Vec::new(),
            inline_policies: std::collections::HashMap::new(),
            labeled_break_targets: HashMap::new(),
            labeled_continue_targets: HashMap::new(),
            pending_labels: Vec::new(),
            callee_type: None,
            current_call: None,
            current_cascade_target: None,
            receiver_ready: None,
            handled_null_aware: None,
            errors: Vec::new(),
            null_chain: None,
            continue_chain: false,
            old_value: None,
            cells: std::collections::HashSet::new(),
            n_closures: 0,
            local_names: std::collections::HashSet::new(),
            done_entries: std::collections::HashSet::new(),
            extra_globals: Vec::new(),
            constant_in_progress: None,
            in_const_context: false,
            local_const_keys: HashMap::new(),
            refutable_pattern: false,
            cast_by_class_only: false,
            in_adapter: false,
            guaranteed_arity: false,
            arg_types: None,
            checked_args: std::collections::HashSet::new(),
            allowed_directs: std::collections::HashSet::new(),
            assigned: std::collections::HashSet::new(),
            pointers: std::collections::HashSet::new(),
            direct_functions: HashMap::new(),
            without_directs: false,
            typed_data: None,
            header_caches: std::collections::HashMap::new(),
            read_from_global: std::collections::HashSet::new(),
            async_state: None,
            function_type_params: Vec::new(),
            local_params: Vec::new(),
            class_by_tuple: false,
            type_tuple: None,
            concrete_class: None,
            creation_type: None,
            armed_tuple: None,
            call_shape: None,
            this_extension: None,
            this_ext_type: None,
            constructor_return: None,
            pattern_tuple_te: None,
            fixed_lists: Vec::new(),
            access_fixed: None,
            proven_typed: Vec::new(),
            access_proven: None,
            member_class: None,
            callback_sites: 0,
            position: None,
            stack_trace: FunctionStackTrace::default(),
        }
    }

    /// Passes to the debug info of the function what the stack trace needs from it and inserts the
    /// trace stack, when requested. Without the symbolic stack trace, nothing.
    pub fn close_stack_trace(&mut self) {
        if !self.ctx.stack_trace {
            return;
        }
        let r = std::mem::take(&mut self.stack_trace);
        let needs = r.marks != 0
            || r.async_body.is_some()
            || r.closure_entry.is_some()
            || r.chain_link.is_some()
            || r.stack.is_some();
        if self.func.debug_info.is_none() {
            if !needs {
                return;
            }
            let unit = self.ctx.program.unit(self.unit_id);
            self.func.debug_info = Some(Box::new(crate::hir::FunctionDebugInfo {
                file: unit
                    .path
                    .as_ref()
                    .map_or_else(|| unit.uri.clone(), |p| p.display().to_string()),
                url: self.ctx.stack_trace_url(self.unit_id),
                line: r.token.map_or(0, |t| t.0),
                ..Default::default()
            }));
        }
        if let Some(d) = self.func.debug_info.as_mut() {
            d.token = r.token.unwrap_or((d.line, 0));
            d.marks |= r.marks;
            d.async_body = r.async_body;
            d.closure_entry = r.closure_entry;
            d.chain_link = r.chain_link;
        }
        if let Some((ty, value)) = r.stack {
            self.instrument_stack_trace_stack(ty, value);
        }
    }

    /// `dartforge_stack_trace_enter(ty, value)` at the start of the entry block
    /// (after the definition of `value`, when it belongs to it) and
    /// `dartforge_stack_trace_exit(depth)` before each `Return` and `Throw`:
    /// the depth returned by the entry also cuts what functions
    /// unwound by an exception left on top.
    fn instrument_stack_trace_stack(&mut self, ty: i64, value: Operand) {
        if self.func.blocks.is_empty() {
            return;
        }
        let new = |n: &mut u32| {
            let v = ValueId(*n);
            *n += 1;
            v
        };
        let prof = new(&mut self.next_value);
        let call = Instruction::CallRuntime {
            name: "dartforge_stack_trace_enter".to_string(),
            args: vec![
                (Operand::Constant(Constant::Int(ty)), Type::I64),
                (value.clone(), Type::Ref),
            ],
            ret_ty: Type::I64,
        };
        self.value_types.insert(prof, Type::I64);
        let block = &mut self.func.blocks[0];
        let after = match &value {
            Operand::Val(v) => block
                .instructions
                .iter()
                .position(|(x, _, _)| x == v)
                .map_or(0, |i| i + 1),
            _ => 0,
        };
        // There is no `phi` in the entry block; the `alloca`s may come before.
        block.instructions.insert(after, (prof, call, Type::I64));
        let mut exits = Vec::new();
        for (bi, b) in self.func.blocks.iter().enumerate() {
            if matches!(b.terminator, Terminator::Return(_) | Terminator::Throw(_)) {
                exits.push(bi);
            }
        }
        for bi in exits {
            let v = new(&mut self.next_value);
            self.value_types.insert(v, Type::Void);
            self.func.blocks[bi].instructions.push((
                v,
                Instruction::CallRuntime {
                    name: "dartforge_stack_trace_exit".to_string(),
                    args: vec![(Operand::Val(prof), Type::I64)],
                    ret_ty: Type::Void,
                },
                Type::Void,
            ));
        }
    }

    /// J05: the current position becomes that of byte `offset` of `ast`, if it is
    /// the tree of the unit of this function (the position is its own) and debugging
    /// is on.
    pub fn mark_position(&mut self, ast: &ast::Ast, offset: usize) {
        if self.ctx.debug_info.is_none()
            || !std::ptr::eq(ast, &self.ctx.program.unit(self.unit_id).ast)
        {
            return;
        }
        self.position = self.ctx.line_and_column(self.unit_id, offset);
    }

    /// Declares `this` (when `with_this`) and the outline parameters of `fid`.
    pub fn declare_parameters(&mut self, fid: usize, with_this: bool) {
        if with_this {
            let this_vid = self.add_param("this".to_string(), Type::Ref);
            self.this_param = Some(Operand::Val(this_vid));
            self.finalizable_this = self.ctx.classify_this(fid);
        }
        let Some(data) = self.ctx.outline.functions.get(fid) else {
            return;
        };
        // The offsets of the names in the declaration: the key of the cells (P1).
        let program = self.ctx.program;
        let ast_params: &[ast::Parameter] = match program.functions[fid].node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => program
                .unit(unit)
                .ast
                .function(function)
                .parameters
                .as_deref()
                .unwrap_or(&[]),
            dartforge_elements::model::FunctionRef::Constructor { unit, member } => {
                match &program.unit(unit).ast.member(member).kind {
                    ast::MemberKind::Constructor(c) => &c.parameters[..],
                    _ => &[],
                }
            }
            dartforge_elements::model::FunctionRef::None => &[],
        };
        for (i, p) in data.parameters.iter().enumerate() {
            let p_name = p
                .name
                .map(|s| self.ctx.symbol_name(s).to_string())
                .unwrap_or_else(|| "arg".to_string());
            let p_ty = self.repr(p.ty);
            let vid = self.add_param(p_name, p_ty);
            if self.ctx.arc_memory && matches!(p_ty, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
                self.dart_scalar_parameters
                    .entry(self.func.symbol.clone())
                    .or_default()
                    .insert(vid);
            }
            if let Some(sym) = p.name {
                match ast_params.get(i).and_then(|a| a.name) {
                    Some(n) => self.declare_variable(sym, n.span.start, p_ty, Operand::Val(vid)),
                    None => self.declare_local_with_value(sym, p_ty, Operand::Val(vid)),
                }
                // The outline keeps the Dart type even without an offset in the body.
                self.assign_semantic_type(sym, Some(p.ty));
            }
        }
    }

    /// Delivers the function (and the local functions) to the module, with the diagnostics.
    pub fn finish(mut self, module: &mut Module) {
        self.close_stack_trace();
        if let (Some(class), Some(Operand::Val(value))) = (self.enclosing_class, &self.this_param)
            && self
                .ctx
                .program
                .library(self.ctx.program.class(class).library)
                .is_sdk
            && self
                .func
                .params
                .iter()
                .any(|(id, _, ty)| id == value && *ty == Type::Ref)
            && let Some(cid) = self.ctx.class_id_of(class)
        {
            module
                .pruning_this_receivers
                .insert(self.func.symbol.clone(), (*value, cid));
        }
        // P6: the diagnostic of a source function only holds if pruning
        // keeps it (`source::prune`).
        let lib = self.ctx.program.unit(self.unit_id).library;
        if self.ctx.from_source.contains(&lib) && crate::source::sdk_symbol(&self.func.symbol) {
            if !self.errors.is_empty() {
                module
                    .source_errors
                    .push((self.func.symbol.clone(), self.errors));
            }
        } else {
            module.errors.extend(self.errors);
        }
        module.globals.extend(self.extra_globals);
        module
            .dart_scalar_parameters
            .extend(self.dart_scalar_parameters);
        module.dart_rti_parameters.extend(self.dart_rti_parameters);
        module
            .dart_vector_parameters
            .extend(self.dart_vector_parameters);
        module.arc_uniform_returns.extend(self.arc_uniform_returns);
        module.dart_ref_returns.extend(self.dart_ref_returns);
        module.arc_rti_caches.extend(self.arc_rti_caches);
        module
            .arc_shared_functions
            .extend(self.arc_shared_functions);
        module
            .arc_direct_local_functions
            .extend(self.arc_direct_local_functions);
        module.inline_policies.extend(self.inline_policies);
        module.async_frame_layouts.extend(self.async_frame_layouts);
        module.functions.push(self.func);
        module.functions.extend(self.extra_functions);
    }

    /// A construct that the lowering cannot lower: a diagnostic with a position
    /// (N1). The returned operand is never emitted — a module with
    /// errors does not generate code.
    /// The body of a function (or global getter) that the closed world of the
    /// program does not reach (C7, `native_world.rs`): throws
    /// `UnsupportedError` with the symbol. It only runs if the analysis was wrong — the error
    /// is loud and says what was missing.
    pub fn pruned_body(&mut self) {
        let msg = format!(
            "dartforge: code pruned as unreachable was called: {}",
            self.func.symbol
        );
        let m = self.emit(Instruction::Const(Constant::String(msg)), Type::Ref);
        let e = self.new_runtime_error("dartforge_unsupported_error_new", vec![(m, Type::Ref)]);
        self.emit_throw_op(e);
    }

    pub fn unsupported(&mut self, what: &str, span: dartforge_diagnostics::Span) -> Operand {
        let unit = self.ctx.program.unit(self.unit_id);
        // P6: in the code of the SDK compiled from source (open world: pruning is
        // conservative — the `toString` of every class, every target of a
        // dispatch), the missing construct becomes an `UnsupportedError` at
        // runtime, with the same text: the program that does not go through there
        // compiles, and the one that does fails loudly, never silently.
        if self.ctx.from_source.contains(&unit.library) {
            // `DARTFORGE_SOURCE_UNSUPPORTED=1`: lists, during compilation, each
            // construct that became an `UnsupportedError` in the source code.
            if std::env::var_os("DARTFORGE_SOURCE_UNSUPPORTED").is_some() {
                eprintln!("source unsupported in {}: {what}", self.func.symbol);
            }
            if !self.is_terminated() {
                let msg = format!("{}{what}", crate::UNSUPPORTED_PREFIX);
                let m = self.emit(Instruction::Const(Constant::String(msg)), Type::Ref);
                let e =
                    self.new_runtime_error("dartforge_unsupported_error_new", vec![(m, Type::Ref)]);
                self.emit_throw_op(e);
            }
            return Operand::Constant(Constant::Null);
        }
        let position = self.position(span);
        self.errors
            .push(format!("{}{what} ({position})", crate::UNSUPPORTED_PREFIX));
        Operand::Constant(Constant::Null)
    }

    /// `file:line:column` of `span` in the current unit.
    fn position(&self, span: dartforge_diagnostics::Span) -> String {
        let unit = self.ctx.program.unit(self.unit_id);
        let source = &unit.source;
        let ini = span.start.min(source.len());
        let before = &source[..ini];
        let line = before.matches('\n').count() + 1;
        let column = ini - before.rfind('\n').map_or(0, |p| p + 1) + 1;
        let file = unit
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(|| unit.uri.clone(), |n| n.to_string_lossy().into_owned());
        format!("{file}:{line}:{column}")
    }

    /// A language compile error (the program is invalid, not a
    /// missing construct), with the text of the VM's front end. In the code of the
    /// SDK compiled from source it does not happen: the SDK is valid.
    pub fn language_error(&mut self, message: &str, span: dartforge_diagnostics::Span) -> Operand {
        let position = self.position(span);
        self.errors.push(format!("{message} ({position})"));
        Operand::Constant(Constant::Null)
    }

    /// Converts an operand to the representation `to` (R4).
    ///
    /// It is the only place that changes representation: a scalar in a `Ref` position
    /// becomes a box (`Box`), a `Ref` in a scalar position comes back through `Unbox` —
    /// which throws `TypeError` for null or another type, with the exception
    /// check of a call. `int` to `double` is a numeric conversion (the
    /// literal `1` in a `double` context); the `double` bits that come from the
    /// heap use an explicit `Bitcast`, never this function.
    pub fn coerce(&mut self, op: Operand, to: Type) -> Operand {
        let of = self.operand_type(&op);
        if of == to || matches!(to, Type::Void | Type::Ptr) || matches!(of, Type::Void | Type::Ptr)
        {
            return op;
        }
        match (of, to) {
            // The SIMD vector and the box of the static type (`lower/simd.rs`): the
            // non-nullable static type guarantees the right box.
            (k, Type::Ref) if k.is_vector() => {
                self.emit(Instruction::Box { op, from: k }, Type::Ref)
            }
            (Type::Ref, k) if k.is_vector() => self.emit(Instruction::Unbox { op, to: k }, k),
            (Type::I64 | Type::F64 | Type::I1, Type::Ref) => {
                self.emit(Instruction::Box { op, from: of }, Type::Ref)
            }
            (Type::I8, Type::Ref) => {
                let b = self.emit(
                    Instruction::Trunc {
                        op,
                        from: Type::I8,
                        to: Type::I1,
                    },
                    Type::I1,
                );
                self.emit(
                    Instruction::Box {
                        op: b,
                        from: Type::I1,
                    },
                    Type::Ref,
                )
            }
            (Type::Ref, Type::I64 | Type::F64 | Type::I1) => {
                self.emit_call_with_check(Instruction::Unbox { op, to }, to)
            }
            (Type::Ref, Type::I8) => {
                let b =
                    self.emit_call_with_check(Instruction::Unbox { op, to: Type::I1 }, Type::I1);
                self.emit(
                    Instruction::ZExt {
                        op: b,
                        from: Type::I1,
                        to: Type::I8,
                    },
                    Type::I8,
                )
            }
            (Type::I64, Type::F64) => match op {
                Operand::Constant(Constant::Int(n)) => {
                    Operand::Constant(Constant::Double(n as f64))
                }
                op => self.emit(Instruction::IntToDouble(op), Type::F64),
            },
            (Type::F64, Type::I64) => self.emit(Instruction::DoubleToInt(op), Type::I64),
            (Type::I1, Type::I64 | Type::I8) => self.emit(
                Instruction::ZExt {
                    op,
                    from: Type::I1,
                    to,
                },
                to,
            ),
            (Type::I8, Type::I64) => self.emit(
                Instruction::ZExt {
                    op,
                    from: Type::I8,
                    to: Type::I64,
                },
                Type::I64,
            ),
            (Type::I8, Type::I1) => self.emit(
                Instruction::Trunc {
                    op,
                    from: Type::I8,
                    to: Type::I1,
                },
                Type::I1,
            ),
            (Type::I64, Type::I1) => self.emit(
                Instruction::ICmp(ICmpOp::Ne, op, Operand::Constant(Constant::Int(0))),
                Type::I1,
            ),
            (Type::I64, Type::I8) => self.emit(
                Instruction::Trunc {
                    op,
                    from: Type::I64,
                    to: Type::I8,
                },
                Type::I8,
            ),
            (Type::F64, Type::I1 | Type::I8) => self.emit(
                Instruction::FCmp(FCmpOp::Ne, op, Operand::Constant(Constant::Double(0.0))),
                Type::I1,
            ),
            (Type::I1 | Type::I8, Type::F64) => {
                let i = self.emit(
                    Instruction::ZExt {
                        op,
                        from: of,
                        to: Type::I64,
                    },
                    Type::I64,
                );
                self.emit(Instruction::IntToDouble(i), Type::F64)
            }
            _ => op,
        }
    }

    /// Initializer of a variable/field, from any unit (the lowering caller
    /// uses `lower_expr_of` with the unit of the variable).
    pub fn variable_initializer_in(
        &self,
        var_id: dartforge_elements::model::VariableId,
    ) -> Option<dartforge_frontend::ast::ExprId> {
        super::layouts_arc::initializer(self.ctx, var_id)
    }

    pub fn add_param(&mut self, name: String, ty: Type) -> ValueId {
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty);
        self.func.params.push((vid, name, ty));
        vid
    }

    /// Declares a vector of the uniform entry, without inferring a role from the Ptr type.
    pub(super) fn add_param_vector(&mut self, role: DartEntryVector) -> ValueId {
        let name_str = match role {
            DartEntryVector::Arguments => "args",
            DartEntryVector::Descriptor => "desc",
        };
        let id = self.add_param(name_str.to_string(), Type::Ptr);
        if self.ctx.arc_memory {
            self.dart_vector_parameters
                .entry(self.func.symbol.clone())
                .or_default()
                .insert(id, role);
        }
        id
    }

    /// Declares the target convention of a uniform entry generated in this image.
    /// Only the complete preparation/verification can publish its Owned body;
    /// callbacks and indirect dispatch still require contracts from the callers.
    pub(super) fn declare_arc_uniform_return(&mut self) {
        if !self.ctx.arc_memory || self.func.return_ty != Type::Ref {
            return;
        }
        assert_eq!(
            self.func.params.iter().map(|p| p.2).collect::<Vec<_>>(),
            [Type::Ref, Type::Ptr, Type::Ptr],
            "incompatible uniform entry declaration"
        );
        let roles = &self.dart_vector_parameters[&self.func.symbol];
        assert_eq!(
            roles.get(&self.func.params[1].0),
            Some(&DartEntryVector::Arguments)
        );
        assert_eq!(
            roles.get(&self.func.params[2].0),
            Some(&DartEntryVector::Descriptor)
        );
        self.arc_uniform_returns.insert(self.func.symbol.clone());
    }

    /// A parameter in the representation of the Dart type (of a closure, local function
    /// or capture by value): under ARC, the scalar representation is a scalar
    /// fact, as in `declare_parameters`.
    pub(super) fn add_param_dart(&mut self, name: String, ty: Type) -> ValueId {
        let v = self.add_param(name, ty);
        if self.ctx.arc_memory && matches!(ty, Type::I64 | Type::F64 | Type::I1 | Type::I8) {
            self.dart_scalar_parameters
                .entry(self.func.symbol.clone())
                .or_default()
                .insert(v);
        }
        v
    }

    /// Declares an ID of the RTI universe, without inferring a reference from the width.
    /// The caller must pass only canonical native IDs of types/tuples.
    pub fn add_param_rti(&mut self, name: String) -> ValueId {
        let v = self.add_param(name, Type::I64);
        if self.ctx.arc_memory {
            self.dart_rti_parameters
                .entry(self.func.symbol.clone())
                .or_default()
                .insert(v);
        }
        v
    }

    pub fn operand_type(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.value_types.get(v).cloned().unwrap_or(Type::Ref),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::Function(_)) => Type::I64,
        }
    }

    pub fn operand_tag(&self, op: &Operand) -> u8 {
        match self.operand_type(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::Ref => 3,
            Type::F64 => 4,
            _ => 1,
        }
    }

    pub fn source(&self) -> &str {
        &self.ctx.program.unit(self.unit_id).source
    }

    pub fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block += 1;
        self.func.blocks.push(BasicBlock {
            id,
            instructions: Vec::new(),
            terminator: Terminator::Return(None),
        });
        id
    }

    pub fn set_block(&mut self, block: BlockId) {
        self.current_block = block;
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated_blocks.contains(&self.current_block)
    }

    pub fn emit(&mut self, inst: Instruction, ty: Type) -> Operand {
        if self.is_terminated() {
            let dead = self.new_block();
            self.set_block(dead);
        }
        let vid = ValueId(self.next_value);
        self.next_value += 1;
        self.value_types.insert(vid, ty);
        let idx = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == self.current_block)
            .unwrap();
        self.func.blocks[idx].instructions.push((vid, inst, ty));
        if let Some((pos, d)) = self.function_debug_info() {
            d.positions.insert(vid, pos);
        }
        Operand::Val(vid)
    }

    /// J05: the current position and the positions of the function (created at the first
    /// instruction with a position), with debugging on.
    fn function_debug_info(&mut self) -> Option<((u32, u32), &mut crate::hir::FunctionDebugInfo)> {
        let pos = self.position?;
        let unit = self.ctx.program.unit(self.unit_id);
        let d = self.func.debug_info.get_or_insert_with(|| {
            Box::new(crate::hir::FunctionDebugInfo {
                file: unit
                    .path
                    .as_ref()
                    .map_or_else(|| unit.uri.clone(), |p| p.display().to_string()),
                url: self.ctx.stack_trace_url(self.unit_id),
                line: pos.0,
                ..Default::default()
            })
        });
        Some((pos, d))
    }

    pub fn terminate(&mut self, term: Terminator) {
        if self.is_terminated() {
            return;
        }
        // P6: the `return` of an `async` body completes the `Future`
        // (`_asyncReturn`); the suspension and the top-level handler are raw.
        if let Terminator::Return(r) = &term
            && self.async_state.as_ref().is_some_and(|e| !e.raw_return)
        {
            let r = r.clone();
            self.async_return(r);
            return;
        }
        // R4: the returned value in the representation of the return of the function.
        let term = match term {
            Terminator::Return(Some(op)) if !matches!(self.func.return_ty, Type::Void) => {
                let r = self.func.return_ty;
                // `=> print(x)` in a function that returns a value: the `void`
                // expression is worth null (there is no SSA value to return).
                let op = if matches!(self.operand_type(&op), Type::Void) {
                    Self::zero_value(r)
                } else {
                    op
                };
                let op = self.coerce(op, r);
                if self.is_terminated() {
                    return;
                }
                Terminator::Return(Some(op))
            }
            // `set x(v) => expr;` in a void function: the expression has already been
            // evaluated and its value is discarded (LLVM emits `ret void`).
            Terminator::Return(Some(_)) => Terminator::Return(None),
            t => t,
        };
        self.terminated_blocks.insert(self.current_block);
        let block = self.current_block;
        if let Some((pos, d)) = self.function_debug_info() {
            d.exits.insert(block, pos);
        }
        let idx = self
            .func
            .blocks
            .iter()
            .position(|b| b.id == self.current_block)
            .unwrap();
        self.func.blocks[idx].terminator = term;
    }

    pub fn default_return_operand(&self) -> Operand {
        match self.func.return_ty {
            Type::I64 => Operand::Constant(Constant::Int(0)),
            Type::I1 | Type::I8 => Operand::Constant(Constant::Bool(false)),
            Type::F64 => Operand::Constant(Constant::Double(0.0)),
            _ => Operand::Constant(Constant::Null),
        }
    }

    pub fn default_return_operand_opt(&self) -> Option<Operand> {
        if self.func.return_ty == Type::Void {
            None
        } else {
            Some(self.default_return_operand())
        }
    }

    pub fn route_return(&mut self, ret_val: Option<Operand>) {
        let ret_val = ret_val.or_else(|| self.constructor_return.clone());
        // The value in the representation of the return of the function (the body of a
        // typed closure returns an unboxed `int`, for example).
        let ret_val = match ret_val {
            Some(r)
                if !matches!(self.func.return_ty, Type::Void) && self.finally_scopes.is_empty() =>
            {
                Some(self.coerce(r, self.func.return_ty))
            }
            other => other,
        };
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        if self.finally_scopes.is_empty() {
            self.terminate(Terminator::Return(ret_val));
        } else {
            let default_val = self.default_return_operand();
            let val = ret_val.unwrap_or(default_val);
            // Phi entry of the return value of the `finally`: in its
            // representation (R4), coerced here, in the source block.
            let ty_phi = if self.func.return_ty == Type::Void {
                Type::Ref
            } else {
                self.func.return_ty
            };
            let val = self.coerce(val, ty_phi);
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((self.current_block, 1, val));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::Branch(fin_entry));
        }
        let dead = self.new_block();
        self.set_block(dead);
    }

    pub fn route_break(&mut self) {
        self.route_break_to(None);
    }

    pub fn route_break_to(&mut self, label: Option<SymbolId>) {
        self.jump(false, label);
    }

    /// `break`/`continue` (with or without a label): directly to the target, or through the
    /// innermost `finally` that the jump crosses (the reason `5 + k` of the
    /// jump `k` of that `finally`; at its end the jump continues — through other
    /// outer `finally`s, if it crosses more). A jump to a target INSIDE
    /// the `try` (the loop of the body itself) does not go through the `finally`.
    pub fn jump(&mut self, is_continue: bool, label: Option<SymbolId>) {
        self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_clear".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        let (stack, labeled_set) = if is_continue {
            (&self.continue_targets, &self.labeled_continue_targets)
        } else {
            (&self.break_targets, &self.labeled_break_targets)
        };
        let target_opt = match label {
            Some(sym) => labeled_set.get(&sym).copied(),
            None => stack.last().copied(),
        };
        let index = stack.len().checked_sub(1);
        if let Some(target) = target_opt {
            let crosses = self.finally_scopes.last().is_some_and(|fin| match label {
                Some(sym) => {
                    if is_continue {
                        fin.continue_labels.contains(&sym)
                    } else {
                        fin.break_labels.contains(&sym)
                    }
                }
                None => {
                    let prof = if is_continue {
                        fin.prof_continue
                    } else {
                        fin.prof_break
                    };
                    index.is_some_and(|i| i < prof)
                }
            });
            if !crosses {
                self.terminate(Terminator::Branch(target));
            } else {
                let default_ret = self.default_return_operand();
                let current = self.current_block;
                let fin = self.finally_scopes.last_mut().unwrap();
                let k = match fin.jumps.iter().position(|s| *s == (is_continue, label)) {
                    Some(k) => k,
                    None => {
                        fin.jumps.push((is_continue, label));
                        fin.jumps.len() - 1
                    }
                };
                fin.incoming.push((current, 5 + k as i64, default_ret));
                let fin_entry = fin.entry_block;
                self.terminate(Terminator::Branch(fin_entry));
            }
        }
        let dead = self.new_block();
        self.set_block(dead);
    }

    pub fn route_continue(&mut self) {
        self.route_continue_to(None);
    }

    pub fn route_continue_to(&mut self, label: Option<SymbolId>) {
        self.jump(true, label);
    }

    /// The safepoint of a loop round trip (J01), at the start of the body: reads the
    /// interrupt request of the isolate (one byte, through the thread context) and,
    /// with a request, calls the runtime, which serves the control port (`ping`,
    /// pause, `kill`). An immediate `kill` leaves an uncatchable pending
    /// exception, and the common exception path unwinds the isolate. It plays the role
    /// of the VM's stack check in loop round trips.
    pub fn emit_safepoint(&mut self) {
        if self.is_terminated() {
            return;
        }
        let request = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_pending_interrupt".to_string(),
                args: Vec::new(),
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let ha = self.emit(
            Instruction::ICmp(ICmpOp::Ne, request, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let slow = self.new_block();
        let follows = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ha,
            then_block: slow,
            else_block: follows,
        });
        self.set_block(slow);
        self.emit_call_with_check(
            Instruction::CallRuntime {
                name: "dartforge_safepoint".to_string(),
                args: Vec::new(),
                ret_ty: Type::Void,
            },
            Type::Void,
        );
        self.terminate(Terminator::Branch(follows));
        self.set_block(follows);
    }

    pub fn emit_call_with_check(&mut self, inst: Instruction, ret_ty: Type) -> Operand {
        // The extern that the effects table marks as not throwing (and not running
        // Dart) leaves no pending exception: there is nothing to check after
        // it (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.2, E1.1). The
        // read of the pending flag itself is the standalone check (the end of a
        // statement inside `try`) and stays.
        // `DARTFORGE_NO_EFFECTS_IN_CHECK=1` goes back to checking everything (measurement).
        if let Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } =
            &inst
            && name != "dartforge_exception_pending"
            && !Self::check_every_extern()
        {
            let e = crate::llvm::externs::effects_of(name);
            if !e.throws && !e.calls_dart {
                return self.emit(inst, ret_ty);
            }
        }
        let res_op = self.emit(inst, ret_ty);
        if self.is_terminated() {
            return res_op;
        }
        let pending = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_exception_pending".to_string(),
                args: Vec::new(),
                ret_ty: Type::I8,
            },
            Type::I8,
        );
        let is_exc = self.emit(
            Instruction::ICmp(ICmpOp::Ne, pending, Operand::Constant(Constant::Int(0))),
            Type::I1,
        );

        let cont_b = self.new_block();
        let curr_b = self.current_block;

        if let Some(&exc_target) = self.exception_targets.last() {
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: exc_target,
                else_block: cont_b,
            });
        } else if !self.finally_scopes.is_empty() {
            let default_ret = self.default_return_operand();
            let fin = self.finally_scopes.last_mut().unwrap();
            fin.incoming.push((curr_b, 2, default_ret));
            let fin_entry = fin.entry_block;
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: fin_entry,
                else_block: cont_b,
            });
        } else {
            let unwind_b = self.new_block();
            self.terminate(Terminator::CondBranch {
                cond: is_exc,
                then_block: unwind_b,
                else_block: cont_b,
            });
            let prev = self.current_block;
            self.set_block(unwind_b);
            self.terminate(Terminator::Return(self.default_return_operand_opt()));
            self.set_block(prev);
        }

        self.set_block(cont_b);
        res_op
    }

    /// The check after every extern, also those that do not throw (the
    /// behavior before E1.1, for measuring).
    fn check_every_extern() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| {
            std::env::var_os("DARTFORGE_NO_EFFECTS_IN_CHECK")
                .is_some_and(|v| !v.is_empty() && v != "0")
        })
    }

    pub fn emit_throw(&mut self, ast: &ast::Ast, expr_id: ExprId) -> Operand {
        let ex_op = self.lower_expr(ast, expr_id);
        self.emit_throw_op(ex_op)
    }

    pub fn emit_throw_op(&mut self, ex_op: Operand) -> Operand {
        self.emit_throw_value(ex_op, None);
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
        self.default_return_operand()
    }

    pub fn emit_rethrow(&mut self) -> Operand {
        if let Some((ex_op, _)) = self.active_catch_stack.last().cloned() {
            self.emit_throw_value(ex_op, None);
        }
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
        self.default_return_operand()
    }

    /// `op is T` in the representation of `op`.
    ///
    /// A scalar (`I64`/`F64`/`I1`) has its type decided at compile time. `Ref`
    /// asks the runtime for the class (`dartforge_value_class`, which returns
    /// -12 for null, -9/-10/-11 for the int/double/bool boxes, -2 for
    /// String…) — without dereferencing null (H6).
    pub fn test_type(&mut self, ast_ty: &ast::TypeAnnotation, op: Operand) -> Operand {
        if let Some(r) = self.test_source_type(ast_ty, op.clone()) {
            return r;
        }
        // `x is List<int>`, `x is T`, `x is FutureOr<T>`, a function or
        // record type: through the RTI (`rti.rs`). The test by class below only holds
        // for a class without type arguments (or with trivial arguments).
        if self.annotation_needs_rti(ast_ty) {
            return match self.annotation_recipe(ast_ty) {
                Some(r) => {
                    let t = self.recipe_rti(&r);
                    self.test_rti(op, t)
                }
                None => self.unsupported("type test with an unresolved name", ast_ty.span),
            };
        }
        let ast::TypeKind::Named { name, .. } = &ast_ty.kind else {
            return self.unsupported("structural type test", ast_ty.span);
        };
        let Some(last_one) = name.last() else {
            return self.unsupported("type test", ast_ty.span);
        };
        let name_str = self.ctx.symbol_name(last_one.sym).to_string();
        let repr = self.operand_type(&op);
        if repr != Type::Ref {
            let r = matches!(
                (name_str.as_str(), repr),
                ("int", Type::I64)
                    | ("double", Type::F64)
                    | ("bool", Type::I1 | Type::I8)
                    | ("num", Type::I64 | Type::F64)
                    | ("Object" | "dynamic" | "Comparable", _)
            );
            return Operand::Constant(Constant::Bool(r));
        }
        let cls = self.emit(
            Instruction::CallRuntime {
                name: "dartforge_value_class".to_string(),
                args: vec![(op, Type::Ref)],
                ret_ty: Type::I64,
            },
            Type::I64,
        );
        let equal = |b: &mut Self, v: i64| {
            b.emit(
                Instruction::ICmp(ICmpOp::Eq, cls.clone(), Operand::Constant(Constant::Int(v))),
                Type::I1,
            )
        };
        let base = match name_str.as_str() {
            "dynamic" => Operand::Constant(Constant::Bool(true)),
            "Object" => self.emit(
                Instruction::ICmp(
                    ICmpOp::Ne,
                    cls.clone(),
                    Operand::Constant(Constant::Int(-12)),
                ),
                Type::I1,
            ),
            "Null" => equal(self, -12),
            "num" => {
                let i = equal(self, -9);
                let d = equal(self, -10);
                self.emit(Instruction::Or(i, d), Type::I1)
            }
            _ => {
                let lib = self.ctx.program.unit(self.unit_id).library;
                // `p.Type` (an import with a prefix) or `Type`.
                let binding = match &name[..] {
                    [p, t] => self.ctx.program.lookup_prefixed(lib, p.sym, t.sym),
                    _ => self.ctx.program.lookup(lib, last_one.sym),
                };
                let cid = binding.and_then(|b| match b.getter {
                    Some(dartforge_elements::model::Element::Class(c)) => Some(c),
                    _ => None,
                });
                let Some(id) = cid.and_then(|c| self.class_id_of(c)) else {
                    return self.unsupported(&format!("type test `{name_str}`"), ast_ty.span);
                };
                if id < 0 {
                    equal(self, id)
                } else {
                    let sub = self.emit(
                        Instruction::CallRuntime {
                            name: "dartforge_is_subclass".to_string(),
                            args: vec![
                                (cls.clone(), Type::I64),
                                (Operand::Constant(Constant::Int(id)), Type::I64),
                            ],
                            ret_ty: Type::I8,
                        },
                        Type::I8,
                    );
                    self.emit(
                        Instruction::ICmp(ICmpOp::Ne, sub, Operand::Constant(Constant::Int(0))),
                        Type::I1,
                    )
                }
            }
        };
        if !ast_ty.nullable {
            return base;
        }
        let null = equal(self, -12);
        self.emit(Instruction::Or(base, null), Type::I1)
    }

    /// Implicit or explicit `as T`: `TypeError` if the value is not a `T`,
    /// with the VM's message for the `context` ("… in type cast" for `as`, without a
    /// suffix in the implicit assignment of a `dynamic`).
    pub fn check_type_or_throw(
        &mut self,
        ast_ty: &ast::TypeAnnotation,
        op: Operand,
        context: super::rti::CastContext,
    ) {
        // `as List<int>`, `as T`…: the whole cast through the RTI.
        if self.annotation_needs_rti(ast_ty) {
            match self.annotation_recipe(ast_ty) {
                Some(r) => {
                    let t = self.recipe_rti(&r);
                    self.cast_rti_in(op, t, context);
                }
                None => {
                    self.unsupported("cast with an unresolved name", ast_ty.span);
                }
            }
            return;
        }
        let ok = self.test_type(ast_ty, op.clone());
        let ok = self.to_bool(ok);
        let fail_b = self.new_block();
        let pass_b = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: ok,
            then_block: pass_b,
            else_block: fail_b,
        });
        self.set_block(fail_b);
        // The failure (cold path): the RTI decides and throws with the VM's message
        // — the dynamic type of the value and that of the annotation.
        match self.annotation_recipe(ast_ty) {
            Some(r) => {
                let t = self.recipe_rti(&r);
                self.cast_rti_in(op, t, context);
                self.terminate(Terminator::Branch(pass_b));
            }
            None => {
                let err_op = self.new_runtime_error("dartforge_type_error_new", Vec::new());
                self.emit_throw_op(err_op);
            }
        }
        self.set_block(pass_b);
    }

    pub fn emit_trunc_div(&mut self, lop: Operand, rop: Operand) -> Operand {
        match rop {
            // Constant divisor: neither a zero to check, nor the -1.
            Operand::Constant(Constant::Int(-1)) => {
                return self.emit(Instruction::Neg(lop), Type::I64);
            }
            Operand::Constant(Constant::Int(c)) if c != 0 => {
                return self.emit(Instruction::SDiv(lop, rop), Type::I64);
            }
            _ => {}
        }
        self.require_divisor(&rop);
        // `x ~/ -1` is `-x` with 64-bit overflow (`-2^63 ~/ -1 == -2^63`);
        // LLVM's `sdiv` does not define that case (x86's `idiv` gives SIGFPE).
        let minus_one = self.emit(
            Instruction::ICmp(
                ICmpOp::Eq,
                rop.clone(),
                Operand::Constant(Constant::Int(-1)),
            ),
            Type::I1,
        );
        let negate = self.new_block();
        let split = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: minus_one,
            then_block: negate,
            else_block: split,
        });
        self.set_block(negate);
        let n = self.emit(Instruction::Neg(lop.clone()), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(split);
        let q = self.emit(Instruction::SDiv(lop, rop), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(
            Instruction::Phi {
                incoming: vec![(negate, n), (split, q)],
                ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// `a % b` of `int` with a variable divisor, inline after the zero
    /// guard: the truncated remainder plus `|b|` when it is negative, which gives the
    /// value in `[0, |b|)` of Dart (`Integer::ArithmeticOp`, kMOD). `b == -1`
    /// always gives 0 and avoids the `srem` of `i64::MIN` by `-1`, undefined in
    /// LLVM; `|i64::MIN|` gives `i64::MIN`, which added gives the same as subtracted.
    pub fn emit_module(&mut self, lop: Operand, rop: Operand) -> Operand {
        self.require_divisor(&rop);
        let int = |c| Operand::Constant(Constant::Int(c));
        let minus_one = self.emit(
            Instruction::ICmp(ICmpOp::Eq, rop.clone(), int(-1)),
            Type::I1,
        );
        let zero = self.new_block();
        let rest = self.new_block();
        let join = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: minus_one,
            then_block: zero,
            else_block: rest,
        });
        self.set_block(zero);
        self.terminate(Terminator::Branch(join));
        self.set_block(rest);
        let r = self.emit(Instruction::SRem(lop, rop.clone()), Type::I64);
        let sign_b = self.emit(Instruction::AShr(rop.clone(), int(63)), Type::I64);
        let inverted = self.emit(Instruction::Xor(rop, sign_b.clone()), Type::I64);
        let module_b = self.emit(Instruction::Sub(inverted, sign_b), Type::I64);
        let sign_r = self.emit(Instruction::AShr(r.clone(), int(63)), Type::I64);
        let adjustment = self.emit(Instruction::And(sign_r, module_b), Type::I64);
        let value = self.emit(Instruction::Add(r, adjustment), Type::I64);
        self.terminate(Terminator::Branch(join));
        self.set_block(join);
        self.emit(
            Instruction::Phi {
                incoming: vec![(zero, int(0)), (rest, value)],
                ty: Type::I64,
            },
            Type::I64,
        )
    }

    /// A zero integer divisor (`~/` and `%` of `int`) throws
    /// `IntegerDivisionByZeroException`; the current block continues on the path of the
    /// valid divisor.
    pub fn require_divisor(&mut self, rop: &Operand) {
        let is_zero = self.emit(
            Instruction::ICmp(ICmpOp::Eq, rop.clone(), Operand::Constant(Constant::Int(0))),
            Type::I1,
        );
        let div_zero_block = self.new_block();
        let normal_div_block = self.new_block();
        self.terminate(Terminator::CondBranch {
            cond: is_zero,
            then_block: div_zero_block,
            else_block: normal_div_block,
        });

        self.set_block(div_zero_block);
        // Source SDK: the `IntegerDivisionByZeroException` of `dart:core`, what
        // the VM throws (and what `on IntegerDivisionByZeroException` catches).
        if let Some(class) = self.ctx.sdk_class("core", "IntegerDivisionByZeroException")
            && let Some(empty) = self.ctx.interner.lookup("")
            && let Some(&ctor) = self.ctx.program.classes[class.0 as usize]
                .constructors
                .get(&empty)
        {
            let error = self.instantiate_evaluated(
                ctor,
                &[],
                dartforge_diagnostics::Span { start: 0, end: 0 },
            );
            if !self.is_terminated() {
                self.emit_throw_op(error);
            }
            self.set_block(normal_div_block);
            return;
        }
        let msg = self.emit(
            Instruction::Const(Constant::String(
                "IntegerDivisionByZeroException".to_string(),
            )),
            Type::Ref,
        );
        let err_obj = self.emit(
            Instruction::AllocObject {
                class_id: 1005, // UnsupportedError
                fields: vec![msg],
            },
            Type::Ref,
        );
        self.emit_throw_value(err_obj, None);
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

        self.set_block(normal_div_block);
    }

    /// Operator of a compound assignment (`a op= b`).
    pub fn lower_binary_op_helper(
        &mut self,
        op: BinaryOp,
        lop: Operand,
        rop: Operand,
        admits_smi: bool,
    ) -> Operand {
        // The current value in `Ref` may be a `String`, a nullable number or
        // another object. The type of the right operand does not prove that it is
        // numeric (`s *= 2`); dynamic dispatch handles those cases.
        let (tl, tr) = (self.operand_type(&lop), self.operand_type(&rop));
        let rop = if tr == Type::Ref && tl == Type::F64 {
            // `d op= n` with `d` a `double` and `n` a `num`: the `int` becomes a `double`
            // (the `other.toDouble()` of the `_Double` operators). The path
            // that unboxes may leave a pending `TypeError`.
            self.emit_call_with_check(
                Instruction::CallRuntime {
                    name: "df.num_to_double".to_string(),
                    args: vec![(rop, Type::Ref)],
                    ret_ty: Type::F64,
                },
                Type::F64,
            )
        } else if tr == Type::Ref && tl == Type::I64 {
            self.coerce(rop, tl)
        } else {
            rop
        };
        self.operate_with_smi(
            op,
            lop,
            rop,
            false,
            admits_smi,
            dartforge_diagnostics::Span { start: 0, end: 0 },
        )
    }
}

/// A runtime list with the length and the address of the elements read once
/// before the round trips of a loop that cannot change them (N13).
#[derive(Debug, Clone)]
pub struct FixedList {
    /// The address of the local of the list (its `alloca`): the identity of the
    /// local, also of a parameter.
    pub key: Operand,
    pub length: Operand,
    pub data: Operand,
    /// The shape of the elements (N14, `heap::FormaDeLista`: 0 general, 1 `int`,
    /// 2 `double`, 3 `bool`), checked before the round trips: in them, the
    /// size and the interpretation of each element are known at emission.
    pub shape: i64,
    /// In a compact shape, the length for direct stores
    /// (`dartforge_list_len_writable`, called before the round trips: the
    /// length if the list is modifiable, otherwise 0 — and every store goes
    /// to the SDK's `[]=`).
    pub writable_length: Option<Operand>,
}

/// A typed list (`dart:typed_data`) of the fast round trips of a
/// versioned loop (`statements::Counted::TypedSet`): before them, it was proven that
/// every value of the counter stays in `0..length`, and the length and the
/// address of the elements were read once — fixed while the list
/// lives (a typed list does not change size, and the bytes live behind a
/// header at a fixed address, `heap::CabecalhoTipado`).
#[derive(Debug, Clone)]
pub struct ProvenTyped {
    /// The address of the local of the list (its `alloca`).
    pub key: Operand,
    /// The address of the local of the loop counter.
    pub counter: Operand,
    pub data: Operand,
    pub length: Operand,
    /// Also proven for the store: the length of a view came from
    /// `dartforge_typed_len(.., 1)`, which gives 0 for the unmodifiable one.
    pub write: bool,
}
