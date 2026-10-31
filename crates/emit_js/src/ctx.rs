//! Global emission context: program, types, well-known classes, the set of
//! native types (`dartx`), library names and type utilities.

use crate::js;
use crate::ty::{Ty, TyParam};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, ExtensionId, FunctionElementId, FunctionKind, LibraryId, Program,
    UnitId, VariableId,
};
use dartforge_frontend::ast;
use dartforge_frontend::ast::DeclKind as DeclKindRef;
use dartforge_intern::{Interner, SymbolId};
use dartforge_types::resolve::OutlineTypes;
use dartforge_types::resolved::BodyTypes;
use dartforge_types::table::{CoreTypes, Type, TypeId, TypeParamId, TypeTable};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Information about a library for emission.
#[derive(Clone, Debug)]
pub struct LibInfo {
    /// JS identifier (`main`, `lib__util`, `foo__src__a`, `core`).
    pub ident: String,
    /// Module path (`main.js`, `packages/foo/a.js`); empty for the SDK.
    pub module_path: String,
    pub is_sdk: bool,
    /// JS variable of the namespace (`L$main` for user code; `core` for the SDK).
    pub js_var: String,
}

/// Member found in a class.
#[derive(Clone, Debug)]
pub struct Member {
    pub class: ClassId,
    pub kind: MemberKind,
    /// Substitution of the declaring class's parameters.
    pub subst: HashMap<u32, Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberKind {
    Method(FunctionElementId),
    Getter(FunctionElementId),
    Setter(FunctionElementId),
    /// Field (implicit getter); `setter` tells whether it can be assigned.
    Field(VariableId),
}

type MemberLookup = (ClassId, SymbolId, bool);
type MemberResolution = Option<(ClassId, MemberKind)>;

pub struct Ctx<'a> {
    pub program: &'a Program,
    pub interner: &'a Interner,
    pub table: &'a TypeTable,
    pub core: &'a CoreTypes,
    pub outline: &'a OutlineTypes,
    pub bodies: &'a BodyTypes,
    pub libs: Vec<LibInfo>,
    pub object: Option<ClassId>,
    pub int_: Option<ClassId>,
    pub double_: Option<ClassId>,
    pub num_: Option<ClassId>,
    pub bool_: Option<ClassId>,
    pub string_: Option<ClassId>,
    pub null_: Option<ClassId>,
    pub list_: Option<ClassId>,
    pub map_: Option<ClassId>,
    pub set_: Option<ClassId>,
    pub iterable_: Option<ClassId>,
    pub function_: Option<ClassId>,
    pub record_: Option<ClassId>,
    pub future_: Option<ClassId>,
    pub stream_: Option<ClassId>,
    pub enum_: Option<ClassId>,
    pub underscore_enum: Option<ClassId>,
    pub type_: Option<ClassId>,
    pub symbol_: Option<ClassId>,
    pub comparable_: Option<ClassId>,
    pub js_array: Option<ClassId>,
    pub linked_hash_map: Option<ClassId>,
    pub linked_hash_set: Option<ClassId>,
    pub jsstring: Option<ClassId>,
    pub jsnumber: Option<ClassId>,
    pub jsbool: Option<ClassId>,
    pub stack_trace: Option<ClassId>,
    pub map_entry: Option<ClassId>,
    pub invocation_: Option<ClassId>,
    pub future_or: Option<ClassId>,
    /// Native classes (JSArray, JSString…) and the interfaces they implement:
    /// members accessed through `dartx`.
    pub ext_set: HashSet<ClassId>,
    pub native_set: HashSet<ClassId>,
    /// Direct supertypes (types instantiated in terms of the class's parameters).
    pub class_supers: Vec<Vec<Ty>>,
    pub class_params: Vec<Vec<TyParam>>,
    /// Bounds by type parameter id.
    pub param_bounds: RefCell<HashMap<u32, Ty>>,
    pub param_names: RefCell<HashMap<u32, String>>,
    pub next_param_id: RefCell<u32>,
    pub empty_sym: Option<SymbolId>,
    /// Strongly connected components of the import graph of the user's
    /// libraries: each one becomes a JS module (classes of libraries in a cycle
    /// need a single order). Key: id of each member → sorted group.
    pub groups: HashMap<u32, Vec<LibraryId>>,
    /// Classes `@JS` (package:js / dart:_js_annotations / dart:js_interop).
    pub js_classes: HashMap<ClassId, JsClass>,
    /// Libraries with `@JS(...)` on the `library` directive: JS prefix.
    pub js_libs: HashMap<LibraryId, Option<String>>,
    /// Functions (top-level, methods, constructors) with an explicit `@JS(...)`.
    pub fn_js: HashMap<FunctionElementId, Option<String>>,
    /// Variables (fields, top-level) with an explicit `@JS(...)`.
    pub var_js: HashMap<VariableId, Option<String>>,
    /// `dart:_interceptors::JSObject`: target of the erasure of interop
    /// extension types in rti recipes (it is what the DDC emits).
    pub js_object_ic: Option<ClassId>,
    /// Interop extension types that are kept (not erased to their
    /// representation): those of `dart:js_interop` and of `@JS()` libraries.
    pub interop_ext_types: HashSet<ClassId>,
    /// `@JSName('x')` of members of native classes: the JS name differs from
    /// the Dart name, and the DDC symbolizes these members (`_isSymbolizedMember`).
    pub js_names: HashMap<u32, String>,
    /// Memo of the member lookup: `(initial class, name, setter)` →
    /// `(declaring class, kind)`. The lookup walks the hierarchy and what it
    /// finds depends only on the class graph, not on the receiver's type
    /// arguments — that is why it can be stored. Measured on `new_sali`:
    /// 316 thousand lookups cost 1.84 M allocations (queue, visited set and
    /// instantiated supertypes on every call).
    member_memo: RefCell<HashMap<MemberLookup, MemberResolution>>,
    /// [`Ctx::visible_extensions`] per library.
    extension_memo: RefCell<HashMap<LibraryId, Vec<ExtensionId>>>,
    /// Memo of the rti of closed types (without `Ty::Param`): the recipe and
    /// the expression do not depend on the runtime environment. 132 thousand
    /// calls, 1.78 M allocations.
    pub rti_memo: RefCell<HashMap<Ty, String>>,
    /// [`Ctx::single_method_type`], by member name, computed on first use.
    pub single_method_types: RefCell<Option<HashMap<dartforge_intern::SymbolId, Option<Ty>>>>,
    /// The short ids of the classes in SDK-mode recipes
    /// ([`crate::recipe_ids`]), computed on first use.
    recipe_ids: std::cell::OnceCell<Vec<String>>,
    /// The classes some class of the program mixes in, computed on first use.
    mixed_in: std::cell::OnceCell<HashSet<ClassId>>,
    /// Classes that are a supertype of some class outside its `extends`
    /// chain ([`Ctx::nominal_class`]).
    implemented: std::cell::OnceCell<HashSet<ClassId>>,
    /// The classes in the prototype chain of a native peer (the peer, its
    /// superclasses and their mixins), computed on first use.
    native_chain: std::cell::OnceCell<HashSet<ClassId>>,
    /// [`Ctx::positional_type_variables`], computed on first use.
    positional_type_variables: std::cell::OnceCell<bool>,
    /// `DARTFORGE_JSPROD_SHORT_RECIPE=0` keeps the library in every recipe.
    short_recipe: bool,
    /// Per unit, the nodes that are the target of a compound assignment or of
    /// `++`/`--` ([`Ctx::is_write_target`]).
    write_targets: RefCell<HashMap<u32, std::rc::Rc<HashSet<u32>>>>,
    /// Memo of `as_super`: `(class, target)` → the target written in terms of
    /// the class's parameters. 78 thousand calls, 525 thousand allocations.
    super_memo: RefCell<HashMap<(ClassId, ClassId), Option<Ty>>>,
    /// Reachability filter of the production profile (`crate::filter`). `None`
    /// is the development profile: everything emitted, exactly as before.
    pub filter: Option<&'a dyn crate::filter::Liveness>,
    /// Language errors found during emission (a dot shorthand without a
    /// context or without a member, docs/VERSOES-LINGUAGEM.md §3): emission
    /// fails with them. The emitter is the one with the complete inference of
    /// the context.
    pub errors: RefCell<Vec<dartforge_diagnostics::Diagnostic>>,
    /// Check of the emitter's types against the common inference
    /// (`DARTFORGE_JS_CHECK_TYPES`, see `crate::type_check`); `None` when
    /// it is off.
    pub type_check: Option<RefCell<crate::type_check::TypeCheck>>,
    /// The `dart2js` check pragmas of each unit's declarations
    /// ([`crate::checks`]), computed on the first question about the unit.
    pub check_pragmas: RefCell<HashMap<UnitId, std::rc::Rc<crate::checks::UnitPragmas>>>,
    /// SDK mode (`crate::own_sdk`): the `dart:` libraries are compiled
    /// too, through our own pipeline. `None` on every path that links against
    /// `dart_sdk.js` — the mode's hooks do not fire.
    pub sdk: Option<Box<crate::own_sdk::SdkMode>>,
}

/// JS interop class (the DDC's `js_interop.dart`: `usesJSInterop`,
/// `isJSAnonymousType`, `isStaticInteropType`).
#[derive(Clone, Debug, Default)]
pub struct JsClass {
    /// Name in `@JS('name')`; `None` uses the Dart name.
    pub name: Option<String>,
    pub anonymous: bool,
    pub static_interop: bool,
}

