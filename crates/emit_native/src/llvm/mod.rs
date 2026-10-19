//! LLVM IR generator from the native HIR.

pub mod c_abi;
pub mod externs;
mod debug_info;
mod roots;
mod arc_context;
mod arc_ports;
mod numeric_vectors;
pub(crate) use numeric_vectors::numeric_vector_pooling_enabled;
mod stack_trace;
pub mod verify_maps;
pub mod verify_shadow;
mod simd;
// The emissions that depend on the representation of the runtime values, one per
// package of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md §4.2, step 4).
mod ir_boxes;
mod ir_lists;
mod ir_strings;
mod ir_typed;

use dartforge_runtime::layout;
/// Largest field index read inline (the layout contract).
const INLINE_FIELDS: usize = layout::INLINE_FIELDS;
/// The largest body, in words, with inline allocation (the TLAB of `w` words,
/// `layout::context::tlab_cursor(w)`): an object of up to `TLAB_N` fields.
const TLAB_N: i64 = layout::TLAB_N as i64;
/// The mask of `e_objeto` (`h & (7 | i64::MIN) == 2`).
const OBJECT_MASK: i64 = 7 | i64::MIN;
/// The stack pointer register the stack check reads (`llvm.read_register`,
/// [`LlvmEmitter::emit_stack_check`]): the target is the host.
fn stack_pointer() -> &'static str { if crate::target::is_aarch64() { "sp" } else { "rsp" } }
mod selectors;
mod arc_immediates;
mod arc_tag_guards;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod arc_null_literals;

use crate::hir::*;
use std::fmt::Write;

/// Audited layout premises used to omit redundant field/map stores.
struct FieldWriteFacts {
    known_map: Option<u32>,
    fresh: bool,
}

pub struct LlvmEmitter<'a> {
    module: &'a Module,
    /// The functions of the module by symbol (the first with each name, like the
    /// linear `find` that this table replaced: it was done on every
    /// `CallStatic` and made the emission quadratic — tens of minutes on
    /// new_sali/backend, docs/NATIVO-PROJETOS-REAIS.md).
    function_by_symbol: std::collections::HashMap<&'a str, &'a Function>,
    out: String,
    arc_tag_guards: bool,
    /// The string constants, in emission order (deterministic); the
    /// index of each one is its position.
    string_constants: Vec<Vec<u8>>,
    /// The index of each constant by content: collect and look up without
    /// walking the table (before, quadratic in the number of literals).
    string_index: std::collections::HashMap<Vec<u8>, usize>,
    /// Type of each value of the function being emitted, for operand coercion.
    types: std::collections::HashMap<ValueId, Type>,
    /// Counter of the coercion temporaries (%c0, %c1, ...), per function.
    next_coercion: u32,
    /// Conversions that a phi entry requires, assigned to the SOURCE block:
    /// (block, temporary name, source type, value, phi type).
    conv_phi: Vec<(u32, String, Type, ValueId, Type)>,
    /// Type kept by each `alloca` of the function (for the `store`).
    pointee: std::collections::HashMap<ValueId, Type>,
    /// The `Ref` `alloca`s whose address leaves the function (the capture by address
    /// of a direct local function, `lower/direct_functions.rs`): they live in the
    /// slot of the roots frame itself, and whoever receives the address writes
    /// where the collector sees.
    frame_allocas: std::collections::HashSet<ValueId>,
    /// G: root slot of each `Ref` value of the function (SSA or `alloca`).
    slots: std::collections::HashMap<ValueId, usize>,
    /// The function opened a roots frame (`%gcq`).
    has_frame: bool,
    /// The function read `%ctx` (`dartforge_context`, the thread context of the
    /// runtime) at the entry: pending exception and shadow stack without a call.
    has_ctx: bool,
    /// The LLVM label at which each HIR block that splits into
    /// several ends (the inline allocation, [`LlvmEmitter::inline_allocation`]): it is
    /// the predecessor that the `phi`s of the successors name.
    exit_labels: std::collections::HashMap<u32, String>,
    /// The LLVM label of the stretch being emitted (that of the block, or the last one that a
    /// split instruction opened): the predecessor of an inner `phi`.
    current_label: String,
    // --- P1 (closures, α) ---
    /// Immutable numeric vectors: layouts and call descriptors, shared only in AOT.
    vectors: Vec<Vec<i64>>,
    vector_of: std::collections::HashMap<Vec<i64>, usize>,
    vector_symbols: Vec<String>,
    numeric_vector_pooling: bool,
    /// Constant vectors of function addresses (`@df.fns.<k>`).
    function_tables: Vec<Vec<String>>,
    function_table_of: std::collections::HashMap<Vec<String>, usize>,
    /// The names of the named arguments of the descriptors of the module: the
    /// descriptor only keeps the hash, and the `Invocation` of a `noSuchMethod`
    /// needs the name (`dartforge_register_argument_name`).
    argument_names: std::collections::BTreeSet<String>,
    // --- P5c (source SDK, δ; `selectors.rs`) ---
    /// Call sites by selector already emitted (one cache each).
    selector_caches: usize,
    /// The symbol of the function being emitted and how many selector caches it has already
    /// opened: the name of the slot of a cache is `(function, position)`, stable
    /// across generations of a reload (J04).
    current_function: String,
    /// Lowered SDK `this`, revalidated against the current formal parameters.
    current_receiver_cone: Option<(ValueId, u32)>,
    registry_closed: bool,
    /// Allocation facts retain witnesses to the exact immutable HIR bodies.
    receiver_provenance: Option<crate::pruning::receiver_provenance::Provenance<'a>>,
    current_exact_receivers: std::collections::HashMap<ValueId, crate::pruning::receiver_provenance::Classes>,
    cache_in_function: usize,
    /// The index of each slot name of the layout of the live generation (reload).
    slot_index: std::collections::HashMap<i64, usize>,
    /// Texts of the selectors, in order of first use.
    selector_names: Vec<String>,
    /// Signature of each called symbol, to declare what the module does not
    /// define.
    externals: std::collections::BTreeMap<String, String>,
    /// The generic Owned thunks called (`df.arc.owned.<base>`): the base and the
    /// signature (`selectors::emit_owned_thunks`).
    owned_thunks: std::collections::BTreeMap<String, (String, Vec<Type>)>,
    /// The native scope thunks (`df.arc.scope.<base>`): base, arguments
    /// and return.
    scope_thunks: std::collections::BTreeMap<String, (String, Vec<Type>, Type)>,
    /// Symbols defined in a `comdat` (`selectors::linkage_of`).
    comdats: Vec<String>,
    /// `DARTFORGE_CALL_TRACE=1` at compile time: each function reports the entry and the
    /// exit to the runtime, which shows the stack of Dart functions on an exception
    /// (`DARTFORGE_DEBUG=1`). Only for debugging the source SDK.
    stack_trace: Option<usize>,
    stack_trace_names: Vec<String>,
    // --- Per-isolate global area ---
    /// The slot (`i64`) of each global of the module in the area (`@dfg_…` and the
    /// `$ok` flag): Dart statics are per isolate, and each
    /// isolate is a thread with its own area (`dartforge_globals_area`).
    global_slots: std::collections::HashMap<String, usize>,
    /// The hash of the name of each slot, in order (the area descriptor): 0 for the
    /// selector caches.
    slot_hashes: Vec<i64>,
    /// The layout of the area of the running generation (hot reload): the slots that
    /// remain keep their index, the new ones come after (`emit_globals`).
    previous_area: Option<Vec<i64>>,
    /// The area descriptor without the slot names (`[key, -n]`): the
    /// production executable, which never reloads (`with_lean_area`).
    lean_area: bool,
    /// J05: the debug metadata, when some function has positions.
    debug_info: Option<debug_info::DebugInfo>,
    // --- Unified space (docs/NATIVO-ESPACO-UNIFICADO.md) ---
    /// String literals are static objects of the module (§2.11):
    /// true in AOT and in the SDK modules; false in the program modules in the
    /// JIT, whose memory is freed (J02).
    static_objects: bool,
    /// The address of the fields of an object through a call to `@df.body` instead
    /// of the 15 inline instructions (the development profile of the program,
    /// `-O0`): the IR of new_sali/backend had 465 thousand copies of the sequence,
    /// 21% of the lines (docs/NATIVO-PROJETOS-REAIS.md, C9). Production and the
    /// SDK modules stay inline.
    fields_per_call: bool,
    /// Instance allocation through a call to `@df.new_instance` (the
    /// production of the program with `new_instance` in
    /// [`LlvmEmitter::outlined_helpers`]).
    out_of_line_allocation: bool,
    /// The `@df.*` helpers that go `noinline` in the production program module
    /// ([`out_of_line_helpers`]).
    outlined_helpers: Vec<String>,
    /// Out-of-line dispatch by selector (`selector` in
    /// [`LlvmEmitter::outlined_helpers`]): the call site passes the
    /// static descriptor of the selector (`@df.seld.<k>`: hash, name,
    /// length) to `@df.selector_d`, which has `@df.selector` inline —
    /// three arguments instead of five at each of the ~170 thousand sites of the
    /// large program.
    compact_selector: bool,
    /// `optsize` on each function of the module (the large program in production;
    /// the production SDK already has it, `sdk_module::with_optsize`).
    optimize_size: bool,
    /// The objects freshly allocated in this HIR block, with no collection point
    /// since the allocation (`emit_field_addresses`): the body is in the
    /// block (never outside, never null or a `Smi`) and the object is young (the
    /// barrier does nothing).
    fresh_set: std::collections::HashSet<u32>,
    /// The static reference map written by the allocation of each object of
    /// [`Self::fresh_set`] ([`Self::static_map`]): a store into the object whose
    /// constant `is_ref` agrees with the bit skips the read-modify-write of the
    /// map ([`Self::emit_field_write`]).
    fresh_maps: std::collections::HashMap<u32, u32>,
    /// No object has an outside body (`with_body_without_outside`).
    body_without_outside: bool,
    /// The `this` of the function being emitted, when its body is direct
    /// ([`Self::direct_body`]).
    this_object: Option<u32>,
    /// Each roots frame of this emission sits in its own native frame
    /// (the program without optimization: `-O0` in AOT, `CodeGenLevelNone` in the JIT,
    /// without inlining): the runtime can check the order of the addresses on the
    /// shadow stack (the dead frame, `RT/heap.rs` `visit_frames`). With
    /// inlining, two frames become `alloca`s of the same function, in any
    /// order, and the check would give a false positive.
    ordered_frames: bool,
    /// The module is of the production profile (`--optimize`): with
    /// [`Self::fields_per_call`], `@df.body` goes `noinline` (the
    /// optimizer would put it back inline).
    production: bool,
    /// Production: every method table assembled at link time is registered when
    /// the isolate is prepared (`@df.register.sdk_tables`, `pruning::assemble`),
    /// so the inline allocation does not test `Context::registered`
    /// ([`Self::with_tables_registered_at_startup`]).
    tables_registered_at_startup: bool,
    /// The method tables are assembled at link time
    /// (docs/NATIVO-PODA-DE-TABELAS.md §3.2): the module only declares the function
    /// of each table, and the content goes into the summary (`pruning.rs`). It is the production
    /// SDK.
    tables_at_link: bool,
    arc_ports_at_link: bool,
    /// The state of each package of the emission (§4.2, step 4); the packages use it
    /// as they migrate.
    #[allow(dead_code)]
    strings: ir_strings::StringState,
    #[allow(dead_code)]
    boxes: ir_boxes::BoxState,
    #[allow(dead_code)]
    lists: ir_lists::ListState,
    #[allow(dead_code)]
    typed_items: ir_typed::TypedState,
    /// Exceptions by tables (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
    /// the decision of the `optimize::tables` pass for the function being emitted —
    /// which call is an `invoke`, the landing pads and how each `Return` exits. `None` in the usual
    /// model (the pending flag checked after each call), and the IR is
    /// then the usual one, byte for byte.
    tab: Option<&'a FunctionTables>,
    /// `DARTFORGE_EFFECTS=check` ([`externs::check_effects`]): the names
    /// of the externs checked in this module, in order of first use (the
    /// index is that of the global `@df.efn.<k>` with the text).
    checked_externs: Vec<String>,
    /// Roots by stack maps (`--roots=maps`,
    /// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12): the `Ref` SSA values
    /// live across a collection point do not go into the frame of the
    /// shadow stack; each call that may collect carries, in the
    /// `"deopt"` operand, the values live at its entry, and LLVM's `rewrite-statepoints-for-gc`
    /// turns it into a statepoint, with the place of each one in the stack
    /// map of the call — the preserved register where it already is, or the
    /// slot. The collector does not move objects: nothing is relocated, and the code goes on
    /// with the value itself. Only the `Ref` `alloca`s stay in the frame.
    maps: bool,
    /// With [`Self::maps`], the roots analysis of the function being emitted.
    function_roots: roots::Roots,
    /// With [`Self::maps`], the rooted values of the function being emitted.
    rooted_set: std::collections::HashSet<ValueId>,
    /// Roots by maps: the definitions of `@df.vararg.<k>`, the intermediaries
    /// of the variadic C calls made by a `gc` function
    /// ([`Self::c_call_text`]); they go into the module after the functions.
    variadic_set: Vec<String>,
    /// The checking build of the walk (`DARTFORGE_ROOTS_CHECK=1`
    /// with `--roots=maps`, docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §15.2,
    /// E2.5): in a `gc` function, the first of the check slots at the end
    /// of the roots frame and how many there are. Before each collection point they
    /// receive exactly the values live at it (zero in the others), and the
    /// runtime checks that the walk by maps visited all of them.
    check: Option<(usize, usize)>,
    /// Roots by maps in a JIT module (§3.8): the memory manager of the
    /// JIT finds the map of each object and registers it; the module does not call the
    /// registration nor carries `-ni:1` (`LLJIT` rejects a module with a data
    /// layout different from its own, and the JIT does not run IR optimization).
    maps_in_jit: bool,
    /// How many `gc` functions the module emitted (on Mach-O, the `.no_dead_strip`
    /// of the map only goes to a module that has a map).
    gc_functions: usize,
    /// The stack trace in the VM's format (§13.14, `llvm/stack_trace.rs`): the labels of the
    /// calls and the table of the `dfpcl` section. `None`: no table.
    vm_stack_trace: Option<stack_trace::StackTrace>,
    /// The module goes to the JIT: the stack trace section of each object is
    /// registered by the memory manager of the session (`crates/jit`), and the
    /// module has neither the linker symbols nor the registration call.
    stack_trace_in_jit: bool,
}

