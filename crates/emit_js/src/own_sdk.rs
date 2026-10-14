//! The emitter's SDK mode: the `dart:` libraries compiled through our own
//! pipeline (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §5).
//!
//! User code never exercises what DDC does specially when compiling the SDK: the
//! `dart:_foreign_helper` intrinsics (`JS('', '#.x', o)`, `JS_GET_FLAG`,
//! `TYPE_REF<T>()`…), the native classes (members with a `dartx` symbol,
//! `registerExtension`), the runtime library (`dart:_runtime`, which **is** the
//! `dart` object) and the SDK module with its *bootstrap*. This module brings all
//! of that together; the rest of the emitter only consults it through small
//! hooks, all behind `Ctx::sdk`.
//!
//! Without `Ctx::sdk` no hook fires: the development profile and the production
//! path that links against `dart_sdk.js` emit byte for byte what they did before.
//!
//! The `compiler.dart:N` references are to DDC **3.6.2**.

use crate::body::{js_member_name, FnEmitter};
use crate::ctx::Ctx;
use crate::js::{self, Js, P_ADD, P_PRIMARY, P_REL};
use crate::module::ModState;
use crate::ty::Ty;
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionKind, FunctionRef, LibraryId, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, ExprId, ExprKind, StringPart};
use std::collections::{BTreeSet, HashMap, HashSet};

/// A call that DDC resolves at compile time, without calling any function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    /// `JS(type, 'template', a…)` (`compiler.dart:6421-6479`).
    Js,
    GetFlag,
    GetName,
    EmbeddedGlobal,
    ClassRef,
    TypeRef,
    LegacyTypeRef,
    RawFunctionRef,
    StringConcat,
    Builtin,
    RawException,
    RtiParameter,
    RuntimeLibrary,
    GetInterceptor,
    /// The runtime's `extensionSymbol('x')` → the `dartx.x` symbol.
    ExtensionSymbol,
    /// The runtime's `_jsInstanceOf(x, T)` → `x instanceof T`.
    InstanceOf,
    GetPrototypeOf,
    SetPrototypeOf,
    /// `staticInteropGlobalContext` (a `dart:_js_helper` getter) → `dart.global`.
    GlobalContext,
    UnsafeCast,
    ExtractTypeArguments,
    Spread,
}

/// What SDK mode knows about the program, computed once (`SdkMode::new`).
pub struct SdkMode {
    /// `@patch class X` declarations of each SDK class: their members (those
    /// that replace the `external` ones and the new ones) go into the class.
    pub patches: HashMap<ClassId, Vec<(UnitId, ast::DeclId)>>,
    /// `dart:_runtime`: the library whose object **is** `dart`.
    pub runtime: Option<LibraryId>,
    /// `dart:_rti`, for the `_as`/`_is` symbols of `JS_GET_NAME`.
    pub rti: Option<LibraryId>,
    pub intrinsics: HashMap<FunctionElementId, Intrinsic>,
    /// `@JSExportName('x')` of functions (top-level and members).
    pub export_names: HashMap<FunctionElementId, String>,
    /// `@JSExportName('x')` of top-level variables (`global_` → `dart.global`).
    pub var_export_names: HashMap<VariableId, String>,
    /// Tags of `@Native('A,B')`/`@JsPeerInterface(name: 'A')` per class
    /// (without those starting with `!`, `native_types.dart:143-155`).
    pub peers: HashMap<ClassId, Vec<String>>,
    /// Libraries with `@ReifyFunctionTypes(false)` on the `library` directive.
    pub no_reify_libs: HashSet<LibraryId>,
    /// Functions with `@ReifyFunctionTypes(false)`.
    pub no_reify_fns: HashSet<FunctionElementId>,
    /// Functions with `@NoReifyGeneric()`: no type parameters in the JS
    /// (`compiler.dart:3596-3598, 6394`).
    pub no_reify_generic: HashSet<FunctionElementId>,
    /// Parameters with `@nullCheck`, by the start of the name in the source (unit, byte).
    pub null_checked_params: HashSet<(u32, u32)>,
    /// Classes that are primitive in JS (`JSBool`, `JSNumber`, `JSString`): they
    /// get `definePrimitiveHashCode` (`compiler.dart:1114-1117`).
    pub primitives: HashSet<ClassId>,
    /// The SDK's `assert`s are not emitted (dart2js does not emit the SDK's
    /// either; the program's stay, as in the development profile).
    pub no_sdk_asserts: bool,
    /// Member names declared by descendants of each class (through the chain of
    /// superclasses and mixins) and the classes used as mixins:
    /// [`non_virtual_field`], computed on the first query.
    pub virtuality: std::cell::OnceCell<(HashMap<ClassId, HashSet<dartforge_intern::SymbolId>>, HashSet<ClassId>)>,
    /// Simple class names that repeat in the program ([`needs_library_uri`]).
    pub repeated_names: std::cell::OnceCell<HashSet<dartforge_intern::SymbolId>>,
    /// Complete canonical Number modulo certificate, independent of emit order.
    pub number_modulo_certified: std::cell::OnceCell<bool>,
}

/// `dart.setLibraryUri(C, uri)` is only read by the error message of `as` when
/// both types have the same name (`errors.dart:110-117`): in the production
/// profile it is emitted only for the classes whose simple name repeats.
pub fn needs_library_uri(ctx: &Ctx, c: ClassId) -> bool {
    let Some(s) = ctx.sdk.as_ref() else { return true };
    let p = ctx.program;
    let repeated = s.repeated_names.get_or_init(|| {
        let mut seen: HashSet<dartforge_intern::SymbolId> = HashSet::new();
        let mut rep: HashSet<dartforge_intern::SymbolId> = HashSet::new();
        for k in p.classes.iter() {
            if !seen.insert(k.name) {
                rep.insert(k.name);
            }
        }
        rep
    });
    repeated.contains(&p.class(c).name)
}

/// A public instance field that no class of the program overrides and that does
/// not override an inherited concrete member: in the production profile (closed
/// world) it becomes a plain property of the object, without the `get`/`set` pair
/// over a storage symbol that DDC writes for every public field because, being
/// modular, it does not know whether some module will override it
/// (`docs/JS-PRODUCAO-TAMANHO.md` §3.4). Native classes, mixins and classes used
/// as mixins stay as before.
pub fn non_virtual_field(ctx: &Ctx, c: ClassId, name: dartforge_intern::SymbolId) -> bool {
    let Some(s) = ctx.sdk.as_ref() else { return false };
    let p = ctx.program;
    // Only in program code: the SDK classes have accesses the emitter does not
    // see (`JS()`, *patches*, the runtime reading fields by name).
    if is_native_class(ctx, c) || ctx.libs[p.class(c).library.0 as usize].is_sdk || p.class(c).kind != dartforge_elements::model::ClassKind::Class {
        return false;
    }
    let (desc, mixins) = s.virtuality.get_or_init(|| {
        let mut desc: HashMap<ClassId, HashSet<dartforge_intern::SymbolId>> = HashMap::new();
        let mut mixins: HashSet<ClassId> = HashSet::new();
        for (i, k) in p.classes.iter().enumerate() {
            mixins.extend(k.mixin_classes.iter().copied());
            // The ancestors of `k` through the superclass and the applied mixins.
            let mut seen: HashSet<ClassId> = HashSet::new();
            let mut stack: Vec<ClassId> = k.supertype_class.into_iter().chain(k.mixin_classes.iter().copied()).collect();
            while let Some(a) = stack.pop() {
                if a.0 as usize == i || !seen.insert(a) {
                    continue;
                }
                desc.entry(a).or_default().extend(k.instance_members.keys().copied());
                let ka = p.class(a);
                stack.extend(ka.supertype_class);
                stack.extend(ka.mixin_classes.iter().copied());
            }
        }
        (desc, mixins)
    });
    if mixins.contains(&c) {
        return false;
    }
    let text = ctx.interner.resolve(name);
    let setter = ctx.interner.lookup(&format!("{text}_="));
    let has = |k: ClassId| {
        let m = &p.class(k).instance_members;
        m.contains_key(&name) || setter.is_some_and(|s| m.contains_key(&s))
    };
    // Overridden by a descendant.
    if desc.get(&c).is_some_and(|ns| ns.contains(&name) || setter.is_some_and(|s| ns.contains(&s))) {
        return false;
    }
    // Overrides a member of an ancestor (superclass or applied mixin).
    let mut seen: HashSet<ClassId> = HashSet::new();
    let k = p.class(c);
    let mut stack: Vec<ClassId> = k.supertype_class.into_iter().chain(k.mixin_classes.iter().copied()).collect();
    while let Some(a) = stack.pop() {
        if a == c || !seen.insert(a) {
            continue;
        }
        if has(a) {
            return false;
        }
        let ka = p.class(a);
        stack.extend(ka.supertype_class);
        stack.extend(ka.mixin_classes.iter().copied());
    }
    true
}

fn annotation_name<'a>(ctx: &'a Ctx, a: &ast::Annotation) -> &'a str {
    a.name.last().map(|n| ctx.interner.resolve(n.sym)).unwrap_or("")
}

