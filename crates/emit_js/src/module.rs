//! Module emission: one ES6 module per library, classes, top-level functions
//! and variables, rti rules and `main.mjs`.

use crate::Emitted;
use crate::body::{AsyncKind, FnEmitter, js_member_name, static_member_name};
use crate::ctx::Ctx;
use crate::js::{self, Js, P_ASSIGN, Writer};
use crate::ty::Ty;
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, UnitId,
    VariableId, VariableRef,
};
use dartforge_frontend::ast::{self, DeclKind, FunctionBody, MemberKind};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Per-module state: library uses, `dartx` symbols and private names.
pub struct ModState {
    pub lib: LibraryId,
    /// Libraries emitted in this module (a strongly connected component of imports).
    pub group: Vec<LibraryId>,
    pub sdk_used: RefCell<BTreeSet<String>>,
    pub user_imports: RefCell<BTreeSet<u32>>,
    pub dartx_used: RefCell<BTreeMap<String, String>>,
    pub private_syms: RefCell<BTreeMap<String, (u32, String)>>,
    pub noted_classes: RefCell<BTreeSet<u32>>,
    /// Libraries whose emitted constant records call their canonical cache.
    pub const_record_libraries: RefCell<BTreeSet<u32>>,
    /// Private descriptor guards required by emitted typed-array loop versions.
    pub typed_read_guard_libraries: RefCell<BTreeSet<u32>>,
}

impl ModState {
    pub fn new(lib: LibraryId) -> Self {
        ModState {
            lib,
            group: vec![lib],
            sdk_used: RefCell::new(BTreeSet::new()),
            user_imports: RefCell::new(BTreeSet::new()),
            dartx_used: RefCell::new(BTreeMap::new()),
            private_syms: RefCell::new(BTreeMap::new()),
            noted_classes: RefCell::new(BTreeSet::new()),
            const_record_libraries: RefCell::new(BTreeSet::new()),
            typed_read_guard_libraries: RefCell::new(BTreeSet::new()),
        }
    }
    pub fn use_sdk(&self, name: &str) {
        self.sdk_used.borrow_mut().insert(name.to_string());
    }
    pub fn lib_var(&self, ctx: &Ctx, lib: LibraryId) -> String {
        let info = &ctx.libs[lib.0 as usize];
        if info.is_sdk {
            self.use_sdk(&info.js_var);
        } else if !self.group.contains(&lib) {
            self.user_imports.borrow_mut().insert(lib.0);
        }
        info.js_var.clone()
    }
    pub fn dartx(&self, name: &str) -> String {
        let var = dartx_var(name);
        self.dartx_used
            .borrow_mut()
            .insert(var.clone(), name.to_string());
        var
    }
    pub fn private_sym(&self, ctx: &Ctx, lib: LibraryId, name: &str) -> String {
        // Production profile, program library: the private name is a plain
        // property with the library tag (`_x$1c`), not a `Symbol` — Dart privacy
        // is per library, and the tag already separates the `_x` of different
        // libraries. No `dart.privateName` declaration per name, and property
        // renaming shortens it (`docs/JS-PRODUCAO-TAMANHO.md` §3.4).
        if ctx.sdk.is_some() && !ctx.libs[lib.0 as usize].is_sdk {
            return js::string_literal(&crate::production_private_name(name, lib));
        }
        let _ = self.lib_var(ctx, lib);
        let ident = ctx.lib_ident(lib).to_string();
        let var = format!("$P_{ident}_{}", js_safe(name));
        self.private_syms
            .borrow_mut()
            .insert(var.clone(), (lib.0, name.to_string()));
        var
    }
    pub(crate) fn note_const_record(&self, lib: LibraryId) {
        self.const_record_libraries.borrow_mut().insert(lib.0);
    }
    pub fn note_class(&self, c: ClassId) {
        self.noted_classes.borrow_mut().insert(c.0);
    }

    /// Empty state with the same module and group, to emit an isolated fragment
    /// (one class, the rest of a library) and keep it in the cache.
    pub fn derived(&self) -> ModState {
        let mut m = ModState::new(self.lib);
        m.group = self.group.clone();
        m
    }

    /// Merges what a fragment accumulated. All fields are insert-only sets, and
    /// the generated names derive only from the source name (`dartx_var`,
    /// `$P_<lib>_<name>`), so merging is commutative — that is what makes the
    /// per-fragment cache correct.
    pub fn merge(&self, other: &ModState) {
        self.sdk_used
            .borrow_mut()
            .extend(other.sdk_used.borrow().iter().cloned());
        self.user_imports
            .borrow_mut()
            .extend(other.user_imports.borrow().iter().copied());
        self.dartx_used.borrow_mut().extend(
            other
                .dartx_used
                .borrow()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.private_syms.borrow_mut().extend(
            other
                .private_syms
                .borrow()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.noted_classes
            .borrow_mut()
            .extend(other.noted_classes.borrow().iter().copied());
        self.const_record_libraries
            .borrow_mut()
            .extend(other.const_record_libraries.borrow().iter().copied());
        self.typed_read_guard_libraries
            .borrow_mut()
            .extend(other.typed_read_guard_libraries.borrow().iter().copied());
    }
}

/// An already emitted module fragment: the text and what it accumulated in the
/// [`ModState`]. Kept across compilations by the session (`crates/dev`) so that an
/// edit re-emits only the classes of the changed library.
pub struct Fragment {
    pub text: String,
    pub sdk_used: BTreeSet<String>,
    pub user_imports: BTreeSet<u32>,
    pub dartx_used: BTreeMap<String, String>,
    pub private_syms: BTreeMap<String, (u32, String)>,
    pub noted_classes: BTreeSet<u32>,
    pub const_record_libraries: BTreeSet<u32>,
    pub typed_read_guard_libraries: BTreeSet<u32>,
}

impl Fragment {
    fn new(text: String, m: &ModState) -> Fragment {
        Fragment {
            text,
            sdk_used: m.sdk_used.borrow().clone(),
            user_imports: m.user_imports.borrow().clone(),
            dartx_used: m.dartx_used.borrow().clone(),
            private_syms: m.private_syms.borrow().clone(),
            noted_classes: m.noted_classes.borrow().clone(),
            const_record_libraries: m.const_record_libraries.borrow().clone(),
            typed_read_guard_libraries: m.typed_read_guard_libraries.borrow().clone(),
        }
    }
    fn apply_to(&self, m: &ModState) {
        m.sdk_used
            .borrow_mut()
            .extend(self.sdk_used.iter().cloned());
        m.user_imports
            .borrow_mut()
            .extend(self.user_imports.iter().copied());
        m.dartx_used
            .borrow_mut()
            .extend(self.dartx_used.iter().map(|(k, v)| (k.clone(), v.clone())));
        m.private_syms.borrow_mut().extend(
            self.private_syms
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        m.noted_classes
            .borrow_mut()
            .extend(self.noted_classes.iter().copied());
        m.const_record_libraries
            .borrow_mut()
            .extend(self.const_record_libraries.iter().copied());
        m.typed_read_guard_libraries
            .borrow_mut()
            .extend(self.typed_read_guard_libraries.iter().copied());
    }
}

/// Key of a fragment: a class or the rest of a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FragmentKey {
    Class(u32),
    Rest(u32),
}

/// Cache of fragments across compilations, owned by the session.
#[derive(Default)]
pub struct FragmentCache {
    pub(crate) map: HashMap<FragmentKey, Fragment>,
}

impl FragmentCache {
    /// Discards the fragments of the given libraries (the ones that changed).
    pub fn invalidate(&mut self, ctx: &Ctx, libs: &[LibraryId]) {
        let target: HashSet<u32> = libs.iter().map(|l| l.0).collect();
        self.map.retain(|k, _| match k {
            FragmentKey::Rest(l) => !target.contains(l),
            FragmentKey::Class(c) => !target.contains(&ctx.program.classes[*c as usize].library.0),
        });
    }
    pub fn clear(&mut self) {
        self.map.clear();
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

fn js_safe(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '$'
            }
        })
        .collect()
}

/// Name of the variable that holds a `dartx` symbol.
pub fn dartx_var(name: &str) -> String {
    let op = match name {
        "+" => "plus",
        "-" => "minus",
        "*" => "times",
        "/" => "divide",
        "~/" => "floorDivide",
        "%" => "modulo",
        "<<" => "leftShift",
        ">>" => "rightShift",
        ">>>" => "tripleShift",
        "&" => "bitAnd",
        "|" => "bitOr",
        "^" => "bitXor",
        "~" => "bitNot",
        "<" => "lessThan",
        ">" => "greaterThan",
        "<=" => "lessOrEquals",
        ">=" => "greaterOrEquals",
        _ => return format!("${name}"),
    };
    format!("${op}")
}

/// Emits all modules of the program.
pub fn emit(ctx: &Ctx) -> Result<Emitted, Vec<Diagnostic>> {
    emit_filtered(ctx, None)
}

/// Like [`emit`], emitting the text only of the modules that contain one of the
/// libraries in `only` (resident session: the rest stays valid from the previous
/// compilation). `None` emits everything.
///
/// The skipped modules do not go into `Emitted::modules`; the caller merges them
/// with what it already had.
pub fn emit_filtered(ctx: &Ctx, only: Option<&HashSet<u32>>) -> Result<Emitted, Vec<Diagnostic>> {
    emit_with_cache(ctx, only, None)
}

/// Like [`emit_filtered`], reusing the already emitted fragments.
pub fn emit_with_cache(
    ctx: &Ctx,
    only: Option<&HashSet<u32>>,
    cache: Option<&RefCell<FragmentCache>>,
) -> Result<Emitted, Vec<Diagnostic>> {
    // Pruned text never goes into the fragment cache of `dartforge serve`.
    debug_assert!(
        ctx.filter.is_none() || cache.is_none(),
        "production filter with a fragment cache"
    );
    let mut modules = Vec::new();
    let mut entry_ident = String::from("main");
    let mut entry_path = String::from("main.js");
    let mut main_async = false;
    let mut main_args = String::new();
    // The library of the `main` the entry library exports: its own, or one it
    // re-exports (`export 'other.dart';`), as the VM finds it.
    let main_library = entry_main_library(ctx.program, ctx.interner).or(ctx.program.entry);
    let mut done: HashSet<u32> = HashSet::new();
    for (i, lib) in ctx.program.libraries.iter().enumerate() {
        let lid = LibraryId(i as u32);
        let info = &ctx.libs[i];
        if info.is_sdk || lib.units.is_empty() || done.contains(&lid.0) {
            continue;
        }
        let group: Vec<LibraryId> = ctx.groups.get(&lid.0).cloned().unwrap_or_else(|| vec![lid]);
        for g in &group {
            done.insert(g.0);
        }
        let emit_this = only.is_none_or(|s| group.iter().any(|g| s.contains(&g.0)));
        if emit_this {
            let text = emit_group(ctx, &group, cache);
            modules.push((info.module_path.clone(), text));
        }
        for &g in &group {
            if Some(g) == main_library {
                entry_ident = ctx.libs[g.0 as usize].ident.clone();
                entry_path = info.module_path.clone();
                main_async = lib_main_is_async(ctx, g);
                main_args = main_arguments(ctx, g);
            }
        }
    }
    let _ = main_async;
    if let Some(c) = &ctx.type_check {
        c.borrow().report();
    }
    let errors = std::mem::take(&mut *ctx.errors.borrow_mut());
    if !errors.is_empty() {
        return Err(errors);
    }
    // DDC's `dart_sdk.js` is the browser one: `self` is the global (Node only has `globalThis`).
    modules.push((
        "preambulo.js".to_string(),
        "if (typeof self === 'undefined') globalThis.self = globalThis;\n".to_string(),
    ));
    let sdk_imports = if main_args.is_empty() {
        "dart"
    } else {
        "_interceptors, dart, dart_rti"
    };
    let entry = format!(
        "if (typeof process !== 'undefined') process.on('uncaughtException', (e) => {{\n  console.error('Unhandled exception:\\n' + e);\n  process.exit(255);\n}});\nimport './preambulo.js';\nimport {{ {sdk_imports} }} from './dart_sdk.js';\nimport {{ {entry_ident} as m }} from './{entry_path}';\nm.main({main_args});\n"
    );
    Ok(Emitted { modules, entry })
}

/// The library declaring the top-level `main` that the entry library exports.
pub fn entry_main_library(
    program: &dartforge_elements::model::Program,
    interner: &dartforge_intern::Interner,
) -> Option<LibraryId> {
    let name = interner.lookup("main")?;
    let binding = program.library(program.entry?).exported.get(&name)?;
    match binding.getter? {
        Element::Function(f) => {
            let f = &program.functions[f.0 as usize];
            (f.class.is_none() && f.extension.is_none() && f.kind == FunctionKind::Function)
                .then_some(f.library)
        }
        _ => None,
    }
}

fn lib_main_is_async(ctx: &Ctx, lib: LibraryId) -> bool {
    let Some(sym) = ctx.sym("main") else {
        return false;
    };
    let Some(b) = ctx.program.library(lib).declared.get(&sym) else {
        return false;
    };
    let Some(Element::Function(fid)) = b.getter else {
        return false;
    };
    let f = ctx.program.function(fid);
    if let FunctionRef::Function { unit, function } = f.node {
        let af = ctx.program.unit(unit).ast.function(function);
        return af.modifier == ast::AsyncModifier::Async;
    }
    false
}

/// The arguments the entry passes to `main`, as the VM and `dart2js` do:
/// an empty growable `List<String>` for the first positional parameter
/// (`dart2js` passes no process arguments, not even under Node) and `null`
/// for a second one. Empty when `main` declares no positional parameter.
fn main_arguments(ctx: &Ctx, lib: LibraryId) -> String {
    let Some(sym) = ctx.sym("main") else {
        return String::new();
    };
    let Some(Element::Function(fid)) = ctx
        .program
        .library(lib)
        .declared
        .get(&sym)
        .and_then(|b| b.getter)
    else {
        return String::new();
    };
    let FunctionRef::Function { unit, function } = ctx.program.function(fid).node else {
        return String::new();
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let positional = af
        .parameters
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter(|p| p.kind != ast::ParameterKind::Named)
        .count();
    let (Some(array), Some(string)) = (ctx.js_array, ctx.string_) else {
        return String::new();
    };
    if positional == 0 {
        return String::new();
    }
    let m = ModState::new(lib);
    let e = FnEmitter::new(ctx, &m, unit, None, true);
    let rti = e.rti(&Ty::iface_args(array, vec![Ty::iface(string)]));
    let list = format!("_interceptors.JSArray.of({rti}, [])");
    if positional == 1 {
        list
    } else {
        format!("{list}, null")
    }
}

/// Orders classes: superclasses and mixins before subclasses.
pub(crate) fn order_classes(ctx: &Ctx, classes: &[ClassId]) -> Vec<ClassId> {
    let set: HashSet<ClassId> = classes.iter().copied().collect();
    let mut out = Vec::new();
    let mut done: HashSet<ClassId> = HashSet::new();
    fn visit(
        ctx: &Ctx,
        c: ClassId,
        set: &HashSet<ClassId>,
        done: &mut HashSet<ClassId>,
        out: &mut Vec<ClassId>,
    ) {
        if done.contains(&c) {
            return;
        }
        done.insert(c);
        let class = ctx.program.class(c);
        if let Some(s) = class.supertype_class {
            if set.contains(&s) {
                visit(ctx, s, set, done, out);
            }
        }
        for &m in &ctx.mixins_of(c) {
            if set.contains(&m) {
                visit(ctx, m, set, done, out);
            }
        }
        out.push(c);
    }
    for &c in classes {
        visit(ctx, c, &set, &mut done, &mut out);
    }
    out
}

fn emit_group(ctx: &Ctx, group: &[LibraryId], cache: Option<&RefCell<FragmentCache>>) -> String {
    let lib = group[0];
    let mut m = ModState::new(lib);
    m.group = group.to_vec();
    let info = &ctx.libs[lib.0 as usize];
    let mut body = Writer::default();

    // Classes of all libraries in the group, ordered by hierarchy.
    let in_group = |l: LibraryId| group.contains(&l);
    let mut classes: Vec<ClassId> = Vec::new();
    for (i, c) in ctx.program.classes.iter().enumerate() {
        if in_group(c.library)
            && c.decl.is_some()
            && ctx.class_level(ClassId(i as u32)) != crate::filter::Level::Dead
        {
            classes.push(ClassId(i as u32));
        }
    }
    // `DARTFORGE_MODULE_TIMES=1` prints the cost per module (ordering, classes, rest,
    // rules) and how many fragments came from the cache — that is how we found
    // that an edit re-emitted 1,343 classes of a module with 345 libraries.
    let cron = std::env::var("DARTFORGE_MODULE_TIMES").is_ok();
    let t0 = std::time::Instant::now();
    let ordered = order_classes(ctx, &classes);
    let t_ord = t0.elapsed();
    let t1 = std::time::Instant::now();
    let n_classes = ordered.len();
    let mut reused = 0usize;
    // Each class is emitted in isolation and kept: the module state is pure
    // accumulation and the generated names do not depend on the order, so
    // reusing the text of an untouched class yields the same module.
    for c in ordered {
        m.note_class(c);
        if let Some(cache) = cache {
            if let Some(f) = cache.borrow().map.get(&FragmentKey::Class(c.0)) {
                f.apply_to(&m);
                body.push_raw(&f.text);
                reused += 1;
                continue;
            }
        }
        let mf = m.derived();
        let mut w = Writer::default();
        mf.note_class(c);
        emit_class(ctx, &mf, c, &mut w);
        let text = w.out;
        m.merge(&mf);
        body.push_raw(&text);
        if let Some(cache) = cache {
            cache
                .borrow_mut()
                .map
                .insert(FragmentKey::Class(c.0), Fragment::new(text, &mf));
        }
    }
    let t_cls = t1.elapsed();
    let t2 = std::time::Instant::now();
    for &glib in group {
        if let Some(cache) = cache {
            if let Some(f) = cache.borrow().map.get(&FragmentKey::Rest(glib.0)) {
                f.apply_to(&m);
                body.push_raw(&f.text);
                reused += 1;
                continue;
            }
        }
        let mf = m.derived();
        let mut w = Writer::default();
        emit_library_rest(ctx, &mf, glib, &mut w);
        let text = w.out;
        m.merge(&mf);
        body.push_raw(&text);
        if let Some(cache) = cache {
            cache
                .borrow_mut()
                .map
                .insert(FragmentKey::Rest(glib.0), Fragment::new(text, &mf));
        }
    }
    let t_rest = t2.elapsed();

    // rti rules of the module's classes (and of the referenced ones).
    let t3 = std::time::Instant::now();
    let rules = emit_rules(ctx, &m);
    let t_rules = t3.elapsed();
    if cron {
        eprintln!(
            "[crono] {} libs={} classes={} reused={reused} order={:.1}ms classes={:.1}ms rest={:.1}ms rules={:.1}ms",
            info.module_path,
            group.len(),
            n_classes,
            t_ord.as_secs_f64() * 1e3,
            t_cls.as_secs_f64() * 1e3,
            t_rest.as_secs_f64() * 1e3,
            t_rules.as_secs_f64() * 1e3
        );
    }

    // Prelude.
    let mut out = String::new();
    let mut exports: Vec<String> = Vec::new();
    for &glib in group {
        let gi = &ctx.libs[glib.0 as usize];
        out.push_str(&format!(
            "var {} = Object.create(dart.library);\n",
            gi.js_var
        ));
        exports.push(format!("{} as {}", gi.js_var, gi.ident));
    }
    out.push_str(&format!("export {{ {} }};\n", exports.join(", ")));
    let up = "../".repeat(info.module_path.matches('/').count());
    let mut sdk: BTreeSet<String> = m.sdk_used.borrow().clone();
    for s in ["dart", "dart_rti", "core", "dartx"] {
        sdk.insert(s.to_string());
    }
    sdk.insert("async".to_string());
    let sdk_list: Vec<String> = sdk.iter().cloned().collect();
    out.push_str(&format!(
        "import {{ {} }} from './{up}dart_sdk.js';\n",
        sdk_list.join(", ")
    ));
    for &ul in m.user_imports.borrow().iter() {
        let uinfo = &ctx.libs[ul as usize];
        out.push_str(&format!(
            "import {{ {} as {} }} from './{up}{}';\n",
            uinfo.ident, uinfo.js_var, uinfo.module_path
        ));
    }
    for (var, name) in m.dartx_used.borrow().iter() {
        if js::is_js_ident(name) {
            out.push_str(&format!("var {var} = dartx.{name};\n"));
        } else {
            out.push_str(&format!(
                "var {var} = dartx[{}];\n",
                js::string_literal(name)
            ));
        }
    }
    out.push_str("var _is = dart.privateName(dart_rti, \"_is\");\nvar _as = dart.privateName(dart_rti, \"_as\");\nvar _eval = dart.privateName(dart_rti, \"_eval\");\nvar _bind = dart.privateName(dart_rti, \"_bind\");\n");
    out.push_str("var _current = dart.privateName(async, \"_current\");\nvar _datum = dart.privateName(async, \"_datum\");\nvar _yieldStar = dart.privateName(async, \"_yieldStar\");\n");
    for (var, (l, name)) in m.private_syms.borrow().iter() {
        let lv = &ctx.libs[*l as usize].js_var;
        out.push_str(&format!(
            "var {var} = dart.privateName({lv}, {});\n",
            js::string_literal(name)
        ));
    }
    out.push_str("dart._checkModuleNullSafetyMode(true);\n");
    emit_const_caches(ctx, &m, group, &mut out);
    crate::typed_read_guards::emit(&m, &mut out);
    out.push_str(&body.out);
    out.push_str(&rules);
    let tracked: Vec<String> = group
        .iter()
        .map(|&g| {
            format!(
                "  {}: {}",
                js::string_literal(&ctx.program.library(g).uri),
                ctx.libs[g.0 as usize].js_var
            )
        })
        .collect();
    out.push_str(&format!(
        "dart.trackLibraries({}, {{\n{}\n}}, {{\n}}, null);\n",
        js::string_literal(&info.ident),
        tracked.join(",\n")
    ));
    out
}

/// Keep the development prelude stable; production caches only serve emitted
/// constant records. In particular, an unused cache must not allocate a Map in
/// every imported library. Uses accumulated by cached fragments are included.
pub(crate) fn emit_const_caches(
    ctx: &Ctx,
    m: &ModState,
    libraries: &[LibraryId],
    out: &mut String,
) {
    let prune = ctx.production()
        && std::env::var("DARTFORGE_JSPROD_CONST_CACHES").map_or(true, |v| v != "0");
    let used = m.const_record_libraries.borrow();
    for &lib in libraries {
        if prune && !used.contains(&lib.0) {
            continue;
        }
        let lvar = &ctx.libs[lib.0 as usize].js_var;
        out.push_str(&format!("{lvar}.$constCache = new Map();\n{lvar}.$C = function(k, f) {{ let v = {lvar}.$constCache.get(k); if (v === void 0) {{ v = f(); {lvar}.$constCache.set(k, v); }} return v; }};\n"));
    }
}

/// Top-level functions and variables and extension statics of a library.
pub(crate) fn emit_library_rest(ctx: &Ctx, m: &ModState, lib: LibraryId, body: &mut Writer) {
    emit_library_rest_parts(ctx, m, lib, body, true, true);
}

/// [`emit_library_rest`] in two parts: `with_functions` (top-level functions and
/// accessors — definitions only, nothing runs on load) and `with_rest` (variables
/// and extension statics). The SDK module emits the functions of all libraries
/// before the classes, because the definition of a class runs code
/// (`registerExtension`…) that may call them.
pub(crate) fn emit_library_rest_parts(
    ctx: &Ctx,
    m: &ModState,
    lib: LibraryId,
    body: &mut Writer,
    with_functions: bool,
    with_rest: bool,
) {
    let lvar = ctx.libs[lib.0 as usize].js_var.clone();
    let mut functions: Vec<FunctionElementId> = Vec::new();
    let mut variables: Vec<VariableId> = Vec::new();
    for (i, f) in ctx.program.functions.iter().enumerate() {
        if f.library == lib
            && f.class.is_none()
            && f.kind != FunctionKind::ImplicitAccessor
            && ctx.fn_state(FunctionElementId(i as u32)) != crate::filter::State::Dead
        {
            functions.push(FunctionElementId(i as u32));
        }
    }
    let mut ext_variables: Vec<VariableId> = Vec::new();
    for (i, v) in ctx.program.variables.iter().enumerate() {
        if v.library == lib
            && v.class.is_none()
            && ctx.var_state(VariableId(i as u32)) != crate::filter::State::Dead
        {
            if v.extension.is_none() {
                variables.push(VariableId(i as u32));
            } else {
                ext_variables.push(VariableId(i as u32));
            }
        }
    }
    // Top-level and extension functions; top-level accessors grouped by name.
    let mut accessors: Vec<(String, String)> = Vec::new();
    if with_functions {
        for fid in functions {
            emit_top_function(ctx, m, fid, body, &mut accessors);
        }
        if !accessors.is_empty() {
            crate::emit_line!(body, "dart.copyProperties({lvar}, {{");
            let texts: Vec<String> = accessors.iter().map(|(_, t)| indent(t)).collect();
            body.push_raw(&texts.join(",\n"));
            body.push_raw("\n});\n");
        }
    }
    if !with_rest {
        return;
    }
    // Top-level variables.
    emit_top_variables(ctx, m, lib, &variables, body);
    // Extension statics: `L['Ext|name']`.
    if !ext_variables.is_empty() {
        let mut lazy: Vec<String> = Vec::new();
        for vid in ext_variables {
            let v = ctx.program.variable(vid);
            let Some(ext) = v.extension else { continue };
            let e = ctx.program.extension(ext);
            let tmp = FnEmitter::new(ctx, m, e.decl.unit, None, true);
            let ext_name = tmp.extension_js_name(ext);
            let name = ctx.name(v.name);
            let key = format!("{ext_name}|{name}");
            let VariableRef::Field {
                unit: fu,
                member,
                index,
            } = v.node
            else {
                continue;
            };
            let mem = ctx.program.unit(fu).ast.member(member);
            let MemberKind::Field(list) = &mem.kind else {
                continue;
            };
            let var = &list.variables[index];
            let ty = ctx.var_ty(vid);
            let mut e2 = FnEmitter::new(ctx, m, fu, None, true);
            e2.in_const = v.const_;
            let init = match var.initializer {
                Some(i) => {
                    let (js, _) = e2.emit_expr(i, Some(&ty));
                    let pre = if e2.temps.is_empty() {
                        String::new()
                    } else {
                        format!("let {};\n", e2.temps.join(", "))
                    };
                    format!("{pre}{}return {};", e2.w.out, js.code)
                }
                None => "return null;".to_string(),
            };
            let mut entry = format!(
                "get [{}]() {{\n{}\n}}",
                js::string_literal(&key),
                indent(&init)
            );
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(
                    ",\nset [{}](value) {{}}",
                    js::string_literal(&key)
                ));
            }
            lazy.push(entry);
        }
        if !lazy.is_empty() {
            crate::emit_line!(body, "dart.defineLazy({lvar}, {{");
            body.push_raw(&indent(&lazy.join(",\n")));
            body.push_raw("\n});\n");
        }
    }
}

