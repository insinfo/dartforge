//! Inheritance clauses (`extends`/`with`/`implements`/`on`) of the classes,
//! class aliases, enums and mixins of a library, as analyzer 6.11.0 checks
//! them in two places:
//!
//! * `InheritanceOverrideVerifier._checkDirectSuperTypes`
//!   (`src/error/inheritance_override.dart`): a type that cannot be extended
//!   or implemented (`subtype_of_disallowed_type`) and `Enum` as a
//!   superinterface of a concrete class (`concrete_class_has_enum_superinterface`);
//! * `ErrorVerifier._checkClassInheritance` and `_checkMixinInheritance`
//!   (`src/generated/error_verifier.dart`): a **gate** — forbidden supertype,
//!   deferred clause, mixin error or a superclass with only `factory`
//!   constructors — which, if something was reported, turns off the
//!   following checks (`class_used_as_mixin`, modifiers outside the library…).
//!
//! The gate is exposed ([`gate`]) so that the modifier verifiers
//! (`modifiers.rs`) follow the same rule. Where the gate depends on types this
//! layer does not compute (`on` constraints of the applied mixin, members
//! invoked through `super` in the mixin), it answers [`Gate::Uncertain`] and
//! nothing that depends on it is emitted.

use dartforge_diagnostics::{Code, Diagnostic, Span, codes::compile_time_error as c};
use dartforge_elements::model::{
    ClassId, ClassKind, Element, FunctionKind, LibraryId, Program, UnitId,
};
use dartforge_frontend::ast::{self, DeclKind, TypeKind, TypedefKind};
use dartforge_intern::{Interner, SymbolId};

/// The type layer's decisions about the mixins (`MixinElement`) of the `with`
/// clauses (`dartforge_types::phase_mixins::decisoes`): by `(class, position
/// in the with)`, `None` without a report or the report
/// (`mixin_application_not_implemented_interface`,
/// `mixin_application_no_concrete_super_invoked_*`,
/// `mixin_application_concrete_super_invoked_member_type`). Without a
/// decision, the gate stays uncertain as before.
pub type MixinDecisions = std::collections::HashMap<(ClassId, usize), Option<Diagnostic>>;

thread_local! {
    static MIXIN_DECISIONS: std::cell::RefCell<Option<std::rc::Rc<MixinDecisions>>> = const { std::cell::RefCell::new(None) };
}

/// Sets (or clears, with `None`) the mixin decisions for the following
/// analyses of this thread.
pub fn set_mixin_decisions(d: Option<MixinDecisions>) {
    MIXIN_DECISIONS.with(|c| *c.borrow_mut() = d.map(std::rc::Rc::new));
}

fn mixin_decision(id: ClassId, i: usize) -> Option<Option<Diagnostic>> {
    MIXIN_DECISIONS.with(|c| c.borrow().as_ref().and_then(|m| m.get(&(id, i)).cloned()))
}

/// The result of the `ErrorVerifier` gate for a declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Nothing reported: the following checks run.
    Open,
    /// Something was reported: the analyzer stops here.
    Closed,
    /// Depends on types this layer does not compute; whoever depends on the
    /// gate emits nothing.
    Uncertain,
}

/// What a type written in a clause names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseTarget {
    /// A class, mixin, enum or extension type (an interface type).
    Class(ClassId),
    /// A type that is not an interface type (function, record, `void`, `dynamic`).
    NonInterface,
    /// Not resolved here (unknown or ambiguous name, type parameter).
    Unknown,
}

/// The element of a clause type for the `ResolutionVisitor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClauseResolution {
    /// Class, mixin, enum or extension type.
    Class(ClassId),
    /// Another type element: a type parameter, `dynamic`, `Never`, an alias of
    /// a function, a record or `void`.
    NonClass,
    /// Already reported by name resolution, or not decided here.
    Ignore,
    /// An alias that expands to one of its type parameters
    /// (`_verifyTypeAliasForContext`): `*_type_alias_expands_to_type_parameter`.
    ParamAlias,
}

/// A reader of the clauses in the scope of a library.
struct Reader<'a> {
    program: &'a Program,
    names: &'a Interner,
}