/// First positional argument of an annotation, when it is a string literal
/// (or the named argument `name:` in `@JsPeerInterface(name: 'X')`).
fn annotation_text(ctx: &Ctx, unit: UnitId, a: &ast::Annotation) -> Option<String> {
    let args = a.arguments.as_ref()?;
    for arg in args.args.iter() {
        if let Some(n) = arg.name {
            if ctx.interner.resolve(n.sym) != "name" {
                continue;
            }
        }
        if let ExprKind::String(lit) = &ctx.program.unit(unit).ast.expr(arg.value).kind {
            return lit.constant_value().map(|v| v.to_string_lossy());
        }
    }
    None
}

fn annotation_bool(ctx: &Ctx, unit: UnitId, a: &ast::Annotation) -> Option<bool> {
    let args = a.arguments.as_ref()?;
    let first = args.args.first()?;
    match &ctx.program.unit(unit).ast.expr(first.value).kind {
        ExprKind::Bool(b) => Some(*b),
        _ => None,
    }
}

impl SdkMode {
    /// Reads from the program what SDK mode needs: intrinsics, DDC annotations,
    /// native classes.
    pub fn new(ctx: &Ctx) -> SdkMode {
        let p = ctx.program;
        let lib_by_uri = |uri: &str| p.libraries.iter().position(|l| l.uri == uri).map(|i| LibraryId(i as u32));
        let runtime = lib_by_uri("dart:_runtime");
        let rti = lib_by_uri("dart:_rti");
        let foreign = lib_by_uri("dart:_foreign_helper");
        let js_helper = lib_by_uri("dart:_js_helper");
        let internal = lib_by_uri("dart:_internal");
        let mut m = SdkMode {
            runtime,
            rti,
            intrinsics: HashMap::new(),
            export_names: HashMap::new(),
            var_export_names: HashMap::new(),
            peers: HashMap::new(),
            no_reify_libs: HashSet::new(),
            no_reify_fns: HashSet::new(),
            no_reify_generic: HashSet::new(),
            null_checked_params: HashSet::new(),
            primitives: [ctx.jsbool, ctx.jsnumber, ctx.jsstring].into_iter().flatten().collect(),
            no_sdk_asserts: true,
            patches: HashMap::new(),
            virtuality: std::cell::OnceCell::new(),
            repeated_names: std::cell::OnceCell::new(),
            number_modulo_certified: std::cell::OnceCell::new(),
        };
        // Intrinsics, by (library, name).
        for (i, f) in p.functions.iter().enumerate() {
            let fid = FunctionElementId(i as u32);
            if f.class.is_some() || f.extension.is_some() {
                continue;
            }
            let n = ctx.interner.resolve(f.name);
            let lib = Some(f.library);
            let k = if lib == foreign {
                match n {
                    "JS" => Some(Intrinsic::Js),
                    "JS_GET_FLAG" => Some(Intrinsic::GetFlag),
                    "JS_GET_NAME" => Some(Intrinsic::GetName),
                    "JS_EMBEDDED_GLOBAL" => Some(Intrinsic::EmbeddedGlobal),
                    "JS_CLASS_REF" => Some(Intrinsic::ClassRef),
                    "TYPE_REF" => Some(Intrinsic::TypeRef),
                    "LEGACY_TYPE_REF" => Some(Intrinsic::LegacyTypeRef),
                    "RAW_DART_FUNCTION_REF" => Some(Intrinsic::RawFunctionRef),
                    "JS_STRING_CONCAT" => Some(Intrinsic::StringConcat),
                    "JS_BUILTIN" => Some(Intrinsic::Builtin),
                    "JS_RAW_EXCEPTION" => Some(Intrinsic::RawException),
                    "JS_RTI_PARAMETER" => Some(Intrinsic::RtiParameter),
                    "DART_RUNTIME_LIBRARY" => Some(Intrinsic::RuntimeLibrary),
                    "getInterceptor" => Some(Intrinsic::GetInterceptor),
                    "spread" => Some(Intrinsic::Spread),
                    _ => None,
                }
            } else if lib == runtime {
                match n {
                    "extensionSymbol" => Some(Intrinsic::ExtensionSymbol),
                    "_jsInstanceOf" => Some(Intrinsic::InstanceOf),
                    _ => None,
                }
            } else if lib == js_helper {
                match n {
                    "jsObjectGetPrototypeOf" => Some(Intrinsic::GetPrototypeOf),
                    "jsObjectSetPrototypeOf" => Some(Intrinsic::SetPrototypeOf),
                    "staticInteropGlobalContext" => Some(Intrinsic::GlobalContext),
                    "spread" => Some(Intrinsic::Spread),
                    _ => None,
                }
            } else if lib == internal {
                match n {
                    "unsafeCast" => Some(Intrinsic::UnsafeCast),
                    "extractTypeArguments" => Some(Intrinsic::ExtractTypeArguments),
                    _ => None,
                }
            } else {
                None
            };
            if let Some(k) = k {
                m.intrinsics.insert(fid, k);
            }
        }
        // Annotations, scanning the SDK units.
        let mut fn_by_node: HashMap<(u32, u32), FunctionElementId> = HashMap::new();
        for (i, f) in p.functions.iter().enumerate() {
            if !p.library(f.library).is_sdk {
                continue;
            }
            if let FunctionRef::Function { unit, function } = f.node {
                fn_by_node.insert((unit.0, function.0), FunctionElementId(i as u32));
            }
        }
        let mut var_by_node: HashMap<(u32, u32, usize), VariableId> = HashMap::new();
        for (i, v) in p.variables.iter().enumerate() {
            if !p.library(v.library).is_sdk {
                continue;
            }
            if let VariableRef::TopLevel { unit, decl, index } = v.node {
                var_by_node.insert((unit.0, decl.0, index), VariableId(i as u32));
            }
        }
        let mut class_by_decl: HashMap<(u32, u32), ClassId> = HashMap::new();
        for (i, c) in p.classes.iter().enumerate() {
            if let Some(d) = c.decl {
                if p.library(c.library).is_sdk {
                    class_by_decl.insert((d.unit.0, d.decl.0), ClassId(i as u32));
                }
            }
        }
        for (li, lib) in p.libraries.iter().enumerate() {
            if !lib.is_sdk {
                continue;
            }
            let lid = LibraryId(li as u32);
            for (ui, &uid) in lib.units.iter().enumerate() {
                let unit = p.unit(uid);
                if ui == 0 {
                    for d in &unit.unit.directives {
                        if let ast::DirectiveKind::Library { .. } = d.kind {
                            for a in &d.metadata {
                                if annotation_name(ctx, a) == "ReifyFunctionTypes" && annotation_bool(ctx, uid, a) == Some(false) {
                                    m.no_reify_libs.insert(lid);
                                }
                            }
                        }
                    }
                }
                let ast = &unit.ast;
                for (di, decl) in ast.decls.iter().enumerate() {
                    let did = di as u32;
                    match &decl.kind {
                        ast::DeclKind::Function(fid) => {
                            let Some(&fe) = fn_by_node.get(&(uid.0, fid.0)) else { continue };
                            m.function_annotations(ctx, uid, &decl.metadata, fe);
                            m.params(uid, ctx, ast.function(*fid));
                        }
                        ast::DeclKind::Variables(l) => {
                            for a in decl.metadata.iter() {
                                if annotation_name(ctx, a) == "JSExportName" {
                                    if let Some(n) = annotation_text(ctx, uid, a) {
                                        for idx in 0..l.variables.len() {
                                            if let Some(&v) = var_by_node.get(&(uid.0, did, idx)) {
                                                m.var_export_names.insert(v, n.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        ast::DeclKind::Class(cd) => {
                            // `@patch class X`: the members go to the original class.
                            let is_patch = decl.metadata.iter().any(|a| annotation_name(ctx, a) == "patch");
                            if is_patch {
                                if let Some(Element::Class(orig)) = lib.declared.get(&cd.name.sym).and_then(|b| b.getter) {
                                    if p.class(orig).decl.is_some_and(|d| (d.unit, d.decl) != (uid, ast::DeclId(did))) {
                                        m.patches.entry(orig).or_default().push((uid, ast::DeclId(did)));
                                    }
                                }
                            }
                            if let Some(&c) = class_by_decl.get(&(uid.0, did)) {
                                for a in decl.metadata.iter() {
                                    let n = annotation_name(ctx, a);
                                    if n == "Native" || n == "JsPeerInterface" {
                                        if let Some(t) = annotation_text(ctx, uid, a) {
                                            let tags: Vec<String> = t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && !s.starts_with('!')).collect();
                                            m.peers.insert(c, tags);
                                        }
                                    }
                                }
                            }
                            for &mid in cd.members.iter() {
                                m.member(ctx, uid, &fn_by_node, ast.member(mid));
                            }
                        }
                        ast::DeclKind::Mixin(md) => {
                            for &mid in md.members.iter() {
                                m.member(ctx, uid, &fn_by_node, ast.member(mid));
                            }
                        }
                        ast::DeclKind::Extension(ed) => {
                            for &mid in ed.members.iter() {
                                m.member(ctx, uid, &fn_by_node, ast.member(mid));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        m
    }

    fn member(&mut self, ctx: &Ctx, uid: UnitId, fn_by_node: &HashMap<(u32, u32), FunctionElementId>, mem: &ast::Member) {
        match &mem.kind {
            ast::MemberKind::Method(fid) => {
                if let Some(&fe) = fn_by_node.get(&(uid.0, fid.0)) {
                    self.function_annotations(ctx, uid, &mem.metadata, fe);
                }
                self.params(uid, ctx, ctx.program.unit(uid).ast.function(*fid));
            }
            ast::MemberKind::Constructor(k) => {
                for p in k.parameters.iter() {
                    self.param(uid, ctx, p);
                }
            }
            ast::MemberKind::Field(_) => {}
        }
    }

    fn function_annotations(&mut self, ctx: &Ctx, uid: UnitId, metadata: &[ast::Annotation], fe: FunctionElementId) {
        for a in metadata {
            match annotation_name(ctx, a) {
                "JSExportName" => {
                    if let Some(n) = annotation_text(ctx, uid, a) {
                        self.export_names.insert(fe, n);
                    }
                }
                "ReifyFunctionTypes" if annotation_bool(ctx, uid, a) == Some(false) => {
                    self.no_reify_fns.insert(fe);
                }
                "NoReifyGeneric" => {
                    self.no_reify_generic.insert(fe);
                }
                _ => {}
            }
        }
    }

    fn params(&mut self, uid: UnitId, ctx: &Ctx, f: &ast::Function) {
        for p in f.parameters.iter().flat_map(|ps| ps.iter()) {
            self.param(uid, ctx, p);
        }
    }

    fn param(&mut self, uid: UnitId, ctx: &Ctx, p: &ast::Parameter) {
        if p.metadata.iter().any(|a| annotation_name(ctx, a) == "nullCheck") {
            if let Some(n) = p.name {
                self.null_checked_params.insert((uid.0, n.span.start as u32));
            }
        }
    }

    /// Value of `JS_GET_FLAG('X')` (`compiler.dart:6147-6177`), with sound null
    /// safety and DDC as the compiler.
    pub fn flag(name: &str) -> Option<bool> {
        match name {
            "DEV_COMPILER" | "SOUND_NULL_SAFETY" | "VARIANCE" => Some(true),
            "MINIFIED" | "LEGACY" | "EXTRA_NULL_SAFETY_CHECKS" | "PRINT_LEGACY_STARS" => Some(false),
            _ => None,
        }
    }
}

// --------------------------------------------------------------------- queries

/// The members of a class together with those of its `@patch` declarations (in
/// SDK mode); outside it, the usual ones
/// ([`dartforge_elements::model::Program::class_members`]).
pub fn class_members(ctx: &Ctx, c: ClassId) -> Vec<(UnitId, ast::MemberId)> {
    let mut out = ctx.program.class_members(c);
    if let Some(ps) = ctx.sdk.as_ref().and_then(|s| s.patches.get(&c)) {
        for &(u, d) in ps {
            if let ast::DeclKind::Class(cd) = &ctx.program.unit(u).ast.decl(d).kind {
                out.extend(cd.members.iter().map(|&m| (u, m)));
            }
        }
    }
    out
}

/// The class is native in DDC's sense (`isNativeClass`): `int`, `double`,
/// `bool`, `String` and the `@Native` ones of the interceptors, typed data and
/// web libraries. SDK mode only.
pub fn is_native_class(ctx: &Ctx, c: ClassId) -> bool {
    ctx.sdk.is_some() && ctx.native_set.contains(&c)
}

/// The emitter is compiling code of an SDK library in SDK mode.
pub fn in_sdk(ctx: &Ctx, lib: LibraryId) -> bool {
    ctx.sdk.is_some() && ctx.libs[lib.0 as usize].is_sdk
}

/// Property name of a top-level (or static) function with `@JSExportName`:
/// `throw_` → `throw`.
pub fn export_name<'c>(ctx: &'c Ctx, fid: FunctionElementId) -> Option<&'c str> {
    ctx.sdk.as_ref()?.export_names.get(&fid).map(String::as_str)
}

/// Property name of a top-level variable with `@JSExportName`.
pub fn export_name_var<'c>(ctx: &'c Ctx, vid: VariableId) -> Option<&'c str> {
    ctx.sdk.as_ref()?.var_export_names.get(&vid).map(String::as_str)
}

/// Functions of this library/function get no `dart.fn` (`@ReifyFunctionTypes(false)`,
/// `compiler.dart:7306-7338`).
pub fn no_reify(ctx: &Ctx, lib: LibraryId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.no_reify_libs.contains(&lib))
}

pub fn function_without_reify(ctx: &Ctx, fid: FunctionElementId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.no_reify_fns.contains(&fid) || s.no_reify_libs.contains(&ctx.program.function(fid).library))
}

/// The function has `@NoReifyGeneric()`.
pub fn no_reify_generic_fn(ctx: &Ctx, fid: FunctionElementId) -> bool {
    ctx.sdk.as_ref().is_some_and(|s| s.no_reify_generic.contains(&fid))
}

/// The `assert` is not emitted: SDK mode and the unit belongs to the SDK.
pub fn omit_assert(ctx: &Ctx, lib: LibraryId) -> bool {
    // The only `assert` of `dart:_runtime` is the one in `assertInterop` (a Dart
    // function passed to JS without `allowInterop`), observable behavior that
    // DDC's `dart_sdk.js` keeps: it stays.
    (ctx.sdk.as_ref().is_some_and(|s| s.no_sdk_asserts && s.runtime != Some(lib)) && ctx.libs[lib.0 as usize].is_sdk)
        || ctx.filter.is_some_and(|f| !f.keep_asserts())
}

/// `@nullCheck` checks of the already declared parameters (`compiler.dart:3855`):
/// `if (p == null) dart.argumentError(p);`.
pub fn null_checks(e: &FnEmitter, params: &[ast::Parameter]) -> String {
    let Some(s) = e.ctx.sdk.as_ref() else { return String::new() };
    if s.null_checked_params.is_empty() || !e.ctx.libs[e.lib.0 as usize].is_sdk {
        return String::new();
    }
    let mut out = String::new();
    for p in params {
        let Some(n) = p.name else { continue };
        if !s.null_checked_params.contains(&(e.unit.0, n.span.start as u32)) {
            continue;
        }
        if let Some(l) = e.lookup_local(n.sym) {
            out.push_str(&format!("if ({0} == null) dart.argumentError({0});\n", l.js));
        }
    }
    out
}

/// Initializer that DDC folds at the use site (`kernel/constants.dart:105-120`,
/// `shouldInlineConstant`): a numeric, boolean or null literal, or a *string*
/// with fewer than 32 units. The same criterion is in `crates/world`
/// (`is_inlinable_constant`), which does not keep a variable read this way alive.
pub fn inlinable_literal(ast: &ast::Ast, x: ExprId) -> bool {
    match &ast.expr(x).kind {
        ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::Bool(_) | ExprKind::Null => true,
        ExprKind::String(lit) => lit.constant_value().is_some_and(|v| v.utf16_len() < 32),
        ExprKind::Unary { op: ast::UnaryOp::Neg, operand } => matches!(ast.expr(*operand).kind, ExprKind::Int(_) | ExprKind::Double(_)),
        ExprKind::Parenthesized(i) => inlinable_literal(ast, *i),
        _ => false,
    }
}

/// Reading a primitive constant becomes the value itself (SDK mode, which is the
/// production profile): `dart_rti.interfaceTypeRecipePropertyName` comes out as
/// `"$interfaceRecipe"`, and the variable no longer exists in the file.
pub fn inline_constant(e: &FnEmitter, vid: VariableId) -> Option<Js> {
    inline_constant_depth(e, vid, 0)
}

/// [`inline_constant`], following aliases (`static const String
/// separatorString = _commaString;`): the alias folds to the value of the
/// constant it names. The same rule is `dartforge_world::is_inlinable_constant`.
fn inline_constant_depth(e: &FnEmitter, vid: VariableId, depth: usize) -> Option<Js> {
    e.ctx.sdk.as_ref()?;
    let v = e.ctx.program.variable(vid);
    if !v.const_ || v.extension.is_some() || depth > 8 {
        return None;
    }
    let (unit, init) = match v.node {
        VariableRef::TopLevel { unit, decl, index } => match &e.ctx.program.unit(unit).ast.decl(decl).kind {
            ast::DeclKind::Variables(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        VariableRef::Field { unit, member, index } => match &e.ctx.program.unit(unit).ast.member(member).kind {
            ast::MemberKind::Field(l) => (unit, l.variables.get(index)?.initializer?),
            _ => return None,
        },
        _ => return None,
    };
    if !inlinable_literal(&e.ctx.program.unit(unit).ast, init) {
        let target = const_alias(e.ctx.program, vid, &e.ctx.program.unit(unit).ast, init)?;
        return inline_constant_depth(e, target, depth + 1);
    }
    let mut sub = FnEmitter::new(e.ctx, e.m, unit, None, true);
    let ty = e.ctx.var_ty(vid);
    let (js, _) = sub.emit_expr(init, Some(&ty));
    Some(js)
}

/// The constant variable that the initializer `x` of `v` names with a plain
/// identifier: a static of `v`'s class or a declaration of its library (the
/// same lookup as `dartforge_world::const_alias`).
fn const_alias(p: &dartforge_elements::model::Program, v: VariableId, a: &ast::Ast, x: ExprId) -> Option<VariableId> {
    let ExprKind::Identifier(n) = &a.expr(x).kind else { return None };
    let var = p.variable(v);
    let as_variable = |el: Element| match el {
        Element::Variable(t) => Some(t),
        Element::Function(f) => p.function(f).variable,
        _ => None,
    };
    if let Some(c) = var.class {
        if let Some(f) = p.class(c).static_members.get(&n.sym) {
            return p.function(*f).variable;
        }
    }
    let lib = p.library(var.library);
    lib.declared.get(&n.sym).or_else(|| lib.scope.get(&n.sym)).and_then(|b| b.getter).and_then(as_variable)
}

/// No covariance checks with `--omit-checks`.
fn checks_omitted(e: &FnEmitter) -> bool {
    e.ctx.filter.is_some_and(|f| f.omit_implicit_checks())
}

/// Covariance checks of the parameters of an instance method of a generic SDK
/// class (`compiler.dart:3848-3856`): a parameter whose type references a type
/// parameter of the class (or that is marked `covariant`) is checked on entry —
/// `List<num> l = <int>[]; l.add(2.5)` throws `TypeError` in `JSArray.add`.
pub fn covariance_checks(e: &FnEmitter, params: &[ast::Parameter]) -> String {
    let Some(c) = e.class else { return String::new() };
    if e.ctx.sdk.is_none() || e.is_static || !e.ctx.libs[e.lib.0 as usize].is_sdk || checks_omitted(e) {
        return String::new();
    }
    let class_param_ids: HashSet<u32> = e.ctx.class_params[c.0 as usize].iter().map(|p| p.id).collect();
    let mut out = String::new();
    for p in params {
        let Some(n) = p.name else { continue };
        let Some(l) = e.lookup_local(n.sym) else { continue };
        // Only a **covariant** occurrence of the type parameter calls for a check
        // (`void Function(T)` is contravariant in `T`: always safe).
        let mentions = occurs_covariantly(&l.ty, &class_param_ids, true);
        if !(mentions || p.covariant) || matches!(l.ty, Ty::Dynamic) {
            continue;
        }
        // `null` passes: the local type may have lost the `?` of a function
        // parameter (`E Function()? orElse`), and the typed call already proved
        // the nullability.
        out.push_str(&format!("if ({0} != null) {1}[_as]({0});\n", l.js, e.rti(&l.ty)));
    }
    out
}

/// A type parameter in `ids` occurs in a covariant position of `t` (`positive`
/// is the current polarity; a function parameter flips it).
fn occurs_covariantly(t: &Ty, ids: &HashSet<u32>, positive: bool) -> bool {
    match t {
        Ty::Param { id, .. } => positive && ids.contains(id),
        Ty::Iface { args, .. } => args.iter().any(|a| occurs_covariantly(a, ids, positive)),
        Ty::FutureOr { arg, .. } => occurs_covariantly(arg, ids, positive),
        Ty::Fn { ret, pos, opt, named, .. } => {
            occurs_covariantly(ret, ids, positive)
                || pos.iter().chain(opt.iter()).any(|p| occurs_covariantly(p, ids, !positive))
                || named.iter().any(|(_, p, _)| occurs_covariantly(p, ids, !positive))
        }
        Ty::Record { pos, named, .. } => pos.iter().any(|p| occurs_covariantly(p, ids, positive)) || named.iter().any(|(_, p)| occurs_covariantly(p, ids, positive)),
        _ => false,
    }
}

/// `@rest` on the last positional parameter (`compiler.dart:3722-3726`): the JS
/// parameter becomes `...name` — `gbind(f, @rest typeArgs)` receives all the
/// following arguments in an array.
pub fn with_rest(e: &FnEmitter, params: &[ast::Parameter], js: String) -> String {
    if e.ctx.sdk.is_none() || !e.ctx.libs[e.lib.0 as usize].is_sdk {
        return js;
    }
    let Some(last) = params.iter().rev().find(|p| p.kind != ast::ParameterKind::Named) else { return js };
    if !last.metadata.iter().any(|a| annotation_name(e.ctx, a) == "rest") {
        return js;
    }
    let Some(n) = last.name else { return js };
    let Some(l) = e.lookup_local(n.sym) else { return js };
    let name = l.js.clone();
    // The name is the last item of the positional parameter list.
    match js.rfind(name.as_str()) {
        Some(p) if js[p..].trim_end() == name => format!("{}...{}", &js[..p], &js[p..]),
        _ => js,
    }
}

// --------------------------------------------------------------------- intrinsics

/// The intrinsic that `fid` is, if it is one.
pub fn intrinsic_of(ctx: &Ctx, fid: FunctionElementId) -> Option<Intrinsic> {
    ctx.sdk.as_ref()?.intrinsics.get(&fid).copied()
}

/// Constant string value of an expression: a literal without interpolation or an
/// identifier of a top-level constant with a literal initializer (the
/// `JS_EMBEDDED_GLOBAL('', RTI_UNIVERSE)` of `rti.dart`).
fn constant_string(e: &FnEmitter, x: ExprId) -> Option<String> {
    match &e.expr(x).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        ExprKind::Identifier(id) => {
            if let crate::expr::IdentTarget::Element(Element::Variable(v)) = e.ident_target(id.sym, x) {
                return variable_string(e.ctx, v);
            }
            None
        }
        ExprKind::Property { target, name, .. } => {
            // `prefix.CONST`
            if let ExprKind::Identifier(id) = &e.expr(*target).kind {
                if let crate::expr::IdentTarget::Prefix(pf) = e.ident_target(id.sym, *target) {
                    let b = e.ctx.program.lookup_prefixed_in_unit(e.unit, pf, name.sym)?;
                    if let Some(Element::Variable(v)) = b.getter {
                        return variable_string(e.ctx, v);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn variable_string(ctx: &Ctx, v: VariableId) -> Option<String> {
    let var = ctx.program.variable(v);
    if !var.const_ {
        return None;
    }
    let VariableRef::TopLevel { unit, decl, index } = var.node else { return None };
    let ast = &ctx.program.unit(unit).ast;
    let ast::DeclKind::Variables(l) = &ast.decl(decl).kind else { return None };
    let init = l.variables.get(index)?.initializer?;
    match &ast.expr(init).kind {
        ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
        _ => None,
    }
}

/// Name of the referenced enum member (`JsGetName.RTI_NAME` → `RTI_NAME`).
fn enum_member_name(e: &FnEmitter, x: ExprId) -> Option<String> {
    match &e.expr(x).kind {
        ExprKind::Property { name, .. } => Some(e.name(name.sym).to_string()),
        ExprKind::Identifier(id) => Some(e.name(id.sym).to_string()),
        _ => None,
    }
}

/// The class of a type literal (`Foo` or `prefix.Foo`).
fn literal_class(e: &FnEmitter, x: ExprId) -> Option<ClassId> {
    match &e.expr(x).kind {
        ExprKind::Identifier(id) => match e.ident_target(id.sym, x) {
            crate::expr::IdentTarget::Element(Element::Class(c)) => Some(c),
            crate::expr::IdentTarget::Element(Element::Typedef(td)) => match e.ctx.ty_of(e.ctx.outline.typedefs[td.0 as usize].target_type) {
                Ty::Iface { class, .. } => Some(class),
                _ => None,
            },
            _ => None,
        },
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(id) = &e.expr(*target).kind else { return None };
            let crate::expr::IdentTarget::Prefix(pf) = e.ident_target(id.sym, *target) else { return None };
            let b = e.ctx.program.lookup_prefixed_in_unit(e.unit, pf, name.sym)?;
            match b.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        }
        ExprKind::TypeArguments { target, .. } => literal_class(e, *target),
        _ => None,
    }
}

/// Static type of a call to a generic intrinsic (`JS<T>`, `unsafeCast<T>`): the
/// explicit type argument, otherwise the context's (downward inference, as the
/// CFE does), otherwise `dynamic`.
fn generic_call_type(e: &FnEmitter, args: &ast::Arguments, expected: Option<&Ty>) -> Ty {
    if let Some(t) = args.type_args.first() {
        return e.resolve_type(*t);
    }
    match expected {
        Some(t) if !matches!(t, Ty::Void) => t.clone(),
        _ => Ty::Dynamic,
    }
}

/// Emits an argument of `JS()` (`_isInForeignJS`, `compiler.dart:6460-6463`).
fn foreign_arg(e: &mut FnEmitter, x: ExprId) -> Js {
    let saved = std::mem::replace(&mut e.in_foreign_js, true);
    let (js, _) = e.emit_expr(x, None);
    e.in_foreign_js = saved;
    js
}

/// A piece of the *template*: JS text or a hole.
enum Piece {
    Text(String),
    Hole(ExprId),
}

/// The pieces of the `JS()` *template*: a literal with `#` (holes for the
/// following arguments) or with Dart interpolations (each `$x` becomes a hole,
/// in order).
fn template_pieces(e: &FnEmitter, tpl: ExprId, rest: &[ExprId]) -> Option<Vec<Piece>> {
    let ExprKind::String(lit) = &e.expr(tpl).kind else { return None };
    let mut text = String::new();
    let mut interp: Vec<ExprId> = Vec::new();
    for part in lit.parts.iter() {
        match part {
            StringPart::Text(t) => text.push_str(&t.to_string_lossy()),
            StringPart::Interpolation(x) => {
                // Interpolation of a string constant: DDC folds the constant and
                // the value goes in as JS text (`compiler.dart:6435`).
                if let Some(s) = constant_string(e, *x) {
                    text.push_str(&s);
                } else {
                    text.push('#');
                    interp.push(*x);
                }
            }
        }
    }
    let holes: Vec<ExprId> = if interp.is_empty() { rest.to_vec() } else { interp };
    let mut out = Vec::new();
    let mut i = 0usize;
    let b = text.as_bytes();
    let mut start = 0usize;
    let mut n = 0usize;
    while i < b.len() {
        match b[i] {
            b'\'' | b'"' | b'`' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'#' => {
                out.push(Piece::Text(text[start..i].to_string()));
                let x = *holes.get(n)?;
                n += 1;
                out.push(Piece::Hole(x));
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    out.push(Piece::Text(text[start.min(text.len())..].to_string()));
    Some(out)
}

fn is_plain_js_ident(s: &str) -> bool {
    js::is_js_ident(s)
}

/// Builds the text of `JS(type, 'template', a…)`. `None` if the call does not
/// have the expected shape (then it is emitted as a plain call, and the error
/// shows up).
pub fn js_text(e: &mut FnEmitter, args: &ast::Arguments) -> Option<String> {
    let list: Vec<ExprId> = args.args.iter().map(|a| a.value).collect();
    if list.len() < 2 {
        return None;
    }
    let pieces = template_pieces(e, list[1], &list[2..])?;
    // The arguments are emitted in the order they appear (effects and temps).
    let mut emitted: HashMap<u32, (Js, Option<String>)> = HashMap::new();
    for p in &pieces {
        if let Piece::Hole(x) = p {
            // A string literal that is an identifier: it may become a property
            // name (`#.#` with `'foo'` gives `.foo`, `compiler.dart` via the
            // `js_ast` printer).
            let lit = match &e.expr(*x).kind {
                ExprKind::String(l) => l.constant_value().map(|v| v.to_string_lossy()),
                _ => None,
            };
            let js = foreign_arg(e, *x);
            emitted.insert(x.0, (js, lit));
        }
    }
    let mut out = String::new();
    for p in &pieces {
        match p {
            Piece::Text(t) => out.push_str(t),
            Piece::Hole(x) => {
                let (js, lit) = &emitted[&x.0];
                let before = out.trim_end();
                // Only a dot on the same line: one ending a comment does not count.
                let trimmed = out.trim_end_matches([' ', '\t']);
                let after_dot = trimmed.ends_with('.') && !trimmed.ends_with("..");
                if after_dot {
                    match lit {
                        Some(s) if is_plain_js_ident(s) => out.push_str(s),
                        _ => {
                            // `a.#` with an expression: `a[expr]`.
                            let cut = trimmed.len() - 1;
                            out.truncate(cut);
                            out.push('[');
                            out.push_str(&js.code);
                            out.push(']');
                        }
                    }
                    continue;
                }
                let is_key = (before.ends_with('{') || before.ends_with(',')) && next_non_blank_is(&pieces, x, ':');
                if is_key {
                    match lit {
                        Some(s) if is_plain_js_ident(s) => out.push_str(s),
                        _ => {
                            out.push('[');
                            out.push_str(&js.code);
                            out.push(']');
                        }
                    }
                    continue;
                }
                let after_new = before.ends_with("new") && before.len() >= 3 && !before[..before.len() - 3].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$');
                if after_new {
                    out.push('(');
                    out.push_str(&js.code);
                    out.push(')');
                } else {
                    out.push_str(&js.at(P_PRIMARY));
                }
            }
        }
    }
    Some(out)
}

/// The text following hole `x` starts with `c` (after whitespace).
fn next_non_blank_is(pieces: &[Piece], x: &ExprId, c: char) -> bool {
    let mut found = false;
    for p in pieces {
        match p {
            Piece::Hole(y) if y.0 == x.0 => found = true,
            Piece::Text(t) if found => return t.trim_start().starts_with(c),
            Piece::Hole(_) if found => return false,
            _ => {}
        }
    }
    false
}

/// `JS()` whose *template* is a statement (`throw …`, `compiler.dart` via
/// `parseForeignJS`): the text, to be written in statement position.
pub fn js_statement(e: &mut FnEmitter, x: ExprId) -> Option<String> {
    e.ctx.sdk.as_ref()?;
    let ExprKind::Call { target, arguments } = &e.expr(x).kind else { return None };
    let fid = called_function(e, *target)?;
    if intrinsic_of(e.ctx, fid) != Some(Intrinsic::Js) {
        return None;
    }
    let tpl = arguments.args.get(1)?.value;
    let ExprKind::String(lit) = &e.expr(tpl).kind else { return None };
    let starts = match lit.parts.first() {
        Some(StringPart::Text(t)) => t.to_string_lossy().trim_start().starts_with("throw "),
        _ => false,
    };
    if !starts {
        return None;
    }
    js_text(e, arguments)
}

/// The top-level function called by `callee(...)` (`f` or `prefix.f`).
pub(crate) fn called_function(e: &FnEmitter, callee: ExprId) -> Option<FunctionElementId> {
    match &e.expr(callee).kind {
        ExprKind::Identifier(id) => match e.ident_target(id.sym, callee) {
            crate::expr::IdentTarget::Element(Element::Function(f)) => Some(f),
            _ => None,
        },
        ExprKind::Property { target, name, .. } => {
            let ExprKind::Identifier(id) = &e.expr(*target).kind else { return None };
            let crate::expr::IdentTarget::Prefix(pf) = e.ident_target(id.sym, *target) else { return None };
            match e.ctx.program.lookup_prefixed_in_unit(e.unit, pf, name.sym)?.getter {
                Some(Element::Function(f)) => Some(f),
                _ => None,
            }
        }
        ExprKind::TypeArguments { target, .. } => called_function(e, *target),
        _ => None,
    }
}

/// Emits the call to an intrinsic. `None`: it is not an intrinsic (or the shape
/// does not match) — the caller emits the plain call.
pub fn emit_intrinsic(e: &mut FnEmitter, fid: FunctionElementId, args: &ast::Arguments, expected: Option<&Ty>) -> Option<(Js, Ty)> {
    let k = intrinsic_of(e.ctx, fid)?;
    let pos: Vec<ExprId> = args.args.iter().filter(|a| a.name.is_none()).map(|a| a.value).collect();
    let declared_ret = || match e.ctx.fn_ty(fid) {
        Ty::Fn { ret, .. } if !ret.mentions_params() => (*ret).clone(),
        _ => Ty::Dynamic,
    };
    match k {
        Intrinsic::Js => {
            let ty = generic_call_type(e, args, expected);
            // `Function.apply` (`core_patch.dart:96-101`) builds the object of
            // named arguments with `JS('', '#[#] = #', map, _symbolToString(symbol), arg)`:
            // with the short keys of named parameters
            // (`emit_js_production/src/nomeados.rs`), the source name is translated
            // at run time by `dart.nomeadoJS`.
            if e.class.is_some() && e.class == e.ctx.function_ && e.ctx.filter.is_some_and(|f| f.named_table().is_some()) {
                let tpl = pos.get(1).and_then(|t| constant_string(e, *t));
                if tpl.as_deref() == Some("#[#] = #") && pos.len() == 5 {
                    let a = foreign_arg(e, pos[2]).code;
                    let k = foreign_arg(e, pos[3]).code;
                    let v = foreign_arg(e, pos[4]).code;
                    return Some((Js::prim(format!("({a}[dart.nomeadoJS({k})] = {v})")), ty));
                }
            }
            let statement_template = pos.get(1).is_some_and(|t| match &e.expr(*t).kind {
                ExprKind::String(lit) => matches!(lit.parts.first(), Some(StringPart::Text(x)) if x.to_string_lossy().trim_start().starts_with("throw ")),
                _ => false,
            });
            let text = js_text(e, args)?;
            if statement_template {
                // A statement in expression position: inside an arrow function.
                return Some((Js::prim(format!("(() => {{ {text}; }})()")), ty));
            }
            // In parentheses: the *template* is free-form JS, and not every place
            // in the emitter checks precedence (`!JS('', '# || #', a, b)`).
            let simple = text.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.'));
            Some((if simple { Js::prim(text) } else { Js::prim(format!("({text})")) }, ty))
        }
        Intrinsic::GetFlag => {
            let name = constant_string(e, *pos.first()?)?;
            let v = SdkMode::flag(&name)?;
            Some((Js::prim(if v { "true" } else { "false" }), e.ctx.t_bool()))
        }
        Intrinsic::GetName => {
            let n = enum_member_name(e, *pos.first()?)?;
            let js = match n.as_str() {
                "OPERATOR_IS_PREFIX" => "\"$is_\"".to_string(),
                "SIGNATURE_NAME" => "dart._functionRti".to_string(),
                "RTI_NAME" => "\"$ti\"".to_string(),
                "FUTURE_CLASS_TYPE_NAME" => js::string_literal(&e.ctx.recipe_of("dart:async", "Future", "async|Future")),
                "LIST_CLASS_TYPE_NAME" => js::string_literal(&e.ctx.recipe_of("dart:core", "List", "core|List")),
                "RTI_FIELD_AS" | "RTI_FIELD_IS" => {
                    let rti = e.ctx.sdk.as_ref()?.rti?;
                    e.private_sym(rti, if n == "RTI_FIELD_AS" { "_as" } else { "_is" })
                }
                _ => return None,
            };
            Some((Js::prim(js), declared_ret()))
        }
        Intrinsic::EmbeddedGlobal => {
            let name = constant_string(e, *pos.get(1)?)?;
            let js = if name == "arrayRti" { e.dartx("arrayRti") } else { format!("dart{}", js::prop_access(&name)) };
            Some((Js::prim(js), generic_call_type(e, args, expected)))
        }
        Intrinsic::ClassRef => {
            let x = *pos.first()?;
            if let ExprKind::Identifier(id) = &e.expr(x).kind {
                if e.name(id.sym) == "Null" {
                    return Some((Js::prim("core.Null"), declared_ret()));
                }
            }
            let c = literal_class(e, x)?;
            Some((Js::prim(e.class_ref(c)), declared_ret()))
        }
        Intrinsic::TypeRef | Intrinsic::LegacyTypeRef => {
            let t = e.resolve_type(*args.type_args.first()?);
            let mut rti = e.rti(&t);
            if k == Intrinsic::LegacyTypeRef {
                // `T*`: the closed recipe gets the star.
                let pre = "dart_rti._Universe.eval(dart_rti._theUniverse(), \"";
                if let Some(r) = rti.strip_prefix(pre).and_then(|r| r.strip_suffix("\", true)")) {
                    rti = format!("{pre}{r}*\", true)");
                }
            }
            Some((Js::prim(rti), declared_ret()))
        }
        Intrinsic::RawFunctionRef => {
            let js = foreign_arg(e, *pos.first()?);
            Some((js, declared_ret()))
        }
        Intrinsic::StringConcat => {
            let a = foreign_arg(e, *pos.first()?);
            let b = foreign_arg(e, *pos.get(1)?);
            Some((Js::new(format!("{} + {}", a.at(P_ADD), b.at(P_ADD + 1)), P_ADD), e.ctx.t_string()))
        }
        Intrinsic::Builtin => {
            let n = enum_member_name(e, *pos.get(1)?)?;
            let js = match n.as_str() {
                "dartClosureConstructor" => "Function",
                "dartObjectConstructor" => "core.Object",
                _ => return None,
            };
            Some((Js::prim(js), generic_call_type(e, args, expected)))
        }
        Intrinsic::RawException => {
            let v = e.rethrow_var.last().cloned()?;
            Some((Js::prim(v), Ty::Dynamic))
        }
        Intrinsic::RtiParameter => Some((Js::prim("_ti"), Ty::Dynamic)),
        Intrinsic::RuntimeLibrary => Some((Js::prim("dart"), Ty::Dynamic)),
        Intrinsic::GetInterceptor => {
            let a = foreign_arg(e, *pos.first()?);
            Some((Js::prim(format!("dart.getInterceptorForRti({})", a.code)), Ty::Dynamic))
        }
        Intrinsic::ExtensionSymbol => {
            let x = *pos.first()?;
            let ExprKind::String(lit) = &e.expr(x).kind else { return None };
            let n = lit.constant_value()?.to_string_lossy();
            Some((Js::prim(e.dartx(&js_member_name(&n))), Ty::Dynamic))
        }
        Intrinsic::InstanceOf => {
            let c = literal_class(e, *pos.get(1)?)?;
            if !e.ctx.class_params[c.0 as usize].is_empty() {
                return None;
            }
            let a = foreign_arg(e, *pos.first()?);
            Some((Js::new(format!("{} instanceof {}", a.at(P_REL), e.class_ref(c)), P_REL), e.ctx.t_bool()))
        }
        Intrinsic::GetPrototypeOf => {
            let a = foreign_arg(e, *pos.first()?);
            Some((Js::prim(format!("Object.getPrototypeOf({})", a.code)), Ty::Dynamic))
        }
        Intrinsic::SetPrototypeOf => {
            let a = foreign_arg(e, *pos.first()?);
            let b = foreign_arg(e, *pos.get(1)?);
            Some((Js::prim(format!("Object.setPrototypeOf({}, {})", a.code, b.code)), Ty::Dynamic))
        }
        Intrinsic::GlobalContext => Some((Js::prim("dart.global"), Ty::Dynamic)),
        Intrinsic::UnsafeCast => {
            let ty = generic_call_type(e, args, expected);
            let (js, _) = e.emit_expr(*pos.first()?, None);
            Some((js, ty))
        }
        Intrinsic::ExtractTypeArguments => {
            let t = e.resolve_type(*args.type_args.first()?);
            let Ty::Iface { class, .. } = t else { return None };
            let cname = e.ctx.class_name(class).to_string();
            let params: Vec<String> = e.ctx.class_params[class.0 as usize].iter().map(|p| p.name.clone()).collect();
            let (inst, _) = e.emit_expr(*pos.first()?, None);
            let (f, _) = e.emit_expr(*pos.get(1)?, None);
            let tmp = e.temp();
            let extracted: Vec<String> = params.iter().map(|p| format!("dart_rti.evalInInstance({tmp}, {})", js::string_literal(&format!("{cname}.{p}")))).collect();
            Some((Js::prim(format!("({tmp} = {}, dart.dgcall({}, [{}], []))", inst.code, f.code, extracted.join(", "))), Ty::Dynamic))
        }
        Intrinsic::Spread => {
            let a = foreign_arg(e, *pos.first()?);
            Some((Js::prim(format!("...{}", a.at(P_PRIMARY))), Ty::Dynamic))
        }
    }
}

// --------------------------------------------------------------------- native classes

/// Declaration key of a public instance member of a native class: the `dartx`
/// symbol (`compiler.dart:2766-2773`).
pub fn native_key(m: &ModState, name: &str) -> String {
    format!("[{}]", m.dartx(&js_member_name(name)))
}

/// Body of an `external` (or `native`) instance member of a native class
/// (`_emitNativeFunctionBody`, `compiler.dart:2322-2343`): the JS property with
/// the same name (or the `@JSName` one). In a web library, a non-nullable return
/// goes through `dart.checkNativeNonNull`.
pub fn native_external_member(ctx: &Ctx, head: &str, fid: FunctionElementId, dart_name: &str, kind: ast::FunctionKind) -> String {
    let f = ctx.program.function(fid);
    let jsn = ctx.js_names.get(&fid.0).cloned().unwrap_or_else(|| dart_name.to_string());
    let access = format!("this{}", js::prop_access(&jsn));
    let web = ctx.is_web_library(f.library);
    let ret = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
    let check = |x: String| if web && !ret.is_nullable() && !matches!(ret, Ty::Void | Ty::Dynamic) { format!("dart.checkNativeNonNull({x})") } else { x };
    match kind {
        ast::FunctionKind::Getter => format!("{head}() {{\n  return {};\n}}", check(access)),
        ast::FunctionKind::Setter => format!("{head}(v) {{\n  {access} = v;\n}}"),
        _ => format!("{head}(...args) {{\n  return {};\n}}", check(format!("{access}.apply(this, args)"))),
    }
}

/// Lines that close the declaration of an SDK class (`compiler.dart:1108-1117`):
/// `Object` installs identity equality; each native *peer* is registered.
pub fn class_epilogue(ctx: &Ctx, c: ClassId, cref: &str) -> String {
    let Some(s) = ctx.sdk.as_ref() else { return String::new() };
    let mut out = String::new();
    if Some(c) == ctx.object {
        out.push_str("dart._installIdentityEquals();\n");
        return out;
    }
    if let Some(tags) = s.peers.get(&c) {
        if !tags.is_empty() && s.primitives.contains(&c) {
            out.push_str(&format!("dart.definePrimitiveHashCode({cref}.prototype);\n"));
        }
        for t in tags {
            out.push_str(&format!("dart.registerExtension({}, {cref});\n", js::string_literal(t)));
        }
    }
    out
}

/// Extra body of special classes (`compiler.dart:2163-2183`).
pub fn special_constructor(ctx: &Ctx, c: ClassId) -> Option<&'static str> {
    ctx.sdk.as_ref()?;
    if Some(c) == ctx.object {
        return Some("constructor() {\n  throw Error(\"use `new \" + dart.typeName(dart.getReifiedType(this)) + \".new(...)` to create a Dart object\");\n}");
    }
    if Some(c) == ctx.js_array {
        return Some("constructor() {\n  return [];\n}");
    }
    None
}

// --------------------------------------------------------------------- the SDK module

/// SDK libraries with source, in program order.
pub fn sdk_libraries(ctx: &Ctx) -> Vec<LibraryId> {
    (0..ctx.program.libraries.len() as u32)
        .map(LibraryId)
        .filter(|l| ctx.libs[l.0 as usize].is_sdk && !ctx.program.library(*l).units.is_empty())
        .collect()
}

/// The *bootstrap* (`compiler.dart:7934-7989`): the `dart` object, one namespace
/// per library, `dartx`, `dart.privateName` and the `dartx` symbols used by any
/// module of the file — **one** `Symbol()` per name.
pub fn bootstrap(ctx: &Ctx, dartx: &BTreeSet<String>, late_fields: bool) -> String {
    let mut out = String::new();
    out.push_str("const _library = Object.create(null);\nconst dart = Object.create(_library);\ndart.library = _library;\n");
    for l in sdk_libraries(ctx) {
        let v = &ctx.libs[l.0 as usize].js_var;
        if v != "dart" {
            out.push_str(&format!("var {v} = Object.create(dart.library);\n"));
        }
    }
    out.push_str("var dartx = Object.create(dart.library);\n");
    out.push_str(
        "const _privateNames = Symbol(\"_privateNames\");\ndart.privateName = function(library, name) {\n  let names = library[_privateNames];\n  if (names == null) names = library[_privateNames] = new Map();\n  let symbol = names.get(name);\n  if (symbol == null) names.set(name, symbol = Symbol(name));\n  return symbol;\n};\n",
    );
    if let Some(t) = ctx.filter.and_then(|f| f.named_table()) {
        out.push_str(&format!(
            "dart.nomeadoJS = (function () {{\n  const t = JSON.parse({});\n  return n => t[n] !== void 0 ? t[n] : n;\n}})();\n",
            js::string_literal(&t)
        ));
    }
    if late_fields {
        // `late` without an initializer (`module.rs`, `late_fields`): the same
        // `get`/`set` pair the class would write, defined once per field.
        out.push_str("dart.lateFields = function(p, fin, outros) {
  for (const [o, f] of [[fin, true], [outros, false]]) {
    for (const k of Reflect.ownKeys(o)) {
      const n = o[k] || '', s = Symbol(n);
      Object.defineProperty(p, k, {
        get() { let t = this[s]; return t == null ? dart.throw(new _internal.LateError.fieldNI(n)) : t; },
        set: f ? function(v) { if (this[s] != null) dart.throw(new _internal.LateError.fieldAI(n)); this[s] = v; } : function(v) { this[s] = v; },
        configurable: true
      });
    }
  }
};
");
    }
    // One loop instead of a `dartx.n = Symbol("dartx.n");` line per name
    // (65 in the smallest program). The names stay *strings* of the runtime,
    // which the property renaming keeps (`minify::IdentifierStrings`).
    let names: Vec<String> = dartx.into_iter().map(|n| js::string_literal(n)).collect();
    if !names.is_empty() {
        out.push_str(&format!("for (const n of [{}]) dartx[n] = Symbol(\"dartx.\" + n);\n", names.join(", ")));
    }
    out
}

/// A runtime top-level field emitted in place (`compiler.dart:2635-2681`): a null
/// initializer, a literal, `JS()` or the construction of a runtime class.
fn eager_field(e: &FnEmitter, init: Option<ExprId>) -> bool {
    let Some(x) = init else { return true };
    match &e.expr(x).kind {
        ExprKind::Null | ExprKind::Bool(_) | ExprKind::Int(_) | ExprKind::Double(_) | ExprKind::String(_) => true,
        ExprKind::Call { target, .. } => called_function(e, *target).is_some_and(|f| intrinsic_of(e.ctx, f) == Some(Intrinsic::Js)),
        _ => false,
    }
}

/// The SDK module: everything the filter leaves live of the `dart:` libraries, in
/// the order loading requires (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §5.6): the
/// runtime's functions and eager fields → classes (superclass first) → the rest
/// of each library → rti rules. Returns the text and the `dartx` names it uses
/// (for the *bootstrap*).
pub fn emit_sdk_module(ctx: &Ctx) -> (String, BTreeSet<String>) {
    let libs = sdk_libraries(ctx);
    let Some(&first) = libs.first() else { return (String::new(), BTreeSet::new()) };
    let mut m = ModState::new(first);
    m.group = libs.clone();
    let runtime = ctx.sdk.as_ref().and_then(|s| s.runtime);
    let mut body = crate::js::Writer::default();

    // 1. The runtime: the type universe, the functions and the eager fields.
    let mut runtime_lazy_vars: Vec<VariableId> = Vec::new();
    if let Some(rt) = runtime {
        body.line("dart.typeUniverse = {eC: new Map(), tR: {}, eT: {}, tPV: {}, sEA: []};");
        let mut accessors: Vec<(String, String)> = Vec::new();
        for (i, f) in ctx.program.functions.iter().enumerate() {
            let fid = FunctionElementId(i as u32);
            if f.library != rt || f.class.is_some() || f.extension.is_some() || f.kind == FunctionKind::ImplicitAccessor {
                continue;
            }
            if ctx.fn_state(fid) == crate::filter::State::Dead || external_without_body(ctx, fid) {
                continue;
            }
            crate::module::emit_top_function(ctx, &m, fid, &mut body, &mut accessors);
        }
        if !accessors.is_empty() {
            body.line("dart.copyProperties(dart, {");
            let texts: Vec<String> = accessors.iter().map(|(_, t)| crate::module::indent(t)).collect();
            body.push_raw(&texts.join(",\n"));
            body.push_raw("\n});\n");
        }
        for (i, v) in ctx.program.variables.iter().enumerate() {
            let vid = VariableId(i as u32);
            if v.library != rt || v.class.is_some() || v.extension.is_some() || ctx.var_state(vid) == crate::filter::State::Dead {
                continue;
            }
            let VariableRef::TopLevel { unit, decl, index } = v.node else { continue };
            let d = ctx.program.unit(unit).ast.decl(decl);
            let ast::DeclKind::Variables(list) = &d.kind else { continue };
            let var = &list.variables[index];
            let mut e = FnEmitter::new(ctx, &m, unit, None, true);
            if v.late || !eager_field(&e, var.initializer) {
                runtime_lazy_vars.push(vid);
                continue;
            }
            let name = export_name_var(ctx, vid).map(str::to_string).unwrap_or_else(|| ctx.name(v.name).to_string());
            let ty = ctx.var_ty(vid);
            let value = match var.initializer {
                Some(x) => {
                    let (js, _) = e.emit_expr(x, Some(&ty));
                    let pre = crate::module::finish_body(&mut e);
                    body.push_raw(&pre);
                    js.code
                }
                None => "null".to_string(),
            };
            body.line(&format!("dart{} = {value};", js::prop_access(&name)));
        }
    }

    // 1b. The runtime's lazy fields.
    if let Some(rt) = runtime {
        if !runtime_lazy_vars.is_empty() {
            crate::module::emit_top_variables(ctx, &m, rt, &runtime_lazy_vars, &mut body);
        }
    }

    // 2. Top-level functions and variables of all libraries (definitions only:
    // the variables are lazy): the definition of a class runs code
    // (`addRtiResources`, `registerExtension`) that may read them.
    for &l in &libs {
        if Some(l) != runtime {
            crate::module::emit_library_rest_parts(ctx, &m, l, &mut body, true, true);
        }
    }

    // 3. Classes of all libraries, superclass first.
    let mut classes: Vec<ClassId> = Vec::new();
    for (i, c) in ctx.program.classes.iter().enumerate() {
        let cid = ClassId(i as u32);
        if libs.contains(&c.library) && c.decl.is_some() && ctx.class_level(cid) != crate::filter::Level::Dead {
            classes.push(cid);
        }
    }
    // `core.Object` first: all the others extend it, including the runtime's.
    let ordered = crate::module::order_classes(ctx, &classes);
    let trace = std::env::var("DARTFORGE_SDK_TRACE").is_ok();
    for c in ordered {
        if trace {
            eprintln!("[sdk] class {}::{}", ctx.program.library(ctx.program.class(c).library).uri, ctx.class_name(c));
        }
        m.note_class(c);
        crate::module::emit_class(ctx, &m, c, &mut body);
    }

    // 5. rti rules of the SDK classes.
    let rules = crate::module::emit_rules_with(ctx, &m, true);
    body.push_raw(&rules);

    // Module prelude: `dartx` aliases, private names, the rti and async symbols
    // the emitter references unqualified, constant caches.
    let mut out = String::new();
    let dartx_names: BTreeSet<String> = m.dartx_used.borrow().values().cloned().collect();
    for (var, name) in m.dartx_used.borrow().iter() {
        if js::is_js_ident(name) {
            out.push_str(&format!("var {var} = dartx.{name};\n"));
        } else {
            out.push_str(&format!("var {var} = dartx[{}];\n", js::string_literal(name)));
        }
    }
    out.push_str("var _is = dart.privateName(dart_rti, \"_is\");\nvar _as = dart.privateName(dart_rti, \"_as\");\nvar _eval = dart.privateName(dart_rti, \"_eval\");\nvar _bind = dart.privateName(dart_rti, \"_bind\");\n");
    out.push_str("var _current = dart.privateName(async, \"_current\");\nvar _datum = dart.privateName(async, \"_datum\");\nvar _yieldStar = dart.privateName(async, \"_yieldStar\");\n");
    for (var, (l, name)) in m.private_syms.borrow().iter() {
        let lv = &ctx.libs[*l as usize].js_var;
        out.push_str(&format!("var {var} = dart.privateName({lv}, {});\n", js::string_literal(name)));
    }
    crate::module::emit_const_caches(ctx, &m, &libs, &mut out);
    crate::typed_read_guards::emit(&m, &mut out);
    out.push_str(&body.out);
    let object_recipe = ctx.recipe_of("dart:core", "Object", "core|Object");
    out.push_str(&format!("dart_rti.findType(\"{object_recipe}*\");\ndart_rti.findType(\"{object_recipe}?\");\ndart_rti.findType(\"0&*\");\n"));
    (out, dartx_names)
}

/// `external` with neither a body nor a *patch*: never emitted (`compiler.dart:3096-3099`).
pub fn external_without_body(ctx: &Ctx, fid: FunctionElementId) -> bool {
    let f = ctx.program.function(fid);
    if !f.external {
        return false;
    }
    match f.node {
        FunctionRef::Function { unit, function } => matches!(ctx.program.unit(unit).ast.function(function).body, ast::FunctionBody::Empty),
        _ => true,
    }
}

/// Certify only the positive-result comparison in the SDK's Euclidean modulo.
///
/// The canonical Number peer, operator identity and complete body shape must
/// match. The emitted typeof guard retains symbol dispatch for foreign BigInt
/// borrowing; the divisor comparison is deliberately left unchanged.
pub(crate) fn modulo_number_comparison(e: &mut FnEmitter, fid: FunctionElementId) -> Option<ExprId> {
    use ast::{BinaryOp, FunctionBody, StmtKind};
    if !e.ctx.production() || std::env::var("DARTFORGE_JSPROD_NUMBER_MODULO").is_ok_and(|v| v == "0") { return None; }
    let ctx = e.ctx;
    let sdk = ctx.sdk.as_ref()?;
    let c = ctx.jsnumber?;
    let class = ctx.program.class(c);
    let f = ctx.program.function(fid);
    if f.class != Some(c) || f.static_ || e.ctx.name(f.name) != "%" || f.kind != FunctionKind::Operator
        || !class.modifiers.final_ || e.ctx.program.library(class.library).uri != "dart:_interceptors"
        || !sdk.primitives.contains(&c) || !sdk.peers.get(&c).is_some_and(|p| p.len() == 1 && p[0] == "Number") { return None; }
    let FunctionRef::Function { unit, function } = f.node else { return None };
    if unit != e.unit { return None; }
    let af = e.ast().function(function);
    let [param] = af.parameters.as_deref()? else { return None };
    let other = param.name?;
    if param.kind != ast::ParameterKind::Required || !e.ctx.is_num_like(&e.lookup_local(other.sym)?.ty)
        || !sdk.null_checked_params.contains(&(unit.0, other.span.start as u32)) || af.modifier != ast::AsyncModifier::None { return None; }
    let FunctionBody::Block(body) = af.body else { return None };
    let StmtKind::Block(stmts) = &e.ast().stmt(body).kind else { return None };
    let [declaration, zero, positive, negative] = stmts.as_ref() else { return None };
    let StmtKind::Variables(list) = &e.ast().stmt(*declaration).kind else { return None };
    let [result] = list.variables.as_ref() else { return None };
    if list.late || list.external { return None; }
    let result_ty = e.resolve_type(list.ty?);
    if result_ty.class() != Some(c) || result_ty.is_nullable() || e.ret_ty.class() != Some(c) { return None; }
    let init = modulo_js_arguments(e, result.initializer?, "# % #")?;
    if init.len() != 2 || !matches!(e.expr(init[0]).kind, ExprKind::This) || !modulo_identifier(e, init[1], other.sym) { return None; }
    let StmtKind::If { condition: z, then: zret, else_: None, case_pattern: None, guard: None } = e.ast().stmt(*zero).kind else { return None };
    if !modulo_binary_zero(e, z, BinaryOp::Eq, result.name.sym) || !modulo_js_arguments(e, modulo_return(e, zret)?, "0")?.is_empty() { return None; }
    let StmtKind::If { condition, then, else_: None, case_pattern: None, guard: None } = e.ast().stmt(*positive).kind else { return None };
    if !modulo_binary_zero(e, condition, BinaryOp::Gt, result.name.sym) || !modulo_identifier(e, modulo_return(e, then)?, result.name.sym) { return None; }
    let StmtKind::If { condition: n, then: sub, else_: Some(add), case_pattern: None, guard: None } = e.ast().stmt(*negative).kind else { return None };
    let ExprKind::Binary { op: BinaryOp::Lt, left, right } = e.expr(n).kind else { return None };
    if !modulo_zero(e, right) { return None; }
    let args = modulo_js_arguments(e, left, "#")?;
    if args.len() != 1 || !modulo_identifier(e, args[0], other.sym) { return None; }
    for (st, template) in [(sub, "# - #"), (add, "# + #")] {
        let args = modulo_js_arguments(e, modulo_return(e, st)?, template)?;
        if args.len() != 2 || !modulo_identifier(e, args[0], result.name.sym) || !modulo_identifier(e, args[1], other.sym) { return None; }
    }
    // Also certify the dispatched comparison being replaced. SDK changes or a
    // patched/custom comparator must not silently lose behavior.
    let gt = *class.instance_members.get(&e.ctx.sym(">")?)?;
    let gf = e.ctx.program.function(gt);
    let FunctionRef::Function { unit: gu, function: gn } = gf.node else { return None };
    if gu != unit || gf.class != Some(c) || gf.patched_by.is_some() || gf.static_ || gf.kind != FunctionKind::Operator { return None; }
    let Ty::Fn { pos, opt, named, ret, .. } = ctx.fn_ty(gt) else { return None };
    if pos.len() != 1 || !ctx.is_num_like(&pos[0]) || !opt.is_empty() || !named.is_empty() || !ctx.is_bool(&ret) { return None; }
    let ga = e.ast().function(gn);
    let [gp] = ga.parameters.as_deref()? else { return None };
    let gname = gp.name?;
    if !sdk.null_checked_params.contains(&(gu.0, gname.span.start as u32)) { return None; }
    let FunctionBody::Block(gbody) = ga.body else { return None };
    let args = modulo_js_arguments(e, modulo_return(e, gbody)?, "# > #")?;
    if args.len() != 2 || !matches!(e.expr(args[0]).kind, ExprKind::This) || !modulo_identifier(e, args[1], gname.sym) { return None; }
    Some(condition)
}

fn modulo_identifier(e: &FnEmitter, x: ExprId, name: dartforge_intern::SymbolId) -> bool {
    matches!(e.expr(x).kind, ExprKind::Identifier(n) if n.sym == name)
}

fn modulo_zero(e: &FnEmitter, x: ExprId) -> bool {
    matches!(e.expr(x).kind, ExprKind::Int(s) if e.text(s) == "0")
}

fn modulo_binary_zero(e: &FnEmitter, x: ExprId, op: ast::BinaryOp, name: dartforge_intern::SymbolId) -> bool {
    matches!(e.expr(x).kind, ExprKind::Binary { op: o, left, right } if o == op && modulo_identifier(e, left, name) && modulo_zero(e, right))
}

fn modulo_return(e: &FnEmitter, s: ast::StmtId) -> Option<ExprId> {
    match &e.ast().stmt(s).kind {
        ast::StmtKind::Return(Some(x)) => Some(*x),
        ast::StmtKind::Block(stmts) if stmts.len() == 1 => modulo_return(e, stmts[0]),
        _ => None,
    }
}

fn modulo_js_arguments(e: &FnEmitter, x: ExprId, template: &str) -> Option<Vec<ExprId>> {
    let ExprKind::Call { target, arguments } = &e.expr(x).kind else { return None };
    if intrinsic_of(e.ctx, called_function(e, *target)?) != Some(Intrinsic::Js) || arguments.args.iter().any(|a| a.name.is_some()) { return None; }
    let [tag, text, rest @ ..] = arguments.args.as_ref() else { return None };
    let ExprKind::String(t) = &e.expr(tag.value).kind else { return None };
    let ExprKind::String(s) = &e.expr(text.value).kind else { return None };
    if t.constant_value()?.to_string_lossy() != "!" || s.constant_value()?.to_string_lossy() != template { return None; }
    Some(rest.iter().map(|a| a.value).collect())
}

/// Certify the actual SDK helper in its own lexical/type environment.
pub(crate) fn certified_number_modulo(ctx: &Ctx) -> bool {
    let Some(sdk) = &ctx.sdk else { return false };
    *sdk.number_modulo_certified.get_or_init(|| {
        let Some(class) = ctx.jsnumber else { return false };
        let Some(name) = ctx.sym("%") else { return false };
        let Some(fid) = ctx.program.class(class).instance_members.get(&name).copied() else { return false };
        let f = ctx.program.function(fid);
        let FunctionRef::Function { unit, function } = f.node else { return false };
        let state = ModState::new(f.library);
        let mut e = FnEmitter::new(ctx, &state, unit, Some(class), false);
        let af = ctx.program.unit(unit).ast.function(function);
        e.ret_ty = ctx.ty_of(ctx.outline.functions[fid.0 as usize].return_type);
        let signature = ctx.fn_ty(fid);
        e.declare_params(af.parameters.as_deref().unwrap_or(&[]), Some(&signature));
        modulo_number_comparison(&mut e, fid).is_some()
    })
}