impl<'a> LlvmEmitter<'a> {
    fn arc_retain_target(&self) -> &'static str {
        if self.arc_tag_guards { arc_tag_guards::RETAIN } else { "dartforge_arc_retain" }
    }

    fn arc_release_target(&self) -> &'static str {
        if self.arc_tag_guards { arc_tag_guards::RELEASE } else { "dartforge_arc_release" }
    }

    /// Supplies a jointly audited SDK snapshot. Its Function borrows prevent
    /// changing a callee after its return fact was used to prove a receiver.
    pub(crate) fn with_receiver_provenance(mut self, proof: crate::pruning::receiver_provenance::Provenance<'a>) -> Self {
        self.receiver_provenance = Some(proof);
        self
    }
    pub fn new(module: &'a Module) -> Self {
        Self {
            module,
            out: String::new(),
            arc_tag_guards: module.arc_memory && !std::env::var("DARTFORGE_NO_ARC_TAG_GUARDS").is_ok_and(|value| value == "1"),
            string_constants: Vec::new(),
            string_index: std::collections::HashMap::new(),
            types: std::collections::HashMap::new(),
            next_coercion: 0,
            conv_phi: Vec::new(),
            pointee: std::collections::HashMap::new(),
            frame_allocas: std::collections::HashSet::new(),
            slots: std::collections::HashMap::new(),
            has_frame: false,
            has_ctx: false,
            exit_labels: std::collections::HashMap::new(),
            current_label: String::new(),

            vectors: Vec::new(),
            vector_of: std::collections::HashMap::new(),
            vector_symbols: Vec::new(),
            numeric_vector_pooling: numeric_vector_pooling_enabled(),
            function_tables: Vec::new(),
            function_table_of: std::collections::HashMap::new(),
            argument_names: std::collections::BTreeSet::new(),
            selector_caches: 0,
            current_function: String::new(),
            current_receiver_cone: None,
            registry_closed: crate::pruning::receiver_provenance::registry::closed(&[module]),
            receiver_provenance: None,
            current_exact_receivers: std::collections::HashMap::new(),
            cache_in_function: 0,
            slot_index: std::collections::HashMap::new(),
            selector_names: Vec::new(),
            externals: std::collections::BTreeMap::new(),
            owned_thunks: std::collections::BTreeMap::new(),
            scope_thunks: std::collections::BTreeMap::new(),
            comdats: Vec::new(),
            stack_trace: std::env::var("DARTFORGE_CALL_TRACE").is_ok_and(|v| v == "1").then_some(0),
            stack_trace_names: Vec::new(),
            global_slots: std::collections::HashMap::new(),
            slot_hashes: Vec::new(),
            previous_area: None,
            lean_area: false,
            // The positions serve the line tables only with `--debug`; without
            // it, they belong only to the symbolic stack trace (`vm_stack_trace`).
            debug_info: (module.dwarf && module.functions.iter().any(|f| f.debug_info.is_some())).then(debug_info::DebugInfo::new_one),
            function_by_symbol: {
                let mut m = std::collections::HashMap::with_capacity(module.functions.len());
                for f in &module.functions {
                    m.entry(f.symbol.as_str()).or_insert(f);
                }
                m
            },
            static_objects: false,
            fields_per_call: false,
            production: false,
            ordered_frames: false,
            out_of_line_allocation: false,
            outlined_helpers: Vec::new(),
            optimize_size: false,
            fresh_set: std::collections::HashSet::new(),
            fresh_maps: std::collections::HashMap::new(),
            body_without_outside: false,
            this_object: None,
            compact_selector: false,
            tables_registered_at_startup: false,
            tables_at_link: false,
            arc_ports_at_link: false,
            strings: Default::default(),
            boxes: Default::default(),
            lists: Default::default(),
            typed_items: Default::default(),
            tab: None,
            checked_externs: Vec::new(),
            maps: false,
            function_roots: roots::Roots::default(),
            rooted_set: std::collections::HashSet::new(),
            variadic_set: Vec::new(),
            check: None,
            gc_functions: 0,
            maps_in_jit: false,
            vm_stack_trace: None,
            stack_trace_in_jit: false,
        }
    }

    /// Delegates the registration of the stack trace sections to the memory manager of the JIT.
    /// The module stops emitting the linker symbols and the registration call.
    pub fn with_stack_trace_in_jit(mut self, yes: bool) -> Self {
        self.stack_trace_in_jit = yes;
        self
    }

    /// The symbolic stack trace (§13.14): the labels before the calls and the
    /// table of the image (`target::symbolic_stack_trace`; in the JIT, with
    /// [`LlvmEmitter::with_stack_trace_in_jit`]).
    pub fn with_stack_trace(mut self, yes: bool) -> Self {
        self.vm_stack_trace = yes.then(stack_trace::StackTrace::new);
        self
    }

    /// The registration call of the stack maps of the image, per format: the
    /// base of the PE image (the runtime finds `.dfgcm` and `.llvm_st` through the
    /// headers); the bounds of the `dfgcm` section that `ld.lld` defines; on
    /// Mach-O, those of LLVM's `__llvm_stackmaps`.
    fn maps_registration_call() -> &'static str {
        match crate::target::target_os() {
            crate::target::Os::Windows => "  call void @dartforge_register_map(ptr @__ImageBase)\n",
            crate::target::Os::Linux => "  call void @dartforge_register_map_section(ptr @__start_dfgcm, ptr @__stop_dfgcm)\n",
            crate::target::Os::MacOs => {
                "  call void @dartforge_register_map_llvm(ptr @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\", ptr @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\")\n"
            }
        }
    }

    /// The registration of the `dart:async` fields that the stack trace walks (§13.14).
    fn trace_field_registration_calls(&self) -> String {
        let mut s = String::new();
        for (cid, name_str, position) in &self.module.trace_fields {
            let idx = self.string_const_index(name_str.as_bytes()).unwrap_or(0);
            writeln!(s, "  call void @dartforge_register_stack_trace_field(i64 {cid}, ptr @.str.{idx}, i64 {}, i64 {position})", name_str.len()).unwrap();
        }
        // The tear-off entries of the stream branches.
        // Only those that survived pruning (the entry disappears with its creator).
        for (sym_name, kind) in &self.module.stack_trace_tearoffs {
            if !self.module.functions.iter().any(|f| f.symbol == *sym_name) {
                continue;
            }
            writeln!(s, "  call void @dartforge_register_stack_trace_tearoff(ptr @{sym_name}, i64 {kind})").unwrap();
        }
        s
    }

    /// The registration call of the stack trace table of the image (§13.14).
    fn trace_registration_call(&self) -> Option<String> {
        self.vm_stack_trace.as_ref()?;
        if self.stack_trace_in_jit {
            return None;
        }
        let (_, start, end) = stack_trace::markers();
        Some(format!("  call void @dartforge_register_stack_trace(ptr {start}, ptr {end})\n"))
    }

    /// Gets the address of the fields through a call to `@df.body`, out of line.
    /// Used in the development profile to reduce the size of the IR.
    pub fn with_fields_per_call(mut self, yes: bool) -> Self {
        self.fields_per_call = yes;
        self
    }

    /// Selects the production profile; with fields per call, keeps
    /// `@df.body` out of line through `noinline`.
    pub fn with_production(mut self, yes: bool) -> Self {
        self.production = yes;
        self
    }

    /// Marks roots frames as ordered for the runtime check.
    /// Requires emission without inlining, with one native frame per function.
    pub fn with_ordered_frames(mut self, yes: bool) -> Self {
        self.ordered_frames = yes;
        self
    }

    /// No object will have an outside body (the `OUT_OF_LINE` bit, which only the reload migration
    /// of the JIT creates — `runtime/src/heap.rs`, `swap_fields`): AOT
    /// and the production SDK. With this, the field of the `this` of a method is read
    /// directly from the block (`Self::direct_body`).
    pub fn with_body_without_outside(mut self, yes: bool) -> Self {
        self.body_without_outside = yes;
        self
    }

    /// Applies `optsize` to each function emitted in the module.
    pub fn with_optimize_size(mut self, yes: bool) -> Self {
        self.optimize_size = yes;
        self
    }

    /// The out-of-line helpers of the program module (see
    /// [`out_of_line_helpers`]); an SDK module ignores them.
    pub fn with_outlined_helpers(mut self, names: Vec<String>) -> Self {
        if self.module.sdk_library {
            return self;
        }
        self.out_of_line_allocation = names.iter().any(|n| n == "new_instance");
        self.compact_selector = names.iter().any(|n| n == "selector");
        // `@df.selector` stays `alwaysinline`: only `@df.selector_d`
        // (out of line) calls it.
        self.outlined_helpers = names.into_iter().filter(|n| n != "selector").collect();
        self
    }

    pub fn with_static_objects(mut self, yes: bool) -> Self {
        self.static_objects = yes;
        self
    }

    /// Delegates the registration of the stack maps to the memory manager of the JIT.
    /// The module stops emitting the registration and the `-ni:1` layout option.
    pub fn with_maps_in_jit(mut self, yes: bool) -> Self {
        self.maps_in_jit = yes;
        self
    }

    /// Emits SSA roots through the stack maps of LLVM's statepoints.
    /// The `alloca`s of references stay in the frame of the shadow stack.
    pub fn with_roots_by_maps(mut self, yes: bool) -> Self {
        self.maps = yes;
        self
    }

    pub fn with_tables_at_link(mut self, yes: bool) -> Self {
        self.tables_at_link = yes;
        self
    }

    /// Production (the program and the SDK modules): the method tables assembled
    /// at link time are registered when the isolate is prepared, and the inline
    /// allocation skips the registration test (`Self::emit_inline_allocation`).
    /// The program module declares `@df.register.sdk_tables`, which
    /// `pruning::assemble` defines with one `dartforge_register_table` per
    /// assembled table, and calls it from `df.prepare_isolate` right after
    /// `df.register.program`. Development and the JIT keep the lazy
    /// registration (`dartforge_object_new_t`): a reload republishes the tables
    /// with `n_registered = 0`.
    pub fn with_tables_registered_at_startup(mut self, yes: bool) -> Self {
        self.tables_registered_at_startup = yes;
        self
    }

    pub(crate) fn with_arc_ports_at_link(mut self, yes: bool) -> Self {
        self.arc_ports_at_link = yes && crate::pruning::arc_ports::enabled();
        self
    }

    /// The emitter of a new generation of a running program (JIT hot
    /// reload): `previous` are the names of the slots of the area of the live generation
    /// ([`crate::ir_area`]). The globals that remain keep their index and
    /// the new ones come after, so the code of both generations reads and writes the
    /// same area — the suspended `async` frames and the closures that still
    /// run old code see the same statics as the new code.
    pub fn with_previous_area(mut self, previous: Option<Vec<i64>>) -> Self {
        self.previous_area = previous;
        self
    }

    /// The area descriptor with only the key and the number of slots, `[key, -n]`
    /// (the sign marks the lean form for `dartforge_globals_area`): the
    /// hashes of the names only serve reload (the migration of the area and the
    /// extension of the layout), and the production executable never reloads. The
    /// JIT and the development DLL keep the complete descriptor.
    pub fn with_lean_area(mut self, lean: bool) -> Self {
        self.lean_area = lean;
        self
    }

    /// The whole module. A module with diagnostics does not get here:
    /// `emit_ir` returns the error before (N1).
    ///
    /// ```
    /// use dartforge_emit_native::{hir::Module, llvm::LlvmEmitter};
    /// let module = Module::new();
    /// let ir = LlvmEmitter::new(&module).emit_all();
    /// assert!(ir.contains("declare"));
    /// ```
    ///
    /// # Panics
    /// A module with diagnostics or incoherent exceptional metadata.
    /// Resume still requires Itanium without statepoints; Windows needs funclets.
    pub fn emit_all(mut self) -> String {
        if !self.registry_closed {
            self.receiver_provenance = Some(crate::pruning::receiver_provenance::Provenance::default());
        }
        if self.receiver_provenance.is_none() {
            self.receiver_provenance = Some(if crate::pruning::receiver_provenance::enabled() {
                crate::pruning::receiver_provenance::analyze(&[self.module])
            } else { crate::pruning::receiver_provenance::Provenance::default() });
        }
        assert!(self.module.errors.is_empty(), "emit_all with diagnostics: {:?}", self.module.errors);
        assert!(crate::target::is_unix() || !self.module.tables.iter().any(|t|
            t.exits.values().any(|s| *s == ExceptionExit::Resume)),
            "Resume requires Itanium; Windows needs SEH funclets");
        // Collects the string literals of the module for declaration as global constants
        self.collect_string_constants();

        // 1. Target header
        self.emit_header();

        // 2. Runtime declarations
        self.emit_runtime_decls();
        self.emit_arc_aot_ports();

        // 3. Global string constants
        self.emit_string_constants();

        // 4. Classes and vtables
        self.emit_vtables();

        // 4b. User globals (N6)
        self.emit_globals();

        // 4c. Code table of the closures and constant vectors (P1)
        self.emit_closures();

        // 5. Compiled functions
        let module = self.module;
        if module.table_exceptions {
            assert!(
                module.tables.len() == module.functions.len(),
                "compiler bug: the module's functions changed after the table exceptions pass"
            );
        }
        for (k, func) in module.functions.iter().enumerate() {
            self.tab = if module.table_exceptions { module.tables.get(k) } else { None };
            self.emit_function(func);
        }
        self.tab = None;
        for d in std::mem::take(&mut self.variadic_set) {
            self.out.push_str(&d);
        }

        if self.module.sdk_library {
            // A source SDK library (P5c): without the entry or the `toString`
            // dispatch, which belong to the program; the registration function of its
            // classes, which the program entry calls.
            let reg = self.module.record.clone().unwrap_or_else(|| "df.register".to_string());
            self.emit_registration(&reg);
            self.emit_selector_globals();
            self.emit_area_descriptor();
            self.out.push_str(GET_AREA);
            self.emit_string_globals();
            self.emit_external_declarations();
            self.close_maps();
            if let Some(r) = self.vm_stack_trace.take() {
                r.finish(&mut self.out);
            }
            if let Some(d) = self.debug_info.take() {
                d.finish(&mut self.out);
            }
            return self.out;
        }

        // 6. Polymorphic dispatch functions
        if self.module.sdk_mode {
            self.emit_to_string_by_selector();
        } else {
            self.emit_dispatch_functions();
        }

        // 7. Entrada global @dartforge_entry
        self.emit_entry();

        self.emit_selector_globals();
        self.emit_area_descriptor();
        // The area of this isolate with the layout of this generation, created (or
        // extended) now: the publication of a JIT reload calls it at the
        // safepoint, before the new code runs.
        self.out.push_str(
            "define void @df.prepare_area() {\n  %a = call ptr @dartforge_globals_area(ptr @df.area)\n  ret void\n}\n",
        );
        // The area in the prologue of each function: the module id indexes the table
        // of this thread in the `Context` (offsets 16 and 24); an absent entry
        // goes to the runtime, which fills it (and gives the id the first time).
        // The index of the area of the program has its own name: partitioning
        // (`partition.rs`) promotes it to external, and the full LTO of the
        // SDK modules, when splitting the joined module into partitions, promotes the
        // internal `@df.area_id` of one of them with the same name — "duplicate
        // symbol: df.area_id" (docs/NATIVO-PRODUCAO-GRANDE.md §5).
        let get_area = GET_AREA.replace("@df.area_id", "@df.area_id.program");
        self.out.push_str(&out_of_line(&get_area, &self.outlined_helpers));
        self.emit_string_globals();
        self.emit_external_declarations();
        self.close_maps();
        if let Some(r) = self.vm_stack_trace.take() {
            r.finish(&mut self.out);
        }
        if let Some(d) = self.debug_info.take() {
            d.finish(&mut self.out);
        }
        self.out
    }

    /// Roots by maps on Mach-O: the `__llvm_stackmaps` stays in LLVM's format
    /// (`ld64.lld` does not separate the ThinLTO indices, and the converter
    /// does not run in the linker's LTO), and nothing references it — `-dead_strip` would
    /// discard it. `.no_dead_strip` marks as live the local atom
    /// `__LLVM_StackMaps` that LLVM emits in each object with a map, without
    /// making it global (Perry does the same,
    /// `perry-codegen/src/module.rs`). Only in a module with a `gc` function: without
    /// a map, the symbol does not exist.
    ///
    /// Before, the empty `"deopt"()` on every call that may collect and does not
    /// have its own yet ([`empty_deopt`]).
    fn close_maps(&mut self) {
        if self.maps {
            self.out = empty_deopt(&self.out);
        }
        if self.maps && !self.maps_in_jit && self.gc_functions > 0 && crate::target::target_os() == crate::target::Os::MacOs {
            self.out.push_str("module asm \".no_dead_strip __LLVM_StackMaps\"\n");
        }
    }

    fn collect_string_constants(&mut self) {
        for func in &self.module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    if let Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) = inst {
                        let bytes: &[u8] = match inst {
                            Instruction::Const(Constant::String(s)) => s.as_bytes(),
                            Instruction::Const(Constant::StringWtf8(s)) => s,
                            _ => unreachable!(),
                        };
                        Self::register_string(&mut self.string_constants, &mut self.string_index, bytes);
                    }
                }
            }
        }
        for class in &self.module.classes {
            Self::register_string(&mut self.string_constants, &mut self.string_index, class.name.as_bytes());
        }
        for (_, name_str, _) in &self.module.trace_fields {
            Self::register_string(&mut self.string_constants, &mut self.string_index, name_str.as_bytes());
        }
    }

    /// Adds `bytes` to the constant table, if not there yet.
    fn register_string(table: &mut Vec<Vec<u8>>, idx_value: &mut std::collections::HashMap<Vec<u8>, usize>, bytes: &[u8]) {
        if !idx_value.contains_key(bytes) {
            idx_value.insert(bytes.to_vec(), table.len());
            table.push(bytes.to_vec());
        }
    }

    fn emit_header(&mut self) {
        if self.maps && !self.maps_in_jit {
            // `-ni:1`: `addrspace(1)` is of non-integral pointers — the
            // optimizer does not fabricate `ptrtoint`/`inttoptr` with the roots.
            self.out.push_str(&crate::target::ir_header().replacen("-S128\"", "-S128-ni:1\"", 1));
        } else {
            self.out.push_str(crate::target::ir_header());
        }
        // The ids of the program classes, for the next generation of a
        // JIT reload (J03, `context::ir_ids`).
        if crate::pruning::universal_parent::declared(self.module) {
            self.out.push_str("; df.universal-parent 0\n");
        }
        let mut nominal_classes: Vec<_> = self.module.pruning_class_ids.iter().copied().collect();
        nominal_classes.sort_unstable();
        for cid in nominal_classes {
            writeln!(self.out, "; df.cone-class {cid}").unwrap();
        }
        let mut ambiguous_classes: Vec<_> = self.module.pruning_ambiguous_class_ids.iter().copied().collect();
        ambiguous_classes.sort_unstable();
        for cid in ambiguous_classes {
            writeln!(self.out, "; df.cone-ambiguous {cid}").unwrap();
        }
        let mut parent_edges = self.module.subtyping_edges.clone();
        parent_edges.sort_unstable();
        parent_edges.dedup();
        for (child, parent) in parent_edges {
            writeln!(self.out, "; df.cone-parent {child} {parent}").unwrap();
        }
        for (id, lib, class) in &self.module.program_ids {
            writeln!(self.out, "; df.class {id} {} {}", crate::context::escape_str(lib), crate::context::escape_str(class)).unwrap();
        }
        let mut frames: Vec<_> = self.module.async_frame_layouts.iter().filter(|(owner, site, _, count)| {
            self.module.functions.iter().find(|f| &f.symbol == owner).is_some_and(|f| {
                f.blocks.iter().flat_map(|b| &b.instructions).any(|(id, instruction, _)| {
                    id == site && matches!(instruction, Instruction::AllocObject { class_id, fields }
                        if *class_id == crate::lower::async_sm::ASYNC_FRAME_ID && fields.len() == *count)
                })
            })
        }).collect();
        frames.sort_unstable_by(|left, right| left.0.cmp(&right.0)
            .then(left.1.0.cmp(&right.1.0)).then(left.2.cmp(&right.2)).then(left.3.cmp(&right.3)));
        for (owner, site, body, fields) in frames {
            writeln!(self.out, "; df.async-frame {owner} v{} {body} {fields}", site.0).unwrap();
        }
        // The layout of the objects, to migrate the live instances (J03):
        // `name:flags:ty:owner:type_identity`, with `a` nullable and `l` late. The
        // declaring owner distinguishes inherited fields with equal spellings.
        for (id, base, fields) in &self.module.program_fields {
            let list: Vec<String> = fields
                .iter()
                .map(|c| {
                    let flags = format!("{}{}", if c.nullable { "a" } else { "" }, if c.late { "l" } else { "" });
                    let owner = c.owner.as_deref().map(crate::context::escape_str).unwrap_or_default();
                    let identity = c.type_identity.as_deref().map(crate::context::escape_str).unwrap_or_default();
                    format!("{}:{flags}:{}:{owner}:{identity}", crate::context::escape_str(&c.name), crate::context::escape_str(&c.ty))
                })
                .collect();
            writeln!(self.out, "; df.fields {id} {base} {}", list.join(",")).unwrap();
        }
    }

    fn emit_runtime_decls(&mut self) {
        self.out.push_str("; Declarations of the native Rust runtime (table in llvm/externs.rs)\n");
        for e in externs::EXTERNS {
            self.out.push_str(e.decl);
            // Roots by maps: the extern that does not collect is a leaf — the call
            // to it does not become a collection point (nor an entry in the map).
            if self.maps {
                let ef = externs::effects_of(e.name());
                if (!ef.allocates && !ef.calls_dart) || crate::target::sabotaged_leaf(e.name()) {
                    self.out.push_str(" \"gc-leaf-function\"");
                }
            }
            self.out.push('\n');
        }
        if self.vm_stack_trace.is_some() && !self.stack_trace_in_jit {
            // The start and the end of the stack trace section of the image (§13.14).
            self.out.push_str(stack_trace::markers().0);
        }
        if self.maps && !self.maps_in_jit {
            // The registration of the map of the image (`dartforge_register_map*`): the
            // base of the PE image (the runtime finds `.dfgcm` and `.llvm_st` through the
            // headers); the bounds of the `dfgcm` section that `ld.lld` defines;
            // on Mach-O, those of LLVM's `__llvm_stackmaps`.
            self.out.push_str(match crate::target::target_os() {
                crate::target::Os::Windows => "declare void @dartforge_register_map(ptr)\n@__ImageBase = external constant i8\n",
                crate::target::Os::Linux => {
                    "declare void @dartforge_register_map_section(ptr, ptr)\n@__start_dfgcm = external hidden global i8\n@__stop_dfgcm = external hidden global i8\n"
                }
                crate::target::Os::MacOs => {
                    "declare void @dartforge_register_map_llvm(ptr, ptr)\n\
                     @\"\\01section$start$__LLVM_STACKMAPS$__llvm_stackmaps\" = external hidden global i8\n\
                     @\"\\01section$end$__LLVM_STACKMAPS$__llvm_stackmaps\" = external hidden global i8\n"
                }
            });
        }
        // The native calls with a struct by value copy the bytes into a
        // stack temporary (`llvm/c_abi.rs`).
        let composites = self.module.functions.iter().any(|f| {
            f.blocks.iter().any(|b| b.instructions.iter().any(|(_, i, _)| matches!(i, Instruction::CompositeNativeCall { .. })))
        });
        self.out.push_str(simd::DECLARATIONS);
        self.out.push_str("declare i8 @llvm.expect.i8(i8, i8)\n");
        self.out.push_str("declare i1 @llvm.expect.i1(i1, i1)\n");
        self.out.push_str("declare i64 @llvm.read_register.i64(metadata)\n");
        for (_, decl) in externs::GLOBALS {
            self.out.push_str(decl);
            self.out.push('\n');
        }
        // The `@df.*` helpers of each package (§3.5).
        let helpers_start = self.out.len();
        self.out.push_str(ir_strings::HELPERS);
        self.out.push_str(ir_boxes::HELPERS);
        if self.arc_tag_guards { self.out.push_str(arc_tag_guards::HELPERS); }
        self.out.push_str(&ir_lists::helpers(self.module.arc_memory));
        self.out.push_str(ir_typed::HELPERS);
        let value_class = value_class(&self.module.runtime_cids);
        self.out.push_str(&value_class);
        self.out.push_str(&space_helpers());
        self.out.push_str(&body_helper(self.fields_per_call, self.production, self.body_without_outside));
        if !self.outlined_helpers.is_empty() {
            let helpers = self.out.split_off(helpers_start);
            self.out.push_str(&out_of_line(&helpers, &self.outlined_helpers));
        }
        if self.maps {
            let helpers = self.out.split_off(helpers_start);
            self.out.push_str(&leaf_helpers(&helpers));
        }
        if self.out_of_line_allocation {
            // Without the closed set of active program tables (experimental), every
            // table of the module is registered at startup, and the helper
            // has no registration test.
            let registered = self.tables_registered_at_startup && self.module.closed_class_tables.is_none();
            self.out.push_str(&new_instance(registered));
        }
        if self.compact_selector {
            self.out.push_str(SELECTOR_BY_DESCRIPTOR);
        }
        if composites || !self.module.ffi_callbacks.is_empty() {
            self.out.push_str(
                "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\n\
                 declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\n\
                 declare ptr @llvm.stacksave.p0()\n\
                 declare void @llvm.stackrestore.p0(ptr)\n",
            );
        }
        if self.module.table_exceptions {
            self.out.push_str(if !crate::target::is_unix() { EXCEPTIONS_BY_TABLES } else { EXCEPTIONS_BY_TABLES_ITANIUM });
        }
        if externs::check_effects() {
            // The frame of the check does not collect: with roots by maps, it is a
            // leaf (no statepoint around it).
            if self.maps {
                self.out.push_str(
                    "declare void @dartforge_effects_before(ptr, i64, i64) \"gc-leaf-function\"\ndeclare void @dartforge_effects_after() \"gc-leaf-function\"\n",
                );
            } else {
                self.out.push_str("declare void @dartforge_effects_before(ptr, i64, i64)\ndeclare void @dartforge_effects_after()\n");
            }
            self.out.push_str("declare i64 @dartforge_effects_level() nounwind \"gc-leaf-function\"\ndeclare void @dartforge_effects_restore(i64) nounwind \"gc-leaf-function\"\n");
        }
        self.out.push('\n');
    }

    fn emit_string_constants(&mut self) {
        if self.string_constants.is_empty() {
            return;
        }
        self.out.push_str("; Constantes de string WTF-8\n");
        for (idx, s) in self.string_constants.iter().enumerate() {
            let bytes = s.as_slice();
            let len = bytes.len();
            let mut escaped = String::new();
            for &b in bytes {
                if (b as char).is_ascii_alphanumeric() || b == b' ' || b == b'_' || b == b'.' {
                    escaped.push(b as char);
                } else {
                    write!(escaped, "\\{:02X}", b).unwrap();
                }
            }
            writeln!(
                self.out,
                "@.str.{idx} = private unnamed_addr constant [{len} x i8] c\"{escaped}\""
            ).unwrap();
        }
        self.out.push('\n');
    }

    fn string_const_index(&self, s: &[u8]) -> Option<usize> {
        self.string_index.get(s).copied()
    }

    fn emit_vtables(&mut self) {
        // Global vtables if there are classes
        for class in &self.module.classes {
            writeln!(self.out, "; VTable of class {} (id {})", class.name, class.id).unwrap();
        }
    }

    fn emit_function(&mut self, func: &Function) {
        let arc_retain_target = self.arc_retain_target();
        let arc_release_target = self.arc_release_target();
        self.current_function.clone_from(&func.symbol);
        self.current_exact_receivers = self.receiver_provenance.as_ref()
            .and_then(|proof| proof.values_for(func)).cloned().unwrap_or_default();
        self.current_receiver_cone = self.module.pruning_this_receivers.get(&func.symbol)
            .filter(|_| self.registry_closed)
            .copied().filter(|(id, _)| func.params.iter().any(|(parameter, _, ty)| parameter == id && *ty == Type::Ref));
        self.cache_in_function = 0;
        self.frame_allocas = Self::escaping_ref_allocas(func);
        self.prepare_types_and_phis(func);
        let immediate_refs = arc_immediates::ImmediateRefs::new(
            func, !self.function_by_symbol.contains_key("dartforge_arc_box_int_owned_v1"),
        );

        let ret_ty = func.return_ty.llvm_ir();
        let params: Vec<String> = func
            .params
            .iter()
            .map(|(vid, _, ty)| format!("{} %v{}", ty.llvm_ir(), vid.0))
            .collect();
        let params_str = params.join(", ");

        let (linkage, comdat) = self.linkage_of(&func.symbol);
        // One copy per image (`linkonce_odr`): the stack trace does not label it.
        let shared = !linkage.is_empty();
        // §7.4, shadow mode: the `Ref` values of the function (the candidates of
        // `roots::analyze`) for the dominance checker.
        if !self.maps && verify_shadow::linked() {
            let mut refs: Vec<u32> = func.params.iter().filter(|(_, _, t)| *t == Type::Ref).map(|(v, _, _)| v.0).collect();
            for b in &func.blocks {
                for (v, inst, t) in &b.instructions {
                    if *t == Type::Ref && !matches!(inst, Instruction::Const(Constant::Null) | Instruction::Alloca(_)) {
                        refs.push(v.0);
                    }
                }
            }
            let list: Vec<String> = refs.iter().map(|v| format!("v{v}")).collect();
            writeln!(self.out, "{} {}", verify_shadow::REFS_MARK, list.join(" ")).unwrap();
        }
        let function_start = self.out.len();
        let mut written_position: Option<(u32, u32)> = None;
        // The position of an instruction copied by inlining (only the stack trace).
        let mut written_embedded: Option<(u32, u32, u32)> = None;
        let inline_attribute = match self.module.inline_policies.get(&func.symbol) {
            Some(InlinePolicy::Never) => " noinline",
            // Debug/stack-trace policies retain precedence over a preference.
            Some(InlinePolicy::Prefer) if func.debug_info.is_none() && !self.module.dwarf => " inlinehint",
            _ => "",
        };
        // Dart recursion must still grow the stack and throw StackOverflowError.
        // Reading sp does not make a frame escape: without this attribute LLVM
        // can replace a recursive call with a loop and hoist its stack check.
        let attributes = format!("{}{} \"disable-tail-calls\"=\"true\"", if self.optimize_size { " optsize" } else { "" }, inline_attribute);
        // The `this` of a method is always an object of the space (never null or a
        // `Smi`: the class has the read field); without an outside body, the address
        // of its fields is `this − 2 + 16`.
        self.this_object = func
            .params
            .first()
            .filter(|(_, name_str, t)| self.body_without_outside && !self.module.arc_memory && name_str == "this" && *t == Type::Ref)
            .map(|(vid, _, _)| vid.0);
        // Exceptions by tables: the decision of the pass for this function. Only the
        // function with a landing pad names the personality (it is the one that reads the table of the
        // landing pads); the unwinding crosses the others.
        let tab = self.tab;
        let has_landing_pad = tab.is_some_and(|t| !t.landing_pads.is_empty());
        let has_resume = tab.is_some_and(|t| t.exits.values().any(|s| *s == ExceptionExit::Resume));
        if let Some(c) = tab.and_then(|t| t.foreign_cleanup.as_ref()) {
            c.check(func, tab.unwrap()).expect("invalid foreign cleanup");
        }
        let has_cleanup = has_resume || (crate::target::is_unix() && tab.is_some_and(|t| t.foreign_cleanup.is_some()));
        assert!(!has_cleanup || !self.maps, "Resume does not admit a statepoint landingpad token");
        if has_resume {
            crate::optimize::arc::check_resumes(func, tab.unwrap()).expect("invalid Resume");
        }
        let personality = if has_cleanup { " personality ptr @dartforge_personality_cleanup_itanium" }
            else if has_landing_pad { " personality ptr @dartforge_personality" } else { "" };

        // G1/G2 (docs/NATIVO-PLANO.md §6.5): one slot per `alloca` of type
        // `Ref`, and the `Ref` SSA values live at some collection point, with a
        // slot shared among those that are never live together
        // (`roots.rs`).
        let converting_blocks: std::collections::HashSet<u32> = self.conv_phi.iter().map(|(b, ..)| *b).collect();
        let mut analysis = roots::analyze_with(
            func,
            &self.types,
            &|inst| self.may_collect(inst),
            &|b| converting_blocks.contains(&b.0),
            &|inst| self.callee_roots_the_arguments(inst),
        );
        // The budget of `rewrite-statepoints-for-gc` (§3.4): the pass grows
        // superlinearly with `live × collection points`. Above the ceiling
        // the whole function stays on the shadow stack, as a residual frame
        // (§3.7): without `gc`, with the usual slots.
        let cost = analysis.rooted_set.len() as u64 * (analysis.live_at.len() + analysis.live_at_end.len()) as u64;
        let within_budget = cost <= crate::target::maps_budget();
        if self.maps && !within_budget && std::env::var_os("DARTFORGE_MAPS_REPORT").is_some() {
            eprintln!("dartforge: {} outside the maps budget ({cost}): shadow stack", func.symbol);
        }
        if self.maps && within_budget {
            // Roots by maps: only the `Ref` `alloca`s stay in the frame (the
            // stack map describes SSA values, not memory); the rooted SSA
            // values go into the map.
            analysis.slots.retain(|v, _| self.pointee.contains_key(v));
            self.rooted_set = analysis.rooted_set.iter().copied().collect();
        } else {
            self.rooted_set.clear();
        }
        self.slots = std::mem::take(&mut analysis.slots);
        self.function_roots = analysis;
        // Roots by maps: every function is `gc` — the
        // `rewrite-statepoints-for-gc` turns its calls that may
        // collect into statepoints (`crates/llvm`, `generate`), each one with the
        // `"deopt"` operand of the values live at it (§14.12): empty in a function without
        // roots in the map (those outside the budget are in the frame), but
        // present, so that the inliner joins to it the live values of its callers.
        let has_gc = self.maps;
        if !self.rooted_set.is_empty() {
            self.gc_functions += 1;
        }
        // The checking build: as many extra slots as the largest
        // set of live values at a collection point.
        self.check = None;
        if !self.rooted_set.is_empty() && crate::target::check_roots() {
            let k = self
                .function_roots
                .live_at
                .values()
                .chain(self.function_roots.live_at_end.values())
                .map(Vec::len)
                .max()
                .unwrap_or(0);
            if k > 0 {
                let first_one = self.slots.values().max().map_or(0, |m| m + 1);
                self.check = Some((first_one, k));
            }
        }
        let gc = if has_gc { " gc \"statepoint-example\"" } else { "" };
        writeln!(self.out, "define {linkage}{ret_ty} @{}({}){attributes}{comdat}{gc}{personality} {{", func.symbol, params_str)
            .unwrap();
        self.has_frame = !self.slots.is_empty() || self.check.is_some();
        self.exit_labels = func
            .blocks
            .iter()
            .filter_map(|b| {
                b.instructions.iter().rev().find_map(|(vid, i, _)| {
                    let v = vid.0;
                    if tab.is_some_and(|t| t.invocations.contains_key(vid)) {
                        // The normal path of an `invoke` ([`Self::make_invoke`]).
                        Some((b.id.0, format!("inv{v}.fim")))
                    } else if Self::inline_allocation(i).is_some() && !self.out_of_line_allocation {
                        Some((b.id.0, format!("ao{v}.fim")))
                    } else if self.inline_closure_entry(i) {
                        Some((b.id.0, format!("ce{v}.fim")))
                    } else if self.inline_barrier(i) {
                        Some((b.id.0, format!("wb{v}.fim")))
                    } else if !self.static_objects && matches!(i, Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))) {
                        // Without the static objects (the JIT), the literal goes to the
                        // cache of the use site (`ir_strings`), which splits the
                        // block; with them, it is a constant (§2.11).
                        Some((b.id.0, format!("ls{v}.fim")))
                    } else if matches!(i, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_exception_clear") {
                        Some((b.id.0, format!("xc{v}.fim")))
                    } else if !self.maps && Self::rti_evaluate_cached(i) {
                        Some((b.id.0, format!("ra{v}.fim")))
                    } else if Self::rti_test_cached(i) {
                        Some((b.id.0, format!("rt{v}.fim")))
                    } else {
                        None
                    }
                })
            })
            .collect();
        // The landing pad with the Dart/foreign branch ends in `lpad{b}.dart`
        // (below): a `phi` emitted before it already needs the label.
        if has_cleanup && let Some(t) = tab {
            for p in &t.landing_pads {
                if t.exits.get(p) != Some(&ExceptionExit::Resume) {
                    self.exit_labels.entry(p.0).or_insert_with(|| format!("lpad{}.dart", p.0));
                }
            }
        }
        self.has_ctx = self.has_frame || Self::requires_explicit_context(func, tab);

        for block in &func.blocks {
            writeln!(self.out, "b{}:", block.id.0).unwrap();
            let block_start = self.out.len();
            self.current_label = format!("b{}", block.id.0);
            // The landing pad of an `invoke`: whoever unwound up to here left the
            // exception pending and its roots frames (and those of the crossed
            // frames) still chained — the top of the shadow stack goes back
            // to that of this function, before any other instruction.
            if tab.is_some_and(|t| t.landing_pads.contains(&block.id)) {
                if tab.is_some_and(|t| t.exits.get(&block.id) == Some(&ExceptionExit::Resume)) {
                    writeln!(self.out, "  %lpad{} = landingpad {{ ptr, i32 }} cleanup", block.id.0).unwrap();
                } else if has_gc {
                    // In a `gc` function the `invoke` becomes a statepoint, and its
                    // landing pad is a `token` (what the relocations of the
                    // exception path reference).
                    writeln!(self.out, "  %lpad{} = landingpad token cleanup", block.id.0).unwrap();
                } else {
                    let cleanup = if has_cleanup { " cleanup" } else { "" };
                    writeln!(self.out, "  %lpad{} = landingpad {{ ptr, i32 }}{cleanup} catch ptr null", block.id.0).unwrap();
                }
                let top = if self.has_frame { "%gcq" } else { "%topo0" };
                if !crate::target::sabotage("pad_without_top") {
                    writeln!(self.out, "  store ptr {top}, ptr %ctxtopo, align 8").unwrap();
                }
                if externs::check_effects() {
                    writeln!(self.out, "  call void @dartforge_effects_restore(i64 %efnivel)").unwrap();
                }
                if has_cleanup && tab.is_some_and(|t| t.exits.get(&block.id) != Some(&ExceptionExit::Resume)) {
                    // The inliner may combine catch and cleanup. A zero selector
                    // runs cleanup, but does not authorize treating a foreign exception as Dart.
                    let b = block.id.0;
                    writeln!(self.out, "  %lpseletor{b} = extractvalue {{ ptr, i32 }} %lpad{b}, 1").unwrap();
                    writeln!(self.out, "  %lptipo{b} = call i32 @llvm.eh.typeid.for(ptr null)").unwrap();
                    writeln!(self.out, "  %lpdart{b} = icmp eq i32 %lpseletor{b}, %lptipo{b}").unwrap();
                    writeln!(self.out, "  br i1 %lpdart{b}, label %lpad{b}.dart, label %lpad{b}.estrangeira").unwrap();
                    writeln!(self.out, "lpad{b}.estrangeira:").unwrap();
                    if let Some(c) = tab.and_then(|t| t.foreign_cleanup.as_ref()) {
                        for v in c.owners(block.id) {
                            writeln!(self.out, "  call void @{arc_release_target}(i64 %v{})", v.0).unwrap();
                        }
                        for q in c.frames(block.id).iter().rev() {
                            writeln!(self.out, "  call void @dartforge_arc_frame_close_v1(i64 %v{})", q.0).unwrap();
                        }
                    }
                    if self.stack_trace.is_some() {
                        writeln!(self.out, "  call void @dartforge_call_trace_exit()").unwrap();
                    }
                    if self.has_frame {
                        writeln!(self.out, "  store ptr %gcant, ptr %ctxtopo, align 8").unwrap();
                    }
                    writeln!(self.out, "  resume {{ ptr, i32 }} %lpad{b}").unwrap();
                    writeln!(self.out, "lpad{b}.dart:").unwrap();
                    self.current_label = format!("lpad{b}.dart");
                    self.exit_labels.entry(b).or_insert_with(|| format!("lpad{b}.dart"));
                }
                // Roots by maps: what the handler still reads was live at the
                // entry of the `invoke` and goes into its `"deopt"`.
            }
            if block.id.0 == 0 {
                self.emit_closure_buffers(func);
                if has_landing_pad && externs::check_effects() {
                    writeln!(self.out, "  %efnivel = call i64 @dartforge_effects_level()").unwrap();
                }
                // The roots frame is also born before any call
                // (N17): the `df.get_area` below is `alwaysinline` and has
                // branches, and an `alloca` after it stopped being in the entry
                // block — it became a dynamic allocation, with `__chkstk` and
                // `stacksave` on each call of the function, and prevented the inliner
                // from taking the frame into the entry block of the caller.
                // `DARTFORGE_NO_FRAME_AT_ENTRY=1` at compile time goes back to the
                // old place (measurement).
                if self.has_frame && Self::frame_at_entry() {
                    let n = self.frame_size();
                    writeln!(self.out, "  %gcq = alloca {{ ptr, i64, [{n} x i64] }}, align 8").unwrap();
                }
                if Self::uses_area(func) {
                    writeln!(self.out, "  %area = call ptr @df.get_area()").unwrap();
                }
            }
            if block.id.0 == 0
                && let Some(k) = self.stack_trace.as_mut()
            {
                let n = *k;
                *k += 1;
                self.stack_trace_names.push(func.symbol.clone());
                writeln!(self.out, "  call void @dartforge_call_trace_entry(ptr @df.call_trace.{n}, i64 {})", func.symbol.len()).unwrap();
            }
            // The thread context (`runtime/src/heap.rs`, `Context`): the
            // pending exception at offset 0, the top of the shadow stack at 8.
            if block.id.0 == 0 && self.has_ctx {
                writeln!(self.out, "  %ctx = call ptr @dartforge_context()").unwrap();
                writeln!(self.out, "  %ctxtopo = getelementptr inbounds i8, ptr %ctx, i64 8").unwrap();
                // Without a roots frame, the top that the landing pad restores is that of the
                // entry of the function.
                if has_landing_pad && !self.has_frame {
                    writeln!(self.out, "  %topo0 = load ptr, ptr %ctxtopo, align 8").unwrap();
                }
                self.emit_stack_check(func);
            }
            if block.id.0 == 0 && self.has_frame {
                // The roots frame on the stack of the function (the shadow stack,
                // the runtime's `RootFrame`): previous, number of slots and
                // the slots, zeroed before the runtime chains it. Each root
                // is a `store` into its slot (shared slots: the size
                // is that of the largest).
                let n = self.frame_size();
                let t = format!("{{ ptr, i64, [{n} x i64] }}");
                if !Self::frame_at_entry() {
                    writeln!(self.out, "  %gcq = alloca {t}, align 8").unwrap();
                }
                let header = self.frame_header(n);
                // The `dirty_frame` sabotage (D5, §7.3): the slots are born with a
                // handle value that points to no block; a slot
                // read before being written falls into handle validation.
                let slots = if n > 0 && crate::target::sabotage("dirty_frame") {
                    format!("[{}]", vec!["i64 4098"; n].join(", "))
                } else {
                    "zeroinitializer".to_string()
                };
                writeln!(self.out, "  store {t} {{ ptr null, i64 {header}, [{n} x i64] {slots} }}, ptr %gcq").unwrap();
                for slot in 0..n {
                    writeln!(self.out, "  %gcs{slot} = getelementptr inbounds {t}, ptr %gcq, i64 0, i32 2, i64 {slot}").unwrap();
                }
                // Chains the frame: `previous` = top; top = frame.
                writeln!(self.out, "  %gcant = load ptr, ptr %ctxtopo, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcant, ptr %gcq, align 8").unwrap();
                writeln!(self.out, "  store ptr %gcq, ptr %ctxtopo, align 8").unwrap();
                for (vid, _, _) in &func.params {
                    if let Some(&slot) = self.slots.get(vid) {
                        writeln!(self.out, "  store i64 %v{}, ptr %gcs{slot}", vid.0).unwrap();
                    }
                }
                let mut in_frame: Vec<ValueId> = self.frame_allocas.iter().copied().collect();
                in_frame.sort_by_key(|v| v.0);
                for vid in in_frame {
                    let slot = self.slots[&vid];
                    writeln!(self.out, "  %v{} = getelementptr inbounds i8, ptr %gcs{slot}, i64 0", vid.0).unwrap();
                }
            }
            if block.id.0 == 0 {
                // Roots by maps: the calls of the prologue (the area of the module,
                // the stack overflow, the conversions of the parameters) may
                // collect, with the rooted parameters live — every
                // rooted parameter is live at the entry.
                if !self.rooted_set.is_empty() {
                    let params: Vec<ValueId> = func.params.iter().map(|p| p.0).filter(|v| self.rooted_set.contains(v)).collect();
                    self.attach_deopt_with(block_start, &params, false);
                }
                // The `raw_in_map` sabotage: a value that the optimizer does not
                // fold (the volatile load) put in the map as a root.
                if !self.rooted_set.is_empty() && crate::target::sabotage("raw_in_map") {
                    self.out.push_str(
                        "  %dfsab = alloca i64, align 8\n  store volatile i64 4098, ptr %dfsab, align 8\n  %dfsabv = load volatile i64, ptr %dfsab, align 8\n",
                    );
                }
            }
            // `phi` must be the first instruction of the block: the roots of the
            // `phi`s all come out after the last of them.
            let mut phi_roots: Vec<(usize, u32)> = Vec::new();
            self.fresh_set.clear();
            self.fresh_maps.clear();

            for (instruction_index, (vid, inst, ty)) in block.instructions.iter().enumerate() {
                let v = vid.0;
                // J05: the position of the statement, when it changes (`llvm/debug_info.rs`);
                // that of an instruction copied by HIR inlining, with its
                // context, goes into a marker only for the stack trace (§13.14).
                if let Some(e) = func.debug_info.as_ref().and_then(|d| d.embedded_positions.get(vid)) {
                    if written_embedded != Some(*e) {
                        writeln!(self.out, "{}{} {} {}", debug_info::EMBEDDED_MARKER, e.0, e.1, e.2).unwrap();
                        written_embedded = Some(*e);
                        written_position = None;
                    }
                } else if let Some(p) = func.debug_info.as_ref().and_then(|d| d.positions.get(vid))
                    && written_position != Some(*p)
                {
                    writeln!(self.out, "{}{} {}", debug_info::MARKER, p.0, p.1).unwrap();
                    written_position = Some(*p);
                    written_embedded = None;
                }
                if !matches!(inst, Instruction::Phi { .. }) && !phi_roots.is_empty() {
                    for (slot, pv) in std::mem::take(&mut phi_roots) {
                        self.phi_root(slot, pv);
                    }
                }
                // The checking build: the values live at this collection point,
                // exactly, in the check slots.
                if self.check.is_some()
                    && let Some(live_set) = self.function_roots.live_at.get(vid).cloned()
                {
                    self.store_check(&live_set);
                }
                // The shadow stack: the slots that may still hold a value that
                // died are zeroed before the collection point (`roots::dead_slots`).
                if let Some(dead) = self.function_roots.clear_at.get(vid) {
                    self.clear_dead_slots(&dead.clone());
                }
                // A collection point may promote the freshly allocated object
                // (old after the collection): it stops being fresh.
                if self.may_collect(inst) {
                    self.fresh_set.clear();
                    self.fresh_maps.clear();
                }
                let instruction_start = self.out.len();
                match inst {
                    Instruction::ArcCopy { value } | Instruction::ArcMove { value } => {
                        // A proven non-handle has no physical owner. Keep its SSA token;
                        // the verified HIR and ownership plan remain unchanged.
                        let val_ref = self.arc_reference(value);
                        if matches!(inst, Instruction::ArcCopy { .. })
                            && !immediate_refs.contains(value, block.id, instruction_index) {
                            writeln!(self.out, "  call void @{arc_retain_target}(i64 {val_ref})").unwrap();
                        }
                        writeln!(self.out, "  %v{v} = add i64 {val_ref}, 0").unwrap();
                    }
                    Instruction::ArcDrop { value } => {
                        let val_ref = self.arc_reference(value);
                        if !immediate_refs.contains(value, block.id, instruction_index) {
                            writeln!(self.out, "  call void @{arc_release_target}(i64 {val_ref})").unwrap();
                        }
                    }
                    Instruction::ArcLoadStrong { slot: StrongSlot::Global { symbol: sym_name } } => {
                        self.check_arc_global(sym_name);
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  %v{v} = load i64, ptr %ga{v}").unwrap();
                        writeln!(self.out, "  call void @{arc_retain_target}(i64 %v{v})").unwrap();
                    }
                    Instruction::ArcStoreStrong { slot: StrongSlot::Global { symbol: sym_name }, value, mode } => {
                        self.check_arc_global(sym_name);
                        let val_ref = self.arc_reference(value);
                        self.global_address(v, sym_name);
                        if *mode == StrongStoreMode::Copy {
                            writeln!(self.out, "  call void @{arc_retain_target}(i64 {val_ref})").unwrap();
                        }
                        writeln!(self.out, "  store i64 {val_ref}, ptr %ga{v}").unwrap();
                        writeln!(self.out, "  %gr{v} = ptrtoint ptr %ga{v} to i64").unwrap();
                        if *mode == StrongStoreMode::Move {
                            writeln!(self.out, "  call void @dartforge_arc_global_receive_v1(i64 %gr{v}, i64 {val_ref})").unwrap();
                        } else {
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 %gr{v}, i64 {val_ref}) nounwind").unwrap();
                            writeln!(self.out, "  call void @{arc_release_target}(i64 {val_ref})").unwrap();
                        }
                    }
                    Instruction::ArcLoadStrong { slot } => {
                        let (frame, idx_value) = self.slot_arc(slot);
                        writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_frame_load_v1(i64 {frame}, i64 {idx_value})").unwrap();
                    }
                    Instruction::ArcStoreStrong { slot, value, mode } => {
                        let (frame, idx_value) = self.slot_arc(slot);
                        let val_ref = self.arc_reference(value);
                        let name_str = if *mode == StrongStoreMode::Copy { "copy" } else { "receive" };
                        writeln!(self.out, "  call void @dartforge_arc_frame_{name_str}_v1(i64 {frame}, i64 {idx_value}, i64 {val_ref})").unwrap();
                    }
                    Instruction::Const(Constant::Int(n)) => {
                        writeln!(self.out, "  %v{v} = add i64 0, {n}").unwrap();
                    }
                    Instruction::Const(Constant::Double(d)) => {
                        let bits = d.to_bits();
                        writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap();
                    }
                    Instruction::Const(Constant::Bool(b)) => {
                        let bit = if *b { 1 } else { 0 };
                        writeln!(self.out, "  %v{v} = add i1 0, {bit}").unwrap();
                    }
                    Instruction::Const(Constant::Null) => {
                        writeln!(self.out, "  %v{v} = add i64 0, 0").unwrap();
                    }
                    Instruction::Const(Constant::String(s)) => self.emit_const_string(v, s.as_bytes()),
                    Instruction::Const(Constant::StringWtf8(s)) => self.emit_const_string(v, s),
                    Instruction::Add(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = add i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Sub(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Mul(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = mul i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SDiv(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = sdiv i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::SRem(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = srem i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Shl(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = shl i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::AShr(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = ashr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::And(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = and i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Or(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = or i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Xor(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Neg(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sub i64 0, {sa}").unwrap();
                    }
                    Instruction::Not(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = xor i64 {sa}, -1").unwrap();
                    }
                    Instruction::FAdd(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fadd double {sa}, {sb}").unwrap();
                    }
                    Instruction::FSub(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fsub double {sa}, {sb}").unwrap();
                    }
                    Instruction::FMul(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fmul double {sa}, {sb}").unwrap();
                    }
                    Instruction::FDiv(a, b) => {
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fdiv double {sa}, {sb}").unwrap();
                    }
                    Instruction::FNeg(a) => {
                        let sa = self.coerce(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fneg double {sa}").unwrap();
                    }
                    Instruction::ICmp(op, a, b) => {
                        let op_str = match op {
                            ICmpOp::Eq => "eq",
                            ICmpOp::Ne => "ne",
                            ICmpOp::Slt => "slt",
                            ICmpOp::Sle => "sle",
                            ICmpOp::Sgt => "sgt",
                            ICmpOp::Sge => "sge",
                            ICmpOp::Ult => "ult",
                        };
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = icmp {op_str} i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::FCmp(op, a, b) => {
                        let op_str = match op {
                            FCmpOp::Eq => "oeq",
                            FCmpOp::Ne => "one",
                            FCmpOp::Lt => "olt",
                            FCmpOp::Le => "ole",
                            FCmpOp::Gt => "ogt",
                            FCmpOp::Ge => "oge",
                        };
                        let sa = self.coerce(a, Type::F64);
                        let sb = self.coerce(b, Type::F64);
                        writeln!(self.out, "  %v{v} = fcmp {op_str} double {sa}, {sb}").unwrap();
                    }
                    Instruction::LNot(a) => {
                        let sa = self.coerce(a, Type::I1);
                        writeln!(self.out, "  %v{v} = xor i1 {sa}, true").unwrap();
                    }
                    Instruction::IntToDouble(a) => {
                        let sa = self.coerce(a, Type::I64);
                        writeln!(self.out, "  %v{v} = sitofp i64 {sa} to double").unwrap();
                    }
                    Instruction::DoubleToInt(a) => {
                        let sa = self.coerce(a, Type::F64);
                        writeln!(self.out, "  %v{v} = fptosi double {sa} to i64").unwrap();
                    }
                    Instruction::ZExt { op, from, to } => {
                        let sop = self.coerce(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = zext {f} {sop} to {t}").unwrap();
                    }
                    Instruction::Trunc { op, from, to } => {
                        let sop = self.coerce(op, *from);
                        let f = from.llvm_ir();
                        let t = to.llvm_ir();
                        writeln!(self.out, "  %v{v} = trunc {f} {sop} to {t}").unwrap();
                    }
                    Instruction::AllocObject { class_id, fields } => {
                        let count = fields.len();
                        // Production: the reference map of the first 32 fields in
                        // one word, written by the runtime at the allocation
                        // (`dartforge_object_new_m`); the stores below then skip
                        // the read-modify-write of each bit ([`Self::static_map`]).
                        let map = self.static_maps().then(|| {
                            fields.iter().take(32).enumerate().fold(0u32, |m, (i, f)| {
                                if self.type_of(f) == Type::Ref { m | 1 << i } else { m }
                            })
                        });
                        match map {
                            Some(m) if m != 0 => writeln!(
                                self.out,
                                "  %v{v} = call i64 @dartforge_object_new_m(i64 {class_id}, i64 {count}, i64 {m})"
                            ).unwrap(),
                            _ => writeln!(
                                self.out,
                                "  %v{v} = call i64 @dartforge_object_new(i64 {class_id}, i64 {count})"
                            ).unwrap(),
                        }
                        // The inline fields, through the field table (the object
                        // is new: no other code sees it before this).
                        if !fields.is_empty() {
                            self.emit_field_addresses_of(v, &format!("%v{v}"), !self.module.arc_memory);
                        }
                        for (idx, field) in fields.iter().enumerate() {
                            // E1: `is_ref` by the representation of the value.
                            let is_ref = u8::from(self.type_of(field) == Type::Ref).to_string();
                            let sf = self.coerce(field, Type::I64);
                            self.emit_field_write(v, &format!(".{idx}"), idx, &sf, &is_ref, FieldWriteFacts { known_map: map, fresh: !self.module.arc_memory });
                        }
                        // Under ARC, the fields written inline count.
                        if fields.iter().any(|f| self.type_of(f) == Type::Ref) {
                            self.count_initial_in_arc(v);
                        }
                    }
                    // The inline field: the 8-byte word after the
                    // header of the body of the object (the runtime's `Header`).
                    Instruction::GetField { object, index } => {
                        let only = self.coerce(object, Type::I64);
                        let bits = if matches!(ty, Type::Ref | Type::I64) {
                            format!("%v{v}")
                        } else { format!("%fb{v}") };
                        if *index < INLINE_FIELDS {
                            let direct = self.direct_body(object);
                            self.emit_field_addresses_of(v, &only, direct);
                            writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {index}").unwrap();
                            writeln!(self.out, "  {bits} = load i64, ptr %fg{v}, align 8").unwrap();
                        } else {
                            writeln!(self.out, "  {bits} = call i64 @dartforge_object_get(i64 {only}, i64 {index})").unwrap();
                        }
                        if !matches!(ty, Type::Ref | Type::I64) {
                            self.emit_named_conversion(&format!("%v{v}"), Type::I64, &bits, *ty);
                        }
                    }
                    // Under ARC the store goes to the runtime (`dartforge_object_set`), which counts the swap.
                    Instruction::SetField { object, index, value } if *index < INLINE_FIELDS && !self.module.arc_memory => {
                        let is_ref = u8::from(self.type_of(value) == Type::Ref);
                        let only = self.coerce(object, Type::I64);
                        let sv = self.coerce(value, Type::I64);
                        let fresh = self.fresh(object);
                        let known = self.fresh_map(object);
                        let direct = self.direct_body(object);
                        self.emit_field_addresses_of(v, &only, direct);
                        self.emit_field_write(v, "", *index, &sv, &is_ref.to_string(), FieldWriteFacts { known_map: known, fresh });
                        if self.inline_barrier(inst) {
                            if fresh {
                                self.emit_barrier_label(v);
                            } else {
                                self.emit_barrier(v, &only, &sv, None);
                            }
                        }
                    }
                    Instruction::SetField { object, index, value } => {
                        let is_ref = u8::from(self.type_of(value) == Type::Ref);
                        let only = self.coerce(object, Type::I64);
                        let sv = self.coerce(value, Type::I64);
                        writeln!(
                            self.out,
                            "  call void @dartforge_object_set(i64 {only}, i64 {index}, i64 {sv}, i8 {is_ref})"
                        ).unwrap();
                    }
                    Instruction::CallStatic { symbol, args, ret_ty } => {
                        let target_func = self.function_by_symbol.get(symbol.as_str()).copied();
                        let mut args_str: Vec<String> = Vec::with_capacity(args.len());
                        let mut args_types = Vec::with_capacity(args.len());
                        for (idx, a) in args.iter().enumerate() {
                            // Function of another module (source SDK): the type of the
                            // operand, which the lowering already coerced to the
                            // representation of the parameter.
                            let target = target_func
                                .and_then(|f| f.params.get(idx))
                                .map_or_else(|| self.type_of(a), |p| p.2);
                            let target = if target == Type::Void { Type::I64 } else { target };
                            let s = self.coerce(a, target);
                            args_str.push(format!("{} {s}", target.llvm_ir()));
                            args_types.push(target);
                        }
                        if target_func.is_none() {
                            self.annotate_external(symbol, *ret_ty, &args_types);
                        }
                        let joined = args_str.join(", ");
                        let r = ret_ty.llvm_ir();
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{symbol}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{symbol}({joined})").unwrap();
                        }
                    }
                    // Field with a constant index: inline, like `GetField`.
                    Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
                        if name == "dartforge_object_get"
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let only = self.coerce(&args[0].0, Type::I64);
                        let direct = self.direct_body(&args[0].0);
                        self.emit_field_addresses_of(v, &only, direct);
                        writeln!(self.out, "  %fg{v} = getelementptr inbounds i64, ptr %fp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  %v{v} = load i64, ptr %fg{v}, align 8").unwrap();
                    }
                    Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
                        if name == "dartforge_object_set"
                            && !self.module.arc_memory
                            && args.len() == 4
                            && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
                    {
                        let Some((Operand::Constant(Constant::Int(i)), _)) = args.get(1) else { unreachable!() };
                        let only = self.coerce(&args[0].0, Type::I64);
                        let sv = self.coerce(&args[2].0, Type::I64);
                        // `is_ref` is a Rust `bool`: only 0 or 1.
                        let is_ref = match &args[3].0 {
                            Operand::Constant(Constant::Int(n)) => u8::from(*n != 0).to_string(),
                            Operand::Constant(Constant::Bool(b)) => u8::from(*b).to_string(),
                            other => {
                                let x = self.coerce(other, Type::I64);
                                writeln!(self.out, "  %fb{v} = icmp ne i64 {x}, 0").unwrap();
                                writeln!(self.out, "  %fz{v} = zext i1 %fb{v} to i8").unwrap();
                                format!("%fz{v}")
                            }
                        };
                        let fresh = self.fresh(&args[0].0);
                        let known = self.fresh_map(&args[0].0);
                        let direct = self.direct_body(&args[0].0);
                        self.emit_field_addresses_of(v, &only, direct);
                        self.emit_field_write(v, "", *i as usize, &sv, &is_ref, FieldWriteFacts { known_map: known, fresh });
                        if self.inline_barrier(inst) {
                            if fresh {
                                self.emit_barrier_label(v);
                            } else {
                                let dynamic = is_ref.starts_with('%').then_some(is_ref.as_str());
                                self.emit_barrier(v, &only, &sv, dynamic);
                            }
                        }
                    }
                    Instruction::CallRuntime { .. } | Instruction::DeclaredFieldCall { .. } if Self::inline_allocation(inst).is_some() => {
                        let (c, n) = Self::inline_allocation(inst).expect("checked at the guard");
                        self.emit_inline_allocation(v, c, n);
                    }
                    Instruction::CallRuntime { name, args, ret_ty } | Instruction::DeclaredFieldCall { name, args, ret_ty, .. }
                        if (name == "dartforge_object_new" || name == "dartforge_arc_object_owned_v1")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        // Source SDK: the first allocation registers the method table
                        // of the class (`selectors.rs`).
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let n = self.coerce(&args[1].0, Type::I64);
                        let factory = if name == "dartforge_arc_object_owned_v1" {
                            "dartforge_arc_object_owned_t_v1"
                        } else { "dartforge_object_new_t" };
                        writeln!(self.out, "  %v{v} = call i64 @{factory}(i64 {c}, i64 {n}, ptr @{f})").unwrap();
                        let _ = ret_ty;
                    }
                    Instruction::CallRuntime { name, args, ret_ty } | Instruction::DeclaredFieldCall { name, args, ret_ty, .. }
                        if (name == "dartforge_typed_new" || name == "dartforge_view_new" || name == "dartforge_typed_external")
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        // The first typed list (or view) of a class
                        // registers its method table, like
                        // `dartforge_object_new_t`.
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let rest: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coerce(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{name}_t({}, ptr @{f})", rest.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    // The same through the generic Owned thunk (`optimize::arc::natives`):
                    // the thunk of the `_t` variant, which registers the table.
                    Instruction::CallRuntime { name, args, ret_ty } | Instruction::DeclaredFieldCall { name, args, ret_ty, .. }
                        if name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX).is_some_and(|b|
                            matches!(b, "dartforge_typed_new" | "dartforge_view_new" | "dartforge_typed_external"))
                            && matches!(args.first(), Some((Operand::Constant(Constant::Int(c)), _))
                                if self.module.table_functions.contains_key(&(*c as u32))) =>
                    {
                        let Some((Operand::Constant(Constant::Int(c)), _)) = args.first() else { unreachable!() };
                        let f = self.module.table_functions[&(*c as u32)].clone();
                        self.annotate_external(&f, Type::Ptr, &[]);
                        let base = format!("{}_t", &name[crate::optimize::arc::GENERIC_OWNED_PREFIX.len()..]);
                        let thunk = format!("{}{base}", crate::optimize::arc::GENERIC_OWNED_PREFIX);
                        let mut types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                        types.push(Type::Ptr);
                        self.owned_thunks.insert(thunk.clone(), (base, types));
                        let rest: Vec<String> = args.iter().map(|(a, t)| format!("{} {}", t.llvm_ir(), self.coerce(a, *t))).collect();
                        writeln!(self.out, "  %v{v} = call i64 @{thunk}({}, ptr @{f})", rest.join(", ")).unwrap();
                        let _ = ret_ty;
                    }
                    // The pending exception: the mirror in the thread context.
                    Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_exception_pending" && self.has_ctx => {
                        writeln!(self.out, "  %v{v} = load i8, ptr %ctx, align 8").unwrap();
                    }
                    // The `dartforge_exception_clear` of each `return` and jump:
                    // without a pending exception (the mirror in the context), there is nothing
                    // to clear — the entry of the `catch` and the `finally` already
                    // cleared the stack trace together with the exception. Only a pending one
                    // (the `finally` that leaves through `return`) calls the runtime.
                    Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_exception_clear" && self.has_ctx => {
                        writeln!(self.out, "  %xcp{v} = load i8, ptr %ctx, align 8").unwrap();
                        writeln!(self.out, "  %xcn{v} = icmp ne i8 %xcp{v}, 0").unwrap();
                        writeln!(self.out, "  %xce{v} = call i1 @llvm.expect.i1(i1 %xcn{v}, i1 false)").unwrap();
                        writeln!(self.out, "  br i1 %xce{v}, label %xc{v}.limpar, label %xc{v}.fim").unwrap();
                        writeln!(self.out, "xc{v}.limpar:").unwrap();
                        writeln!(self.out, "  call void @dartforge_exception_clear()").unwrap();
                        if tab.is_some() {
                            // The clear that did not clear is the unwinding of the
                            // isolate (`Isolate.exit`, `kill`), which no
                            // handler holds: in the check model the
                            // function returned with it pending; here it
                            // unwinds (the outer `catch` and `finally`, in the
                            // same function, would not handle it anyway).
                            writeln!(self.out, "  %xcd{v} = load i8, ptr %ctx, align 8").unwrap();
                            writeln!(self.out, "  %xcq{v} = icmp ne i8 %xcd{v}, 0").unwrap();
                            writeln!(self.out, "  br i1 %xcq{v}, label %xc{v}.sai, label %xc{v}.fim").unwrap();
                            writeln!(self.out, "xc{v}.sai:").unwrap();
                            writeln!(self.out, "  call void @df.throw()").unwrap();
                            writeln!(self.out, "  unreachable").unwrap();
                        } else {
                            writeln!(self.out, "  br label %xc{v}.fim").unwrap();
                        }
                        writeln!(self.out, "xc{v}.fim:").unwrap();
                        self.current_label = format!("xc{v}.fim");
                    }
                    // The interrupt request (J01): the byte in the context
                    // (offset 32), read atomically — another thread sets it, and
                    // the load cannot leave the loop.
                    Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_pending_interrupt" && self.has_ctx => {
                        writeln!(self.out, "  %ip{v} = getelementptr inbounds i8, ptr %ctx, i64 32").unwrap();
                        writeln!(self.out, "  %iv{v} = load atomic i8, ptr %ip{v} monotonic, align 8").unwrap();
                        // The request is rare: the slow path stays outside the body
                        // of the loop.
                        writeln!(self.out, "  %v{v} = call i8 @llvm.expect.i8(i8 %iv{v}, i8 0)").unwrap();
                    }
                    // The cache miss helper has more conservative effects than
                    // the source operation. In maps mode retain the direct
                    // operation until root placement models that extra call.
                    Instruction::CallRuntime { args, .. } | Instruction::DeclaredFieldCall { args, .. } if !self.maps && Self::rti_evaluate_cached(inst) => {
                        self.emit_rti_evaluate_cached(v, args);
                    }
                    Instruction::CallRuntime { args, .. } | Instruction::DeclaredFieldCall { args, .. } if Self::rti_test_cached(inst) => {
                        self.emit_rti_test_cached(v, args);
                    }
                    // The class of the receiver: that of the object of the space read from the
                    // header, inline (`df.class`); the rest, the runtime.
                    Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_value_class" && args.len() == 1 => {
                        let h = self.coerce(&args[0].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i64 @df.class(i64 {h})").unwrap();
                    }
                    // `cid <: C` through the bitmap of the target (`df.subclass`),
                    // without a call after the first query.
                    Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } if name == "dartforge_is_subclass" && args.len() == 2 => {
                        let a = self.coerce(&args[0].0, Type::I64);
                        let b = self.coerce(&args[1].0, Type::I64);
                        writeln!(self.out, "  %v{v} = call i8 @df.subclass(i64 {a}, i64 {b})").unwrap();
                    }
                    Instruction::CallRuntime { name, args, ret_ty } | Instruction::DeclaredFieldCall { name, args, ret_ty, .. } => {
                        if name.starts_with("dartforge_native_") {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            self.annotate_external(name, *ret_ty, &types);
                        }
                        // The code of the tear-off may live in another module.
                        if name == "dartforge_arc_tearoff_owned_v1"
                            && let [(Operand::Constant(Constant::Function(code)), _)] = args.as_slice()
                        {
                            self.annotate_external(code, Type::Ref, &[Type::Ref, Type::Ptr, Type::Ptr]);
                        }
                        if let Some(base) = name.strip_prefix(crate::optimize::arc::GENERIC_OWNED_PREFIX) {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            if base.starts_with("dartforge_native_") {
                                self.annotate_external(base, Type::Ref, &types);
                            }
                            self.owned_thunks.insert(name.clone(), (base.to_string(), types));
                        }
                        if let Some(base) = name.strip_prefix(crate::optimize::arc::NATIVE_SCOPE_PREFIX) {
                            let types: Vec<Type> = args.iter().map(|(_, t)| *t).collect();
                            if base.starts_with("dartforge_native_") {
                                self.annotate_external(base, *ret_ty, &types);
                            }
                            self.scope_thunks.insert(name.clone(), (base.to_string(), types, *ret_ty));
                        }
                        let mut args_formatted = Vec::new();
                        for (a, ty) in args {
                            let s = self.coerce(a, *ty);
                            let t = ty.llvm_ir();
                            args_formatted.push(format!("{t} {s}"));
                        }
                        let joined = args_formatted.join(", ");
                        let r = ret_ty.llvm_ir();
                        // `DARTFORGE_EFFECTS=check`: the extern marked as not
                        // collecting or not throwing runs watched by the runtime.
                        let marks = if externs::check_effects() { externs::marks_to_check(name) } else { 0 };
                        if marks != 0 {
                            let k = match self.checked_externs.iter().position(|n| n == name) {
                                Some(k) => k,
                                None => {
                                    self.checked_externs.push(name.clone());
                                    self.checked_externs.len() - 1
                                }
                            };
                            writeln!(self.out, "  call void @dartforge_effects_before(ptr @df.efn.{k}, i64 {}, i64 {marks})", name.len()).unwrap();
                        }
                        if *ret_ty == Type::Void {
                            writeln!(self.out, "  call {r} @{name}({joined})").unwrap();
                        } else {
                            writeln!(self.out, "  %v{v} = call {r} @{name}({joined})").unwrap();
                        }
                        if marks != 0 {
                            writeln!(self.out, "  call void @dartforge_effects_after()").unwrap();
                        }
                    }
                    Instruction::NativeCall { target, args, ret } => {
                        // The C call: each argument converted to the C type
                        // (narrow ones with the ABI extension), the return
                        // extended back explicitly (Windows x64
                        // does not guarantee the high bits of a narrow return).
                        let a = self.coerce(target, Type::I64);
                        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
                        let mut parts = Vec::with_capacity(args.len());
                        for (i, (op, tc)) in args.iter().enumerate() {
                            parts.push(self.c_argument(v, i, op, *tc));
                        }
                        let list = parts.join(", ");
                        let conv = match ret {
                            CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {} %nr{v} to i64", ret.llvm())),
                            CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {} %nr{v} to i64", ret.llvm())),
                            CType::F32 => Some(format!("fpext float %nr{v} to double")),
                            CType::Ptr | CType::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                            CType::I64 | CType::U64 | CType::F64 | CType::Bool | CType::Void => None,
                        };
                        match (ret, conv) {
                            (CType::Void, _) => writeln!(self.out, "  call void %fn{v}({list})").unwrap(),
                            (_, None) => writeln!(self.out, "  %v{v} = call {} %fn{v}({list})", ret.llvm()).unwrap(),
                            (_, Some(c)) => {
                                writeln!(self.out, "  %nr{v} = call {} %fn{v}({list})", ret.llvm()).unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                        }
                    }
                    Instruction::NativeLoad { address, index: idx_value, ty: typ } => {
                        // No assumed alignment (the bytes of a typed
                        // list are from a `Vec<u8>`).
                        let e = self.coerce(address, Type::I64);
                        let i = self.coerce(idx_value, Type::I64);
                        let t = typ.llvm();
                        let inv = "";
                        writeln!(self.out, "  %cp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %cg{v} = getelementptr {t}, ptr %cp{v}, i64 {i}").unwrap();
                        let conv = match typ {
                            CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {t} %cl{v} to i64")),
                            CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {t} %cl{v} to i64")),
                            CType::F32 => Some(format!("fpext float %cl{v} to double")),
                            _ => None,
                        };
                        match conv {
                            Some(c) => {
                                writeln!(self.out, "  %cl{v} = load {t}, ptr %cg{v}, align 1").unwrap();
                                writeln!(self.out, "  %v{v} = {c}").unwrap();
                            }
                            None => writeln!(self.out, "  %v{v} = load {t}, ptr %cg{v}, align 1{inv}").unwrap(),
                        }
                    }
                    Instruction::NativeWrite { address, index: idx_value, ty: typ, value: val_ref } => {
                        let e = self.coerce(address, Type::I64);
                        let i = self.coerce(idx_value, Type::I64);
                        let t = typ.llvm();
                        let x = match typ {
                            CType::F32 => {
                                let d = self.coerce(val_ref, Type::F64);
                                writeln!(self.out, "  %gx{v} = fptrunc double {d} to float").unwrap();
                                format!("%gx{v}")
                            }
                            CType::F64 => self.coerce(val_ref, Type::F64),
                            CType::I64 | CType::U64 => self.coerce(val_ref, Type::I64),
                            _ => {
                                let n = self.coerce(val_ref, Type::I64);
                                writeln!(self.out, "  %gx{v} = trunc i64 {n} to {t}").unwrap();
                                format!("%gx{v}")
                            }
                        };
                        writeln!(self.out, "  %gp{v} = inttoptr i64 {e} to ptr").unwrap();
                        writeln!(self.out, "  %gg{v} = getelementptr {t}, ptr %gp{v}, i64 {i}").unwrap();
                        writeln!(self.out, "  store {t} {x}, ptr %gg{v}, align 1").unwrap();
                    }
                    Instruction::CompositeNativeCall { target, args, ret, dest, variadic_flag } => {
                        self.composite_native_call(v, target, args, ret, dest.as_ref(), *variadic_flag);
                    }
                    Instruction::AllocList { elements } => self.emit_alloc_list(v, elements),
                    Instruction::AllocRecord { elements } => self.emit_alloc_record(v, elements),
                    // The `alloca` is born in the entry block (`emit_closure_buffers`).
                    Instruction::Alloca(_) => {}
                    Instruction::Load { ptr, ty } => {
                        let sp = self.operand_str(ptr);
                        writeln!(self.out, "  %v{v} = load {}, ptr {sp}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::Store { ptr, val } => {
                        // The local keeps the representation of its type (R6); the
                        // value arrives already coerced by the lowering, and here only
                        // the width is adjusted. A pointer that is not an `alloca`
                        // of this function (the address of a local of a caller of
                        // a direct local function, `direct_functions.rs`)
                        // writes in the width of the value, that of the local over there: a
                        // `bool` written as an `i64` trampled 7 bytes of the frame
                        // of the caller (the `listenerHasError` of
                        // `_Future._propagateToListeners`).
                        let t = match ptr {
                            Operand::Val(p) => self.pointee.get(p).copied().unwrap_or_else(|| match self.type_of(val) {
                                Type::I1 => Type::I1,
                                Type::I8 => Type::I8,
                                Type::F64 => Type::F64,
                                // Unboxed SIMD local of the caller (16 bytes).
                                t if t.is_vector() => t,
                                _ => Type::I64,
                            }),
                            _ => Type::I64,
                        };
                        let sp = self.operand_str(ptr);
                        let sv = self.coerce(val, t);
                        writeln!(self.out, "  store {} {sv}, ptr {sp}", t.llvm_ir()).unwrap();
                        // G2: the `Ref` local has its own slot, updated on
                        // each write — it lives longer than the SSA value that wrote it.
                        if let Operand::Val(pv) = ptr
                            && !self.frame_allocas.contains(pv)
                            && let Some(&slot) = self.slots.get(pv)
                        {
                            writeln!(self.out, "  store i64 {sv}, ptr %gcs{slot}").unwrap();
                        }
                    }
                    Instruction::Box { op, from } if from.is_vector() => self.emit_simd_box(v, op, *from),
                    Instruction::Unbox { op, to } if to.is_vector() => self.emit_simd_unbox(v, op, *to),
                    Instruction::Simd { op, args } => {
                        let ty = self.types.get(vid).copied().unwrap_or(Type::Void);
                        self.emit_simd(v, *op, args, ty);
                    }
                    Instruction::Box { op, from } => self.emit_box(v, op, *from),
                    Instruction::Unbox { op, to } => self.emit_unbox(v, op, *to),
                    Instruction::LShr(a, b) => {
                        let sa = self.coerce(a, Type::I64);
                        let sb = self.coerce(b, Type::I64);
                        writeln!(self.out, "  %v{v} = lshr i64 {sa}, {sb}").unwrap();
                    }
                    Instruction::Bitcast { op, to } => {
                        if *to == Type::F64 {
                            let only = self.coerce(op, Type::I64);
                            writeln!(self.out, "  %v{v} = bitcast i64 {only} to double").unwrap();
                        } else {
                            let only = self.coerce(op, Type::F64);
                            writeln!(self.out, "  %v{v} = bitcast double {only} to i64").unwrap();
                        }
                    }
                    Instruction::LoadGlobal { symbol: sym_name, ty } => {
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  %v{v} = load {}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                    }
                    Instruction::StoreGlobal { symbol: sym_name, val, ty, root } => {
                        let sv = self.coerce(val, *ty);
                        self.global_address(v, sym_name);
                        writeln!(self.out, "  store {} {sv}, ptr %ga{v}", ty.llvm_ir()).unwrap();
                        if root.is_some() {
                            // The root is identified by the address of the slot:
                            // unique across modules and isolates.
                            writeln!(self.out, "  %gr{v} = ptrtoint ptr %ga{v} to i64").unwrap();
                            writeln!(self.out, "  call void @dartforge_gc_global_root(i64 %gr{v}, i64 {sv})").unwrap();
                        }
                    }
                    Instruction::Phi { incoming, ty } => {
                        let t = ty.llvm_ir();
                        // The coercion of a `phi` entry can NOT be emitted
                        // here: `phi` must be the first instruction of the block and the
                        // conversion belongs to the source block. So only constants
                        // are rewritten in the type of the `phi`; values that arrive with
                        // a different width are fixed at the source, when finishing
                        // that block.
                        let in_strs: Vec<String> = incoming
                            .iter()
                            .map(|(b, op)| {
                                let sop = self
                                    .constant_in_type(op, *ty)
                                    .or_else(|| match op {
                                        Operand::Val(v) => self
                                            .conv_phi
                                            .iter()
                                            .find(|(bb, _, _, vv, to_repr)| {
                                                *bb == b.0 && vv == v && to_repr.llvm_ir() == ty.llvm_ir()
                                            })
                                            .map(|(_, name_str, _, _, _)| name_str.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| self.operand_str(op));
                                match self.exit_labels.get(&b.0) {
                                    Some(r) => format!("[ {sop}, %{r} ]"),
                                    None => format!("[ {sop}, %b{} ]", b.0),
                                }
                            })
                            .collect();
                        let joined = in_strs.join(", ");
                        writeln!(self.out, "  %v{v} = phi {t} {joined}").unwrap();
                    }
                    _ if self.emit_closure_inst(v, inst, *ty) => {}
                    _ => {
                        // E3: the HIR verifier rejects, before the emission,
                        // every instruction without a lowering here (the old
                        // "; inst pendente" that Clang silently accepted).
                        unreachable!("instruction without emission passed the verifier: {inst:?}");
                    }
                }
                // Exceptions by tables: the Dart call with a landing pad is an `invoke`
                // (the root of the result, below, already comes out on the normal path).
                if let Some(landing_pad) = tab.and_then(|t| t.invocations.get(vid)) {
                    self.make_invoke(instruction_start, v, landing_pad.0);
                }
                // G1: the root right after the definition — nothing allocates between
                // the return of the call and this root store (G5).
                if let Some(&slot) = self.slots.get(vid) {
                    if matches!(inst, Instruction::Phi { .. }) {
                        phi_roots.push((slot, v));
                    } else if !matches!(inst, Instruction::Alloca(_)) {
                        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
                    }
                }
                // Roots by maps: what was live at the entry of this collection
                // point (the operands included) goes into the `"deopt"` of each of its
                // calls that may collect — that is what puts it in the map of the
                // call (§14.12).
                if !self.rooted_set.is_empty()
                    && let Some(live_set) = self.function_roots.live_at.get(vid).cloned()
                {
                    self.attach_deopt(instruction_start, &live_set);
                }
                if !self.module.arc_memory
                    && (matches!(inst, Instruction::AllocObject { .. }) || Self::inline_allocation(inst).is_some()
                        || matches!(inst, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_object_new"))
                {
                    self.fresh_set.insert(v);
                }
                let _ = ty;
            }
            for (slot, pv) in std::mem::take(&mut phi_roots) {
                self.phi_root(slot, pv);
            }

            // The checking build: the collection point at the end of the block.
            if self.check.is_some() {
                let live_set = self.function_roots.live_at_end.get(&block.id).cloned().unwrap_or_default();
                self.store_check(&live_set);
            }
            if let Some(dead) = self.function_roots.clear_at_end.get(&block.id) {
                self.clear_dead_slots(&dead.clone());
            }
            let end_start = self.out.len();
            for (b, name_str, of, v, to_repr) in self.conv_phi.clone() {
                if b == block.id.0 {
                    let origin = format!("%v{}", v.0);
                    self.emit_named_conversion(&name_str, of, &origin, to_repr);
                }
            }
            // Roots by maps: the collection point at the end of the block (the
            // conversions above may box; the `throw` is handled in the
            // terminator, after its call).
            if !self.rooted_set.is_empty()
                && !matches!(block.terminator, Terminator::Throw(_))
                && let Some(live_set) = self.function_roots.live_at_end.get(&block.id).cloned()
            {
                self.attach_deopt(end_start, &live_set);
            }

            if let Some(p) = func.debug_info.as_ref().and_then(|d| d.exits.get(&block.id))
                && written_position != Some(*p)
            {
                writeln!(self.out, "{}{} {}", debug_info::MARKER, p.0, p.1).unwrap();
                written_position = Some(*p);
                written_embedded = None;
            }
            // Exceptions by tables: the `Return` with the exception pending does not
            // return — it unwinds up to the landing pad of whoever handles it (which restores
            // the top of the shadow stack: the roots frame is not closed
            // here). With an uncertain pending flag, it checks first.
            if matches!(block.terminator, Terminator::Return(_)) {
                match tab.and_then(|t| t.exits.get(&block.id)) {
                    Some(ExceptionExit::Throws) => {
                        writeln!(self.out, "  call void @df.throw()").unwrap();
                        writeln!(self.out, "  unreachable").unwrap();
                        continue;
                    }
                    Some(ExceptionExit::Resume) => {
                        if self.stack_trace.is_some() {
                            writeln!(self.out, "  call void @dartforge_call_trace_exit()").unwrap();
                        }
                        if self.has_frame {
                            writeln!(self.out, "  store ptr %gcant, ptr %ctxtopo, align 8").unwrap();
                        }
                        writeln!(self.out, "  resume {{ ptr, i32 }} %lpad{}", block.id.0).unwrap();
                        continue;
                    }
                    Some(ExceptionExit::Guard) => {
                        let b = block.id.0;
                        writeln!(self.out, "  %xgp{b} = load i8, ptr %ctx, align 8").unwrap();
                        writeln!(self.out, "  %xgn{b} = icmp ne i8 %xgp{b}, 0").unwrap();
                        writeln!(self.out, "  %xge{b} = call i1 @llvm.expect.i1(i1 %xgn{b}, i1 false)").unwrap();
                        writeln!(self.out, "  br i1 %xge{b}, label %xg{b}.lanca, label %xg{b}.ret").unwrap();
                        writeln!(self.out, "xg{b}.lanca:").unwrap();
                        writeln!(self.out, "  call void @df.throw()").unwrap();
                        writeln!(self.out, "  unreachable").unwrap();
                        writeln!(self.out, "xg{b}.ret:").unwrap();
                    }
                    None => {}
                }
            }
            if self.stack_trace.is_some() && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  call void @dartforge_call_trace_exit()").unwrap();
            }
            // G3: the roots frame closes before EVERY `ret`, including that
            // of the exception exits (which return the default value).
            if self.has_frame && matches!(block.terminator, Terminator::Return(_)) {
                writeln!(self.out, "  %gcvolta{} = load ptr, ptr %gcq, align 8", block.id.0).unwrap();
                writeln!(self.out, "  store ptr %gcvolta{}, ptr %ctxtopo, align 8", block.id.0).unwrap();
            }
            match &block.terminator {
                Terminator::Return(Some(op)) => {
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        let sop = self.coerce(op, func.return_ty);
                        writeln!(self.out, "  ret {} {sop}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Return(None) => {
                    let zero = match func.return_ty {
                        Type::F64 => "0.0",
                        Type::I1 => "false",
                        _ => "0",
                    };
                    if func.return_ty == Type::Void {
                        writeln!(self.out, "  ret void").unwrap();
                    } else {
                        writeln!(self.out, "  ret {} {zero}", func.return_ty.llvm_ir()).unwrap();
                    }
                }
                Terminator::Branch(target) => {
                    writeln!(self.out, "  br label %b{}", target.0).unwrap();
                }
                // The branch to the landing pad is that of the `invoke` of this block
                // (`optimize/tables.rs`): here only the continuation follows.
                Terminator::CondBranch { then_block, else_block, .. } if tab.is_some_and(|t| t.landing_pads.contains(then_block)) => {
                    writeln!(self.out, "  br label %b{}", else_block.0).unwrap();
                }
                Terminator::CondBranch { cond, then_block, else_block } => {
                    let sc = self.coerce(cond, Type::I1);
                    writeln!(self.out, "  br i1 {sc}, label %b{}, label %b{}", then_block.0, else_block.0).unwrap();
                }
                Terminator::Switch { val, default, cases } => {
                    let sv = self.coerce(val, Type::I64);
                    write!(self.out, "  switch i64 {sv}, label %b{} [", default.0).unwrap();
                    for (c, b) in cases {
                        write!(self.out, " i64 {c}, label %b{}", b.0).unwrap();
                    }
                    writeln!(self.out, " ]").unwrap();
                }
                Terminator::Throw(op) => {
                    let tag = match self.type_of(op) {
                        Type::I64 => 1,
                        Type::I1 | Type::I8 => 2,
                        Type::F64 => 4,
                        _ => 3,
                    };
                    let sop = self.coerce(op, Type::I64);
                    let throw_start = self.out.len();
                    writeln!(self.out, "  call void @dartforge_exception_throw(i64 {sop}, i8 {tag})").unwrap();
                    if !self.rooted_set.is_empty()
                        && let Some(live_set) = self.function_roots.live_at_end.get(&block.id).cloned()
                    {
                        self.attach_deopt(throw_start, &live_set);
                    }
                    if tab.is_some() {
                        writeln!(self.out, "  call void @df.throw()").unwrap();
                    }
                    writeln!(self.out, "  unreachable").unwrap();
                }
                Terminator::Unreachable => {
                    writeln!(self.out, "  unreachable").unwrap();
                }
            }
        }

        writeln!(self.out, "}}\n").unwrap();
        // §13.14: the stack trace labels, before the DWARF (which consumes the
        // position markers).
        if (func.debug_info.is_some() || shared)
            && let Some(mut r) = self.vm_stack_trace.take()
        {
            let text = self.out.split_off(function_start);
            let labeled = r.label(&text, func, func.debug_info.as_deref(), self.debug_info.is_some(), shared);
            self.out.push_str(&labeled);
            self.vm_stack_trace = Some(r);
        }
        if let Some(d) = func.debug_info.as_deref()
            && let Some(mut dep) = self.debug_info.take()
        {
            let text = self.out.split_off(function_start);
            let annotated = dep.annotate(&text, func, d);
            self.out.push_str(&annotated);
            self.debug_info = Some(dep);
        }
    }

    /// The number of slots of the roots frame of the function being emitted: the usual
    /// ones and, in the checking build, the check ones after them.
    fn frame_size(&self) -> usize {
        match self.check {
            Some((first_one, k)) => first_one + k,
            None => self.slots.values().max().map_or(0, |m| m + 1),
        }
    }

    /// The `n` field of the frame (the runtime's `RootFrame`): the number of
    /// slots and, from bit 40 on, how many of the last ones are check
    /// slots.
    fn frame_header(&self, n: usize) -> u64 {
        // Bit 62: the frame is in its own native frame
        // ([`Self::ordered_frames`]).
        let order = if self.ordered_frames { 1u64 << 62 } else { 0 };
        match self.check {
            Some((_, k)) => n as u64 | ((k as u64) << 40) | order,
            None => n as u64 | order,
        }
    }

    /// The checking build: writes `live_set` into the check slots and zeroes
    /// the remaining ones.
    fn store_check(&mut self, live_set: &[ValueId]) {
        let Some((first_one, k)) = self.check else { return };
        for i in 0..k {
            match live_set.get(i) {
                Some(x) => writeln!(self.out, "  store i64 %v{}, ptr %gcs{}", x.0, first_one + i).unwrap(),
                None => writeln!(self.out, "  store i64 0, ptr %gcs{}", first_one + i).unwrap(),
            }
        }
    }

    /// Zeroes the shadow-stack slots `dead` (values that died since they were
    /// written, [`roots::Roots::clear_at`]). Not with roots by maps (the SSA
    /// values are not in the frame) nor under ARC (whose slots own their values).
    fn clear_dead_slots(&mut self, dead: &[usize]) {
        if !self.rooted_set.is_empty() || self.module.arc_memory {
            return;
        }
        for &slot in dead {
            writeln!(self.out, "  store i64 0, ptr %gcs{slot}").unwrap();
        }
    }

    /// The root of a `phi`, emitted after the last `phi` of the block: the `store`
    /// into the slot of the frame.
    fn phi_root(&mut self, slot: usize, v: u32) {
        writeln!(self.out, "  store i64 %v{v}, ptr %gcs{slot}").unwrap();
    }

    /// Roots by maps (§14.12): the rooted values `live_set` in the `"deopt"`
    /// operand of each call that may collect emitted since `start` (the
    /// text of a collection point). The operand makes the value live *at* the
    /// call and puts its place in the map; it also holds for the argument whose
    /// last use is the call itself (the runtime counts on the caller to
    /// keep it alive while it runs: LLVM keeps it in a preserved
    /// register or in a slot).
    fn attach_deopt(&mut self, start: usize, live_set: &[ValueId]) {
        self.attach_deopt_with(start, live_set, true);
    }

    /// [`Self::attach_deopt`]; `sabotageable` says whether the value of the
    /// `raw_in_map` sabotage (defined after the prologue) already exists here.
    fn attach_deopt_with(&mut self, start: usize, live_set: &[ValueId], sabotageable: bool) {
        let mut operands: Vec<String> = if crate::target::sabotage("no_fake_use") {
            Vec::new()
        } else {
            live_set.iter().map(|x| format!("i64 %v{}", x.0)).collect()
        };
        if sabotageable && crate::target::sabotage("raw_in_map") {
            operands.push("i64 %dfsabv".to_string());
        }
        let beam = format!("\"deopt\"({})", operands.join(", "));
        let text = self.out.split_off(start);
        for line in text.split_inclusive('\n') {
            self.out.push_str(&with_deopt(line, &beam));
        }
    }

    /// Exceptions by tables: the Dart call that instruction `v` has just
    /// emitted (the last line since `start`: that of `CallStatic`,
    /// `TypedCall`, of the closure or of the selector) becomes
    /// `invoke … to label %inv<v>.fim unwind label %b<landing_pad>`, and the emission
    /// continues at the label of the normal path — the HIR block now
    /// ends there ([`LlvmEmitter::exit_labels`]). The calls to the
    /// runtime that the same instruction emits before (the entry of the closure, the
    /// lookup of the selector) do not unwind and stay `call`.
    fn make_invoke(&mut self, start: usize, v: u32, landing_pad: u32) {
        let text = self.out.split_off(start);
        let body = text.strip_suffix('\n').unwrap_or(&text);
        let (before, call) = match body.rfind('\n') {
            Some(i) => (&body[..=i], &body[i + 1..]),
            None => ("", body),
        };
        assert!(
            call.starts_with("  call ") || call.contains(" = call "),
            "compiler bug: the instruction with a landing pad does not end in a call: {call}"
        );
        self.out.push_str(before);
        self.out.push_str(&call.replacen("call ", "invoke ", 1));
        writeln!(self.out, " to label %inv{v}.fim unwind label %b{landing_pad}").unwrap();
        writeln!(self.out, "inv{v}.fim:").unwrap();
        self.current_label = format!("inv{v}.fim");
    }

    /// The ARC uniform invokers (closures, selectors, typed calls):
    /// in bodies prepared by the ARC pass, only with the AOT ports
    /// certificate. Without preparation, the ARC image follows the legacy regime of the CLI,
    /// with the direct call of the entry.
    ///
    /// # Panics
    /// Prepared ARC bodies without the ports certificate.
    pub(super) fn arc_uniform_abi(&self) -> bool {
        if !self.module.arc_memory || !(self.module.arc_prepared || self.module.arc_aot_ports.is_some()) {
            return false;
        }
        assert!(self.module.arc_aot_ports.is_some(), "prepared ARC invocation requires certified AOT ports");
        true
    }

    /// The factories with a code owner (list literal, interpolation): only
    /// when the ARC preparation inserted the release of that owner. In the legacy
    /// regime nobody would release it, and the object would never be freed.
    pub(super) fn arc_owned_factories(&self) -> bool {
        self.module.arc_memory && self.module.arc_prepared
    }

    /// The call of a function value reads the entry inline
    /// ([`Self::closure_entry`]): the HIR block ends at `ce{v}.fim`. Under ARC
    /// the entry stays with the runtime (`dartforge_closure_entry`, or the
    /// uniform ARC invocation).
    fn inline_closure_entry(&self, inst: &Instruction) -> bool {
        !self.module.arc_memory && matches!(inst, Instruction::CallClosure { .. } | Instruction::CallClosureForward { .. })
    }

    /// `%cf<v>`: the pointer of the uniform entry of the closure `c` — its
    /// code, or `@df_clo_invalido` when the value is not a closure (the runtime
    /// returns 0 and leaves the `NoSuchMethodError` pending). An object of
    /// class `_Closure` (`layout::cid::CLOSURE`) has the code in its first
    /// field (`layout::offsets::CLOSURE_CODE`), read inline — the same read
    /// `dartforge_closure_entry` does (`Heap::closure`), without the call and
    /// the heap borrow; anything else (null, a `Smi`, a callable class with
    /// `call`, a value that is not a function) goes to the runtime, as before.
    /// The block ends at `ce{v}.fim` ([`LlvmEmitter::exit_labels`]).
    fn closure_entry(&mut self, v: u32, c: &str) {
        if self.module.arc_memory {
            writeln!(self.out, "  %cc{v} = call i64 @dartforge_closure_entry(i64 {c})").unwrap();
        } else {
            let o = &mut self.out;
            writeln!(o, "  %cek{v} = and i64 {c}, {OBJECT_MASK}").unwrap();
            writeln!(o, "  %ceo{v} = icmp eq i64 %cek{v}, 2").unwrap();
            writeln!(o, "  br i1 %ceo{v}, label %ce{v}.obj, label %ce{v}.lento").unwrap();
            writeln!(o, "ce{v}.obj:").unwrap();
            writeln!(o, "  %ceca{v} = add i64 {c}, {}", layout::offsets::CLASS as i64 - layout::HANDLE_OFFSET).unwrap();
            writeln!(o, "  %cecp{v} = inttoptr i64 %ceca{v} to ptr").unwrap();
            writeln!(o, "  %cec{v} = load i32, ptr %cecp{v}, align 4").unwrap();
            writeln!(o, "  %ceq{v} = icmp eq i32 %cec{v}, {}", layout::cid::CLOSURE).unwrap();
            writeln!(o, "  br i1 %ceq{v}, label %ce{v}.rapido, label %ce{v}.lento").unwrap();
            writeln!(o, "ce{v}.rapido:").unwrap();
            writeln!(o, "  %cefa{v} = add i64 {c}, {}", layout::offsets::CLOSURE_CODE as i64 - layout::HANDLE_OFFSET).unwrap();
            writeln!(o, "  %cefp{v} = inttoptr i64 %cefa{v} to ptr").unwrap();
            writeln!(o, "  %cer{v} = load i64, ptr %cefp{v}, align 8").unwrap();
            writeln!(o, "  br label %ce{v}.fim").unwrap();
            writeln!(o, "ce{v}.lento:").unwrap();
            writeln!(o, "  %cel{v} = call i64 @dartforge_closure_entry(i64 {c})").unwrap();
            writeln!(o, "  br label %ce{v}.fim").unwrap();
            writeln!(o, "ce{v}.fim:").unwrap();
            writeln!(o, "  %cc{v} = phi i64 [ %cer{v}, %ce{v}.rapido ], [ %cel{v}, %ce{v}.lento ]").unwrap();
            self.current_label = format!("ce{v}.fim");
        }
        writeln!(self.out, "  %cz{v} = icmp eq i64 %cc{v}, 0").unwrap();
        writeln!(
            self.out,
            "  %cq{v} = select i1 %cz{v}, i64 ptrtoint (ptr @df_clo_invalido to i64), i64 %cc{v}"
        )
        .unwrap();
        writeln!(self.out, "  %cf{v} = inttoptr i64 %cq{v} to ptr").unwrap();
    }

    /// The descriptor of a call through the uniform convention:
    /// [n_positional, n_named, hash(name)…].
    fn descriptor(args: usize, names: &[String]) -> Vec<i64> {
        let mut d = vec![(args - names.len()) as i64, names.len() as i64];
        d.extend(names.iter().map(|n| crate::lower::closures::hash_name(n)));
        d
    }

    fn register_vector(&mut self, v: Vec<i64>) -> usize {
        if let Some(&k) = self.vector_of.get(&v) {
            return k;
        }
        let k = self.vectors.len();
        self.vector_symbols.push(if self.pool_numeric_vectors() {
            numeric_vectors::symbol(&v)
        } else {
            format!("df.arr.{k}")
        });
        self.vector_of.insert(v.clone(), k);
        self.vectors.push(v);
        k
    }

    fn pool_numeric_vectors(&self) -> bool {
        self.numeric_vector_pooling && self.body_without_outside && self.module.arc_fixed_ancestry
            && self.previous_area.is_none() && !self.maps_in_jit && !self.stack_trace_in_jit
    }

    /// `@df_clo_invalido` (the entry that the call uses when the value is not a
    /// closure — the runtime has already left the `NoSuchMethodError` pending) and the
    /// constant vectors.
    fn emit_closures(&mut self) {
        let module = self.module;
        for func in &module.functions {
            for block in &func.blocks {
                for (_, inst, _) in &block.instructions {
                    match inst {
                        Instruction::ConstArray(v) => {
                            self.register_vector(v.clone());
                        }
                        Instruction::FunctionTable(v) => {
                            if !self.function_table_of.contains_key(v) {
                                self.function_table_of.insert(v.clone(), self.function_tables.len());
                                self.function_tables.push(v.clone());
                            }
                            for s in v {
                                if !self.function_by_symbol.contains_key(s.as_str()) {
                                    self.annotate_external(s, Type::Ref, &[]);
                                }
                            }
                        }
                        Instruction::CallClosure { args, names, .. } | Instruction::CallSelector { args, names, .. } => {
                            self.register_vector(Self::descriptor(args.len(), names));
                            self.argument_names.extend(names.iter().cloned());
                        }
                        _ => {}
                    }
                }
            }
        }
        if self.module.sdk_mode {
            // The descriptor without arguments of `dartforge_dispatch_toString`.
            self.register_vector(vec![0, 0]);
        }
        self.out.push_str("; Closures: invalid entry and constant vectors\n");
        self.out.push_str("define internal i64 @df_clo_invalido(i64 %c, ptr %a, ptr %d) {\nb0:\n  ret i64 0\n}\n");
        for (k, v) in self.vectors.iter().enumerate() {
            let items: Vec<String> = v.iter().map(|x| format!("i64 {x}")).collect();
            let symbol = &self.vector_symbols[k];
            let shared = self.pool_numeric_vectors();
            let linkage = if shared { "linkonce_odr hidden" } else { "private" };
            let comdat = if shared && crate::target::has_comdat() {
                self.comdats.push(symbol.clone());
                format!(", comdat($\"{symbol}\")")
            } else { String::new() };
            writeln!(
                self.out,
                "@{symbol} = {linkage} unnamed_addr constant [{} x i64] [{}]{comdat}, align 8",
                v.len(),
                items.join(", ")
            )
            .unwrap();
        }
        for (k, v) in self.function_tables.iter().enumerate() {
            let items: Vec<String> = v.iter().map(|s| format!("ptr @{s}")).collect();
            writeln!(self.out, "@df.fns.{k} = private unnamed_addr constant [{} x ptr] [{}]", v.len(), items.join(", ")).unwrap();
        }
        self.out.push('\n');
    }

    /// Converts the i64 bits read from a cell/environment (%u) to the
    /// representation `ty` of the result `%v<v>`.
    fn bits_to_repr(&mut self, v: u32, bits: &str, ty: Type) {
        match ty {
            Type::F64 => writeln!(self.out, "  %v{v} = bitcast i64 {bits} to double").unwrap(),
            Type::I1 => writeln!(self.out, "  %v{v} = icmp ne i64 {bits}, 0").unwrap(),
            Type::I8 => writeln!(self.out, "  %v{v} = trunc i64 {bits} to i8").unwrap(),
            _ => writeln!(self.out, "  %v{v} = add i64 {bits}, 0").unwrap(),
        }
    }

    /// ABI tag (bits, tag) of a value by its representation (E1).
    fn tag_of(&self, op: &Operand) -> u8 {
        match self.type_of(op) {
            Type::I64 => 1,
            Type::I1 | Type::I8 => 2,
            Type::F64 => 4,
            _ => 3,
        }
    }

    /// Emits a closure instruction (P1); `false` if it is not one of them.
    fn emit_closure_inst(&mut self, v: u32, inst: &Instruction, ty: Type) -> bool {
        match inst {
            Instruction::AllocCell { value } => self.emit_alloc_cell(v, value),
            Instruction::CellGet { cell } => self.emit_cell_get(v, cell, ty),
            Instruction::CellSet { cell, value } => self.emit_cell_set(cell, value),
            Instruction::EnvGet { env, index } => self.emit_env_get(v, env, *index, ty),
            Instruction::ConcatStrings { parts } => self.emit_concat_strings(v, parts),
            Instruction::AllocEnv { values } => self.emit_alloc_env(v, values),
            Instruction::AllocClosure { code_symbol, env } => self.emit_alloc_closure(v, code_symbol, env),
            Instruction::AllocClosureTyped { code_symbol, env, typed, abi, direct } => {
                self.emit_alloc_typed_closure(v, code_symbol, env, typed, *abi, *direct);
            }
            Instruction::TypedCall { target, args, ret } => {
                // The call is the same in both regimes; the predicate only checks
                // the certificate when the bodies were prepared.
                self.arc_uniform_abi();
                let a = self.coerce(target, Type::I64);
                let mut parts = Vec::with_capacity(args.len());
                for (x, t) in args {
                    let s = self.coerce(x, *t);
                    parts.push(format!("{} {s}", t.llvm_ir()));
                }
                writeln!(self.out, "  %ct{v} = inttoptr i64 {a} to ptr").unwrap();
                if *ret == Type::Void {
                    writeln!(self.out, "  call void %ct{v}({})", parts.join(", ")).unwrap();
                } else {
                    writeln!(self.out, "  %v{v} = call {} %ct{v}({})", ret.llvm_ir(), parts.join(", ")).unwrap();
                }
            }
            Instruction::TearOff { code_symbol } => self.emit_tearoff(v, code_symbol),
            Instruction::CallClosure { closure, args, names, tuple_types, .. } => {
                let k = self.vector_of[&Self::descriptor(args.len(), names)];
                // The slot after the arguments carries the type tuple (the
                // same convention as the call by selector).
                let n = args.len() + 1;
                for (i, a) in args.iter().enumerate() {
                    let s = self.coerce(a, Type::I64);
                    writeln!(self.out, "  %ca{v}_{i} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {i}").unwrap();
                    writeln!(self.out, "  store i64 {s}, ptr %ca{v}_{i}").unwrap();
                }
                let tuple = self.coerce(tuple_types, Type::I64);
                writeln!(self.out, "  %cat{v} = getelementptr [{n} x i64], ptr %cargs{v}, i64 0, i64 {}", args.len()).unwrap();
                writeln!(self.out, "  store i64 {tuple}, ptr %cat{v}").unwrap();
                let c = self.coerce(closure, Type::Ref);
                let descriptor = &self.vector_symbols[k];
                if self.arc_uniform_abi() {
                    writeln!(self.out, "  %cav{v} = ptrtoint ptr %cargs{v} to i64").unwrap();
                    writeln!(self.out, "  %cdv{v} = ptrtoint ptr @{descriptor} to i64").unwrap();
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invoke_uniform_closure_v1(i64 {c}, i64 %cav{v}, i64 %cdv{v})").unwrap();
                } else {
                    self.closure_entry(v, &c);
                    writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr %cargs{v}, ptr @{})", self.vector_symbols[k]).unwrap();
                }
            }
            Instruction::CallClosureForward { arc_owned, closure, args, desc } => {
                let c = self.coerce(closure, Type::Ref);
                let a = self.operand_str(args);
                let d = self.operand_str(desc);
                if *arc_owned {
                    assert!(self.module.arc_memory && self.module.arc_aot_ports.is_some(),
                        "ARC closure forwarding requires certified AOT ports");
                    writeln!(self.out, "  %cav{v} = ptrtoint ptr {a} to i64").unwrap();
                    writeln!(self.out, "  %cdv{v} = ptrtoint ptr {d} to i64").unwrap();
                    writeln!(self.out, "  %v{v} = call i64 @dartforge_arc_invoke_uniform_closure_v1(i64 {c}, i64 %cav{v}, i64 %cdv{v})").unwrap();
                } else {
                    self.closure_entry(v, &c);
                    writeln!(self.out, "  %v{v} = call i64 %cf{v}(i64 {c}, ptr {a}, ptr {d})").unwrap();
                }
            }
            Instruction::CallSelector { selector, recv, args, names, tuple_types } => {
                self.emit_selector_call(v, selector, recv, args, names, tuple_types);
            }
            Instruction::CallSelectorForward { arc_owned, selector, recv, args, desc } => {
                self.emit_selector_forward(v, selector, recv, args, desc, *arc_owned);
            }
            Instruction::LoadIndexed { base, index } => {
                let b = self.operand_str(base);
                let i = self.coerce(index, Type::I64);
                writeln!(self.out, "  %li{v} = getelementptr i64, ptr {b}, i64 {i}").unwrap();
                writeln!(self.out, "  %v{v} = load i64, ptr %li{v}").unwrap();
            }
            Instruction::ConstArray(vals) => {
                let k = self.vector_of[vals];
                writeln!(self.out, "  %v{v} = getelementptr i64, ptr @{}, i64 0", self.vector_symbols[k]).unwrap();
            }
            Instruction::FunctionTable(fns) => {
                let k = self.function_table_of[fns];
                writeln!(self.out, "  %v{v} = getelementptr ptr, ptr @df.fns.{k}, i64 0").unwrap();
            }
            _ => return false,
        }
        true
    }

    /// Every `alloca` of the function is born in the entry block — the vectors of the
    /// closures (arguments, pairs of the environment), those of the list,
    /// map and record literals, and the locals: an `alloca` outside the entry reserves new
    /// stack on each execution, and in a loop the stack grows until it overflows.
    fn emit_closure_buffers(&mut self, func: &Function) {
        for block in &func.blocks {
            for (vid, inst, _) in &block.instructions {
                match inst {
                    Instruction::Alloca(_) if self.frame_allocas.contains(vid) => {}
                    Instruction::Alloca(ty) => {
                        writeln!(self.out, "  %v{} = alloca {}", vid.0, ty.llvm_ir()).unwrap();
                    }
                    Instruction::AllocList { elements } => self.list_buffer(vid.0, elements),
                    Instruction::AllocRecord { elements } => self.record_buffer(vid.0, elements),
                    Instruction::CallClosure { args, .. } => {
                        writeln!(self.out, "  %cargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::CallSelector { args, .. } => {
                        writeln!(self.out, "  %sargs{} = alloca [{} x i64]", vid.0, args.len() + 1).unwrap();
                    }
                    Instruction::AllocEnv { values } => self.env_buffer(vid.0, values),
                    Instruction::ConcatStrings { parts } => self.concat_strings_buffer(vid.0, parts),
                    _ => {}
                }
            }
        }
    }

    /// The globals of the module (`@dfg_<id>` and the `$ok` flag) become slots of the
    /// global area of the isolate: the VM keeps the statics in the *field table*
    /// of each isolate, and here each isolate (a thread) has its own area, which
    /// the runtime creates the first time (`dartforge_globals_area`, zeroed).
    /// The selector caches (`selectors.rs`) get slots after these.
    ///
    /// In a hot reload generation ([`LlvmEmitter::with_previous_area`]), the
    /// area starts with the whole layout of the live generation — including the slots
    /// that disappeared and its selector caches, which the old code still
    /// uses — and only appends: a global that remains stays at the same index.
    fn emit_globals(&mut self) {
        let previous: std::collections::HashMap<i64, usize> = match &self.previous_area {
            Some(a) => {
                self.slot_hashes = a.clone();
                a.iter().enumerate().filter(|(_, h)| **h != 0).map(|(i, h)| (*h, i)).collect()
            }
            None => std::collections::HashMap::new(),
        };
        self.slot_index.clone_from(&previous);
        for (_, _, sym_name) in &self.module.globals {
            for name_str in [sym_name.clone(), format!("{sym_name}$ok")] {
                let h = slot_hash(&name_str);
                let n = match previous.get(&h) {
                    Some(&i) => i,
                    None => {
                        self.slot_hashes.push(h);
                        self.slot_hashes.len() - 1
                    }
                };
                self.global_slots.insert(name_str, n);
            }
        }
    }

    /// The descriptor of the global area of the module: `[key, n, name_0…]`, with
    /// the stable key of the module and the hash of the name of each slot. The
    /// selector cache slots have [`CACHE_BIT`] in the name: they do not migrate to a
    /// new area and the publication of a reload zeroes them (they keep code
    /// addresses, and the method tables have just changed). In production,
    /// `[key, -n]` ([`LlvmEmitter::with_lean_area`]).
    fn emit_area_descriptor(&mut self) {
        let key = self.module.record.clone().unwrap_or_else(|| "df.program".to_string());
        if self.lean_area {
            // Production (`with_lean_area`): `[key, -n]`, without the names.
            let n = self.slot_hashes.len() as i64;
            writeln!(self.out, "@df.area = private unnamed_addr constant [2 x i64] [i64 {}, i64 {}]", slot_hash(&key), -n)
                .unwrap();
            return;
        }
        let mut values = vec![slot_hash(&key).to_string(), String::new()];
        for h in &self.slot_hashes {
            values.push(h.to_string());
        }
        values[1] = (values.len() - 2).to_string();
        let items: Vec<String> = values.iter().map(|v| format!("i64 {v}")).collect();
        writeln!(self.out, "@df.area = private unnamed_addr constant [{} x i64] [{}]", items.len(), items.join(", ")).unwrap();
    }

    /// The slot of the next selector cache of the function being emitted: the two
    /// words (class id, entry) with the name `(function, position)`, at the
    /// same index of the live generation when it already had it — the code of a
    /// function that did not change is the same text in both generations (J04).
    pub(super) fn cache_slot(&mut self) -> usize {
        let name_str = format!("{}#{}", self.current_function, self.cache_in_function);
        self.cache_in_function += 1;
        let (h0, h1) = (cache_hash(&name_str, 0), cache_hash(&name_str, 1));
        if let Some(&i) = self.slot_index.get(&h0)
            && self.slot_hashes.get(i + 1) == Some(&h1)
        {
            return i;
        }
        let i = self.slot_hashes.len();
        self.slot_hashes.push(h0);
        self.slot_hashes.push(h1);
        i
    }

    /// The `Ref` `alloca`s used outside their `load`/`store` (the address
    /// passed to a direct local function): see `frame_allocas`.
    fn escaping_ref_allocas(func: &Function) -> std::collections::HashSet<ValueId> {
        let mut refs = std::collections::HashSet::new();
        for b in &func.blocks {
            for (vid, inst, _) in &b.instructions {
                if matches!(inst, Instruction::Alloca(Type::Ref)) {
                    refs.insert(*vid);
                }
            }
        }
        let mut escape = std::collections::HashSet::new();
        if refs.is_empty() {
            return escape;
        }
        for b in &func.blocks {
            for (_, inst, _) in &b.instructions {
                let uses = match inst {
                    Instruction::Load { .. } => Vec::new(),
                    Instruction::Store { val: Operand::Val(v), .. } => vec![*v],
                    Instruction::Store { .. } => Vec::new(),
                    other => crate::lower::async_sm::uses_of(other),
                };
                escape.extend(uses.into_iter().filter(|v| refs.contains(v)));
            }
            escape.extend(crate::lower::async_sm::terminator_uses(&b.terminator).into_iter().filter(|v| refs.contains(v)));
        }
        escape
    }

    /// Does the function use the global area (a global, a flag or a selector cache)?
    fn uses_area(func: &Function) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(
                    i,
                    Instruction::LoadGlobal { .. }
                        | Instruction::StoreGlobal { .. }
                        | Instruction::ArcLoadStrong { slot: StrongSlot::Global { .. } }
                        | Instruction::ArcStoreStrong { slot: StrongSlot::Global { .. }, .. }
                        | Instruction::CallSelector { .. }
                        | Instruction::CallSelectorForward { .. }
                        | Instruction::Const(Constant::String(_) | Constant::StringWtf8(_))
                ) || Self::rti_evaluate_cached(i)
                    || Self::rti_test_cached(i)
            })
        })
    }

    /// The roots frame at the top of the entry block (see the prologue).
    fn frame_at_entry() -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        !*OFF.get_or_init(|| std::env::var_os("DARTFORGE_NO_FRAME_AT_ENTRY").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `dartforge_rti_eval` without a tuple and with `this`: goes through the cache of the
    /// use site in the area ([`Self::emit_rti_evaluate_cached`]).
    /// `DARTFORGE_NO_RTI_CACHE=1` at compile time turns it off (measurement).
    fn rti_evaluate_cached(i: &Instruction) -> bool {
        static OFF: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        matches!(i, Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
            if name == "dartforge_rti_eval"
                && args.len() == 4
                && matches!(args[3].0, Operand::Constant(Constant::Int(0)))
                && !matches!(args[1].0, Operand::Constant(_)))
            && !*OFF.get_or_init(|| std::env::var_os("DARTFORGE_NO_RTI_CACHE").is_some_and(|v| !v.is_empty() && v != "0"))
    }

    /// `dartforge_rti_e` (`v is T`) with a cache at the use site
    /// ([`Self::emit_rti_test_cached`]). Not under `DARTFORGE_EFFECTS=check`,
    /// which watches the extern itself; `DARTFORGE_NO_RTI_CACHE=1` turns it off too.
    fn rti_test_cached(i: &Instruction) -> bool {
        matches!(i, Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
            if name == "dartforge_rti_e" && args.len() == 2 && !matches!(args[0].0, Operand::Constant(_)))
            && !externs::check_effects()
            && std::env::var_os("DARTFORGE_NO_RTI_CACHE").is_none_or(|v| v.is_empty() || v == "0")
    }

    /// `v is T` with a cache at the use site, like [`Self::emit_rti_evaluate_cached`]:
    /// two words of the isolate's area keep the key of the type of the last
    /// object tested (the runtime's `value_key`: the header metadata, or
    /// `-16 - class`) and `T << 1 | answer`. The runtime's memo of `is` is keyed
    /// by the same pair, so an object with the stored key and the same `T` answers
    /// with three loads and no call — the covariance check of `List.[]=` in a
    /// sort called `dartforge_rti_e` (13% of `ordenar_objetos`). The rest (null,
    /// `Smi`, runtime values, a miss) calls `dartforge_rti_e_cache`, which
    /// answers and stores.
    fn emit_rti_test_cached(&mut self, v: u32, args: &[(Operand, Type)]) {
        let value = self.coerce(&args[0].0, Type::I64);
        let t = self.coerce(&args[1].0, Type::I64);
        let slot = self.cache_slot();
        let o = &mut self.out;
        writeln!(o, "  %rt{v}c = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %rt{v}m = and i64 {value}, {OBJECT_MASK}").unwrap();
        writeln!(o, "  %rt{v}o = icmp eq i64 %rt{v}m, 2").unwrap();
        writeln!(o, "  br i1 %rt{v}o, label %rt{v}.obj, label %rt{v}.lenta").unwrap();
        writeln!(o, "rt{v}.obj:").unwrap();
        writeln!(o, "  %rt{v}p = inttoptr i64 {value} to ptr").unwrap();
        writeln!(o, "  %rt{v}mp = getelementptr inbounds i8, ptr %rt{v}p, i64 10").unwrap();
        writeln!(o, "  %rt{v}md = load i32, ptr %rt{v}mp, align 4").unwrap();
        writeln!(o, "  %rt{v}cp = getelementptr inbounds i8, ptr %rt{v}p, i64 2").unwrap();
        writeln!(o, "  %rt{v}cl = load i32, ptr %rt{v}cp, align 4").unwrap();
        writeln!(o, "  %rt{v}mz = zext i32 %rt{v}md to i64").unwrap();
        writeln!(o, "  %rt{v}cs = sext i32 %rt{v}cl to i64").unwrap();
        writeln!(o, "  %rt{v}cn = sub i64 -16, %rt{v}cs").unwrap();
        writeln!(o, "  %rt{v}tm = icmp ne i32 %rt{v}md, 0").unwrap();
        writeln!(o, "  %rt{v}k = select i1 %rt{v}tm, i64 %rt{v}mz, i64 %rt{v}cn").unwrap();
        writeln!(o, "  %rt{v}k0 = load i64, ptr %rt{v}c, align 8").unwrap();
        writeln!(o, "  %rt{v}wp = getelementptr inbounds i64, ptr %rt{v}c, i64 1").unwrap();
        writeln!(o, "  %rt{v}w = load i64, ptr %rt{v}wp, align 8").unwrap();
        writeln!(o, "  %rt{v}wt = lshr i64 %rt{v}w, 1").unwrap();
        writeln!(o, "  %rt{v}ok1 = icmp eq i64 %rt{v}k0, %rt{v}k").unwrap();
        writeln!(o, "  %rt{v}ok2 = icmp eq i64 %rt{v}wt, {t}").unwrap();
        writeln!(o, "  %rt{v}ok = and i1 %rt{v}ok1, %rt{v}ok2").unwrap();
        writeln!(o, "  br i1 %rt{v}ok, label %rt{v}.acerto, label %rt{v}.lenta").unwrap();
        writeln!(o, "rt{v}.acerto:").unwrap();
        writeln!(o, "  %rt{v}b = and i64 %rt{v}w, 1").unwrap();
        writeln!(o, "  %rt{v}r1 = trunc i64 %rt{v}b to i8").unwrap();
        writeln!(o, "  br label %rt{v}.fim").unwrap();
        writeln!(o, "rt{v}.lenta:").unwrap();
        writeln!(o, "  %rt{v}r2 = call i8 @dartforge_rti_e_cache(ptr %rt{v}c, i64 {value}, i64 {t})").unwrap();
        writeln!(o, "  br label %rt{v}.fim").unwrap();
        writeln!(o, "rt{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i8 [ %rt{v}r1, %rt{v}.acerto ], [ %rt{v}r2, %rt{v}.lenta ]").unwrap();
        self.current_label = format!("rt{v}.fim");
    }

    /// `P<i>` evaluated with a cache at the use site (N17): two words of the
    /// area of the isolate keep the key of the type of `this` — the metadata of the
    /// object header (the runtime's `Header`, offset 12 of the block, 10
    /// of the handle), or `-16 - class` without metadata, the same key as the memory
    /// of the runtime (`value_key`) — and the result. An object of the space
    /// with the stored key answers with two loads; the rest (runtime
    /// values, the first time, another key) calls
    /// `dartforge_rti_eval_cache`, which answers and stores. The model and the
    /// class are constants of the site; the tuple is 0. The area is per isolate (the
    /// type ids are of its universe) and reload zeroes the caches.
    fn emit_rti_evaluate_cached(&mut self, v: u32, args: &[(Operand, Type)]) {
        let model = self.coerce(&args[0].0, Type::I64);
        let this = self.coerce(&args[1].0, Type::I64);
        let class = self.coerce(&args[2].0, Type::I64);
        let slot = self.cache_slot();
        let o = &mut self.out;
        writeln!(o, "  %ra{v}c = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %ra{v}m = and i64 {this}, {OBJECT_MASK}").unwrap();
        writeln!(o, "  %ra{v}o = icmp eq i64 %ra{v}m, 2").unwrap();
        writeln!(o, "  br i1 %ra{v}o, label %ra{v}.obj, label %ra{v}.lenta").unwrap();
        writeln!(o, "ra{v}.obj:").unwrap();
        writeln!(o, "  %ra{v}p = inttoptr i64 {this} to ptr").unwrap();
        writeln!(o, "  %ra{v}mp = getelementptr inbounds i8, ptr %ra{v}p, i64 10").unwrap();
        writeln!(o, "  %ra{v}md = load i32, ptr %ra{v}mp, align 4").unwrap();
        writeln!(o, "  %ra{v}cp = getelementptr inbounds i8, ptr %ra{v}p, i64 2").unwrap();
        writeln!(o, "  %ra{v}cl = load i32, ptr %ra{v}cp, align 4").unwrap();
        writeln!(o, "  %ra{v}mz = zext i32 %ra{v}md to i64").unwrap();
        writeln!(o, "  %ra{v}cs = sext i32 %ra{v}cl to i64").unwrap();
        writeln!(o, "  %ra{v}cn = sub i64 -16, %ra{v}cs").unwrap();
        writeln!(o, "  %ra{v}tm = icmp ne i32 %ra{v}md, 0").unwrap();
        writeln!(o, "  %ra{v}k = select i1 %ra{v}tm, i64 %ra{v}mz, i64 %ra{v}cn").unwrap();
        writeln!(o, "  %ra{v}k0 = load i64, ptr %ra{v}c, align 8").unwrap();
        writeln!(o, "  %ra{v}ok = icmp eq i64 %ra{v}k0, %ra{v}k").unwrap();
        writeln!(o, "  br i1 %ra{v}ok, label %ra{v}.acerto, label %ra{v}.lenta").unwrap();
        writeln!(o, "ra{v}.acerto:").unwrap();
        writeln!(o, "  %ra{v}rp = getelementptr inbounds i64, ptr %ra{v}c, i64 1").unwrap();
        writeln!(o, "  %ra{v}r1 = load i64, ptr %ra{v}rp, align 8").unwrap();
        writeln!(o, "  br label %ra{v}.fim").unwrap();
        writeln!(o, "ra{v}.lenta:").unwrap();
        writeln!(o, "  %ra{v}r2 = call i64 @dartforge_rti_eval_cache(ptr %ra{v}c, i64 {model}, i64 {this}, i64 {class})").unwrap();
        writeln!(o, "  br label %ra{v}.fim").unwrap();
        writeln!(o, "ra{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %ra{v}r1, %ra{v}.acerto ], [ %ra{v}r2, %ra{v}.lenta ]").unwrap();
        self.current_label = format!("ra{v}.fim");
    }

    /// The address of the slot of a global, in `%ga<v>`.
    fn global_address(&mut self, v: u32, sym_name: &str) {
        let slot = *self
            .global_slots
            .get(sym_name)
            .unwrap_or_else(|| panic!("compiler bug: global @{sym_name} without a slot in the area"));
        writeln!(self.out, "  %ga{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
    }

    fn emit_dispatch_functions(&mut self) {
        self.out.push_str("define i64 @dartforge_dispatch_toString(i64 %obj) {\n");
        self.out.push_str("b0:\n");
        self.out.push_str("  %is_null = icmp eq i64 %obj, 0\n");
        self.out.push_str("  br i1 %is_null, label %ret_null, label %check_obj\n");
        self.out.push_str("ret_null:\n");
        self.out.push_str("  %null_s = call i64 @dartforge_to_string_handle(i64 0)\n");
        self.out.push_str("  ret i64 %null_s\n");
        self.out.push_str("check_obj:\n");
        self.out.push_str("  %cls = call i64 @df.class(i64 %obj)\n");

        let mut cases = Vec::new();
        for class in &self.module.classes {
            if self.module.closed_class_tables.as_ref().is_some_and(|classes| !classes.contains(&class.id)) {
                continue;
            }
            if let Some(sym) = &class.to_string_symbol {
                cases.push((class.id, sym.clone()));
            }
        }

        if cases.is_empty() {
            self.out.push_str("  br label %fallback\n");
        } else {
            write!(self.out, "  switch i64 %cls, label %fallback [").unwrap();
            for (cid, _) in &cases {
                write!(self.out, " i64 {cid}, label %case_{cid}").unwrap();
            }
            writeln!(self.out, " ]").unwrap();
            for (cid, sym) in &cases {
                writeln!(self.out, "case_{cid}:").unwrap();
                writeln!(self.out, "  %res_{cid} = call i64 @{sym}(i64 %obj)").unwrap();
                writeln!(self.out, "  ret i64 %res_{cid}").unwrap();
            }
        }
        self.out.push_str("fallback:\n");
        self.out.push_str("  %fb = call i64 @dartforge_to_string_handle(i64 %obj)\n");
        self.out.push_str("  ret i64 %fb\n");
        self.out.push_str("}\n\n");
    }

    /// An already evaluated reference: do not use coercion, which could allocate a box.
    /// Invalid HIR is an internal failure; the verifier checks this precondition.
    fn arc_reference(&self, value: &Operand) -> String {
        match value {
            Operand::Val(v) => {
                assert_eq!(self.types.get(v), Some(&Type::Ref), "ARC operation requires an SSA Ref");
                format!("%v{}", v.0)
            }
            Operand::Constant(Constant::Null) => "0".into(),
            _ => panic!("ARC operation requires an already evaluated reference"),
        }
    }

    /// Classified frame descriptor; never coerce a pointer to an ID.
    fn slot_arc(&self, slot: &StrongSlot) -> (String, u32) {
        let StrongSlot::Frame { frame, index: idx_value } = slot else { panic!("descriptor is not a frame") };
        let Operand::Val(v) = frame else {
            panic!("ARC slot requires an SSA frame")
        };
        assert_eq!(
            self.types.get(v),
            Some(&Type::I64),
            "ARC frame requires a scalar I64 ID"
        );
        (format!("%v{}", v.0), *idx_value)
    }

    /// Schema of the real storage; do not infer Ref by width or address.
    fn check_arc_global(&self, sym_name: &str) {
        let mut declarations = self.module.globals.iter().filter(|(_, _, s)| s == sym_name);
        assert_eq!(declarations.next().map(|(_, ty, _)| *ty), Some(Type::Ref), "ARC global must be a declared Ref");
        assert!(declarations.next().is_none(), "ARC global must have a single declaration");
    }

    /// Checks the token ABI before the registrations and before any Dart code.
    /// The failure is internal and fatal: it cannot become an ignored pending flag.
    fn start_arc(&mut self) {
        if !self.module.arc_memory {
            return;
        }
        self.externals.insert("llvm.trap".into(), "declare void @llvm.trap()".into());
        self.out.push_str("  call void @dartforge_memory_arc_v1()\n\
            \x20 %df.arc.abi = call i8 @dartforge_arc_verify_abi(i64 1)\n\
            \x20 %df.arc.abi.ok = icmp eq i8 %df.arc.abi, 1\n\
            \x20 br i1 %df.arc.abi.ok, label %df.arc.compatible, label %df.arc.incompatible\n\
            df.arc.incompatible:\n\
            \x20 call void @llvm.trap()\n\
            \x20 unreachable\n\
            df.arc.compatible:\n");
        if !self.module.sdk_mode {
            self.register_arc_aot_ports();
        }
    }

    fn emit_entry(&mut self) {
        if self.module.table_exceptions {
            self.emit_ports();
        }
        if self.module.sdk_mode {
            self.emit_registration("df.register.program");
            let ids: Vec<String> = self.module.runtime_cids.iter().map(|c| format!("i64 {c}")).collect();
            writeln!(
                self.out,
                "@df.cids = private unnamed_addr constant [{} x i64] [{}]",
                ids.len(),
                ids.join(", ")
            )
            .unwrap();
            if let Some(v) = &self.module.sdk_version {
                writeln!(
                    self.out,
                    "@df.sdk_version = private unnamed_addr constant [{} x i8] c\"{}\"",
                    v.len(),
                    selectors::llvm_bytes(v)
                )
                .unwrap();
            }
            // The preparation of an isolate (the main one and that of each
            // `Isolate.spawn`, which the runtime calls on the new thread): the
            // registrations of the libraries, the RTI and the embedder.
            writeln!(self.out, "define void @df.prepare_isolate() {{").unwrap();
            // ARC memory, turned on before any Dart code of the isolate.
            self.start_arc();
            // Prepared ARC image: the Dart ports that the runtime calls here
            // (registrations, RTI, embedder) return owners to a native scope.
            let root_scope = self.module.arc_memory && self.module.arc_prepared;
            if root_scope {
                writeln!(self.out, "  %escopo.prep = call i64 @dartforge_arc_native_scope_open_v1()").unwrap();
            }
            writeln!(self.out, "  call void @dartforge_register_cids(ptr @df.cids, i64 {})", ids.len()).unwrap();
            if let Some(v) = &self.module.sdk_version {
                writeln!(self.out, "  call void @dartforge_register_sdk_version(ptr @df.sdk_version, i64 {})", v.len()).unwrap();
            }
            // The tables of the classes of the runtime values (which do not go
            // through `dartforge_object_new_t`).
            for c in self.module.runtime_cids.clone() {
                if let Some(f) = self.module.table_functions.get(&(c as u32)).cloned() {
                    self.annotate_external(&f, Type::Ptr, &[]);
                    writeln!(self.out, "  call void @dartforge_register_table(i64 {c}, ptr @{f})").unwrap();
                }
            }
            for r in &self.module.sdk_records {
                writeln!(self.out, "  call void @{r}()").unwrap();
                self.externals.insert(r.clone(), format!("declare void @{r}()"));
            }
            writeln!(self.out, "  call void @df.register.program()").unwrap();
            if self.tables_registered_at_startup {
                // Production: the SDK tables assembled at link time
                // (`pruning::assemble` defines `@df.register.sdk_tables` from
                // this declaration), registered before any Dart code: the
                // inline allocation does not test the registration.
                writeln!(self.out, "  call void @df.register.sdk_tables()").unwrap();
                self.externals.insert("df.register.sdk_tables".into(), "declare void @df.register.sdk_tables()".into());
            }
            // RTI and the event loop, as in the usual entry (below).
            if let Some(start) = &self.module.start_rti {
                writeln!(self.out, "  {}", entry_call(self.module.table_exceptions, start, &[])).unwrap();
            }
            // What the VM's embedder prepares before `main` (the script of
            // `dart:io`, `Uri.base`), already with the libraries registered.
            // `dart:ffi`: the native types and the trampolines of the signatures
            // (`lower/ffi.rs`).
            for c in self.module.ffi_composites.clone() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_register_compound(i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {})",
                    c.rti, c.class, c.fields, c.base_index, c.offset_index, c.size, c.alignment
                )
                .unwrap();
            }
            for (c, letter) in &self.module.ffi_types {
                writeln!(self.out, "  call void @dartforge_ffi_register_type(i64 {c}, i64 {})", u32::from(*letter)).unwrap();
            }
            for (i, (key, sym_name)) in self.module.ffi_trampolines.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_register_trampoline(ptr @df.ffi.key.{i}, i64 {}, ptr @\"{sym_name}\")",
                    key.len()
                )
                .unwrap();
            }
            for (i, cb) in self.module.ffi_callbacks.iter().enumerate() {
                writeln!(
                    self.out,
                    "  call void @dartforge_ffi_register_callback(ptr @df.ffi.cbkey.{i}, i64 {}, ptr @df.ffi.cbentry.{i})",
                    cb.key.len()
                )
                .unwrap();
            }
            writeln!(self.out, "  call void @dartforge_prepare_embedder()").unwrap();
            if root_scope {
                writeln!(self.out, "  call void @dartforge_arc_native_scope_close_v1(i64 %escopo.prep)").unwrap();
            }
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            for (i, (key, _)) in self.module.ffi_trampolines.iter().enumerate() {
                writeln!(self.out, "@df.ffi.key.{i} = private unnamed_addr constant [{} x i8] c\"{key}\"", key.len()).unwrap();
            }
            self.emit_callbacks_ffi();
            writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
            // Exceptions by tables: the ports through which the runtime calls
            // Dart code, delivered before any Dart code runs.
            if self.module.table_exceptions {
                writeln!(self.out, "  call void @dartforge_register_ports(ptr @df.ports, i64 {N_PORTS})").unwrap();
            }
            let call = self.module.call_dart.as_ref().map_or("null".to_string(), |c| format!("@{c}"));
            writeln!(self.out, "  call void @dartforge_register_isolates(ptr @df.prepare_isolate, ptr {call})").unwrap();
            writeln!(self.out, "  call void @df.prepare_isolate()").unwrap();
            // The root scope of the main isolate: `main` and the event
            // loop, whose calls from the runtime into Dart return owners.
            if root_scope {
                writeln!(self.out, "  %escopo.raiz = call i64 @dartforge_arc_native_scope_open_v1()").unwrap();
            }
            self.call_main();
            if let Some(call) = &self.module.call_dart {
                writeln!(self.out, "  call void @dartforge_event_loop(ptr @{call})").unwrap();
            }
            if root_scope {
                writeln!(self.out, "  call void @dartforge_arc_native_scope_close_v1(i64 %escopo.raiz)").unwrap();
            }
            writeln!(self.out, "  ret void\n}}\n").unwrap();
            // The runtime and the SDK live in the source SDK DLL: the `main` of the
            // executable is this one, and it hands the entry to the runtime.
            writeln!(
                self.out,
                "define i32 @main() {{\n  %r = call i32 @dartforge_init(ptr @dartforge_entry, ptr @dartforge_dispatch_toString)\n  ret i32 %r\n}}\n"
            )
            .unwrap();
            return;
        }
        writeln!(self.out, "define void @dartforge_entry() {{").unwrap();
        // ARC memory, turned on before any Dart code.
        self.start_arc();
        // Exceptions by tables: the ports through which the runtime calls
        // Dart code, delivered before any Dart code runs.
        if self.module.table_exceptions {
            writeln!(self.out, "  call void @dartforge_register_ports(ptr @df.ports, i64 {N_PORTS})").unwrap();
        }
        // Registra classes
        for class in &self.module.classes {
            let idx = self.string_const_index(class.name.as_bytes()).unwrap_or(0);
            let len = class.name.len();
            writeln!(
                self.out,
                "  call void @dartforge_register_class_name(i64 {}, ptr @.str.{}, i64 {})",
                class.id, idx, len
            ).unwrap();
        }

        if self.maps && !self.maps_in_jit {
            self.out.push_str(Self::maps_registration_call());
        }
        if let Some(c) = self.trace_registration_call() {
            self.out.push_str(&c);
            let fields = self.trace_field_registration_calls();
            self.out.push_str(&fields);
        }
        // Registers the subtyping graph
        for (sub, sup) in &self.module.subtyping_edges {
            writeln!(
                self.out,
                "  call void @dartforge_register_subclass(i64 {sub}, i64 {sup})"
            ).unwrap();
        }

        // RTI: the type universe (cited classes and supertype rules).
        if let Some(start) = &self.module.start_rti {
            writeln!(self.out, "  {}", entry_call(self.module.table_exceptions, start, &[])).unwrap();
        }
        self.call_main();
        // P6: microtasks and timers after `main` (runtime, `events.rs`).
        if let Some(call) = &self.module.call_dart {
            writeln!(self.out, "  call void @dartforge_event_loop(ptr @{call})").unwrap();
        }
        writeln!(self.out, "  ret void").unwrap();
        writeln!(self.out, "}}\n").unwrap();
    }

    /// The call of `main` at the entry: with parameters, the first is the list
    /// of the command line arguments (`dartforge_main_arguments`) and
    /// the second, `null`.
    /// A primitive argument of a C call: the operand converted to the C type
    /// (narrow ones with the ABI extension), as `ty attributes value`.
    fn c_argument(&mut self, v: u32, i: usize, op: &Operand, tc: CType) -> String {
        let x = match tc {
            CType::F32 => {
                let d = self.coerce(op, Type::F64);
                writeln!(self.out, "  %na{v}_{i} = fptrunc double {d} to float").unwrap();
                format!("%na{v}_{i}")
            }
            CType::F64 => self.coerce(op, Type::F64),
            CType::Bool => self.coerce(op, Type::I1),
            CType::Ptr | CType::Handle => {
                let n = self.coerce(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                format!("%na{v}_{i}")
            }
            CType::I64 | CType::U64 => self.coerce(op, Type::I64),
            CType::Void => unreachable!("void native argument"),
            narrow => {
                let n = self.coerce(op, Type::I64);
                writeln!(self.out, "  %na{v}_{i} = trunc i64 {n} to {}", narrow.llvm()).unwrap();
                format!("%na{v}_{i}")
            }
        };
        format!("{} {}{x}", tc.llvm(), tc.extension())
    }

    /// The `call …` text of a C call through `%fn<v>`. In a variadic one with
    /// roots by maps, the call goes through an intermediary
    /// `@df.vararg.<k>` with a fixed signature, `noinline` and outside the collection
    /// strategy: `rewrite-statepoints-for-gc` does not wrap variadic calls
    /// that return a value (`gc.statepoint doesn't support wrapping non-void
    /// vararg functions yet`), and the call to the intermediary becomes a common
    /// statepoint, with the live roots in its `"deopt"` (§14.12: every call that
    /// collects carries the operand, also that of a function without roots, which may be
    /// copied by the inliner into one with roots). The intermediary passes on the
    /// arguments with the same types and attributes.
    fn c_call_text(&mut self, v: u32, ret: &str, typ: &str, parts: &[String], variadic_flag: bool) -> String {
        let list = parts.join(", ");
        if !variadic_flag || !self.maps {
            return format!("call {typ} %fn{v}({list})");
        }
        let name_str = format!("df.vararg.{}", self.variadic_set.len());
        let mut params = vec!["ptr %f".to_string()];
        let mut forward = Vec::with_capacity(parts.len());
        for (j, p) in parts.iter().enumerate() {
            let head = p.rsplit_once(' ').map_or(p.as_str(), |(c, _)| c);
            params.push(format!("{head} %p{j}"));
            forward.push(format!("{head} %p{j}"));
        }
        let internal = format!("call {typ} %f({})", forward.join(", "));
        let body = if ret == "void" { format!("  {internal}\n  ret void\n") } else { format!("  %r = {internal}\n  ret {ret} %r\n") };
        self.variadic_set.push(format!("define internal {ret} @{name_str}({}) noinline {{\n{body}}}\n\n", params.join(", ")));
        let mut args = vec![format!("ptr %fn{v}")];
        args.extend(parts.iter().cloned());
        format!("call {ret} @{name_str}({})", args.join(", "))
    }

    /// A C call with structs/unions by value (`llvm/c_abi.rs`): each
    /// compound is copied from the bytes of the operand into an aligned temporary
    /// on the stack (released by `stackrestore` after the call), and passed
    /// in pieces, `byval` or by pointer; a compound return goes to
    /// `dest` (direct: through the pieces written into a temporary; `sret`: the
    /// destination itself).
    fn composite_native_call(
        &mut self,
        v: u32,
        target: &Operand,
        args: &[(Operand, NativeType)],
        ret: &NativeType,
        dest: Option<&Operand>,
        variadic_flag: Option<usize>,
    ) {
        use c_abi::{ArgPassing, RetPassing};
        let conv = c_abi::Convention::of_target();
        let mut regs = c_abi::Registers::new_set();
        let a = self.coerce(target, Type::I64);
        writeln!(self.out, "  %fn{v} = inttoptr i64 {a} to ptr").unwrap();
        writeln!(self.out, "  %pilha{v} = call ptr @llvm.stacksave.p0()").unwrap();
        // Return first: an `sret` takes the first integer register.
        let ret_passing = match ret {
            NativeType::Composite(l) => Some((c_abi::ret(conv, l, &mut regs), l.clone())),
            NativeType::Prim(_) => None,
        };
        let mut parts = Vec::with_capacity(args.len() + 1);
        let dest_ptr = dest.map(|d| {
            let d = self.coerce(d, Type::I64);
            writeln!(self.out, "  %dst{v} = inttoptr i64 {d} to ptr").unwrap();
            format!("%dst{v}")
        });
        if let (Some((RetPassing::Sret { alignment }, l)), Some(d)) = (&ret_passing, &dest_ptr) {
            parts.push(format!("ptr sret([{} x i8]) align {alignment} {d}", l.size));
        }
        let mut fixed_types: Vec<String> = Vec::new();
        if matches!(&ret_passing, Some((RetPassing::Sret { .. }, _))) {
            fixed_types.push("ptr".to_string());
        }
        for (i, (op, t)) in args.iter().enumerate() {
            let variadic = variadic_flag.is_some_and(|n| i >= n);
            match t {
                // Default C argument promotions in a variadic argument:
                // `float` goes as `double`; integers smaller than `int`,
                // as `int` (with the sign of the type).
                NativeType::Prim(tc) if variadic => {
                    regs.consume_primitive(*tc);
                    let p = match tc {
                        CType::F32 => self.c_argument(v, i, op, CType::F64),
                        CType::I8 | CType::I16 => self.c_argument(v, i, op, CType::I32),
                        CType::U8 | CType::U16 | CType::Bool => {
                            let n = self.coerce(op, if *tc == CType::Bool { Type::I1 } else { Type::I64 });
                            let ext = if *tc == CType::Bool { format!("zext i1 {n} to i32") } else { format!("trunc i64 {n} to i32") };
                            writeln!(self.out, "  %na{v}_{i} = {ext}").unwrap();
                            format!("i32 %na{v}_{i}")
                        }
                        _ => self.c_argument(v, i, op, *tc),
                    };
                    parts.push(p);
                }
                NativeType::Prim(tc) => {
                    regs.consume_primitive(*tc);
                    let p = self.c_argument(v, i, op, *tc);
                    fixed_types.push(p.split(' ').next().unwrap_or("i64").to_string());
                    parts.push(p);
                }
                NativeType::Composite(l) => {
                    // The copy: aligned, zeroed up to the word, with the bytes.
                    let n = self.coerce(op, Type::I64);
                    let sz = l.size.div_ceil(16) * 16;
                    let al = l.alignment.max(16);
                    writeln!(self.out, "  %src{v}_{i} = inttoptr i64 {n} to ptr").unwrap();
                    writeln!(self.out, "  %tmp{v}_{i} = alloca [{sz} x i8], align {al}").unwrap();
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %tmp{v}_{i}, i8 0, i64 {sz}, i1 false)").unwrap();
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %tmp{v}_{i}, ptr %src{v}_{i}, i64 {}, i1 false)", l.size).unwrap();
                    // In the variadic part the pieces do not enter the type of the
                    // call (`ret (fixed, ...)`).
                    let passing = if variadic {
                        c_abi::variadic_argument(conv, l, &mut regs)
                    } else {
                        c_abi::argument(conv, l, &mut regs)
                    };
                    let mut fixed = |t: String| {
                        if !variadic {
                            fixed_types.push(t);
                        }
                    };
                    match passing {
                        ArgPassing::Direct(pieces) => {
                            for (k, p) in pieces.iter().enumerate() {
                                writeln!(self.out, "  %pp{v}_{i}_{k} = getelementptr i8, ptr %tmp{v}_{i}, i64 {}", p.offset).unwrap();
                                writeln!(self.out, "  %pc{v}_{i}_{k} = load {}, ptr %pp{v}_{i}_{k}, align 1", p.ty).unwrap();
                                parts.push(format!("{} {}%pc{v}_{i}_{k}", p.ty, p.attributes));
                                fixed(p.ty.clone());
                            }
                        }
                        ArgPassing::Byval { alignment } => {
                            parts.push(format!("ptr byval([{} x i8]) align {alignment} %tmp{v}_{i}", l.size));
                            fixed("ptr".to_string());
                        }
                        ArgPassing::Indirect => {
                            parts.push(format!("ptr %tmp{v}_{i}"));
                            fixed("ptr".to_string());
                        }
                    }
                }
            }
        }
        // The type of the call: in a variadic one, `ret (fixed, ...)`.
        let call_type = |ret: &str| match variadic_flag {
            Some(_) if fixed_types.is_empty() => format!("{ret} (...)"),
            Some(_) => format!("{ret} ({}, ...)", fixed_types.join(", ")),
            None => ret.to_string(),
        };
        let return_type = match (ret, &ret_passing) {
            (NativeType::Prim(CType::Void), _) | (NativeType::Composite(_), Some((RetPassing::Sret { .. }, _))) => "void".to_string(),
            (NativeType::Prim(tc), _) => tc.llvm().to_string(),
            (NativeType::Composite(_), Some((RetPassing::Direct(pieces), _))) => c_abi::return_ty(pieces),
            (NativeType::Composite(_), None) => unreachable!("compound return without passing"),
        };
        let call = self.c_call_text(v, &return_type, &call_type(&return_type), &parts, variadic_flag.is_some());
        match (ret, &ret_passing) {
            (NativeType::Prim(CType::Void), _) => writeln!(self.out, "  {call}").unwrap(),
            (NativeType::Prim(tc), _) => {
                let conv_ret = match tc {
                    CType::I8 | CType::I16 | CType::I32 => Some(format!("sext {} %nr{v} to i64", tc.llvm())),
                    CType::U8 | CType::U16 | CType::U32 => Some(format!("zext {} %nr{v} to i64", tc.llvm())),
                    CType::F32 => Some(format!("fpext float %nr{v} to double")),
                    CType::Ptr | CType::Handle => Some(format!("ptrtoint ptr %nr{v} to i64")),
                    _ => None,
                };
                match conv_ret {
                    Some(c) => {
                        writeln!(self.out, "  %nr{v} = {call}").unwrap();
                        writeln!(self.out, "  %v{v} = {c}").unwrap();
                    }
                    None => writeln!(self.out, "  %v{v} = {call}").unwrap(),
                }
            }
            (NativeType::Composite(_), Some((RetPassing::Sret { .. }, _))) => {
                writeln!(self.out, "  {call}").unwrap();
            }
            (NativeType::Composite(_), Some((RetPassing::Direct(pieces), l))) => {
                let typ = c_abi::return_ty(pieces);
                let sz = l.size.div_ceil(16) * 16;
                writeln!(self.out, "  %rr{v} = {call}").unwrap();
                writeln!(self.out, "  %rt{v} = alloca [{sz} x i8], align 16").unwrap();
                for (k, p) in pieces.iter().enumerate() {
                    let val = if pieces.len() == 1 {
                        format!("%rr{v}")
                    } else {
                        writeln!(self.out, "  %re{v}_{k} = extractvalue {typ} %rr{v}, {k}").unwrap();
                        format!("%re{v}_{k}")
                    };
                    writeln!(self.out, "  %rp{v}_{k} = getelementptr i8, ptr %rt{v}, i64 {}", p.offset).unwrap();
                    writeln!(self.out, "  store {} {val}, ptr %rp{v}_{k}, align 1", p.ty).unwrap();
                }
                if let Some(d) = &dest_ptr {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr {d}, ptr %rt{v}, i64 {}, i1 false)", l.size).unwrap();
                }
            }
            (NativeType::Composite(_), None) => unreachable!("compound return without passing"),
        }
        writeln!(self.out, "  call void @llvm.stackrestore.p0(ptr %pilha{v})").unwrap();
    }

    /// The C entries of the `dart:ffi` callbacks (`ffi_callbacks.rs` of the
    /// runtime). Each entry has the C ABI of the signature and the context of the
    /// callback in the `nest` parameter (r10 on x86-64, x15 on AArch64), which the
    /// trampoline written by the runtime loads: in listener mode it copies
    /// the arguments into the message; otherwise it converts them to the
    /// Dart representation, calls the HIR body and converts the return — or
    /// returns the exceptional return, if the closure threw.
    /// Converts runtime scalar result bits to the exact native callback ABI.
    /// Float results use widened Double bits, matching the callback runtime.
    fn emit_callback_bits_return(&mut self, native: CType, llvm_return: &str, bits: &str, prefix: &str) {
        let conversion = match native {
            CType::I64 | CType::U64 => format!("add i64 {bits}, 0"),
            CType::F64 => format!("bitcast i64 {bits} to double"),
            CType::F32 => {
                writeln!(self.out, "  %{prefix}_double = bitcast i64 {bits} to double").unwrap();
                format!("fptrunc double %{prefix}_double to float")
            }
            CType::Ptr | CType::Handle => format!("inttoptr i64 {bits} to ptr"),
            narrow => format!("trunc i64 {bits} to {}", narrow.llvm()),
        };
        writeln!(self.out, "  %{prefix}_value = {conversion}\n  ret {llvm_return} %{prefix}_value").unwrap();
    }

    fn emit_callbacks_ffi(&mut self) {
        use c_abi::{ArgPassing, RetPassing};
        if self.module.ffi_callbacks.is_empty() {
            return;
        }
        let conv = c_abi::Convention::of_target();
        for (i, cb) in self.module.ffi_callbacks.clone().iter().enumerate() {
            writeln!(self.out, "@df.ffi.cbkey.{i} = private unnamed_addr constant [{} x i8] c\"{}\"", cb.key.len(), cb.key).unwrap();
            let mut regs = c_abi::Registers::new_set();
            // The return first: an `sret` takes the first register.
            let ret = match &cb.ret {
                NativeType::Composite(l) => Some((c_abi::ret(conv, l, &mut regs), l.clone())),
                NativeType::Prim(_) => None,
            };
            let mut decls = vec!["ptr nest %ctx".to_string()];
            if let Some((RetPassing::Sret { alignment }, l)) = &ret {
                decls.push(format!("ptr sret([{} x i8]) align {alignment} %sret", l.size));
            }
            // How each compound arrives: in pieces (reassembled into a
            // temporary) or already in memory (`byval`/pointer).
            let mut pieces_of: Vec<Option<(Vec<c_abi::Piece>, usize)>> = Vec::with_capacity(cb.params.len());
            for (j, t) in cb.params.iter().enumerate() {
                match t {
                    NativeType::Prim(tc) => {
                        regs.consume_primitive(*tc);
                        decls.push(format!("{} {}%a{j}", tc.llvm(), tc.extension()));
                        pieces_of.push(None);
                    }
                    NativeType::Composite(l) => match c_abi::argument(conv, l, &mut regs) {
                        ArgPassing::Direct(pieces) => {
                            for (k, p) in pieces.iter().enumerate() {
                                decls.push(format!("{} {}%a{j}_{k}", p.ty, p.attributes));
                            }
                            pieces_of.push(Some((pieces, l.size)));
                        }
                        ArgPassing::Byval { alignment } => {
                            decls.push(format!("ptr byval([{} x i8]) align {alignment} %a{j}", l.size));
                            pieces_of.push(None);
                        }
                        ArgPassing::Indirect => {
                            decls.push(format!("ptr %a{j}"));
                            pieces_of.push(None);
                        }
                    },
                }
            }
            let (ret_type, ret_ext) = match (&cb.ret, &ret) {
                (NativeType::Prim(tc), _) => (
                    tc.llvm().to_string(),
                    match tc {
                        CType::I8 | CType::I16 => "signext ",
                        CType::U8 | CType::U16 | CType::Bool => "zeroext ",
                        _ => "",
                    },
                ),
                (_, Some((RetPassing::Direct(pieces), _))) => (c_abi::return_ty(pieces), ""),
                _ => ("void".to_string(), ""),
            };
            let r = ret_type.as_str();
            // Exceptions by tables: the entry lands the unwinding of the body
            // (below) — C code is not crossed. The
            // `callback_without_pad` sabotage (D9 of §7.3) removes the landing pad.
            let by_tables = self.module.table_exceptions && !crate::target::sabotage("callback_without_pad");
            let personality = if by_tables { " personality ptr @dartforge_personality" } else { "" };
            writeln!(self.out, "define internal {ret_ext}{r} @df.ffi.cbentry.{i}({}){personality} {{", decls.join(", "))
                .unwrap();
            let n = cb.params.len().max(1);
            // A retired callback is a permanent tombstone. Do not touch Dart
            // TLS, roots or native argument memory on the closed path.
            writeln!(self.out, "entrada:\n  %modo = call i64 @dartforge_ffi_callback_enter(ptr %ctx)\n  %is_closed_callback = icmp eq i64 %modo, 2\n  br i1 %is_closed_callback, label %closed_callback, label %active_callback\nclosed_callback:").unwrap();
            match (&cb.ret, &ret) {
                (NativeType::Prim(CType::Void), _) => writeln!(self.out, "  ret void").unwrap(),
                (_, Some((RetPassing::Sret { .. }, layout))) => {
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %sret, i8 0, i64 {}, i1 false)\n  ret void", layout.size).unwrap();
                }
                (_, Some((RetPassing::Direct(_), _))) => {
                    writeln!(self.out, "  ret {r} zeroinitializer").unwrap();
                }
                (NativeType::Prim(native), _) => {
                    writeln!(self.out, "  %late_bits = call i64 @dartforge_ffi_callback_late_return(ptr %ctx)").unwrap();
                    self.emit_callback_bits_return(*native, r, "%late_bits", "late");
                }
                _ => unreachable!("compound callback return without ABI passing"),
            }
            writeln!(self.out, "active_callback:\n  %saida = alloca i64\n  %buf = alloca [{n} x i64]").unwrap();
            if let Some((_, l)) = &ret {
                writeln!(self.out, "  %rt = alloca [{} x i8], align 16", l.size.div_ceil(16) * 16).unwrap();
            }
            // The address of each compound (`%end{j}`, i64).
            for (j, t) in cb.params.iter().enumerate() {
                let NativeType::Composite(_) = t else { continue };
                match &pieces_of[j] {
                    Some((pieces, size)) => {
                        writeln!(self.out, "  %m{j} = alloca [{} x i8], align 16", size.div_ceil(16) * 16).unwrap();
                        for (k, p) in pieces.iter().enumerate() {
                            writeln!(self.out, "  %mp{j}_{k} = getelementptr i8, ptr %m{j}, i64 {}", p.offset).unwrap();
                            writeln!(self.out, "  store {} %a{j}_{k}, ptr %mp{j}_{k}, align 1", p.ty).unwrap();
                        }
                        writeln!(self.out, "  %end{j} = ptrtoint ptr %m{j} to i64").unwrap();
                    }
                    None => writeln!(self.out, "  %end{j} = ptrtoint ptr %a{j} to i64").unwrap(),
                }
            }
            writeln!(self.out, "  %e_ouvinte = icmp ne i64 %modo, 0\n  br i1 %e_ouvinte, label %ouvinte, label %local").unwrap();
            // Listener: the bits of each argument in the message (of a
            // compound, the address: the runtime copies the bytes).
            writeln!(self.out, "ouvinte:").unwrap();
            for (j, t) in cb.params.iter().enumerate() {
                let bits = match t {
                    NativeType::Composite(_) => format!("add i64 %end{j}, 0"),
                    NativeType::Prim(tc) => match tc {
                        CType::I8 | CType::I16 | CType::I32 => format!("sext {} %a{j} to i64", tc.llvm()),
                        CType::U8 | CType::U16 | CType::U32 | CType::Bool => format!("zext {} %a{j} to i64", tc.llvm()),
                        CType::I64 | CType::U64 => format!("add i64 %a{j}, 0"),
                        CType::F32 => {
                            writeln!(self.out, "  %of{j} = fpext float %a{j} to double").unwrap();
                            format!("bitcast double %of{j} to i64")
                        }
                        CType::F64 => format!("bitcast double %a{j} to i64"),
                        CType::Ptr | CType::Handle => format!("ptrtoint ptr %a{j} to i64"),
                        CType::Void => unreachable!("void native parameter"),
                    },
                };
                writeln!(self.out, "  %o{j} = {bits}\n  %g{j} = getelementptr [{n} x i64], ptr %buf, i64 0, i64 {j}\n  store i64 %o{j}, ptr %g{j}").unwrap();
            }
            // A foreign synchronous call shares only the raw C argument packing
            // with listeners. Runtime delivery executes Dart on the owner;
            // this thread never acquires Dart TLS, roots or exception state.
            writeln!(self.out, "  %is_sync_callback = icmp eq i64 %modo, 3\n  br i1 %is_sync_callback, label %foreign_sync, label %listener_post\nforeign_sync:\n  %sync_bits = call i64 @dartforge_ffi_callback_sync(ptr %ctx, ptr %buf, i64 {})", cb.params.len()).unwrap();
            match (&cb.ret, &ret) {
                (NativeType::Prim(CType::Void), _) => writeln!(self.out, "  ret void").unwrap(),
                (NativeType::Prim(native), _) => self.emit_callback_bits_return(*native, r, "%sync_bits", "sync"),
                // Runtime creation rejects aggregate returns for synchronous
                // callbacks. These defensive paths cannot execute for a valid
                // registration; local/listener aggregate ABIs remain intact.
                (_, Some((RetPassing::Sret { .. }, layout))) => {
                    writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %sret, i8 0, i64 {}, i1 false)\n  ret void", layout.size).unwrap();
                }
                (_, Some((RetPassing::Direct(_), _))) => writeln!(self.out, "  ret {r} zeroinitializer").unwrap(),
                _ => unreachable!("compound callback return without ABI passing"),
            }
            writeln!(self.out, "listener_post:\n  call void @dartforge_ffi_callback_post(ptr %ctx, ptr %buf, i64 {})", cb.params.len()).unwrap();
            match &cb.ret {
                NativeType::Prim(CType::Void) => writeln!(self.out, "  ret void").unwrap(),
                NativeType::Prim(CType::Ptr | CType::Handle) => writeln!(self.out, "  ret ptr null").unwrap(),
                NativeType::Prim(CType::F32 | CType::F64) => writeln!(self.out, "  ret {r} 0.0").unwrap(),
                NativeType::Prim(_) => writeln!(self.out, "  ret {r} 0").unwrap(),
                NativeType::Composite(_) if r == "void" => writeln!(self.out, "  ret void").unwrap(),
                NativeType::Composite(_) => writeln!(self.out, "  ret {r} zeroinitializer").unwrap(),
            }
            // Local: the call to the HIR body in the Dart representation.
            writeln!(self.out, "local:\n  %c = ptrtoint ptr %ctx to i64").unwrap();
            let mut args = vec!["i64 %c".to_string()];
            for (j, t) in cb.params.iter().enumerate() {
                let tc = match t {
                    NativeType::Composite(_) => {
                        args.push(format!("i64 %end{j}"));
                        continue;
                    }
                    NativeType::Prim(tc) => *tc,
                };
                let (conv_arg, ty) = match tc {
                    CType::I8 | CType::I16 | CType::I32 => (Some(format!("sext {} %a{j} to i64", tc.llvm())), "i64"),
                    CType::U8 | CType::U16 | CType::U32 => (Some(format!("zext {} %a{j} to i64", tc.llvm())), "i64"),
                    CType::F32 => (Some(format!("fpext float %a{j} to double")), "double"),
                    CType::Ptr | CType::Handle => (Some(format!("ptrtoint ptr %a{j} to i64")), "i64"),
                    CType::I64 | CType::U64 => (None, "i64"),
                    CType::F64 => (None, "double"),
                    CType::Bool => (None, "i1"),
                    CType::Void => unreachable!("void native parameter"),
                };
                match conv_arg {
                    Some(c) => {
                        writeln!(self.out, "  %d{j} = {c}").unwrap();
                        args.push(format!("{ty} %d{j}"));
                    }
                    None => args.push(format!("{ty} %a{j}")),
                }
            }
            let hr = cb.ret.hir_type().llvm_ir();
            let empty = matches!(cb.ret, NativeType::Prim(CType::Void));
            if by_tables {
                // The body that throws unwinds: the landing pad restores the top of the
                // shadow stack and goes on with the default value and the pending
                // exception, as the body returned in the check model.
                self.out.push_str(
                    "  %tctx = call ptr @dartforge_context()\n  %ttp = getelementptr inbounds i8, ptr %tctx, i64 8\n  %ttopo = load ptr, ptr %ttp, align 8\n",
                );
                let landing_pad = "tlp:\n  %tx = landingpad { ptr, i32 } catch ptr null\n  store ptr %ttopo, ptr %ttp, align 8\n  br label %tcont\n";
                if empty {
                    writeln!(self.out, "  invoke void @{}({}) to label %tcont unwind label %tlp", cb.body, args.join(", ")).unwrap();
                    self.out.push_str(landing_pad);
                    self.out.push_str("tcont:\n");
                } else {
                    let zero = match hr {
                        "double" => "0.0",
                        "i1" => "false",
                        _ => "0",
                    };
                    writeln!(self.out, "  %r.ok = invoke {hr} @{}({}) to label %tok unwind label %tlp", cb.body, args.join(", ")).unwrap();
                    self.out.push_str(landing_pad);
                    self.out.push_str("tok:\n  br label %tcont\ntcont:\n");
                    writeln!(self.out, "  %r = phi {hr} [ %r.ok, %tok ], [ {zero}, %tlp ]").unwrap();
                }
            } else if empty {
                writeln!(self.out, "  call void @{}({})", cb.body, args.join(", ")).unwrap();
            } else {
                writeln!(self.out, "  %r = call {hr} @{}({})", cb.body, args.join(", ")).unwrap();
            }
            // A returned struct: the bytes copied right away (before any
            // allocation); with an exception the body returns 0 and the return is zeroed.
            if let Some((_, l)) = &ret {
                let sz = l.size.div_ceil(16) * 16;
                writeln!(self.out, "  call void @llvm.memset.p0.i64(ptr %rt, i8 0, i64 {sz}, i1 false)").unwrap();
                writeln!(self.out, "  call void @dartforge_ffi_copy_compound(ptr %rt, i64 %r, i64 {})", l.size).unwrap();
            }
            writeln!(self.out, "  %x = call i8 @dartforge_ffi_callback_exit(ptr %ctx, ptr %saida)").unwrap();
            match (&cb.ret, &ret) {
                (NativeType::Prim(CType::Void), _) => {
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((RetPassing::Sret { .. }, l))) => {
                    writeln!(self.out, "  call void @llvm.memcpy.p0.p0.i64(ptr %sret, ptr %rt, i64 {}, i1 false)", l.size).unwrap();
                    writeln!(self.out, "  ret void\n}}\n").unwrap();
                    continue;
                }
                (_, Some((RetPassing::Direct(pieces), _))) => {
                    let mut current = "undef".to_string();
                    for (k, p) in pieces.iter().enumerate() {
                        writeln!(self.out, "  %rp{k} = getelementptr i8, ptr %rt, i64 {}", p.offset).unwrap();
                        writeln!(self.out, "  %rv{k} = load {}, ptr %rp{k}, align 1", p.ty).unwrap();
                        if pieces.len() == 1 {
                            current = format!("%rv{k}");
                        } else {
                            writeln!(self.out, "  %ra{k} = insertvalue {r} {current}, {} %rv{k}, {k}", p.ty).unwrap();
                            current = format!("%ra{k}");
                        }
                    }
                    writeln!(self.out, "  ret {r} {current}\n}}\n").unwrap();
                    continue;
                }
                _ => {}
            }
            let NativeType::Prim(tr) = cb.ret else { unreachable!("compound return without passing") };
            writeln!(self.out, "  %e_excecao = icmp ne i8 %x, 0\n  br i1 %e_excecao, label %excecao, label %normal").unwrap();
            // The exceptional return (bits of the C type) and the normal one (Dart → C).
            writeln!(self.out, "excecao:\n  %e = load i64, ptr %saida").unwrap();
            let exc = match tr {
                CType::I64 | CType::U64 => "add i64 %e, 0".to_string(),
                CType::F64 => "bitcast i64 %e to double".to_string(),
                CType::F32 => {
                    writeln!(self.out, "  %ed = bitcast i64 %e to double").unwrap();
                    "fptrunc double %ed to float".to_string()
                }
                CType::Ptr | CType::Handle => "inttoptr i64 %e to ptr".to_string(),
                narrow => format!("trunc i64 %e to {}", narrow.llvm()),
            };
            writeln!(self.out, "  %ev = {exc}\n  ret {r} %ev").unwrap();
            let normal = match tr {
                CType::I64 | CType::U64 | CType::F64 | CType::Bool => None,
                CType::F32 => Some("fptrunc double %r to float".to_string()),
                CType::Ptr | CType::Handle => Some("inttoptr i64 %r to ptr".to_string()),
                narrow => Some(format!("trunc i64 %r to {}", narrow.llvm())),
            };
            match normal {
                Some(c) => writeln!(self.out, "normal:\n  %nv = {c}\n  ret {r} %nv\n}}\n").unwrap(),
                None => writeln!(self.out, "normal:\n  ret {r} %r\n}}\n").unwrap(),
            }
        }
    }

    fn call_main(&mut self) {
        let Some(entry) = self.module.entry_symbol.clone() else { return };
        let by_tables = self.module.table_exceptions;
        let n = self.module.entry_params.min(2);
        if n == 0 {
            writeln!(self.out, "  {}", entry_call(by_tables, &entry, &[])).unwrap();
            return;
        }
        writeln!(self.out, "  %df.args = call i64 @dartforge_main_arguments()").unwrap();
        let args: &[&str] = if n == 1 { &["i64 %df.args"] } else { &["i64 %df.args", "i64 0"] };
        writeln!(self.out, "  {}", entry_call(by_tables, &entry, args)).unwrap();
    }

    /// Exceptions by tables: the ports through which the runtime calls Dart
    /// code (`runtime/src/exceptions_tables.rs`), one per signature —
    /// `@df.port.<v|r><n>(function, n words)`, `v` without a value and `r`
    /// returning a word — and the `@df.ports` table, at index
    /// `2n + (1 if it returns)`, which the program entry registers. The port
    /// calls the function with `invoke`; an unwinding lands in it, which
    /// restores the top of the shadow stack and returns with the exception pending — what
    /// the runtime expects from a Dart function that threw. This way no
    /// runtime frame stays in the path of an unwinding.
    fn emit_ports(&mut self) {
        let mut table = Vec::with_capacity(N_PORTS);
        for n in 0..N_PORTS / 2 {
            for returns in [false, true] {
                let name_str = format!("df.port.{}{n}", if returns { 'r' } else { 'v' });
                let params: String = (0..n).map(|i| format!(", i64 %a{i}")).collect();
                let args: Vec<String> = (0..n).map(|i| format!("i64 %a{i}")).collect();
                let (typ, result, back, pattern) =
                    if returns { ("i64", "%r = ", "ret i64 %r", "ret i64 0") } else { ("void", "", "ret void", "ret void") };
                writeln!(self.out, "define internal {typ} @{name_str}(ptr %f{params}) personality ptr @dartforge_personality {{")
                    .unwrap();
                self.out.push_str(
                    "entrada:\n  %ctx = call ptr @dartforge_context()\n  %tp = getelementptr inbounds i8, ptr %ctx, i64 8\n  %topo = load ptr, ptr %tp, align 8\n",
                );
                writeln!(self.out, "  {result}invoke {typ} %f({}) to label %volta unwind label %pouso", args.join(", ")).unwrap();
                writeln!(
                    self.out,
                    "volta:\n  {back}\npouso:\n  %lp = landingpad {{ ptr, i32 }} catch ptr null\n  store ptr %topo, ptr %tp, align 8\n  {pattern}\n}}"
                )
                .unwrap();
                table.push(format!("ptr @{name_str}"));
            }
        }
        writeln!(self.out, "@df.ports = private unnamed_addr constant [{N_PORTS} x ptr] [{}]\n", table.join(", ")).unwrap();
    }

    /// Type of the value that the emitter actually prints for an instruction.
    ///
    /// It is not enough to trust the type recorded in the HIR: several arms print a
    /// fixed type (every Add comes out as i64, every ICmp as i1) and a divergent
    /// record would make the coercion work with wrong information. The recorded
    /// type only holds where the emitter uses it (Load, Phi, Alloca and the
    /// instructions not expanded yet).
    pub(crate) fn result_type(inst: &Instruction, registered: Type) -> Type {
        match inst {
            Instruction::Const(Constant::Bool(_)) => Type::I1,
            Instruction::Const(Constant::Double(_)) => Type::F64,
            Instruction::Const(Constant::Int(_)) => Type::I64,
            Instruction::Const(_) => Type::Ref,
            Instruction::Add(..)
            | Instruction::Sub(..)
            | Instruction::Mul(..)
            | Instruction::SDiv(..)
            | Instruction::SRem(..)
            | Instruction::Shl(..)
            | Instruction::AShr(..)
            | Instruction::And(..)
            | Instruction::Or(..)
            | Instruction::Xor(..)
            | Instruction::Neg(..)
            | Instruction::Not(..)
            | Instruction::LShr(..)
            | Instruction::DoubleToInt(..) => Type::I64,
            Instruction::ArcLoadStrong { .. }
            | Instruction::ArcCopy { .. }
            | Instruction::ArcMove { .. }
            | Instruction::AllocObject { .. }
            | Instruction::AllocList { .. }
            | Instruction::AllocRecord { .. }
            | Instruction::Box { .. }
            | Instruction::AllocCell { .. }
            | Instruction::AllocEnv { .. }
            | Instruction::ConcatStrings { .. }
            | Instruction::AllocClosure { .. }
            | Instruction::AllocClosureTyped { .. }
            | Instruction::TearOff { .. }
            | Instruction::CallClosure { .. }
            | Instruction::CallSelector { .. }
            | Instruction::CallSelectorForward { .. }
            | Instruction::CallClosureForward { .. } => Type::Ref,
            Instruction::NativeCall { ret, .. } => ret.hir_type(),
            Instruction::TypedCall { ret, .. } => *ret,
            Instruction::NativeLoad { ty: typ, .. } => typ.hir_type(),
            Instruction::NativeWrite { .. } => Type::Void,
            Instruction::CompositeNativeCall { ret, dest, .. } => {
                if dest.is_some() { Type::Void } else { ret.hir_type() }
            }
            Instruction::CellSet { .. } | Instruction::ArcDrop { .. } | Instruction::ArcStoreStrong { .. } => Type::Void,
            Instruction::ConstArray(_) | Instruction::FunctionTable(_) => Type::Ptr,
            Instruction::Unbox { to, .. } => *to,
            Instruction::Alloca(_) => Type::Ptr,
            Instruction::GetField { .. } => registered,
            Instruction::FAdd(..)
            | Instruction::FSub(..)
            | Instruction::FMul(..)
            | Instruction::FDiv(..)
            | Instruction::FNeg(..)
            | Instruction::IntToDouble(..) => Type::F64,
            Instruction::ICmp(..) | Instruction::FCmp(..) | Instruction::LNot(..) => Type::I1,
            Instruction::ZExt { to, .. } | Instruction::Trunc { to, .. } | Instruction::Bitcast { to, .. } => *to,
            Instruction::LoadGlobal { ty, .. } => *ty,
            Instruction::CallStatic { ret_ty, .. } | Instruction::CallRuntime { ret_ty, .. } | Instruction::DeclaredFieldCall { ret_ty, .. } => *ret_ty,
            Instruction::Load { ty, .. } => *ty,
            Instruction::Phi { ty, .. } => *ty,
            _ => registered,
        }
    }


    /// The stack check in the prologue (the VM's `stack_overflow_check`,
    /// `runtime/vm/compiler/backend/il.h:9693`, `CheckStackOverflowInstr`): the
    /// stack pointer at the entry of the function (`llvm.read_register` of
    /// `sp`/`rsp`, the VM's `cmp sp, [thr + stack_limit]`) below
    /// `Context::stack_limit` throws `StackOverflowError`
    /// (`dartforge_stack_overflow`) and the function returns with the exception
    /// pending, before chaining the roots frame. Only in the functions that
    /// call (those that have `%ctx`): a leaf does not deepen the stack. Before,
    /// the address of a marker `alloca` stood for the stack pointer: 16 bytes of
    /// frame per activation (`fib`: 0x40 → 0x30, which then fits the pre-indexed
    /// `stp`/`ldp`). The register is read before the `sub sp` of the function:
    /// the slack of `STACK_SLACK` (`runtime/src/gc_roots.rs`, 256 KiB) covers the
    /// frame of the function itself. `llvm.sponentry` would do the same but
    /// crashes the x86-64 Windows backend of LLVM 22.
    fn emit_stack_check(&mut self, func: &Function) {
        let by_tables = self.tab.is_some();
        let o = &mut self.out;
        writeln!(o, "  %pilhalp = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::context::STACK_LIMIT).unwrap();
        writeln!(o, "  %pilhal = load i64, ptr %pilhalp, align 8").unwrap();
        writeln!(o, "  %pilhasp = call i64 @llvm.read_register.i64(metadata !{{!\"{}\"}})", stack_pointer()).unwrap();
        writeln!(o, "  %pilhab = icmp ult i64 %pilhasp, %pilhal").unwrap();
        writeln!(o, "  %pilhax = call i1 @llvm.expect.i1(i1 %pilhab, i1 false)").unwrap();
        writeln!(o, "  br i1 %pilhax, label %pilha.estouro, label %pilha.ok").unwrap();
        writeln!(o, "pilha.estouro:").unwrap();
        writeln!(o, "  call void @dartforge_stack_overflow()").unwrap();
        if self.stack_trace.is_some() {
            writeln!(o, "  call void @dartforge_call_trace_exit()").unwrap();
        }
        // Exceptions by tables: the `StackOverflowError` unwinds from here.
        if by_tables {
            writeln!(o, "  call void @df.throw()").unwrap();
            writeln!(o, "  unreachable").unwrap();
            writeln!(o, "pilha.ok:").unwrap();
            self.current_label = "pilha.ok".to_string();
            self.exit_labels.entry(0).or_insert_with(|| "pilha.ok".to_string());
            return;
        }
        match func.return_ty {
            Type::Void => writeln!(o, "  ret void").unwrap(),
            Type::F64 => writeln!(o, "  ret double 0.0").unwrap(),
            Type::I1 => writeln!(o, "  ret i1 false").unwrap(),
            Type::Ptr => writeln!(o, "  ret ptr null").unwrap(),
            Type::I64 | Type::Ref => writeln!(o, "  ret i64 0").unwrap(),
            Type::I8 => writeln!(o, "  ret i8 0").unwrap(),
            t => writeln!(o, "  ret {} zeroinitializer", t.llvm_ir()).unwrap(),
        }
        writeln!(o, "pilha.ok:").unwrap();
        self.current_label = "pilha.ok".to_string();
        self.exit_labels.entry(0).or_insert_with(|| "pilha.ok".to_string());
    }

    /// The body of the object `only` (`%fcb{v}`, the header) and its fields
    /// (`%fp{v}`), inline: an object of the object space (`h & 3 == 2` and
    /// `h > 0`; `crates/runtime/src/heap.rs`, `Header`) has the header at
    /// `h - 2`; anything else — null, a `Smi`, a runtime value — uses
    /// `EMPTY_OBJECT` (the pointer in `Context::empties`, offset 40 of the
    /// context), zeroed fields, as `dartforge_object_fields` gave. With the
    /// `OUT_OF_LINE` bit in the `flags`, the body is the address kept in the first
    /// field (the object that changed its number of fields): a select, without a
    /// branch. No call. Without outside bodies (`body_without_outside`: AOT and
    /// the production SDK — an outside body is only born in the instance
    /// migration of a JIT reload, `Heap::swap_fields`; `ensure_fields` never
    /// grows an object there, `exceptions.rs` only calls it for a field that
    /// exists) the flags test and the second select are not emitted: the body is
    /// the first select.
    /// The operand is an object freshly allocated in this block, without a collection
    /// point since the allocation (`fresh_set`).
    fn fresh(&self, op: &Operand) -> bool {
        !self.module.arc_memory && matches!(op, Operand::Val(x) if self.fresh_set.contains(&x.0))
    }

    /// The body of the object `op` is the block itself: the freshly allocated one, or
    /// `this` when no object has an outside body (`this_object`).
    fn direct_body(&self, op: &Operand) -> bool {
        self.fresh(op) || matches!(op, Operand::Val(x) if self.this_object == Some(x.0))
    }

    /// [`Self::emit_field_addresses`]; `direct`: the body is the block
    /// itself ([`Self::direct_body`], `handle − 2`), without the select of
    /// null/`Smi` or the outside body.
    fn emit_field_addresses_of(&mut self, v: u32, only: &str, direct: bool) {
        if !direct {
            return self.emit_field_addresses(v, only);
        }
        let o = &mut self.out;
        writeln!(o, "  %fxa{v} = add i64 {only}, -{}", layout::HANDLE_OFFSET).unwrap();
        writeln!(o, "  %fcb{v} = inttoptr i64 %fxa{v} to ptr").unwrap();
        writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
    }

    fn emit_field_addresses(&mut self, v: u32, only: &str) {
        let o = &mut self.out;
        if self.fields_per_call {
            writeln!(o, "  %fcb{v} = call ptr @df.body(i64 {only}, ptr %ctx)").unwrap();
            writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
            return;
        }
        // The sign bit enters the mask: a negative is never an object.
        writeln!(o, "  %fxk{v} = and i64 {only}, {OBJECT_MASK}").unwrap();
        writeln!(o, "  %fxo{v} = icmp eq i64 %fxk{v}, 2").unwrap();
        writeln!(o, "  %fxa{v} = add i64 {only}, -2").unwrap();
        writeln!(o, "  %fxq{v} = inttoptr i64 %fxa{v} to ptr").unwrap();
        writeln!(o, "  %fxvp{v} = getelementptr inbounds i8, ptr %ctx, i64 40").unwrap();
        writeln!(o, "  %fxv{v} = load ptr, ptr %fxvp{v}, align 8").unwrap();
        if self.body_without_outside {
            writeln!(o, "  %fcb{v} = select i1 %fxo{v}, ptr %fxq{v}, ptr %fxv{v}").unwrap();
            writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
            return;
        }
        writeln!(o, "  %fxh{v} = select i1 %fxo{v}, ptr %fxq{v}, ptr %fxv{v}").unwrap();
        writeln!(o, "  %fxfp{v} = getelementptr inbounds i8, ptr %fxh{v}, i64 1").unwrap();
        writeln!(o, "  %fxf{v} = load i8, ptr %fxfp{v}, align 1").unwrap();
        writeln!(o, "  %fxfb{v} = and i8 %fxf{v}, 1").unwrap();
        writeln!(o, "  %fxfo{v} = icmp ne i8 %fxfb{v}, 0").unwrap();
        writeln!(o, "  %fxcp{v} = getelementptr inbounds i8, ptr %fxh{v}, i64 16").unwrap();
        writeln!(o, "  %fxc{v} = load ptr, ptr %fxcp{v}, align 8").unwrap();
        writeln!(o, "  %fcb{v} = select i1 %fxfo{v}, ptr %fxc{v}, ptr %fxh{v}").unwrap();
        writeln!(o, "  %fp{v} = getelementptr inbounds i8, ptr %fcb{v}, i64 16").unwrap();
    }

    /// Writes `sv` into the field `idx` of the body `%fcb{v}` and sets or clears its
    /// bit in the reference map (`is_ref`: `0`, `1` or an `i8` only
    /// known at runtime) — the first 32 in the header, the others in the
    /// words after the fields (found through the `n` of the body). `known`: the
    /// static map the allocation of the (fresh) object wrote
    /// ([`Self::static_map`]); a constant `is_ref` equal to its bit leaves the
    /// map alone (the read-modify-write stays for the other fields and for a
    /// bit that disagrees, which it corrects as always). `fresh`: the object was
    /// allocated in this block with no collection point since
    /// ([`Self::fresh`]), so its fields are still the zeros of the handout
    /// (contract E8, `Heap::take_from_region`; the runtime allocations too):
    /// the store of the constant `0` (null, `0`, `false`) is not emitted.
    fn emit_field_write(&mut self, v: u32, suffix: &str, idx: usize, sv: &str, is_ref: &str, facts: FieldWriteFacts) {
        let FieldWriteFacts { known_map: known, fresh } = facts;
        let o = &mut self.out;
        let t = format!("{v}{suffix}");
        if !(fresh && sv == "0") {
            writeln!(o, "  %fg{t} = getelementptr inbounds i64, ptr %fp{v}, i64 {idx}").unwrap();
            writeln!(o, "  store i64 {sv}, ptr %fg{t}, align 8").unwrap();
        }
        let constant = match is_ref {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        };
        if let (Some(map), Some(is_reference)) = (known.filter(|_| idx < 32), constant)
            && (map >> idx & 1 == 1) == is_reference
        {
            return;
        }
        let (word, width, bit) = if idx < 32 {
            writeln!(o, "  %fmp{t} = getelementptr inbounds i8, ptr %fcb{v}, i64 8").unwrap();
            (format!("%fmp{t}"), "i32", idx)
        } else {
            writeln!(o, "  %fnp{t} = getelementptr inbounds i8, ptr %fcb{v}, i64 2").unwrap();
            writeln!(o, "  %fn{t} = load i16, ptr %fnp{t}, align 2").unwrap();
            writeln!(o, "  %fnz{t} = zext i16 %fn{t} to i64").unwrap();
            writeln!(o, "  %fnw{t} = add i64 %fnz{t}, {}", (idx - 32) / 64).unwrap();
            writeln!(o, "  %fmp{t} = getelementptr inbounds i64, ptr %fp{v}, i64 %fnw{t}").unwrap();
            (format!("%fmp{t}"), "i64", (idx - 32) % 64)
        };
        // The constants in the width of the word, signed (the IR text).
        let (cleans, lights_up) = if width == "i32" {
            (i64::from(!(1u32 << bit) as i32), i64::from((1u32 << bit) as i32))
        } else {
            (!(1u64 << bit) as i64, (1u64 << bit) as i64)
        };
        writeln!(o, "  %fm{t} = load {width}, ptr {word}, align 4").unwrap();
        writeln!(o, "  %fmc{t} = and {width} %fm{t}, {cleans}").unwrap();
        match is_ref {
            "0" => writeln!(o, "  store {width} %fmc{t}, ptr {word}, align 4").unwrap(),
            "1" => {
                writeln!(o, "  %fms{t} = or {width} %fm{t}, {lights_up}").unwrap();
                writeln!(o, "  store {width} %fms{t}, ptr {word}, align 4").unwrap();
            }
            r => {
                writeln!(o, "  %fmz{t} = zext i8 {r} to {width}").unwrap();
                writeln!(o, "  %fmb{t} = shl {width} %fmz{t}, {bit}").unwrap();
                writeln!(o, "  %fms{t} = or {width} %fmc{t}, %fmb{t}").unwrap();
                writeln!(o, "  store {width} %fms{t}, ptr {word}, align 4").unwrap();
            }
        }
    }

    /// `dartforge_object_new(c, n)` with a constant class and number of fields
    /// and `n ≤ TLAB_N`: the allocation comes out inline
    /// ([`LlvmEmitter::emit_inline_allocation`]). Returns `(c, n)`.
    fn inline_allocation(inst: &Instruction) -> Option<(i64, i64)> {
        let (Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }) = inst else { return None };
        if name != "dartforge_object_new" || args.len() != 2 {
            return None;
        }
        match (&args[0].0, &args[1].0) {
            (Operand::Constant(Constant::Int(c)), Operand::Constant(Constant::Int(n))) if (0..=TLAB_N).contains(n) && (0..=i64::from(i32::MAX)).contains(c) => {
                Some((*c, *n))
            }
            _ => None,
        }
    }

    /// Level A of the static reference map: production without ARC. The
    /// allocation of a class whose layout the module declares writes the map of
    /// the first 32 fields ([`Self::static_map`]), and the stores into the fresh
    /// object skip the read-modify-write of the bits that agree. Development and
    /// the JIT keep the map by store (the layout of a class changes on reload).
    fn static_maps(&self) -> bool {
        self.production && !self.module.arc_memory
    }

    /// The reference map of the first 32 fields of an instance of class `c`
    /// with `n` fields, from the declared representations of the module
    /// (`declared_field_layouts`, the same `representation` the stores of the
    /// lowering follow — contract E1): bit `i` set when field `i` holds a `Ref`.
    /// `None` when the module does not declare the layout (an SDK class
    /// allocated from the program module), when a slot is synthetic or unknown
    /// (the enum prefix) or when the layout does not have exactly `n` slots:
    /// then the map stays 0 and every store keeps the read-modify-write, as
    /// before. A set bit over a field still null (0) is what the collector
    /// already sees in the objects the runtime creates (`Heap::new_object`):
    /// null is not a handle.
    fn static_map(&self, c: i64, n: i64) -> Option<u32> {
        if !self.static_maps() {
            return None;
        }
        let layout = self.module.declared_field_layouts.get(&(c as u32))?;
        if layout.len() != usize::try_from(n).ok()? {
            return None;
        }
        let mut map = 0u32;
        for (i, slot) in layout.iter().enumerate().take(32) {
            let declaration = slot.as_ref()?;
            if declaration.layout_index != i {
                return None;
            }
            if declaration.storage_type == Type::Ref {
                map |= 1 << i;
            }
        }
        Some(map)
    }

    /// The static map written by the allocation of the fresh object `op`
    /// ([`Self::fresh`], [`Self::static_map`]), if any.
    fn fresh_map(&self, op: &Operand) -> Option<u32> {
        match op {
            Operand::Val(x) if self.fresh(op) => self.fresh_maps.get(&x.0).copied(),
            _ => None,
        }
    }

    /// Is the method table of class `c` registered before any Dart code of the
    /// isolate? In production with [`Self::tables_registered_at_startup`]: every
    /// SDK table (`@df.register.sdk_tables`) and every program table of
    /// `df.register.program` — all of them, unless the experimental closed set
    /// of active program tables (`closed_class_tables`) leaves one out, which
    /// then keeps the lazy registration of `dartforge_object_new_t`.
    fn registered_at_startup(&self, c: i64) -> bool {
        if !self.tables_registered_at_startup {
            return false;
        }
        match &self.module.closed_class_tables {
            Some(active) if self.module.method_tables.iter().any(|(cid, _, _)| i64::from(*cid) == c) => {
                active.contains(&(c as u32))
            }
            _ => true,
        }
    }

    /// The inline allocation of an object of class `c` and `n` fields, like the
    /// VM's `TryAllocateObject` (`stub_code_compiler.cc`): advances the cursor
    /// of the TLAB range of `n` fields of the isolate (`Context::tlab`: cursor
    /// at `64 + 16n`, end at `72 + 16n`; the blocks have already been counted as
    /// allocated and come zeroed, header and fields — `Heap::take_from_region`
    /// zeroes a recycled range on handout, a new page is zeroed memory),
    /// writes the header (young, `n`, the class) and, when the module declares
    /// the layout of the class, the reference map ([`Self::static_map`]), and
    /// returns the handle (`block + 2`). An exhausted range — or, in development,
    /// for the class that registers the method table on the first allocation
    /// (`dartforge_object_new_t`), a class not registered yet
    /// ([`Self::registered_at_startup`]) — goes to the runtime, which collects
    /// if needed, refills and writes the same map (`dartforge_object_new_m`,
    /// `_mt`). The HIR block ends at the label `ao{v}.fim`
    /// ([`LlvmEmitter::exit_labels`]).
    fn emit_inline_allocation(&mut self, v: u32, c: i64, n: i64) {
        let table = self.module.table_functions.get(&(c as u32)).cloned();
        if let Some(f) = &table {
            self.annotate_external(f, Type::Ptr, &[]);
        }
        let test_registration = table.is_some() && !self.registered_at_startup(c);
        let map = self.static_map(c, n);
        if let Some(m) = map {
            self.fresh_maps.insert(v, m);
        }
        // A map 0 is what the zeroed block already has.
        let written_map = map.filter(|m| *m != 0);
        // The TLAB is by words of the body: `n ≤ TLAB_N` fields take
        // `max(n, 1)` words (no map extension).
        let w = layout::instance_words(n as usize);
        let size = layout::block_bytes(w);
        let header = layout::header_word(layout::state::YOUNG, layout::flags::INSTANCE, n as usize, c as i32);
        if self.out_of_line_allocation {
            // Production of the large program: the same computation, in a call to
            // `@df.new_instance` (docs/NATIVO-PRODUCAO-GRANDE.md §3).
            let t = table.as_ref().map_or("null".to_string(), |f| format!("@{f}"));
            writeln!(
                self.out,
                "  %v{v} = call i64 @df.new_instance(ptr %ctx, i64 {}, i64 {size}, i64 {}, i64 {c}, i64 {n}, ptr {t}, i64 {})",
                layout::context::tlab_cursor(w),
                header as i64,
                written_map.unwrap_or(0)
            )
            .unwrap();
            return;
        }
        let o = &mut self.out;
        writeln!(o, "  %ta{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::context::tlab_cursor(w)).unwrap();
        writeln!(o, "  %tb{v} = load ptr, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  %tfa{v} = getelementptr inbounds i8, ptr %ctx, i64 {}", layout::context::tlab_end(w)).unwrap();
        writeln!(o, "  %tf{v} = load ptr, ptr %tfa{v}, align 8").unwrap();
        writeln!(o, "  %tnx{v} = getelementptr i8, ptr %tb{v}, i64 {size}").unwrap();
        writeln!(o, "  %tz{v} = icmp ugt ptr %tnx{v}, %tf{v}").unwrap();
        if test_registration {
            writeln!(o, "  %tnp{v} = getelementptr inbounds i8, ptr %ctx, i64 56").unwrap();
            writeln!(o, "  %tn{v} = load i64, ptr %tnp{v}, align 8").unwrap();
            writeln!(o, "  %tk{v} = icmp ule i64 %tn{v}, {c}").unwrap();
            writeln!(o, "  %tq{v} = or i1 %tz{v}, %tk{v}").unwrap();
            writeln!(o, "  %tx{v} = call i1 @llvm.expect.i1(i1 %tq{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %tx{v}, label %ao{v}.lento, label %ao{v}.reg").unwrap();
            writeln!(o, "ao{v}.reg:").unwrap();
            writeln!(o, "  %trp{v} = getelementptr inbounds i8, ptr %ctx, i64 48").unwrap();
            writeln!(o, "  %tr{v} = load ptr, ptr %trp{v}, align 8").unwrap();
            writeln!(o, "  %trb{v} = getelementptr inbounds i8, ptr %tr{v}, i64 {c}").unwrap();
            writeln!(o, "  %trv{v} = load i8, ptr %trb{v}, align 1").unwrap();
            writeln!(o, "  %trz{v} = icmp eq i8 %trv{v}, 0").unwrap();
            writeln!(o, "  %try{v} = call i1 @llvm.expect.i1(i1 %trz{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %try{v}, label %ao{v}.lento, label %ao{v}.rapido").unwrap();
        } else {
            writeln!(o, "  %tx{v} = call i1 @llvm.expect.i1(i1 %tz{v}, i1 false)").unwrap();
            writeln!(o, "  br i1 %tx{v}, label %ao{v}.lento, label %ao{v}.rapido").unwrap();
        }
        // The free block comes zeroed (header and fields; the free list is a
        // list of ranges, nothing is chained through the fields): the cursor is
        // advanced and the header word is written (young state, no flags, `n`,
        // the class), plus the static map when it is not 0.
        writeln!(o, "ao{v}.rapido:").unwrap();
        writeln!(o, "  store ptr %tnx{v}, ptr %ta{v}, align 8").unwrap();
        writeln!(o, "  store i64 {}, ptr %tb{v}, align 8", header as i64).unwrap();
        if let Some(m) = written_map {
            writeln!(o, "  %tmp{v} = getelementptr inbounds i8, ptr %tb{v}, i64 {}", layout::offsets::MAP).unwrap();
            writeln!(o, "  store i32 {}, ptr %tmp{v}, align 4", m as i32).unwrap();
        }
        writeln!(o, "  %thb{v} = ptrtoint ptr %tb{v} to i64").unwrap();
        writeln!(o, "  %th{v} = add i64 %thb{v}, 2").unwrap();
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.lento:").unwrap();
        match (&table, written_map) {
            (Some(f), Some(m)) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new_mt(i64 {c}, i64 {n}, i64 {m}, ptr @{f})").unwrap(),
            (Some(f), None) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new_t(i64 {c}, i64 {n}, ptr @{f})").unwrap(),
            (None, Some(m)) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new_m(i64 {c}, i64 {n}, i64 {m})").unwrap(),
            (None, None) => writeln!(o, "  %tl{v} = call i64 @dartforge_object_new(i64 {c}, i64 {n})").unwrap(),
        }
        writeln!(o, "  br label %ao{v}.fim").unwrap();
        writeln!(o, "ao{v}.fim:").unwrap();
        writeln!(o, "  %v{v} = phi i64 [ %th{v}, %ao{v}.rapido ], [ %tl{v}, %ao{v}.lento ]").unwrap();
        self.current_label = format!("ao{v}.fim");
    }

    /// The inline field store that may put a `Ref` into an object: it carries the
    /// write barrier ([`LlvmEmitter::emit_barrier`]). A scalar constant
    /// or null does not need it.
    fn inline_barrier(&self, inst: &Instruction) -> bool {
        // Under ARC the store goes to the runtime, with the barrier inside.
        if self.module.arc_memory {
            return false;
        }
        let may_be_ref = |op: &Operand| {
            !matches!(op, Operand::Constant(Constant::Null | Constant::Int(_) | Constant::Bool(_) | Constant::Double(_)))
        };
        match inst {
            Instruction::SetField { index, value, .. } => {
                *index < INLINE_FIELDS && self.type_of(value) == Type::Ref && may_be_ref(value)
            }
            Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
                if name == "dartforge_object_set"
                    && args.len() == 4
                    && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
            {
                let scalar = matches!(&args[3].0, Operand::Constant(Constant::Int(0)) | Operand::Constant(Constant::Bool(false)));
                !scalar && may_be_ref(&args[2].0)
            }
            _ => false,
        }
    }

    /// The write barrier after storing the `Ref` `sv` into the object `only`
    /// (`@df.barrier`, docs/NATIVO-ESPACO-UNIFICADO.md §2.7): the old object
    /// that receives a young child goes into the remembered set of the next minor collection —
    /// the *store buffer* of the VM's barrier. `dynamic`: `is_ref` only known at
    /// runtime (0/1, `i8`). The HIR block now ends at `wb{v}.fim`.
    fn emit_barrier(&mut self, v: u32, only: &str, sv: &str, dynamic: Option<&str>) {
        let o = &mut self.out;
        match dynamic {
            Some(r) => {
                writeln!(o, "  %wbr{v} = icmp ne i8 {r}, 0").unwrap();
                writeln!(o, "  br i1 %wbr{v}, label %wb{v}.ref, label %wb{v}.fim").unwrap();
                writeln!(o, "wb{v}.ref:").unwrap();
                writeln!(o, "  call void @df.barrier(i64 {only}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
            None => {
                writeln!(o, "  call void @df.barrier(i64 {only}, i64 {sv})").unwrap();
                writeln!(o, "  br label %wb{v}.fim").unwrap();
            }
        }
        writeln!(o, "wb{v}.fim:").unwrap();
        self.current_label = format!("wb{v}.fim");
    }

    /// The store into a fresh object needs no barrier (it is young), but the
    /// HIR block still ends at `wb{v}.fim` ([`LlvmEmitter::exit_labels`]
    /// decides the label before knowing which objects are fresh).
    fn emit_barrier_label(&mut self, v: u32) {
        writeln!(self.out, "  br label %wb{v}.fim\nwb{v}.fim:").unwrap();
        self.current_label = format!("wb{v}.fim");
    }

    /// Context required by the body or by the tables, before deciding the
    /// roots frames. It also requires a stack check in the prologue;
    /// the ARC summaries share this decision so as not to omit unwinding.
    /// Frames computed by liveness may require additional context.
    pub(crate) fn requires_explicit_context(func: &Function, tab: Option<&FunctionTables>) -> bool {
        func.blocks.iter().any(|b| {
            b.instructions.iter().any(|(_, i, _)| {
                matches!(i, Instruction::CallRuntime { name, .. } | Instruction::DeclaredFieldCall { name, .. } if name == "dartforge_exception_pending")
                    || Self::uses_context(i)
            })
        }) || tab.is_some_and(|t| {
            !t.landing_pads.is_empty() || t.checks_stack
                || t.exits.values().any(|s| *s == ExceptionExit::Guard)
        })
    }

    /// The instruction reads the thread context (`%ctx`): inline fields or
    /// inline allocation.
    fn uses_context(inst: &Instruction) -> bool {
        match inst {
            Instruction::GetField { index, .. } | Instruction::SetField { index, .. } => *index < INLINE_FIELDS,
            Instruction::AllocObject { fields, .. } => !fields.is_empty(),
            Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } => {
                Self::inline_allocation(inst).is_some()
                    || name == "dartforge_exception_clear"
                    || ((name == "dartforge_object_get" || (name == "dartforge_object_set" && args.len() == 4))
                        && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)))
            }
            _ => false,
        }
    }
    /// Static type of an operand inside the current function.
    /// Can the instruction allocate (and therefore collect)? Conservative: only what
    /// provably emits only arithmetic, local memory or an extern that does not allocate
    /// does not collect — and without a conversion that boxes (a `Ref` operand
    /// where a scalar is expected may throw `TypeError`, which allocates; a
    /// scalar where a `Ref` is expected, or a text constant, allocates).
    fn may_collect(&self, inst: &Instruction) -> bool {
        let scalar = |op: &Operand| {
            self.type_of(op) != Type::Ref && !matches!(op, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
        };
        let exact = |op: &Operand, t: Type| match op {
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => false,
            Operand::Constant(Constant::Null) => t == Type::Ref,
            _ => {
                let a = self.type_of(op);
                a == t || (a != Type::Ref && t != Type::Ref)
            }
        };
        match inst {
            Instruction::Const(Constant::Int(_) | Constant::Double(_) | Constant::Bool(_) | Constant::Null) => false,
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
            | Instruction::FCmp(_, a, b) => !(scalar(a) && scalar(b)),
            Instruction::Neg(a)
            | Instruction::Not(a)
            | Instruction::FNeg(a)
            | Instruction::LNot(a)
            | Instruction::IntToDouble(a)
            | Instruction::DoubleToInt(a) => !scalar(a),
            Instruction::Alloca(_) | Instruction::Load { .. } | Instruction::Phi { .. } => false,
            // Vector arithmetic and unboxing (a direct load): they do not allocate.
            Instruction::Simd { .. } => false,
            Instruction::Unbox { to, .. } if to.is_vector() => false,
            // Inline (load/store in the field vector), without a conversion that
            // allocates: a `Ref` is already an `i64`, a `double` is a `bitcast`.
            Instruction::GetField { index, .. } => *index >= INLINE_FIELDS,
            Instruction::SetField { index, value, .. } => {
                *index >= INLINE_FIELDS
                    || matches!(value, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)))
            }
            Instruction::Store { ptr, val } => {
                let t = match ptr {
                    Operand::Val(p) => self.pointee.get(p).copied().unwrap_or(Type::I64),
                    _ => return true,
                };
                !exact(val, t)
            }
            // The field read and write with a constant inline index come out
            // inline like `GetField`/`SetField` (only loads, stores and the
            // barrier; the operands go through `coerce`, which never boxes):
            // the object freshly allocated before them stays fresh — direct
            // body, no barrier — and no root is kept across them.
            Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. }
                if !self.module.arc_memory
                    && (name == "dartforge_object_get" || (name == "dartforge_object_set" && args.len() == 4))
                    && matches!(args.get(1), Some((Operand::Constant(Constant::Int(i)), _)) if (0..INLINE_FIELDS as i64).contains(i)) =>
            {
                args.iter().any(|(a, _)| matches!(a, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))))
            }
            Instruction::NativeLoad { address, index: idx_value, .. } => !(scalar(address) && scalar(idx_value)),
            Instruction::NativeWrite { address, index: idx_value, value: val_ref, .. } => {
                !(scalar(address) && scalar(idx_value) && scalar(val_ref))
            }
            Instruction::CallRuntime { name, args, .. } | Instruction::DeclaredFieldCall { name, args, .. } => {
                let e = externs::effects_of(name);
                let does_not_allocate = !e.allocates && !e.calls_dart;
                !(does_not_allocate && args.iter().all(|(a, t)| exact(a, *t)))
            }
            // In tracing memory: the constant text is a static object
            // of the image (`ir_strings.rs`, `emit_const_string`), and
            // the constant vectors and the read of an argument vector are
            // addresses and loads. None allocates; before, each text literal
            // forced rooting everything that was live at it.
            Instruction::Const(Constant::String(_) | Constant::StringWtf8(_)) => {
                !(self.static_objects && !self.module.arc_memory)
            }
            Instruction::LoadIndexed { .. } | Instruction::ConstArray(_) | Instruction::FunctionTable(_) => {
                self.module.arc_memory
            }
            _ => true,
        }
    }

    /// Whether the call `inst` roots its own arguments
    /// (`roots::analyze_with`): a direct call to a Dart function — of this
    /// module, or of the source SDK (`df.dart$3a…`) — whose operands arrive
    /// without a conversion that allocates (the type of the parameter, no constant
    /// text). Only on the shadow stack of tracing memory: roots
    /// by maps and ARC follow the usual rule. Only with
    /// `DARTFORGE_ROOTS_IN_CALLEE=1` ([`roots_in_callee`]).
    fn callee_roots_the_arguments(&self, inst: &Instruction) -> bool {
        if self.maps || self.module.arc_memory || !roots_in_callee() {
            return false;
        }
        let Instruction::CallStatic { symbol, args, .. } = inst else { return false };
        let target = self.function_by_symbol.get(symbol.as_str()).copied();
        if target.is_none() && !symbol.starts_with("df.dart$3a") {
            return false;
        }
        args.iter().enumerate().all(|(i, a)| {
            if matches!(a, Operand::Constant(Constant::String(_) | Constant::StringWtf8(_))) {
                return false;
            }
            let t = self.type_of(a);
            match target.and_then(|f| f.params.get(i)).map(|p| p.2) {
                Some(p) => p == t || (p != Type::Ref && t != Type::Ref && p != Type::Void),
                None => true,
            }
        })
    }

    fn type_of(&self, op: &Operand) -> Type {
        match op {
            Operand::Val(v) => self.types.get(v).copied().unwrap_or(Type::I64),
            Operand::Constant(Constant::Int(_)) => Type::I64,
            Operand::Constant(Constant::Double(_)) => Type::F64,
            Operand::Constant(Constant::Bool(_)) => Type::I1,
            Operand::Constant(Constant::Null) => Type::Ref,
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => Type::Ref,
            Operand::Constant(Constant::Function(_)) => Type::I64,
        }
    }

    fn width(t: Type) -> u32 {
        match t {
            Type::I1 => 1,
            Type::I8 => 8,
            _ => 64,
        }
    }

    /// Double literal in LLVM's hexadecimal form.
    ///
    /// format!("{d}") prints 1 for 1.0 and LLVM rejects "double 1"; the
    /// 0x form with the 16 digits of the IEEE 754 pattern always works.
    fn double_literal(d: f64) -> String {
        format!("0x{:016X}", d.to_bits())
    }

    /// Rewrites a constant directly in the requested type, without an instruction.
    /// Returns None when the operand is not a constant.
    fn constant_in_type(&self, op: &Operand, target: Type) -> Option<String> {
        let Operand::Constant(c) = op else { return None };
        let s = match (c, target) {
            (Constant::String(_) | Constant::StringWtf8(_), _) => panic!("string must be loaded via Instruction::Const"),
            (Constant::Function(f), Type::I64) => format!("ptrtoint (ptr @{f} to i64)"),
            (Constant::Function(_), _) => panic!("function address only as i64"),
            (Constant::Int(n), Type::I1) => if *n != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Bool(b), Type::I1) => if *b { "true".to_string() } else { "false".to_string() },
            (Constant::Null, Type::I1) => "false".to_string(),
            (Constant::Double(d), Type::I1) => if d.to_bits() != 0 { "true".to_string() } else { "false".to_string() },
            (Constant::Double(d), Type::F64) => Self::double_literal(*d),
            (Constant::Int(n), Type::F64) => Self::double_literal(f64::from_bits(*n as u64)),
            (Constant::Bool(b), Type::F64) => Self::double_literal(f64::from_bits(u64::from(*b))),
            (Constant::Null, Type::F64) => Self::double_literal(0.0),
            (Constant::Int(n), _) => n.to_string(),
            (Constant::Bool(b), _) => if *b { "1".to_string() } else { "0".to_string() },
            (Constant::Null, _) => "0".to_string(),
            (Constant::Double(d), _) => (d.to_bits() as i64).to_string(),
        };
        Some(s)
    }

    /// Converts an operand to the type that the position requires.
    ///
    /// The emitter prints the LLVM type at each position ("ret i64", "add i64",
    /// "br i1", the declared type of each runtime argument), but the HIR
    /// carries bool sometimes as i1 (result of icmp), sometimes as i8 (the boundary
    /// with Rust), and int/handle as i64. Without this conversion Clang rejects the
    /// whole module — it was the cause of 172 of the 214 failures of the native corpus.
    ///
    /// Between i1/i8/i64 the conversion is zext/trunc. Between double and i64 it is a bitcast,
    /// not sitofp/fptosi: the HIR has its own nodes (IntToDouble/DoubleToInt) for
    /// numeric conversion, so a double occupying an i64 slot can only be the
    /// bit pattern — that is how Const(Double) is emitted and how doubles
    /// cross the runtime.
    fn coerce(&mut self, op: &Operand, target: Type) -> String {
        if target == Type::Void {
            return self.operand_str(op);
        }
        if let Some(s) = self.constant_in_type(op, target) {
            return s;
        }
        let mut current = self.type_of(op);
        let mut text = self.operand_str(op);
        if current.llvm_ir() == target.llvm_ir() && (current == Type::F64) == (target == Type::F64) {
            return text;
        }
        // double becomes i64 (and vice versa) always through the bits; smaller widths
        // go through i64 first because bitcast requires the same size.
        if current == Type::F64 && target != Type::F64 {
            text = self.emit_conversion("bitcast", Type::F64, &text, Type::I64);
            current = Type::I64;
        }
        if target == Type::F64 {
            if current != Type::I64 && current != Type::Ref {
                text = self.emit_conversion("zext", current, &text, Type::I64);
                current = Type::I64;
            }
            if current != Type::F64 {
                text = self.emit_conversion("bitcast", Type::I64, &text, Type::F64);
            }
            return text;
        }
        if Self::width(current) < Self::width(target) {
            self.emit_conversion("zext", current, &text, target)
        } else if Self::width(current) > Self::width(target) {
            self.emit_conversion("trunc", current, &text, target)
        } else {
            text
        }
    }

    /// Same conversion, but writing into a name chosen by the caller
    /// (used by the phi entries, which need a stable name).
    fn emit_named_conversion(&mut self, name_str: &str, of: Type, origin: &str, to_repr: Type) {
        let mut current = of;
        let mut text = origin.to_string();
        if current == Type::F64 && to_repr != Type::F64 && Self::width(to_repr) != 64 {
            text = self.emit_conversion("bitcast", Type::F64, &text, Type::I64);
            current = Type::I64;
        }
        if to_repr == Type::F64 && current != Type::F64 && Self::width(current) != 64 {
            text = self.emit_conversion("zext", current, &text, Type::I64);
            current = Type::I64;
        }
        let op = if current == Type::F64 || to_repr == Type::F64 {
            "bitcast"
        } else if Self::width(current) < Self::width(to_repr) {
            "zext"
        } else {
            "trunc"
        };
        writeln!(self.out, "  {name_str} = {op} {} {text} to {}", current.llvm_ir(), to_repr.llvm_ir()).unwrap();
    }

    fn emit_conversion(&mut self, op: &str, of: Type, text: &str, to_repr: Type) -> String {
        let c = self.next_coercion;
        self.next_coercion += 1;
        writeln!(self.out, "  %c{c} = {op} {} {text} to {}", of.llvm_ir(), to_repr.llvm_ir()).unwrap();
        format!("%c{c}")
    }

    fn operand_str(&self, op: &Operand) -> String {
        match op {
            Operand::Val(v) => format!("%v{}", v.0),
            Operand::Constant(Constant::Int(n)) => n.to_string(),
            Operand::Constant(Constant::Double(d)) => {
                let _bits = d.to_bits();
                // In an instruction with an immediate double, LLVM accepts a point or scientific notation
                format!("{d}")
            }
            Operand::Constant(Constant::Bool(b)) => if *b { "true" } else { "false" }.to_string(),
            Operand::Constant(Constant::Null) => "0".to_string(),
            Operand::Constant(Constant::String(_) | Constant::StringWtf8(_)) => {
                panic!("string must be loaded via Instruction::Const");
            }
            Operand::Constant(Constant::Function(f)) => format!("ptrtoint (ptr @{f} to i64)"),
        }
    }
}

