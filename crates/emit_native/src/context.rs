//! Compilation context of the `dartforge-emit-native` crate.

mod layout_type_identity;
mod constant_strings;

use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::{BodyTypes, Resolved};
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeTable};


pub struct Context<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
    /// Proven interpolated strings for this module, evaluated once during lowering.
    constant_strings: std::sync::OnceLock<std::collections::HashMap<(UnitId, dartforge_frontend::ast::ExprId), Vec<u8>>>,
    /// Identity of the SDK marker, never of a same-named user class.
    finalizable_class: Option<dartforge_elements::model::ClassId>,
    pub entry_lib: Option<LibraryId>,
    /// Runtime class id of each program class (P2): the order of the
    /// stable path (`library_name`, class name), starting from 1,
    /// skipping the 1000–1012 range of the runtime error classes. `None` for
    /// the SDK classes.
    pub class_ids: Vec<Option<u32>>,
    /// With the source SDK, the ids of the SDK classes come from this table
    /// (`sdk_module::sdk_class_ids`), the same as the SDK module's: the
    /// program loads only the libraries it imports, and numbering by what it
    /// loaded would give ids different from those of the compiled SDK.
    pub fixed_sdk_ids: Option<std::sync::Arc<IdTable>>,
    /// The record shapes with a named field of the program (P3): number of
    /// positionals and the names, sorted — each one is a record "class"
    /// with id `SHAPE_BASE_ID + index`.
    pub record_shapes: Vec<(usize, Vec<String>)>,
    /// Directory of the entry library: the `file:` libraries are
    /// named by the path relative to it (stable across machines).
    root: Option<std::path::PathBuf>,
    /// This context lowers an SDK library into a separate object.
    pub sdk_library: bool,
    /// Enables certified enum identity comparisons during constant-pattern lowering.
    pub(crate) enum_pattern_identity_enabled: bool,
    /// Produces ownership facts only for the --memory=arc policy.
    pub arc_memory: bool,
    /// The `assert`s are compiled (`DartFlags::enable_asserts`,
    /// `lower/statements.rs`); off, they emit nothing.
    pub enable_asserts: bool,
    /// `const bool.fromEnvironment("dart.vm.product")` (`DartFlags::product`,
    /// `lower/environment.rs`).
    pub product: bool,
    /// Production AOT (`--optimize`, or the production profile of the SDK):
    /// the platform constants of the target fold (`lower/platform.rs`).
    /// Development and the JIT read them at run time.
    pub production: bool,
    /// Per library: the body of its functions is compiled (the program's;
    /// with [`Context::with_source_sdk`], also those of the source SDK).
    pub compiled: Vec<bool>,
    /// Per library: its functions are lowered **in this** module (the
    /// program's; in a source SDK module, only its library). The other
    /// compiled ones live in another object and are called through the symbol.
    pub in_module: Vec<bool>,
    /// P6: the SDK libraries compiled from source with the program
    /// (`source.rs`); their `is_sdk` is already off in the copy of the
    /// `Program`. Empty for those who do not use `dart:async`.
    pub from_source: std::collections::HashSet<LibraryId>,
    /// RTI: the position of each SDK class (without a heap class id) in the
    /// order of the stable path — the RTI id is `0x2000_0000 +` it
    /// (`lower::rti`, `Context::id_rti`).
    pub sdk_rti_ids: std::collections::HashMap<dartforge_elements::model::ClassId, u32>,
    /// P6: the program uses `dart:async` and has the event loop after
    /// `main` (with the source SDK, `dart:async` is that of the cached module, and
    /// `from_source` stays empty).
    pub uses_dart_async: bool,
    /// P6: the symbols of the source functions with a body (`lower::with_source_body`).
    pub with_source_body: std::sync::OnceLock<std::collections::HashSet<String>>,
    /// `dart:ffi`: the layout of the structs and unions of the program (`lower::ffi::composites`).
    pub ffi_composites: std::sync::OnceLock<Option<crate::lower::ffi::Composites>>,
    /// The erased extension types (`erasure.rs`), computed before the
    /// context; empty without an extension type.
    pub te: crate::erasure::ExtensionTypes,
    /// J05: with debug information, the start of each line of each
    /// unit (by the index of the unit), for the positions of the instructions.
    /// Also turned on by the symbolic stack trace (§13.14), which uses the positions without
    /// DWARF.
    pub debug_info: Option<Vec<Vec<u32>>>,
    /// J05: the positions become line tables of the debugger (`--debug`);
    /// without this, only the symbolic stack trace uses them.
    pub dwarf: bool,
    /// The symbolic stack trace (§13.14) is on: the lowering marks the functions
    /// (hidden frames, `async` bodies, links of awaiters) and inserts the
    /// stack trace stack.
    pub stack_trace: bool,
    /// JIT reload (J03): the ids the live generation gave to the program
    /// classes, by `(library, class)`. The same class keeps the same
    /// id; a new class gets an id above all of them (the live objects
    /// in the heap keep the id).
    previous_ids: Option<std::collections::HashMap<(String, String), u32>>,
    /// The closed world of the program (`native_world.rs`): a program function or
    /// global outside it becomes a body that throws. `None`: no pruning
    /// (the SDK modules, `DARTFORGE_NO_PROGRAM_PRUNING=1`).
    pub world: Option<dartforge_world::World>,
    /// Memo of `sdk_source::implementations` by (class, name): the lookup
    /// walks ALL the program classes and the linearization of each one, and
    /// is done at each member access by class dispatch. In a real
    /// program (new_sali/backend: thousands of classes, hundreds of thousands
    /// of accesses) it was quadratic and dominated emission (minutes). The
    /// result only depends on the program and the compiled libraries, fixed
    /// after the construction of the context.
    pub implementations_memo:
        std::sync::RwLock<std::collections::HashMap<(dartforge_elements::model::ClassId, SymbolId), std::sync::Arc<Vec<crate::lower::sdk_source::Implementation>>>>,
    /// The subtypes of each class (`Context::subtypes`), computed on the
    /// first query.
    pub subtypes_memo:
        std::sync::RwLock<std::collections::HashMap<dartforge_elements::model::ClassId, std::sync::Arc<Vec<dartforge_elements::model::ClassId>>>>,
    /// Likewise for `sdk_source::implementations_by_class`.
    pub implementations_by_class_memo: std::sync::RwLock<
        std::collections::HashMap<
            (dartforge_elements::model::ClassId, SymbolId),
            Option<std::sync::Arc<Vec<(i64, crate::lower::sdk_source::Implementation)>>>,
        >,
    >,
    /// Selected SDK multiplication semantics, certified once for this immutable program.
    pub(crate) primitive_multiply_certificate: std::sync::OnceLock<bool>,
    /// Experimental guarded multiplication; disabled unless explicitly requested.
    pub(crate) primitive_multiply_enabled: bool,
    /// Explicit opt-in; selected source and immutable registration are separate proofs.
    pub(crate) primitive_clamp_enabled: bool,
}