// ---------------------------------------------------------------------------
// rti rules
// ---------------------------------------------------------------------------

fn emit_rules(ctx: &Ctx, m: &ModState) -> String {
    emit_rules_with(ctx, m, false)
}

/// Like [`emit_rules`]; with `sdk`, the SDK classes as well (the SDK module in
/// SDK mode, `crate::own_sdk`).
pub(crate) fn emit_rules_with(ctx: &Ctx, m: &ModState, sdk: bool) -> String {
    // Only the rti reads the rules.
    if ctx.filter.is_some_and(|f| !f.rti_live()) {
        return String::new();
    }
    let mut entries: Vec<String> = Vec::new();
    let noted: Vec<u32> = m.noted_classes.borrow().iter().copied().collect();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut queue: Vec<ClassId> = noted.into_iter().map(ClassId).collect();
    // Interop types (`liveInterfaceTypeRules`): each one forwards to
    // `LegacyJavaScriptObject`, which receives them as supertypes.
    let mut interop: Vec<String> = Vec::new();
    while let Some(c) = queue.pop() {
        if !seen.insert(c.0) {
            continue;
        }
        let lib = ctx.lib_of_class(c);
        if ctx.libs[lib.0 as usize].is_sdk != sdk {
            continue;
        }
        if ctx.program.class(c).decl.is_none() {
            continue;
        }
        if ctx.interop_ext_types.contains(&c) {
            continue; // erased to `_interceptors|JSObject` in the recipes
        }
        if ctx.is_js_class(c) {
            entries.push(format!(
                "{}:{}",
                json_str(&ctx.class_recipe(c)),
                json_str(&ctx.recipe_of(
                    "dart:_interceptors",
                    "LegacyJavaScriptObject",
                    "_interceptors|LegacyJavaScriptObject"
                ))
            ));
            interop.push(ctx.class_recipe(c));
            continue;
        }
        let class = ctx.program.class(c);
        let params = &ctx.class_params[c.0 as usize];
        let mut items: Vec<String> = Vec::new();
        // Positional type variables: the substitutions are derived from the
        // supertype entries (`Ctx::positional_type_variables`).
        let positional = ctx.positional_type_variables();
        for (i, p) in params.iter().enumerate().filter(|_| !positional) {
            items.push(format!(
                "{}:\"{}\"",
                json_str(&ctx.type_var_key(c, &p.name.to_string())),
                i + 1
            ));
        }
        // Transitive supertypes.
        let this_ty = ctx.this_ty(c);
        let mut sseen: HashSet<u32> = HashSet::new();
        let mut squeue: Vec<Ty> = ctx.direct_supers(&this_ty);
        let mut supers: Vec<Ty> = Vec::new();
        while let Some(s) = squeue.pop() {
            let Some(sc) = s.class() else { continue };
            if Some(sc) == ctx.object || !sseen.insert(sc.0) {
                continue;
            }
            supers.push(s.clone());
            squeue.extend(ctx.direct_supers(&s));
        }
        let _ = class;
        for s in supers {
            let Ty::Iface {
                class: sc, args, ..
            } = &s
            else {
                continue;
            };
            // The supertype's `addRules` takes one argument per parameter it
            // declares; a raw supertype (`implements Box`) counts as
            // `Box<dynamic>` (`Ctx::args_at_arity`).
            let args = ctx.args_at_arity(*sc, args);
            let sparams = &ctx.class_params[sc.0 as usize];
            for (p, a) in sparams.iter().zip(args.iter()).filter(|_| !positional) {
                items.push(format!(
                    "{}:{}",
                    json_str(&ctx.type_var_key(*sc, &p.name.to_string())),
                    json_str(&rule_recipe(ctx, a, c))
                ));
            }
            let arg_recipes: Vec<String> = args
                .iter()
                .map(|a| json_str(&rule_recipe(ctx, a, c)))
                .collect();
            items.push(format!(
                "{}:[{}]",
                json_str(&ctx.class_recipe(*sc)),
                arg_recipes.join(",")
            ));
            if ctx.libs[ctx.lib_of_class(*sc).0 as usize].is_sdk == sdk {
                queue.push(*sc);
            }
        }
        if !items.is_empty() {
            entries.push(format!(
                "{}:{{{}}}",
                json_str(&ctx.class_recipe(c)),
                items.join(",")
            ));
        }
    }
    if entries.is_empty() {
        return String::new();
    }
    let json = format!("{{{}}}", entries.join(","));
    let mut out = format!(
        "dart_rti._Universe.addRules(dart.typeUniverse, JSON.parse({}));\n",
        js::string_literal(&json)
    );
    if !interop.is_empty() {
        interop.sort();
        m.use_sdk("_interceptors");
        let update: Vec<String> = interop
            .iter()
            .map(|r| format!("{}:[]", json_str(r)))
            .collect();
        let ljo = ctx.recipe_of(
            "dart:_interceptors",
            "LegacyJavaScriptObject",
            "_interceptors|LegacyJavaScriptObject",
        );
        out.push_str(&format!(
            "dart_rti._Universe.addOrUpdateRules(dart.typeUniverse, JSON.parse({}));\n",
            js::string_literal(&format!("{{{}:{{{}}}}}", json_str(&ljo), update.join(",")))
        ));
        let names: Vec<String> = interop.iter().map(|r| js::string_literal(r)).collect();
        out.push_str(&format!(
            "dart.addRtiResources(_interceptors.LegacyJavaScriptObject, [{}, {}]);\n",
            js::string_literal(&ljo),
            names.join(", ")
        ));
    }
    out
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Recipe in a class environment (own parameters by index 1..n).
fn rule_recipe(ctx: &Ctx, t: &Ty, class: ClassId) -> String {
    if let Ty::Iface {
        class: c,
        args,
        nullable,
    } = t
    {
        if ctx.interop_ext_types.contains(c) {
            if let Some(e) = ctx.erase_ext(*c, args) {
                let e = if *nullable { e.with_nullable(true) } else { e };
                return rule_recipe(ctx, &e, class);
            }
        }
    }
    match t {
        Ty::Dynamic => "@".into(),
        Ty::Void => "~".into(),
        Ty::Never => "0&".into(),
        Ty::Null => ctx
            .null_
            .map(|n| ctx.class_recipe(n))
            .unwrap_or("core|Null".into()),
        Ty::Iface {
            class: c,
            args,
            nullable,
        } => {
            let mut s = ctx.class_recipe(*c);
            // Declared arity, always (`Ctx::args_at_arity`).
            let args = ctx.args_at_arity(*c, args);
            if !args.is_empty() {
                s.push('<');
                let parts: Vec<String> = args.iter().map(|a| rule_recipe(ctx, a, class)).collect();
                s.push_str(&parts.join(","));
                s.push('>');
            }
            if *nullable {
                s.push('?');
            }
            s
        }
        Ty::Param { id, nullable, .. } => {
            let params = &ctx.class_params[class.0 as usize];
            match params.iter().position(|p| p.id == *id) {
                Some(i) => format!("{}{}", i + 1, if *nullable { "?" } else { "" }),
                None => "@".into(),
            }
        }
        Ty::FutureOr { arg, nullable } => format!(
            "{}/{}",
            rule_recipe(ctx, arg, class),
            if *nullable { "?" } else { "" }
        ),
        Ty::Fn {
            ret,
            pos,
            opt,
            named,
            type_params,
            nullable,
        } => {
            let mut s = rule_recipe(ctx, ret, class);
            s.push('(');
            let mut parts: Vec<String> = pos.iter().map(|a| rule_recipe(ctx, a, class)).collect();
            if !opt.is_empty() {
                parts.push(format!(
                    "[{}]",
                    opt.iter()
                        .map(|a| rule_recipe(ctx, a, class))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            if !named.is_empty() {
                parts.push(format!(
                    "{{{}}}",
                    named
                        .iter()
                        .map(|(n, t, r)| format!(
                            "{n}{}{}",
                            if *r { "!" } else { ":" },
                            rule_recipe(ctx, t, class)
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
            s.push_str(&parts.join(","));
            s.push(')');
            if !type_params.is_empty() {
                s.push('<');
                s.push_str(
                    &type_params
                        .iter()
                        .map(|p| rule_recipe(ctx, &p.bound, class))
                        .collect::<Vec<_>>()
                        .join(","),
                );
                s.push('>');
            }
            if *nullable {
                s.push('?');
            }
            s
        }
        Ty::Record {
            pos,
            named,
            nullable,
        } => {
            let mut s = String::from("+");
            s.push_str(
                &named
                    .iter()
                    .map(|(n, _)| n.clone())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            s.push('(');
            let mut parts: Vec<String> = pos.iter().map(|a| rule_recipe(ctx, a, class)).collect();
            parts.extend(named.iter().map(|(_, t)| rule_recipe(ctx, t, class)));
            s.push_str(&parts.join(","));
            s.push(')');
            if *nullable {
                s.push('?');
            }
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level functions and variables
// ---------------------------------------------------------------------------

/// Emits `function name(params) { body }` (full text) for a function element.
pub(crate) fn function_text(
    ctx: &Ctx,
    m: &ModState,
    fid: FunctionElementId,
    head_name: Option<&str>,
    class: Option<ClassId>,
    is_static: bool,
    extension_this: Option<&Ty>,
) -> (String, AsyncKind) {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else {
        return (String::new(), AsyncKind::None);
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let mut e = FnEmitter::new(ctx, m, unit, class, is_static);
    e.checks = crate::checks::Checks::of_function(ctx, fid);
    let data = &ctx.outline.functions[fid.0 as usize];
    // The function's type parameters.
    let mut tp_js: Vec<String> = Vec::new();
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = e.type_param_js_name(&p.name);
        e.fn_type_params.push((p.id, jsn.clone()));
        tp_js.push(jsn);
    }
    let kind = match af.modifier {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    };
    let ret_ty = ctx.ty_of(data.return_type);
    let sig_ty = ctx.fn_ty(fid);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    let mut extra_prologue = String::new();
    // SDK mode: `@NoReifyGeneric` does not declare the type parameters.
    let mut params_js: Vec<String> = if crate::own_sdk::no_reify_generic_fn(ctx, fid) {
        Vec::new()
    } else {
        tp_js
    };
    if let Some(t) = extension_this {
        // `$this` is the extension's receiver.
        params_js.push("$this".into());
        if let Some(sym) = ctx.sym("this") {
            e.declare_js(sym, "$this".into(), t.clone());
        }
        e.extension_this = Some(t.clone());
    }
    let ps: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologue) = e.declare_params(ps, Some(&sig_ty));
    // SDK mode: `@rest` on the last parameter.
    let pjs = crate::own_sdk::with_rest(&e, ps, pjs);
    if !pjs.is_empty() {
        params_js.push(pjs);
    }
    extra_prologue.push_str(&prologue);
    // SDK mode: `@nullCheck` on the parameters.
    if ctx.sdk.is_some() {
        extra_prologue.push_str(&crate::own_sdk::covariance_checks(&e, ps));
        extra_prologue.push_str(&crate::own_sdk::null_checks(&e, ps));
    }
    extra_prologue.push_str(&crate::own_sdk::user_covariance_checks(&e, fid, ps));
    e.sdk_number_comparison = crate::own_sdk::modulo_number_comparison(&mut e, fid);
    e.emit_body(&af.body);
    let body = finish_body(&mut e);
    let head = match head_name {
        Some(n) => format!("{n}({}) {{", params_js.join(", ")),
        None => format!("function({}) {{", params_js.join(", ")),
    };
    (
        e.wrap_async_head(kind, &head, &extra_prologue, &body, &ret_ty),
        kind,
    )
}

/// `noSuchMethod` forwarder of an abstract method (without the name), in the
/// DDC contract: the CFE synthesizes it with the member's full signature and DDC
/// emits it as a plain method — type parameters first, optional positionals with
/// the declaration's default, named ones read from `opts` with the default — whose
/// body builds the `Invocation` with the types (`createRuntimeType`), **all** the
/// positionals and a `LinkedMap<Symbol, dynamic>` with **all** the named ones, in
/// declaration order:
///
/// ```js
/// pinta(x, opts) {
///   let brilho = opts && 'brilho' in opts ? opts.brilho : false;
///   return String[_as](dart.noSuchMethod(this, new core._Invocation.method(
///     #pinta, null, [x], new _js_helper.LinkedMap.from(LinkedMap<Symbol,dynamic>, [#brilho, brilho]))));
/// }
/// ```
///
/// The return value is cast to the declared type when it does not depend on the
/// class's type parameters (the method's own are in scope).
fn nsm_forwarder(
    ctx: &Ctx,
    m: &ModState,
    c: ClassId,
    class_unit: UnitId,
    fid: FunctionElementId,
    sym: &str,
) -> String {
    let f = ctx.program.function(fid);
    let data = &ctx.outline.functions[fid.0 as usize];
    // The defaults are constants of the library (and class) that declares the
    // abstract member: they resolve in its scope.
    let (unit, declaring_class) = match f.node {
        FunctionRef::Function { unit, .. } => (unit, f.class.unwrap_or(c)),
        _ => (class_unit, c),
    };
    let mut e = FnEmitter::new(ctx, m, unit, Some(declaring_class), false);
    let mut params_js: Vec<String> = Vec::new();
    let mut types: Vec<Ty> = Vec::new();
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = js::ident(&p.name);
        e.fn_type_params.push((p.id, jsn.clone()));
        params_js.push(jsn);
        types.push(Ty::Param {
            id: p.id,
            name: p.name.clone(),
            nullable: false,
        });
    }
    let sig = ctx.fn_ty(fid);
    let (pjs, mut prologue, names) = match f.node {
        FunctionRef::Function { unit, function } => {
            let af = ctx.program.unit(unit).ast.function(function);
            let ps: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
            e.declare_params_with_names(ps, Some(&sig))
        }
        _ => {
            // No node (synthetic): the signature gives the parameters, without defaults.
            let mut names = Vec::new();
            let mut js_ps = Vec::new();
            let mut prologue = String::new();
            if let Ty::Fn {
                pos, opt, named, ..
            } = &sig
            {
                for i in 0..pos.len() {
                    let n = format!("p{i}");
                    js_ps.push(n.clone());
                    names.push((ast::ParameterKind::Required, n.clone(), n));
                }
                for i in 0..opt.len() {
                    let n = format!("p{}", pos.len() + i);
                    js_ps.push(format!("{n} = null"));
                    names.push((ast::ParameterKind::Optional, n.clone(), n));
                }
                for (i, (n, _, _)) in named.iter().enumerate() {
                    let jsn = format!("n{i}");
                    let key = ctx.named_key(n);
                    prologue.push_str(&format!(
                        "let {jsn} = opts && {} ? opts{} : null;
",
                        js::has_named("opts", &key),
                        js::prop_access(&key)
                    ));
                    names.push((ast::ParameterKind::Named, n.clone(), jsn));
                }
                if !named.is_empty() {
                    js_ps.push("opts".into());
                }
            }
            (js_ps.join(", "), prologue, names)
        }
    };
    if !pjs.is_empty() {
        params_js.push(pjs);
    }
    let positionals: Vec<&str> = names
        .iter()
        .filter(|(k, _, _)| *k != ast::ParameterKind::Named)
        .map(|(_, _, j)| j.as_str())
        .collect();
    let named: Vec<(&str, &str)> = names
        .iter()
        .filter(|(k, _, _)| *k == ast::ParameterKind::Named)
        .map(|(_, n, j)| (n.as_str(), j.as_str()))
        .collect();
    let types_js = if types.is_empty() {
        "null".to_string()
    } else {
        format!(
            "[{}]",
            types
                .iter()
                .map(|t| format!("dart_rti.createRuntimeType({})", e.rti(t)))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let map = match ctx.symbol_ {
        Some(symbol) if !named.is_empty() => {
            let (cls, rti) = e.map_impl(&Ty::iface(symbol), &Ty::Dynamic);
            let pairs: Vec<String> = named
                .iter()
                .map(|(n, j)| {
                    format!(
                        "dart.const(new _internal.Symbol.new({})), {j}",
                        js::string_literal(n)
                    )
                })
                .collect();
            format!("new {cls}.from({rti}, [{}])", pairs.join(", "))
        }
        _ => "null".to_string(),
    };
    let ret = ctx.ty_of(data.return_type);
    let mut used = Vec::new();
    ret.collect_params(&mut used);
    let method_only = used.iter().all(|id| {
        types
            .iter()
            .any(|t| matches!(t, Ty::Param { id: p, .. } if p == id))
    });
    // The CFE's `as` on the forwarder's result is an implicit downcast
    // (`isTypeError`), trusted from `-O3` up.
    let cast = if matches!(ret, Ty::Dynamic | Ty::Void) || !method_only || !e.checks.downcasts {
        String::new()
    } else {
        format!("{}[_as]", e.rti(&ret))
    };
    prologue.push_str(&finish_body(&mut e));
    format!(
        "({}) {{
{prologue}return {cast}(dart.noSuchMethod(this, new core._Invocation.method({sym}, {types_js}, [{}], {map})));
}}",
        params_js.join(", "),
        positionals.join(", ")
    )
}

/// Closes the body: declares the temps.
pub(crate) fn finish_body(e: &mut FnEmitter) -> String {
    let mut body = String::new();
    if !e.temps.is_empty() {
        body.push_str(&format!("let {};\n", e.temps.join(", ")));
    }
    body.push_str(&e.w.out);
    body
}

pub(crate) fn emit_top_function(
    ctx: &Ctx,
    m: &ModState,
    fid: FunctionElementId,
    w: &mut Writer,
    accessors: &mut Vec<(String, String)>,
) {
    let f = ctx.program.function(fid);
    // Declaration completed by another one in the augmentation chain: only the
    // effective element is emitted (docs/AUGMENTATIONS.md).
    if f.patched_by.is_some() && (!ctx.program.library(f.library).is_sdk || ctx.sdk.is_some()) {
        return;
    }
    // SDK mode: an `external` without a body (the intrinsics) is never emitted.
    if ctx.sdk.is_some() && crate::own_sdk::external_without_body(ctx, fid) {
        return;
    }
    let lvar = &ctx.libs[f.library.0 as usize].js_var;
    let name = crate::own_sdk::export_name(ctx, fid).unwrap_or(ctx.name(f.name));
    if ctx.is_js_member(fid) {
        return; // interop `external`: direct access to `dart.global`
    }
    if let Some(ext) = f.extension {
        emit_extension_function(ctx, m, fid, ext, w);
        return;
    }
    if ctx.fn_state(fid) == crate::filter::State::Stub {
        let r = js::string_literal(&ctx.pruned_label(f.library, None, name));
        match f.kind {
            FunctionKind::Getter => accessors.push((
                name.to_string(),
                format!(
                    "get {}() {{ return dart_podado({r}); }}",
                    js::prop_key(name)
                ),
            )),
            FunctionKind::Setter => accessors.push((
                name.to_string(),
                format!("set {}(v) {{ dart_podado({r}); }}", js::prop_key(name)),
            )),
            _ => w.line(&format!(
                "{lvar}{} = function(...a) {{ return dart_podado({r}); }};",
                js::prop_access(name)
            )),
        }
        return;
    }
    match f.kind {
        FunctionKind::Getter | FunctionKind::Setter => {
            // Top-level accessors: `dart.copyProperties(L, { get x() {...}, set x(v) {...} })`.
            let (text, _) = function_text(
                ctx,
                m,
                fid,
                Some(&format!(
                    "{} {}",
                    if f.kind == FunctionKind::Getter {
                        "get"
                    } else {
                        "set"
                    },
                    js::prop_key(name)
                )),
                None,
                true,
                None,
            );
            accessors.push((name.to_string(), text));
        }
        _ => {
            let (text, _) = function_text(
                ctx,
                m,
                fid,
                Some(&format!("function {}", js::ident(name))),
                None,
                true,
                None,
            );
            w.line(&format!("{lvar}{} = {text};", js::prop_access(name)));
        }
    }
}

fn emit_extension_function(
    ctx: &Ctx,
    m: &ModState,
    fid: FunctionElementId,
    ext: dartforge_elements::model::ExtensionId,
    w: &mut Writer,
) {
    let f = ctx.program.function(fid);
    let lvar = &ctx.libs[f.library.0 as usize].js_var;
    let name = ctx.name(f.name);
    let e = ctx.program.extension(ext);
    let tmp = FnEmitter::new(ctx, m, e.decl.unit, None, true);
    let ext_name = tmp.extension_js_name(ext);
    let key = match f.kind {
        FunctionKind::Getter => format!("{ext_name}|get#{name}"),
        FunctionKind::Setter => format!("{ext_name}|set#{name}"),
        _ => format!("{ext_name}|{name}"),
    };
    let data = &ctx.outline.extensions[ext.0 as usize];
    let on_ty = ctx.ty_of(data.on);
    // The extension's type parameters come first.
    let ext_tps: Vec<(u32, String)> = data
        .type_params
        .iter()
        .map(|p| {
            let tp = ctx.ty_param_of(*p);
            (tp.id, js::ident(&tp.name))
        })
        .collect();
    let (text, _) = if f.static_ {
        function_text_ext(ctx, m, fid, &ext_tps, None, ext)
    } else {
        function_text_ext(ctx, m, fid, &ext_tps, Some(&on_ty), ext)
    };
    w.line(&format!("{lvar}[{}] = {text};", js::string_literal(&key)));
}

fn function_text_ext(
    ctx: &Ctx,
    m: &ModState,
    fid: FunctionElementId,
    ext_tps: &[(u32, String)],
    this_ty: Option<&Ty>,
    ext: dartforge_elements::model::ExtensionId,
) -> (String, AsyncKind) {
    let f = ctx.program.function(fid);
    let FunctionRef::Function { unit, function } = f.node else {
        return (String::new(), AsyncKind::None);
    };
    let af = ctx.program.unit(unit).ast.function(function);
    let mut e = FnEmitter::new(ctx, m, unit, None, true);
    e.current_extension = Some(ext);
    let data = &ctx.outline.functions[fid.0 as usize];
    let mut params_js: Vec<String> = Vec::new();
    for (id, jsn) in ext_tps {
        e.fn_type_params.push((*id, jsn.clone()));
        params_js.push(jsn.clone());
    }
    for &pid in data.type_params.iter() {
        let p = ctx.ty_param_of(pid);
        let jsn = e.type_param_js_name(&p.name);
        e.fn_type_params.insert(0, (p.id, jsn.clone()));
        params_js.push(jsn);
    }
    let kind = match af.modifier {
        ast::AsyncModifier::None => AsyncKind::None,
        ast::AsyncModifier::Async => AsyncKind::Async,
        ast::AsyncModifier::AsyncStar => AsyncKind::AsyncStar,
        ast::AsyncModifier::SyncStar => AsyncKind::SyncStar,
    };
    let ret_ty = ctx.ty_of(data.return_type);
    let sig_ty = ctx.fn_ty(fid);
    e.async_kind = kind;
    e.ret_ty = ret_ty.clone();
    if let Some(t) = this_ty {
        params_js.push("$this".into());
        e.extension_this = Some(t.clone());
        if let Some(sym) = ctx.sym("this") {
            e.declare_js(sym, "$this".into(), t.clone());
        }
    }
    let ps: &[ast::Parameter] = af.parameters.as_deref().unwrap_or(&[]);
    let (pjs, prologue) = e.declare_params(ps, Some(&sig_ty));
    if !pjs.is_empty() {
        params_js.push(pjs);
    }
    e.emit_body(&af.body);
    let body = finish_body(&mut e);
    let head = format!("function({}) {{", params_js.join(", "));
    (
        e.wrap_async_head(kind, &head, &prologue, &body, &ret_ty),
        kind,
    )
}

pub(crate) fn emit_top_variables(
    ctx: &Ctx,
    m: &ModState,
    lib: LibraryId,
    vars: &[VariableId],
    w: &mut Writer,
) {
    if vars.is_empty() {
        return;
    }
    let lvar = &ctx.libs[lib.0 as usize].js_var;
    let mut lazy: Vec<String> = Vec::new();
    let mut props: Vec<String> = Vec::new();
    let mut eager: Vec<String> = Vec::new();
    for &vid in vars {
        let v = ctx.program.variable(vid);
        let name = ctx.name(v.name);
        if ctx.is_js_var(vid) {
            continue;
        }
        if ctx.var_state(vid) == crate::filter::State::Stub {
            let r = js::string_literal(&ctx.pruned_label(lib, None, name));
            lazy.push(format!(
                "get {}() {{ return dart_podado({r}); }}",
                js::prop_key(name)
            ));
            continue;
        }
        let VariableRef::TopLevel { unit, decl, index } = v.node else {
            continue;
        };
        let d = ctx.program.unit(unit).ast.decl(decl);
        let DeclKind::Variables(list) = &d.kind else {
            continue;
        };
        let var = &list.variables[index];
        let ty = ctx.var_ty(vid);
        let mut e = FnEmitter::new(ctx, m, unit, None, true);
        e.in_const = v.const_;
        let init = match var.initializer {
            Some(i) => {
                let (js, _) = e.emit_expr(i, Some(&ty));
                let mut pre = String::new();
                if !e.temps.is_empty() {
                    pre = format!("let {};\n", e.temps.join(", "));
                }
                format!("{pre}{}return {};", e.w.out, js.code)
            }
            None => "return null;".to_string(),
        };
        if v.late {
            m.use_sdk("_internal");
            let store = format!("{lvar}[{}]", js::string_literal(&format!("_#{name}")));
            let is_set = (ty.is_nullable() || ty.is_dynamic())
                .then(|| format!("{lvar}[{}]", js::string_literal(&format!("_#{name}#isSet"))));
            let (get, set) = late_accessors(
                &store,
                is_set.as_deref(),
                var.initializer.is_some().then_some(init.as_str()),
                v.final_,
                ctx.late_name(name),
                "",
            );
            props.push(format!(
                "get {}() {{\n{}\n}}",
                js::prop_key(name),
                indent(&get)
            ));
            props.push(format!(
                "set {}(v) {{\n{}\n}}",
                js::prop_key(name),
                indent(&set)
            ));
        } else if let Some(lit) = eager_literal(ctx, &init, name) {
            eager.push(format!("{lvar}{} = {lit};", js::prop_access(name)));
        } else {
            let mut entry = format!("get {}() {{\n{}\n}}", js::prop_key(name), indent(&init));
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(",\nset {}(value) {{}}", js::prop_key(name)));
            }
            lazy.push(entry);
        }
    }
    for l in &eager {
        w.line(l);
    }
    if !props.is_empty() {
        crate::emit_line!(w, "dart.copyProperties({lvar}, {{");
        w.push_raw(&indent(&props.join(",\n")));
        w.push_raw("\n});\n");
    }
    if !lazy.is_empty() {
        crate::emit_line!(w, "dart.defineLazy({lvar}, {{");
        w.push_raw(&indent(&lazy.join(",\n")));
        w.push_raw("\n});\n");
    }
}

/// Getter and setter bodies (the setter's parameter is `v`) of a `late`
/// variable stored at `store`, as the CFE lowers it: a nullable type records
/// whether it was set in `is_set`, a non-nullable one uses `null`. `init` is
/// the initializer's function body; a `late final` one rejects an assignment
/// made while the initializer runs (`LateError.fieldADI`). `check` precedes
/// the write (a covariant field's check).
pub(crate) fn late_accessors(
    store: &str,
    is_set: Option<&str>,
    init: Option<&str>,
    final_: bool,
    late_name: &str,
    check: &str,
) -> (String, String) {
    let name = js::string_literal(late_name);
    let getter = match (init, is_set) {
        (Some(init), Some(set)) => {
            let adi = if final_ {
                format!("if ({set}) dart.throw(new _internal.LateError.fieldADI({name})); ")
            } else {
                String::new()
            };
            format!(
                "if (!{set}) {{ let t = (() => {{\n{init}\n}})(); {adi}{store} = t; {set} = true; }}\nreturn {store};"
            )
        }
        (Some(init), None) => {
            let adi = if final_ {
                format!(
                    "if ({store} != null) dart.throw(new _internal.LateError.fieldADI({name})); "
                )
            } else {
                String::new()
            };
            format!(
                "let t = {store};\nif (t == null) {{ t = (() => {{\n{init}\n}})(); {adi}{store} = t; }}\nreturn t;"
            )
        }
        (None, Some(set)) => {
            format!("return {set} ? {store} : dart.throw(new _internal.LateError.fieldNI({name}));")
        }
        (None, None) => format!(
            "let t = {store};\nreturn t == null ? dart.throw(new _internal.LateError.fieldNI({name})) : t;"
        ),
    };
    let mark = is_set
        .map(|set| format!(" {set} = true;"))
        .unwrap_or_default();
    let setter = if final_ {
        let assigned = is_set.map_or_else(|| format!("{store} != null"), str::to_string);
        format!(
            "if ({assigned}) dart.throw(new _internal.LateError.fieldAI({name}));\n{check}{store} = v;{mark}"
        )
    } else {
        format!("{check}{store} = v;{mark}")
    };
    (getter, setter)
}

/// Production profile: the literal that a static or top-level variable's
/// initializer text (`return null;`, `return 0;`, `return "a";`) returns,
/// when there is nothing else in it. Such a variable needs no lazy getter:
/// evaluating the literal at load has no effect, so `C.x = literal` reads and
/// writes the same. `None` in development, for any other initializer and for
/// the names a JS function owns or guards (`length`, `caller`…), which an
/// assignment on a class cannot shadow.
pub(crate) fn eager_literal(ctx: &Ctx, init: &str, name: &str) -> Option<String> {
    if !ctx.production()
        || matches!(
            name,
            "length" | "name" | "prototype" | "caller" | "arguments"
        )
    {
        return None;
    }
    let lit = init.strip_prefix("return ")?.strip_suffix(';')?;
    let number = |t: &str| {
        let t = t.strip_prefix('-').unwrap_or(t);
        !t.is_empty()
            && t.chars().all(|c| {
                c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-'
            })
            && t.starts_with(|c: char| c.is_ascii_digit())
    };
    let string = lit.len() >= 2
        && lit.starts_with('"')
        && lit.ends_with('"')
        && !lit[1..lit.len() - 1].contains(['"', '\\', '\n']);
    (matches!(lit, "null" | "true" | "false") || number(lit) || string).then(|| lit.to_string())
}

pub(crate) fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

/// Instance field and how it is stored.
struct FieldInfo {
    vid: VariableId,
    /// Emits a getter/setter pair (virtual field).
    virtual_: bool,
    /// Private `late` without an initializer through the `dart.lateField` helper
    /// (production profile): no visible storage, no `= null` in the constructor.
    helper: bool,
    name: String,
    /// Storage symbol (`this[sym]`); `None` → direct property.
    storage: Option<String>,
    /// `late` with a nullable type: the symbol of its "was set" flag.
    is_set: Option<String>,
    late: bool,
    final_: bool,
    init: Option<ast::ExprId>,
    unit: UnitId,
    ty: Ty,
}

/// Does `text` mention `name` as a free identifier (not after `.`, nor glued to
/// another identifier)? Approximate: an occurrence inside a *string* also counts,
/// and then it is merely renamed without need.
fn mentions_free_name(text: &str, name: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + name.len()..].chars().next();
        !before.is_some_and(|c| ident(c) || c == '.') && !after.is_some_and(ident)
    })
}

pub(crate) fn emit_class(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) {
    let class = ctx.program.class(c);
    let Some(decl) = class.decl else { return };
    let unit = decl.unit;
    let lvar = ctx.libs[class.library.0 as usize].js_var.clone();
    let cname = ctx.class_name(c).to_string();
    let cref = format!("{lvar}.{cname}");
    let is_enum = class.kind == ClassKind::Enum;
    let is_mixin = class.kind == ClassKind::Mixin;
    if class.kind == ClassKind::ExtensionType && !ctx.is_js_class(c) {
        // Erased extension type: support object with static functions.
        crate::extension_type::emit_extension_type(ctx, m, c, w);
        return;
    }
    if ctx.is_js_class(c) {
        emit_js_interop_class(ctx, m, c, w);
        return;
    }
    let generic = ctx.requires_rti(c);
    let d = ctx.program.unit(unit).ast.decl(decl.decl);
    // The enum values, each with its unit: those of the declaration and then
    // those of the augmentations, in application order (docs/AUGMENTATIONS.md).
    let enum_constants: Vec<(UnitId, &ast::EnumConstant)> = match &d.kind {
        DeclKind::Enum(ed) => {
            let mut v: Vec<(UnitId, &ast::EnumConstant)> =
                ed.constants.iter().map(|k| (unit, k)).collect();
            for a in ctx.program.augmentations.get(&c).into_iter().flatten() {
                if let DeclKind::Enum(ea) = &ctx.program.unit(a.unit).ast.decl(a.decl).kind {
                    v.extend(ea.constants.iter().map(|k| (a.unit, k)));
                }
            }
            v
        }
        _ => vec![],
    };
    // The members of the declaration and of its augmentations
    // (docs/AUGMENTATIONS.md), each with its unit.
    let members: Vec<(UnitId, ast::MemberId)> = match &d.kind {
        DeclKind::Class(_) | DeclKind::Mixin(_) | DeclKind::Enum(_) => {
            crate::own_sdk::class_members(ctx, c)
        }
        _ => vec![],
    };

    // Instance fields.
    let native_names = if ctx.sdk.is_some() {
        native_member_names(ctx, c)
    } else {
        HashSet::new()
    };
    let mut fields: Vec<FieldInfo> = Vec::new();
    for &vid in &class.fields {
        let v = ctx.program.variable(vid);
        if v.static_ {
            continue;
        }
        let name = ctx.name(v.name).to_string();
        let VariableRef::Field {
            unit: fu,
            member,
            index,
        } = v.node
        else {
            continue;
        };
        let mem = ctx.program.unit(fu).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else {
            continue;
        };
        // An abstract field only declares a getter and a setter.
        if list.abstract_ {
            continue;
        }
        let var = &list.variables[index];
        let private = name.starts_with('_');
        // Production profile: a public field that nobody overrides is a plain
        // property (`own_sdk::non_virtual_field`).
        // A member name of a native supertype stays an accessor:
        // `defineExtensionAccessors` copies the *getter* to the `dartx` key, which
        // is where it is read from.
        let plain = !private
            && !is_enum
            && !v.late
            && !native_names.contains(&name)
            && crate::own_sdk::non_virtual_field(ctx, c, v.name);
        let helper =
            v.late && var.initializer.is_none() && ctx.sdk.is_some() && private && !is_mixin;
        // A private field that overrides an inherited member of its library
        // (a getter, or a `noSuchMethod` forwarder) is an accessor pair too.
        let private_override = private && !v.late && private_field_is_virtual(ctx, c, &name);
        let storage = if helper {
            None
        } else if v.late {
            Some(m.private_sym(ctx, class.library, &format!("_#{cname}#{name}")))
        } else if private && !private_override {
            Some(m.private_sym(ctx, class.library, &name))
        } else if is_enum || plain {
            None
        } else {
            Some(m.private_sym(ctx, class.library, &format!("{cname}.{name}")))
        };
        let virtual_ = v.late || private_override || (!private && !is_enum && !plain);
        let ty = ctx.var_ty(vid);
        // A nullable `late` field records whether it was set (`null` is a
        // value), as the CFE lowers it.
        let is_set = (v.late && !helper && (ty.is_nullable() || ty.is_dynamic()))
            .then(|| m.private_sym(ctx, class.library, &format!("_#{cname}#{name}#isSet")));
        fields.push(FieldInfo {
            vid,
            name,
            virtual_,
            helper,
            storage,
            is_set,
            late: v.late,
            final_: v.final_,
            init: var.initializer,
            unit: fu,
            ty,
        });
    }

    // JS superclass.
    let super_ref = superclass_js(ctx, m, c, w);

    // Class body.
    let mut cw = Writer {
        indent: 1,
        ..Writer::default()
    };
    let mut mixin_statics = Writer {
        indent: 1,
        ..Writer::default()
    };
    let mut ext_methods: Vec<String> = Vec::new();
    let mut ext_accessors: Vec<String> = Vec::new();
    // SDK mode: a native class declares its members through the `dartx` symbol
    // and has no `defineExtensionMethods` (`property_model.dart:283`).
    let native_class = crate::own_sdk::is_native_class(ctx, c);
    let natives = if native_class {
        HashSet::new()
    } else {
        native_member_names(ctx, c)
    };
    if let Some(k) = crate::own_sdk::special_constructor(ctx, c) {
        for l in k.lines() {
            cw.line(l);
        }
    }

    // `late` fields through the `dart.lateField` helper (production profile).
    // (key, name, final) — the key is the JS expression of the private name.
    let mut late_fields: Vec<(String, String, bool)> = Vec::new();
    // Getters/setters of virtual fields.
    for f in &fields {
        if !f.virtual_ {
            continue;
        }
        if ctx.filter.is_some() {
            // The field's accessors: live if the implicit getter or setter is live.
            let vv = ctx.program.variable(f.vid);
            let live = |x: Option<FunctionElementId>| {
                x.is_some_and(|g| ctx.fn_state(g) != crate::filter::State::Dead)
            };
            if !live(vv.getter) && !live(vv.setter) {
                continue;
            }
        }
        if f.helper {
            // Production profile: the `get`/`set` pair with `LateError` comes
            // from a *bootstrap* helper (`dart.lateField`, which creates the
            // storage symbol), one call per field after the class
            // (`docs/JS-PRODUCAO-TAMANHO.md` §3.4).
            let key = m.private_sym(ctx, class.library, &f.name);
            late_fields.push((key, f.name.clone(), f.final_));
            continue;
        }
        if let Some(sym) = &f.storage {
            // A final field has no setter of its own; the forwarding one
            // keeps an inherited setter reachable. In production it is only
            // written when a superclass or a mixin declares that setter
            // (`DARTFORGE_JSPROD_FINAL_SETTERS=0` writes it always).
            let forward_setter = !ctx.production()
                || std::env::var("DARTFORGE_JSPROD_FINAL_SETTERS").is_ok_and(|v| v == "0")
                || class
                    .supertype_class
                    .and_then(|sc| ctx.lookup_member(&ctx.this_ty(sc), &f.name, true))
                    .is_some()
                || ctx.mixins_of(c).iter().any(|mx| {
                    ctx.lookup_member(&ctx.this_ty(*mx), &f.name, true)
                        .is_some()
                });
            let key = if f.name.starts_with('_') {
                format!("[{}]", m.private_sym(ctx, class.library, &f.name))
            } else {
                js::prop_key(&crate::body::js_member_name(&f.name))
            };
            // A covariant field: its setter checks the value against the
            // declared type (`Fields<Object> c = Fields<int>(); c.x = 's'`).
            let covariant_check = if crate::own_sdk::covariant_field(ctx, c, f.vid)
                && !ctx.libs[class.library.0 as usize].is_sdk
                && crate::checks::Checks::of_variable(ctx, f.vid).parameters
                && !matches!(f.ty, Ty::Dynamic)
            {
                let e = FnEmitter::new(ctx, m, f.unit, Some(c), false);
                Some(e.rti(&f.ty))
            } else {
                None
            };
            if f.late {
                m.use_sdk("_internal");
                let mut e = FnEmitter::new(ctx, m, f.unit, Some(c), false);
                let init = f.init.map(|i| {
                    let (js, _) = e.emit_expr(i, Some(&f.ty));
                    let pre = if e.temps.is_empty() {
                        String::new()
                    } else {
                        format!("let {};\n", e.temps.join(", "))
                    };
                    format!("{pre}{}return {};", e.w.out, js.code)
                });
                let is_set = f.is_set.as_ref().map(|set| format!("this[{set}]"));
                let check = covariant_check
                    .as_ref()
                    .map(|r| format!("{r}[_as](v); "))
                    .unwrap_or_default();
                let (get, set) = late_accessors(
                    &format!("this[{sym}]"),
                    is_set.as_deref(),
                    init.as_deref(),
                    f.final_,
                    ctx.late_name(&f.name),
                    &check,
                );
                crate::emit_line!(cw, "get {key}() {{");
                for l in get.lines() {
                    cw.line(&format!("  {l}"));
                }
                cw.line("}");
                if f.final_ && f.init.is_some() {
                    // No setter of its own: an inherited one, if any.
                    let sup = if key.starts_with('[') {
                        key.clone()
                    } else {
                        js::prop_access(&crate::body::js_member_name(&f.name))
                    };
                    if forward_setter {
                        crate::emit_line!(cw, "set {key}(v) {{ super{sup} = v; }}");
                    }
                } else {
                    crate::emit_line!(cw, "set {key}(v) {{");
                    for l in set.lines() {
                        cw.line(&format!("  {l}"));
                    }
                    cw.line("}");
                }
            } else {
                crate::emit_line!(cw, "get {key}() {{ return this[{sym}]; }}");
                if f.final_ {
                    if forward_setter {
                        cw.line(&format!(
                            "set {key}(value) {{ super{} = value; }}",
                            if f.name.starts_with('_') {
                                format!("[{sym}]")
                            } else {
                                js::prop_access(&crate::body::js_member_name(&f.name))
                            }
                        ));
                    }
                } else if let Some(check) = &covariant_check {
                    crate::emit_line!(
                        cw,
                        "set {key}(value) {{ {check}[_as](value); this[{sym}] = value; }}"
                    );
                } else {
                    crate::emit_line!(cw, "set {key}(value) {{ this[{sym}] = value; }}");
                }
            }
            if !f.name.starts_with('_') && natives.contains(&f.name) {
                ext_accessors.push(f.name.clone());
            }
        }
    }

    // Enum: `_enumToString`.
    if is_enum {
        m.use_sdk("core");
        let name_sym = m.private_sym(ctx, ctx.program.core.expect("core"), "_name");
        let ets = m.private_sym(ctx, ctx.program.core.expect("core"), "_enumToString");
        cw.line(&format!(
            "[{ets}]() {{ return {} + this[{name_sym}]; }}",
            js::string_literal(&format!("{cname}."))
        ));
    }

    // Methods, getters, setters, operators, statics, factories.
    let mut method_sigs: Vec<(String, Ty)> = Vec::new();
    let mut getter_sigs: Vec<(String, Ty)> = Vec::new();
    let mut setter_sigs: Vec<(String, Ty)> = Vec::new();
    let mut static_methods: Vec<String> = Vec::new();
    let mut generic_methods: Vec<(String, Vec<Ty>)> = Vec::new();
    let mut ctors: Vec<(ast::MemberId, ast::Constructor)> = Vec::new();
    let _ = &mut ctors;
    let mut ctor_members: Vec<(UnitId, ast::MemberId)> = Vec::new();
    let mut has_equals = false;
    for &(mu, mid) in &members {
        let ast = &ctx.program.unit(mu).ast;
        let mem = ast.member(mid);
        match &mem.kind {
            MemberKind::Method(fid) => {
                let af = ast.function(*fid);
                let Some(fname) = af.name else { continue };
                let mut name = ctx.name(fname.sym).to_string();
                if af.kind == ast::FunctionKind::Operator
                    && name == "-"
                    && af.parameters.as_ref().is_some_and(|p| p.is_empty())
                {
                    name = "unary-".to_string();
                }
                let key = if af.kind == ast::FunctionKind::Setter {
                    format!("{name}_=")
                } else {
                    name.clone()
                };
                let ksym = ctx.sym(&key);
                let fe = ksym.and_then(|s| {
                    if af.static_ {
                        class.static_members.get(&s)
                    } else {
                        class.instance_members.get(&s)
                    }
                    .copied()
                });
                let Some(feid) = fe else { continue };
                // Member completed by another declaration of the augmentation
                // chain: the effective element is that one's.
                if ctx.program.function(feid).node
                    != (FunctionRef::Function {
                        unit: mu,
                        function: *fid,
                    })
                {
                    continue;
                }
                if matches!(af.body, FunctionBody::Empty) && !af.external {
                    continue; // abstract
                }
                let state = ctx.fn_state(feid);
                if state == crate::filter::State::Dead {
                    continue;
                }
                // SDK mode: a static `external` without a body does not exist in
                // JS (the access is `dart.global.Peer.x`); an instance one only in
                // a native class.
                let native_external = native_class
                    && !af.static_
                    && (af.external || matches!(af.body, FunctionBody::Native(_)));
                if ctx.sdk.is_some()
                    && !native_external
                    && crate::own_sdk::external_without_body(ctx, feid)
                {
                    continue;
                }
                let jsname = js_member_name(&name);
                let e_tmp = FnEmitter::new(ctx, m, mu, Some(c), af.static_);
                let key_js = e_tmp.decl_member_key(c, &name);
                let data = &ctx.outline.functions[feid.0 as usize];
                let fty = ctx.fn_ty(feid);
                let is_generic = !data.type_params.is_empty();
                let head = if af.static_ {
                    let sk = js::prop_key(&static_member_name(&name));
                    match af.kind {
                        ast::FunctionKind::Getter => format!("static get {sk}"),
                        ast::FunctionKind::Setter => format!("static set {sk}"),
                        _ => format!("static {sk}"),
                    }
                } else {
                    match af.kind {
                        ast::FunctionKind::Getter => format!("get {key_js}"),
                        ast::FunctionKind::Setter => format!("set {key_js}"),
                        _ => {
                            if name.starts_with('_') || native_class {
                                key_js.clone()
                            } else {
                                js::prop_key(&jsname)
                            }
                        }
                    }
                };
                // SDK mode: `@JSExportName('Symbol.iterator')` on a member.
                let head = match crate::own_sdk::export_name(ctx, feid) {
                    Some(x) if !af.static_ && x.contains('.') => format!("[{x}]"),
                    Some(x) if !af.static_ => js::prop_key(x),
                    _ => head,
                };
                if state == crate::filter::State::Stub {
                    let r = js::string_literal(&ctx.pruned_label(class.library, Some(c), &name));
                    match af.kind {
                        ast::FunctionKind::Getter => {
                            cw.line(&format!("{head}() {{ return dart_podado({r}); }}"))
                        }
                        ast::FunctionKind::Setter => {
                            cw.line(&format!("{head}(v) {{ dart_podado({r}); }}"))
                        }
                        _ => cw.line(&format!("{head}(...a) {{ return dart_podado({r}); }}")),
                    }
                    continue;
                }
                let (text, _) = if native_external {
                    (
                        crate::own_sdk::native_external_member(ctx, &head, feid, &name, af.kind),
                        crate::body::AsyncKind::None,
                    )
                } else {
                    function_text(ctx, m, feid, Some(&head), Some(c), af.static_, None)
                };
                let text = if name == "==" && !af.static_ {
                    has_equals = true;
                    // `_equals(other)`: checks for null first.
                    text.replacen("{\n", "{\n  if (other == null) return false;\n", 1)
                } else {
                    text
                };
                let text = if name == "==" {
                    fix_equals_param(&text)
                } else {
                    text
                };
                // A `mixin` static goes to the declared class (the shell), not to
                // the `mixinOn` one: that is where `M.f()` finds it.
                let dest = if af.static_ && is_mixin {
                    &mut mixin_statics
                } else {
                    &mut cw
                };
                for line in text.lines() {
                    dest.line(line);
                }
                if af.static_ {
                    if af.kind == ast::FunctionKind::Function {
                        static_methods.push(name.clone());
                    }
                } else {
                    match af.kind {
                        ast::FunctionKind::Getter => {
                            let rt = ctx.ty_of(data.return_type);
                            getter_sigs.push((name.clone(), rt));
                            if natives.contains(&name) {
                                ext_accessors.push(name.clone());
                            }
                        }
                        ast::FunctionKind::Setter => {
                            let pt = data
                                .parameters
                                .first()
                                .map(|p| ctx.ty_of(p.ty))
                                .unwrap_or(Ty::Dynamic);
                            setter_sigs.push((name.clone(), pt));
                            if natives.contains(&name) {
                                ext_accessors.push(name.clone());
                            }
                        }
                        _ => {
                            method_sigs.push((
                                jsname.clone(),
                                crate::own_sdk::reified_method_type(ctx, c, feid, fty.clone()),
                            ));
                            if is_generic {
                                // Instantiate to bounds: a bound over the
                                // class's parameters is read from the
                                // receiver's instantiation (`_ti`); the
                                // method's own parameters in a bound take
                                // their own defaults.
                                let own: Vec<crate::ty::TyParam> = data
                                    .type_params
                                    .iter()
                                    .map(|p| ctx.ty_param_of(*p))
                                    .collect();
                                let class_ids: Vec<u32> = ctx.class_params[c.0 as usize]
                                    .iter()
                                    .map(|p| p.id)
                                    .collect();
                                let mut defaults: Vec<Ty> = Vec::new();
                                for tp in &own {
                                    let map: HashMap<u32, Ty> = own
                                        .iter()
                                        .zip(
                                            defaults
                                                .iter()
                                                .cloned()
                                                .chain(std::iter::repeat(Ty::Dynamic)),
                                        )
                                        .map(|(q, d)| (q.id, d))
                                        .collect();
                                    let bound = tp.bound.subst(&map);
                                    let mut free = Vec::new();
                                    bound.collect_params(&mut free);
                                    let d = if crate::own_sdk::is_top(ctx, &bound)
                                        || free.iter().any(|id| !class_ids.contains(id))
                                    {
                                        Ty::Dynamic
                                    } else {
                                        bound
                                    };
                                    defaults.push(d);
                                }
                                generic_methods.push((jsname.clone(), defaults));
                            }
                            if natives.contains(&name) || (name == "==" && !native_class) {
                                ext_methods.push(jsname.clone());
                            }
                        }
                    }
                }
            }
            MemberKind::Constructor(ctor) => {
                let effective = ctor
                    .name
                    .map(|n| n.sym)
                    .or(ctx.empty_sym)
                    .and_then(|s| class.constructors.get(&s).copied());
                if effective.is_some_and(|f| {
                    ctx.program.function(f).node
                        != (FunctionRef::Constructor {
                            unit: mu,
                            member: mid,
                        })
                }) {
                    continue;
                }
                let state = effective.map_or(crate::filter::State::Live, |f| ctx.fn_state(f));
                if state == crate::filter::State::Dead {
                    continue;
                }
                if ctor.factory && state == crate::filter::State::Stub {
                    let n = ctor
                        .name
                        .map(|n| ctx.name(n.sym).to_string())
                        .unwrap_or("new".into());
                    let r = js::string_literal(&ctx.pruned_label(class.library, Some(c), &n));
                    cw.line(&format!(
                        "static {}(...a) {{ return dart_podado({r}); }}",
                        js::prop_key(&static_member_name(&n))
                    ));
                    static_methods.push(n);
                } else if ctor.factory {
                    emit_factory(ctx, m, c, mu, ctor, mid, &mut cw);
                    static_methods.push(
                        ctor.name
                            .map(|n| ctx.name(n.sym).to_string())
                            .unwrap_or("new".into()),
                    );
                } else {
                    ctor_members.push((mu, mid));
                }
            }
            MemberKind::Field(_) => {}
        }
    }
    let _ = has_equals;

    // `[Symbol.iterator]` for Iterable classes whose superclass is not Iterable.
    if !is_mixin && !native_class && ctx.class_level(c) == crate::filter::Level::Instantiated {
        if let Some(it) = ctx.iterable_ {
            let declares_iterator = ctx
                .sym("iterator")
                .is_some_and(|s| class.instance_members.contains_key(&s));
            let super_is_iterable = ctx
                .superclass_of(c)
                .is_some_and(|sc| ctx.is_subclass(sc, it) && Some(sc) != ctx.object);
            if declares_iterator && ctx.is_subclass(c, it) && !super_is_iterable {
                cw.line(&format!(
                    "[Symbol.iterator]() {{ return new dart.JsIterator(this[{}]); }}",
                    m.dartx("iterator")
                ));
            }
        }
    }

    // `noSuchMethod` forwarders in concrete classes with abstract members.
    // Without a `noSuchMethod`, only the throwers of the private members of
    // another library's supertype (`class Foo implements lib.Bar {}`).
    let user_nsm = ctx.has_user_nsm(c);
    if !class.modifiers.abstract_
        && !is_mixin
        && (user_nsm || foreign_private_supertype(ctx, c))
        && ctx.class_level(c) == crate::filter::Level::Instantiated
    {
        m.use_sdk("_internal");
        m.use_sdk("core");
        let mut se = FnEmitter::new(ctx, m, unit, Some(c), false);
        for (name, mk) in ctx.unimplemented_abstract(c) {
            // A private member forwards only within its own library; one of
            // another library is not accessible here.
            let declaring = match mk {
                crate::ctx::MemberKind::Method(f)
                | crate::ctx::MemberKind::Getter(f)
                | crate::ctx::MemberKind::Setter(f) => Some(ctx.program.function(f).library),
                crate::ctx::MemberKind::Field(_) => None,
            };
            if name == "noSuchMethod" {
                continue;
            }
            let inaccessible = name.starts_with('_') && declaring != Some(class.library);
            if !user_nsm && !inaccessible {
                continue;
            }
            // A superclass of the declaring library that has the member in its
            // interface already forwards it.
            let owner = match mk {
                crate::ctx::MemberKind::Method(f)
                | crate::ctx::MemberKind::Getter(f)
                | crate::ctx::MemberKind::Setter(f) => ctx.program.function(f).class,
                crate::ctx::MemberKind::Field(_) => None,
            };
            if inaccessible
                && let Some(owner) = owner
                && std::iter::successors(ctx.superclass_of(c), |&k| ctx.superclass_of(k)).any(|k| {
                    Some(ctx.lib_of_class(k)) == declaring
                        && !ctx.program.class(k).modifiers.abstract_
                        && ctx.has_user_nsm(k)
                        && ctx.as_super(&ctx.this_ty(k), owner).is_some()
                })
            {
                continue;
            }
            // Abstract setter: the forwarder exists only if someone writes;
            // getter and method, if someone reads or calls.
            let live = match mk {
                crate::ctx::MemberKind::Setter(_) => ctx.write_selector(&name),
                _ => ctx.selector_live(&name),
            };
            if !live {
                continue;
            }
            let sym = format!(
                "dart.const(new _internal.Symbol.new({}))",
                js::string_literal(&name)
            );
            // A private member of another library: a thrower, not a forwarder
            // (the class's `noSuchMethod` cannot see the name).
            if inaccessible && let Some(lib) = declaring {
                let key = format!("[{}]", se.private_sym(lib, &name));
                let throw = |invocation: String| {
                    format!(
                        "return dart.throw(new core.NoSuchMethodError._withInvocation(this, {invocation}));"
                    )
                };
                // Signed under the declaring library's key, so that dynamic
                // accesses find the thrower too (`RAW_SIG_KEY`).
                let raw = format!("{RAW_SIG_KEY}{name}{RAW_SIG_KEY}{key}");
                match mk {
                    crate::ctx::MemberKind::Method(f) => {
                        cw.line(&format!(
                            "{key}(...args) {{ {} }}",
                            throw(format!(
                                "new core._Invocation.method({sym}, null, args, null)"
                            ))
                        ));
                        method_sigs.push((raw, ctx.fn_ty(f)));
                    }
                    crate::ctx::MemberKind::Getter(f) => {
                        cw.line(&format!(
                            "get {key}() {{ {} }}",
                            throw(format!("new core._Invocation.getter({sym})"))
                        ));
                        let ret = ctx.ty_of(ctx.outline.functions[f.0 as usize].return_type);
                        getter_sigs.push((raw, ret));
                    }
                    crate::ctx::MemberKind::Setter(f) => {
                        cw.line(&format!(
                            "set {key}(v) {{ {} }}",
                            throw(format!(
                                "new core._Invocation.setter(dart.const(new _internal.Symbol.new({})), v)",
                                js::string_literal(&format!("{name}="))
                            ))
                        ));
                        let value = ctx.outline.functions[f.0 as usize]
                            .parameters
                            .first()
                            .map_or(Ty::Dynamic, |p| ctx.ty_of(p.ty));
                        setter_sigs.push((raw, value));
                    }
                    crate::ctx::MemberKind::Field(_) => {}
                }
                continue;
            }
            match mk {
                crate::ctx::MemberKind::Method(fid) => {
                    let jsname = js_member_name(&name);
                    let text = nsm_forwarder(ctx, m, c, unit, fid, &sym);
                    cw.line(&format!("{}{text}", sig_key(&jsname, &se, c)));
                    method_sigs.push((jsname.clone(), ctx.fn_ty(fid)));
                    // A generic forwarder's default type arguments (`dart.bind`
                    // and dynamic calls read them).
                    let data = &ctx.outline.functions[fid.0 as usize];
                    if !data.type_params.is_empty() {
                        let defaults = data
                            .type_params
                            .iter()
                            .map(|p| {
                                // An unbounded parameter defaults to `dynamic`.
                                let bound = *ctx.ty_param_of(*p).bound;
                                if bound.mentions_params() || crate::own_sdk::is_top(ctx, &bound) {
                                    Ty::Dynamic
                                } else {
                                    bound
                                }
                            })
                            .collect();
                        generic_methods.push((jsname.clone(), defaults));
                    }
                    if natives.contains(&name) {
                        ext_methods.push(jsname);
                    }
                }
                crate::ctx::MemberKind::Getter(fid) => {
                    let ret = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
                    let cast = if matches!(ret, Ty::Dynamic | Ty::Void)
                        || ret.mentions_params()
                        || !se.checks.downcasts
                    {
                        String::new()
                    } else {
                        format!("{}[_as]", se.rti(&ret))
                    };
                    cw.line(&format!("get {}() {{ return {cast}(dart.noSuchMethod(this, new core._Invocation.getter({sym}))); }}", sig_key(&name, &se, c)));
                    getter_sigs.push((name.clone(), ret));
                    if natives.contains(&name) {
                        ext_accessors.push(name.clone());
                    }
                }
                crate::ctx::MemberKind::Setter(fid) => {
                    let ssym = format!(
                        "dart.const(new _internal.Symbol.new({}))",
                        js::string_literal(&format!("{name}="))
                    );
                    cw.line(&format!("set {}(v) {{ dart.noSuchMethod(this, new core._Invocation.setter({ssym}, v)); }}", sig_key(&name, &se, c)));
                    // Signed, so that a dynamic write checks the value.
                    // An implicit setter (a field of the interface) has the
                    // field's type.
                    let value = match ctx.program.function(fid).variable {
                        Some(v) => ctx.var_ty(v),
                        None => ctx.outline.functions[fid.0 as usize]
                            .parameters
                            .first()
                            .map_or(Ty::Dynamic, |p| ctx.ty_of(p.ty)),
                    };
                    if !value.mentions_params() {
                        setter_sigs.push((name.clone(), value));
                    }
                }
                crate::ctx::MemberKind::Field(_) => {}
            }
        }
        let _ = &mut se;
    }

    // Setter forwarding stubs: a mixin's setter under a covariant supertype
    // setter checks the value; the getter keeps forwarding.
    if !is_mixin {
        let se = FnEmitter::new(ctx, m, unit, Some(c), false);
        for (name, ty) in crate::own_sdk::setter_covariance_stubs(ctx, c) {
            let key = sig_key(&name, &se, c);
            let access = se.member_access(&ctx.this_ty(c), &name, true);
            cw.line(&format!(
                "set {key}(v) {{ {}[_as](v); super{access} = v; }}",
                se.rti(&ty)
            ));
            let getter = se.member_access(&ctx.this_ty(c), &name, false);
            cw.line(&format!("get {key}() {{ return super{getter}; }}"));
        }
    }

    // Covariance forwarding stubs: an inherited implementation that does not
    // check a parameter this class's interface makes covariant.
    if !is_mixin {
        let se = FnEmitter::new(ctx, m, unit, Some(c), false);
        let this_ty = ctx.this_ty(c);
        for stub in crate::own_sdk::covariance_stubs(ctx, c) {
            if !ctx.selector_live(&stub.name) {
                continue;
            }
            let data = &ctx.outline.functions[stub.implementation.0 as usize];
            let offset = if crate::own_sdk::no_reify_generic_fn(ctx, stub.implementation) {
                0
            } else {
                data.type_params.len()
            };
            let positional = data
                .parameters
                .iter()
                .filter(|p| p.kind != ast::ParameterKind::Named)
                .count();
            let mut checks = String::new();
            for (param, ty) in &stub.checks {
                let rti = se.rti(ty);
                match param {
                    crate::own_sdk::StubParam::Positional(i) => {
                        let a = format!("args[{}]", offset + i);
                        if data.parameters[*i].kind == ast::ParameterKind::Required {
                            checks.push_str(&format!("{rti}[_as]({a}); "));
                        } else {
                            checks.push_str(&format!("if ({a} !== void 0) {rti}[_as]({a}); "));
                        }
                    }
                    crate::own_sdk::StubParam::Named(n) => {
                        let o = format!("args[{}]", offset + positional);
                        let k = js::prop_access(&ctx.named_key(n));
                        checks.push_str(&format!(
                            "if ({o} != null && {o}{k} !== void 0) {rti}[_as]({o}{k}); "
                        ));
                    }
                }
            }
            let jsname = js_member_name(&stub.name);
            let access = se.member_access(&this_ty, &stub.name, false);
            cw.line(&format!(
                "{}(...args) {{ {checks}return super{access}(...args); }}",
                if access.starts_with('[') {
                    access.clone()
                } else {
                    js::prop_key(&jsname)
                }
            ));
            // Reified with the checked parameters as `Object?`.
            let object_q = ctx.t_object_q();
            let reified = match stub.signature {
                Ty::Fn {
                    type_params,
                    ret,
                    mut pos,
                    mut opt,
                    mut named,
                    nullable,
                } => {
                    // The parameters the implementation already checks
                    // (`covariant` there) are `Object?` too.
                    let mut positional = 0;
                    let own: Vec<crate::own_sdk::StubParam> =
                        crate::own_sdk::ast_params(ctx, stub.implementation)
                            .iter()
                            .filter_map(|p| {
                                let named = p.kind == ast::ParameterKind::Named;
                                let index = positional;
                                if !named {
                                    positional += 1;
                                }
                                if !p.covariant {
                                    return None;
                                }
                                Some(if named {
                                    crate::own_sdk::StubParam::Named(
                                        ctx.name(p.name?.sym).to_string(),
                                    )
                                } else {
                                    crate::own_sdk::StubParam::Positional(index)
                                })
                            })
                            .collect();
                    for param in stub.checks.iter().map(|(p, _)| p).chain(own.iter()) {
                        match param {
                            crate::own_sdk::StubParam::Positional(i) => {
                                if *i < pos.len() {
                                    pos[*i] = object_q.clone();
                                } else if let Some(t) = opt.get_mut(*i - pos.len()) {
                                    *t = object_q.clone();
                                }
                            }
                            crate::own_sdk::StubParam::Named(n) => {
                                if let Some(e) = named.iter_mut().find(|(m, _, _)| m == n) {
                                    e.1 = object_q.clone();
                                }
                            }
                        }
                    }
                    Ty::Fn {
                        type_params,
                        ret,
                        pos,
                        opt,
                        named,
                        nullable,
                    }
                }
                t => t,
            };
            method_sigs.push((jsname, reified));
        }
    }

    // Constructor tear-offs (`C.new == C.new`).
    if !is_mixin {
        for (csym, cfid) in class.constructors() {
            let cf = ctx.program.function(cfid);
            let cn = ctx.name(csym);
            let jsname = if cn.is_empty() {
                "new".to_string()
            } else {
                static_member_name(cn)
            };
            // With its own type parameters the static tear-off does not exist
            // (the value is built at the use site, with the arguments).
            if !ctx.class_params[c.0 as usize].is_empty() || !ctx.tearoff_live(cfid) {
                continue;
            }
            let mut args = String::from("...args");
            if generic {
                let e = FnEmitter::new(ctx, m, unit, Some(c), true);
                args = format!("{}, ...args", e.rti(&ctx.this_ty_default(c)));
            }
            let call = if cf.factory {
                format!("{cref}.{jsname}({args})")
            } else {
                format!("new {cref}.{jsname}({args})")
            };
            cw.line(&format!(
                "static [{}](...args) {{ return {call}; }}",
                js::string_literal(&format!("_#{jsname}#tearOff"))
            ));
        }
    }

    // Getter only (or setter only) overriding an inherited field/accessor: emits the missing half.
    {
        let this_ty = ctx.this_ty(c);
        let mut extra: Vec<String> = Vec::new();
        for (n, _) in &getter_sigs {
            if setter_sigs.iter().any(|(m, _)| m == n) || fields.iter().any(|f| f.name == *n) {
                continue;
            }
            let has_super_setter = class
                .supertype_class
                .and_then(|sc| ctx.lookup_member(&ctx.this_ty(sc), n, true))
                .is_some()
                || ctx
                    .mixins_of(c)
                    .iter()
                    .any(|mx| ctx.lookup_member(&ctx.this_ty(*mx), n, true).is_some());
            if has_super_setter {
                let key = if n.starts_with('_') {
                    format!("[{}]", m.private_sym(ctx, class.library, n))
                } else {
                    js::prop_key(&crate::body::js_member_name(n))
                };
                let acc = if n.starts_with('_') {
                    format!("[{}]", m.private_sym(ctx, class.library, n))
                } else {
                    js::prop_access(n)
                };
                extra.push(format!("set {key}(value) {{ super{acc} = value; }}"));
            }
        }
        for (n, _) in &setter_sigs {
            if getter_sigs.iter().any(|(m, _)| m == n) || fields.iter().any(|f| f.name == *n) {
                continue;
            }
            let has_super_getter = class
                .supertype_class
                .and_then(|sc| ctx.lookup_member(&ctx.this_ty(sc), n, false))
                .is_some()
                || ctx
                    .mixins_of(c)
                    .iter()
                    .any(|mx| ctx.lookup_member(&ctx.this_ty(*mx), n, false).is_some());
            if has_super_getter {
                let key = if n.starts_with('_') {
                    format!("[{}]", m.private_sym(ctx, class.library, n))
                } else {
                    js::prop_key(&crate::body::js_member_name(n))
                };
                let acc = if n.starts_with('_') {
                    format!("[{}]", m.private_sym(ctx, class.library, n))
                } else {
                    js::prop_access(n)
                };
                extra.push(format!("get {key}() {{ return super{acc}; }}"));
            }
        }
        let _ = this_ty;
        for x in extra {
            cw.line(&x);
        }
    }

    // A generic mixin can supply a plain member without implementing the
    // native interface itself. Its concrete application must also expose the
    // symbol used by that interface, overriding any inherited older alias.
    // Keep fields out: a plain instance field has no prototype descriptor.
    let mut inherited_native_aliases = HashSet::new();
    let mut inherited_native_method_signatures = HashSet::new();
    if !native_class && !is_mixin && !class.modifiers.abstract_ {
        let mut names: Vec<_> = natives.iter().collect();
        names.sort();
        for name in names {
            for setter in [false, true] {
                if ctx.declared_member(c, name, setter).is_some() {
                    continue;
                }
                // Runtime mixin applications overwrite the superclass and
                // earlier mixins (interface/BFS lookup is not that order);
                // then the superclass chain.
                let supers: Vec<ClassId> =
                    std::iter::successors(ctx.superclass_of(c), |&k| ctx.superclass_of(k))
                        .take_while(|&k| Some(k) != ctx.object)
                        .collect();
                let member = ctx
                    .mixins_of(c)
                    .into_iter()
                    .rev()
                    .chain(supers)
                    .find_map(|mixin| {
                        let kind = ctx.declared_member(mixin, name, setter)?;
                        let applied = ctx.as_super(&ctx.this_ty(c), mixin)?;
                        let subst = ctx.class_params[mixin.0 as usize]
                            .iter()
                            .enumerate()
                            .map(|(i, p)| {
                                (p.id, applied.args().get(i).cloned().unwrap_or(Ty::Dynamic))
                            })
                            .collect();
                        Some(crate::ctx::Member {
                            class: mixin,
                            kind,
                            subst,
                        })
                    });
                let Some(member) = member else { continue };
                if ctx.is_ext_member(&ctx.this_ty(member.class), name, setter) {
                    continue;
                }
                let (fid, accessor) = match member.kind {
                    crate::ctx::MemberKind::Method(fid) => (fid, false),
                    crate::ctx::MemberKind::Getter(fid) | crate::ctx::MemberKind::Setter(fid) => {
                        (fid, true)
                    }
                    crate::ctx::MemberKind::Field(_) => continue,
                };
                if ctx.program.function(fid).abstract_
                    || ctx.fn_state(fid) == crate::filter::State::Dead
                {
                    continue;
                }
                inherited_native_aliases.insert(name.clone());
                let ty = ctx.member_ty(&member);
                if accessor {
                    ext_accessors.push(name.clone());
                    if setter {
                        setter_sigs.push((name.clone(), ty));
                    } else {
                        getter_sigs.push((name.clone(), ty));
                    }
                } else {
                    let jsname = js_member_name(name);
                    ext_methods.push(jsname.clone());
                    inherited_native_method_signatures.insert(jsname.clone());
                    method_sigs.push((jsname.clone(), ty));
                    let data = &ctx.outline.functions[fid.0 as usize];
                    if !data.type_params.is_empty() {
                        let defaults = data
                            .type_params
                            .iter()
                            .map(|p| {
                                let tp = ctx.ty_param_of(*p);
                                let bound = tp.bound.subst(&member.subst);
                                if bound.mentions_params() || crate::own_sdk::is_top(ctx, &bound) {
                                    Ty::Dynamic
                                } else {
                                    bound
                                }
                            })
                            .collect();
                        generic_methods.push((jsname, defaults));
                    }
                }
            }
        }
    }

    // Class declaration.
    ext_methods.sort();
    ext_methods.dedup();
    ext_accessors.sort();
    ext_accessors.dedup();
    // Mixin with native interface members: the `defineExtension*` calls go
    // inside `mixinOn`, on the class of each application — that is the one that
    // enters the prototype chain (`compiler.dart:1236-1248`).
    let ext_in_mixin = is_mixin && (!ext_methods.is_empty() || !ext_accessors.is_empty());
    // The class expression's own name: a JS reserved word gets a `$`.
    let cid = js::ident(&cname);
    let head = if ext_in_mixin {
        format!(
            "{cref} = class {cid} extends core.Object {{{shell}}};\n{cref}[dart.mixinOn] = {cid}$mixin_super => {{\nconst {cid}$m = class {cid} extends {cid}$mixin_super {{",
            shell = mixin_shell(&mixin_statics)
        )
    } else if is_mixin {
        format!(
            "{cref} = class {cid} extends core.Object {{{}}};\n{cref}[dart.mixinOn] = {}$mixin_super => class {cid} extends {}$mixin_super {{",
            mixin_shell(&mixin_statics),
            cid,
            cid
        )
    } else if ctx.sdk.is_some() && Some(c) == ctx.object {
        // SDK mode: `Object` has no superclass (`compiler.dart:1257-1259`).
        format!("{cref} = class {cid} {{")
    } else if ctx.sdk.is_some() && mentions_free_name(&cw.out, &cname) {
        // SDK mode: the name of the class expression would shadow, in the body,
        // the global with the same name that a `JS()` *template* references —
        // `new ResizeObserver(#)` in the `_create_1` of `html.ResizeObserver`.
        // DDC renames the class's temporary identifier (`ResizeObserver$`).
        format!("{cref} = class {cid}$ extends {super_ref} {{")
    } else {
        format!("{cref} = class {cid} extends {super_ref} {{")
    };
    m.use_sdk("core");
    w.line(&head);
    w.push_raw(&cw.out);
    w.line("};");
    // One call per class: `dart.lateFields(C.prototype, {finals}, {others})`,
    // with the key as a property of the object literal (`_x$1c: "_x"`, which
    // property renaming shortens together with the `this._x$1c` accesses) or
    // computed (`[sym]: "_x"`, in the SDK).
    // Production without the names, every key a plain property: the keys as
    // one *string* each, `"_a$1c _b$1c"` (`dart.lateFields` splits it; the
    // minifier renames and sorts the words, `emit_js_production`'s
    // `rewrite_late_fields`). `DARTFORGE_JSPROD_LATE_STRING=0` keeps the
    // objects.
    let as_strings = ctx.production()
        && std::env::var("DARTFORGE_JSPROD_LATE_STRING").map_or(true, |v| v != "0")
        && late_fields.iter().all(|(k, n, _)| {
            ctx.late_name(n).is_empty()
                && k.strip_prefix('"')
                    .and_then(|x| x.strip_suffix('"'))
                    .is_some_and(js::is_js_ident)
        });
    if !late_fields.is_empty() && as_strings {
        let words = |fin: bool| -> String {
            let items: Vec<&str> = late_fields
                .iter()
                .filter(|t| t.2 == fin)
                .filter_map(|(k, _, _)| k.strip_prefix('"').and_then(|x| x.strip_suffix('"')))
                .collect();
            js::string_literal(&items.join(" "))
        };
        w.line(&format!(
            "dart.lateFields({cref}.prototype, {}, {});",
            words(true),
            words(false)
        ));
    } else if !late_fields.is_empty() {
        let object = |fin: bool| -> String {
            let items: Vec<String> = late_fields
                .iter()
                .filter(|t| t.2 == fin)
                .map(|(k, n, _)| {
                    let key = match k.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
                        Some(x) if js::is_js_ident(x) => x.to_string(),
                        _ => format!("[{k}]"),
                    };
                    // Without the names (`Ctx::late_name`) the value is `0`,
                    // which `dart.lateFields` reads as `""`.
                    let n = ctx.late_name(n);
                    if n.is_empty() {
                        format!("{key}: 0")
                    } else {
                        format!("{key}: {}", js::string_literal(n))
                    }
                })
                .collect();
            format!("{{{}}}", items.join(", "))
        };
        w.line(&format!(
            "dart.lateFields({cref}.prototype, {}, {});",
            object(true),
            object(false)
        ));
    }
    if ext_in_mixin {
        if !ext_methods.is_empty() {
            let items: Vec<String> = ext_methods.iter().map(|n| js::string_literal(n)).collect();
            w.line(&format!(
                "dart.defineExtensionMethods({cname}$m, [{}]);",
                items.join(", ")
            ));
        }
        if !ext_accessors.is_empty() {
            let items: Vec<String> = ext_accessors
                .iter()
                .map(|n| js::string_literal(n))
                .collect();
            w.line(&format!(
                "dart.defineExtensionAccessors({cname}$m, [{}]);",
                items.join(", ")
            ));
        }
        w.line(&format!("return {cname}$m;\n}};"));
        ext_methods.clear();
        ext_accessors.clear();
    }

    // Generative constructors.
    let mut ctor_names: Vec<String> = Vec::new();
    let has_synthetic = class
        .constructors
        .values()
        .any(|f| ctx.program.function(*f).kind == FunctionKind::SyntheticConstructor);
    // With a filter: the synthetic one lives by its element; without an element
    // (class without constructors) or for an enum, by the instantiated class.
    let synthetic_state = if ctx.filter.is_none() {
        crate::filter::State::Live
    } else {
        let by_class = if ctx.class_level(c) == crate::filter::Level::Instantiated {
            crate::filter::State::Live
        } else {
            crate::filter::State::Dead
        };
        match class
            .constructors
            .values()
            .find(|f| ctx.program.function(**f).kind == FunctionKind::SyntheticConstructor)
        {
            Some(f) if !is_enum => ctx.fn_state(*f),
            _ => by_class,
        }
    };
    if ctor_members.is_empty()
        && !is_mixin
        && (has_synthetic || class.constructors.is_empty() || is_enum)
        && synthetic_state == crate::filter::State::Stub
    {
        let r = js::string_literal(&ctx.pruned_label(class.library, Some(c), "new"));
        crate::emit_line!(
            w,
            "({cref}.new = function(...a) {{ dart_podado({r}); }}).prototype = {cref}.prototype;"
        );
    } else if ctor_members.is_empty()
        && !is_mixin
        && is_mixin_application(ctx, c)
        && !ctx.mixins_of(c).is_empty()
    {
        // `class C = S with M;`: a forwarding constructor for each generative
        // constructor of `S`, with the same arguments (without it the SDK's
        // `_AsyncStreamController = _StreamController with …` lost its
        // `onListen`).
        let sup = ctx.superclass_of(c);
        let s_generic = sup.is_some_and(|s| ctx.requires_rti(s));
        let tmp = FnEmitter::new(ctx, m, unit, None, true);
        let base = mixin_base_ref(
            ctx,
            c,
            &sup.map(|s| tmp.class_ref(s))
                .unwrap_or_else(|| "core.Object".to_string()),
        );
        let mut names: Vec<String> = sup
            .map(|s| {
                ctx.program
                    .class(s)
                    .constructors()
                    .into_iter()
                    .filter(|(_, f)| {
                        !ctx.program.function(*f).factory
                            && ctx.fn_state(*f) != crate::filter::State::Dead
                    })
                    .map(|(n, _)| {
                        let n = ctx.name(n);
                        if n.is_empty() {
                            "new".to_string()
                        } else {
                            static_member_name(n)
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        if names.is_empty() {
            names.push("new".into());
        }
        for n in names {
            ctor_names.push(n.clone());
            let (params, call) = match (generic, s_generic) {
                (true, true) => (
                    "_ti, ...args",
                    format!("{base}.{n}.call(this, _ti, ...args);"),
                ),
                (true, false) => ("_ti, ...args", format!("{base}.{n}.apply(this, args);")),
                (false, true) => ("...args", format!("{base}.{n}.call(this, null, ...args);")),
                (false, false) => ("...args", format!("{base}.{n}.apply(this, args);")),
            };
            let ti = if generic {
                "this.$ti = this.$ti || _ti || dart.getReifiedType(this); "
            } else {
                ""
            };
            crate::emit_line!(
                w,
                "({cref}.{n} = function({params}) {{ {ti}{call} }}).prototype = {cref}.prototype;"
            );
        }
    } else if ctor_members.is_empty()
        && !is_mixin
        && (has_synthetic || class.constructors.is_empty() || is_enum)
        && synthetic_state == crate::filter::State::Live
    {
        // Synthetic constructor.
        let jsname = "new";
        ctor_names.push(jsname.into());
        let mut body = Writer {
            indent: 1,
            ..Writer::default()
        };
        if generic {
            body.line("this.$ti = this.$ti || _ti || dart.getReifiedType(this);");
        }
        emit_field_inits(ctx, m, c, &fields, &HashSet::new(), &mut body);
        if is_enum {
            let base = mixin_base_ref(ctx, c, "core._Enum");
            crate::emit_line!(body, "{base}.new.call(this, t$index, t$name);");
        } else {
            emit_super_call_default(ctx, m, c, &mut body);
        }
        let params = if is_enum && generic {
            "t$index, t$name, _ti".to_string()
        } else if is_enum {
            "t$index, t$name".to_string()
        } else if generic {
            "_ti".to_string()
        } else {
            String::new()
        };
        crate::emit_line!(w, "({cref}.{jsname} = function({params}) {{");
        w.push_raw(&body.out);
        crate::emit_line!(w, "}}).prototype = {cref}.prototype;");
    }
    for (mu, mid) in ctor_members {
        let mem = ctx.program.unit(mu).ast.member(mid);
        let MemberKind::Constructor(ctor) = &mem.kind else {
            continue;
        };
        let name = ctor.name.map(|n| ctx.name(n.sym).to_string());
        let jsname = name
            .clone()
            .map(|n| static_member_name(&n))
            .unwrap_or("new".into());
        ctor_names.push(jsname.clone());
        let state = ctor
            .name
            .map(|n| n.sym)
            .or(ctx.empty_sym)
            .and_then(|s| class.constructors.get(&s).copied())
            .map_or(crate::filter::State::Live, |f| ctx.fn_state(f));
        if state == crate::filter::State::Stub {
            let r = js::string_literal(&ctx.pruned_label(class.library, Some(c), &jsname));
            let text = format!("function(...a) {{ dart_podado({r}); }}");
            w.line(&constructor_assignment(ctx, c, &cref, &jsname, &text));
            continue;
        }
        let text = emit_constructor(ctx, m, c, mu, ctor, &fields, generic, is_enum);
        w.line(&constructor_assignment(ctx, c, &cref, &jsname, &text));
    }
    // A plain class extending `Object` can be mixed in by a library before
    // 3.0; its field initializers then run through the application too. With
    // the whole program known (production), only the classes actually mixed in.
    let legacy_mixin = class.kind == ClassKind::Class
        && class.supertype_class == ctx.object
        && class.mixin_classes.is_empty()
        && !fields.is_empty()
        && (!ctx.production() || ctx.is_mixed_in(c));
    if is_mixin || class.modifiers.mixin || legacy_mixin {
        let mut body = Writer {
            indent: 1,
            ..Writer::default()
        };
        emit_field_inits(ctx, m, c, &fields, &HashSet::new(), &mut body);
        crate::emit_line!(w, "({cref}[dart.mixinNew] = function() {{");
        w.push_raw(&body.out);
        crate::emit_line!(w, "}}).prototype = {cref}.prototype;");
    }

    // rti resources: the class itself and the implemented interfaces (transitively).
    let mut recipes: Vec<String> = vec![ctx.class_recipe(c)];
    let mut iseen: HashSet<u32> = HashSet::new();
    let mut iqueue: Vec<ClassId> = ctx.interfaces_of(c);
    if is_mixin {
        iqueue.extend(class.on_classes.iter().copied());
    }
    while let Some(i) = iqueue.pop() {
        if Some(i) == ctx.object || !iseen.insert(i.0) {
            continue;
        }
        recipes.push(ctx.class_recipe(i));
        let ic = ctx.program.class(i);
        iqueue.extend(ic.interface_classes.iter().copied());
        if let Some(s) = ic.supertype_class {
            iqueue.push(s);
        }
        iqueue.extend(ic.mixin_classes.iter().copied());
    }
    let recipes_js: Vec<String> = recipes.iter().map(|r| js::string_literal(r)).collect();
    // A class no instance has in its prototype chain needs neither its recipe
    // nor its tags, unless the runtime copies them at load time: a mixin
    // (`applyMixin`), `Object` and a native peer (`registerExtension`).
    let copied = is_mixin
        || class.modifiers.mixin
        || legacy_mixin
        || Some(c) == ctx.object
        || ctx.is_mixed_in(c)
        || ctx
            .sdk
            .as_ref()
            .is_some_and(|s| s.peers.get(&c).is_some_and(|t| !t.is_empty()));
    if ctx.filter.is_none_or(|f| f.rti_live())
        && (copied || ctx.filter.is_none_or(|f| f.rti_resources(c)))
    {
        w.line(&format!(
            "dart.addRtiResources({cref}, [{}]);",
            recipes_js.join(", ")
        ));
    }

    // Signatures.
    let mut se = FnEmitter::new(ctx, m, unit, Some(c), false);
    se.sig_mode = true;
    // SDK mode: the base of `Object`'s signatures is an empty object
    // (`compiler.dart:1568-1572`); and a native interface member of an SDK Dart
    // class also shows up under the `dartx` key (`:1702-1720`), which is the one
    // that `bind`/`dsend` on the symbol looks for.
    let base_sig = |kind: &str| {
        if ctx.sdk.is_some() && Some(c) == ctx.object {
            "Object.create(null)".to_string()
        } else {
            format!("dart.{kind}(Object.getPrototypeOf({cref}))")
        }
    };
    // SDK mode: the object literal with `__proto__` (DDC's form,
    // `compiler.dart:1568-1590`) instead of `Object.setPrototypeOf`.
    let sig = |items: &[String], kind: &str| -> String {
        if ctx.sdk.is_some() {
            let base = if Some(c) == ctx.object {
                "null".to_string()
            } else {
                format!("dart.{kind}({cref}.__proto__)")
            };
            format!("() => ({{__proto__: {base}, {}}})", items.join(", "))
        } else {
            format!(
                "() => Object.setPrototypeOf({{{}}}, {})",
                items.join(", "),
                base_sig(kind)
            )
        }
    };
    let duplicate = ctx.sdk.is_some() && !native_class && ctx.libs[class.library.0 as usize].is_sdk;
    // Production profile: outside a native peer's prototype chain the `dartx`
    // key is only read by a tear-off through the symbol
    // (`Liveness::symbol_signature`).
    let symbol_read =
        |n: &str| !ctx.production() || ctx.in_native_chain(c) || ctx.symbol_signature_live(n);
    let with_dartx = |items: &mut Vec<String>, n: &str, value: &str| {
        if (duplicate || (ctx.sdk.is_some() && inherited_native_aliases.contains(n)))
            && natives.contains(n)
            && symbol_read(n)
        {
            items.push(format!("{}: {value}", crate::own_sdk::native_key(m, n)));
        }
    };
    // Production profile: only the entries that dynamic dispatch and the
    // *tear-off* read (`Liveness::signature`); without a filter, all of them.
    // Newly bridged native-interface aliases require their actual substituted
    // signature: retaining only an older superclass signature loses generic
    // function information during dynamic calls of the bound method.
    // Production profile: a bridged alias is read like any other entry, by
    // dynamic dispatch or a tear-off of its name
    // (`DARTFORGE_JSPROD_ALIAS_SIGNATURES=0` keeps them all).
    let keep_aliases = !ctx.production()
        || std::env::var("DARTFORGE_JSPROD_ALIAS_SIGNATURES").is_ok_and(|v| v == "0");
    let alias_method = |n: &str| keep_aliases && inherited_native_method_signatures.contains(n);
    let alias_accessor = |n: &str| keep_aliases && inherited_native_aliases.contains(n);
    method_sigs.retain(|(n, _)| ctx.signature_live(signature_name(n)) || alias_method(n));
    generic_methods.retain(|(n, _)| ctx.signature_live(n) || alias_method(n));
    // A `late` field is a getter/setter pair (its storage is the field):
    // dynamic accesses find it by its name, as in the CFE's lowering.
    for f in fields.iter().filter(|f| f.late && f.virtual_ && !f.helper) {
        getter_sigs.push((f.name.clone(), f.ty.clone()));
        if !(f.final_ && f.init.is_some()) {
            setter_sigs.push((f.name.clone(), f.ty.clone()));
        }
    }
    getter_sigs.retain(|(n, _)| ctx.signature_live(signature_name(n)) || alias_accessor(n));
    setter_sigs.retain(|(n, _)| ctx.signature_live(signature_name(n)) || alias_accessor(n));
    let fields_sig: Vec<&FieldInfo> = fields
        .iter()
        .filter(|f| ctx.signature_live(&f.name))
        .collect();
    // Production profile: a signature that does not depend on the instance's
    // type arguments is the rti itself, not a `_ti => rti` function
    // (`rtiFromSignature` and the other readers of the runtime only call the
    // entry when it is a function); the whole object is already built lazily.
    let sig_value = |code: String| {
        if ctx.production() && !code.contains("_ti") {
            code
        } else {
            format!("_ti => {code}")
        }
    };
    if !method_sigs.is_empty() {
        let mut items: Vec<String> = Vec::new();
        for (n, t) in method_sigs.iter() {
            let value = sig_value(se.rti(t));
            items.push(format!("{}: {value}", sig_key(n, &se, c)));
            let dart_n = match n.as_str() {
                "_equals" => "==",
                "_get" => "[]",
                "_set" => "[]=",
                "_negate" => "unary-",
                x => x,
            };
            with_dartx(&mut items, dart_n, &value);
        }
        w.line(&format!(
            "dart.setMethodSignature({cref}, {});",
            sig(&items, "getMethods")
        ));
    }
    if !generic_methods.is_empty() {
        let items: Vec<String> = generic_methods
            .iter()
            .map(|(n, ds)| {
                format!(
                    "{}: {}",
                    sig_key(n, &se, c),
                    sig_value(format!(
                        "[{}]",
                        ds.iter().map(|d| se.rti(d)).collect::<Vec<_>>().join(", ")
                    ))
                )
            })
            .collect();
        w.line(&format!(
            "dart.setMethodsDefaultTypeArgSignature({cref}, {});",
            sig(&items, "getMethodsDefaultTypeArgs")
        ));
    }
    if !getter_sigs.is_empty() {
        let mut items: Vec<String> = Vec::new();
        for (n, t) in getter_sigs.iter() {
            let value = sig_value(se.rti(t));
            items.push(format!("{}: {value}", sig_key(n, &se, c)));
            with_dartx(&mut items, n, &value);
        }
        w.line(&format!(
            "dart.setGetterSignature({cref}, {});",
            sig(&items, "getGetters")
        ));
    }
    if !setter_sigs.is_empty() {
        let mut items: Vec<String> = Vec::new();
        for (n, t) in setter_sigs.iter() {
            let value = sig_value(se.rti(t));
            items.push(format!("{}: {value}", sig_key(n, &se, c)));
            with_dartx(&mut items, n, &value);
        }
        w.line(&format!(
            "dart.setSetterSignature({cref}, {});",
            sig(&items, "getSetters")
        ));
    }
    // Production profile: only the statics that dynamic dispatch reads
    // (`dload`/`dsend` on a `Type`), like the instance signatures.
    static_methods.retain(|n| ctx.signature_live(n));
    if !static_methods.is_empty() {
        let items: Vec<String> = static_methods
            .iter()
            .map(|n| js::string_literal(&static_member_name(n)))
            .collect();
        w.line(&format!(
            "dart.setStaticMethodSignature({cref}, () => [{}]);",
            items.join(", ")
        ));
    }
    let lib_uri = js::string_literal(&ctx.program.library(class.library).uri);
    if crate::own_sdk::needs_library_uri(ctx, c) {
        crate::emit_line!(w, "dart.setLibraryUri({cref}, {lib_uri});");
    }
    if !fields_sig.is_empty() {
        let items: Vec<String> = fields_sig
            .iter()
            .map(|f| {
                let key = match &f.storage {
                    _ if f.helper => format!("[{}]", m.private_sym(ctx, class.library, &f.name)),
                    Some(s) if f.name.starts_with('_') || f.late => format!("[{s}]"),
                    _ => js::prop_key(&crate::body::js_member_name(&f.name)),
                };
                let ty = if f.late {
                    f.ty.with_nullable(true)
                } else {
                    f.ty.clone()
                };
                let final_ = f.final_ && !f.late;
                if ctx.production() {
                    // Production profile: `isConst` is only read by the
                    // debugger, and a missing `isFinal` reads as `false`.
                    let fin = if final_ { ", isFinal: true" } else { "" };
                    format!("{key}: {{type: {}{fin}}}", sig_value(se.rti(&ty)))
                } else {
                    format!(
                        "{key}: {{type: _ti => {}, isConst: false, isFinal: {final_}}}",
                        se.rti(&ty)
                    )
                }
            })
            .collect();
        w.line(&format!(
            "dart.setFieldSignature({cref}, {});",
            sig(&items, "getFields")
        ));
    }
    // The constants (and `values`) that the closed world keeps: an enum
    // constant lives one by one, only when read (`dartforge_world`).
    let enum_constant_live = |ec: &ast::EnumConstant| {
        class
            .enum_constants
            .iter()
            .find(|v| ctx.program.variable(**v).name == ec.name.sym)
            .is_none_or(|v| ctx.var_state(*v) != crate::filter::State::Dead)
    };
    let enum_values_live = ctx
        .sym("values")
        .and_then(|s| class.static_members.get(&s))
        .is_none_or(|f| ctx.fn_state(*f) != crate::filter::State::Dead);
    // Statics: fields.
    let static_fields: Vec<VariableId> = class
        .fields
        .iter()
        .copied()
        .filter(|v| {
            ctx.program.variable(*v).static_ && ctx.var_state(*v) != crate::filter::State::Dead
        })
        .collect();
    let mut static_names: Vec<String> = static_fields
        .iter()
        .map(|v| ctx.name(ctx.program.variable(*v).name).to_string())
        .collect();
    if is_enum {
        if enum_values_live {
            static_names.insert(0, "values".into());
        }
        for (_, ec) in enum_constants
            .iter()
            .filter(|(_, ec)| enum_constant_live(ec))
        {
            static_names.push(ctx.name(ec.name.sym).to_string());
        }
    }
    static_names.retain(|n| ctx.signature_live(n));
    if !static_names.is_empty() {
        let items: Vec<String> = static_names
            .iter()
            .map(|n| js::string_literal(&static_member_name(n)))
            .collect();
        w.line(&format!(
            "dart.setStaticFieldSignature({cref}, () => [{}]);",
            items.join(", ")
        ));
    }
    // Extensions (members of native interfaces).
    ext_methods.sort();
    ext_methods.dedup();
    ext_accessors.sort();
    ext_accessors.dedup();
    // Production: `Object`'s `dartx.toString` forwards to the JavaScript
    // `toString`, which is the Dart one; a class outside the prototype
    // chains of the natives, with no ancestor inside them, needs no alias
    // of its own (`object_member_forwarders`).
    if object_member_forwarders(ctx) {
        if Some(c) == ctx.object {
            if let Some(i) = ext_methods.iter().position(|n| n == "toString") {
                ext_methods.remove(i);
                w.line(&format!(
                    "{cref}.prototype[dartx.toString] = function() {{ return this.toString(); }};"
                ));
            }
        } else if !in_or_under_native_chain(ctx, c) {
            ext_methods.retain(|n| n != "toString");
        }
    }
    if !ext_methods.is_empty() {
        let items: Vec<String> = ext_methods.iter().map(|n| js::string_literal(n)).collect();
        w.line(&format!(
            "dart.defineExtensionMethods({cref}, [{}]);",
            items.join(", ")
        ));
    }
    if !ext_accessors.is_empty() {
        let items: Vec<String> = ext_accessors
            .iter()
            .map(|n| js::string_literal(n))
            .collect();
        w.line(&format!(
            "dart.defineExtensionAccessors({cref}, [{}]);",
            items.join(", ")
        ));
    }
    // SDK mode: `_installIdentityEquals`, `definePrimitiveHashCode`, `registerExtension`.
    if ctx.sdk.is_some() {
        w.push_raw(&crate::own_sdk::class_epilogue(ctx, c, &cref));
        w.push_raw(&crate::fresh_list_append::emit_helper(ctx, m, c, &cref));
    }
    // Static fields and enum constants (lazy).
    let mut lazy: Vec<String> = Vec::new();
    let mut clashes: Vec<String> = Vec::new();
    if is_enum {
        let mut e = FnEmitter::new(ctx, m, unit, Some(c), true);
        e.in_const = true;
        let mut names = Vec::new();
        for (i, &(unit_k, ec)) in enum_constants.iter().enumerate() {
            if !enum_constant_live(ec) {
                continue;
            }
            // The arguments of an augmented value are read in its own unit.
            let mut e_k = (unit_k != unit).then(|| {
                let mut x = FnEmitter::new(ctx, m, unit_k, Some(c), true);
                x.in_const = true;
                x
            });
            let e = e_k.as_mut().unwrap_or(&mut e);
            let n = ctx.name(ec.name.sym).to_string();
            names.push(format!(
                "{cref}{}",
                js::prop_access(&static_member_name(&n))
            ));
            let ctor_name = ec
                .constructor
                .map(|c| ctx.name(c.sym).to_string())
                .unwrap_or_default();
            let mut args: Vec<String> = vec![i.to_string(), js::string_literal(&n)];
            if let Some(a) = &ec.arguments {
                let (pos, named, _) = e.emit_args_plain(a);
                args.extend(pos);
                if let Some(nm) = named {
                    args.push(nm);
                }
            }
            let jsname = if ctor_name.is_empty() {
                "new".to_string()
            } else {
                static_member_name(&ctor_name)
            };
            if generic {
                let mut tys: Vec<Ty> = ec.type_args.iter().map(|t| e.resolve_type(*t)).collect();
                // Without written arguments, those the value was inferred
                // with (its static field's type: `v1` of `E<T extends num>`
                // is an `E<num>`).
                if tys.is_empty()
                    && let Some(crate::ctx::MemberKind::Field(v)) =
                        ctx.declared_static(c, &n, false)
                    && let Ty::Iface { class, args, .. } = ctx.var_ty(v)
                    && class == c
                    && !args.iter().any(|a| a.mentions_params())
                {
                    tys = args;
                }
                // Otherwise the instantiation the common inference chose for
                // the value's arguments, or the bounds when it has none.
                let params = ctx.class_params[c.0 as usize].clone();
                if tys.is_empty() {
                    tys = match &ec.arguments {
                        Some(a) if !a.args.is_empty() => {
                            e.common_instantiation(a, params.len()).unwrap_or_default()
                        }
                        _ => params
                            .iter()
                            .map(|p| e.default_type_arg(&p.bound))
                            .collect(),
                    };
                }
                while tys.len() < params.len() {
                    tys.push(Ty::Dynamic);
                }
                let t = Ty::Iface {
                    class: c,
                    args: tys,
                    nullable: false,
                };
                args.insert(2, e.rti(&t));
            }
            lazy.push(format!(
                "get {}() {{ return dart.const(new {cref}.{jsname}({})); }}",
                js::prop_key(&static_member_name(&n)),
                args.join(", ")
            ));
        }
        if enum_values_live {
            let rti = e.rti(&Ty::iface(c));
            lazy.insert(
                0,
                format!(
                    "get values() {{ return dart.constList({rti}, [{}]); }}",
                    names.join(", ")
                ),
            );
        }
    }
    for vid in static_fields {
        let v = ctx.program.variable(vid);
        let name = static_member_name(ctx.name(v.name));
        if ctx.var_state(vid) == crate::filter::State::Stub {
            let r = js::string_literal(&ctx.pruned_label(class.library, Some(c), &name));
            lazy.push(format!(
                "get {}() {{ return dart_podado({r}); }}",
                js::prop_key(&name)
            ));
            continue;
        }
        let VariableRef::Field {
            unit: fu,
            member,
            index,
        } = v.node
        else {
            continue;
        };
        let mem = ctx.program.unit(fu).ast.member(member);
        let MemberKind::Field(list) = &mem.kind else {
            continue;
        };
        let var = &list.variables[index];
        let ty = ctx.var_ty(vid);
        let mut e = FnEmitter::new(ctx, m, fu, Some(c), true);
        e.in_const = v.const_;
        let init = match var.initializer {
            Some(i) => {
                let (js, _) = e.emit_expr(i, Some(&ty));
                let pre = if e.temps.is_empty() {
                    String::new()
                } else {
                    format!("let {};\n", e.temps.join(", "))
                };
                format!("{pre}{}return {};", e.w.out, js.code)
            }
            None => "return null;".to_string(),
        };
        if v.late {
            m.use_sdk("_internal");
            let store = format!("{cref}[{}]", js::string_literal(&format!("_#{name}")));
            let is_set = (ty.is_nullable() || ty.is_dynamic())
                .then(|| format!("{cref}[{}]", js::string_literal(&format!("_#{name}#isSet"))));
            let (get, set) = late_accessors(
                &store,
                is_set.as_deref(),
                var.initializer.is_some().then_some(init.as_str()),
                v.final_,
                ctx.late_name(&name),
                "",
            );
            w.line(&format!(
                "dart.copyProperties({cref}, {{\n{},\n{}\n}});",
                indent(&format!(
                    "get {}() {{\n{}\n}}",
                    js::prop_key(&name),
                    indent(&get)
                )),
                indent(&format!(
                    "set {}(v) {{\n{}\n}}",
                    js::prop_key(&name),
                    indent(&set)
                ))
            ));
        } else if let Some(lit) = eager_literal(ctx, &init, &name)
            .filter(|_| !static_setter_clash(ctx, c, &name, v.final_ || v.const_))
        {
            w.line(&format!("{cref}{} = {lit};", js::prop_access(&name)));
        } else {
            let mut entry = format!("get {}() {{\n{}\n}}", js::prop_key(&name), indent(&init));
            if !v.final_ && !v.const_ {
                entry.push_str(&format!(",\nset {}(value) {{}}", js::prop_key(&name)));
            } else if static_setter_clash(ctx, c, &name, true) {
                clashes.push(name.clone());
            }
            lazy.push(entry);
        }
    }
    // A final static field with a same-named static setter
    // (`static const f = 0; static set f(v)`): the lazy getter and the
    // declared setter share the property.
    for name in &clashes {
        let key = js::string_literal(name);
        crate::emit_line!(
            w,
            "const {cname}$set${name} = Object.getOwnPropertyDescriptor({cref}, {key}).set;"
        );
    }
    if !lazy.is_empty() {
        crate::emit_line!(w, "dart.defineLazy({cref}, {{");
        w.push_raw(&indent(&lazy.join(",\n")));
        w.push_raw("\n});\n");
    }
    for name in &clashes {
        let key = js::string_literal(name);
        crate::emit_line!(
            w,
            "Object.defineProperty({cref}, {key}, {{get: Object.getOwnPropertyDescriptor({cref}, {key}).get, set: {cname}$set${name}, configurable: true}});"
        );
    }
}

/// Whether the final (or const) static field `name` of `c` shares its name
/// with a static setter of `c`.
fn static_setter_clash(ctx: &Ctx, c: ClassId, name: &str, final_: bool) -> bool {
    final_
        && ctx
            .sym(&format!("{name}_="))
            .is_some_and(|s| ctx.program.class(c).static_members.contains_key(&s))
}

/// `@JS` class (`_emitJSInteropClassNonExternalMembers`): only the non-`external`
/// factories and statics, plus the constructor tear-offs (`_#name#tearOff`: an
/// object literal if `@anonymous`, otherwise `new dart.global.X`).
fn emit_js_interop_class(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) {
    let class = ctx.program.class(c);
    let Some(decl) = class.decl else { return };
    let unit = decl.unit;
    let lvar = ctx.libs[class.library.0 as usize].js_var.clone();
    let cname = ctx.class_name(c).to_string();
    let cref = format!("{lvar}.{cname}");
    let d = ctx.program.unit(unit).ast.decl(decl.decl);
    let members: Vec<ast::MemberId> = match &d.kind {
        DeclKind::Class(cd) => cd.members.clone(),
        DeclKind::ExtensionType(ed) => ed.members.clone(),
        _ => vec![],
    };
    let ast = &ctx.program.unit(unit).ast;
    let anonymous = ctx.js_classes.get(&c).is_some_and(|j| j.anonymous);
    let mut cw = Writer {
        indent: 1,
        ..Writer::default()
    };
    for &mid in &members {
        let mem = ast.member(mid);
        match &mem.kind {
            MemberKind::Method(fid) => {
                let af = ast.function(*fid);
                if af.external || !af.static_ {
                    continue;
                }
                let Some(fname) = af.name else { continue };
                let name = ctx.name(fname.sym).to_string();
                let key = if af.kind == ast::FunctionKind::Setter {
                    format!("{name}_=")
                } else {
                    name.clone()
                };
                let Some(feid) = ctx
                    .sym(&key)
                    .and_then(|s| class.static_members.get(&s).copied())
                else {
                    continue;
                };
                let sk = js::prop_key(&static_member_name(&name));
                let head = match af.kind {
                    ast::FunctionKind::Getter => format!("static get {sk}"),
                    ast::FunctionKind::Setter => format!("static set {sk}"),
                    _ => format!("static {sk}"),
                };
                let (text, _) = function_text(ctx, m, feid, Some(&head), Some(c), true, None);
                for line in text.lines() {
                    cw.line(line);
                }
            }
            MemberKind::Constructor(ctor) => {
                if ctor.factory && !ctor.external && ctor.redirect.is_none() {
                    emit_factory(ctx, m, c, unit, ctor, mid, &mut cw);
                } else if !ctor.factory
                    && !ctor.external
                    && class.kind == ClassKind::ExtensionType
                    && ctx.class_params[c.0 as usize].is_empty()
                {
                    // Non-`external` generative constructor of an interop extension
                    // type (`HTMLStyleElement() : _ = document.createElement('style')`
                    // from package:web): only an `external` member is interop
                    // (`usesJSInterop`, dev_compiler js_interop.dart:151-161); the
                    // rest is plain Dart, as in an erased extension type
                    // (`_emitLibraryProcedures`, compiler.dart:3527).
                    let sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
                    if let Some(fid) = sym.and_then(|s| class.constructors.get(&s).copied()) {
                        let text =
                            crate::extension_type::constructor_text(ctx, m, c, unit, ctor, fid);
                        for line in text.lines() {
                            cw.line(line);
                        }
                    }
                }
            }
            MemberKind::Field(_) => {}
        }
    }
    for (csym, cfid) in class.constructors() {
        let cf = ctx.program.function(cfid);
        let cn = ctx.name(csym);
        let jsname = if cn.is_empty() {
            "new".to_string()
        } else {
            static_member_name(cn)
        };
        let external = cf.external || cf.kind == FunctionKind::SyntheticConstructor;
        let call = if !external {
            format!("{cref}.{jsname}(...args)")
        } else if anonymous {
            // Named parameters become properties of the literal.
            let names: Vec<String> = ctx.outline.functions[cfid.0 as usize]
                .parameters
                .iter()
                .filter(|p| p.kind == ast::ParameterKind::Named)
                .filter_map(|p| p.name.map(|n| ctx.name(n).to_string()))
                .collect();
            // The literal's key is the JS name (interop); the read from `opts`
            // uses the call's key (`Ctx::named_key`).
            let props: Vec<String> = names
                .iter()
                .map(|n| {
                    let c = ctx.named_key(n);
                    format!(
                        "{}: opts && {} ? opts{} : null",
                        js::prop_key(n),
                        js::has_named("opts", &c),
                        js::prop_access(&c)
                    )
                })
                .collect();
            cw.line(&format!(
                "static [{}](opts) {{ return {{{}}}; }}",
                js::string_literal(&format!("_#{jsname}#tearOff")),
                props.join(", ")
            ));
            continue;
        } else {
            format!("new {}(...args)", ctx.js_class_global(c))
        };
        cw.line(&format!(
            "static [{}](...args) {{ return {call}; }}",
            js::string_literal(&format!("_#{jsname}#tearOff"))
        ));
    }
    crate::emit_line!(w, "{cref} = class {cname} {{");
    w.push_raw(&cw.out);
    w.line("};");
    // Non-`external` static fields (package:web's `Node.ELEMENT_NODE`) are
    // plain Dart, as the static methods above.
    let static_fields: Vec<VariableId> = class
        .fields
        .iter()
        .copied()
        .filter(|v| {
            let var = ctx.program.variable(*v);
            var.static_ && !var.external && ctx.var_state(*v) != crate::filter::State::Dead
        })
        .collect();
    crate::extension_type::emit_static_fields(ctx, m, c, &cref, &static_fields, w);
}

fn fix_equals_param(text: &str) -> String {
    // Makes sure the null test uses the actual name of `_equals`'s parameter.
    // The text starts with `_equals(<param>) {` — renames the test to the real name.
    if let Some(rest) = text.strip_prefix("_equals(") {
        if let Some(end) = rest.find(')') {
            let pname = rest[..end].trim().to_string();
            if !pname.is_empty() && pname != "other" {
                return text.replacen(
                    "if (other == null) return false;",
                    &format!("if ({pname} == null) return false;"),
                    1,
                );
            }
        }
    }
    text.to_string()
}

/// Whether a supertype of `c` from another program library declares private
/// instance members (which `c` cannot implement).
fn foreign_private_supertype(ctx: &Ctx, c: ClassId) -> bool {
    let library = ctx.program.class(c).library;
    let mut seen = HashSet::new();
    let mut queue: Vec<ClassId> = ctx.class_supers[c.0 as usize]
        .iter()
        .filter_map(|s| s.class())
        .collect();
    while let Some(k) = queue.pop() {
        if Some(k) == ctx.object || !seen.insert(k) {
            continue;
        }
        let class = ctx.program.class(k);
        if class.library != library
            && !ctx.libs[class.library.0 as usize].is_sdk
            && class
                .instance_members
                .keys()
                .any(|s| ctx.interner.resolve(*s).starts_with('_'))
        {
            return true;
        }
        queue.extend(
            ctx.class_supers[k.0 as usize]
                .iter()
                .filter_map(|s| s.class()),
        );
    }
    false
}

/// Whether the private field `name` of `c` is an accessor pair over its own
/// storage: it overrides an inherited member, or a subtype overrides it.
pub(crate) fn private_field_is_virtual(ctx: &Ctx, c: ClassId, name: &str) -> bool {
    overrides_supertype_member(ctx, c, name) || overridden_in_library(ctx, c, name)
}

/// Whether another class of `c`'s library that has `c` as a supertype
/// declares an instance member `name` (a private member is only overridden in
/// its own library).
fn overridden_in_library(ctx: &Ctx, c: ClassId, name: &str) -> bool {
    let keys: Vec<_> = [ctx.sym(name), ctx.sym(&format!("{name}_="))]
        .into_iter()
        .flatten()
        .collect();
    let library = ctx.program.class(c).library;
    ctx.program.classes.iter().enumerate().any(|(i, k)| {
        let k_id = ClassId(i as u32);
        k_id != c
            && k.library == library
            && keys.iter().any(|s| k.instance_members.contains_key(s))
            && ctx.as_super(&ctx.this_ty(k_id), c).is_some()
    })
}

/// Whether a proper supertype of `c` declares an instance member `name`.
fn overrides_supertype_member(ctx: &Ctx, c: ClassId, name: &str) -> bool {
    let keys: Vec<_> = [ctx.sym(name), ctx.sym(&format!("{name}_="))]
        .into_iter()
        .flatten()
        .collect();
    let mut seen = HashSet::new();
    let mut queue: Vec<ClassId> = ctx.class_supers[c.0 as usize]
        .iter()
        .filter_map(|s| s.class())
        .collect();
    while let Some(k) = queue.pop() {
        if Some(k) == ctx.object || !seen.insert(k) {
            continue;
        }
        let class = ctx.program.class(k);
        // A private name is only the same member within its library.
        let comparable = !name.starts_with('_') || class.library == ctx.program.class(c).library;
        if comparable && keys.iter().any(|s| class.instance_members.contains_key(s)) {
            return true;
        }
        queue.extend(
            ctx.class_supers[k.0 as usize]
                .iter()
                .filter_map(|s| s.class()),
        );
    }
    false
}

/// Separator of a signature entry that carries its own JS key,
/// `\u{1}name\u{1}key` (a thrower for a private member of another library).
const RAW_SIG_KEY: char = '\u{1}';

/// The Dart name of a signature entry.
fn signature_name(entry: &str) -> &str {
    match entry.strip_prefix(RAW_SIG_KEY) {
        Some(rest) => rest.split(RAW_SIG_KEY).next().unwrap_or(rest),
        None => entry,
    }
}

fn sig_key(name: &str, e: &FnEmitter, c: ClassId) -> String {
    if let Some((_, key)) = name
        .strip_prefix(RAW_SIG_KEY)
        .and_then(|rest| rest.split_once(RAW_SIG_KEY))
    {
        return key.to_string();
    }
    // SDK mode: for a native class, the key is the `dartx` symbol (the declaration's).
    if crate::own_sdk::is_native_class(e.ctx, c)
        && (!name.starts_with('_') || matches!(name, "_equals" | "_get" | "_set" | "_negate"))
    {
        return crate::own_sdk::native_key(e.m, name);
    }
    if name.starts_with('_') && !matches!(name, "_equals" | "_get" | "_set" | "_negate") {
        e.decl_member_key(c, name)
    } else {
        js::prop_key(&js_member_name(name))
    }
}

/// Public instance member names of supertypes that are native interfaces.
fn native_member_names(ctx: &Ctx, c: ClassId) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut queue = vec![c];
    if let Some(o) = ctx.object {
        queue.push(o);
    }
    while let Some(k) = queue.pop() {
        if !seen.insert(k.0) {
            continue;
        }
        let class = ctx.program.class(k);
        if ctx.ext_set.contains(&k) {
            for (&sym, &fid) in &class.instance_members {
                let f = ctx.program.function(fid);
                let n = ctx.name(f.name);
                if !n.starts_with('_') {
                    let _ = sym;
                    names.insert(n.to_string());
                }
            }
        }
        for s in &ctx.class_supers[k.0 as usize] {
            if let Some(sc) = s.class() {
                queue.push(sc);
            }
        }
    }
    names
}

/// JS reference to the superclass (with the mixin applications emitted before it).
fn superclass_js(ctx: &Ctx, m: &ModState, c: ClassId, w: &mut Writer) -> String {
    let class = ctx.program.class(c);
    let (mut base, base_class) = if class.kind == ClassKind::Enum {
        m.use_sdk("core");
        ("core._Enum".to_string(), ctx.underscore_enum)
    } else {
        match ctx.superclass_of(c) {
            Some(s) if Some(s) != ctx.object => {
                let tmp = FnEmitter::new(
                    ctx,
                    m,
                    class.decl.map(|d| d.unit).unwrap_or(UnitId(0)),
                    None,
                    true,
                );
                (tmp.class_ref(s), Some(s))
            }
            _ => {
                m.use_sdk("core");
                ("core.Object".to_string(), None)
            }
        }
    };
    // A mixin that applies mixins itself (`mixin class A = Object with M`, or
    // a class with `with`): its mixins come first, then its own members —
    // `dart.applyMixin` only copies the members declared in the class.
    fn expand(ctx: &Ctx, mx: ClassId, depth: usize, out: &mut Vec<ClassId>) {
        if depth < 32 {
            for &inner in ctx.mixins_of(mx).iter() {
                expand(ctx, inner, depth + 1, out);
            }
        }
        out.push(mx);
    }
    let mut mixins = Vec::new();
    for &mx in ctx.mixins_of(c).iter() {
        expand(ctx, mx, 0, &mut mixins);
    }
    for (i, &mx) in mixins.iter().enumerate() {
        let tmp = FnEmitter::new(
            ctx,
            m,
            class.decl.map(|d| d.unit).unwrap_or(UnitId(0)),
            None,
            true,
        );
        let mref = tmp.class_ref(mx);
        let app = format!("{}$mixin{}", ctx.class_name(c), i);
        crate::emit_line!(w, "const {app} = class {app} extends {base} {{}};");
        // Forwarding constructors to the superclass.
        let mut ctor_names: Vec<String> = match base_class {
            Some(b) if Some(b) != ctx.object => ctx
                .program
                .class(b)
                .constructors()
                .into_iter()
                // With a filter: only forwards to a base constructor that is live.
                .filter(|(_, f)| {
                    !ctx.program.function(*f).factory
                        && ctx.fn_state(*f) != crate::filter::State::Dead
                })
                .map(|(s, _)| {
                    let n = ctx.name(s);
                    if n.is_empty() {
                        "new".to_string()
                    } else {
                        n.to_string()
                    }
                })
                .collect(),
            _ => vec!["new".into()],
        };
        // Base without a declared constructor: its synthetic `new` (with a filter,
        // it exists only if the base is instantiated).
        let base_without_ctor = base_class.is_some_and(|b| {
            ctx.program
                .class(b)
                .constructors
                .values()
                .all(|f| ctx.program.function(*f).factory)
        });
        if ctor_names.is_empty()
            && (ctx.filter.is_none()
                || (base_without_ctor
                    && base_class
                        .is_some_and(|b| ctx.class_level(b) == crate::filter::Level::Instantiated)))
        {
            ctor_names.push("new".into());
        }
        for n in ctor_names {
            let n = if n == "new" {
                n
            } else {
                static_member_name(&n)
            };
            if base == "core.Object" {
                crate::emit_line!(
                    w,
                    "({app}.{n} = function() {{ if ({mref}[dart.mixinNew]) {mref}[dart.mixinNew].call(this); }}).prototype = {app}.prototype;"
                );
            } else {
                crate::emit_line!(
                    w,
                    "({app}.{n} = function(...args) {{ if ({mref}[dart.mixinNew]) {mref}[dart.mixinNew].call(this); {base}.{n}.apply(this, args); }}).prototype = {app}.prototype;"
                );
            }
        }
        crate::emit_line!(w, "dart.applyMixin({app}, {mref});");
        base = app;
    }
    base
}

/// Initializers of declared fields (`int x = 0;` / `null`), except those already
/// initialized by `this.x` or by the initializer list.
/// Field initializations to `null`, in production chained in one statement
/// (`this.c = this.b = this.a = null;`, written backwards so that the
/// properties are still created in declaration order): the views of an ngdart
/// project initialize dozens of nullable fields in a row.
#[derive(Default)]
struct NullInits {
    production: bool,
    targets: Vec<String>,
}

impl NullInits {
    fn push(&mut self, target: String, body: &mut Writer) {
        if self.production {
            self.targets.push(target);
        } else {
            crate::emit_line!(body, "{target} = null;");
        }
    }

    fn flush(&mut self, body: &mut Writer) {
        if !self.targets.is_empty() {
            self.targets.reverse();
            crate::emit_line!(body, "{} = null;", self.targets.join(" = "));
            self.targets.clear();
        }
    }
}

fn emit_field_inits(
    ctx: &Ctx,
    m: &ModState,
    c: ClassId,
    fields: &[FieldInfo],
    skip: &HashSet<String>,
    body: &mut Writer,
) {
    let mut nulls = NullInits {
        production: ctx.production(),
        ..NullInits::default()
    };
    for f in fields {
        if skip.contains(&f.name) || f.helper {
            continue;
        }
        let target = match &f.storage {
            Some(s) => format!("this[{s}]"),
            None => format!(
                "this{}",
                js::prop_access(&crate::body::js_member_name(&f.name))
            ),
        };
        match f.init {
            Some(i) if !f.late => {
                nulls.flush(body);
                let mut e = FnEmitter::new(ctx, m, f.unit, Some(c), false);
                let (js, _) = e.emit_expr(i, Some(&f.ty));
                // Each initializer has its own emitter, whose temporaries go
                // in a block of their own.
                let block = !e.temps.is_empty();
                if block {
                    body.line(&format!("{{ let {};", e.temps.join(", ")));
                }
                for line in e.w.out.lines() {
                    body.line(line);
                }
                crate::emit_line!(body, "{target} = {};", js.code);
                if block {
                    body.line("}");
                }
            }
            _ => nulls.push(target, body),
        }
    }
    nulls.flush(body);
}

/// Whether the generative constructor `ctor` stores the class's fields for the
/// first time in their declaration order without the leading `null`s: first
/// the fields it does not `skip` (their initializers, in order, by
/// [`emit_field_inits`]), then the `this.x` parameters and the field
/// initializers of its list, in that order (what the constructor emits after
/// them).
fn const_stores_in_order(
    ctx: &Ctx,
    fields: &[FieldInfo],
    ctor: &ast::Constructor,
    skip: &HashSet<String>,
) -> bool {
    let mut order: Vec<String> = fields
        .iter()
        .filter(|f| !f.helper && !skip.contains(&f.name))
        .map(|f| f.name.clone())
        .collect();
    let mut stored = |name: String| {
        if fields.iter().any(|f| f.name == name) && !order.contains(&name) {
            order.push(name);
        }
    };
    for p in ctor.parameters.iter() {
        if p.this_
            && let Some(n) = p.name
        {
            stored(ctx.name(n.sym).to_string());
        }
    }
    for i in ctor.initializers.iter() {
        if let ast::Initializer::Field { name, .. } = i {
            stored(ctx.name(name.sym).to_string());
        }
    }
    let declared: Vec<&String> = fields
        .iter()
        .filter(|f| !f.helper)
        .map(|f| &f.name)
        .collect();
    order.iter().collect::<Vec<_>>() == declared
}

/// The body of a `mixin` shell: empty (as it always was), or its statics.
fn mixin_shell(w: &Writer) -> String {
    if w.out.is_empty() {
        String::new()
    } else {
        format!("\n{}", w.out)
    }
}

/// The class is a named mixin application (`class C = S with M;`).
fn is_mixin_application(ctx: &Ctx, c: ClassId) -> bool {
    let Some(d) = ctx.program.class(c).decl else {
        return false;
    };
    matches!(&ctx.program.unit(d.unit).ast.decl(d.decl).kind, ast::DeclKind::Class(k) if k.mixin_application)
}

/// Is `ctor` the elaborated primary constructor (Dart 3.13) of class `c`?
fn is_primary_constructor(ctx: &Ctx, c: ClassId, ctor: &ast::Constructor) -> bool {
    let Some(d) = ctx.program.class(c).decl else {
        return false;
    };
    let ast = &ctx.program.unit(d.unit).ast;
    let primary = match &ast.decl(d.decl).kind {
        ast::DeclKind::Class(k) => k.primary_constructor,
        ast::DeclKind::Enum(k) => k.primary_constructor,
        _ => None,
    };
    primary.is_some_and(|mid| matches!(&ast.member(mid).kind, ast::MemberKind::Constructor(k) if std::ptr::eq(k, ctor)))
}

/// Field initializers emitted by the constructor's own emitter (primary
/// constructor): its parameters are in scope.
fn emit_field_inits_in_constructor(
    e: &mut FnEmitter,
    fields: &[FieldInfo],
    skip: &HashSet<String>,
    body: &mut Writer,
) {
    let mut nulls = NullInits {
        production: e.ctx.production(),
        ..NullInits::default()
    };
    for f in fields {
        if skip.contains(&f.name) || f.helper {
            continue;
        }
        let target = match &f.storage {
            Some(s) => format!("this[{s}]"),
            None => format!(
                "this{}",
                js::prop_access(&crate::body::js_member_name(&f.name))
            ),
        };
        match f.init {
            Some(i) if !f.late => {
                nulls.flush(body);
                let (js, _) = e.emit_expr(i, Some(&f.ty));
                flush_stmts(e, body);
                crate::emit_line!(body, "{target} = {};", js.code);
            }
            _ => nulls.push(target, body),
        }
    }
    nulls.flush(body);
}

fn emit_super_call_default(ctx: &Ctx, m: &ModState, c: ClassId, body: &mut Writer) {
    let class = ctx.program.class(c);
    let sup = match ctx.superclass_of(c) {
        Some(s) if Some(s) != ctx.object => Some(s),
        _ => None,
    };
    if sup.is_none() && ctx.mixins_of(c).is_empty() {
        return;
    }
    let tmp = FnEmitter::new(
        ctx,
        m,
        class.decl.map(|d| d.unit).unwrap_or(UnitId(0)),
        None,
        true,
    );
    let sref = sup
        .map(|s| tmp.class_ref(s))
        .unwrap_or_else(|| "core.Object".to_string());
    let sgeneric = sup.is_some_and(|s| ctx.requires_rti(s));
    if ctx.mixins_of(c).is_empty() && sup.is_some_and(|s| trivial_constructor(ctx, s, None)) {
        return;
    }
    let base = mixin_base_ref(ctx, c, &sref);
    if sgeneric {
        crate::emit_line!(body, "{base}.new.call(this, null);");
    } else {
        crate::emit_line!(body, "{base}.new.call(this);");
    }
}

/// Production: whether the generative constructor `sym` of `s` does nothing
/// when called without arguments, so a subclass's `super()` call can be left
/// out: no parameters, no initializers, an empty body, no instance fields to
/// initialize, no type arguments to record, no mixin applications, and a
/// superclass constructor (the implicit `super()`) that does nothing either.
/// `DARTFORGE_JSPROD_EMPTY_SUPER_CALLS=0` keeps every call.
fn trivial_constructor(ctx: &Ctx, s: ClassId, sym: Option<dartforge_intern::SymbolId>) -> bool {
    fn go(ctx: &Ctx, s: ClassId, sym: dartforge_intern::SymbolId, depth: u32) -> bool {
        if Some(s) == ctx.object {
            return true;
        }
        let class = ctx.program.class(s);
        if depth > 64
            || class.kind != ClassKind::Class
            || ctx.requires_rti(s)
            || !ctx.mixins_of(s).is_empty()
            || ctx.is_js_class(s)
            || class
                .fields
                .iter()
                .any(|&v| !ctx.program.variable(v).static_)
        {
            return false;
        }
        let Some(&fid) = class.constructors.get(&sym) else {
            return false;
        };
        let func = ctx.program.function(fid);
        if func.factory || func.external && func.patched_by.is_none() {
            return false;
        }
        let node = func
            .patched_by
            .map_or(func.node, |p| ctx.program.function(p).node);
        let implicit_super = match node {
            FunctionRef::Constructor { unit, member } => {
                let ast = &ctx.program.unit(unit).ast;
                let MemberKind::Constructor(ctor) = &ast.member(member).kind else {
                    return false;
                };
                let empty_body = match ctor.body {
                    FunctionBody::Empty => true,
                    FunctionBody::Block(b) => {
                        matches!(&ast.stmt(b).kind, ast::StmtKind::Block(v) if v.is_empty())
                    }
                    _ => false,
                };
                if ctor.external
                    || ctor.factory
                    || !ctor.parameters.is_empty()
                    || !ctor.initializers.is_empty()
                    || ctor.redirect.is_some()
                    || !empty_body
                {
                    return false;
                }
                true
            }
            FunctionRef::None => func.kind == FunctionKind::SyntheticConstructor,
            _ => false,
        };
        implicit_super
            && match ctx.superclass_of(s) {
                None => true,
                Some(sup) => ctx.empty_sym.is_some_and(|e| go(ctx, sup, e, depth + 1)),
            }
    }
    ctx.production()
        && std::env::var("DARTFORGE_JSPROD_EMPTY_SUPER_CALLS").map_or(true, |v| v != "0")
        && sym.or(ctx.empty_sym).is_some_and(|sym| go(ctx, s, sym, 0))
}

/// Production: `Object.prototype[dartx.toString]` calls `this.toString()`
/// instead of being `Object`'s own `toString`, and the classes that cannot
/// sit in a native prototype chain drop their `dartx.toString` alias: the
/// runtime's `dart.str`/`dart.toString` read `o[dartx.toString]`, which then
/// reaches the object's own JavaScript `toString` (the Dart override, or
/// `Object`'s). A tear-off through the symbol binds the JavaScript
/// `toString` of a Dart object (`toStringTearoff`, patched), so it equals
/// the other tear-offs of the same method. `dartx._equals` and
/// `dartx.hashCode` keep their aliases: the hash collections compare
/// `o[dartx._equals]` with `Object`'s to choose identity semantics. The native chains keep their aliases, which
/// `registerExtension` copies onto the JavaScript prototypes.
/// `DARTFORGE_JSPROD_TOSTRING_FORWARDER=0` keeps every alias.
fn object_member_forwarders(ctx: &Ctx) -> bool {
    ctx.production()
        && ctx.sdk.is_some()
        && std::env::var("DARTFORGE_JSPROD_TOSTRING_FORWARDER").map_or(true, |v| v != "0")
}

/// `c`, one of its superclasses or one of their mixins (but `Object`) sits
/// in the prototype chain of a native class: its `dartx` aliases stay, and so
/// do those of its subclasses (an ancestor's alias would shadow the
/// inherited forwarder).
fn in_or_under_native_chain(ctx: &Ctx, c: ClassId) -> bool {
    let mut k = Some(c);
    let mut depth = 0;
    while let Some(x) = k {
        if Some(x) == ctx.object {
            return false;
        }
        if depth > 256
            || ctx.in_native_chain(x)
            || ctx.mixins_of(x).iter().any(|m| ctx.in_native_chain(*m))
        {
            return true;
        }
        depth += 1;
        k = ctx.superclass_of(x);
    }
    false
}

/// If the class has mixins, the constructor of the immediate superclass is that of the last application.
fn mixin_base_ref(ctx: &Ctx, c: ClassId, sref: &str) -> String {
    if ctx.mixins_of(c).is_empty() {
        sref.to_string()
    } else {
        format!("{}$mixin{}", ctx.class_name(c), ctx.mixins_of(c).len() - 1)
    }
}

/// Text `function(params) { ... }` of a generative constructor.
fn emit_constructor(
    ctx: &Ctx,
    m: &ModState,
    c: ClassId,
    unit: UnitId,
    ctor: &ast::Constructor,
    fields: &[FieldInfo],
    generic: bool,
    is_enum: bool,
) -> String {
    let class = ctx.program.class(c);
    let mut e = FnEmitter::new(ctx, m, unit, Some(c), false);
    e.explicit_super = true;
    e.in_constructor = true;
    let ctor_sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
    let fid = ctor_sym.and_then(|s| class.constructors.get(&s).copied());
    if let Some(f) = fid {
        e.checks = crate::checks::Checks::of_function(ctx, f);
    }
    let sig = fid.map(|f| ctx.fn_ty(f));
    let (pjs, prologue) = e.declare_params(&ctor.parameters, sig.as_ref());
    let mut params: Vec<String> = Vec::new();
    if is_enum {
        params.push("t$index".into());
        params.push("t$name".into());
    }
    if generic {
        params.push("_ti".into());
    }
    if !pjs.is_empty() {
        params.push(pjs);
    }
    let mut body = Writer {
        indent: 1,
        ..Writer::default()
    };
    for line in prologue.lines() {
        body.line(line);
    }
    if generic {
        body.line("this.$ti = this.$ti || _ti || dart.getReifiedType(this);");
    }
    // Redirection `: this(...)`.
    let redirect = ctor.initializers.iter().find_map(|i| match i {
        ast::Initializer::Redirect {
            constructor,
            arguments,
            ..
        } => Some((constructor, arguments)),
        _ => None,
    });
    let cref = e.class_ref(c);
    if let Some((target, args)) = redirect {
        let tname = target
            .map(|n| static_member_name(ctx.name(n.sym)))
            .unwrap_or("new".into());
        let tsym = target.map(|n| n.sym).or(ctx.empty_sym);
        let tfid = tsym.and_then(|s| class.constructors.get(&s).copied());
        let tsig = tfid.map(|f| ctx.fn_ty(f)).unwrap_or(Ty::Dynamic);
        let (arg_js, _, _) = e.emit_args_for(&tsig, args, None);
        let mut all: Vec<String> = vec!["this".into()];
        if is_enum {
            all.push("t$index".into());
            all.push("t$name".into());
        }
        if generic {
            all.push("null".into());
        }
        all.extend(arg_js);
        for line in e.w.out.lines() {
            body.line(line);
        }
        body.line(&format!("{cref}.{tname}.call({});", all.join(", ")));
        let mut text = String::new();
        text.push_str(&format!("function({}) {{\n", params.join(", ")));
        if !e.temps.is_empty() {
            text.push_str(&format!("  let {};\n", e.temps.join(", ")));
        }
        text.push_str(&body.out);
        text.push('}');
        return text;
    }
    // Field initializers run first (even when `this.x` or the list overwrite
    // them); fields without an initializer that will be assigned do not need the
    // initial `null`.
    let mut skip: HashSet<String> = HashSet::new();
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                let name = ctx.name(n.sym).to_string();
                if fields.iter().any(|f| f.name == name && f.init.is_none()) {
                    skip.insert(name);
                }
            }
        }
    }
    for i in ctor.initializers.iter() {
        if let ast::Initializer::Field { name, .. } = i {
            let name = ctx.name(name.sym).to_string();
            if fields.iter().any(|f| f.name == name && f.init.is_none()) {
                skip.insert(name);
            }
        }
    }
    // A `const` constructor stores every field in declaration order first:
    // the runtime canonicalizes constants by their properties in insertion
    // order, which must not depend on the constructor that made them.
    // Production: when this constructor's own stores already come in
    // declaration order, the `null`s add nothing to the order, and they would
    // cost: V8 gives a field first stored as `null` the general
    // representation, so every `double` stored into it afterwards is boxed
    // (`DARTFORGE_JSPROD_CONST_ORDER=0` keeps them).
    if ctor.const_
        && !(ctx.production()
            && std::env::var("DARTFORGE_JSPROD_CONST_ORDER").map_or(true, |v| v != "0")
            && const_stores_in_order(ctx, fields, ctor, &skip))
    {
        skip.clear();
    }
    if is_primary_constructor(ctx, c, ctor) {
        // Primary constructor (3.13): the non-`late` field initializers are in
        // the scope of its parameters ("primary initializer scope"). Emitted by
        // the constructor's own emitter, which already declared them — that is
        // what DDC does (`this[up] = cru[$toUpperCase]()`).
        emit_field_inits_in_constructor(&mut e, fields, &skip, &mut body);
    } else {
        emit_field_inits(ctx, m, c, fields, &skip, &mut body);
    }
    // `this.x` params.
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                let name = ctx.name(n.sym).to_string();
                let target = field_target(fields, &name);
                let jsn = e.param_js_name(n);
                crate::emit_line!(body, "{target} = {jsn};");
                mark_set(fields, &name, &mut body);
            }
        }
    }
    // Initializer list.
    let mut super_call: Option<String> = None;
    for i in ctor.initializers.iter() {
        match i {
            ast::Initializer::Field { name, value, .. } => {
                let n = ctx.name(name.sym).to_string();
                let fty = fields.iter().find(|f| f.name == n).map(|f| f.ty.clone());
                let (js, _) = e.emit_expr(*value, fty.as_ref());
                flush_stmts(&mut e, &mut body);
                let target = field_target(fields, &n);
                crate::emit_line!(body, "{target} = {};", js.code);
                mark_set(fields, &n, &mut body);
            }
            ast::Initializer::Assert { .. } if ctx.filter.is_some_and(|f| !f.keep_asserts()) => {}
            ast::Initializer::Assert {
                condition, message, ..
            } => {
                let (cjs, _) = e.emit_cond(*condition);
                let msg = message
                    .map(|mm| e.emit_expr(mm, None).0.code)
                    .unwrap_or("null".into());
                flush_stmts(&mut e, &mut body);
                crate::emit_line!(
                    body,
                    "if (!({cjs})) dart.assertFailed({msg}, null, 0, 0, \"\");"
                );
            }
            ast::Initializer::Super {
                constructor,
                arguments,
                ..
            } => {
                let sup = ctx.superclass_of(c);
                if let Some(s) = sup {
                    if Some(s) != ctx.object {
                        let sname = constructor
                            .map(|n| static_member_name(ctx.name(n.sym)))
                            .unwrap_or("new".into());
                        let ssym = constructor.map(|n| n.sym).or(ctx.empty_sym);
                        let sfid =
                            ssym.and_then(|x| ctx.program.class(s).constructors.get(&x).copied());
                        let ssig = sfid.map(|f| ctx.fn_ty(f)).unwrap_or(Ty::Dynamic);
                        let (arg_js, _, _) = e.emit_args_for(&ssig, arguments, None);
                        flush_stmts(&mut e, &mut body);
                        let sref = e.class_ref(s);
                        let base = mixin_base_ref(ctx, c, &sref);
                        let mut all: Vec<String> = vec!["this".into()];
                        if is_enum {
                            all.push("t$index".into());
                            all.push("t$name".into());
                        } else if ctx.requires_rti(s) {
                            all.push("null".into());
                        }
                        // Positional `super.x` go after the explicit ones; named ones join `opts`.
                        let mut pos_js: Vec<String> = Vec::new();
                        let mut named_js: Vec<String> = Vec::new();
                        let mut opts_js: Option<String> = None;
                        for a in arg_js {
                            if a.starts_with('{') && a.ends_with('}') {
                                opts_js = Some(a);
                            } else {
                                pos_js.push(a);
                            }
                        }
                        for p in ctor.parameters.iter().filter(|p| p.super_) {
                            let Some(n) = p.name else { continue };
                            let jsn = e.param_js_name(n);
                            if p.kind == ast::ParameterKind::Named {
                                named_js.push(format!(
                                    "{}: {jsn}",
                                    js::prop_key(&ctx.named_key(ctx.name(n.sym)))
                                ));
                            } else {
                                pos_js.push(jsn);
                            }
                        }
                        all.extend(pos_js);
                        match (opts_js, named_js.is_empty()) {
                            (Some(o), true) => all.push(o),
                            (Some(o), false) => {
                                all.push(format!("{{...{o}, {}}}", named_js.join(", ")))
                            }
                            (None, false) => all.push(format!("{{{}}}", named_js.join(", "))),
                            (None, true) => {}
                        }
                        // An empty constructor called without arguments
                        // (`trivial_constructor`) needs no call.
                        let trivial = !is_enum
                            && ctx.mixins_of(c).is_empty()
                            && arguments.args.is_empty()
                            && !ctor.parameters.iter().any(|p| p.super_)
                            && trivial_constructor(ctx, s, constructor.map(|n| n.sym));
                        super_call = if trivial {
                            Some(String::new())
                        } else {
                            Some(format!("{base}.{sname}.call({});", all.join(", ")))
                        };
                    }
                }
            }
            ast::Initializer::Redirect { .. } => {}
        }
    }
    // `super.x` params (Dart 2.17) → passed to the super constructor.
    if super_call.is_none() {
        if is_enum {
            m.use_sdk("core");
            let base = mixin_base_ref(ctx, c, "core._Enum");
            super_call = Some(format!("{base}.new.call(this, t$index, t$name);"));
        } else if ctx.superclass_of(c).is_none_or(|s| Some(s) == ctx.object)
            && !ctx.mixins_of(c).is_empty()
        {
            let base = mixin_base_ref(ctx, c, "core.Object");
            super_call = Some(format!("{base}.new.call(this);"));
        } else if let Some(s) = ctx.superclass_of(c) {
            if Some(s) != ctx.object
                && !(ctx.mixins_of(c).is_empty()
                    && !ctor.parameters.iter().any(|p| p.super_)
                    && trivial_constructor(ctx, s, None))
            {
                let sref = e.class_ref(s);
                let base = mixin_base_ref(ctx, c, &sref);
                let mut super_params: Vec<String> = Vec::new();
                let mut super_named: Vec<String> = Vec::new();
                for p in ctor.parameters.iter().filter(|p| p.super_) {
                    let Some(n) = p.name else { continue };
                    let jsn = e.param_js_name(n);
                    if p.kind == ast::ParameterKind::Named {
                        let named_key = ctx.named_key(ctx.name(n.sym));
                        let key = js::prop_key(&named_key);
                        if p.default_value.is_none() && !p.required {
                            super_named.push(format!(
                                "...(opts && {} ? {{{key}: {jsn}}} : {{}})",
                                js::has_named("opts", &named_key)
                            ));
                        } else {
                            super_named.push(format!("{key}: {jsn}"));
                        }
                    } else {
                        super_params.push(jsn);
                    }
                }
                let mut all: Vec<String> = vec!["this".into()];
                if ctx.requires_rti(s) {
                    all.push("null".into());
                }
                all.extend(super_params);
                if !super_named.is_empty() {
                    // Optional positionals of the super constructor omitted before `opts`.
                    let ssym = ctx.empty_sym;
                    if let Some(sfid) =
                        ssym.and_then(|x| ctx.program.class(s).constructors.get(&x).copied())
                    {
                        let n_pos = ctx.outline.functions[sfid.0 as usize]
                            .parameters
                            .iter()
                            .filter(|p| p.kind != ast::ParameterKind::Named)
                            .count();
                        while all.len() - 1 - (if ctx.requires_rti(s) { 1 } else { 0 }) < n_pos {
                            all.push("void 0".into());
                        }
                    }
                    all.push(format!("{{{}}}", super_named.join(", ")));
                }
                super_call = Some(format!("{base}.new.call({});", all.join(", ")));
            }
        }
    }
    if let Some(sc) = super_call.filter(|sc| !sc.is_empty()) {
        body.line(&sc);
    }
    // In the body, the `x` of `this.x` refers to the field.
    for p in ctor.parameters.iter() {
        if p.this_ {
            if let Some(n) = p.name {
                for sc in e.scopes.iter_mut() {
                    sc.remove(&n.sym);
                }
            }
        }
    }
    // Body.
    e.emit_body(&ctor.body);
    flush_stmts(&mut e, &mut body);
    let mut text = String::new();
    text.push_str(&format!("function({}) {{\n", params.join(", ")));
    if !e.temps.is_empty() {
        text.push_str(&format!("  let {};\n", e.temps.join(", ")));
    }
    text.push_str(&body.out);
    text.push('}');
    text
}

pub(crate) fn flush_stmts(e: &mut FnEmitter, body: &mut Writer) {
    let out = std::mem::take(&mut e.w.out);
    for line in out.lines() {
        body.line(line);
    }
}

/// `(C.name = f).prototype = C.prototype;`. A name that the constructor
/// function `C` already has as a read-only property (`length`, `caller`,
/// `arguments`), or that a superclass's static occupies (an accessor
/// without a setter), is defined with `dart.defineValue`, as the DDC does.
fn constructor_assignment(ctx: &Ctx, c: ClassId, cref: &str, jsname: &str, text: &str) -> String {
    // A superclass's static field or getter is an accessor on its
    // constructor function (methods and constructors are writable data).
    let inherited = std::iter::successors(ctx.superclass_of(c), |&k| ctx.superclass_of(k))
        .take_while(|&k| Some(k) != ctx.object)
        .any(|k| {
            matches!(
                ctx.declared_static(k, jsname, false),
                Some(crate::ctx::MemberKind::Field(_) | crate::ctx::MemberKind::Getter(_))
            )
        });
    if matches!(jsname, "length" | "caller" | "arguments") || inherited {
        format!(
            "dart.defineValue({cref}, {}, {text}).prototype = {cref}.prototype;",
            js::string_literal(jsname)
        )
    } else {
        format!("({cref}.{jsname} = {text}).prototype = {cref}.prototype;")
    }
}

/// A constructor's write of a nullable `late` field also records that it was
/// set.
fn mark_set(fields: &[FieldInfo], name: &str, body: &mut Writer) {
    if let Some(set) = fields
        .iter()
        .find(|f| f.name == name)
        .and_then(|f| f.is_set.as_ref())
    {
        crate::emit_line!(body, "this[{set}] = true;");
    }
}

fn field_target(fields: &[FieldInfo], name: &str) -> String {
    match fields.iter().find(|f| f.name == name) {
        Some(FieldInfo {
            storage: Some(s), ..
        }) => format!("this[{s}]"),
        _ => format!(
            "this{}",
            js::prop_access(&crate::body::js_member_name(name))
        ),
    }
}

/// `static name(params) { body }` of a factory.
fn emit_factory(
    ctx: &Ctx,
    m: &ModState,
    c: ClassId,
    unit: UnitId,
    ctor: &ast::Constructor,
    mid: ast::MemberId,
    cw: &mut Writer,
) {
    let class = ctx.program.class(c);
    let name = ctor
        .name
        .map(|n| static_member_name(ctx.name(n.sym)))
        .unwrap_or("new".into());
    let ctor_sym = ctor.name.map(|n| n.sym).or(ctx.empty_sym);
    let fid = ctor_sym.and_then(|s| class.constructors.get(&s).copied());
    let _ = mid;
    // `_ti` goes in when the class *or a superclass* is generic (DDC's rule, the
    // same as for the generative constructor): `static deBytes(_ti, bytes)`.
    let generic = ctx.requires_rti(c);
    let mut e = FnEmitter::new(ctx, m, unit, Some(c), true);
    // In a factory the class's type parameters come in as function parameters.
    let mut params: Vec<String> = Vec::new();
    if generic {
        params.push("_ti".into());
        e.factory_ti = true;
    }
    if ctor.redirect.is_some() {
        // Redirection: direct calls already resolve the target; the body forwards
        // to serve the tear-offs (`C.new` as a value).
        let body = match fid.and_then(|f| e.factory_redirect_target(f)) {
            Some((tclass, tname, is_target_factory)) => {
                let tref = e.class_ref(tclass);
                let tjs = if tname.is_empty() {
                    "new".to_string()
                } else {
                    static_member_name(&tname)
                };
                let mut args: Vec<String> = Vec::new();
                if ctx.requires_rti(tclass) {
                    if tclass == c && generic {
                        args.push("_ti".into());
                    } else {
                        args.push(e.rti(&ctx.this_ty_default(tclass)));
                    }
                }
                args.push("...args".into());
                if is_target_factory {
                    format!("return {tref}.{tjs}({});", args.join(", "))
                } else {
                    format!("return new {tref}.{tjs}({});", args.join(", "))
                }
            }
            None => String::new(),
        };
        let mut ps = params.clone();
        ps.push("...args".into());
        cw.line(&format!(
            "static {}({}) {{ {body} }}",
            js::prop_key(&name),
            ps.join(", ")
        ));
        return;
    }
    let sig = fid.map(|f| ctx.fn_ty(f));
    let (pjs, prologue) = e.declare_params(&ctor.parameters, sig.as_ref());
    if !pjs.is_empty() {
        params.push(pjs);
    }
    let ret_ty = ctx.this_ty(c);
    e.ret_ty = ret_ty.clone();
    e.emit_body(&ctor.body);
    let body = finish_body(&mut e);
    let head = format!("static {}({}) {{", js::prop_key(&name), params.join(", "));
    let text = e.wrap_async_head(AsyncKind::None, &head, &prologue, &body, &ret_ty);
    for line in text.lines() {
        cw.line(line);
    }
}

#[allow(dead_code)]
fn _unused(_: Js, _: HashMap<u32, u32>, _: BTreeMap<u32, u32>, _: u8) {
    let _ = P_ASSIGN;
}

#[cfg(test)]
mod const_record_cache_tests {
    use super::*;

    #[test]
    fn derived_and_reused_fragments_preserve_per_library_const_record_uses() {
        let mut module = ModState::new(LibraryId(1));
        module.group = vec![LibraryId(1), LibraryId(2)];
        let fragment_state = module.derived();
        assert!(fragment_state.const_record_libraries.borrow().is_empty());
        fragment_state.note_const_record(LibraryId(2));
        module.merge(&fragment_state);
        assert_eq!(*module.const_record_libraries.borrow(), BTreeSet::from([2]));
        let fragment = Fragment::new("cached record body".into(), &fragment_state);
        let fresh_module = module.derived();
        fragment.apply_to(&fresh_module);
        fragment.apply_to(&fresh_module);
        assert_eq!(
            *fresh_module.const_record_libraries.borrow(),
            BTreeSet::from([2])
        );
        let second = fresh_module.derived();
        second.note_const_record(LibraryId(1));
        fresh_module.merge(&second);
        assert_eq!(
            *fresh_module.const_record_libraries.borrow(),
            BTreeSet::from([1, 2])
        );
    }
}

#[cfg(test)]
mod typed_read_guard_cache_tests {
    use super::*;

    #[test]
    fn captured_guard_uses_survive_fragment_merge_and_reuse() {
        let module = ModState::new(LibraryId(1));
        let state = module.derived();
        state.typed_read_guard_libraries.borrow_mut().insert(1);
        module.merge(&state);
        assert_eq!(
            *module.typed_read_guard_libraries.borrow(),
            BTreeSet::from([1])
        );
        let fragment = Fragment::new("cached typed loop".into(), &state);
        let reused = module.derived();
        fragment.apply_to(&reused);
        fragment.apply_to(&reused);
        let mut emitted = String::new();
        crate::typed_read_guards::emit(&reused, &mut emitted);
        assert_eq!(emitted.matches("const t$typedReadGuard1 =").count(), 1);
        let mut unused = String::new();
        crate::typed_read_guards::emit(&ModState::new(LibraryId(2)), &mut unused);
        assert!(unused.is_empty());
    }
}