/// Roots by maps: the calls that never collect besides the externs that the
/// effects table marks that way — the leaf helpers ([`leaf_helpers`]),
/// the unwinding and the frame of the effects check.
const MAP_LEAVES: &[&str] = &[
    arc_tag_guards::RETAIN,
    arc_tag_guards::RELEASE,
    "df.body",
    "df.barrier",
    "df.element_barrier",
    "df.is_object",
    "df.young_child",
    "df.throw",
    "dartforge_effects_before",
    "dartforge_effects_after",
    "dartforge_effects_level",
    "dartforge_effects_restore",
];

/// The rule of roots by the callee (`LlvmEmitter::callee_roots_the_arguments`)
/// is on: `DARTFORGE_ROOTS_IN_CALLEE=1` (measurement; off by
/// default). It removes ~8% of the root stores of new_sali/backend, but
/// changes the coloring of the slots: a shared slot keeps the last value
/// until it is reused or the frame closes, and with fewer rooted values the
/// dead value stays longer — the `json` core of `bench/comparison`
/// ended with 1.05 million live objects instead of 650 thousand (peak of
/// 121 against 95 MB). Since then the slot of a dead value is zeroed before the next
/// collection point (`roots::dead_slots`), which this measurement did not have yet.
fn roots_in_callee() -> bool {
    static ENABLED_IN_CALLEE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED_IN_CALLEE.get_or_init(|| std::env::var("DARTFORGE_ROOTS_IN_CALLEE").is_ok_and(|v| v == "1"))
}