/// The name of the variable of a `:x`/`:var x`/`:x?`/`:x as T` pattern.
pub fn pattern_variable_name(
    ast: &dartforge_frontend::ast::Ast,
    mut p: dartforge_frontend::ast::PatternId,
) -> Option<SymbolId> {
    use dartforge_frontend::ast::PatternKind;
    loop {
        match &ast.pattern(p).kind {
            PatternKind::Variable { name, .. } => return Some(name.sym),
            PatternKind::NullCheck(x) | PatternKind::NullAssert(x) | PatternKind::Cast { pattern: x, .. } => p = *x,
            _ => return None,
        }
    }
}

/// First class id of the record shapes with a named field (well above
/// the ids of the program classes).
pub const SHAPE_BASE_ID: u32 = 0x4000_0000;

/// Escapes a part of a stable symbol: letters, digits and `_` stay; the
/// rest becomes `$` and two hexadecimal digits per UTF-8 byte. The `.` separates the
/// parts, so it never appears raw inside one — the symbol is injective.
pub fn escape_str(part: &str) -> String {
    let mut s = String::with_capacity(part.len());
    for b in part.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' {
            s.push(b as char);
        } else {
            s.push_str(&format!("${b:02x}"));
        }
    }
    s
}

impl<'a> Context<'a> {
    /// Program function `f` was left outside the closed world (it becomes a body that
    /// throws). Only functions of program libraries, with a function node
    /// (methods, top-level, accessors); never constructors.
    pub fn pruned_function(&self, f: usize) -> bool {
        let Some(m) = &self.world else { return false };
        let func = &self.program.functions[f];
        !self.program.library(func.library).is_sdk
            && matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. })
            && !m.is_live_function(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// Instance field `v` of a program class was left outside the
    /// closed world (nobody reads or writes it).
    pub fn pruned_field(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.world else { return false };
        let var = &self.program.variables[v.0 as usize];
        if self.program.library(var.library).is_sdk {
            return false;
        }
        // The world follows the implicit accessors of the field (instance
        // members with `variable`), not the variable.
        let Some(c) = var.class else { return false };
        let mut accessors = self.program.classes[c.0 as usize]
            .instance_members
            .values()
            .filter(|f| self.program.functions[f.0 as usize].variable == Some(v))
            .peekable();
        accessors.peek().is_some() && accessors.all(|f| !m.is_live_function(*f))
    }

    /// Instance member `f` of a program class (a method, an explicit or implicit
    /// field accessor) was left outside the closed world: no
    /// entry in the method table nor adapter.
    pub fn pruned_member(&self, f: usize) -> bool {
        let Some(m) = &self.world else { return false };
        let func = &self.program.functions[f];
        // Only what has a written body (`FunctionRef::Function`) or is a field
        // accessor: the synthetic members (the `noSuchMethod` forwarders
        // that the CFE would create) do not go through the world, and without an
        // entry in the table the call fell into `NoSuchMethodError`
        // (corpus/js/223_nosuchmethod_argumentos).
        let with_node = matches!(func.node, dartforge_elements::model::FunctionRef::Function { .. }) || func.variable.is_some();
        with_node
            && !self.program.library(func.library).is_sdk
            && !matches!(
                func.kind,
                dartforge_elements::model::FunctionKind::Constructor | dartforge_elements::model::FunctionKind::SyntheticConstructor
            )
            && !m.is_live_function(dartforge_elements::model::FunctionElementId(f as u32))
    }

    /// Program global `v` was left outside the closed world.
    pub fn pruned_global(&self, v: dartforge_elements::model::VariableId) -> bool {
        let Some(m) = &self.world else { return false };
        let var = &self.program.variables[v.0 as usize];
        // A `const` always stays: it is the default value of a parameter of an abstract
        // member, which the `noSuchMethod` forwarder generated by the lowering
        // reads and the world does not see (the `Pintor.pattern` of
        // corpus/js/223_nosuchmethod_argumentos); the getter is small.
        !var.const_ && !self.program.library(var.library).is_sdk && !m.is_live_variable(v)
    }

    /// The program classes that are subtypes of `cid` (itself included), in
    /// element id order — what the implementation lookups used to
    /// walk by scanning ALL the classes at each member access.
    pub fn subtypes(&self, cid: dartforge_elements::model::ClassId) -> std::sync::Arc<Vec<dartforge_elements::model::ClassId>> {
        if let Some(r) = self.subtypes_memo.read().unwrap_or_else(|e| e.into_inner()).get(&cid) {
            return r.clone();
        }
        let r: std::sync::Arc<Vec<_>> = std::sync::Arc::new(
            (0..self.program.classes.len() as u32)
                .map(dartforge_elements::model::ClassId)
                .filter(|&k| crate::lower::members::subclass_of(self, k, cid))
                .collect(),
        );
        self.subtypes_memo.write().unwrap_or_else(|e| e.into_inner()).insert(cid, r.clone());
        r
    }

    /// Turns debug information on (J05): indexes the lines of the units.
    pub fn link_debug(&mut self) {
        let lines = self
            .program
            .units
            .iter()
            .map(|u| {
                std::iter::once(0)
                    .chain(u.source.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i as u32 + 1))
                    .collect()
            })
            .collect();
        self.debug_info = Some(lines);
    }

    /// The url of the script of the unit in the VM stack trace (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    /// §13.14): that of the unit (`file:///…`, `package:x/y.dart`); in the SDK, that of the
    /// library for the unit that defines it (`dart:core`) and
    /// `dart:<library>/<file>` for the parts (`dart:core/list.dart`),
    /// with `-patch` for those of the VM and of the native overlay
    /// (`dart:core-patch/growable_array.dart`).
    pub fn stack_trace_url(&self, unit: dartforge_elements::model::UnitId) -> String {
        let u = self.program.unit(unit);
        let library = &self.program.library(u.library).uri;
        let Some(name) = library.strip_prefix("dart:") else { return u.uri.clone() };
        if u.uri == *library {
            return library.clone();
        }
        let file = u
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| u.uri.rsplit('/').next().unwrap_or_default().to_string());
        let patch = u
            .path
            .as_ref()
            .is_some_and(|p| p.components().any(|c| matches!(c.as_os_str().to_str(), Some("_internal" | "native_sdk"))));
        if patch { format!("dart:{name}-patch/{file}") } else { format!("dart:{name}/{file}") }
    }

    /// `(line, column)`, starting from 1, of byte `offset` of unit `unit`,
    /// with debug info on.
    pub fn line_and_column(&self, unit: dartforge_elements::model::UnitId, offset: usize) -> Option<(u32, u32)> {
        let starts = self.debug_info.as_ref()?.get(unit.0 as usize)?;
        let offset = u32::try_from(offset).ok()?;
        let line = starts.partition_point(|&i| i <= offset);
        let start = starts[line.checked_sub(1)?];
        Some((line as u32, offset - start + 1))
    }

    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Self {
        let root = program.entry.and_then(|l| {
            let u = *program.library(l).units.first()?;
            program.unit(u).path.as_ref()?.parent().map(|p| p.to_path_buf())
        });
        let mut ctx = Self {
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            constant_strings: std::sync::OnceLock::new(),
            finalizable_class: program.classes.iter().enumerate().find_map(|(i, c)| {
                (program.library(c.library).uri == "dart:ffi" && interner.resolve(c.name) == "Finalizable")
                    .then_some(dartforge_elements::model::ClassId(i as u32))
            }),
            entry_lib: program.entry,
            class_ids: Vec::new(),
            fixed_sdk_ids: None,
            record_shapes: Vec::new(),
            root,
            sdk_library: false,
            enum_pattern_identity_enabled: std::env::var("DARTFORGE_NO_ENUM_PATTERN_IDENTITY").as_deref() != Ok("1"),
            arc_memory: false,
            enable_asserts: true,
            product: false,
            production: false,
            compiled: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            in_module: program.libraries.iter().map(|l| !l.is_sdk).collect(),
            from_source: std::collections::HashSet::new(),
            sdk_rti_ids: std::collections::HashMap::new(),
            uses_dart_async: false,
            with_source_body: std::sync::OnceLock::new(),
            ffi_composites: std::sync::OnceLock::new(),
            implementations_memo: Default::default(),
            world: None,
            subtypes_memo: Default::default(),
            implementations_by_class_memo: Default::default(),
            primitive_multiply_certificate: Default::default(),
            primitive_multiply_enabled: std::env::var("DARTFORGE_PRIMITIVE_MULTIPLY").as_deref() == Ok("1"),
            primitive_clamp_enabled: std::env::var("DARTFORGE_PRIMITIVE_CLAMP").as_deref() == Ok("1"),
            te: crate::erasure::ExtensionTypes::default(),
            previous_ids: None,
            debug_info: None,
            dwarf: false,
            stack_trace: false,
        };
        // Record shapes with a named field: literals, patterns and types of
        // all the units of the program (the whole set, before the
        // lowering — an untyped access `r.x` tests all those that have `x`).
        let mut shapes = std::collections::BTreeSet::new();
        for u in &program.units {
            if program.library(u.library).is_sdk {
                continue;
            }
            for e in &u.ast.exprs {
                if let dartforge_frontend::ast::ExprKind::Record { positional, named, .. } = &e.kind
                    && !named.is_empty()
                {
                    let mut n: Vec<String> = named.iter().map(|(k, _)| interner.resolve(k.sym).to_string()).collect();
                    n.sort();
                    shapes.insert((positional.len(), n));
                }
            }
            for p in &u.ast.patterns {
                if let dartforge_frontend::ast::PatternKind::Record { fields } = &p.kind {
                    let mut npos = 0;
                    let mut n = Vec::new();
                    for f in fields.iter() {
                        match f.name {
                            Some(k) => n.push(interner.resolve(k.sym).to_string()),
                            None => {
                                // `:x` — the name is that of the variable inside the field.
                                let text = &u.source[f.span.start..f.span.end];
                                if text.trim_start().starts_with(':')
                                    && let Some(s) = pattern_variable_name(&u.ast, f.pattern)
                                {
                                    n.push(interner.resolve(s).to_string());
                                } else {
                                    npos += 1;
                                }
                            }
                        }
                    }
                    if !n.is_empty() {
                        n.sort();
                        shapes.insert((npos, n));
                    }
                }
            }
            for t in &u.ast.types {
                if let dartforge_frontend::ast::TypeKind::Record { positional, named } = &t.kind
                    && !named.is_empty()
                {
                    let mut n: Vec<String> = named.iter().map(|(k, _)| interner.resolve(k.sym).to_string()).collect();
                    n.sort();
                    shapes.insert((positional.len(), n));
                }
            }
        }
        ctx.record_shapes = shapes.into_iter().collect();
        ctx.number_classes();
        ctx
    }

    /// Turns on the source SDK (P5c): the libraries of `SOURCE_LIBRARIES`
    /// get a compiled body and classes with ids.
    pub fn with_source_sdk(mut self) -> Self {
        for (i, l) in self.program.libraries.iter().enumerate() {
            if let Some(name) = l.uri.strip_prefix("dart:")
                && crate::sdk_module::SOURCE_LIBRARIES.contains(&name)
            {
                self.compiled[i] = true;
            }
        }
        self.number_classes();
        self
    }

    /// [`Context::with_source_sdk`] with the ids of the SDK classes fixed
    /// by the table of the compiled SDK.
    pub fn with_source_sdk_and_ids(mut self, ids: std::sync::Arc<IdTable>) -> Self {
        self.fixed_sdk_ids = Some(ids);
        self.with_source_sdk()
    }

    /// The module of a source SDK library (P5c): only it is lowered
    /// here; the program and the other libraries stay out.
    pub fn only_the_library(mut self, lib: LibraryId) -> Self {
        self.sdk_library = true;
        self.in_module = vec![false; self.program.libraries.len()];
        self.in_module[lib.0 as usize] = true;
        self
    }

    /// JIT reload (J03): numbers the program classes keeping the ids
    /// of the live generation (`ids`, from [`ir_ids`]).
    pub fn with_previous_ids(mut self, ids: std::collections::HashMap<(String, String), u32>) -> Self {
        self.previous_ids = Some(ids);
        self.number_classes();
        self
    }

    /// The ids of the program classes, `(id, library, class)`. On a
    /// reload, also those of the live generation whose class is not in this one (J03):
    /// an object of it may be alive, and the class may come back — the id stays
    /// reserved for it from generation to generation.
    pub fn program_ids(&self) -> Vec<(u32, String, String)> {
        let mut ids: Vec<(u32, String, String)> = self
            .program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| !self.program.library(c.library).is_sdk)
            .filter_map(|(i, c)| {
                let id = self.class_ids.get(i).copied().flatten()?;
                Some((id, self.library_name(c.library), self.interner.resolve(c.name).to_string()))
            })
            .collect();
        if let Some(previous_ones) = &self.previous_ids {
            let present_set: std::collections::HashSet<(String, String)> =
                ids.iter().map(|(_, l, c)| (l.clone(), c.clone())).collect();
            let mut reserved: Vec<(u32, String, String)> = previous_ones
                .iter()
                .filter(|(key, _)| !present_set.contains(*key))
                .map(|((l, c), id)| (*id, l.clone(), c.clone()))
                .collect();
            reserved.sort();
            ids.extend(reserved);
        }
        ids
    }

    /// The layout of the objects of each program class (J03, see
    /// `hir::Module::program_fields`).
    pub fn program_fields(&self) -> Vec<(u32, usize, Vec<crate::hir::LayoutField>)> {
        (0..self.program.classes.len() as u32)
            .map(dartforge_elements::model::ClassId)
            .filter(|&c| !self.program.library(self.program.class(c).library).is_sdk)
            .filter_map(|c| {
                let id = self.class_id_of(c)?;
                let fields = crate::lower::members::layout(self, c)
                    .into_iter()
                    .map(|v| {
                        let var = &self.program.variables[v.0 as usize];
                        let t = crate::lower::members::variable_type(self, v);
                        let nullable = matches!(self.table.get(t), Type::Dynamic | Type::Void | Type::Null)
                            || self.table.get(t).is_declared_nullable();
                        crate::hir::LayoutField {
                            name: self.symbol_name(var.name).to_string(),
                            owner: var.class.map(|class| format!("{}:{}",
                                escape_str(&self.library_name(var.library)),
                                escape_str(self.symbol_name(self.program.class(class).name)))),
                            nullable,
                            late: var.late,
                            ty: self.table.format(t, self.interner, self.program),
                            type_identity: self.layout_type_identity(t),
                        }
                    })
                    .collect();
                Some((id, crate::lower::enums::layout_base(self, c), fields))
            })
            .collect()
    }

    /// Is the body of the functions of the library compiled?
    pub fn compiled_library(&self, lib: LibraryId) -> bool {
        self.compiled[lib.0 as usize]
    }

    /// Are the functions of the library lowered in this module?
    pub fn library_in_module(&self, lib: LibraryId) -> bool {
        self.in_module[lib.0 as usize]
    }

    /// Class `name` of `dart:<lib>` (source SDK), if loaded.
    pub fn sdk_class(&self, lib: &str, name: &str) -> Option<dartforge_elements::model::ClassId> {
        let uri = format!("dart:{lib}");
        let sym = self.interner.lookup(name)?;
        self.program
            .classes
            .iter()
            .position(|c| c.name == sym && self.program.library(c.library).uri == uri)
            .map(|i| dartforge_elements::model::ClassId(i as u32))
    }

    /// Stable class ids (P2): the compiled classes in path order — the SDK
    /// ones first (group 0), in a range that only depends on the
    /// SDK, then the program ones —, starting at 128, skipping 1000–1012; the
    /// classes of `layout::cid::FROM_SDK` have a fixed cid (1–65). With the table
    /// of the SDK (`fixed_sdk_ids`), the SDK ones come from it and the program ones
    /// start after the largest.
    fn number_classes(&mut self) {
        let program = self.program;
        let mut ids = vec![None; program.classes.len()];
        let mut keys: Vec<(String, String, usize)> = Vec::new();
        // 1–127 belong to the fixed cids (docs/NATIVO-ESPACO-UNIFICADO.md §2.4).
        let mut next = dartforge_runtime::layout::FIRST_FREE_CID as u32;
        match self.fixed_sdk_ids.clone() {
            Some(table) => {
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiled[c.library.0 as usize] {
                        continue;
                    }
                    let lib = self.library_name(c.library);
                    let name = self.interner.resolve(c.name).to_string();
                    if program.library(c.library).is_sdk {
                        let id = table.ids.get(&(lib, name)).copied().unwrap_or_else(|| {
                            panic!(
                                "class {}::{} without an id in the table of the compiled SDK (stale cache)",
                                self.library_name(c.library),
                                self.interner.resolve(c.name)
                            )
                        });
                        ids[i] = Some(id);
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        // An extension type has no object in the heap: the
                        // value is that of the representation.
                        keys.push((lib, name, i));
                    }
                }
                next = table.next_item;
            }
            None => {
                let sdk = sdk_class_ids(program, self.interner, &self.compiled);
                for (i, c) in program.classes.iter().enumerate() {
                    if !self.compiled[c.library.0 as usize] {
                        continue;
                    }
                    if program.library(c.library).is_sdk {
                        ids[i] = sdk.ids.get(&(self.library_name(c.library), self.interner.resolve(c.name).to_string())).copied();
                    } else if c.kind != dartforge_elements::model::ClassKind::ExtensionType {
                        keys.push((self.library_name(c.library), self.interner.resolve(c.name).to_string(), i));
                    }
                }
                next = next.max(sdk.next_item);
            }
        }
        keys.sort();
        // Reload (J03): a class the live generation already numbered keeps its
        // id; the new ones come after the largest (never that of one that vanished:
        // an object of it may be alive).
        if let Some(previous_ones) = &self.previous_ids {
            if let Some(&larger) = previous_ones.values().max() {
                next = next.max(larger + 1);
            }
            keys.retain(|(lib, name, i)| match previous_ones.get(&(lib.clone(), name.clone())) {
                Some(&id) => {
                    ids[*i] = Some(id);
                    false
                }
                None => true,
            });
        }
        for (_, _, i) in keys {
            if (1000..=1012).contains(&next) {
                next = 1013;
            }
            ids[i] = Some(next);
            next += 1;
        }
        self.class_ids = ids;
        // RTI: the SDK classes without a heap id (the uncompiled ones), in
        // the order of the stable path.
        let mut sdk: Vec<(String, String, usize)> = program
            .classes
            .iter()
            .enumerate()
            .filter(|(_, c)| program.library(c.library).is_sdk && !self.compiled[c.library.0 as usize])
            .map(|(i, c)| (self.library_name(c.library), self.interner.resolve(c.name).to_string(), i))
            .collect();
        sdk.sort();
        self.sdk_rti_ids = sdk
            .into_iter()
            .enumerate()
            .map(|(k, (_, _, i))| (dartforge_elements::model::ClassId(i as u32), k as u32))
            .collect();
    }

    /// The stable name of a library (P2): `dart:x` and `package:a/b.dart`
    /// as they are; `file:` by the path relative to the directory of the entry
    /// library, with `/` (the same program has the same symbols on
    /// any machine and checkout).
    pub fn library_name(&self, lib: LibraryId) -> String {
        let l = self.program.library(lib);
        if !l.uri.starts_with("file:") {
            return l.uri.clone();
        }
        let path = l
            .units
            .first()
            .and_then(|u| self.program.unit(*u).path.clone());
        if let (Some(c), Some(r)) = (path, self.root.as_ref())
            && let Ok(rel) = c.strip_prefix(r)
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
        l.uri.clone()
    }

    /// Class id of the record shape `(npos, names)`, if the program has it.
    pub fn shape_id(&self, npos: usize, names: &[String]) -> Option<u32> {
        self.record_shapes
            .iter()
            .position(|(p, n)| *p == npos && n.as_slice() == names)
            .map(|i| SHAPE_BASE_ID + i as u32)
    }

    /// Runtime class id of a program class.
    pub fn class_id_of(&self, cid: dartforge_elements::model::ClassId) -> Option<u32> {
        self.class_ids.get(cid.0 as usize).copied().flatten()
    }

    pub fn symbol_name(&self, sym: SymbolId) -> &str {
        self.interner.resolve(sym)
    }

    /// The static type of `expr`, **erased** (`erasure.rs`): it is the type
    /// the value has at run time, which decides representation,
    /// fast paths and type tests.
    pub fn get_type(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        self.get_type_raw(unit, expr).map(|t| self.te.erase(t))
    }

    /// The static type of `expr` as inference gave it, with extension
    /// types: only the route of their members and the type arguments of those
    /// members use it.
    pub fn get_type_raw(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<TypeId> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_type(expr)
        } else {
            None
        }
    }

    /// The erased type of `t` (see [`Context::get_type`]).
    pub fn erase(&self, t: TypeId) -> TypeId {
        self.te.erase(t)
    }

    /// `c` is an extension type (erased at run time).
    pub fn is_extension_type(&self, c: dartforge_elements::model::ClassId) -> bool {
        self.program.classes[c.0 as usize].kind == dartforge_elements::model::ClassKind::ExtensionType
    }

    pub fn get_resolved(&self, unit: UnitId, expr: dartforge_frontend::ast::ExprId) -> Option<&Resolved> {
        let u_idx = unit.0 as usize;
        if u_idx < self.bodies.units.len() {
            self.bodies.units[u_idx].get_resolved(expr)
        } else {
            None
        }
    }

    pub fn is_int(&self, ty: TypeId) -> bool {
        if self.core.int_class.is_some() && ty == self.core.int {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "int"
            }
            _ => false,
        }
    }

    pub fn is_double(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "double"
            }
            _ => false,
        }
    }

    pub fn is_bool(&self, ty: TypeId) -> bool {
        if self.core.bool_class.is_some() && ty == self.core.bool_ {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "bool"
            }
            _ => false,
        }
    }

    pub fn is_string(&self, ty: TypeId) -> bool {
        if self.core.string_class.is_some() && ty == self.core.string {
            return true;
        }
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "String"
            }
            _ => false,
        }
    }

    pub fn is_list(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "List"
            }
            _ => false,
        }
    }

    pub fn is_map(&self, ty: TypeId) -> bool {
        match self.table.get(ty) {
            Type::Interface { class, .. } => {
                let name = self.symbol_name(self.program.classes[class.0 as usize].name);
                name == "Map"
            }
            _ => false,
        }
    }

    pub fn is_void(&self, ty: TypeId) -> bool {
        ty == self.core.void_
    }

    /// Representation of a Dart type (R1, `docs/NATIVO-PLANO.md` §6.2).
    ///
    /// Only **non-nullable** `int`, `double` and `bool` are scalars; any
    /// nullable type (`int?` included), `num`, `Object`, `dynamic` and
    /// type parameters are `Ref` — `Type` included: it is the canonical object
    /// of the RTI (`lower/rti.rs`).
    /// The HIR type of the return of function `fid` (declared or inferred). A
    /// synchronous `void` function with a `=> e` body returns the value of `e` (`Ref`):
    /// the `void` is only static, and whoever calls through `dynamic` or `Function`
    /// receives the value, as in the VM (the `handleSpace` of `intl`, `void f() =>
    /// cond ? '' : error()`, kept in a `Map<String, Function>` and called
    /// to write the result; docs/NATIVO-PROJETOS-REAIS.md C23). Direct
    /// callers ignore the value.
    pub fn hir_return(&self, fid: usize) -> crate::hir::Type {
        let Some(d) = self.outline.functions.get(fid) else { return crate::hir::Type::Ref };
        let t = self.to_hir_type(d.return_type);
        if t == crate::hir::Type::Void && self.arrow_void(fid) { crate::hir::Type::Ref } else { t }
    }

    /// A synchronous `void f() => e;`, an ordinary function or method (not a getter, setter,
    /// operator or constructor). See [`Self::hir_return`].
    fn arrow_void(&self, fid: usize) -> bool {
        let f = &self.program.functions[fid];
        // `main` is called by the entry as `void` (`lower/mod.rs`).
        if f.kind != dartforge_elements::model::FunctionKind::Function
            || (f.class.is_none() && self.symbol_name(f.name) == "main")
        {
            return false;
        }
        let dartforge_elements::model::FunctionRef::Function { unit, function } = f.node else { return false };
        let func_decl = self.program.unit(unit).ast.function(function);
        matches!(func_decl.body, dartforge_frontend::ast::FunctionBody::Expression(_))
            && func_decl.modifier == dartforge_frontend::ast::AsyncModifier::None
    }

    pub fn to_hir_type(&self, ty: TypeId) -> crate::hir::Type {
        let ty = self.erase(ty);
        if self.is_void(ty) {
            return crate::hir::Type::Void;
        }
        let Type::Interface { class, nullable, .. } = self.table.get(ty) else {
            return crate::hir::Type::Ref;
        };
        let cls = &self.program.classes[class.0 as usize];
        if *nullable || !self.program.library(cls.library).is_sdk {
            return crate::hir::Type::Ref;
        }
        match self.symbol_name(cls.name) {
            "int" => crate::hir::Type::I64,
            "double" => crate::hir::Type::F64,
            "bool" => crate::hir::Type::I1,
            _ => crate::hir::Type::Ref,
        }
    }

    /// Declared (or inferred) type of the local variable whose name starts at
    /// `offset` (R6).
    pub fn local_type(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.semantic_local_type(unit, offset).map(|t| self.erase(t))
    }

    /// Classifies the static obligation; None still requires proof in the lowering.
    pub(crate) fn classify_finalizable(&self, ty: TypeId) -> Option<bool> {
        crate::finalizables::classify(self.table, &self.outline.hierarchy, self.finalizable_class, ty)
    }

    /// Obligation of a receiver whose class identity is already known.
    pub(crate) fn classify_finalizable_class(&self, cls: dartforge_elements::model::ClassId) -> Option<bool> {
        crate::finalizables::classify_class(&self.outline.hierarchy, self.finalizable_class, cls)
    }

    /// Static type of this in the member: the class or the on type of an extension.
    pub(crate) fn classify_this(&self, fid: usize) -> Option<bool> {
        let f = self.program.functions.get(fid)?;
        if let Some(e) = f.extension {
            self.classify_finalizable(self.outline.extensions.get(e.0 as usize)?.on)
        } else {
            f.class.and_then(|c| self.classify_finalizable_class(c))
        }
    }

    /// Original Dart type of the declaration, before erasure to a representation.
    /// Lexical obligations depend on the static identity, including extension
    /// types; absence in the table does not prove the absence of those obligations.
    pub(crate) fn semantic_local_type(&self, unit: UnitId, offset: usize) -> Option<TypeId> {
        self.bodies.units.get(unit.0 as usize)?.local_type(offset)
    }
}