impl Reader<'_> {
    fn name(&self, s: SymbolId) -> &str {
        self.names.resolve(s)
    }

    /// The element that `name` (or `prefix.name`) designates in the scope of
    /// unit `u` (with `parts-with-imports`, its own).
    fn element(&self, u: UnitId, name: &[ast::Name]) -> Option<Element> {
        let b = match name {
            [n] => self.program.lookup_in_unit(u, n.sym),
            [p, n] => self.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
            _ => None,
        }?;
        if b.ambiguous {
            return None;
        }
        b.getter
    }

    /// The target of type `t` (of tree `ast`, written in unit `u`), following
    /// type aliases up to the class they name.
    fn target(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, depth: u32) -> ClauseTarget {
        if depth > 8 {
            return ClauseTarget::Unknown;
        }
        let node = ast_.ty(t);
        let TypeKind::Named { name, .. } = &node.kind else {
            return ClauseTarget::NonInterface;
        };
        let Some(e) = self.element(u, name) else {
            return match name.last().map(|n| self.name(n.sym)) {
                Some("dynamic") if name.len() == 1 => ClauseTarget::NonInterface,
                _ => ClauseTarget::Unknown,
            };
        };
        match e {
            Element::Class(id) => ClauseTarget::Class(id),
            Element::Typedef(tid) => {
                let td = self.program.typedef(tid);
                let ast_td = &self.program.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return ClauseTarget::Unknown;
                };
                let TypedefKind::Alias(body) = d.kind else {
                    return ClauseTarget::NonInterface;
                };
                if let TypeKind::Named { name: n2, .. } = &ast_td.ty(body).kind
                    && let [unique] = &n2[..]
                    && d.type_params.iter().any(|p| p.name.sym == unique.sym)
                {
                    // An alias that expands to a type parameter: the analyzer
                    // reports `supertype_expands_to_type_parameter` (another
                    // verifier) and does not check the clause.
                    return ClauseTarget::Unknown;
                }
                self.target(td.decl.unit, ast_td, body, depth + 1)
            }
            _ => ClauseTarget::Unknown,
        }
    }

    /// The written type as the analyzer displays it (`DartType.getDisplayString`),
    /// with the alias name when the type comes from a `typedef`; `None` where
    /// the display would depend on types not computed here.
    fn display(
        &self,
        u: UnitId,
        ast_: &ast::Ast,
        t: ast::TypeId,
        params: &[SymbolId],
    ) -> Option<String> {
        let node = ast_.ty(t);
        let base = match &node.kind {
            TypeKind::Void => "void".to_string(),
            TypeKind::Named { name, args } => {
                let last = name.last()?;
                let text = self.name(last.sym).to_string();
                let bounded: Vec<bool> = match self.element(u, name) {
                    Some(Element::Class(id)) => self
                        .program
                        .class(id)
                        .type_params
                        .iter()
                        .map(|p| p.bound.is_some())
                        .collect(),
                    Some(Element::Typedef(tid)) => self
                        .program
                        .typedef(tid)
                        .type_params
                        .iter()
                        .map(|p| p.bound.is_some())
                        .collect(),
                    None if name.len() == 1
                        && (params.contains(&last.sym) || text == "dynamic") =>
                    {
                        if !args.is_empty() {
                            return None;
                        }
                        Vec::new()
                    }
                    _ => return None,
                };
                if args.is_empty() {
                    if bounded.is_empty() {
                        text
                    } else if bounded.iter().all(|b| !b) {
                        // Instantiated to bounds: without a bound, `dynamic`.
                        format!("{text}<{}>", vec!["dynamic"; bounded.len()].join(", "))
                    } else {
                        return None;
                    }
                } else if args.len() == bounded.len() {
                    let parts: Option<Vec<String>> = args
                        .iter()
                        .map(|&a| self.display(u, ast_, a, params))
                        .collect();
                    format!("{text}<{}>", parts?.join(", "))
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        Some(if node.nullable {
            format!("{base}?")
        } else {
            base
        })
    }

    /// `namedType.isDeferred`: the type's prefix comes from a `deferred` import.
    fn is_deferred(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId) -> bool {
        let TypeKind::Named { name, .. } = &ast_.ty(t).kind else {
            return false;
        };
        let [p, _] = &name[..] else { return false };
        self.program
            .library(self.program.unit(u).library)
            .imports
            .iter()
            .any(|i| i.deferred && i.prefix == Some(p.sym))
    }

    /// `TypeProvider.isNonSubtypableClass`.
    fn is_forbidden(&self, id: ClassId) -> bool {
        let c = self.program.class(id);
        match &self.program.library(c.library).uri[..] {
            "dart:core" => {
                matches!(
                    self.name(c.name),
                    "bool" | "double" | "Enum" | "int" | "Null" | "num" | "Record" | "String"
                )
            }
            "dart:async" => self.name(c.name) == "FutureOr",
            "dart:typed_data" => false,
            _ => false,
        }
    }

    /// The written type is nullable: `?` on it or on an alias it expands.
    fn is_nullable(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, depth: u32) -> bool {
        let node = ast_.ty(t);
        if node.nullable {
            return true;
        }
        let TypeKind::Named { name, .. } = &node.kind else { return false };
        if depth > 8 {
            return false;
        }
        match self.element(u, name) {
            Some(Element::Typedef(tid)) => {
                let td = self.program.typedef(tid);
                let ast_td = &self.program.unit(td.decl.unit).ast;
                match &ast_td.decl(td.decl.decl).kind {
                    DeclKind::Typedef(d) => match d.kind {
                        TypedefKind::Alias(body) => self.is_nullable(td.decl.unit, ast_td, body, depth + 1),
                        TypedefKind::Legacy { .. } => false,
                    },
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// `_verifyNullability` sees the resolved type (`nullabilitySuffix`): the
    /// class, the type parameter, the function or record alias and `Never`
    /// take the `?`; `dynamic` does not, and a name that is not a type (or does
    /// not exist) becomes `InvalidType` without a report.
    fn nullability_checked(&self, u: UnitId, ast_: &ast::Ast, t: ast::TypeId, params: &[SymbolId], resolved: &ClauseResolution) -> bool {
        match resolved {
            ClauseResolution::Class(_) => true,
            ClauseResolution::NonClass => {
                let TypeKind::Named { name, .. } = &ast_.ty(t).kind else { return false };
                if let [n] = &name[..]
                    && params.contains(&n.sym)
                {
                    return true;
                }
                match self.element(u, name) {
                    Some(Element::Typedef(_)) => true,
                    Some(_) => false,
                    None => matches!(&name[..], [n] if self.name(n.sym) == "Never"),
                }
            }
            _ => false,
        }
    }

    /// The `dart:core` class `name`.
    fn core_class(&self, name: &str) -> Option<ClassId> {
        self.program.classes.iter().position(|c| {
            c.decl.is_some() && self.program.library(c.library).uri == "dart:core" && self.name(c.name) == name
        })
        .map(|i| ClassId(i as u32))
    }

    /// What `ResolutionVisitor._resolveType` sees in a clause type: the type's
    /// element, following aliases; [`ClauseResolution::Ignore`] when name
    /// resolution already reported an error (undefined name, not a type) or
    /// nothing is decided here.
    fn resolve(
        &self,
        u: UnitId,
        ast_: &ast::Ast,
        t: ast::TypeId,
        params: &[SymbolId],
        depth: u32,
        ignores_non_type: bool,
    ) -> ClauseResolution {
        if depth > 8 {
            return ClauseResolution::Ignore;
        }
        let TypeKind::Named { name, .. } = &ast_.ty(t).kind else {
            return ClauseResolution::Ignore;
        };
        if let [n] = &name[..]
            && params.contains(&n.sym)
        {
            return ClauseResolution::NonClass;
        }
        // Ambiguous name (`ambiguous_import`): not decided.
        let binding = match &name[..] {
            [n] => self.program.lookup_in_unit(u, n.sym),
            [p, n] => self.program.lookup_prefixed_in_unit(u, p.sym, n.sym),
            _ => None,
        };
        if binding.is_some_and(|b| b.ambiguous) {
            return ClauseResolution::Ignore;
        }
        match self.element(u, name) {
            Some(Element::Class(id)) => ClauseResolution::Class(id),
            Some(Element::Typedef(tid)) => {
                let td = self.program.typedef(tid);
                let ast_td = &self.program.unit(td.decl.unit).ast;
                let DeclKind::Typedef(d) = &ast_td.decl(td.decl.decl).kind else {
                    return ClauseResolution::Ignore;
                };
                let TypedefKind::Alias(body) = d.kind else {
                    return ClauseResolution::NonClass;
                };
                let ps: Vec<SymbolId> = d.type_params.iter().map(|p| p.name.sym).collect();
                match &ast_td.ty(body).kind {
                    TypeKind::Named { name: n2, .. } => {
                        if let [unique] = &n2[..]
                            && ps.contains(&unique.sym)
                        {
                            // `aliasedType is TypeParameterType`: the alias
                            // written in the clause. An alias that reaches
                            // another depends on the substitution of the
                            // arguments (not decided here).
                            return if depth == 0 { ClauseResolution::ParamAlias } else { ClauseResolution::Ignore };
                        }
                        self.resolve(td.decl.unit, ast_td, body, &[], depth + 1, ignores_non_type)
                    }
                    _ => ClauseResolution::NonClass,
                }
            }
            // A name that is not a type, or that does not exist: in `extends`,
            // `implements` and `with` the `NamedTypeResolver` reports nothing
            // (`reportNullOrNonTypeElement`), and the error is the one here.
            Some(_) => {
                if ignores_non_type {
                    ClauseResolution::NonClass
                } else {
                    ClauseResolution::Ignore
                }
            }
            None => match &name[..] {
                [n] if matches!(self.name(n.sym), "dynamic" | "Never")
                    && self.program.lookup_in_unit(u, n.sym).is_none() =>
                {
                    ClauseResolution::NonClass
                }
                [.., n] if ignores_non_type && depth == 0 => {
                    let prefix = if name.len() == 2 { Some(name[0].sym) } else { None };
                    if self.ignores_undefined(u, prefix, n.sym) {
                        ClauseResolution::Ignore
                    } else {
                        ClauseResolution::NonClass
                    }
                }
                _ => ClauseResolution::Ignore,
            },
        }
    }

    /// `CompilationUnitElementImpl.shouldIgnoreUndefined`: the name may come
    /// from an import that does not exist (with the prefix, or in a `show`),
    /// or, if it starts with `_$`, from a generated part that does not exist yet.
    fn ignores_undefined(&self, u: UnitId, wanted_prefix: Option<SymbolId>, name: SymbolId) -> bool {
        let lib_id = self.program.unit(u).library;
        let lib = self.program.library(lib_id);
        for &uid in &lib.units {
            let unit = self.program.unit(uid);
            for (i, d) in unit.unit.directives.iter().enumerate() {
                match &d.kind {
                    ast::DirectiveKind::Import { prefix, combinators, .. } => {
                        if prefix.map(|p| p.sym) != wanted_prefix {
                            continue;
                        }
                        let synthetic = !lib
                            .imports
                            .iter()
                            .any(|im| im.unit == uid && im.directive == i && !self.program.library(im.library).units.is_empty());
                        if !synthetic {
                            continue;
                        }
                        let shows: Vec<&Vec<ast::Name>> = combinators
                            .iter()
                            .filter_map(|c| match c {
                                ast::Combinator::Show(v) => Some(v),
                                ast::Combinator::Hide(_) => None,
                            })
                            .collect();
                        if wanted_prefix.is_some() && shows.is_empty() {
                            return true;
                        }
                        if shows.iter().any(|v| v.iter().any(|n| n.sym == name)) {
                            return true;
                        }
                    }
                    ast::DirectiveKind::Part { uri } if wanted_prefix.is_none() && self.name(name).starts_with("_$") => {
                        let Some(text) = dartforge_elements::load::string_lit_value(uri) else { continue };
                        let generated = [".g.dart", ".pb.dart", ".pbenum.dart", ".pbserver.dart", ".pbjson.dart", ".template.dart"]
                            .iter()
                            .any(|s| text.ends_with(s));
                        let exists = text.contains(':')
                            || unit.path.as_ref().and_then(|p| p.parent()).is_none_or(|d| d.join(&text).is_file());
                        if generated && !exists {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    /// The element as the analyzer displays it (`ElementDisplayStringBuilder.writeClassElement`):
    /// modifiers, name, type parameters and clauses; `None` where the display
    /// would depend on types not computed here.
    fn display_class(&self, id: ClassId) -> Option<String> {
        let c = self.program.class(id);
        if !matches!(c.kind, ClassKind::Class | ClassKind::MixinApplication) {
            return None;
        }
        let decl = c.decl?;
        let ast_ = &self.program.unit(decl.unit).ast;
        let DeclKind::Class(d) = &ast_.decl(decl.decl).kind else { return None };
        let m = c.modifiers;
        let mut s = String::new();
        if m.sealed {
            s.push_str("sealed ");
        } else if m.abstract_ {
            s.push_str("abstract ");
        }
        if m.base {
            s.push_str("base ");
        } else if m.interface {
            s.push_str("interface ");
        } else if m.final_ {
            s.push_str("final ");
        }
        if m.mixin {
            s.push_str("mixin ");
        }
        s.push_str("class ");
        s.push_str(self.name(c.name));
        let params: Vec<SymbolId> = d.type_params.iter().map(|p| p.name.sym).collect();
        if !d.type_params.is_empty() {
            let mut ps = Vec::new();
            for p in d.type_params.iter() {
                let mut x = self.name(p.name.sym).to_string();
                if let Some(b) = p.bound {
                    x.push_str(" extends ");
                    x.push_str(&self.display(decl.unit, ast_, b, &params)?);
                }
                ps.push(x);
            }
            s.push_str(&format!("<{}>", ps.join(", ")));
        }
        if let Some(t) = d.extends {
            let e = self.display(decl.unit, ast_, t, &params)?;
            if e != "Object" {
                s.push_str(" extends ");
                s.push_str(&e);
            }
        }
        for (word, types) in [(" with ", &d.with), (" implements ", &d.implements)] {
            if !types.is_empty() {
                let v: Option<Vec<String>> = types.iter().map(|&t| self.display(decl.unit, ast_, t, &params)).collect();
                s.push_str(word);
                s.push_str(&v?.join(", "));
            }
        }
        Some(s)
    }

    /// Does the class (or mixin) declare an instance field?
    fn has_instance_field(&self, id: ClassId) -> bool {
        let c = self.program.class(id);
        if matches!(c.kind, ClassKind::Enum | ClassKind::ExtensionType) {
            return false;
        }
        let Some(decl) = c.decl else { return false };
        let ast_ = &self.program.unit(decl.unit).ast;
        let members = match &ast_.decl(decl.decl).kind {
            DeclKind::Class(d) => &d.members,
            DeclKind::Mixin(d) => &d.members,
            _ => return false,
        };
        members.iter().any(|&m| matches!(&ast_.member(m).kind, ast::MemberKind::Field(vl) if !vl.static_))
    }

    fn is_core_enum(&self, id: ClassId) -> bool {
        let c = self.program.class(id);
        self.program.library(c.library).uri == "dart:core" && self.name(c.name) == "Enum"
    }

    /// Does it have a written generative constructor (neither synthetic nor `factory`)?
    fn declares_constructor(&self, id: ClassId) -> bool {
        self.program.class(id).constructors.values().any(|&f| {
            let f = self.program.function(f);
            f.kind == FunctionKind::Constructor && !f.factory
        })
    }

    /// Only `factory` constructors (and at least one)? A class without a
    /// written constructor has the synthetic, generative one.
    fn only_factories(&self, id: ClassId) -> bool {
        let c = self.program.class(id);
        !c.constructors.is_empty()
            && c.constructors
                .values()
                .all(|&f| self.program.function(f).factory)
    }
}

/// The clauses of a class, alias, enum or mixin declaration.
struct Clauses<'a> {
    extends: Option<ast::TypeId>,
    with: &'a [ast::TypeId],
    implements: &'a [ast::TypeId],
    on: &'a [ast::TypeId],
    params: Vec<SymbolId>,
}

fn clauses(k: &DeclKind) -> Option<Clauses<'_>> {
    let ps = |v: &[ast::TypeParameter]| v.iter().map(|p| p.name.sym).collect::<Vec<_>>();
    match k {
        DeclKind::Class(x) => Some(Clauses {
            extends: x.extends,
            with: &x.with,
            implements: &x.implements,
            on: &[],
            params: ps(&x.type_params),
        }),
        DeclKind::Enum(x) => Some(Clauses {
            extends: None,
            with: &x.with,
            implements: &x.implements,
            on: &[],
            params: ps(&x.type_params),
        }),
        DeclKind::Mixin(x) => Some(Clauses {
            extends: None,
            with: &[],
            implements: &x.implements,
            on: &x.on,
            params: ps(&x.type_params),
        }),
        _ => None,
    }
}

/// What the gate decided and what it reported along the way.
struct GateResult {
    gate: Gate,
    reports: Vec<Diagnostic>,
}

/// The `ErrorVerifier` gate for class `id` (declared in `lib`), with the
/// diagnostics it reports itself.
fn evaluate(l: &Reader<'_>, lib: LibraryId, id: ClassId) -> Option<GateResult> {
    let class = l.program.class(id);
    let decl = class.decl?;
    let u = decl.unit;
    let ast_ = &l.program.unit(u).ast;
    let k = &ast_.decl(decl.decl).kind;
    let cl = clauses(k)?;
    let sdk = l.program.library(lib).is_sdk;
    let mut reports = Vec::new();
    let mut closed = false;
    let mut uncertain = false;
    let target = |t: ast::TypeId| l.target(u, ast_, t, 0);
    // `_checkForExtendsOrImplementsDisallowedClass`: never in a `dart:` library.
    let forbidden = |a: ClauseTarget| !sdk && matches!(a, ClauseTarget::Class(c) if l.is_forbidden(c));

    if class.kind == ClassKind::Mixin {
        // `_checkMixinInheritance`: `on` and `implements`.
        for &t in cl.on {
            match target(t) {
                a if forbidden(a) => closed = true,
                ClauseTarget::Class(_) => {
                    if l.is_deferred(u, ast_, t) {
                        reports.push(Diagnostic::with_code(
                            c::MIXIN_SUPER_CLASS_CONSTRAINT_DEFERRED_CLASS,
                            ast_.ty(t).span,
                            [] as [&str; 0],
                        ));
                        closed = true;
                    }
                }
                ClauseTarget::NonInterface => {}
                ClauseTarget::Unknown => uncertain = true,
            }
        }
        implements(
            l,
            u,
            ast_,
            cl.implements,
            &mut reports,
            &mut closed,
            &mut uncertain,
            sdk,
        );
    } else {
        // `_checkClassInheritance`, in the order of the `&&`s: a step that
        // reports ends the following ones.
        if let Some(t) = cl.extends {
            match target(t) {
                a if forbidden(a) => closed = true,
                ClauseTarget::Unknown => uncertain = true,
                _ => {}
            }
        }
        if !closed {
            implements(
                l,
                u,
                ast_,
                cl.implements,
                &mut reports,
                &mut closed,
                &mut uncertain,
                sdk,
            );
        }
        if !closed {
            mixins(
                l,
                u,
                id,
                ast_,
                cl.with,
                &mut reports,
                &mut closed,
                &mut uncertain,
                sdk,
            );
        }
        // `_checkForNoGenerativeConstructorsInSuperclass`: with mixins, the
        // supertype is the synthetic application `S with M`, whose
        // constructors are the forwarded generative ones of `S` — never only `factory`.
        if !closed
            && class.kind == ClassKind::Class
            && cl.with.is_empty()
            && let (Some(t), Some(s)) = (cl.extends, class.supertype_class)
            && !l.only_factories(id)
            && l.only_factories(s)
        {
            reports.push(Diagnostic::with_code(
                c::NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS,
                ast_.ty(t).span,
                [l.name(class.name), l.name(l.program.class(s).name)],
            ));
            closed = true;
        }
    }
    let gate = if closed {
        Gate::Closed
    } else if uncertain {
        Gate::Uncertain
    } else {
        Gate::Open
    };
    Some(GateResult { gate, reports })
}

/// The unnamed constructor a class offers a subclass: `Ok(f)` declared,
/// `Err(())` the synthetic one without parameters (also that of an abstract
/// class, which the model does not create); in a mixin application, the
/// generative one forwarded from the base.
fn superclass_unnamed_constructor(l: &Reader<'_>, s: ClassId) -> Option<Result<dartforge_elements::model::FunctionElementId, ()>> {
    let empty = l.names.lookup("");
    let mut current = s;
    let mut forwarded = false;
    for _ in 0..16 {
        let ce = l.program.class(current);
        if ce.kind == ClassKind::MixinApplication {
            current = ce.supertype_class?;
            forwarded = true;
            continue;
        }
        if let Some(&f) = empty.and_then(|v| ce.constructors.get(&v)) {
            let fe = l.program.function(f);
            if forwarded && fe.factory {
                return None;
            }
            if matches!(fe.node, dartforge_elements::model::FunctionRef::None) {
                return Some(Err(()));
            }
            return Some(Ok(f));
        }
        return (ce.constructors.is_empty() && ce.kind == ClassKind::Class).then_some(Err(()));
    }
    None
}

/// `_checkForNoDefaultSuperConstructorImplicit`
/// (`an611:src/generated/error_verifier.dart:4634-4678`): a class without a
/// declared constructor whose `extends` is a class with an unnamed factory
/// (`non_generative_implicit_constructor`) or without an unnamed one that
/// needs no arguments (`no_default_super_constructor`, silent for the classes
/// that cannot be extended). On the class name. The `extends` that does not
/// resolve here is left out (the supertype would be `Object`, and the gate is
/// already uncertain).
fn implicit_super_of_class(
    l: &Reader<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    name: SymbolId,
    name_span: dartforge_diagnostics::Span,
    t: ast::TypeId,
    params: &[SymbolId],
) -> Option<Diagnostic> {
    let ClauseTarget::Class(s) = l.target(u, ast_, t, 0) else { return None };
    let se = l.program.class(s);
    if !matches!(se.kind, ClassKind::Class | ClassKind::MixinApplication) || l.is_nullable(u, ast_, t, 0) {
        return None;
    }
    if l.program.library(se.library).uri == "dart:core" && matches!(l.name(se.name), "Function" | "Null") {
        return None;
    }
    let class = l.name(name).to_string();
    match superclass_unnamed_constructor(l, s) {
        Some(Ok(f)) if l.program.function(f).factory => {
            let super_name = l.name(se.name).to_string();
            let displayed = display_unnamed_constructor(l, s, f);
            return Some(Diagnostic::with_code(
                c::NON_GENERATIVE_IMPLICIT_CONSTRUCTOR,
                name_span,
                [super_name.as_str(), class.as_str(), displayed.as_str()],
            ));
        }
        // `isDefaultConstructor`: no required parameter.
        Some(Err(())) => return None,
        Some(Ok(f)) => {
            let dartforge_elements::model::FunctionRef::Constructor { unit, member } = l.program.function(f).node else { return None };
            let ast::MemberKind::Constructor(k) = &l.program.unit(unit).ast.member(member).kind else { return None };
            if !k.parameters.iter().any(|p| p.kind == ast::ParameterKind::Required || (p.kind == ast::ParameterKind::Named && p.required)) {
                return None;
            }
        }
        None => {}
    }
    if l.is_forbidden(s) {
        return None;
    }
    let displayed = l.display(u, ast_, t, params)?;
    Some(Diagnostic::with_code(c::NO_DEFAULT_SUPER_CONSTRUCTOR_IMPLICIT, name_span, [displayed.as_str(), class.as_str()]))
}

/// `ConstructorElement.getDisplayString()` of the unnamed `f` of `s` (it only
/// goes to the correction of `non_generative_implicit_constructor`):
/// `A A(int x)`, with the types as written.
fn display_unnamed_constructor(l: &Reader<'_>, s: ClassId, f: dartforge_elements::model::FunctionElementId) -> String {
    let se = l.program.class(s);
    let params_of_s: Vec<SymbolId> = se.type_params.iter().map(|p| p.name).collect();
    let mut text = l.name(se.name).to_string();
    if !params_of_s.is_empty() {
        let names: Vec<&str> = params_of_s.iter().map(|&p| l.name(p)).collect();
        text = format!("{text}<{}>", names.join(", "));
    }
    let mut s_ = format!("{text} {}(", l.name(se.name));
    if let dartforge_elements::model::FunctionRef::Constructor { unit, member } = l.program.function(f).node
        && let ast::MemberKind::Constructor(k) = &l.program.unit(unit).ast.member(member).kind
    {
        let ast_k = &l.program.unit(unit).ast;
        let source = &l.program.unit(unit).source;
        let mut last: Option<ast::ParameterKind> = None;
        let mut closer = "";
        for (i, p) in k.parameters.iter().enumerate() {
            if i != 0 {
                s_.push_str(", ");
            }
            if last != Some(p.kind) {
                s_.push_str(closer);
                let (opens, closes) = match p.kind {
                    ast::ParameterKind::Required => ("", ""),
                    ast::ParameterKind::Optional => ("[", "]"),
                    ast::ParameterKind::Named => ("{", "}"),
                };
                s_.push_str(opens);
                closer = closes;
                last = Some(p.kind);
            }
            if p.kind == ast::ParameterKind::Named && p.required {
                s_.push_str("required ");
            }
            let ty = match p.ty {
                Some(ty) => l.display(unit, ast_k, ty, &params_of_s).unwrap_or_else(|| {
                    let sp = ast_k.ty(ty).span;
                    source[sp.start..sp.end].to_string()
                }),
                None => "dynamic".to_string(),
            };
            s_.push_str(&ty);
            s_.push(' ');
            if let Some(n) = p.external_name() {
                s_.push_str(l.name(n.sym));
            }
            if let Some(d) = p.default_value {
                let sp = ast_k.expr(d).span;
                s_.push_str(" = ");
                s_.push_str(&source[sp.start..sp.end]);
            }
        }
        s_.push_str(closer);
    }
    s_.push(')');
    s_
}

/// `_checkForImplementsClauseErrorCodes`.
#[allow(clippy::too_many_arguments)]
fn implements(
    l: &Reader<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    types: &[ast::TypeId],
    reports: &mut Vec<Diagnostic>,
    closed: &mut bool,
    uncertain: &mut bool,
    sdk: bool,
) {
    for &t in types {
        match l.target(u, ast_, t, 0) {
            ClauseTarget::Class(c) if !sdk && l.is_forbidden(c) => *closed = true,
            ClauseTarget::Unknown => *uncertain = true,
            _ => {
                if l.is_deferred(u, ast_, t) {
                    reports.push(Diagnostic::with_code(
                        c::IMPLEMENTS_DEFERRED_CLASS,
                        ast_.ty(t).span,
                        [] as [&str; 0],
                    ));
                    *closed = true;
                }
            }
        }
    }
}

/// `_checkForAllMixinErrorCodes`.
#[allow(clippy::too_many_arguments)]
fn mixins(
    l: &Reader<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    types: &[ast::TypeId],
    reports: &mut Vec<Diagnostic>,
    closed: &mut bool,
    uncertain: &mut bool,
    sdk: bool,
) {
    let mut previous: Vec<ClassId> = Vec::new();
    for (position, &t) in types.iter().enumerate() {
        let m = match l.target(u, ast_, t, 0) {
            ClauseTarget::Class(m) => m,
            ClauseTarget::NonInterface => continue,
            ClauseTarget::Unknown => {
                *uncertain = true;
                continue;
            }
        };
        if !sdk && l.is_forbidden(m) {
            *closed = true;
            continue;
        }
        let span = ast_.ty(t).span;
        let previous_here = previous.clone();
        previous.push(m);
        let previous = previous_here;
        if l.is_deferred(u, ast_, t) {
            reports.push(Diagnostic::with_code(
                c::MIXIN_DEFERRED_CLASS,
                span,
                [] as [&str; 0],
            ));
            *closed = true;
        }
        let el = l.program.class(m);
        let duplicated = l
            .program
            .classes
            .iter()
            .filter(|c| c.library == el.library && c.name == el.name)
            .count()
            > 1;
        if m == id && duplicated {
            // `class A {}` and then `class A with A` (`duplicate_definition`):
            // for the analyzer the name is the first declaration's, and here it
            // resolved to the second; not decided.
            *uncertain = true;
            continue;
        }
        match el.kind {
            ClassKind::ExtensionType => {}
            ClassKind::Mixin => {
                // Decided with types (`phase_mixins`): the report closes the gate.
                if let Some(decision) = mixin_decision(id, position) {
                    if let Some(d) = decision {
                        reports.push(d);
                        *closed = true;
                    }
                    continue;
                }
                // `on` constraints and members invoked through `super` depend
                // on types: only a mixin without `on` and without `super` is safe.
                let decl = el.decl.map(|d| {
                    let unit = l.program.unit(d.unit);
                    let sp = unit.ast.decl(d.decl).span;
                    unit.source
                        .get(sp.start..sp.end)
                        .unwrap_or("")
                        .contains("super")
                });
                if decl != Some(false) {
                    *uncertain = true;
                } else if !el.on.is_empty() {
                    // `_checkForMixinSuperclassConstraints`.
                    match constraints_satisfied(l, id, &previous, m) {
                        Some(Ok(())) => {}
                        // `mixin_application_not_implemented_interface`, on
                        // the mixin's name, closes the gate.
                        Some(Err(i)) => {
                            *closed = true;
                            if let Some(d) = not_implemented(l, u, id, ast_, t, m, i) {
                                reports.push(d);
                            }
                        }
                        None => *uncertain = true,
                    }
                }
            }
            ClassKind::Enum => *uncertain = true,
            ClassKind::Class | ClassKind::MixinApplication => {
                let name = l.name(el.name);
                if !el.modifiers.mixin && l.declares_constructor(m) {
                    reports.push(Diagnostic::with_code(
                        c::MIXIN_CLASS_DECLARES_CONSTRUCTOR,
                        span,
                        [name],
                    ));
                    *closed = true;
                }
                // `_checkForMixinInheritsNotFromObject`.
                let object = match el.supertype_class {
                    None => el.supertype.is_none(),
                    Some(s) => {
                        let sc = l.program.class(s);
                        sc.supertype_class.is_none()
                            && l.program.library(sc.library).uri == "dart:core"
                    }
                };
                let resolved = el.supertype.is_none() || el.supertype_class.is_some();
                if !resolved || el.mixins.len() != el.mixin_classes.len() {
                    *uncertain = true;
                    continue;
                }
                let inherits = !object
                    || !(el.mixins.is_empty()
                        || el.kind == ClassKind::MixinApplication && el.mixins.len() < 2);
                if inherits {
                    reports.push(Diagnostic::with_code(
                        c::MIXIN_INHERITS_FROM_NOT_OBJECT,
                        span,
                        [name],
                    ));
                    *closed = true;
                }
            }
        }
    }
}

/// `_checkForMixinClassErrorCodes` of a `mixin class` (or alias `mixin class
/// A = …`): a non-trivial generative constructor, and a supertype that is not
/// `Object` (through `extends` or through the `with` clause).
fn mixin_class(
    l: &Reader<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    source: &str,
) -> Vec<Diagnostic> {
    let class = l.program.class(id);
    let name = l.name(class.name);
    let Some(decl) = class.decl else {
        return Vec::new();
    };
    let DeclKind::Class(d) = &ast_.decl(decl.decl).kind else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for &mid in d.members.iter() {
        let ast::MemberKind::Constructor(k) = &ast_.member(mid).kind else {
            continue;
        };
        // A primary constructor and a `new(…)` member are 3.13 syntax, where the
        // analyzer reports another code
        // (`mixin_class_declares_non_trivial_generative_constructor`).
        let written = source
            .get(k.class_name.span.start..k.class_name.span.end)
            .unwrap_or("");
        if k.factory || k.primary_body_part {
            continue;
        }
        // 3.13's `_checkForMixinClassErrorCodes` (main checkout,
        // `error_verifier.dart:6657-6712`): the primary with parameters, on the
        // name (`A` or `A.name`); without parameters, the `this` part with
        // initializers (on the `:`) or with a block body (on the `{`). It comes
        // out with the 3.6 code, which the 3.13 translation replaces with
        // `MIXIN_CLASS_DECLARES_NON_TRIVIAL_GENERATIVE_CONSTRUCTOR`.
        if d.primary_constructor == Some(mid) {
            let sp = if !k.parameters.is_empty() {
                Some(Span { start: k.class_name.span.start, end: k.name.map_or(k.class_name.span.end, |n| n.span.end) })
            } else {
                let part = ast_.member(mid).span;
                let text = source.get(part.start..part.end).unwrap_or("");
                if !text.starts_with("this") {
                    None
                } else if !k.initializers.is_empty() {
                    text.find(':').map(|i| Span { start: part.start + i, end: part.start + i + 1 })
                } else if matches!(k.body, ast::FunctionBody::Block(_)) {
                    text.find('{').map(|i| Span { start: part.start + i, end: part.start + i + 1 })
                } else {
                    None
                }
            };
            if let Some(sp) = sp {
                out.push(Diagnostic::with_code(c::MIXIN_CLASS_DECLARES_CONSTRUCTOR, sp, [name]));
            }
            continue;
        }
        // The `new(…)` member (3.13) has the same triviality test, on the `new`.
        let _ = written;
        // `ConstructorDeclaration.isTrivial`: `A();` and nothing more.
        let trivial = k.redirect.is_none()
            && k.parameters.is_empty()
            && k.initializers.is_empty()
            && matches!(k.body, ast::FunctionBody::Empty)
            && !k.external;
        if !trivial {
            out.push(Diagnostic::with_code(
                c::MIXIN_CLASS_DECLARES_CONSTRUCTOR,
                k.class_name.span,
                [name],
            ));
        }
    }
    // `superclass.typeOrThrow.isDartCoreObject`; an `extends` that does not
    // resolve here is not decided.
    let extends_another = match d.extends {
        None => Some(false),
        Some(t) => match l.target(u, ast_, t, 0) {
            ClauseTarget::Class(s) => Some(
                !(l.program.class(s).supertype_class.is_none()
                    && l.program.library(l.program.class(s).library).uri == "dart:core"),
            ),
            ClauseTarget::NonInterface => Some(true),
            ClauseTarget::Unknown => None,
        },
    };
    match (extends_another, d.extends) {
        (Some(true), Some(t)) => out.push(Diagnostic::with_code(
            c::MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT,
            ast_.ty(t).span,
            [name],
        )),
        (Some(false), _) => {
            if let (Some(&first), Some(&last)) = (d.with.first(), d.with.last()) {
                let simple_alias = d.mixin_application && d.with.len() < 2;
                // The `WithClause` node starts at the word `with`, right before the first type.
                let before = source[..ast_.ty(first).span.start].trim_end();
                if !simple_alias && before.ends_with("with") {
                    let span = dartforge_diagnostics::Span {
                        start: before.len() - 4,
                        end: ast_.ty(last).span.end,
                    };
                    out.push(Diagnostic::with_code(
                        c::MIXIN_CLASS_DECLARATION_EXTENDS_NOT_OBJECT,
                        span,
                        [name],
                    ));
                }
            }
        }
        _ => {}
    }
    out
}

/// The supertypes of `x`, transitively, including itself (through the bound classes).
fn supertypes(l: &Reader<'_>, x: ClassId, out: &mut Vec<ClassId>) {
    if out.contains(&x) {
        return;
    }
    out.push(x);
    let c = l.program.class(x);
    let direct_supertypes: Vec<ClassId> = c
        .supertype_class
        .into_iter()
        .chain(c.mixin_classes.iter().copied())
        .chain(c.interface_classes.iter().copied())
        .chain(c.on_classes.iter().copied())
        .collect();
    for s in direct_supertypes {
        supertypes(l, s, out);
    }
}

/// Are the `on` constraints of mixin `m`, applied in `id` after the mixins
/// `previous`, satisfied? Interface types are subtypes only nominally, so it is
/// enough for the constraint's class to be among the supertypes of the
/// superclass or of the previous mixins; with type arguments involved (a
/// generic constraint) the answer depends on types: `None`.
fn constraints_satisfied(
    l: &Reader<'_>,
    id: ClassId,
    previous: &[ClassId],
    m: ClassId,
) -> Option<Result<(), usize>> {
    let mixin = l.program.class(m);
    if mixin.on.len() != mixin.on_classes.len() {
        return None;
    }
    // A mixin with `augment` (the constraint may come from the augmentation,
    // which the analyzer does not read without the experiment) or a class in
    // an inheritance cycle.
    let augmented_lib = l.program.library(mixin.library).units.iter().any(|&u| {
        let a = &l.program.unit(u).ast;
        l.program.unit(u).unit.declarations.iter().any(|&d| {
            a.decl(d).augment && matches!(&a.decl(d).kind, DeclKind::Mixin(x) if x.name.sym == mixin.name)
        })
    });
    let repeated = l.program.classes.iter().filter(|c| c.library == mixin.library && c.name == mixin.name).count() > 1;
    let mut steps = 0;
    if augmented_lib || repeated || cycle(l, id, id, &mut Vec::new(), &mut steps).is_some() || steps > 20_000 {
        return None;
    }
    for &(u, t) in &mixin.on {
        if let TypeKind::Named { args, .. } = &l.program.unit(u).ast.ty(t).kind
            && !args.is_empty()
        {
            return None;
        }
    }
    if mixin
        .on_classes
        .iter()
        .any(|&c| !l.program.class(c).type_params.is_empty())
    {
        return None;
    }
    // `_enclosingClass.supertype`: num enum, `Enum`.
    let base = match l.program.class(id).supertype_class {
        Some(b) => b,
        None if l.program.class(id).kind == ClassKind::Enum => l.core_class("Enum")?,
        None => return None,
    };
    let mut known = Vec::new();
    supertypes(l, base, &mut known);
    for &a in previous {
        supertypes(l, a, &mut known);
    }
    match mixin.on_classes.iter().position(|c| !known.contains(c)) {
        None => Some(Ok(())),
        Some(i) => Some(Err(i)),
    }
}

/// The `mixin_application_not_implemented_interface` of mixin `m` (type `t` in
/// the `with` clause of `id`), whose `on` constraint at index `i` the
/// superclass does not implement: `'M' can't be mixed onto 'S' because 'S'
/// doesn't implement 'A'.` The supertype is the written `extends` (`Object`
/// omitted, `Enum` in an enum).
fn not_implemented(
    l: &Reader<'_>,
    u: UnitId,
    id: ClassId,
    ast_: &ast::Ast,
    t: ast::TypeId,
    m: ClassId,
    i: usize,
) -> Option<Diagnostic> {
    let class = l.program.class(id);
    let decl = class.decl?;
    let params: Vec<SymbolId> = clauses(&ast_.decl(decl.decl).kind)?.params;
    let mixin_text = l.display(u, ast_, t, &params)?;
    let mixin_text = mixin_text.strip_suffix('?').unwrap_or(&mixin_text).to_string();
    let super_txt = match (&ast_.decl(decl.decl).kind, clauses(&ast_.decl(decl.decl).kind)?.extends) {
        (_, Some(e)) => {
            let x = l.display(u, ast_, e, &params)?;
            x.strip_suffix('?').unwrap_or(&x).to_string()
        }
        (DeclKind::Enum(_), None) => "Enum".to_string(),
        (_, None) => "Object".to_string(),
    };
    let mixin = l.program.class(m);
    let (mu, mt) = *mixin.on.get(i)?;
    let mast = &l.program.unit(mu).ast;
    let constraint = l.display(mu, mast, mt, &[])?;
    let TypeKind::Named { name, .. } = &ast_.ty(t).kind else { return None };
    let span = name.last()?.span;
    Some(Diagnostic::with_code(
        c::MIXIN_APPLICATION_NOT_IMPLEMENTED_INTERFACE,
        span,
        [mixin_text.as_str(), super_txt.as_str(), constraint.as_str()],
    ))
}

/// Where the types of `extends`, `implements` and `with` of the classes,
/// aliases, enums and mixins of library `lib` start. There the
/// `NamedTypeResolver` reports neither an undefined name nor a name that is
/// not a type (the `ResolutionVisitor` reports `*_non_class`, [`check`]):
/// whoever gathers the `types` diagnostics drops `undefined_class`/`not_a_type`
/// at these points.
pub fn clause_name_starts(program: &Program, lib: LibraryId) -> Vec<(UnitId, usize)> {
    let mut v = Vec::new();
    for class in &program.classes {
        if class.library != lib {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let ast_ = &program.unit(decl.unit).ast;
        let Some(cl) = clauses(&ast_.decl(decl.decl).kind) else { continue };
        for &t in cl.extends.iter().chain(cl.with).chain(cl.implements) {
            if let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                && let Some(p) = name.first()
            {
                v.push((decl.unit, p.span.start));
            }
        }
    }
    v
}

/// The direct supertypes of `x` as the analyzer's element model keeps them:
/// the superclass (a class; the `extends` of a mixin, enum or non-class type
/// does not count), the mixins, the `on` constraints and the interfaces.
fn direct_supertypes(l: &Reader<'_>, x: ClassId) -> (Option<ClassId>, Vec<ClassId>, Vec<ClassId>, Vec<ClassId>) {
    let c = l.program.class(x);
    let Some(decl) = c.decl else { return (None, Vec::new(), Vec::new(), Vec::new()) };
    let ast_ = &l.program.unit(decl.unit).ast;
    let Some(cl) = clauses(&ast_.decl(decl.decl).kind) else { return (None, Vec::new(), Vec::new(), Vec::new()) };
    let class = |t: ast::TypeId, accepts_mixin: bool| match l.target(decl.unit, ast_, t, 0) {
        ClauseTarget::Class(y) => match l.program.class(y).kind {
            ClassKind::Class | ClassKind::MixinApplication => Some(y),
            ClassKind::Mixin if accepts_mixin => Some(y),
            _ => None,
        },
        _ => None,
    };
    let sup = cl.extends.and_then(|t| class(t, false));
    let mixins = cl.with.iter().filter_map(|&t| class(t, true)).collect();
    let on = cl.on.iter().filter_map(|&t| class(t, true)).collect();
    let interfaces = cl.implements.iter().filter_map(|&t| class(t, true)).collect();
    (sup, mixins, on, interfaces)
}

/// `_checkForRecursiveInterfaceInheritance`: the analyzer's depth-first search
/// (superclass, mixins, `on`, interfaces), with the path back to `target`. A
/// step limit avoids blowing up on large hierarchies (nothing is reported then).
fn cycle(l: &Reader<'_>, target: ClassId, el: ClassId, path: &mut Vec<ClassId>, steps: &mut u32) -> Option<Vec<ClassId>> {
    *steps += 1;
    if *steps > 20_000 {
        return None;
    }
    // T1.1 c: same-named declarations of the same kind are the same element
    // for the analyzer's hierarchy; `augment class A extends A` (without the
    // experiment: the second `A` extends the first) is "A implements itself".
    let same = el == target || l.program.class_representative(el) == l.program.class_representative(target);
    if !path.is_empty() && same {
        return Some(path.clone());
    }
    if path.iter().position(|&p| p == el).is_some_and(|i| i > 0) {
        return None;
    }
    path.push(el);
    let (sup, mixins, on, interfaces) = direct_supertypes(l, el);
    for s in sup.into_iter().chain(mixins).chain(on).chain(interfaces) {
        if let Some(r) = cycle(l, target, s, path, steps) {
            return Some(r);
        }
    }
    path.pop();
    None
}

/// The cycle of `target` as analyzer 3.13 keeps it (`interfaceCycle`,
/// `summary2/interface_cycles.dart` of main): the strongly connected
/// component the `DependencyWalker` (`_fe_analyzer_shared/lib/src/util/
/// dependency_walker.dart`, Tarjan's algorithm) finds while walking the
/// library's declarations in order, with the dependencies in the order
/// superclass, mixins, interfaces and `on` constraints. The nodes leave the
/// stack from the last visited to the first, and every class of the component
/// gets the same list (`class A extends B`, `B extends C`, `C extends A`:
/// `C, B, A` in all three). `None` if `target` is not in a component of more
/// than one class, or if the hierarchy is too large.
fn component_3_13(l: &Reader<'_>, lib: LibraryId, target: ClassId) -> Option<Vec<ClassId>> {
    struct Walk {
        /// (index, lowest index reached) of the nodes seen in this walk.
        indices: std::collections::HashMap<ClassId, (u32, u32)>,
        evaluated: std::collections::HashSet<ClassId>,
        stack: Vec<ClassId>,
        next: u32,
        steps: u32,
        found: Option<Vec<ClassId>>,
    }
    fn connect(l: &Reader<'_>, a: &mut Walk, node: ClassId, target: ClassId) {
        a.steps += 1;
        let mine = a.next;
        a.next += 1;
        a.indices.insert(node, (mine, mine));
        a.stack.push(node);
        let (sup, mixins, on, interfaces) = direct_supertypes(l, node);
        for d in sup.into_iter().chain(mixins).chain(interfaces).chain(on) {
            if a.steps > 20_000 {
                return;
            }
            if d == node || a.evaluated.contains(&d) {
                continue;
            }
            let seen = match a.indices.get(&d) {
                Some(&(i, _)) => i,
                None => {
                    connect(l, a, d, target);
                    a.indices.get(&d).map_or(u32::MAX, |x| x.1)
                }
            };
            if let Some(eu) = a.indices.get_mut(&node)
                && seen < eu.1
            {
                eu.1 = seen;
            }
        }
        if a.indices.get(&node).is_some_and(|&(i, lowest)| i == lowest) {
            let mut component = Vec::new();
            while let Some(other) = a.stack.pop() {
                component.push(other);
                a.evaluated.insert(other);
                if other == node {
                    break;
                }
            }
            if component.len() > 1 && component.contains(&target) {
                a.found = Some(component);
            }
        }
    }
    let mut a = Walk {
        indices: std::collections::HashMap::new(),
        evaluated: std::collections::HashSet::new(),
        stack: Vec::new(),
        next: 1,
        steps: 0,
        found: None,
    };
    for (i, class) in l.program.classes.iter().enumerate() {
        let id = ClassId(i as u32);
        if class.library != lib || class.decl.is_none() || a.evaluated.contains(&id) {
            continue;
        }
        // Each walk restarts the indices; the nodes of earlier walks are all
        // already evaluated.
        a.indices.clear();
        a.stack.clear();
        a.next = 1;
        connect(l, &mut a, id, target);
        if a.steps > 20_000 {
            return None;
        }
        if a.found.is_some() || a.evaluated.contains(&target) {
            break;
        }
    }
    a.found
}

/// The classes, aliases, enums and mixins of `lib` on which the analyzer's
/// `InheritanceOverrideVerifier.verify()` passes the checks that end it early
/// (`_checkDirectSuperTypes`, `Enum` on a concrete class, recursive
/// inheritance) and reaches the overrides. A declaration repeated in the
/// library or `augment` does not count.
pub fn inheritance_verifier_proceeds(program: &Program, lib: LibraryId, names: &Interner) -> Vec<ClassId> {
    let l = Reader { program, names };
    let consumer = program.library(lib);
    let version = consumer.features.version();
    let enhanced_enums = (version.major, version.minor) >= (2, 17);
    let mut v = Vec::new();
    for (i, class) in program.classes.iter().enumerate() {
        if class.library != lib {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let id = ClassId(i as u32);
        let ast_ = &program.unit(decl.unit).ast;
        if ast_.decl(decl.decl).augment {
            continue;
        }
        let Some(cl) = clauses(&ast_.decl(decl.decl).kind) else { continue };
        // T1.4 step 4: each same-named declaration is checked through its own
        // clauses, with the names resolved by the scope rule (step 1).
        let may_have_enum =
            class.kind == ClassKind::Enum || class.kind == ClassKind::Mixin || class.modifiers.abstract_;
        let mut error = false;
        for &t in cl.implements.iter().chain(cl.on).chain(cl.extends.as_slice()).chain(cl.with) {
            match l.target(decl.unit, ast_, t, 0) {
                ClauseTarget::Class(a) => {
                    if !consumer.is_sdk {
                        if l.is_core_enum(a) && enhanced_enums {
                            error |= !may_have_enum;
                        } else if l.is_forbidden(a) {
                            error = true;
                        }
                    }
                }
                // Not resolved here: not decided.
                ClauseTarget::Unknown => error = true,
                ClauseTarget::NonInterface => {}
            }
        }
        if class.kind == ClassKind::Enum {
            for &t in cl.with {
                if let ClauseTarget::Class(m) = l.target(decl.unit, ast_, t, 0)
                    && l.has_instance_field(m)
                {
                    error = true;
                }
            }
        }
        // `verify()`: a concrete class with `Enum` among its (transitive)
        // supertypes reports `concrete_class_has_enum_superinterface` and stops.
        if class.kind != ClassKind::Enum && class.kind != ClassKind::Mixin && !class.modifiers.abstract_ && !class.modifiers.sealed {
            let mut all = Vec::new();
            supertypes(&l, id, &mut all);
            if all.iter().any(|&s| s != id && l.is_core_enum(s)) {
                error = true;
            }
        }
        if error {
            continue;
        }
        let mut steps = 0;
        if cycle(&l, id, id, &mut Vec::new(), &mut steps).is_some() || steps > 20_000 {
            continue;
        }
        v.push(id);
    }
    v
}

/// `_checkForRepeatedType`: the second interface type with the same element.
fn repeated(
    l: &Reader<'_>,
    u: UnitId,
    ast_: &ast::Ast,
    types: &[ast::TypeId],
    code: Code,
    out: &mut Vec<(UnitId, Diagnostic)>,
) {
    let mut seen: Vec<ClassId> = Vec::new();
    for &t in types {
        if let ClauseTarget::Class(x) = l.target(u, ast_, t, 0) {
            if seen.contains(&x) {
                let name = l.name(l.program.class(x).name);
                out.push((u, Diagnostic::with_code(code, ast_.ty(t).span, [name])));
            } else {
                seen.push(x);
            }
        }
    }
}

/// The `ErrorVerifier` gate for class `id`: [`Gate::Open`] when the following
/// clause checks (modifiers outside the library, `class_used_as_mixin`) would
/// run in the analyzer.
pub fn gate(program: &Program, lib: LibraryId, names: &Interner, id: ClassId) -> Gate {
    let l = Reader { program, names };
    evaluate(&l, lib, id).map_or(Gate::Uncertain, |r| r.gate)
}

/// The inheritance clause diagnostics of library `lib`.
pub fn check(
    program: &Program,
    lib: LibraryId,
    names: &Interner,
) -> Vec<(UnitId, Diagnostic)> {
    let l = Reader { program, names };
    let consumer = program.library(lib);
    let mut out = Vec::new();
    for (i, class) in program.classes.iter().enumerate() {
        if class.library != lib {
            continue;
        }
        let Some(decl) = class.decl else { continue };
        let id = ClassId(i as u32);
        let u = decl.unit;
        let ast_ = &program.unit(u).ast;
        if let DeclKind::ExtensionType(x) = &ast_.decl(decl.decl).kind {
            // `visitExtensionTypeDeclaration`: repeated `implements`, without the gate.
            repeated(&l, u, ast_, &x.implements, c::IMPLEMENTS_REPEATED, &mut out);
            // `_resolveImplementsClause` → `_resolveType` → `_verifyNullability`
            // (3.6.2 `named_type_resolver.dart:388-414`): the nullable type (the
            // written `?` or the nullable alias) in `implements`, on the whole node.
            let params: Vec<SymbolId> = x.type_params.iter().map(|p| p.name.sym).collect();
            for &t in x.implements.iter() {
                let resolved = l.resolve(u, ast_, t, &params, 0, true);
                if l.is_nullable(u, ast_, t, 0) && l.nullability_checked(u, ast_, t, &params, &resolved) {
                    out.push((u, Diagnostic::with_code(c::NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE, ast_.ty(t).span, [] as [&str; 0])));
                }
            }
            continue;
        }
        let Some(cl) = clauses(&ast_.decl(decl.decl).kind) else {
            continue;
        };

        // `_checkForBadFunctionUse` (`error_verifier.dart:2226-2269`): without
        // `class-modifiers` (before 3.0), `dart:core`'s `Function` in the
        // `extends`, in the first `implements` that cites it and in each `with`
        // of a class or class alias.
        if matches!(&ast_.decl(decl.decl).kind, DeclKind::Class(_)) && consumer.features.version().major < 3 {
            let is_function = |t: ast::TypeId| {
                matches!(l.target(u, ast_, t, 0), ClauseTarget::Class(f) if l.program.library(l.program.class(f).library).uri == "dart:core" && l.name(l.program.class(f).name) == "Function")
                    && !l.is_nullable(u, ast_, t, 0)
            };
            let empty: [&str; 0] = [];
            if let Some(t) = cl.extends
                && is_function(t)
            {
                out.push((decl.unit, Diagnostic::with_code(dartforge_diagnostics::codes::warning::DEPRECATED_EXTENDS_FUNCTION, ast_.ty(t).span, empty)));
            }
            if let Some(&t) = cl.implements.iter().find(|&&t| is_function(t)) {
                out.push((decl.unit, Diagnostic::with_code(dartforge_diagnostics::codes::warning::DEPRECATED_IMPLEMENTS_FUNCTION, ast_.ty(t).span, empty)));
            }
            for &t in cl.with {
                if is_function(t) {
                    out.push((decl.unit, Diagnostic::with_code(dartforge_diagnostics::codes::warning::DEPRECATED_MIXIN_FUNCTION, ast_.ty(t).span, empty)));
                }
            }
        }

        // `ResolutionVisitor._resolveType`: each clause type must name a class
        // (or a mixin, outside the `extends`).
        let mixin_application = matches!(&ast_.decl(decl.decl).kind, DeclKind::Class(d) if d.mixin_application);
        let resolution_groups: [(&[ast::TypeId], Code, bool); 4] = [
            (
                cl.extends.as_slice(),
                if mixin_application || !cl.with.is_empty() {
                    c::MIXIN_WITH_NON_CLASS_SUPERCLASS
                } else {
                    c::EXTENDS_NON_CLASS
                },
                false,
            ),
            (cl.with, c::MIXIN_OF_NON_CLASS, true),
            (cl.implements, c::IMPLEMENTS_NON_CLASS, true),
            (cl.on, c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE, true),
        ];
        for (types, code, accepts_mixin) in resolution_groups {
            let outside_on = code != c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE;
            for &t in types {
                let resolved = l.resolve(u, ast_, t, &cl.params, 0, outside_on);
                // `NamedTypeResolver._verifyNullability`: `?` written on a
                // clause type (the whole node).
                if l.is_nullable(u, ast_, t, 0) && l.nullability_checked(u, ast_, t, &cl.params, &resolved) {
                    let nullable_code = if code == c::IMPLEMENTS_NON_CLASS {
                        c::NULLABLE_TYPE_IN_IMPLEMENTS_CLAUSE
                    } else if code == c::MIXIN_OF_NON_CLASS {
                        c::NULLABLE_TYPE_IN_WITH_CLAUSE
                    } else if code == c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE {
                        c::NULLABLE_TYPE_IN_ON_CLAUSE
                    } else {
                        c::NULLABLE_TYPE_IN_EXTENDS_CLAUSE
                    };
                    out.push((decl.unit, Diagnostic::with_code(nullable_code, ast_.ty(t).span, [] as [&str; 0])));
                }
                // `_verifyTypeAliasForContext`: the clause's code, and the type
                // becomes `InvalidType` (no `*_non_class`).
                if resolved == ClauseResolution::ParamAlias {
                    let alias_code = if code == c::IMPLEMENTS_NON_CLASS {
                        c::IMPLEMENTS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else if code == c::MIXIN_OF_NON_CLASS {
                        c::MIXIN_OF_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else if code == c::MIXIN_SUPER_CLASS_CONSTRAINT_NON_INTERFACE {
                        c::MIXIN_ON_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    } else {
                        c::EXTENDS_TYPE_ALIAS_EXPANDS_TO_TYPE_PARAMETER
                    };
                    if let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                        && let (Some(p), Some(n)) = (name.first(), name.last())
                    {
                        let span = dartforge_diagnostics::Span { start: p.span.start, end: n.span.end };
                        out.push((decl.unit, Diagnostic::with_code(alias_code, span, [] as [&str; 0])));
                    }
                    continue;
                }
                let error = match resolved {
                    ClauseResolution::Ignore | ClauseResolution::ParamAlias => false,
                    ClauseResolution::NonClass => true,
                    ClauseResolution::Class(x) => match program.class(x).kind {
                        ClassKind::Class | ClassKind::MixinApplication => false,
                        ClassKind::Mixin => !accepts_mixin,
                        ClassKind::Enum | ClassKind::ExtensionType => true,
                    },
                };
                if error
                    && let TypeKind::Named { name, .. } = &ast_.ty(t).kind
                    && let (Some(p), Some(n)) = (name.first(), name.last())
                {
                    let span = dartforge_diagnostics::Span { start: p.span.start, end: n.span.end };
                    out.push((decl.unit, Diagnostic::with_code(code, span, [] as [&str; 0])));
                }
            }
        }

        // `InheritanceOverrideVerifier._checkDirectSuperTypes`; `direct_error`
        // is the `true` it returns (and that ends `verify()`).
        let mut direct_error = false;
        if !consumer.is_sdk {
            let version = consumer.features.version();
            let enhanced_enums = (version.major, version.minor) >= (2, 17);
            let may_have_enum = class.kind == ClassKind::Enum
                || class.kind == ClassKind::Mixin
                || class.modifiers.abstract_;
            let groups: [(&[ast::TypeId], Code); 4] = [
                (cl.implements, c::IMPLEMENTS_DISALLOWED_CLASS),
                (cl.on, c::MIXIN_SUPER_CLASS_CONSTRAINT_DISALLOWED_CLASS),
                (cl.extends.as_slice(), c::EXTENDS_DISALLOWED_CLASS),
                (cl.with, c::MIXIN_OF_DISALLOWED_CLASS),
            ];
            for (types, code) in groups {
                for &t in types {
                    let ClauseTarget::Class(target) = l.target(u, ast_, t, 0) else {
                        continue;
                    };
                    let span = ast_.ty(t).span;
                    if l.is_core_enum(target) && enhanced_enums {
                        if !may_have_enum {
                            direct_error = true;
                            out.push((
                                decl.unit,
                                Diagnostic::with_code(
                                    c::CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE,
                                    span,
                                    [] as [&str; 0],
                                ),
                            ));
                        }
                        continue;
                    }
                    if l.is_forbidden(target) {
                        direct_error = true;
                    }
                    if l.is_forbidden(target)
                        && let Some(text) = l.display(u, ast_, t, &cl.params)
                    {
                        out.push((
                            decl.unit,
                            Diagnostic::with_code(code, span, [text.as_str()]),
                        ));
                    }
                }
            }
        }

        // `verify()` after the direct supertypes
        // (`inheritance_override.dart:193-204`): the concrete class (not an
        // enum) that implements `Enum` through some supertype, on the name.
        if !direct_error
            && !consumer.is_sdk
            && matches!(class.kind, ClassKind::Class | ClassKind::MixinApplication)
            && !class.modifiers.abstract_
            && !class.modifiers.sealed
            && let DeclKind::Class(d) = &ast_.decl(decl.decl).kind
        {
            let mut all = Vec::new();
            supertypes(&l, id, &mut all);
            if all.iter().any(|&x| x != id && l.is_core_enum(x)) {
                out.push((decl.unit, Diagnostic::with_code(c::CONCRETE_CLASS_HAS_ENUM_SUPERINTERFACE, d.name.span, [] as [&str; 0])));
                direct_error = true;
            }
        }

        // `_checkMixinOfEnum` (`inheritance_override.dart:742-763`): a mixin
        // with an instance field in an enum, on the whole type.
        if class.kind == ClassKind::Enum {
            for &t in cl.with {
                if let ClauseTarget::Class(m) = l.target(u, ast_, t, 0)
                    && l.has_instance_field(m)
                {
                    out.push((decl.unit, Diagnostic::with_code(c::ENUM_MIXIN_WITH_INSTANCE_VARIABLE, ast_.ty(t).span, [] as [&str; 0])));
                    direct_error = true;
                }
            }
        }

        // `InheritanceOverrideVerifier.verify`: a supertype of itself. With the
        // declaration repeated, the same-named one counts as the class itself
        // (`cycle`, T1.1 c).
        if !direct_error && !ast_.decl(decl.decl).augment {
            let mut steps = 0;
            let mut path = Vec::new();
            if let Some(cycle) = cycle(&l, id, id, &mut path, &mut steps) {
                let name = l.name(class.name);
                let span = match &ast_.decl(decl.decl).kind {
                    DeclKind::Class(d) => d.name.span,
                    DeclKind::Enum(d) => d.name.span,
                    DeclKind::Mixin(d) => d.name.span,
                    _ => ast_.decl(decl.decl).span,
                };
                let d = if cycle.len() > 1 {
                    let mut text: Vec<&str> = cycle.iter().map(|&x| l.name(program.class(x).name)).collect();
                    text.push(name);
                    let old = text.join(", ");
                    // The third argument is the path as 3.13.4 writes it (T2,
                    // case c21); only the 3.13 variant uses it.
                    match component_3_13(&l, lib, id) {
                        Some(component) => {
                            let new = component.iter().map(|&x| l.name(program.class(x).name)).collect::<Vec<&str>>().join(", ");
                            Diagnostic::with_code(c::RECURSIVE_INTERFACE_INHERITANCE, span, [name, old.as_str(), new.as_str()])
                        }
                        None => Diagnostic::with_code(c::RECURSIVE_INTERFACE_INHERITANCE, span, [name, old.as_str()]),
                    }
                } else {
                    let (sup, mixins, on, _) = direct_supertypes(&l, id);
                    let code = if sup == Some(id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_EXTENDS
                    } else if on.contains(&id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_ON
                    } else if mixins.contains(&id) {
                        c::RECURSIVE_INTERFACE_INHERITANCE_WITH
                    } else {
                        c::RECURSIVE_INTERFACE_INHERITANCE_IMPLEMENTS
                    };
                    Diagnostic::with_code(code, span, [name])
                };
                out.push((decl.unit, d));
            }
        }

        // `ErrorVerifier._checkForMixinClassErrorCodes`: `mixin class`.
        if matches!(class.kind, ClassKind::Class | ClassKind::MixinApplication)
            && class.modifiers.mixin
        {
            out.extend(
                mixin_class(&l, u, id, ast_, &program.unit(decl.unit).source)
                    .into_iter()
                    .map(|d| (decl.unit, d)),
            );
        }

        // `ErrorVerifier._checkClassInheritance` / `_checkMixinInheritance`.
        let has_clause = cl.extends.is_some()
            || !cl.with.is_empty()
            || !cl.implements.is_empty()
            || !cl.on.is_empty();
        if !has_clause {
            continue;
        }
        let Some(r) = evaluate(&l, lib, id) else {
            continue;
        };
        out.extend(r.reports.into_iter().map(|d| (decl.unit, d)));
        if r.gate != Gate::Open {
            continue;
        }
        if class.kind == ClassKind::Mixin {
            // `_checkMixinInheritance`, after the gate.
            repeated(&l, u, ast_, cl.on, c::ON_REPEATED, &mut out);
            repeated(&l, u, ast_, cl.implements, c::IMPLEMENTS_REPEATED, &mut out);
            continue;
        }
        // `_checkClassInheritance`, after the gate: repeated `implements`, and
        // the superclass again in `implements` or `with`.
        repeated(&l, u, ast_, cl.implements, c::IMPLEMENTS_REPEATED, &mut out);
        // `_checkForNoDefaultSuperConstructorImplicit`, with the gate open.
        if class.kind == ClassKind::Class
            && let DeclKind::Class(x) = &ast_.decl(decl.decl).kind
            && !x.mixin_application
            && !x.members.iter().any(|&m| matches!(ast_.member(m).kind, ast::MemberKind::Constructor(_)))
            && let Some(t) = cl.extends
            && let Some(d) = implicit_super_of_class(&l, u, ast_, class.name, x.name.span, t, &cl.params)
        {
            out.push((decl.unit, d));
        }
        let superclass = match (class.kind, cl.extends) {
            (ClassKind::Enum, _) => l.core_class("Enum"),
            (_, None) => l.core_class("Object"),
            (_, Some(t)) => match l.target(u, ast_, t, 0) {
                ClauseTarget::Class(s) => Some(s),
                _ => None,
            },
        };
        if let Some(s) = superclass {
            for (types, code) in [(cl.implements, c::IMPLEMENTS_SUPER_CLASS), (cl.with, c::MIXINS_SUPER_CLASS)] {
                for &t in types {
                    if l.target(u, ast_, t, 0) == ClauseTarget::Class(s)
                        && let Some(text) = l.display_class(s)
                    {
                        out.push((decl.unit, Diagnostic::with_code(code, ast_.ty(t).span, [text.as_str()])));
                    }
                }
            }
        }
        // After the gate: deferred `extends` and `class_used_as_mixin`.
        if let Some(t) = cl.extends
            && l.is_deferred(u, ast_, t)
        {
            out.push((
                decl.unit,
                Diagnostic::with_code(c::EXTENDS_DEFERRED_CLASS, ast_.ty(t).span, [] as [&str; 0]),
            ));
        }
        for &t in cl.with {
            let ClauseTarget::Class(m) = l.target(u, ast_, t, 0) else {
                continue;
            };
            let target = program.class(m);
            // `ClassElementImpl`: a class or class alias (`class B = A with M`).
            if !matches!(target.kind, ClassKind::Class | ClassKind::MixinApplication)
                || target.modifiers.mixin
            {
                continue;
            }
            let origin = program.library(target.library);
            if origin.features.version().major < 3 {
                continue;
            }
            // `_mayIgnoreClassModifiers`.
            if origin.is_sdk
                && origin.uri != "dart:ffi"
                && (consumer.is_sdk || consumer.features.version().major < 3)
            {
                continue;
            }
            out.push((
                decl.unit,
                Diagnostic::with_code(
                    c::CLASS_USED_AS_MIXIN,
                    ast_.ty(t).span,
                    [names.resolve(target.name)],
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_elements::{load::load_lenient, sdk::SdkLayout};
    use std::fs;
    use std::path::PathBuf;

    /// A minimal SDK with `dart:core`'s forbidden classes and `FutureOr`.
    fn sdk(root: &std::path::Path) -> SdkLayout {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root.join("sdk/lib/core")).unwrap();
        fs::create_dir_all(root.join("sdk/lib/async")).unwrap();
        fs::write(
            root.join("sdk/lib/libraries.json"),
            r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("sdk/lib/core/core.dart"),
            "class Object {} class int {} class String {} class Null {} abstract interface class Enum {} abstract interface class Comparable<T> {}",
        )
        .unwrap();
        fs::write(
            root.join("sdk/lib/async/async.dart"),
            "abstract class FutureOr<T> {}",
        )
        .unwrap();
        SdkLayout::load(&root.join("sdk/lib"), "dartdevc").unwrap()
    }

    /// `(code, fragment, message)` of each diagnostic of `source`.
    fn run(name: &str, source: &str) -> Vec<(String, String, String)> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/clausulas-{name}-{}",
            std::process::id()
        ));
        let sdk = sdk(&root);
        let entry = root.join("main.dart");
        fs::write(&entry, source).unwrap();
        let mut names = Interner::new();
        let (program, _) = load_lenient(&entry, &sdk, None, &mut names);
        let mut v: Vec<(String, String, String)> =
            check(&program, program.entry.unwrap(), &names)
                .into_iter()
                .map(|(_, d)| {
                    (
                        d.code.map_or("", |c| c.info().name).to_string(),
                        source[d.span.start..d.span.end].to_string(),
                        d.message,
                    )
                })
                .collect();
        v.sort();
        fs::remove_dir_all(&root).unwrap();
        v
    }

    fn corpus(path: &str) -> String {
        fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/diagnosticos/analyzer")
                .join(path),
        )
        .unwrap()
    }

    /// The `class_used_as_mixin` cases of the corpus, with what analyzer 3.6.2
    /// reports (`oracle.jsonl`): a class with a written constructor gives
    /// `mixin_class_declares_constructor`, which closes the gate.
    #[test]
    fn class_as_mixin_follows_the_oracle() {
        let cases: [(&str, Option<(&str, &str)>); 6] = [
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside.dart",
                Some(("class_used_as_mixin", "Foo")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_class_hasGener_5838f665.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_enum_hasGenera_724bb742.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_classTypeAlias_69274ad9.dart",
                Some(("mixin_class_declares_constructor", "A")),
            ),
            (
                "class_used_as_mixin/ClassUsedAsMixin__inside_mixinClass.dart",
                None,
            ),
            // `Enum` is forbidden for the gate, and an abstract class may have it.
            (
                "class_used_as_mixin/ClassUsedAsMixin__coreLib_dartCoreEnum.dart",
                None,
            ),
        ];
        for (i, (file, expected)) in cases.into_iter().enumerate() {
            let v = run(&format!("mixin{i}"), &corpus(file));
            match expected {
                Some((code, fragment)) => {
                    assert_eq!(v.len(), 1, "{file}: {v:?}");
                    assert_eq!(
                        (v[0].0.as_str(), v[0].1.as_str()),
                        (code, fragment),
                        "{file}"
                    );
                }
                None => assert!(v.is_empty(), "{file}: {v:?}"),
            }
        }
        // Before class modifiers (`// @dart=2.19`), nothing.
        let legacy_source =
            corpus("class_used_as_mixin/ClassUsedAsMixin__inside_beforeClassModifiers.dart")
                .replace(
                    "// %before-language-feature: class-modifiers",
                    "// @dart=2.19",
                );
        assert!(run("before-modifiers", &legacy_source).is_empty());
    }

    #[test]
    fn imported_legacy_class_may_be_used_as_mixin_in_current_code() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/tmp-agent/clauses-legacy-{}",
            std::process::id()
        ));
        let sdk = sdk(&root);
        fs::write(root.join("legacy.dart"), "// @dart=2.19\nclass Legacy {}\n").unwrap();
        let entry = root.join("main.dart");
        let source = "import 'legacy.dart';\nclass Current {}\nclass A with Legacy {}\nclass B with Current {}\n";
        fs::write(&entry, source).unwrap();
        let mut names = Interner::new();
        let (program, _) = load_lenient(&entry, &sdk, None, &mut names);
        let diags = check(&program, program.entry.unwrap(), &names);
        assert_eq!(diags.len(), 1, "{diags:?}");
        let d = &diags[0].1;
        assert_eq!(&source[d.span.start..d.span.end], "Current");
        assert_eq!(d.code, Some(c::CLASS_USED_AS_MIXIN));
        fs::remove_dir_all(&root).unwrap();
    }

    /// `subtype_of_disallowed_type` with the type as the analyzer displays it:
    /// the alias by name, raw `FutureOr` instantiated to bounds. Checked
    /// against `dart analyze` 3.6.2 (the same shapes are in the corpus'
    /// `extends_disallowed_class`, `mixin_of_disallowed_class` and
    /// `implements_disallowed_class`).
    #[test]
    fn forbidden_type_with_alias_and_bounds() {
        let source = "import 'dart:async';\n\
            typedef T<X> = FutureOr<X>;\n\
            typedef P<X> = X;\n\
            typedef F = FutureOr<int>;\n\
            class M {}\n\
            class A extends int {}\n\
            class B implements F {}\n\
            class C with T<int> {}\n\
            class H with P<int> {}\n\
            class D implements FutureOr {}\n\
            class E<U> extends FutureOr<U> {}\n\
            class G = String with M;\n";
        let v = run("proibido", source);
        let v: Vec<(&str, &str, &str)> = v
            .iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
            .collect();
        // `class G = String with M`: the gate closes and `class_used_as_mixin`
        // does not come out. `with P<int>` is `supertype_expands_to_type_parameter`,
        // not a forbidden type.
        assert_eq!(
            v,
            vec![
                (
                    "subtype_of_disallowed_type",
                    "F",
                    "Classes and mixins can't implement 'F'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "FutureOr",
                    "Classes and mixins can't implement 'FutureOr<dynamic>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "FutureOr<U>",
                    "Classes can't extend 'FutureOr<U>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "String",
                    "Classes can't extend 'String'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "T<int>",
                    "Classes can't mixin 'T<int>'."
                ),
                (
                    "subtype_of_disallowed_type",
                    "int",
                    "Classes can't extend 'int'."
                ),
                (
                    "supertype_expands_to_type_parameter",
                    "P",
                    "A type alias that expands to a type parameter can't be mixed in."
                ),
            ]
        );
    }

    /// `Enum` as a superinterface: only the concrete class reports; none of
    /// the three goes through the gate.
    #[test]
    fn enum_as_superinterface() {
        let v = run(
            "enum",
            "class M {}\nclass A extends Enum {}\nabstract class B implements Enum {}\nabstract class C extends Enum with M {}\n",
        );
        assert_eq!(
            v,
            vec![(
                "concrete_class_has_enum_superinterface".to_string(),
                "Enum".to_string(),
                "Concrete classes can't have 'Enum' as a superinterface.".to_string()
            )]
        );
    }

    /// `mixin class`: a non-trivial constructor and a supertype that is not
    /// `Object`, at the positions of `dart analyze` 3.6.2.
    #[test]
    fn mixin_class_with_constructor_or_supertype() {
        let source = "mixin M1 {}\nmixin M2 {}\nclass A {}\nmixin class B extends A {}\n\
            mixin class C extends Object with M1 {}\nmixin class D = Object with M1, M2;\nmixin class E = Object with M1;\n\
            mixin class F { F(); F.a() {} F.b(int x); factory F.c() => throw 0; }\n";
        let v = run("classe-mixin", source);
        let mut v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        v.sort();
        assert_eq!(
            v,
            vec![
                ("mixin_class_declaration_extends_not_object", "A"),
                ("mixin_class_declaration_extends_not_object", "with M1"),
                ("mixin_class_declaration_extends_not_object", "with M1, M2"),
                ("mixin_class_declares_constructor", "F"),
                ("mixin_class_declares_constructor", "F"),
            ]
        );
    }

    /// An `on` constraint satisfied by the superclass opens the gate; not
    /// satisfied, `mixin_application_not_implemented_interface` closes it.
    #[test]
    fn on_constraint_of_applied_mixin() {
        let v = run(
            "restricao",
            "class S {}\nmixin M on S {}\nclass N {}\nclass A extends S with M, N {}\nclass B with M, N {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(v, vec![("class_used_as_mixin", "N"), ("mixin_application_not_implemented_interface", "M")]);
    }

    /// A class alias with a single mixin does not inherit from another class,
    /// and is itself an ordinary class used as a mixin (`dart analyze` 3.6.2).
    #[test]
    fn class_alias_as_mixin() {
        let v = run(
            "alias",
            "class A {}\nclass B = Object with A;\nclass C extends Object with B {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(
            v,
            vec![("class_used_as_mixin", "A"), ("class_used_as_mixin", "B")]
        );
    }

    /// `mixin_inherits_from_not_object` closes the gate (the
    /// `class_used_as_mixin` of the same type does not come out), and the
    /// superclass with only `factory` constructors.
    #[test]
    fn mixin_that_inherits_and_factory_only_superclass() {
        let source = "class S {}\nmixin class N {}\nclass A extends S {}\nclass B extends Object with N {}\n\
            class C with A {}\nclass D with B {}\nclass F { factory F() => throw 0; }\nclass G extends F {}\n";
        let v = run("herda", source);
        let v: Vec<(&str, &str)> = v.iter().map(|(a, b, _)| (a.as_str(), b.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("mixin_inherits_from_not_object", "A"),
                ("mixin_inherits_from_not_object", "B"),
                ("no_generative_constructors_in_superclass", "F"),
            ]
        );
    }

    /// Repeated clauses and the superclass again in `implements`/`with`
    /// (`implements_repeated`, `on_repeated`, `implements_super_class`), in the
    /// corpus' shapes (3.6.2 oracle): the class name and the displayed element
    /// (`class A`, `mixin class A`, `class Object`).
    #[test]
    fn repeated_and_superclass() {
        let v = run(
            "repetidos",
            "class A {}\nmixin class B {}\ntypedef TA = A;\nclass C extends A implements A, TA {}\nclass D extends B with B {}\nclass E implements Object {}\nmixin M on A, A implements B, B {}\nenum F implements A, A { v }\nextension type X(int it) implements int, int {}\n",
        );
        let expected = [
            ("implements_repeated", "A", "'A' can only be implemented once."),
            ("implements_repeated", "B", "'B' can only be implemented once."),
            ("implements_repeated", "TA", "'A' can only be implemented once."),
            ("implements_repeated", "int", "'int' can only be implemented once."),
            ("implements_super_class", "A", "'class A' can't be used in both the 'extends' and 'implements' clauses."),
            ("implements_super_class", "B", "'mixin class B' can't be used in both the 'extends' and 'with' clauses."),
            ("implements_super_class", "Object", "'class Object' can't be used in both the 'extends' and 'implements' clauses."),
            ("implements_super_class", "TA", "'class A' can't be used in both the 'extends' and 'implements' clauses."),
            ("on_repeated", "A", "The type 'A' can be included in the superclass constraints only once."),
        ];
        let expected: Vec<(String, String, String)> =
            expected.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        assert_eq!(v, expected);
    }

    /// Clause types that do not name a class (`ResolutionVisitor._resolveType`):
    /// a variable, an undefined name, an enum, a type parameter, `dynamic`; a
    /// name from a nonexistent import with a prefix is ignored (`shouldIgnoreUndefined`).
    #[test]
    fn clause_that_does_not_name_a_class() {
        let v = run(
            "not-a-class",
            "import 'does_not_exist.dart' as p;\nint v = 0;\nenum En { a }\nmixin M {}\nclass A extends v {}\nclass B extends Undefined {}\nclass C<T> implements T, En {}\nclass D extends M {}\nclass E with dynamic {}\nclass F extends p.X {}\nclass G = v with M;\nclass H extends v with M {}\n",
        );
        let expected = [
            ("extends_non_class", "M", "Classes can only extend other classes."),
            ("extends_non_class", "Undefined", "Classes can only extend other classes."),
            ("extends_non_class", "v", "Classes can only extend other classes."),
            ("implements_non_class", "En", "Classes and mixins can only implement other classes and mixins."),
            ("implements_non_class", "T", "Classes and mixins can only implement other classes and mixins."),
            ("mixin_of_non_class", "dynamic", "Classes can only mix in mixins and classes."),
            ("mixin_with_non_class_superclass", "v", "Mixin can only be applied to class."),
            ("mixin_with_non_class_superclass", "v", "Mixin can only be applied to class."),
        ];
        let expected: Vec<(String, String, String)> =
            expected.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        assert_eq!(v, expected);
    }

    /// `_checkForRecursiveInterfaceInheritance`: the depth-first search path
    /// (corpus `recursive_interface_inheritance`, 3.6.2 oracle) and the base cases.
    #[test]
    fn supertype_of_itself() {
        let v = run(
            "ciclo",
            "class A extends B {}\nclass B implements C {}\nclass C extends A {}\nclass D extends B {}\nclass E extends E {\n  var foo = 0;\n}\nmixin M on M {}\nclass F with F {}\nclass G implements G {}\n",
        );
        let expected = [
            ("recursive_interface_inheritance", "A", "'A' can't be a superinterface of itself: A, B, C, A."),
            ("recursive_interface_inheritance", "B", "'B' can't be a superinterface of itself: B, C, A, B."),
            ("recursive_interface_inheritance", "C", "'C' can't be a superinterface of itself: C, A, B, C."),
            ("recursive_interface_inheritance", "E", "'E' can't extend itself."),
            ("recursive_interface_inheritance", "F", "'F' can't use itself as a mixin."),
            ("recursive_interface_inheritance", "G", "'G' can't implement itself."),
            ("recursive_interface_inheritance", "M", "'M' can't use itself as a superclass constraint."),
        ];
        let expected: Vec<(String, String, String)> =
            expected.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string())).collect();
        let v: Vec<_> = v.into_iter().filter(|x| x.0 == "recursive_interface_inheritance").collect();
        assert_eq!(v, expected);
    }

    /// `_verifyNullability` on clause types: `?` written or coming from an
    /// alias (corpus `nullable_type_in_*_clause`, 3.6.2 oracle).
    #[test]
    fn nullable_type_in_clause() {
        let v = run(
            "anulavel",
            "class A {}\nmixin M {}\ntypedef B = A?;\nclass C extends A? {}\nclass D implements B {}\nclass E with M? {}\nmixin N on A? {}\n",
        );
        let v: Vec<(&str, &str)> = v.iter().map(|(c, t, _)| (c.as_str(), t.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("nullable_type_in_extends_clause", "A?"),
                ("nullable_type_in_implements_clause", "B"),
                ("nullable_type_in_on_clause", "A?"),
                ("nullable_type_in_with_clause", "M?"),
            ]
        );
    }

    /// `mixin_application_not_implemented_interface` (corpus, 3.6.2 oracle): on
    /// the mixin's name, with the supertype (`Object` omitted) and the constraint.
    #[test]
    fn mixin_without_the_on_constraint() {
        let v = run("restricao-on", "class A {}\nclass C {}\nmixin M on A {}\nclass B with M {}\nclass D extends C with M {}\nclass E extends A with M {}\n");
        let v: Vec<(&str, &str)> = v.iter().filter(|x| x.0 == "mixin_application_not_implemented_interface").map(|x| (x.1.as_str(), x.2.as_str())).collect();
        assert_eq!(
            v,
            vec![
                ("M", "'M' can't be mixed onto 'C' because 'C' doesn't implement 'A'."),
                ("M", "'M' can't be mixed onto 'Object' because 'Object' doesn't implement 'A'."),
            ]
        );
    }
}