/// Roots by maps: the call of the line may collect — the indirect one, and the
/// direct one to a function that is neither an LLVM intrinsic nor a leaf (the extern that
/// the effects table marks as neither collecting nor calling Dart, except for the
/// `leaf:<name>` sabotage, which treats it as a leaf; the helpers of
/// [`MAP_LEAVES`]). A name outside the tables (the Dart function, the
/// helper that allocates) collects.
fn collecting_call(line: &str) -> bool {
    match stack_trace::call_target(line) {
        None => false,
        Some(None) => true,
        Some(Some(name_str)) => {
            if name_str.starts_with("llvm.") || MAP_LEAVES.contains(&name_str) || crate::target::sabotaged_leaf(name_str) {
                return false;
            }
            let ef = externs::effects_of(name_str);
            ef.allocates || ef.calls_dart
        }
    }
}

/// Roots by maps (§14.12): the line (with or without the `\n`) with the operand
/// `beam` (`"deopt"(…)`), if it is a call that may collect still without
/// it; otherwise, the line itself. The operand goes after the attributes of the
/// call and before the label of an `invoke` and the metadata.
fn with_deopt<'a>(line: &'a str, beam: &str) -> std::borrow::Cow<'a, str> {
    if line.contains("\"deopt\"(") || !collecting_call(line) {
        return std::borrow::Cow::Borrowed(line);
    }
    let (body, end) = match line.strip_suffix('\n') {
        Some(c) => (c, "\n"),
        None => (line, ""),
    };
    let pos = [body.find(" to label "), body.find(", !")].into_iter().flatten().min().unwrap_or(body.len());
    std::borrow::Cow::Owned(format!("{} [ {beam} ]{}{end}", &body[..pos], &body[pos..]))
}