/// The ids of the classes of the compiled SDK: `(library, class) → id`, and the
/// first free id after them (where the program ones start).
#[derive(Debug, Clone, Default)]
pub struct IdTable {
    pub ids: std::collections::HashMap<(String, String), u32>,
    pub next_item: u32,
}

impl IdTable {
    /// The table as text: one `id\tlibrary\tclass` line per class and
    /// a last `proximo\t<n>` line.
    pub fn to_text(&self) -> String {
        let mut v: Vec<_> = self.ids.iter().collect();
        v.sort_by_key(|(_, id)| **id);
        let mut t = String::new();
        for ((lib, name), id) in v {
            t.push_str(&format!("{id}\t{lib}\t{name}\n"));
        }
        t.push_str(&format!("proximo\t{}\n", self.next_item));
        t
    }

    /// Reads the text of [`IdTable::to_text`].
    pub fn from_string(t: &str) -> Option<Self> {
        let mut table = IdTable::default();
        for line in t.lines() {
            let mut parts = line.split('\t');
            let a = parts.next()?;
            if a == "proximo" {
                table.next_item = parts.next()?.parse().ok()?;
                continue;
            }
            let id: u32 = a.parse().ok()?;
            let lib = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            table.ids.insert((lib, name), id);
        }
        (table.next_item > 0).then_some(table)
    }
}