impl<'a> Ctx<'a> {
    pub fn new(
        program: &'a Program,
        interner: &'a Interner,
        table: &'a TypeTable,
        core: &'a CoreTypes,
        outline: &'a OutlineTypes,
        bodies: &'a BodyTypes,
    ) -> Ctx<'a> {
        let find_lib = |uri: &str| -> Option<LibraryId> {
            program
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| LibraryId(i as u32))
        };
        let find_class = |lib: &str, name: &str| -> Option<ClassId> {
            let lib = find_lib(lib)?;
            let sym = interner.lookup(name)?;
            match program.library(lib).declared.get(&sym)?.getter {
                Some(Element::Class(c)) => Some(c),
                _ => None,
            }
        };
        let libs = compute_libs(program);
        let mut ctx = Ctx {
            program,
            interner,
            table,
            core,
            outline,
            bodies,
            libs,
            object: find_class("dart:core", "Object"),
            int_: find_class("dart:core", "int"),
            double_: find_class("dart:core", "double"),
            num_: find_class("dart:core", "num"),
            bool_: find_class("dart:core", "bool"),
            string_: find_class("dart:core", "String"),
            null_: find_class("dart:core", "Null"),
            list_: find_class("dart:core", "List"),
            map_: find_class("dart:core", "Map"),
            set_: find_class("dart:core", "Set"),
            iterable_: find_class("dart:core", "Iterable"),
            function_: find_class("dart:core", "Function"),
            record_: find_class("dart:core", "Record"),
            future_: find_class("dart:async", "Future"),
            stream_: find_class("dart:async", "Stream"),
            enum_: find_class("dart:core", "Enum"),
            underscore_enum: find_class("dart:core", "_Enum"),
            type_: find_class("dart:core", "Type"),
            symbol_: find_class("dart:core", "Symbol"),
            comparable_: find_class("dart:core", "Comparable"),
            js_array: find_class("dart:_interceptors", "JSArray"),
            linked_hash_map: find_class("dart:collection", "LinkedHashMap"),
            linked_hash_set: find_class("dart:collection", "LinkedHashSet"),
            jsstring: find_class("dart:_interceptors", "JSString"),
            jsnumber: find_class("dart:_interceptors", "JSNumber"),
            jsbool: find_class("dart:_interceptors", "JSBool"),
            stack_trace: find_class("dart:core", "StackTrace"),
            map_entry: find_class("dart:core", "MapEntry"),
            invocation_: find_class("dart:core", "Invocation"),
            future_or: find_class("dart:async", "FutureOr"),
            ext_set: HashSet::new(),
            native_set: HashSet::new(),
            class_supers: Vec::new(),
            class_params: Vec::new(),
            param_bounds: RefCell::new(HashMap::new()),
            param_names: RefCell::new(HashMap::new()),
            next_param_id: RefCell::new(1 << 30),
            empty_sym: interner.lookup(""),
            groups: HashMap::new(),
            js_classes: HashMap::new(),
            js_libs: HashMap::new(),
            fn_js: HashMap::new(),
            var_js: HashMap::new(),
            js_object_ic: find_class("dart:_interceptors", "JSObject"),
            interop_ext_types: HashSet::new(),
            js_names: HashMap::new(),
            member_memo: RefCell::new(HashMap::new()),
            extension_memo: RefCell::new(HashMap::new()),
            rti_memo: RefCell::new(HashMap::new()),
            single_method_types: RefCell::new(None),
            recipe_ids: std::cell::OnceCell::new(),
            mixed_in: std::cell::OnceCell::new(),
            implemented: std::cell::OnceCell::new(),
            native_chain: std::cell::OnceCell::new(),
            positional_type_variables: std::cell::OnceCell::new(),
            short_recipe: std::env::var("DARTFORGE_JSPROD_SHORT_RECIPE").map_or(true, |v| v != "0"),
            write_targets: RefCell::new(HashMap::new()),
            super_memo: RefCell::new(HashMap::new()),
            errors: RefCell::new(Vec::new()),
            filter: None,
            type_check: crate::type_check::TypeCheck::from_env().map(RefCell::new),
            check_pragmas: RefCell::new(HashMap::new()),
            sdk: None,
        };
        // Interop comes before the hierarchy: interop extension types are not
        // erased, and the supertypes (`implements JSAny`) need that for the
        // SDK's extensions to apply to the right receiver.
        ctx.compute_js_interop();
        ctx.compute_hierarchy();
        ctx.compute_ext_set(find_lib);
        ctx.compute_groups();
        ctx.compute_js_names();
        ctx
    }

    // -----------------------------------------------------------------------
    // JS interop (package:js): the DDC's `js_interop.dart`
    // -----------------------------------------------------------------------

    /// Annotation `@JS`/`@JS('name')`: `Some(name)`; others: `None`.
    fn ann_js(&self, unit: UnitId, a: &ast::Annotation) -> Option<Option<String>> {
        let last = a.name.last()?;
        if self.interner.resolve(last.sym) != "JS" || a.name.len() > 2 {
            return None;
        }
        let name = a.arguments.as_ref().and_then(|args| {
            let first = args.args.first()?;
            match &self.program.unit(unit).ast.expr(first.value).kind {
                ast::ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
                _ => None,
            }
        });
        Some(name)
    }

    fn ann_is(&self, a: &ast::Annotation, name: &str) -> bool {
        a.name
            .last()
            .is_some_and(|n| self.interner.resolve(n.sym) == name)
    }

    /// Classes, libraries, functions and variables annotated with `@JS` (outside the SDK).
    fn compute_js_interop(&mut self) {
        let mut fn_index: HashMap<(u32, u32), FunctionElementId> = HashMap::new();
        let mut ctor_index: HashMap<(u32, u32), FunctionElementId> = HashMap::new();
        for (i, f) in self.program.functions.iter().enumerate() {
            match f.node {
                dartforge_elements::model::FunctionRef::Function { unit, function } => {
                    fn_index.insert((unit.0, function.0), FunctionElementId(i as u32));
                }
                dartforge_elements::model::FunctionRef::Constructor { unit, member } => {
                    ctor_index.insert((unit.0, member.0), FunctionElementId(i as u32));
                }
                _ => {}
            }
        }
        let mut var_index: HashMap<(u32, u32, usize), VariableId> = HashMap::new();
        for (i, v) in self.program.variables.iter().enumerate() {
            match v.node {
                dartforge_elements::model::VariableRef::TopLevel { unit, decl, index } => {
                    var_index.insert((unit.0, decl.0, index), VariableId(i as u32));
                }
                dartforge_elements::model::VariableRef::Field {
                    unit,
                    member,
                    index,
                } => {
                    var_index.insert((unit.0, member.0 | (1 << 31), index), VariableId(i as u32));
                }
                _ => {}
            }
        }
        let mut js_classes = HashMap::new();
        let mut js_libs = HashMap::new();
        let mut fn_js = HashMap::new();
        let mut var_js = HashMap::new();
        for (li, lib) in self.program.libraries.iter().enumerate() {
            // In the SDK, only the `@JS('…')` names of the unpatched `external`
            // extension members of `dart:js_interop` (`callAsFunction`, which
            // the CFE lowers to `f.call(…)` at each call site:
            // `interop_extension_member`).
            let sdk_extensions_only = self.libs[li].is_sdk;
            if sdk_extensions_only && lib.uri != "dart:js_interop" {
                continue;
            }
            let lib_id = LibraryId(li as u32);
            for (ui, &uid) in lib.units.iter().enumerate() {
                let unit = self.program.unit(uid);
                if ui == 0 && !sdk_extensions_only {
                    for d in &unit.unit.directives {
                        if let ast::DirectiveKind::Library { .. } = d.kind {
                            for a in &d.metadata {
                                if let Some(n) = self.ann_js(uid, a) {
                                    js_libs.insert(lib_id, n);
                                }
                            }
                        }
                    }
                }
                for &did in &unit.unit.declarations {
                    let d = unit.ast.decl(did);
                    if sdk_extensions_only && !matches!(d.kind, DeclKindRef::Extension(_)) {
                        continue;
                    }
                    let members: &[ast::MemberId] = match &d.kind {
                        DeclKindRef::Class(cd) => &cd.members,
                        DeclKindRef::Mixin(md) => &md.members,
                        DeclKindRef::Enum(ed) => &ed.members,
                        DeclKindRef::Extension(ed) => &ed.members,
                        DeclKindRef::ExtensionType(ed) => &ed.members,
                        DeclKindRef::Function(fid) => {
                            if let Some(&fe) = fn_index.get(&(uid.0, fid.0)) {
                                for a in d.metadata.iter() {
                                    if let Some(n) = self.ann_js(uid, a) {
                                        fn_js.insert(fe, n);
                                    }
                                }
                            }
                            continue;
                        }
                        DeclKindRef::Variables(list) => {
                            for (idx, _) in list.variables.iter().enumerate() {
                                if let Some(&vid) = var_index.get(&(uid.0, did.0, idx)) {
                                    for a in d.metadata.iter() {
                                        if let Some(n) = self.ann_js(uid, a) {
                                            var_js.insert(vid, n);
                                        }
                                    }
                                }
                            }
                            continue;
                        }
                        DeclKindRef::Typedef(_) => continue,
                    };
                    for &mid in members {
                        let mem = unit.ast.member(mid);
                        if mem.metadata.is_empty() {
                            continue;
                        }
                        let js_name = mem.metadata.iter().find_map(|a| self.ann_js(uid, a));
                        let Some(js_name) = js_name else { continue };
                        match &mem.kind {
                            ast::MemberKind::Method(fid) => {
                                if let Some(&fe) = fn_index.get(&(uid.0, fid.0)) {
                                    fn_js.insert(fe, js_name);
                                }
                            }
                            ast::MemberKind::Constructor(_) => {
                                if let Some(&fe) = ctor_index.get(&(uid.0, mid.0)) {
                                    fn_js.insert(fe, js_name);
                                }
                            }
                            ast::MemberKind::Field(list) => {
                                for (idx, _) in list.variables.iter().enumerate() {
                                    if let Some(&vid) =
                                        var_index.get(&(uid.0, mid.0 | (1 << 31), idx))
                                    {
                                        var_js.insert(vid, js_name.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        for (i, c) in self.program.classes.iter().enumerate() {
            if self.libs[c.library.0 as usize].is_sdk {
                continue;
            }
            let Some(decl) = c.decl else { continue };
            let d = self.program.unit(decl.unit).ast.decl(decl.decl);
            let mut info: Option<JsClass> = None;
            for a in d.metadata.iter() {
                if let Some(n) = self.ann_js(decl.unit, a) {
                    info.get_or_insert_with(JsClass::default).name = n;
                }
            }
            if let Some(info) = info.as_mut() {
                info.anonymous = d.metadata.iter().any(|a| self.ann_is(a, "anonymous"));
                info.static_interop = d.metadata.iter().any(|a| self.ann_is(a, "staticInterop"));
            }
            // `dart:js_interop`/package:web: `extension type X._(JSObject _)`
            // in a `@JS()` library, with `external` members — it is interop by
            // the same `usesJSInterop` of the DDC (an annotation on the library
            // is enough for `external` members, and those of an extension type
            // never have a class).
            if info.is_none()
                && c.kind == ClassKind::ExtensionType
                && js_libs.contains_key(&c.library)
            {
                info = Some(JsClass {
                    name: None,
                    anonymous: false,
                    static_interop: true,
                });
            }
            if let Some(info) = info {
                js_classes.insert(ClassId(i as u32), info);
            }
        }
        // Extension types of `dart:js_interop` (JSPromise, JSString, JSAny…):
        // we do not emit them, but the type has to survive for the SDK's
        // extension members (`toDart`, `toJS`) to be the right ones.
        let mut ext_types: HashSet<ClassId> = js_classes
            .keys()
            .copied()
            .filter(|c| self.program.class(*c).kind == ClassKind::ExtensionType)
            .collect();
        for (i, c) in self.program.classes.iter().enumerate() {
            if c.kind != ClassKind::ExtensionType {
                continue;
            }
            let uri = self.program.library(c.library).uri.as_str();
            if uri == "dart:js_interop" || uri == "dart:js_interop_unsafe" {
                ext_types.insert(ClassId(i as u32));
            }
        }
        self.interop_ext_types = ext_types;
        self.js_classes = js_classes;
        self.js_libs = js_libs;
        self.fn_js = fn_js;
        self.var_js = var_js;
    }

    /// Annotation `@JSName('x')` (from `dart:_js_helper`): the JS name of the native member.
    fn ann_js_name(&self, unit: UnitId, a: &ast::Annotation) -> Option<String> {
        let last = a.name.last()?;
        if self.interner.resolve(last.sym) != "JSName" {
            return None;
        }
        let args = a.arguments.as_ref()?;
        let first = args.args.first()?;
        match &self.program.unit(unit).ast.expr(first.value).kind {
            ast::ExprKind::String(lit) => lit.constant_value().map(|v| v.to_string_lossy()),
            _ => None,
        }
    }

    /// Map `member → JS name` for the classes of the native hierarchy. It is
    /// read from the outline's `@JSName` annotation (not from a list of names):
    /// in `dart:html` there are 614 members, and the rule applies to any
    /// `@Native` class.
    fn compute_js_names(&mut self) {
        let mut map: HashMap<u32, String> = HashMap::new();
        let classes: Vec<ClassId> = self.ext_set.iter().copied().collect();
        for c in classes {
            let class = self.program.class(c);
            let Some(decl) = class.decl else { continue };
            let unit = self.program.unit(decl.unit);
            let d = unit.ast.decl(decl.decl);
            let members: &[ast::MemberId] = match &d.kind {
                DeclKindRef::Class(cd) => &cd.members,
                DeclKindRef::Mixin(md) => &md.members,
                _ => continue,
            };
            // JS name per AST function (methods, getters, setters) and per
            // field declaration (the implicit accessors inherit the name).
            let mut by_function: HashMap<u32, String> = HashMap::new();
            let mut by_field: HashMap<u32, String> = HashMap::new();
            for &mid in members {
                let mem = unit.ast.member(mid);
                let Some(js_name) = mem
                    .metadata
                    .iter()
                    .find_map(|a| self.ann_js_name(decl.unit, a))
                else {
                    continue;
                };
                match &mem.kind {
                    ast::MemberKind::Method(fid) => {
                        by_function.insert(fid.0, js_name);
                    }
                    ast::MemberKind::Field(_) => {
                        by_field.insert(mid.0, js_name);
                    }
                    ast::MemberKind::Constructor(_) => {}
                }
            }
            if by_function.is_empty() && by_field.is_empty() {
                continue;
            }
            for &fid in class
                .instance_members
                .values()
                .chain(class.static_members.values())
            {
                let f = self.program.function(fid);
                if let dartforge_elements::model::FunctionRef::Function { function, .. } = f.node {
                    if let Some(n) = by_function.get(&function.0) {
                        map.insert(fid.0, n.clone());
                        continue;
                    }
                }
                if let Some(v) = f.variable {
                    if let dartforge_elements::model::VariableRef::Field { member, .. } =
                        self.program.variable(v).node
                    {
                        if let Some(n) = by_field.get(&member.0) {
                            map.insert(fid.0, n.clone());
                        }
                    }
                }
            }
        }
        self.js_names = map;
    }

    /// Representation type of an extension type, with the arguments
    /// substituted: `JSArray<JSString>` → `_interceptors.JSArray<Object?>`,
    /// `JSString` → `String`, `JSObject` → `_interceptors.JSObject`. It is what
    /// the DDC uses in rti recipes (extension types are erased).
    pub fn erase_ext(&self, class: ClassId, args: &[Ty]) -> Option<Ty> {
        let rep = self.program.class(class).representation?;
        let v = &self.outline.variables[rep.0 as usize];
        let t = v.declared_type.or(v.inferred)?;
        let mut map = HashMap::new();
        for (p, a) in self
            .class_params
            .get(class.0 as usize)?
            .iter()
            .zip(args.iter())
        {
            map.insert(p.id, a.clone());
        }
        Some(self.ty_of(t).subst_prop(&map))
    }

    /// Reachability level of the class; without a filter, `Instantiated` (everything emitted).
    pub fn class_level(&self, c: ClassId) -> crate::filter::Level {
        self.filter
            .map_or(crate::filter::Level::Instantiated, |f| f.class_level(c))
    }

    /// State of the function; without a filter, `Live`.
    /// The name a `LateError` message shows for the `late` variable `name`:
    /// in the production profile none, as `dart2js -O2` (`omitLateNames`)
    /// prints `Field '' has not been initialized.`
    /// (`docs/JS-PRODUCAO-TAMANHO.md` §13.5). `DARTFORGE_JSPROD_LATE_NAMES=0`
    /// keeps them; the development profile always keeps them.
    pub fn late_name<'n>(&self, name: &'n str) -> &'n str {
        if self.production()
            && self.filter.is_some_and(|f| f.omit_late_names())
            && std::env::var("DARTFORGE_JSPROD_LATE_NAMES").map_or(true, |v| v != "0")
        {
            ""
        } else {
            name
        }
    }

    /// The production profile's emission choices that are not pruning
    /// ([`crate::filter::Liveness::production`]).
    pub fn production(&self) -> bool {
        self.filter.is_some_and(|f| f.production())
    }

    pub fn fn_state(&self, f: FunctionElementId) -> crate::filter::State {
        self.filter
            .map_or(crate::filter::State::Live, |x| x.function(f))
    }

    /// State of the variable; without a filter, `Live`.
    pub fn var_state(&self, v: VariableId) -> crate::filter::State {
        self.filter
            .map_or(crate::filter::State::Live, |x| x.variable(v))
    }

    /// Name invoked somewhere; without a filter, always.
    pub fn selector_live(&self, name: &str) -> bool {
        self.filter.is_none_or(|x| x.selector(name))
    }

    /// Name written somewhere (`x.name = v`); without a filter, always.
    pub fn write_selector(&self, base_name: &str) -> bool {
        self.filter.is_none_or(|x| x.write_selector(base_name))
    }

    /// Static tearoff of the referenced constructor; without a filter, always.
    pub fn tearoff_live(&self, f: FunctionElementId) -> bool {
        self.filter.is_none_or(|x| x.tearoff_ctor(f))
    }

    /// The signature entry of the member `name` is emitted ([`crate::filter::Liveness::signature`]).
    pub fn signature_live(&self, name: &str) -> bool {
        self.filter.is_none_or(|x| x.signature(name))
    }

    /// The `dartx` signature entry of the member `name` of a Dart class
    /// outside every native chain is emitted
    /// ([`crate::filter::Liveness::symbol_signature`]).
    pub fn symbol_signature_live(&self, name: &str) -> bool {
        self.filter.is_none_or(|x| x.symbol_signature(name))
    }

    /// Label of a pruned element for the *stub* (`dart_podado("…")`).
    pub fn pruned_label(&self, lib: LibraryId, class: Option<ClassId>, name: &str) -> String {
        let uri = &self.program.library(lib).uri;
        match class {
            Some(c) => format!("{uri}::{}.{name}", self.class_name(c)),
            None => format!("{uri}::{name}"),
        }
    }

    pub fn is_js_class(&self, c: ClassId) -> bool {
        self.js_classes.contains_key(&c)
    }

    /// `external` interop member (the DDC's `isJsMember`): outside the SDK,
    /// with `@JS` on the member itself, on the class or (top-level) on the
    /// library.
    pub fn is_js_member(&self, fid: FunctionElementId) -> bool {
        let f = self.program.function(fid);
        if !f.external || self.libs[f.library.0 as usize].is_sdk {
            return false;
        }
        if self.fn_js.contains_key(&fid) || f.variable.is_some_and(|v| self.var_js.contains_key(&v))
        {
            return true;
        }
        if self.interop_extension_member(fid).is_some() {
            return true;
        }
        match f.class {
            Some(c) => self.js_classes.contains_key(&c),
            None => self.js_libs.contains_key(&f.library),
        }
    }

    /// JS property name of an `external` instance member of an `extension`
    /// (`extension SafeElement on Element { @JS('innerHTML') external String
    /// get innerHtml; }`): `@JS('x')` or the Dart name. The CFE only accepts
    /// such members on interop types, and its `JsUtilOptimizer` lowers them to
    /// a direct property access, call or write on the receiver
    /// (`js_util.getProperty`/`callMethod`/`setProperty`); the extension has
    /// no `Ext|get#m` function for them.
    pub fn interop_extension_member(&self, fid: FunctionElementId) -> Option<String> {
        let f = self.program.function(fid);
        if !f.external || f.static_ || f.extension.is_none() {
            return None;
        }
        if self.libs[f.library.0 as usize].is_sdk {
            // `dart:js_interop`'s `@JS('call') external callAsFunction(…)` has
            // no patch: like the CFE, every call becomes `f.call(…)`.
            return self
                .fn_js
                .get(&fid)
                .cloned()
                .flatten()
                .filter(|n| !n.is_empty());
        }
        let explicit = self
            .fn_js
            .get(&fid)
            .cloned()
            .flatten()
            .filter(|n| !n.is_empty());
        Some(explicit.unwrap_or_else(|| self.name(f.name).trim_end_matches("_=").to_string()))
    }

    pub fn is_js_var(&self, vid: VariableId) -> bool {
        let v = self.program.variable(vid);
        if !v.external || self.libs[v.library.0 as usize].is_sdk {
            return false;
        }
        if self.var_js.contains_key(&vid) {
            return true;
        }
        match v.class {
            Some(c) => self.js_classes.contains_key(&c),
            None => self.js_libs.contains_key(&v.library),
        }
    }

    pub fn is_js_member_kind(&self, mk: &MemberKind) -> bool {
        match *mk {
            MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
                self.is_js_member(f)
            }
            MemberKind::Field(v) => self.is_js_var(v),
        }
    }

    /// `_isNullCheckableJsInterop`: an interop member (not `@staticInterop`)
    /// whose type (return or getter) is non-nullable. Never from `-O3` up,
    /// as `dart2js`'s `nativeNullAssertions`.
    pub fn js_null_checkable(&self, mk: &MemberKind) -> bool {
        if !self.is_js_member_kind(mk) || self.filter.is_some_and(|f| f.omit_implicit_checks()) {
            return false;
        }
        let class = match *mk {
            MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
                self.program.function(f).class
            }
            MemberKind::Field(v) => self.program.variable(v).class,
        };
        if class
            .and_then(|c| self.js_classes.get(&c))
            .is_some_and(|j| j.static_interop)
        {
            return false;
        }
        let ty = match *mk {
            MemberKind::Method(f) | MemberKind::Getter(f) => {
                self.ty_of(self.outline.functions[f.0 as usize].return_type)
            }
            MemberKind::Setter(_) => return false,
            MemberKind::Field(v) => self.var_ty(v),
        };
        !ty.is_nullable()
    }

    /// `dart.global.<library prefix>.<parts>` (`_emitJSInteropForGlobal`).
    pub fn js_global(&self, lib: LibraryId, name: &str) -> String {
        let mut full = String::new();
        if let Some(Some(p)) = self.js_libs.get(&lib) {
            full.push_str(p);
            full.push('.');
        }
        full.push_str(name);
        let mut out = String::from("dart.global");
        for part in full.split('.') {
            out.push_str(&js::prop_access(part));
        }
        out
    }

    /// Global JS name of an interop class (`_jsNameWithoutGlobal`).
    pub fn js_class_global(&self, c: ClassId) -> String {
        let class = self.program.class(c);
        let name = self
            .js_classes
            .get(&c)
            .and_then(|j| j.name.clone())
            .unwrap_or_else(|| self.name(class.name).to_string());
        self.js_global(class.library, &name)
    }

    /// JS name of an `external` static/top-level interop member
    /// (`_emitJSInteropExternalStaticMemberName`): `@JS('x')` or the Dart name.
    pub fn js_interop_member_name(&self, mk: &MemberKind, dart_name: &str) -> String {
        let explicit = match *mk {
            MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
                self.fn_js.get(&f).cloned().flatten().or_else(|| {
                    self.program
                        .function(f)
                        .variable
                        .and_then(|v| self.var_js.get(&v).cloned().flatten())
                })
            }
            MemberKind::Field(v) => self.var_js.get(&v).cloned().flatten(),
        };
        explicit.unwrap_or_else(|| dart_name.to_string())
    }

    /// Global reference of an `external` static member of an interop class, or
    /// of an interop top-level function/variable.
    pub fn js_static_ref(
        &self,
        class: Option<ClassId>,
        lib: LibraryId,
        mk: &MemberKind,
        dart_name: &str,
    ) -> String {
        let member = self.js_interop_member_name(mk, dart_name);
        match class {
            Some(c) => format!("{}{}", self.js_class_global(c), js::prop_access(&member)),
            None => self.js_global(lib, &member),
        }
    }

    /// Module granularity (docs/EMISSAO-DDC.md): project libraries → one
    /// module per library; pub cache packages → one module per package
    /// (`packages/<name>.js`). Import cycles between modules are merged into
    /// the module of the first library (classes in a cycle need a single order).
    fn compute_groups(&mut self) {
        let n = self.program.libraries.len();
        let is_user =
            |i: usize, s: &Self| !s.libs[i].is_sdk && !s.program.libraries[i].units.is_empty();
        // 1. Initial module of each library.
        let mut key_of: Vec<Option<String>> = vec![None; n];
        for (i, module_key) in key_of.iter_mut().enumerate() {
            if !is_user(i, self) {
                continue;
            }
            let lib = &self.program.libraries[i];
            let in_pub_cache = lib
                .units
                .first()
                .and_then(|u| self.program.unit(*u).path.as_ref())
                .map(|p| {
                    let s = p.to_string_lossy().replace('\\', "/").to_lowercase();
                    s.contains("/pub/cache/")
                        || s.contains("/.pub-cache/")
                        || s.contains("/hosted/pub.dev/")
                })
                .unwrap_or(false);
            let key = match lib.uri.strip_prefix("package:") {
                Some(rest) if in_pub_cache => {
                    let pkg = rest.split('/').next().unwrap_or(rest);
                    format!("packages/{pkg}.js")
                }
                _ => self.libs[i].module_path.clone(),
            };
            self.libs[i].module_path = key.clone();
            *module_key = Some(key);
        }
        // 2. Module graph.
        let mut keys: Vec<String> = Vec::new();
        let mut index_of: HashMap<String, usize> = HashMap::new();
        for k in key_of.iter().flatten() {
            if !index_of.contains_key(k) {
                index_of.insert(k.clone(), keys.len());
                keys.push(k.clone());
            }
        }
        let m = keys.len();
        let mut edges: Vec<Vec<usize>> = vec![Vec::new(); m];
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); m];
        for i in 0..n {
            let Some(k) = &key_of[i] else { continue };
            let a = index_of[k];
            members[a].push(i);
            let lib = &self.program.libraries[i];
            for j in lib
                .imports
                .iter()
                .map(|x| x.library.0 as usize)
                .chain(lib.exports.iter().map(|x| x.library.0 as usize))
            {
                if let Some(kj) = &key_of[j] {
                    let b = index_of[kj];
                    if b != a && !edges[a].contains(&b) {
                        edges[a].push(b);
                    }
                }
            }
        }
        // 3. Iterative Tarjan over the modules.
        let mut index = vec![usize::MAX; m];
        let mut low = vec![0usize; m];
        let mut on_stack = vec![false; m];
        let mut stack: Vec<usize> = Vec::new();
        let mut next_index = 0usize;
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        for start in 0..m {
            if index[start] != usize::MAX {
                continue;
            }
            let mut call: Vec<(usize, usize)> = vec![(start, 0)];
            index[start] = next_index;
            low[start] = next_index;
            next_index += 1;
            stack.push(start);
            on_stack[start] = true;
            while let Some(&mut (v, ref mut ei)) = call.last_mut() {
                if *ei < edges[v].len() {
                    let w = edges[v][*ei];
                    *ei += 1;
                    if index[w] == usize::MAX {
                        index[w] = next_index;
                        low[w] = next_index;
                        next_index += 1;
                        stack.push(w);
                        on_stack[w] = true;
                        call.push((w, 0));
                    } else if on_stack[w] {
                        low[v] = low[v].min(index[w]);
                    }
                } else {
                    call.pop();
                    if let Some(&(u, _)) = call.last() {
                        low[u] = low[u].min(low[v]);
                    }
                    if low[v] == index[v] {
                        let mut comp = Vec::new();
                        while let Some(w) = stack.pop() {
                            on_stack[w] = false;
                            comp.push(w);
                            if w == v {
                                break;
                            }
                        }
                        sccs.push(comp);
                    }
                }
            }
        }
        // 4. Final groups: the libraries of each component, sorted by id.
        for comp in sccs {
            let mut libs: Vec<usize> = comp
                .iter()
                .flat_map(|&mi| members[mi].iter().copied())
                .collect();
            libs.sort();
            if libs.len() < 2 {
                continue;
            }
            // Path: that of the first library's module (package → `packages/x.js`).
            let path = key_of[libs[0]].clone().unwrap();
            let group: Vec<LibraryId> = libs.iter().map(|&i| LibraryId(i as u32)).collect();
            for &i in &libs {
                self.libs[i].module_path = path.clone();
                self.groups.insert(i as u32, group.clone());
            }
        }
    }

    fn compute_hierarchy(&mut self) {
        let n = self.program.classes.len();
        let mut supers = Vec::with_capacity(n);
        let mut params = Vec::with_capacity(n);
        for i in 0..n {
            let data = &self.outline.classes[i];
            let mut ps = Vec::new();
            for &pid in data.type_params.iter() {
                ps.push(self.ty_param_of(pid));
            }
            params.push(ps);
            let mut s = Vec::new();
            if let Some(t) = data.supertype {
                s.push(self.ty_of(t));
            } else if self.program.classes[i].kind == ClassKind::Enum {
                if let Some(e) = self.underscore_enum {
                    s.push(Ty::iface(e));
                }
            }
            for &t in data.mixins.iter() {
                s.push(self.ty_of(t));
            }
            for &t in data.interfaces.iter() {
                s.push(self.ty_of(t));
            }
            for &t in data.on.iter() {
                s.push(self.ty_of(t));
            }
            supers.push(s);
        }
        self.class_supers = supers;
        self.class_params = params;
    }

    fn compute_ext_set(&mut self, find_lib: impl Fn(&str) -> Option<LibraryId>) {
        let mut natives = Vec::new();
        // `@Native` classes of the SDK: interceptors, typed data and the web
        // libraries (in the DDC with null safety the native members of
        // `dart:html` are symbolized too, with `checkNativeNonNull`).
        for lib_uri in [
            "dart:_interceptors",
            "dart:_native_typed_data",
            "dart:html",
            "dart:indexed_db",
            "dart:svg",
            "dart:web_audio",
            "dart:web_gl",
        ] {
            if let Some(lib) = find_lib(lib_uri) {
                for (i, c) in self.program.classes.iter().enumerate() {
                    if c.library == lib && self.has_native_annotation(ClassId(i as u32)) {
                        natives.push(ClassId(i as u32));
                    }
                }
            }
        }
        for c in [self.int_, self.double_, self.bool_, self.string_]
            .into_iter()
            .flatten()
        {
            natives.push(c);
        }
        let mut extensible: Vec<ClassId> = Vec::new();
        if let Some(f) = self.function_ {
            extensible.push(f);
        }
        for (lib, names) in [
            ("dart:core", &["Comparable", "Map"][..]),
            ("dart:collection", &["ListBase", "MapBase"][..]),
            ("dart:math", &["Rectangle"][..]),
        ] {
            if let Some(l) = find_lib(lib) {
                for n in names {
                    if let Some(sym) = self.interner.lookup(n) {
                        if let Some(Element::Class(c)) = self
                            .program
                            .library(l)
                            .declared
                            .get(&sym)
                            .and_then(|b| b.getter)
                        {
                            extensible.push(c);
                        }
                    }
                }
            }
        }
        let mut ext_set = HashSet::new();
        let mut native_set = HashSet::new();
        let mut queue: Vec<ClassId> = Vec::new();
        for c in natives {
            native_set.insert(c);
            ext_set.insert(c);
            queue.push(c);
        }
        for c in extensible {
            ext_set.insert(c);
            queue.push(c);
        }
        while let Some(c) = queue.pop() {
            for s in &self.class_supers[c.0 as usize] {
                if let Some(sc) = s.class() {
                    if Some(sc) == self.object {
                        continue;
                    }
                    if ext_set.insert(sc) {
                        queue.push(sc);
                    }
                }
            }
        }
        if let Some(o) = self.object {
            ext_set.insert(o);
        }
        self.ext_set = ext_set;
        self.native_set = native_set;
    }

    fn has_native_annotation(&self, c: ClassId) -> bool {
        let class = self.program.class(c);
        let Some(decl) = class.decl else { return false };
        let unit = self.program.unit(decl.unit);
        let d = unit.ast.decl(decl.decl);
        d.metadata.iter().any(|a| {
            let n = a
                .name
                .first()
                .map(|n| self.interner.resolve(n.sym))
                .unwrap_or("");
            n == "Native" || n == "JsPeerInterface"
        })
    }

    // -----------------------------------------------------------------------
    // Type conversion
    // -----------------------------------------------------------------------

    pub fn ty_param_of(&self, pid: TypeParamId) -> TyParam {
        let data = self.table.param(pid);
        let bound = self.ty_of(data.bound);
        let name = self.interner.resolve(data.name).to_string();
        self.param_bounds.borrow_mut().insert(pid.0, bound.clone());
        self.param_names.borrow_mut().insert(pid.0, name.clone());
        TyParam {
            id: pid.0,
            name,
            bound: Box::new(bound),
        }
    }

    pub fn ty_of(&self, id: TypeId) -> Ty {
        match self.table.get(id) {
            Type::Dynamic => Ty::Dynamic,
            Type::Void => Ty::Void,
            Type::Never => Ty::Never,
            Type::Null => Ty::Null,
            Type::Interface {
                class,
                args,
                nullable,
            } => {
                // Raw type coming from the table (`Caixa` for `Caixa<dynamic>`):
                // complete the arity here, at the single entry point, so that
                // recipes, rti rules, substitutions and member lookup always
                // see the same shape (see `args_at_arity`).
                let args: Vec<Ty> = args.iter().map(|a| self.ty_of(*a)).collect();
                let args = self.args_at_arity(*class, &args).into_owned();
                Ty::Iface {
                    class: *class,
                    args,
                    nullable: *nullable,
                }
            }
            Type::Function {
                type_params,
                ret,
                positional,
                optional,
                named,
                nullable,
            } => Ty::Fn {
                type_params: type_params.iter().map(|p| self.ty_param_of(*p)).collect(),
                ret: Box::new(self.ty_of(*ret)),
                pos: positional.iter().map(|a| self.ty_of(*a)).collect(),
                opt: optional.iter().map(|a| self.ty_of(*a)).collect(),
                named: {
                    let mut v: Vec<(String, Ty, bool)> = named
                        .iter()
                        .map(|(n, t, r)| {
                            (self.interner.resolve(*n).to_string(), self.ty_of(*t), *r)
                        })
                        .collect();
                    v.sort_by(|a, b| a.0.cmp(&b.0));
                    v
                },
                nullable: *nullable,
            },
            Type::Record {
                positional,
                named,
                nullable,
            } => {
                let mut named: Vec<(String, Ty)> = named
                    .iter()
                    .map(|(n, t)| (self.interner.resolve(*n).to_string(), self.ty_of(*t)))
                    .collect();
                named.sort_by(|a, b| a.0.cmp(&b.0));
                Ty::Record {
                    pos: positional.iter().map(|a| self.ty_of(*a)).collect(),
                    named,
                    nullable: *nullable,
                }
            }
            Type::TypeParameter { param, nullable } => {
                let data = self.table.param(*param);
                let name = self.interner.resolve(data.name).to_string();
                if !self.param_bounds.borrow().contains_key(&param.0) {
                    // Register the bound (avoiding infinite recursion on F-bounds).
                    self.param_bounds.borrow_mut().insert(param.0, Ty::Dynamic);
                    let b = self.ty_of(data.bound);
                    self.param_bounds.borrow_mut().insert(param.0, b);
                    self.param_names.borrow_mut().insert(param.0, name.clone());
                }
                Ty::Param {
                    id: param.0,
                    name,
                    nullable: *nullable,
                }
            }
            Type::FutureOr { arg, nullable } => Ty::FutureOr {
                arg: Box::new(self.ty_of(*arg)),
                nullable: *nullable,
            },
            // `X & B` (type variable promotion, only in inference): at runtime
            // it is the variable `X` itself.
            Type::Intersection { param, .. } => {
                let p = *param;
                let t = self.table.intern_lookup(Type::TypeParameter {
                    param: p,
                    nullable: false,
                });
                match t {
                    Some(t) => self.ty_of(t),
                    None => Ty::Dynamic,
                }
            }
            Type::ExtensionType {
                decl,
                args,
                nullable,
            } => {
                // Interop (`@JS`): keep the type, so that the `external`
                // members become properties of the JS object; the others are
                // erased to the representation type.
                if self.interop_ext_types.contains(decl) {
                    let args: Vec<Ty> = args.iter().map(|a| self.ty_of(*a)).collect();
                    let args = self.args_at_arity(*decl, &args).into_owned();
                    return Ty::Iface {
                        class: *decl,
                        args,
                        nullable: *nullable,
                    };
                }
                let class = self.program.class(*decl);
                if let Some(rep) = class.representation {
                    let v = &self.outline.variables[rep.0 as usize];
                    if let Some(t) = v.declared_type.or(v.inferred) {
                        let mut map = HashMap::new();
                        // During `compute_hierarchy` the table does not exist
                        // yet: a supertype may mention an extension type, so the
                        // parameters come from the outline in that case.
                        let params: Vec<TyParam> = match self.class_params.get(decl.0 as usize) {
                            Some(p) => p.clone(),
                            None => self.outline.classes[decl.0 as usize]
                                .type_params
                                .iter()
                                .map(|&pid| self.ty_param_of(pid))
                                .collect(),
                        };
                        for (p, a) in params.iter().zip(args.iter()) {
                            map.insert(p.id, self.ty_of(*a));
                        }
                        let t = self.ty_of(t).subst_prop(&map);
                        return if *nullable { t.with_nullable(true) } else { t };
                    }
                }
                Ty::Dynamic
            }
        }
    }

    pub fn fresh_param(&self, name: &str, bound: Ty) -> TyParam {
        let mut n = self.next_param_id.borrow_mut();
        let id = *n;
        *n += 1;
        self.param_bounds.borrow_mut().insert(id, bound.clone());
        self.param_names.borrow_mut().insert(id, name.to_string());
        TyParam {
            id,
            name: name.to_string(),
            bound: Box::new(bound),
        }
    }

    pub fn bound_of(&self, id: u32) -> Ty {
        self.param_bounds
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or(Ty::Dynamic)
    }

    // -----------------------------------------------------------------------
    // Well-known types
    // -----------------------------------------------------------------------

    pub fn t_int(&self) -> Ty {
        self.int_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_double(&self) -> Ty {
        self.double_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_num(&self) -> Ty {
        self.num_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_bool(&self) -> Ty {
        self.bool_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_string(&self) -> Ty {
        self.string_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_object(&self) -> Ty {
        self.object.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }
    pub fn t_object_q(&self) -> Ty {
        self.object
            .map(|c| Ty::Iface {
                class: c,
                args: vec![],
                nullable: true,
            })
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_list(&self, e: Ty) -> Ty {
        self.list_
            .map(|c| Ty::iface_args(c, vec![e]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_map(&self, k: Ty, v: Ty) -> Ty {
        self.map_
            .map(|c| Ty::iface_args(c, vec![k, v]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_set(&self, e: Ty) -> Ty {
        self.set_
            .map(|c| Ty::iface_args(c, vec![e]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_iterable(&self, e: Ty) -> Ty {
        self.iterable_
            .map(|c| Ty::iface_args(c, vec![e]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_future(&self, e: Ty) -> Ty {
        self.future_
            .map(|c| Ty::iface_args(c, vec![e]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_stream(&self, e: Ty) -> Ty {
        self.stream_
            .map(|c| Ty::iface_args(c, vec![e]))
            .unwrap_or(Ty::Dynamic)
    }
    pub fn t_type(&self) -> Ty {
        self.type_.map(Ty::iface).unwrap_or(Ty::Dynamic)
    }

    pub fn is_int(&self, t: &Ty) -> bool {
        t.is_class(self.int_) && !t.is_nullable()
    }
    pub fn is_double(&self, t: &Ty) -> bool {
        t.is_class(self.double_) && !t.is_nullable()
    }
    pub fn is_num_like(&self, t: &Ty) -> bool {
        (t.is_class(self.int_) || t.is_class(self.double_) || t.is_class(self.num_))
            && !t.is_nullable()
    }
    pub fn is_num_like_nullable(&self, t: &Ty) -> bool {
        t.is_class(self.int_) || t.is_class(self.double_) || t.is_class(self.num_)
    }
    pub fn is_string(&self, t: &Ty) -> bool {
        t.is_class(self.string_) && !t.is_nullable()
    }
    pub fn is_bool(&self, t: &Ty) -> bool {
        t.is_class(self.bool_) && !t.is_nullable()
    }
    /// Types whose JS representation is primitive (number, string, bool, null).
    pub fn is_js_primitive(&self, t: &Ty) -> bool {
        let t = self.resolve_param_bound(t);
        match &t {
            Ty::Null => true,
            Ty::Iface { class, .. } => {
                Some(*class) == self.int_
                    || Some(*class) == self.double_
                    || Some(*class) == self.num_
                    || Some(*class) == self.bool_
                    || Some(*class) == self.string_
                    || Some(*class) == self.null_
                    || Some(*class) == self.jsnumber
                    || Some(*class) == self.jsstring
                    || Some(*class) == self.jsbool
            }
            _ => false,
        }
    }
    pub fn resolve_param_bound(&self, t: &Ty) -> Ty {
        let mut t = t.clone();
        let mut guard = 0;
        while let Ty::Param { id, nullable, .. } = &t {
            let b = self.bound_of(*id);
            let n = *nullable;
            t = if n { b.with_nullable(true) } else { b };
            guard += 1;
            if guard > 8 {
                break;
            }
        }
        t
    }
    pub fn is_enum_class(&self, c: ClassId) -> bool {
        self.program.class(c).kind == ClassKind::Enum
    }

    // -----------------------------------------------------------------------
    // Hierarchy and members
    // -----------------------------------------------------------------------

    pub fn class_name(&self, c: ClassId) -> &str {
        self.interner.resolve(self.program.class(c).name)
    }

    /// Direct supertypes of `ty` (an Iface) with the arguments substituted.
    pub fn direct_supers(&self, ty: &Ty) -> Vec<Ty> {
        let Ty::Iface { class, args, .. } = ty else {
            return vec![];
        };
        let params = &self.class_params[class.0 as usize];
        let mut map = HashMap::new();
        for (p, a) in params.iter().zip(args.iter()) {
            map.insert(p.id, a.clone());
        }
        for p in params.iter().skip(args.len()) {
            map.insert(p.id, Ty::Dynamic);
        }
        self.class_supers[class.0 as usize]
            .iter()
            .map(|s| s.subst(&map))
            .collect()
    }

    /// `ty` seen as an instance of the class `target` (`List<int>` → `Iterable<int>`).
    ///
    /// The path through the hierarchy depends only on the two classes: the
    /// answer is computed once in terms of the parameters of `class` (memo
    /// `(class, target)`) and then instantiated with the arguments of `ty`.
    pub fn as_super(&self, ty: &Ty, target: ClassId) -> Option<Ty> {
        let resolved;
        let ty: &Ty = if matches!(ty, Ty::Param { .. }) {
            resolved = self.resolve_param_bound(ty);
            &resolved
        } else {
            ty
        };
        let Ty::Iface { class, args, .. } = ty else {
            return None;
        };
        if *class == target {
            return Some(ty.clone());
        }
        let generic = self.super_memo.borrow().get(&(*class, target)).cloned();
        let generic = match generic {
            Some(g) => g,
            None => {
                let g = self.as_super_generic(*class, target);
                self.super_memo
                    .borrow_mut()
                    .insert((*class, target), g.clone());
                g
            }
        };
        let g = generic?;
        let params = &self.class_params[class.0 as usize];
        if params.is_empty() {
            return Some(g);
        }
        let mut map = HashMap::new();
        for (p, a) in params.iter().zip(args.iter()) {
            map.insert(p.id, a.clone());
        }
        for p in params.iter().skip(args.len()) {
            map.insert(p.id, Ty::Dynamic);
        }
        Some(g.subst_prop(&map))
    }

    /// `target` seen from `class`, written in terms of the parameters of
    /// `class` (`List<E>` → `Iterable<E>`); `None` if it is not a supertype.
    fn as_super_generic(&self, class: ClassId, target: ClassId) -> Option<Ty> {
        let own = Ty::Iface {
            class,
            args: self.class_params[class.0 as usize]
                .iter()
                .map(|p| Ty::Param {
                    id: p.id,
                    name: p.name.clone(),
                    nullable: false,
                })
                .collect(),
            nullable: false,
        };
        let mut seen = HashSet::new();
        let mut queue = vec![own];
        while let Some(t) = queue.pop() {
            for s in self.direct_supers(&t) {
                if let Some(c) = s.class() {
                    if c == target {
                        return Some(s);
                    }
                    if seen.insert(c) {
                        queue.push(s);
                    }
                }
            }
        }
        if Some(target) == self.object {
            return self.object.map(Ty::iface);
        }
        None
    }

    pub fn is_subclass(&self, c: ClassId, target: ClassId) -> bool {
        if c == target {
            return true;
        }
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(x) = queue.pop() {
            for s in &self.class_supers[x.0 as usize] {
                if let Some(sc) = s.class() {
                    if sc == target {
                        return true;
                    }
                    if seen.insert(sc) {
                        queue.push(sc);
                    }
                }
            }
        }
        Some(target) == self.object
    }

    /// Superclass chain (extends) and mixins, from the class upwards.
    pub fn superclass_of(&self, c: ClassId) -> Option<ClassId> {
        // From the type outline (expands typedefs such as `IterableBase`), otherwise by name.
        if let Some(t) = self.outline.classes[c.0 as usize].supertype {
            if let Some(sc) = self.ty_of(t).class() {
                return Some(sc);
            }
        }
        self.program.class(c).supertype_class
    }

    /// Looks up an instance member declared in the class (not inherited).
    pub fn declared_member(&self, c: ClassId, name: &str, setter: bool) -> Option<MemberKind> {
        let class = self.program.class(c);
        let key = if setter {
            format!("{name}_=")
        } else {
            name.to_string()
        };
        let found = self
            .interner
            .lookup(&key)
            .and_then(|sym| class.instance_members.get(&sym).copied());
        let Some(fid) = found else {
            if setter {
                // A `late final` without an initializer accepts one assignment.
                let fsym = self.interner.lookup(name)?;
                let vid = class.fields.iter().copied().find(|v| {
                    let var = self.program.variable(*v);
                    var.name == fsym && !var.static_ && var.late
                })?;
                return Some(MemberKind::Field(vid));
            }
            return None;
        };
        let f = self.program.function(fid);
        if class.kind == ClassKind::Enum
            && name == "name"
            && matches!(f.node, dartforge_elements::model::FunctionRef::None)
        {
            return None;
        }
        Some(match f.kind {
            FunctionKind::ImplicitAccessor => {
                if setter {
                    MemberKind::Setter(fid)
                } else {
                    MemberKind::Field(f.variable?)
                }
            }
            FunctionKind::Getter => MemberKind::Getter(fid),
            FunctionKind::Setter => MemberKind::Setter(fid),
            _ => MemberKind::Method(fid),
        })
    }

    pub fn declared_static(&self, c: ClassId, name: &str, setter: bool) -> Option<MemberKind> {
        let class = self.program.class(c);
        let key = if setter {
            format!("{name}_=")
        } else {
            name.to_string()
        };
        let sym = self.interner.lookup(&key)?;
        let fid = *class.static_members.get(&sym)?;
        let f = self.program.function(fid);
        Some(match f.kind {
            FunctionKind::ImplicitAccessor => {
                if setter {
                    MemberKind::Setter(fid)
                } else {
                    MemberKind::Field(f.variable?)
                }
            }
            FunctionKind::Getter => MemberKind::Getter(fid),
            FunctionKind::Setter => MemberKind::Setter(fid),
            _ => MemberKind::Method(fid),
        })
    }

    /// Looks up an instance member in `ty` and its supertypes (BFS), with substitution.
    pub fn lookup_member(&self, ty: &Ty, name: &str, setter: bool) -> Option<Member> {
        // `resolve_param_bound` clones; only a `Ty::Param` needs it.
        let resolved;
        let ty: &Ty = if matches!(ty, Ty::Param { .. }) {
            resolved = self.resolve_param_bound(ty);
            &resolved
        } else {
            ty
        };
        let initial: Ty = match ty {
            Ty::Iface { class, .. } => Ty::iface(*class),
            Ty::Fn { .. } | Ty::Record { .. } | Ty::Never | Ty::Dynamic | Ty::Void => {
                if matches!(ty, Ty::Fn { .. }) && name == "call" {
                    return None;
                }
                self.object.map(Ty::iface)?
            }
            Ty::Null => self.null_.map(Ty::iface).or(self.object.map(Ty::iface))?,
            Ty::FutureOr { .. } => self.object.map(Ty::iface)?,
            Ty::Param { .. } => self.object.map(Ty::iface)?,
        };
        let initial_class = initial.class()?;
        // Consult the memo: the declaring class and the member kind are the
        // same for any instantiation of the initial class. When the declaring
        // class has no type parameters, the substitution is empty and the
        // result comes out without walking anything.
        let memo_key = self
            .interner
            .lookup(name)
            .map(|sym| (initial_class, sym, setter));
        if let Some(k) = memo_key {
            if let Some(found) = self.member_memo.borrow().get(&k) {
                match found {
                    None => return None,
                    Some((c, kind)) if self.class_params[c.0 as usize].is_empty() => {
                        return Some(Member {
                            class: *c,
                            kind: *kind,
                            subst: HashMap::new(),
                        });
                    }
                    Some(_) => {}
                }
            }
        }
        let store = |found: Option<(ClassId, MemberKind)>| {
            if let Some(k) = memo_key {
                self.member_memo.borrow_mut().insert(k, found);
            }
        };
        let start = match ty {
            Ty::Iface { .. } => ty.clone(),
            _ => initial,
        };
        let mut seen = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        while let Some(t) = queue.pop_front() {
            let Ty::Iface { class, args, .. } = &t else {
                continue;
            };
            if !seen.insert(*class) {
                continue;
            }
            if let Some(kind) = self.declared_member(*class, name, setter) {
                let params = &self.class_params[class.0 as usize];
                let mut subst = HashMap::new();
                for (p, a) in params.iter().zip(args.iter()) {
                    subst.insert(p.id, a.clone());
                }
                for p in params.iter().skip(args.len()) {
                    subst.insert(p.id, Ty::Dynamic);
                }
                store(Some((*class, kind)));
                return Some(Member {
                    class: *class,
                    kind,
                    subst,
                });
            }
            for s in self.direct_supers(&t) {
                queue.push_back(s);
            }
        }
        // Object as a last resort.
        if let Some(o) = self.object {
            if !seen.contains(&o) {
                if let Some(kind) = self.declared_member(o, name, setter) {
                    store(Some((o, kind)));
                    return Some(Member {
                        class: o,
                        kind,
                        subst: HashMap::new(),
                    });
                }
            }
        }
        store(None);
        None
    }

    /// Type of a member, already substituted: getter/field → value type; method → function type; setter → parameter type.
    pub fn member_ty(&self, m: &Member) -> Ty {
        let raw = match m.kind {
            MemberKind::Method(fid) => self.fn_ty(fid),
            MemberKind::Getter(fid) => {
                self.ty_of(self.outline.functions[fid.0 as usize].return_type)
            }
            MemberKind::Setter(fid) => self.outline.functions[fid.0 as usize]
                .parameters
                .first()
                .map(|p| self.ty_of(p.ty))
                .unwrap_or(Ty::Dynamic),
            MemberKind::Field(vid) => self.var_ty(vid),
        };
        // `subst` with an empty map clones the whole tree again; `raw` is
        // already ours (most members come from classes without type parameters).
        if m.subst.is_empty() {
            return raw;
        }
        raw.subst(&m.subst)
    }

    pub fn var_ty(&self, vid: VariableId) -> Ty {
        let v = &self.outline.variables[vid.0 as usize];
        let var = self.program.variable(vid);
        if let dartforge_elements::model::VariableRef::EnumConstant { .. } = var.node {
            if let Some(c) = var.class {
                return Ty::iface(c);
            }
        }
        match v.declared_type.or(v.inferred) {
            Some(t) => self.ty_of(t),
            None => Ty::Dynamic,
        }
    }

    /// Mixins of the class from the type outline (expands typedefs), otherwise by name.
    pub fn mixins_of(&self, c: ClassId) -> Vec<ClassId> {
        let data = &self.outline.classes[c.0 as usize];
        let mut out = Vec::new();
        for &t in data.mixins.iter() {
            if let Some(mc) = self.ty_of(t).class() {
                out.push(mc);
            }
        }
        if out.is_empty() {
            out = self.program.class(c).mixin_classes.clone();
        }
        out
    }

    /// Whether `c` has the constructor `name`: declared, or forwarded to the
    /// superclass by a named mixin application (`class D = B with M;`).
    pub fn has_constructor(&self, mut c: ClassId, name: SymbolId) -> bool {
        for _ in 0..64 {
            let class = self.program.class(c);
            if class.constructors.contains_key(&name) {
                return true;
            }
            if class.mixin_classes.is_empty() || !class.constructors.is_empty() {
                return false;
            }
            match class.supertype_class {
                Some(s) => c = s,
                None => return false,
            }
        }
        false
    }

    /// Whether some class of the program mixes `c` in.
    /// Whether every subtype of the program class `c` is a subclass through
    /// `extends` (no class implements it, mixes it in or has it as an `on`
    /// type), so that the JavaScript `instanceof` of its class is its `is`
    /// test: a user class (`class`, not a mixin, an enum or an extension
    /// type) without type parameters, emitted, not a JavaScript interop
    /// class.
    pub fn nominal_class(&self, c: ClassId) -> bool {
        let class = self.program.class(c);
        if class.kind != ClassKind::Class
            || self.libs[class.library.0 as usize].is_sdk
            || !self.class_params[c.0 as usize].is_empty()
            || self.js_classes.contains_key(&c)
            || self.native_set.contains(&c)
            || self.class_level(c) == crate::filter::Level::Dead
        {
            return false;
        }
        !self
            .implemented
            .get_or_init(|| {
                let mut out = HashSet::new();
                for i in 0..self.program.classes.len() as u32 {
                    let d = ClassId(i);
                    let mut chain = HashSet::new();
                    let mut k = Some(d);
                    while let Some(x) = k {
                        if !chain.insert(x) {
                            break;
                        }
                        k = self.superclass_of(x);
                    }
                    let mut seen = HashSet::new();
                    let mut queue = vec![d];
                    while let Some(x) = queue.pop() {
                        for t in &self.class_supers[x.0 as usize] {
                            if let Some(sc) = t.class() {
                                if !chain.contains(&sc) {
                                    out.insert(sc);
                                }
                                if seen.insert(sc) {
                                    queue.push(sc);
                                }
                            }
                        }
                    }
                }
                out
            })
            .contains(&c)
    }

    pub fn is_mixed_in(&self, c: ClassId) -> bool {
        self.mixed_in
            .get_or_init(|| {
                (0..self.program.classes.len() as u32)
                    .flat_map(|i| self.mixins_of(ClassId(i)))
                    .collect()
            })
            .contains(&c)
    }

    /// Whether `c` is in the prototype chain of a native peer: the native
    /// class itself, a superclass of one or a mixin applied in that chain.
    /// `registerExtension` hands the peer's signature table, whose
    /// prototype chain runs through these classes, to the JavaScript type,
    /// and dynamic dispatch on a native receiver looks members up by their
    /// `dartx` key (`_canonicalMember`).
    pub fn in_native_chain(&self, c: ClassId) -> bool {
        self.native_chain
            .get_or_init(|| {
                let mut out = HashSet::new();
                for &n in &self.native_set {
                    let mut k = Some(n);
                    while let Some(x) = k {
                        if !out.insert(x) {
                            break;
                        }
                        out.extend(self.mixins_of(x));
                        k = self.superclass_of(x);
                    }
                }
                out
            })
            .contains(&c)
    }

    /// The function of `dart:core` that a production SDK patch
    /// (`sdk_patches.rs`) provides for a call of `fid` with `positional`
    /// positional arguments and nothing else: `Object.hash` with `n`
    /// arguments is `_hashN`, which does what `hash` does when the arguments
    /// after the `n`-th are left at their sentinel default.
    pub fn arity_specialization(
        &self,
        fid: FunctionElementId,
        positional: usize,
    ) -> Option<FunctionElementId> {
        crate::arity_specialization(self.program, self.interner, fid, positional)
    }

    /// The production SDK patch that gives `dart:_runtime` its `strNum`
    /// (`sdk_patches.rs`) is applied.
    pub fn str_num_patch(&self) -> bool {
        self.runtime_declares("strNum")
    }

    /// `dart:_runtime` declares `name` (a production SDK patch's helper).
    pub fn runtime_declares(&self, name: &str) -> bool {
        let Some(rt) = self.sdk.as_ref().and_then(|s| s.runtime) else {
            return false;
        };
        self.interner.lookup(name).is_some_and(|sym| {
            self.program
                .library(rt)
                .declared
                .get(&sym)
                .is_some_and(|b| b.getter.is_some())
        })
    }

    /// `print(x)` → `_printLine` ([`crate::print_line_specialization`]).
    pub fn print_line_specialization(
        &self,
        fid: FunctionElementId,
        positional: usize,
    ) -> Option<FunctionElementId> {
        crate::print_line_specialization(self.program, self.interner, fid, positional)
    }

    /// Interfaces of the class from the outline.
    pub fn interfaces_of(&self, c: ClassId) -> Vec<ClassId> {
        let data = &self.outline.classes[c.0 as usize];
        let mut out = Vec::new();
        for &t in data.interfaces.iter() {
            if let Some(ic) = self.ty_of(t).class() {
                out.push(ic);
            }
        }
        if out.is_empty() {
            out = self.program.class(c).interface_classes.clone();
        }
        out
    }

    /// The class (or a superclass) is generic: the constructor receives `_ti`.
    ///
    /// In the production profile only a class with type parameters of its own
    /// receives it: the rti of a class without them is closed (`C`, even
    /// under `extends S<int>`), and `instanceType` finds it through the
    /// constructor; the generic superclass's constructor gets `null` and
    /// stores it in `$ti` (`docs/JS-PRODUCAO-TAMANHO.md` §13.7).
    /// `DARTFORGE_JSPROD_CLOSED_TI=0` goes back to passing it.
    pub fn requires_rti(&self, c: ClassId) -> bool {
        if self.js_classes.contains_key(&c) {
            return false;
        }
        if self.production()
            && self.class_params[c.0 as usize].is_empty()
            && std::env::var("DARTFORGE_JSPROD_CLOSED_TI").map_or(true, |v| v != "0")
        {
            return false;
        }
        let mut cur = Some(c);
        let mut guard = 0;
        while let Some(k) = cur {
            if !self.class_params[k.0 as usize].is_empty() {
                return true;
            }
            cur = self.superclass_of(k);
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        false
    }

    /// Public forwarded member in a native class: looks in the class and its
    /// superclasses (`getDispatchTarget`); `(function, is field)`.
    pub fn native_forwarded_member(
        &self,
        c: ClassId,
        name: &str,
        setter: bool,
    ) -> Option<(FunctionElementId, bool)> {
        let mut cur = Some(c);
        let mut guard = 0;
        while let Some(k) = cur {
            let class = self.program.class(k);
            let key = if setter {
                format!("{name}_=")
            } else {
                name.to_string()
            };
            if let Some(sym) = self.interner.lookup(&key) {
                if let Some(&fid) = class.instance_members.get(&sym) {
                    let f = self.program.function(fid);
                    if !f.abstract_ {
                        return Some((fid, f.kind == FunctionKind::ImplicitAccessor));
                    }
                }
            }
            for &mx in &class.mixin_classes {
                if let Some(r) = self.native_forwarded_member(mx, name, setter) {
                    return Some(r);
                }
            }
            cur = class.supertype_class;
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        None
    }

    /// The function has a `native;` body (equivalent to `external` in the SDK).
    pub fn has_native_body(&self, fid: FunctionElementId) -> bool {
        match self.program.function(fid).node {
            dartforge_elements::model::FunctionRef::Function { unit, function } => {
                matches!(
                    self.program.unit(unit).ast.function(function).body,
                    ast::FunctionBody::Native(_)
                )
            }
            _ => false,
        }
    }

    /// Web library of the SDK (the DDC's `_isWebLibrary`).
    pub fn is_web_library(&self, lib: LibraryId) -> bool {
        matches!(
            self.program.library(lib).uri.as_str(),
            "dart:html"
                | "dart:svg"
                | "dart:indexed_db"
                | "dart:web_audio"
                | "dart:web_gl"
                | "dart:web_sql"
                | "dart:html_common"
        )
    }

    /// Concrete (non-abstract) member in the chain of superclasses and mixins.
    pub fn has_concrete_member(&self, c: ClassId, name: &str, setter: bool) -> bool {
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(k) = queue.pop() {
            if !seen.insert(k) {
                continue;
            }
            if let Some(mk) = self.declared_member(k, name, setter) {
                let abstract_ = match mk {
                    MemberKind::Method(f) | MemberKind::Getter(f) | MemberKind::Setter(f) => {
                        self.program.function(f).abstract_
                    }
                    MemberKind::Field(v) => self.program.variable(v).external && false,
                };
                if !abstract_ {
                    return true;
                }
            }
            let class = self.program.class(k);
            queue.extend(class.mixin_classes.iter().copied());
            if let Some(s) = class.supertype_class {
                queue.push(s);
            }
        }
        false
    }

    /// The class (itself or an ancestor, not counting Object) declares `noSuchMethod`.
    pub fn has_user_nsm(&self, c: ClassId) -> bool {
        let mut cur = Some(c);
        while let Some(k) = cur {
            if Some(k) == self.object {
                return false;
            }
            if self.declared_member(k, "noSuchMethod", false).is_some() {
                return true;
            }
            let class = self.program.class(k);
            for &mx in &class.mixin_classes {
                if self.declared_member(mx, "noSuchMethod", false).is_some() {
                    return true;
                }
            }
            cur = class.supertype_class;
        }
        false
    }

    /// Instance members declared in `k`, in declaration order (the function
    /// id is assigned by the outline in source order). The `instance_members`
    /// map is a `HashMap`: iterating it directly gives an order that changes
    /// from one run to the next.
    fn declared_members(&self, k: ClassId) -> Vec<(SymbolId, FunctionElementId)> {
        let mut v: Vec<(SymbolId, FunctionElementId)> = self
            .program
            .class(k)
            .instance_members
            .iter()
            .map(|(&s, &f)| (s, f))
            .collect();
        v.sort_by_key(|&(_, f)| f);
        v
    }

    /// Order of the instance member names of `c` as the CFE builds the
    /// `memberMap` of `ClassMembersNodeBuilder.build` (`members_node.dart`):
    /// the declared ones (getters and methods, then setters), the
    /// superclass's, the mixins' (an **abstract** mixin member does not enter
    /// as a `mixedInMember`, but through the interface of the application
    /// `S with M`, after `S`) and the interfaces', each name the first time it
    /// appears. The `noSuchMethod` forwarders are born in this order
    /// (`registerMemberComputation` → `ForwardingNode.finalize` →
    /// `cls.addProcedure`) and the DDC emits them in the order of
    /// `cls.procedures`. In deep hierarchies the CFE also separates class
    /// members from interface-only ones (`classMemberMap` before
    /// `interfaceMemberMap`); here the order is that of the depth-first
    /// search — deterministic in both cases.
    fn cfe_member_order(&self, c: ClassId) -> Vec<String> {
        fn visit(ctx: &Ctx, k: ClassId, seen: &mut HashSet<ClassId>, out: &mut Vec<String>) {
            if Some(k) == ctx.object || !seen.insert(k) {
                return;
            }
            let members = ctx.declared_members(k);
            for setters in [false, true] {
                for &(sym, _) in &members {
                    let key = ctx.interner.resolve(sym);
                    let (name, is_setter) = match key.strip_suffix("_=") {
                        Some(n) => (n, true),
                        None => (key, false),
                    };
                    if is_setter == setters && !out.iter().any(|x| x == name) {
                        out.push(name.to_string());
                    }
                }
            }
            let class = ctx.program.class(k);
            if let Some(s) = class.supertype_class {
                visit(ctx, s, seen, out);
            }
            for &mx in &class.mixin_classes {
                visit(ctx, mx, seen, out);
            }
            for &i in class
                .interface_classes
                .iter()
                .chain(class.on_classes.iter())
            {
                visit(ctx, i, seen, out);
            }
        }
        let mut out = Vec::new();
        visit(self, c, &mut HashSet::new(), &mut out);
        out
    }

    /// Abstract members (name, kind) reachable through the supertypes that
    /// have no concrete implementation, in the order in which the CFE creates
    /// the `noSuchMethod` forwarders (`Self::cfe_member_order`; a getter
    /// before the setter of the same name).
    pub fn unimplemented_abstract(&self, c: ClassId) -> Vec<(String, MemberKind)> {
        let mut out: Vec<(String, MemberKind)> = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = vec![c];
        while let Some(k) = queue.pop() {
            if !seen.insert(k) || Some(k) == self.object {
                continue;
            }
            for (sym, fid) in self.declared_members(k) {
                let f = self.program.function(fid);
                // A member of an implemented program class is part of the
                // interface even when it is concrete there; what `c` inherits
                // as an implementation is excluded below
                // (`has_concrete_member`). Concrete SDK members keep the SDK's
                // own forwarding (their signatures use the SDK class's type
                // parameters, out of scope here).
                if !f.abstract_ && (k == c || self.libs[self.lib_of_class(k).0 as usize].is_sdk) {
                    continue;
                }
                let key = self.interner.resolve(sym).to_string();
                let (name, setter) = match key.strip_suffix("_=") {
                    Some(n) => (n.to_string(), true),
                    None => (key.clone(), false),
                };
                if out
                    .iter()
                    .any(|(n, mk)| *n == name && matches!(mk, MemberKind::Setter(_)) == setter)
                {
                    continue;
                }
                if self.has_concrete_member(c, &name, setter) {
                    continue;
                }
                let mk = match f.kind {
                    FunctionKind::Getter => MemberKind::Getter(fid),
                    FunctionKind::Setter => MemberKind::Setter(fid),
                    FunctionKind::ImplicitAccessor => {
                        if setter {
                            MemberKind::Setter(fid)
                        } else {
                            MemberKind::Getter(fid)
                        }
                    }
                    _ => MemberKind::Method(fid),
                };
                out.push((name, mk));
            }
            for s in &self.class_supers[k.0 as usize] {
                if let Some(sc) = s.class() {
                    queue.push(sc);
                }
            }
        }
        if out.len() > 1 {
            let order = self.cfe_member_order(c);
            let pos = |n: &str| order.iter().position(|x| x == n).unwrap_or(usize::MAX);
            out.sort_by(|(na, ka), (nb, kb)| {
                let (sa, sb) = (
                    matches!(ka, MemberKind::Setter(_)),
                    matches!(kb, MemberKind::Setter(_)),
                );
                pos(na)
                    .cmp(&pos(nb))
                    .then_with(|| na.cmp(nb))
                    .then(sa.cmp(&sb))
            });
        }
        out
    }

    /// The function type every instance method named `name` has, in the whole
    /// program, when there is exactly one and it is closed and not generic:
    /// then the tear-off of any `name` method has that runtime type, whatever
    /// the receiver's class (an override with another type, an explicitly
    /// `covariant` parameter, whose forwarding stubs have another type, a
    /// getter or a field of the same name anywhere give `None`).
    pub fn single_method_type(&self, name: dartforge_intern::SymbolId) -> Option<Ty> {
        let mut memo = self.single_method_types.borrow_mut();
        let map = memo.get_or_insert_with(|| {
            let mut map: HashMap<dartforge_intern::SymbolId, Option<Ty>> = HashMap::new();
            for (i, f) in self.program.functions.iter().enumerate() {
                if f.class.is_none() || f.static_ || f.extension.is_some() {
                    continue;
                }
                if matches!(
                    f.kind,
                    FunctionKind::Constructor | FunctionKind::SyntheticConstructor
                ) {
                    continue;
                }
                // Setters live under `name=`: only their getter twin clashes.
                let key = match f.kind {
                    FunctionKind::Setter => continue,
                    _ => f.name,
                };
                // An explicitly `covariant` parameter makes the emitter write
                // forwarding stubs whose type has `Object?` there.
                let covariant = crate::own_sdk::ast_params(self, FunctionElementId(i as u32))
                    .iter()
                    .any(|p| p.covariant);
                let ty = if f.kind == FunctionKind::Function && !covariant {
                    Some(self.fn_ty(FunctionElementId(i as u32)))
                        .filter(|t| t.has_no_params())
                        .filter(
                            |t| matches!(t, Ty::Fn { type_params, .. } if type_params.is_empty()),
                        )
                } else {
                    None
                };
                match map.get(&key) {
                    None => {
                        map.insert(key, ty);
                    }
                    Some(prev) if prev.as_ref() != ty.as_ref() => {
                        map.insert(key, None);
                    }
                    _ => {}
                }
            }
            for v in self.program.variables.iter() {
                if v.class.is_some() && !v.static_ && v.extension.is_none() {
                    map.insert(v.name, None);
                }
            }
            map
        });
        map.get(&name).cloned().flatten()
    }

    pub fn fn_ty(&self, fid: FunctionElementId) -> Ty {
        let data = &self.outline.functions[fid.0 as usize];
        let f = self.program.function(fid);
        if f.kind == FunctionKind::ImplicitAccessor {
            return self.var_ty(f.variable.expect("implicit accessor without a variable"));
        }
        // Rebuild from the parameters (does the constructors' `signature` ignore optional named ones? use it anyway).
        let mut pos = Vec::new();
        let mut opt = Vec::new();
        let mut named = Vec::new();
        for p in data.parameters.iter() {
            let t = self.ty_of(p.ty);
            match p.kind {
                ast::ParameterKind::Required => pos.push(t),
                ast::ParameterKind::Optional => opt.push(t),
                ast::ParameterKind::Named => {
                    let n = p
                        .external_name
                        .or(p.name)
                        .map(|s| self.interner.resolve(s).to_string())
                        .unwrap_or_default();
                    named.push((n, t, p.required));
                }
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        Ty::Fn {
            type_params: data
                .type_params
                .iter()
                .map(|p| self.ty_param_of(*p))
                .collect(),
            ret: Box::new(self.ty_of(data.return_type)),
            pos,
            opt,
            named,
            nullable: false,
        }
    }

    /// `C<dynamic, …>`: the class as a type literal without arguments.
    /// The instantiation to bounds of the type parameters `params` (id and
    /// bound): no bound (or a top one) is `dynamic`; a bound, with the
    /// parameters already instantiated substituted, in dependency order;
    /// those left in a bound cycle get the bound with the cycle's parameters
    /// as `dynamic` (the VM's `C` for `class C<T extends num>` is `C<num>`).
    pub fn bounds_instantiation(&self, params: &[(u32, Ty)]) -> Vec<Ty> {
        let top = |t: &Ty| match t {
            Ty::Dynamic | Ty::Void => true,
            Ty::Iface {
                class,
                nullable: true,
                ..
            } => Some(*class) == self.object,
            _ => false,
        };
        let ids: Vec<u32> = params.iter().map(|(id, _)| *id).collect();
        let mut done: HashMap<u32, Ty> = HashMap::new();
        for (id, b) in params {
            if top(b) {
                done.insert(*id, Ty::Dynamic);
            }
        }
        loop {
            let mut progress = false;
            for (id, b) in params {
                if done.contains_key(id) {
                    continue;
                }
                let mut used = Vec::new();
                b.collect_params(&mut used);
                if used
                    .iter()
                    .all(|u| !ids.contains(u) || done.contains_key(u))
                {
                    done.insert(*id, b.clone().subst_prop(&done));
                    progress = true;
                }
            }
            if !progress {
                break;
            }
        }
        let mut cycle = done.clone();
        for id in &ids {
            cycle.entry(*id).or_insert(Ty::Dynamic);
        }
        params
            .iter()
            .map(|(id, b)| match done.get(id) {
                Some(t) => t.clone(),
                None => b.clone().subst_prop(&cycle),
            })
            .collect()
    }

    pub fn this_ty_default(&self, c: ClassId) -> Ty {
        let args = self.class_params[c.0 as usize]
            .iter()
            .map(|_| Ty::Dynamic)
            .collect();
        Ty::Iface {
            class: c,
            args,
            nullable: false,
        }
    }

    /// The `this` type of a class: `C<T1..Tn>`.
    pub fn this_ty(&self, c: ClassId) -> Ty {
        let args = self.class_params[c.0 as usize]
            .iter()
            .map(|p| Ty::Param {
                id: p.id,
                name: p.name.clone(),
                nullable: false,
            })
            .collect();
        Ty::Iface {
            class: c,
            args,
            nullable: false,
        }
    }

    /// Whether the member seen from `ty` is accessed through `dartx` (decided by the class that declares it).
    pub fn is_ext_member(&self, ty: &Ty, name: &str, setter: bool) -> bool {
        if name.starts_with('_') {
            return false;
        }
        let t = self.resolve_param_bound(ty);
        let class = match &t {
            Ty::Iface { class, .. } => *class,
            Ty::Fn { .. } => match self.function_ {
                Some(f) => f,
                None => return false,
            },
            Ty::Null => match self.null_ {
                Some(n) => n,
                None => return false,
            },
            _ => return false,
        };
        // Receiver of a native type (`_isSymbolizedMember`, compiler.dart:3311):
        // a (public) forwarded member that is a field or `external`/`native`
        // accesses the JS property directly, except (a) when it is `external`
        // in a web library with a non-nullable return — it needs the
        // definition's `checkNativeNonNull` — and (b) when `@JSName('x')`
        // renames it (`x != name`), because then the Dart name does not exist
        // on the JS object. Members with a Dart body are always `dartx` symbols.
        let impl_class = if Some(class) == self.int_
            || Some(class) == self.double_
            || Some(class) == self.num_
        {
            self.jsnumber
        } else if Some(class) == self.bool_ {
            self.jsbool
        } else if Some(class) == self.string_ {
            self.jsstring
        } else {
            None
        };
        let native_class = impl_class.or_else(|| self.native_set.contains(&class).then_some(class));
        if let Some(nc) = native_class {
            return match self.native_forwarded_member(nc, name, setter) {
                Some((fid, is_field)) => {
                    let f = self.program.function(fid);
                    let external = f.external || self.has_native_body(fid);
                    if !is_field && !external {
                        return true; // Dart body: always symbolized
                    }
                    // `_isNullCheckableNative`: only `external` procedures of a
                    // web library with a potentially non-nullable return.
                    if !is_field && external && self.is_web_library(f.library) {
                        let ret = self.ty_of(self.outline.functions[fid.0 as usize].return_type);
                        if !ret.is_nullable() {
                            return true;
                        }
                    }
                    self.js_names.get(&fid.0).is_some_and(|j| j != name)
                }
                None => true,
            };
        }
        match self.lookup_member(&t, name, setter) {
            Some(m) => self.ext_set.contains(&m.class),
            None => self.ext_set.contains(&class),
        }
    }

    // -----------------------------------------------------------------------
    // Approximate subtyping and LUB
    // -----------------------------------------------------------------------

    pub fn is_subtype(&self, a: &Ty, b: &Ty) -> bool {
        self.is_subtype_depth(a, b, 0)
    }

    fn is_subtype_depth(&self, a: &Ty, b: &Ty, depth: u32) -> bool {
        if depth > 16 {
            return true;
        }
        if a == b {
            return true;
        }
        match (a, b) {
            (_, Ty::Dynamic) | (_, Ty::Void) => true,
            (Ty::Dynamic, _) => true,
            (Ty::Never, _) => true,
            (
                _,
                Ty::Iface {
                    class, nullable, ..
                },
            ) if Some(*class) == self.object => *nullable || !a.is_nullable(),
            (Ty::Null, b) => b.is_nullable(),
            (a, Ty::FutureOr { arg, nullable }) => {
                if a.is_nullable() && !*nullable {
                    return false;
                }
                let an = a.non_null();
                self.is_subtype_depth(&an, arg, depth + 1)
                    || self.is_subtype_depth(&an, &self.t_future((**arg).clone()), depth + 1)
            }
            (Ty::FutureOr { arg, nullable }, b) => {
                if *nullable && !b.is_nullable() {
                    return false;
                }
                self.is_subtype_depth(arg, b, depth + 1)
                    && self.is_subtype_depth(&self.t_future((**arg).clone()), b, depth + 1)
            }
            (Ty::Param { id, nullable, .. }, b) => {
                if let Ty::Param {
                    id: id2,
                    nullable: n2,
                    ..
                } = b
                {
                    if id == id2 {
                        return !*nullable || *n2;
                    }
                }
                if *nullable && !b.is_nullable() {
                    return false;
                }
                let bound = self.bound_of(*id);
                self.is_subtype_depth(&bound, b, depth + 1)
            }
            (_, Ty::Param { .. }) => false,
            (
                Ty::Iface {
                    class, nullable, ..
                },
                Ty::Iface {
                    class: c2,
                    args: args2,
                    nullable: n2,
                },
            ) => {
                if *nullable && !*n2 {
                    return false;
                }
                let Some(sup) = self.as_super(&a.non_null(), *c2) else {
                    return false;
                };
                let _ = class;
                let sargs = sup.args();
                if sargs.len() != args2.len() {
                    return true;
                }
                sargs
                    .iter()
                    .zip(args2.iter())
                    .all(|(x, y)| self.is_subtype_depth(x, y, depth + 1))
            }
            (
                Ty::Iface {
                    class, nullable, ..
                },
                Ty::Fn { nullable: n2, .. },
            ) => {
                let _ = (class, nullable, n2);
                false
            }
            (
                Ty::Fn { nullable, .. },
                Ty::Iface {
                    class,
                    nullable: n2,
                    ..
                },
            ) => (!*nullable || *n2) && Some(*class) == self.function_,
            (
                Ty::Fn {
                    ret,
                    pos,
                    opt,
                    named,
                    nullable,
                    ..
                },
                Ty::Fn {
                    ret: r2,
                    pos: p2,
                    opt: o2,
                    named: n2,
                    nullable: nn2,
                    ..
                },
            ) => {
                if *nullable && !*nn2 {
                    return false;
                }
                if !self.is_subtype_depth(ret, r2, depth + 1) {
                    return false;
                }
                if pos.len() > p2.len() || pos.len() + opt.len() < p2.len() + o2.len() {
                    return false;
                }
                for (i, t2) in p2.iter().chain(o2.iter()).enumerate() {
                    let t1 = if i < pos.len() {
                        &pos[i]
                    } else {
                        &opt[i - pos.len()]
                    };
                    if !self.is_subtype_depth(t2, t1, depth + 1) {
                        return false;
                    }
                }
                for (n, t2, _) in n2 {
                    match named.iter().find(|(m, _, _)| m == n) {
                        Some((_, t1, _)) => {
                            if !self.is_subtype_depth(t2, t1, depth + 1) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            }
            (
                Ty::Record {
                    pos,
                    named,
                    nullable,
                },
                Ty::Record {
                    pos: p2,
                    named: n2,
                    nullable: nn2,
                },
            ) => {
                (!*nullable || *nn2)
                    && pos.len() == p2.len()
                    && named.len() == n2.len()
                    && pos
                        .iter()
                        .zip(p2)
                        .all(|(x, y)| self.is_subtype_depth(x, y, depth + 1))
                    && named
                        .iter()
                        .zip(n2)
                        .all(|((a, x), (b, y))| a == b && self.is_subtype_depth(x, y, depth + 1))
            }
            (
                Ty::Record { nullable, .. },
                Ty::Iface {
                    class,
                    nullable: n2,
                    ..
                },
            ) => (!*nullable || *n2) && Some(*class) == self.record_,
            _ => false,
        }
    }

    /// Least upper bound (approximate).
    pub fn lub(&self, a: &Ty, b: &Ty) -> Ty {
        if a == b {
            return a.clone();
        }
        match (a, b) {
            (Ty::Never, x) | (x, Ty::Never) => x.clone(),
            (Ty::Null, x) | (x, Ty::Null) => x.with_nullable(true),
            (Ty::Dynamic, _) | (_, Ty::Dynamic) => Ty::Dynamic,
            (Ty::Void, _) | (_, Ty::Void) => Ty::Void,
            _ => {
                let nullable = a.is_nullable() || b.is_nullable();
                let an = a.non_null();
                let bn = b.non_null();
                if self.is_subtype(&an, &bn) {
                    return bn.with_nullable(nullable);
                }
                if self.is_subtype(&bn, &an) {
                    return an.with_nullable(nullable);
                }
                // int/double → num
                if self.is_num_like(&an) && self.is_num_like(&bn) {
                    return self.t_num().with_nullable(nullable);
                }
                // Same class: lub of the arguments.
                if let (
                    Ty::Iface {
                        class: ca,
                        args: aa,
                        ..
                    },
                    Ty::Iface {
                        class: cb,
                        args: ab,
                        ..
                    },
                ) = (&an, &bn)
                {
                    if ca == cb && aa.len() == ab.len() {
                        let args = aa
                            .iter()
                            .zip(ab.iter())
                            .map(|(x, y)| self.lub(x, y))
                            .collect();
                        return Ty::Iface {
                            class: *ca,
                            args,
                            nullable,
                        };
                    }
                }
                // Look for a common superclass in the `extends` chain of `a`.
                if let (Ty::Iface { .. }, Ty::Iface { .. }) = (&an, &bn) {
                    let mut cur = Some(an.clone());
                    while let Some(t) = cur {
                        if self.is_subtype(&bn, &t) {
                            return t.with_nullable(nullable);
                        }
                        let supers = self.direct_supers(&t);
                        cur = supers.first().cloned();
                    }
                }
                if let (Ty::Fn { .. }, Ty::Fn { .. }) = (&an, &bn) {
                    return self
                        .function_
                        .map(Ty::iface)
                        .unwrap_or(Ty::Dynamic)
                        .with_nullable(nullable);
                }
                self.t_object().with_nullable(nullable)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Names
    // -----------------------------------------------------------------------

    pub fn lib_of_class(&self, c: ClassId) -> LibraryId {
        self.program.class(c).library
    }

    pub fn lib_ident(&self, lib: LibraryId) -> &str {
        &self.libs[lib.0 as usize].ident
    }

    /// The JS key of a named parameter: the Dart name, or the short one of the
    /// production profile (`Liveness::named_key_of`). The same map applies at the
    /// call, in the function's prologue and in the rti recipe of the function
    /// type.
    pub fn named_key(&self, name: &str) -> String {
        self.filter
            .and_then(|f| f.named_key_of(name))
            .unwrap_or(name)
            .to_string()
    }

    /// rti recipe prefix of the class: `core|int`, `main|P`. In SDK mode
    /// (production profile), the program's library is a short label
    /// ([`crate::recipe_tag`]): the part before the `|` is opaque to the
    /// runtime (`_rtiToString` only reads what comes after it), and the full
    /// library name repeated in every recipe was ~20% of the file
    /// (`docs/JS-PRODUCAO-TAMANHO.md` §3.2).
    ///
    /// In SDK mode the recipe is a short id (`|a`, `|bK`,
    /// `docs/JS-PRODUCAO-TAMANHO.md` §9.5): the leading `|` is an identifier
    /// for the recipe *parser* (`Recipe.isIdentifierStart`) and marks the
    /// recipe for the verifier and the rules (`emit_js_production::regras`);
    /// the name that `_rtiToString` shows comes from the table the production
    /// profile puts into `unmangleGlobalNameIfPreservedAnyways`.
    /// The name of the type variable `name` of class `c` in recipes and in
    /// the rti rules (`List.E`). With the short class ids of the production
    /// profile it is the class's id (`|h.E`): the recipe parser reads any
    /// identifier with a period as a type variable, and only the rules give
    /// it meaning. `DARTFORGE_JSPROD_SHORT_TYPE_VARIABLES=0` keeps the class
    /// name.
    ///
    /// With [`Ctx::positional_type_variables`] the variable is named by its
    /// position among the class's type parameters (`|h.0`), and the rules
    /// carry no substitution entries: the patched `evalTypeVariable` derives
    /// them (`sdk_patches.rs`).
    pub fn type_var_key(&self, c: ClassId, name: &str) -> String {
        if self.positional_type_variables()
            && let Some(i) = self.class_params[c.0 as usize]
                .iter()
                .position(|p| p.name == name)
        {
            return format!("{}.{i}", self.class_recipe(c));
        }
        if self.sdk.is_some()
            && self.short_recipe
            && std::env::var("DARTFORGE_JSPROD_SHORT_TYPE_VARIABLES").map_or(true, |v| v != "0")
        {
            return format!("{}.{name}", self.class_recipe(c));
        }
        format!("{}.{name}", self.class_name(c))
    }

    /// Whether class type variables are named by position in recipes and
    /// left out of the rti rules (production, SDK mode). The rules list
    /// every transitive supertype with its type arguments in the class's
    /// environment, and a class's own parameter `i` is always the recipe
    /// `i + 1` there (`module.rs::emit_rules_with`), so the substitution
    /// entries (`"|h.E": "1"`, `"|c.E": "1"` next to `"|c": ["1"]`) repeat
    /// what the patched `_Universe.evalTypeVariable` derives from the name:
    /// `C.k` is `k + 1` in an environment of class `C` and the `k`-th
    /// argument of the `C` supertype entry otherwise. Requires the patch
    /// (its `_positionalTypeVariable` must exist in `dart:_rti`).
    /// `DARTFORGE_JSPROD_POSITIONAL_TYPE_VARIABLES=0` keeps the names.
    pub fn positional_type_variables(&self) -> bool {
        *self.positional_type_variables.get_or_init(|| {
            self.sdk.is_some()
                && std::env::var("DARTFORGE_JSPROD_POSITIONAL_TYPE_VARIABLES")
                    .map_or(true, |v| v != "0")
                && self.program.libraries.iter().any(|l| {
                    l.uri == "dart:_rti"
                        && self
                            .interner
                            .lookup("_positionalTypeVariable")
                            .is_some_and(|n| l.declared.contains_key(&n))
                })
        })
    }

    pub fn class_recipe(&self, c: ClassId) -> String {
        let lib = self.lib_of_class(c);
        if self.sdk.is_some() && self.short_recipe {
            if let Some(ids) = self.filter.and_then(|f| f.recipe_ids()) {
                return format!("|{}", ids[c.0 as usize]);
            }
            let ids = self
                .recipe_ids
                .get_or_init(|| crate::recipe_ids(self.program, self.interner));
            return format!("|{}", ids[c.0 as usize]);
        }
        if self.sdk.is_some() && !self.libs[lib.0 as usize].is_sdk {
            return format!("{}|{}", crate::recipe_tag(lib), self.class_name(c));
        }
        format!("{}|{}", self.lib_ident(lib), self.class_name(c))
    }

    /// [`Ctx::class_recipe`] of the class `name` of `uri`; `fallback` if it
    /// does not exist in the program.
    pub fn recipe_of(&self, uri: &str, name: &str, fallback: &str) -> String {
        let class = self
            .program
            .libraries
            .iter()
            .position(|l| l.uri == uri)
            .and_then(|i| {
                let sym = self.interner.lookup(name)?;
                match self
                    .program
                    .library(LibraryId(i as u32))
                    .declared
                    .get(&sym)?
                    .getter
                {
                    Some(Element::Class(c)) => Some(c),
                    _ => None,
                }
            });
        class.map_or_else(|| fallback.to_string(), |c| self.class_recipe(c))
    }

    /// How many type parameters the class declares.
    ///
    /// Uses `class_params` when it already exists and the outline while
    /// `compute_hierarchy` is still building it.
    pub fn class_arity(&self, c: ClassId) -> usize {
        match self.class_params.get(c.0 as usize) {
            Some(p) => p.len(),
            None => self
                .outline
                .classes
                .get(c.0 as usize)
                .map_or(0, |k| k.type_params.len()),
        }
    }

    /// Whether `e` (of the unit `unit`) is the target of a compound assignment
    /// (`+=`, `??=`…) or of `++`/`--`. The common inference records the read
    /// type of these targets on the assignment node and not on the target
    /// (like the analyzer, which gives `readType` to the
    /// `AssignmentExpression`), so the static type of the target does not
    /// count as the read type. Computed once per unit.
    pub fn is_write_target(&self, unit: dartforge_elements::model::UnitId, e: ast::ExprId) -> bool {
        let targets = self
            .write_targets
            .borrow_mut()
            .entry(unit.0)
            .or_insert_with(|| {
                let mut v = HashSet::new();
                for x in &self.program.unit(unit).ast.exprs {
                    match &x.kind {
                        ast::ExprKind::Assign { op, target, .. }
                            if *op != ast::AssignOp::Assign =>
                        {
                            v.insert(target.0);
                        }
                        ast::ExprKind::Unary {
                            op:
                                ast::UnaryOp::PrefixInc
                                | ast::UnaryOp::PrefixDec
                                | ast::UnaryOp::PostfixInc
                                | ast::UnaryOp::PostfixDec,
                            operand,
                        } => {
                            v.insert(operand.0);
                        }
                        _ => {}
                    }
                }
                std::rc::Rc::new(v)
            })
            .clone();
        targets.contains(&e.0)
    }

    /// Arguments of an interface at the declared arity: a **raw** type
    /// (`Caixa` meaning `Caixa<dynamic>`) receives one `dynamic` per missing
    /// parameter.
    ///
    /// The `rti` requires one argument per declared parameter —
    /// `_areArgumentsSubtypes` (`rti.dart:3676`) has
    /// `assert(length == _Utils.arrayLength(tArgs))`, and a raw recipe brings
    /// the application down at the first `is`/`as` that goes through it.
    /// `dartdevc` emits `raw|Caixa<@>` and never `raw|Caixa` (the name without
    /// arguments only appears as a **key** of `addRules`/`addRtiResources`,
    /// which is something else).
    pub fn args_at_arity<'t>(&self, class: ClassId, args: &'t [Ty]) -> std::borrow::Cow<'t, [Ty]> {
        let n = self.class_arity(class);
        if args.len() >= n {
            return std::borrow::Cow::Borrowed(args);
        }
        let mut v = args.to_vec();
        v.resize(n, Ty::Dynamic);
        std::borrow::Cow::Owned(v)
    }

    pub fn sym(&self, name: &str) -> Option<SymbolId> {
        self.interner.lookup(name)
    }

    pub fn name(&self, s: SymbolId) -> &str {
        self.interner.resolve(s)
    }

    pub fn unit(&self, u: UnitId) -> &'a dartforge_elements::model::Unit {
        self.program.unit(u)
    }

    /// Extensions visible in a library: those declared in it and in the
    /// imported ones (and in `dart:core`).
    pub fn visible_extensions(&self, lib: LibraryId) -> Vec<ExtensionId> {
        if let Some(v) = self.extension_memo.borrow().get(&lib) {
            return v.clone();
        }
        // Those the common inference sees: declared in the library or brought
        // by a non-deferred import, after `show`/`hide`.
        let v = dartforge_types::inference::accessible_extensions_of(self.program, lib);
        self.extension_memo.borrow_mut().insert(lib, v.clone());
        v
    }
}

/// Derives the identifiers and module paths of the libraries.
fn compute_libs(program: &Program) -> Vec<LibInfo> {
    // Directory of the entry point, for relative paths.
    let entry_dir: Option<String> = program.entry.and_then(|e| {
        let lib = program.library(e);
        let uri = &lib.uri;
        uri.strip_prefix("file:///").map(|p| {
            let p = p.replace('\\', "/");
            match p.rfind('/') {
                Some(i) => p[..=i].to_string(),
                None => String::new(),
            }
        })
    });
    let mut used: HashSet<String> = HashSet::new();
    let mut infos = Vec::with_capacity(program.libraries.len());
    for lib in &program.libraries {
        let uri = &lib.uri;
        let info = if let Some(name) = uri.strip_prefix("dart:") {
            // The runtime is the `dart` object and `dart:_rti` is `dart_rti`
            // (the names of the DDC contract); the rti recipe still goes by the
            // path (`_rti|Rti`), which is the `ident`.
            let js_var = match name {
                "_runtime" => "dart",
                "_rti" => "dart_rti",
                n => n,
            };
            LibInfo {
                ident: name.to_string(),
                module_path: String::new(),
                is_sdk: true,
                js_var: js_var.to_string(),
            }
        } else if let Some(rest) = uri.strip_prefix("package:") {
            let path = rest.strip_suffix(".dart").unwrap_or(rest);
            let ident = path_ident(path);
            LibInfo {
                ident: ident.clone(),
                module_path: format!("packages/{path}.js"),
                is_sdk: false,
                js_var: format!("L${ident}"),
            }
        } else {
            let p = uri
                .strip_prefix("file:///")
                .unwrap_or(uri)
                .replace('\\', "/");
            let rel = match &entry_dir {
                Some(d) if p.starts_with(d.as_str()) => p[d.len()..].to_string(),
                _ => p.replace(':', "_"),
            };
            let stem = rel.strip_suffix(".dart").unwrap_or(&rel).to_string();
            let ident = path_ident(&stem);
            LibInfo {
                ident: ident.clone(),
                module_path: format!("{stem}.js"),
                is_sdk: false,
                js_var: format!("L${ident}"),
            }
        };
        let mut info = info;
        if !info.is_sdk {
            let mut candidate = info.ident.clone();
            let mut n = 1;
            while used.contains(&candidate) {
                candidate = format!("{}${n}", info.ident);
                n += 1;
            }
            used.insert(candidate.clone());
            info.js_var = format!("L${candidate}");
            info.ident = candidate;
        }
        infos.push(info);
    }
    infos
}

/// `lib/src/a-b` → `lib__src__a_b`, with invalid characters escaped.
pub fn path_ident(path: &str) -> String {
    let mut out = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '/' | '\\' => out.push_str("__"),
            '-' => out.push('_'),
            '.' => {
                if chars.peek() == Some(&'.') {
                    chars.next();
                    out.push_str("__");
                } else {
                    out.push('.');
                }
            }
            _ => out.push(c),
        }
    }
    let mut res = String::new();
    for c in out.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            res.push(c);
        } else {
            res.push_str(&format!("${}", c as u32));
        }
    }
    if res.chars().next().is_some_and(|c| c.is_ascii_digit()) || js::is_reserved(&res) {
        res.insert(0, '$');
    }
    res
}