/// Roots by maps (§14.12): the empty `"deopt"()` on every call that may
/// collect and does not have its own yet — that of the `@df.*` helpers, of the entry, of the
/// functions without roots in the map. LLVM's inliner only joins the live values of the caller
/// to the `"deopt"` that the copied call already has: without it, the call of the
/// copied body would come out without the roots of the caller. The intermediaries
/// `@df.vararg.<k>` stay as they are (`noinline`; the variadic call does not
/// become a statepoint). Every line between the `define` and the `}` counts: the body of the
/// helpers comes out without indentation (the `\` continuation of Rust strings eats the
/// spaces).
fn empty_deopt(text: &str) -> String {
    let mut exit = String::with_capacity(text.len() + text.len() / 16);
    let mut inside = false;
    let mut intermediate = false;
    for line in text.split_inclusive('\n') {
        if line.starts_with("define ") {
            inside = true;
            intermediate = line.contains("@df.vararg.");
            exit.push_str(line);
            continue;
        }
        if line.starts_with('}') {
            inside = false;
        }
        if inside && !intermediate {
            exit.push_str(&with_deopt(line, "\"deopt\"()"));
        } else {
            exit.push_str(line);
        }
    }
    exit
}

/// Roots by maps: the `@df.*` helpers that provably do not collect
/// get `"gc-leaf-function"` — out of line (the large program), the
/// call to them does not become a collection point. Inline the attribute changes
/// nothing. Only those that do not call the runtime, or call an extern that does not allocate
/// in the heap of the collector (`dartforge_remember`).
fn leaf_helpers(text: &str) -> String {
    const LEAVES: &[&str] = &["@df.body(", "@df.barrier(", "@df.element_barrier(", "@df.is_object(", "@df.young_child("];
    let mut exit = String::with_capacity(text.len() + 256);
    for line in text.split_inclusive('\n') {
        let target = line.starts_with("define internal ") && LEAVES.iter().any(|f| line.contains(f));
        match line.rfind(" {") {
            Some(i) if target => {
                exit.push_str(&line[..i]);
                exit.push_str(" \"gc-leaf-function\"");
                exit.push_str(&line[i..]);
            }
            _ => exit.push_str(line),
        }
    }
    exit
}