/// Numbers the classes of the SDK libraries that `compiled` marks, in
/// (library, class) order, starting from 1, skipping 1000–1012 (the runtime
/// ids). It is the single source of the SDK ids: the SDK module computes it with
/// every library loaded, and the program receives the same table.
pub fn sdk_class_ids(program: &Program, interner: &Interner, compiled: &[bool]) -> IdTable {
    let mut keys: Vec<(String, String)> = program
        .classes
        .iter()
        .filter(|c| compiled[c.library.0 as usize] && program.library(c.library).is_sdk)
        .map(|c| (program.library(c.library).uri.clone(), interner.resolve(c.name).to_string()))
        .collect();
    keys.sort();
    let mut table = IdTable::default();
    // The classes the runtime knows have the fixed cid of the layout contract
    // (docs/NATIVO-ESPACO-UNIFICADO.md §2.4, `layout::cid::FROM_SDK`): 1–127 stay
    // reserved (those that do not exist in the loaded SDK have no instances) and the
    // others start at `FIRST_FREE_CID`, skipping 1000–1012.
    let fixed_set: std::collections::HashMap<(&str, &str), u32> =
        dartforge_runtime::layout::cid::SDK_CLASSES.iter().map(|&(c, lib, name)| ((lib, name), c as u32)).collect();
    let mut next = dartforge_runtime::layout::FIRST_FREE_CID as u32;
    for k in keys {
        if let Some(&c) = fixed_set.get(&(k.0.as_str(), k.1.as_str())) {
            table.ids.insert(k, c);
            continue;
        }
        if (1000..=1012).contains(&next) {
            next = 1013;
        }
        table.ids.insert(k, next);
        next += 1;
    }
    table.next_item = next;
    table
}

/// The ids of the program classes written into the IR by a generation
/// (`; df.class <id> <library> <class>`, with the parts escaped by
/// [`escape_str`]), by `(library, class)`.
pub fn ir_ids(ir: &str) -> std::collections::HashMap<(String, String), u32> {
    ir.lines()
        .filter_map(|l| {
            let mut p = l.strip_prefix("; df.class ")?.split(' ');
            let id = p.next()?.parse().ok()?;
            Some(((unescape(p.next()?)?, unescape(p.next()?)?), id))
        })
        .collect()
}

/// The inverse of [`escape_str`].
pub fn unescape(part: &str) -> Option<String> {
    let b = part.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'$' {
            v.push(u8::from_str_radix(part.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            v.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(v).ok()
}