/// How many Rust → Dart ports the program module defines
/// ([`LlvmEmitter::emit_ports`]): arities 0 to 7, without and with a value. The
/// runtime has the same number (`DART_PORTS`, `exceptions_tables.rs`).
const N_PORTS: usize = 16;

/// What every module declares with exceptions by tables
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13):
///
/// * the personality of the functions with a landing pad (`dartforge_personality`, of the
///   runtime) and the registration of the ports;
/// * `@df.throw`, the unwinding: a structured exception of the system with
///   its own code `0xE0444652` (the integer below), non-continuable, without
///   parameters — the Dart exception itself is the pending flag of the runtime. `internal`:
///   each module (and each part of a split module) has its own copy.
const EXCEPTIONS_BY_TABLES: &str = "; Table exceptions (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)\n\
declare i32 @dartforge_personality(...)\n\
declare void @dartforge_register_ports(ptr, i64)\n\
declare dllimport void @RaiseException(i32, i32, i32, ptr)\n\
define internal void @df.throw() noreturn noinline cold \"gc-leaf-function\" {\n\
  call void @RaiseException(i32 -532396462, i32 1, i32 0, ptr null)\n\
  unreachable\n\
}\n";

/// The same on Itanium targets (Linux, macOS): `@df.throw` asks the runtime for
/// the exception object of the thread (`exceptions_tables.rs`) and hands it itself
/// to the system unwinder (`_Unwind_RaiseException`), like Windows'
/// `RaiseException`: no Rust frame stays between the throw
/// and the landing pad. In the variant of the runtime with `panic=unwind` (the DLL of the source
/// SDK), a Rust `extern "C"` function in the path has the abort guard,
/// and Rust's personality returned `_URC_FATAL_PHASE1_ERROR` (3) to the
/// foreign exception. If the unwinder comes back, the code goes to the runtime,
/// which terminates.
const EXCEPTIONS_BY_TABLES_ITANIUM: &str = "; Table exceptions (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md)
declare i32 @dartforge_personality(...)
declare i32 @dartforge_personality_cleanup_itanium(...)
declare i32 @llvm.eh.typeid.for(ptr)
declare void @dartforge_register_ports(ptr, i64)
declare ptr @dartforge_unwind_object()
declare i32 @_Unwind_RaiseException(ptr)
declare void @dartforge_unwind_failed(i32) noreturn
define internal void @df.throw() noreturn noinline cold \"gc-leaf-function\" {
  %o = call ptr @dartforge_unwind_object()
  %r = call i32 @_Unwind_RaiseException(ptr %o)
  call void @dartforge_unwind_failed(i32 %r)
  unreachable
}
";

/// The call, at the program entry, of a Dart function without a value: direct
/// or, with exceptions by tables, through the port of the arity
/// ([`LlvmEmitter::emit_ports`]) — the entry is called by the runtime, and an
/// unwinding cannot reach it.
fn entry_call(by_tables: bool, sym_name: &str, args: &[&str]) -> String {
    if by_tables {
        let mut list = format!("ptr @{sym_name}");
        for a in args {
            list.push_str(", ");
            list.push_str(a);
        }
        format!("call void @df.port.v{}({list})", args.len())
    } else {
        format!("call void @{sym_name}({})", args.join(", "))
    }
}

/// The hash (64-bit FNV-1a) of the name of a slot or of the key of the module in the
/// global area; never 0, which marks the slot that does not migrate.
fn slot_hash(name_str: &str) -> i64 {
    let h = fnv(name_str.bytes()) & !(CACHE_BIT as u64);
    (if h == 0 { 1 } else { h }) as i64
}

/// The bit that marks, in the area descriptor, the name of a selector cache
/// slot (the runtime has the same constant, `gc_roots.rs`).
pub(crate) const CACHE_BIT: i64 = 1 << 62;

/// The name of the word `word` (0 or 1) of the cache `name`, with [`CACHE_BIT`].
fn cache_hash(name_str: &str, word: u8) -> i64 {
    (fnv(name_str.bytes().chain([0, word])) | CACHE_BIT as u64) as i64
}

/// 64-bit FNV-1a.
fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Inline boxing and unboxing of `int` (R10, `runtime/src/heap.rs`,
/// `smi`): an `int` in `[-2^62, 2^62)` in a `Ref` position is the `Smi`
/// `(v << 1) | 1`, without allocation; outside the range, and to unbox a
/// reference that is not a `Smi` (the `_Mint`, or the `TypeError`), the runtime.
/// `internal` in each module and `alwaysinline`: the common path becomes three
/// instructions in place of the call.
const GET_AREA: &str = "@df.area_id = internal global i64 0, align 8\n\
define internal ptr @df.get_area() alwaysinline {\n\
  %ctx = call ptr @dartforge_context()\n\
  %id = load atomic i64, ptr @df.area_id monotonic, align 8\n\
  %tp = getelementptr inbounds i8, ptr %ctx, i64 16\n\
  %t = load ptr, ptr %tp, align 8\n\
  %np = getelementptr inbounds i8, ptr %ctx, i64 24\n\
  %n = load i64, ptr %np, align 8\n\
  %dentro = icmp ult i64 %id, %n\n\
  br i1 %dentro, label %ler, label %lenta\n\
ler:\n\
  %e = getelementptr ptr, ptr %t, i64 %id\n\
  %p = load ptr, ptr %e, align 8\n\
  %tem = icmp ne ptr %p, null\n\
  br i1 %tem, label %pronto, label %lenta\n\
pronto:\n\
  ret ptr %p\n\
lenta:\n\
  %q = call ptr @dartforge_globals_area_id(ptr @df.area, ptr @df.area_id)\n\
  ret ptr %q\n\
}\n";

/// The class of a value and the cache of the call site by selector,
/// inline. An object of the space (`h & 3 == 2`, `runtime/src/heap.rs`,
/// `Header`) has the `class_id` in the header (`block + 4`, and the handle is
/// `block + 2`): one load, like the VM's `LoadClassId`. A runtime value
/// in a slot (`h > 0`, `h & 3 == 0`, index `h / 4 - 1`) has the class in the
/// dense vector of the heap (`Heap::classes`, 8 bytes per slot, the `cid` in the first
/// 4), whose address and length the thread context publishes
/// (offsets 336 and 344, `runtime/src/heap.rs`, `Context`): reread on
/// each use, because the vector grows (changes address) when the heap gains
/// slots. An index out of range or an unknown class (`i32::MIN`), and the
/// rest (`null` and `Smi` without the source SDK), ask the runtime
/// (`dartforge_value_class`).
///
/// `df.subclass` is `cid <: C` through the bitmap of the target that the runtime
/// builds on the first query (`runtime/src/core.rs`, `SubtypeMaps`;
/// offsets 352–368 of the context), the VM's *type testing stub*: without a
/// call when the map exists and the `cid` is within its width.
/// The cache (`cache[0]` = class + 1, `cache[1]` = entry, in the area of the
/// isolate) is checked here; only a miss calls `dartforge_selector`, which
/// looks up the table of the class and rewrites the cache — the VM's monomorphic
/// *inline cache* (`ICData`), without the call to the runtime on a hit.
///
/// With the source SDK, `null` and the `Smi` also come out without the runtime: their
/// classes (`Null`, `_Smi`) are those of the `cids` table of the module
/// (`sdk_module::runtime_cids`, the one the runtime receives at startup).
fn value_class(cids: &[i64]) -> String {
    let text = VALUE_CLASS
        .replace("@@MASCARA@@", &OBJECT_MASK.to_string())
        .replace("@@CLASSE@@", &(layout::offsets::CLASS as i64 - layout::HANDLE_OFFSET).to_string())
        .replace("@@SUBTIPOS@@", &layout::context::SUBTYPES.to_string())
        .replace("@@N_SUBTIPOS@@", &layout::context::N_SUBTYPES.to_string())
        .replace("@@LARGURA@@", &layout::context::SUBTYPES_WIDTH.to_string());
    // Without the source SDK (the legacy path), `null` and the `Smi` go to the runtime.
    if !matches!(cids, [n, s, ..] if *n >= 0 && *s >= 0) {
        return text.replace("@@RAPIDOS@@", "");
    }
    let fast_set = format!(
        "  %z = icmp eq i64 %h, 0\n\
  br i1 %z, label %nulo, label %s0\n\
nulo:\n\
  ret i64 {null}\n\
s0:\n\
  %b = and i64 %h, 1\n\
  %i = icmp ne i64 %b, 0\n\
  br i1 %i, label %smi, label %s1\n\
smi:\n\
  ret i64 {smi}\n\
s1:\n",
        null = layout::cid::NULL,
        smi = layout::cid::SMI
    );
    text.replace("@@RAPIDOS@@", &fast_set)
}

const VALUE_CLASS: &str = "define internal i64 @df.class(i64 %h) alwaysinline {\n\
@@RAPIDOS@@  %m = and i64 %h, @@MASCARA@@\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %obj, label %rt\n\
obj:\n\
  %p = inttoptr i64 %h to ptr\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 @@CLASSE@@\n\
  %c = load i32, ptr %cp, align 4, !invariant.load !{}\n\
  %r = sext i32 %c to i64\n\
  ret i64 %r\n\
rt:\n\
  %x = call i64 @dartforge_value_class(i64 %h)\n\
  ret i64 %x\n\
}\n\
define internal ptr @df.selector(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l) alwaysinline {\n\
  %cid = call i64 @df.class(i64 %r)\n\
  %k = add i64 %cid, 1\n\
  %c0 = load i64, ptr %c, align 8\n\
  %sim = icmp eq i64 %c0, %k\n\
  br i1 %sim, label %acerto, label %falha\n\
acerto:\n\
  %ep = getelementptr inbounds i64, ptr %c, i64 1\n\
  %e = load ptr, ptr %ep, align 8\n\
  ret ptr %e\n\
falha:\n\
  %f = call ptr @dartforge_selector(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)\n\
  ret ptr %f\n\
}\n\
define internal i8 @df.subclass(i64 %cid, i64 %alvo) alwaysinline {\n\
  %ctx = call ptr @dartforge_context()\n\
  %lp = getelementptr inbounds i8, ptr %ctx, i64 @@LARGURA@@\n\
  %l = load i64, ptr %lp, align 8\n\
  %d1 = icmp ult i64 %cid, %l\n\
  br i1 %d1, label %t1, label %lento\n\
t1:\n\
  %np = getelementptr inbounds i8, ptr %ctx, i64 @@N_SUBTIPOS@@\n\
  %n = load i64, ptr %np, align 8\n\
  %d2 = icmp ult i64 %alvo, %n\n\
  br i1 %d2, label %t2, label %lento\n\
t2:\n\
  %tp = getelementptr inbounds i8, ptr %ctx, i64 @@SUBTIPOS@@\n\
  %t = load ptr, ptr %tp, align 8\n\
  %mp = getelementptr inbounds ptr, ptr %t, i64 %alvo\n\
  %mapa = load ptr, ptr %mp, align 8\n\
  %tem = icmp ne ptr %mapa, null\n\
  br i1 %tem, label %t3, label %lento\n\
t3:\n\
  %w = lshr i64 %cid, 6\n\
  %wp = getelementptr inbounds i64, ptr %mapa, i64 %w\n\
  %pal = load i64, ptr %wp, align 8\n\
  %s = and i64 %cid, 63\n\
  %x = lshr i64 %pal, %s\n\
  %x8 = trunc i64 %x to i8\n\
  %b = and i8 %x8, 1\n\
  ret i8 %b\n\
lento:\n\
  %y = call i8 @dartforge_is_subclass(i64 %cid, i64 %alvo)\n\
  ret i8 %y\n\
}\n";

/// `@df.*` helpers that go out of line (with the `selector` and the `class`, the
/// dynamic dispatch, in [`out_of_line_helpers`]): each one is a
/// sequence that repeats in hundreds of thousands of places of the large
/// program (docs/NATIVO-PRODUCAO-GRANDE.md §1.4).
pub const OUTLINED_HELPERS_LARGE_PROGRAM: &[&str] =
    &["subclass", "allocate", "barrier", "element_barrier", "get_area", "new_instance"];

/// The list of out-of-line helpers: `DARTFORGE_OUTLINED_HELPERS` (a list
/// separated by commas; `1`, all; empty, none); otherwise all in the
/// large program, in development (`-O0`, where `alwaysinline` copies
/// the helper at each use and code generation pays for each copy:
/// `large_development`) and in production (`large_program`), and none
/// in the others — the small program runs twice as long at `-O0` without the
/// inline helpers (the corpus/native: 53 → 116 s).
pub fn out_of_line_helpers(large_development: bool, large_program: bool) -> Vec<String> {
    let all = || ["selector", "class"].iter().chain(OUTLINED_HELPERS_LARGE_PROGRAM).map(|s| (*s).to_string()).collect();
    match std::env::var("DARTFORGE_OUTLINED_HELPERS") {
        Ok(v) if v == "1" => all(),
        Ok(v) => v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
        Err(_) if large_development || large_program => all(),
        Err(_) => Vec::new(),
    }
}

/// Dispatch by selector with the static descriptor (`@df.seld.<k>`), out
/// of line ([`LlvmEmitter::compact_selector`]).
const SELECTOR_BY_DESCRIPTOR: &str = "define internal ptr @df.selector_d(ptr %c, i64 %r, ptr %d) noinline {
  %h = load i64, ptr %d, align 8
  %np = getelementptr inbounds i8, ptr %d, i64 8
  %n = load ptr, ptr %np, align 8
  %lp = getelementptr inbounds i8, ptr %d, i64 16
  %l = load i64, ptr %lp, align 8
  %f = call ptr @df.selector(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)
  ret ptr %f
}
";

/// The instance allocation of `emit_inline_allocation` out of line:
/// `%cur` is the offset of the TLAB cursor of the words of the object in the
/// context (the end comes 8 bytes later), `%tam` the bytes of the block, `%cab` the
/// header word, `%tab` the function of the method table (null when the
/// class does not register the table on the first allocation), `%mapa` the
/// static reference map (0 when unknown: the zeroed block already has it, and
/// the runtime writes 0 too). `registered`: every table is registered at
/// startup (`LlvmEmitter::with_tables_registered_at_startup`), no registration
/// test.
fn new_instance(registered: bool) -> String {
    let registration = if registered {
        "  br i1 %tz, label %lento, label %rapido
"
    } else {
        "  br i1 %tz, label %lento, label %reg
reg:
  br i1 %semtab, label %rapido, label %reg2
reg2:
  %tnp = getelementptr inbounds i8, ptr %ctx, i64 56
  %tn = load i64, ptr %tnp, align 8
  %tk = icmp ule i64 %tn, %c
  br i1 %tk, label %lento, label %reg3
reg3:
  %trp = getelementptr inbounds i8, ptr %ctx, i64 48
  %tr = load ptr, ptr %trp, align 8
  %trb = getelementptr inbounds i8, ptr %tr, i64 %c
  %trv = load i8, ptr %trb, align 1
  %trz = icmp eq i8 %trv, 0
  br i1 %trz, label %lento, label %rapido
"
    };
    format!(
        "define internal i64 @df.new_instance(ptr %ctx, i64 %cur, i64 %tam, i64 %cab, i64 %c, i64 %n, ptr %tab, i64 %mapa) noinline {{
  %ta = getelementptr inbounds i8, ptr %ctx, i64 %cur
  %tb = load ptr, ptr %ta, align 8
  %tfa = getelementptr inbounds i8, ptr %ta, i64 8
  %tf = load ptr, ptr %tfa, align 8
  %tnx = getelementptr i8, ptr %tb, i64 %tam
  %tz = icmp ugt ptr %tnx, %tf
  %semtab = icmp eq ptr %tab, null
{registration}rapido:
  store ptr %tnx, ptr %ta, align 8
  store i64 %cab, ptr %tb, align 8
  %tmp = getelementptr inbounds i8, ptr %tb, i64 {map}
  %mapa32 = trunc i64 %mapa to i32
  store i32 %mapa32, ptr %tmp, align 4
  %thb = ptrtoint ptr %tb to i64
  %th = add i64 %thb, 2
  ret i64 %th
lento:
  br i1 %semtab, label %semt, label %comt
comt:
  %a = call i64 @dartforge_object_new_mt(i64 %c, i64 %n, i64 %mapa, ptr %tab)
  ret i64 %a
semt:
  %b = call i64 @dartforge_object_new_m(i64 %c, i64 %n, i64 %mapa)
  ret i64 %b
}}
",
        map = layout::offsets::MAP
    )
}

/// `text` with the `define internal … @df.<name>(…) alwaysinline` of `names`
/// switched to `noinline`.
fn out_of_line(text: &str, names: &[String]) -> String {
    if names.is_empty() {
        return text.to_string();
    }
    let mut exit = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let target = line.starts_with("define internal ")
            && names.iter().any(|n| line.contains(&format!("@df.{n}(")));
        if target {
            exit.push_str(&line.replacen(" alwaysinline", " noinline", 1));
        } else {
            exit.push_str(line);
        }
    }
    exit
}

/// The helpers of the foundation of the unified space (docs/NATIVO-ESPACO-UNIFICADO.md
/// §3.5), with the offsets of the layout contract:
///
/// * `@df.is_object(h)`: `h & (7 | i64::MIN) == 2`;
/// * `@df.young_child(v)`: the stored child requires the barrier (young object:
///   state `YOUNG`, `SURVIVOR` or `SURVIVOR_B`, `layout::state::is_young`);
/// * `@df.barrier(o, v)`: old `o` receiving a young child → `dartforge_remember`
///   (weight 1:9 on the slow branch, like Julia's barrier);
/// * `@df.element_barrier(o, i, v)`: the same with cards (`o` old or
///   remembered with `CARDS`: dirties the card of element `i`);
/// * `@df.allocate(header, w)`: the TLAB of `w ≤ TLAB_N` words (the cursor
///   advances `16 + 8w` and writes word 0); exhausted or a larger `w`,
///   `dartforge_allocate(cid, w, header >> 8)`.
///
/// `@df.body(h, ctx)`: the start of the body of an object (the header; the
/// fields 16 bytes later), the same computation as `emit_field_addresses`
/// inline — a value that is not an object reads the reserve object of the context
/// (`ctx + 40`), and a body outside the block (attached, `flags & 1`) is followed through the
/// pointer at `+16`. `alwaysinline` when the emitter is inline (so that the
/// optimizer sees the same); without the attribute in development, a call.
/// `without_outside`: no object has an outside body (`body_without_outside`,
/// see `emit_field_addresses`): the body is the first select.
fn body_helper(per_call: bool, production: bool, without_outside: bool) -> String {
    // In production by call: truly out of line, and without effect or
    // memory dependency (`memory(none)`): the body of an object does not
    // change during its life — the collector does not move, and the outside body only
    // appears in the migration of a JIT reload (which is never production) —, so the
    // optimizer joins all the calls on the same object, even with
    // stores and calls in between (the frame of `$async`, the `this`).
    // Before, `memory(read)`: two calls only joined without a write between
    // them. docs/NATIVO-PRODUCAO-GRANDE.md §6.3.
    let attribute = match (per_call, production) {
        (false, _) => " alwaysinline",
        (true, false) => "",
        (true, true) => " noinline nounwind willreturn memory(none)",
    };
    if without_outside {
        return format!(
            "define internal ptr @df.body(i64 %o, ptr %ctx){attribute} {{
  %k = and i64 %o, {OBJECT_MASK}
  %e = icmp eq i64 %k, 2
  %a = add i64 %o, -2
  %q = inttoptr i64 %a to ptr
  %vp = getelementptr inbounds i8, ptr %ctx, i64 40
  %v = load ptr, ptr %vp, align 8
  %h = select i1 %e, ptr %q, ptr %v
  ret ptr %h
}}
"
        );
    }
    format!(
        "define internal ptr @df.body(i64 %o, ptr %ctx){attribute} {{
  %k = and i64 %o, {OBJECT_MASK}
  %e = icmp eq i64 %k, 2
  %a = add i64 %o, -2
  %q = inttoptr i64 %a to ptr
  %vp = getelementptr inbounds i8, ptr %ctx, i64 40
  %v = load ptr, ptr %vp, align 8
  %h = select i1 %e, ptr %q, ptr %v
  %fp = getelementptr inbounds i8, ptr %h, i64 1
  %f = load i8, ptr %fp, align 1
  %b = and i8 %f, 1
  %x = icmp ne i8 %b, 0
  %cp = getelementptr inbounds i8, ptr %h, i64 16
  %c = load ptr, ptr %cp, align 8
  %r = select i1 %x, ptr %c, ptr %h
  ret ptr %r
}}
"
    )
}

fn space_helpers() -> String {
    use layout::{context, offsets, state, flags};
    let d = |x: usize| x as i64 - layout::HANDLE_OFFSET;
    format!(
        "define internal i1 @df.is_object(i64 %h) alwaysinline {{\n\
  %m = and i64 %h, {mask}\n\
  %r = icmp eq i64 %m, 2\n\
  ret i1 %r\n\
}}\n\
define internal i1 @df.young_child(i64 %v) alwaysinline {{\n\
  %b = and i64 %v, 1\n\
  %imediato = icmp ne i64 %b, 0\n\
  %nulo = icmp eq i64 %v, 0\n\
  %nada = or i1 %imediato, %nulo\n\
  br i1 %nada, label %nao, label %s1\n\
s1:\n\
  %m = and i64 %v, {mask}\n\
  %o = icmp eq i64 %m, 2\n\
  br i1 %o, label %obj, label %nao\n\
obj:\n\
  %p = inttoptr i64 %v to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %ey = sub i8 %e, {young}\n\
  %j1 = icmp ule i8 %ey, {young_span}\n\
  %j2 = icmp eq i8 %e, {survivor_b}\n\
  %j = or i1 %j1, %j2\n\
  ret i1 %j\n\
nao:\n\
  ret i1 false\n\
}}\n\
define internal void @df.barrier(i64 %o, i64 %v) alwaysinline {{\n\
  %p = inttoptr i64 %o to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %velho = icmp eq i8 %e, {old_one}\n\
  %x = call i1 @llvm.expect.i1(i1 %velho, i1 false)\n\
  br i1 %x, label %filho, label %fim\n\
filho:\n\
  %j = call i1 @df.young_child(i64 %v)\n\
  br i1 %j, label %lembrar, label %fim\n\
lembrar:\n\
  call void @dartforge_remember(i64 %o)\n\
  br label %fim\n\
fim:\n\
  ret void\n\
}}\n\
define internal void @df.element_barrier(i64 %o, i64 %i, i64 %v) alwaysinline {{\n\
  %p = inttoptr i64 %o to ptr\n\
  %ep = getelementptr inbounds i8, ptr %p, i64 {state_h}\n\
  %e = load i8, ptr %ep, align 8\n\
  %e3 = sub i8 %e, {old_one}\n\
  %velho = icmp ult i8 %e3, 2\n\
  %x = call i1 @llvm.expect.i1(i1 %velho, i1 false)\n\
  br i1 %x, label %filho, label %fim\n\
filho:\n\
  %j = call i1 @df.young_child(i64 %v)\n\
  br i1 %j, label %lento, label %fim\n\
lento:\n\
  %fp = getelementptr inbounds i8, ptr %p, i64 {flags_h}\n\
  %f = load i8, ptr %fp, align 1\n\
  %fc = and i8 %f, {cards}\n\
  %temc = icmp ne i8 %fc, 0\n\
  br i1 %temc, label %cartao, label %lembrar\n\
cartao:\n\
  %lp = getelementptr inbounds i8, ptr %p, i64 {length_h}\n\
  %len = load i64, ptr %lp, align 8\n\
  %w = lshr i64 %i, {log_word}\n\
  %k = add i64 %len, %w\n\
  %kb = shl i64 %k, 3\n\
  %kd = add i64 %kb, {elements_h}\n\
  %cp = getelementptr inbounds i8, ptr %p, i64 %kd\n\
  %cv = load i64, ptr %cp, align 8\n\
  %s = lshr i64 %i, {log_card}\n\
  %s63 = and i64 %s, 63\n\
  %bit = shl i64 1, %s63\n\
  %nv = or i64 %cv, %bit\n\
  store i64 %nv, ptr %cp, align 8\n\
  br label %lembrar\n\
lembrar:\n\
  %ev = icmp eq i8 %e, {old_one}\n\
  br i1 %ev, label %chamar, label %fim\n\
chamar:\n\
  call void @dartforge_remember(i64 %o)\n\
  br label %fim\n\
fim:\n\
  ret void\n\
}}\n\
define internal i64 @df.allocate(i64 %cab, i64 %w) alwaysinline {{\n\
  %pequeno = icmp ule i64 %w, {tlab_n}\n\
  br i1 %pequeno, label %tlab, label %lento\n\
tlab:\n\
  %ctx = call ptr @dartforge_context()\n\
  %o16 = shl i64 %w, 4\n\
  %oc = add i64 %o16, {tlab}\n\
  %ta = getelementptr inbounds i8, ptr %ctx, i64 %oc\n\
  %tb = load ptr, ptr %ta, align 8\n\
  %of = add i64 %oc, 8\n\
  %tfa = getelementptr inbounds i8, ptr %ctx, i64 %of\n\
  %tf = load ptr, ptr %tfa, align 8\n\
  %w8 = shl i64 %w, 3\n\
  %tam = add i64 %w8, {header}\n\
  %tnx = getelementptr i8, ptr %tb, i64 %tam\n\
  %esgotada = icmp ugt ptr %tnx, %tf\n\
  %x = call i1 @llvm.expect.i1(i1 %esgotada, i1 false)\n\
  br i1 %x, label %lento, label %rapido\n\
rapido:\n\
  store ptr %tnx, ptr %ta, align 8\n\
  store i64 %cab, ptr %tb, align 8\n\
  %hb = ptrtoint ptr %tb to i64\n\
  %h = add i64 %hb, {offset}\n\
  ret i64 %h\n\
lento:\n\
  %cid = lshr i64 %cab, 32\n\
  %fl = lshr i64 %cab, 8\n\
  %fn = and i64 %fl, 16777215\n\
  %r = call i64 @dartforge_allocate(i64 %cid, i64 %w, i64 %fn)\n\
  ret i64 %r\n\
}}\n",
        mask = OBJECT_MASK,
        state_h = d(offsets::STATE),
        flags_h = d(offsets::FLAGS),
        length_h = d(offsets::LENGTH),
        elements_h = d(offsets::ELEMENTS),
        young = state::YOUNG,
        young_span = state::SURVIVOR - state::YOUNG,
        survivor_b = state::SURVIVOR_B,
        old_one = state::OLD,
        cards = flags::CARDS,
        log_word = layout::ELEMENTS_PER_CARD_WORD.trailing_zeros(),
        log_card = layout::ELEMENTS_PER_CARD.trailing_zeros(),
        tlab_n = layout::TLAB_N,
        tlab = context::TLAB,
        header = layout::HEADER_SIZE,
        offset = layout::HANDLE_OFFSET,
    )
}

// `@df.box_int`, `@df.unbox_int` and `@df.env_ref` are from P2
// (`ir_boxes::HELPERS`, docs/NATIVO-ESPACO-UNIFICADO.md §3.5).
