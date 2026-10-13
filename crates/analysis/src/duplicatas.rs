//! Names defined more than once: the `DuplicateDefinitionVerifier` and the
//! `MemberDuplicateDefinitionVerifier` of analyzer 6.11.0
//! (`src/error/duplicate_definition_verifier.dart`), called by the
//! `ErrorVerifier` in `visitBlock`, `visitSwitchCase`,
//! `visitForPartsWithDeclarations`, `visitFormalParameterList`,
//! `visitTypeParameterList`, `visitCatchClause` and `visitCompilationUnit`,
//! and by the `LibraryAnalyzer` for the members of each type.
//!
//! The model is the analyzer's: each name enters a scope of getters (and, for
//! setters, one of setters); a field or top-level variable induces a getter
//! and, if it is neither `final` nor `const`, a setter; a getter and a setter
//! with the same name form a pair and do not collide. The error goes on the
//! name of the **second** declaration.

use crate::Unidade;
use dartforge_diagnostics::codes::compile_time_error as c;
use dartforge_diagnostics::{Code, Diagnostic, Span};
use dartforge_frontend::ast::{
    self, Ast, DeclKind, DirectiveKind, ForInit, FunctionKind, ListPatternElement, MemberId, MemberKind,
    Parameter, PatternId, PatternKind, StmtKind, TypeKind, TypeParameter, TypedefKind,
};
use dartforge_intern::Interner;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ElemKind {
    Getter,
    Setter,
    Method,
    Other,
}

/// What the analyzer knows about the previous element to choose the code.
#[derive(Clone, Copy)]
struct Elem {
    kind: ElemKind,
    /// A synthetic accessor of a field (`CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD`).
    from_field: bool,
    /// A `this.x` parameter (`DUPLICATE_FIELD_FORMAL_PARAMETER`).
    field_formal: bool,
    /// The name of the first declaration, for the context message (empty
    /// when not known).
    at: Span,
    /// The unit (index in `units`) of `at`.
    unit: usize,
}

const OTHER: Elem = Elem { kind: ElemKind::Other, from_field: false, field_formal: false, at: Span { start: 0, end: 0 }, unit: 0 };

type Scope = HashMap<String, Elem>;

struct Reporter<'a> {
    interner: &'a Interner,
    unit: usize,
    out: Vec<(usize, Diagnostic)>,
    /// Ver [`duplicatas`].
    merge: bool,
    /// The `source.fullName` of each unit, for the context that points at
    /// another file (empty: without that context).
    paths: &'a [String],
}

impl Reporter<'_> {
    fn name(&self, n: ast::Name) -> String {
        self.interner.resolve(n.sym).to_string()
    }

    fn report(&mut self, code: Code, span: Span, args: &[&str]) {
        self.out.push((self.unit, Diagnostic::with_code(code, span, args.iter().copied())));
    }

    /// `DiagnosticFactory.duplicateDefinition`: the first definition goes as
    /// a context message, on its name (in its file, if it is another one).
    fn with_first(&self, d: Diagnostic, previous: Elem) -> Diagnostic {
        const FIRST_DEFINITION: &str = "The first definition of this name.";
        if previous.at.start == previous.at.end {
            return d;
        }
        if previous.unit == self.unit {
            return d.with_context(previous.at, FIRST_DEFINITION);
        }
        let Some(path) = self.paths.get(previous.unit) else { return d };
        let mut d = d;
        d.context.push(dartforge_diagnostics::ContextMessage { file: Some(path.as_str().into()), span: previous.at, message: FIRST_DEFINITION.into() });
        d
    }

    /// `_checkDuplicateIdentifier` for an element that induces no property.
    fn check(&mut self, getters: &mut Scope, setters: Option<&mut Scope>, name: &str, span: Span, e: Elem) {
        // `identifier.isSynthetic`: the parser's recovery does not create a
        // truly empty name; an empty range is the equivalent here.
        if span.start == span.end {
            return;
        }
        match getters.get(name).copied() {
            Some(previous) => {
                let pair = matches!((previous.kind, e.kind), (ElemKind::Getter, ElemKind::Setter) | (ElemKind::Setter, ElemKind::Getter));
                if !pair {
                    let code = if previous.field_formal && e.field_formal {
                        c::DUPLICATE_FIELD_FORMAL_PARAMETER
                    } else {
                        c::DUPLICATE_DEFINITION
                    };
                    let d = self.with_first(Diagnostic::with_code(code, span, [name]), previous);
                    self.out.push((self.unit, d));
                }
            }
            None => {
                getters.insert(name.to_string(), Elem { at: span, unit: self.unit, ..e });
            }
        }
        if let Some(setters) = setters {
            if e.kind == ElemKind::Setter {
                match setters.get(name).copied() {
                    Some(previous) => {
                        let d = self.with_first(Diagnostic::with_code(c::DUPLICATE_DEFINITION, span, [name]), previous);
                        self.out.push((self.unit, d));
                    }
                    None => {
                        setters.insert(name.to_string(), Elem { at: span, unit: self.unit, ..e });
                    }
                }
            }
        }
    }

    /// A field or top-level variable: the getter and, if mutable, the setter.
    fn check_property(
        &mut self,
        getters: &mut Scope,
        mut setters: Option<&mut Scope>,
        name: &str,
        span: Span,
        mutavel: bool,
    ) {
        let g = Elem { kind: ElemKind::Getter, from_field: true, ..OTHER };
        self.check(getters, setters.as_deref_mut(), name, span, g);
        if mutavel {
            let s = Elem { kind: ElemKind::Setter, from_field: true, ..OTHER };
            self.check(getters, setters, name, span, s);
        }
    }
}

fn function_elem_kind(k: FunctionKind) -> ElemKind {
    match k {
        FunctionKind::Getter => ElemKind::Getter,
        FunctionKind::Setter => ElemKind::Setter,
        FunctionKind::Function | FunctionKind::Operator => ElemKind::Method,
    }
}

/// Every duplicate-name diagnostic of a library, with the index of the unit
/// each one falls in. `wildcard`: the library has `wildcard-variables`
/// (3.7+), and a local `_` declares nothing.
///
/// `merge_same_location`: the behaviour of analyzer 6.11 (Dart 3.6.2), where
/// two declarations with the same name and kind in the same unit are the same
/// element and the members of one are checked against those of the other
/// (`ElementKey`). 3.13.4 does not merge (checked with both SDKs): pass
/// `false` for a library whose reference is 3.13.4
/// (`dartforge_elements::Program::referencia_da_biblioteca`, or
/// `dartforge_frontend::parser::Parsed::referencia` for a single file).
pub fn duplicatas(
    units: &[Unidade<'_>],
    interner: &Interner,
    wildcard: bool,
    merge_same_location: bool,
) -> Vec<(usize, Diagnostic)> {
    duplicates_with_paths(units, &[], interner, wildcard, merge_same_location)
}

/// [`duplicatas`] with each unit's path: the first definition in an earlier
/// unit of the library becomes context in its file.
pub fn duplicates_with_paths(
    units: &[Unidade<'_>],
    paths: &[String],
    interner: &Interner,
    wildcard: bool,
    merge_same_location: bool,
) -> Vec<(usize, Diagnostic)> {
    let mut rel = Reporter { interner, unit: 0, out: Vec::new(), merge: merge_same_location, paths };
    let library_names = top_level_names(units, interner);
    for (i, u) in units.iter().enumerate() {
        rel.unit = i;
        top_level_unit(&mut rel, units, i, &library_names);
        locals(&mut rel, u.ast, wildcard);
    }
    // Members: first the instance ones of every unit, then the static ones
    // (`MemberDuplicateDefinitionVerifier.checkLibrary`).
    let mut contexts: Vec<(Vec<DeclContext>, HashMap<ElementKey, Scopes>)> = Vec::new();
    for (i, u) in units.iter().enumerate() {
        rel.unit = i;
        contexts.push(unit_members(&mut rel, u));
    }
    for (i, u) in units.iter().enumerate() {
        rel.unit = i;
        unit_statics(&mut rel, u, &contexts[i].0, &contexts[i].1);
    }
    rel.out
}

/// Names declared at the top level of the library (`libraryDeclarations`).
fn top_level_names(units: &[Unidade<'_>], interner: &Interner) -> HashSet<String> {
    let mut s = HashSet::new();
    for u in units {
        for &d in &u.unit.declarations {
            let name = |n: ast::Name| interner.resolve(n.sym).to_string();
            match &u.ast.decl(d).kind {
                DeclKind::Class(x) => {
                    s.insert(name(x.name));
                }
                DeclKind::Mixin(x) => {
                    s.insert(name(x.name));
                }
                DeclKind::Enum(x) => {
                    s.insert(name(x.name));
                }
                DeclKind::Extension(x) => {
                    if let Some(n) = x.name {
                        s.insert(name(n));
                    }
                }
                DeclKind::ExtensionType(x) => {
                    s.insert(name(x.name));
                }
                DeclKind::Typedef(x) => {
                    s.insert(name(x.name));
                }
                DeclKind::Function(f) => {
                    if let Some(n) = u.ast.function(*f).name {
                        s.insert(name(n));
                    }
                }
                DeclKind::Variables(vl) => {
                    for v in vl.variables.iter() {
                        s.insert(name(v.name));
                    }
                }
            }
        }
    }
    s
}

/// `addWithoutChecking`: what the earlier units already declared. `j` is the
/// unit's index, for the context of the first definition.
fn without_checking(rel: &Reporter<'_>, j: usize, u: &Unidade<'_>, getters: &mut Scope) {
    let in_ = |n: ast::Name| Elem { at: n.span, unit: j, ..OTHER };
    let mut variables = Vec::new();
    for &d in &u.unit.declarations {
        match &u.ast.decl(d).kind {
            DeclKind::Function(f) => {
                let f = u.ast.function(*f);
                let Some(n) = f.name else { continue };
                let name = rel.name(n);
                match f.kind {
                    FunctionKind::Getter => {
                        getters.insert(name, Elem { kind: ElemKind::Getter, ..in_(n) });
                    }
                    FunctionKind::Setter => {
                        getters.insert(format!("{name}="), Elem { kind: ElemKind::Setter, ..in_(n) });
                    }
                    _ => {
                        getters.insert(name, in_(n));
                    }
                }
            }
            DeclKind::Variables(vl) => variables.push(vl),
            DeclKind::Class(x) => {
                getters.insert(rel.name(x.name), in_(x.name));
            }
            DeclKind::Mixin(x) => {
                getters.insert(rel.name(x.name), in_(x.name));
            }
            DeclKind::Enum(x) => {
                getters.insert(rel.name(x.name), in_(x.name));
            }
            DeclKind::Extension(x) => {
                if let Some(n) = x.name {
                    getters.insert(rel.name(n), in_(n));
                }
            }
            DeclKind::ExtensionType(x) => {
                getters.insert(rel.name(x.name), in_(x.name));
            }
            DeclKind::Typedef(x) => {
                getters.insert(rel.name(x.name), in_(x.name));
            }
        }
    }
    // Variables override the synthetic accessors, as in the analyzer.
    for vl in variables {
        for v in vl.variables.iter() {
            let name = rel.name(v.name);
            if !vl.final_ && !vl.const_ {
                getters.insert(format!("{name}="), in_(v.name));
            }
            getters.insert(name, in_(v.name));
        }
    }
}

/// `checkUnit`: top-level declarations and import prefixes.
fn top_level_unit(rel: &mut Reporter<'_>, units: &[Unidade<'_>], i: usize, names: &HashSet<String>) {
    let u = units[i];
    // PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER, once per prefix.
    let mut prefixes = HashSet::new();
    for d in &u.unit.directives {
        if let DirectiveKind::Import { prefix: Some(p), .. } = &d.kind {
            let name = rel.name(*p);
            if prefixes.insert(name.clone()) && names.contains(&name) {
                rel.report(c::PREFIX_COLLIDES_WITH_TOP_LEVEL_MEMBER, p.span, &[&name]);
            }
        }
    }
    let mut getters = Scope::new();
    let mut setters = Scope::new();
    if i > 0 {
        for (j, previous) in units[..i].iter().enumerate() {
            without_checking(rel, j, previous, &mut getters);
        }
    }
    for &d in &u.unit.declarations {
        let (name, e) = match &u.ast.decl(d).kind {
            DeclKind::Class(x) => (x.name, OTHER),
            DeclKind::Mixin(x) => (x.name, OTHER),
            DeclKind::Enum(x) => (x.name, OTHER),
            DeclKind::ExtensionType(x) => (x.name, OTHER),
            DeclKind::Typedef(x) => (x.name, OTHER),
            DeclKind::Extension(x) => match x.name {
                Some(n) => (n, OTHER),
                None => continue,
            },
            DeclKind::Function(f) => {
                let f = u.ast.function(*f);
                let Some(n) = f.name else { continue };
                let kind = match f.kind {
                    FunctionKind::Getter => ElemKind::Getter,
                    FunctionKind::Setter => ElemKind::Setter,
                    _ => ElemKind::Other,
                };
                (n, Elem { kind, ..OTHER })
            }
            DeclKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    let name = rel.name(v.name);
                    rel.check_property(&mut getters, Some(&mut setters), &name, v.name.span, !vl.final_ && !vl.const_);
                }
                continue;
            }
        };
        let text = rel.name(name);
        rel.check(&mut getters, Some(&mut setters), &text, name.span, e);
    }
}

// ---------------------------------------------------------------------------
// Bodies: blocks, `for`, parameters, type parameters, `catch`.

fn is_wildcard(wildcard: bool, name: &str) -> bool {
    wildcard && name == "_"
}

fn locals(rel: &mut Reporter<'_>, ast: &Ast, wildcard: bool) {
    for s in &ast.stmts {
        match &s.kind {
            StmtKind::Block(list) => stmts(rel, ast, list, wildcard),
            StmtKind::Switch { cases, .. } => {
                for case in cases.iter() {
                    stmts(rel, ast, &case.body, wildcard);
                }
            }
            StmtKind::For { init: Some(ForInit::Variables(vl)), .. } => {
                let mut scope = Scope::new();
                for v in vl.variables.iter() {
                    let name = rel.name(v.name);
                    if !is_wildcard(wildcard, &name) {
                        rel.check(&mut scope, None, &name, v.name.span, OTHER);
                    }
                }
            }
            StmtKind::Try { catches, .. } => {
                for cc in catches.iter() {
                    if let (Some(e), Some(st)) = (cc.exception, cc.stack_trace) {
                        let name = rel.name(e);
                        if !is_wildcard(wildcard, &name) && name == rel.name(st) {
                            rel.report(c::DUPLICATE_DEFINITION, st.span, &[&name]);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Parameter and type parameter lists.
    for f in &ast.functions {
        check_type_params(rel, &f.type_params, wildcard);
        if let Some(ps) = &f.parameters {
            check_params(rel, ps, wildcard);
        }
    }
    for m in &ast.members {
        if let MemberKind::Constructor(k) = &m.kind {
            check_params(rel, &k.parameters, wildcard);
        }
    }
    for d in &ast.decls {
        match &d.kind {
            DeclKind::Class(x) => check_type_params(rel, &x.type_params, wildcard),
            DeclKind::Mixin(x) => check_type_params(rel, &x.type_params, wildcard),
            DeclKind::Enum(x) => check_type_params(rel, &x.type_params, wildcard),
            DeclKind::Extension(x) => check_type_params(rel, &x.type_params, wildcard),
            DeclKind::ExtensionType(x) => check_type_params(rel, &x.type_params, wildcard),
            DeclKind::Typedef(x) => {
                check_type_params(rel, &x.type_params, wildcard);
                if let TypedefKind::Legacy { parameters, .. } = &x.kind {
                    check_params(rel, parameters, wildcard);
                }
            }
            _ => {}
        }
    }
    for t in &ast.types {
        if let TypeKind::Function { type_params, parameters, .. } = &t.kind {
            check_type_params(rel, type_params, wildcard);
            check_params(rel, parameters, wildcard);
        }
    }
}

/// `checkStatements`: variables, local functions and pattern variables of the same list.
fn stmts(rel: &mut Reporter<'_>, ast: &Ast, list: &[ast::StmtId], wildcard: bool) {
    let mut scope = Scope::new();
    for &id in list {
        match &ast.stmt(id).kind {
            StmtKind::Variables(vl) => {
                for v in vl.variables.iter() {
                    let name = rel.name(v.name);
                    if !is_wildcard(wildcard, &name) {
                        rel.check(&mut scope, None, &name, v.name.span, OTHER);
                    }
                }
            }
            StmtKind::Function(f) => {
                if let Some(n) = ast.function(*f).name {
                    let name = rel.name(n);
                    if !is_wildcard(wildcard, &name) {
                        rel.check(&mut scope, None, &name, n.span, OTHER);
                    }
                }
            }
            StmtKind::PatternVariables { pattern, .. } => {
                let mut names = Vec::new();
                pattern_variables(ast, *pattern, &mut names);
                // A repetition inside the pattern is `DUPLICATE_VARIABLE_PATTERN`
                // (`VariableBinder`, `c2_syntactic`), and the second occurrence
                // does not become an element: only the first of each name
                // enters the block's scope.
                let mut seen = Vec::new();
                names.retain(|n| {
                    if seen.contains(&n.sym) {
                        return false;
                    }
                    seen.push(n.sym);
                    true
                });
                for n in names {
                    let name = rel.name(n);
                    if !is_wildcard(wildcard, &name) {
                        rel.check(&mut scope, None, &name, n.span, OTHER);
                    }
                }
            }
            _ => {}
        }
    }
}

fn pattern_variables(ast: &Ast, p: PatternId, out: &mut Vec<ast::Name>) {
    match &ast.pattern(p).kind {
        PatternKind::Variable { name, .. } => out.push(*name),
        PatternKind::Or(a, b) | PatternKind::And(a, b) => {
            pattern_variables(ast, *a, out);
            pattern_variables(ast, *b, out);
        }
        PatternKind::NullCheck(a) | PatternKind::NullAssert(a) | PatternKind::Parenthesized(a) => {
            pattern_variables(ast, *a, out)
        }
        PatternKind::Cast { pattern, .. } => pattern_variables(ast, *pattern, out),
        PatternKind::List { elements, .. } => {
            for e in elements.iter() {
                match e {
                    ListPatternElement::Pattern(x) | ListPatternElement::Rest(Some(x)) => pattern_variables(ast, *x, out),
                    ListPatternElement::Rest(None) => {}
                }
            }
        }
        PatternKind::Map { entries, .. } => {
            for e in entries.iter() {
                pattern_variables(ast, e.value, out);
            }
        }
        PatternKind::Record { fields } | PatternKind::Object { fields, .. } => {
            for f in fields.iter() {
                pattern_variables(ast, f.pattern, out);
            }
        }
        PatternKind::Wildcard { .. } | PatternKind::Constant(_) | PatternKind::Relational { .. } => {}
    }
}

/// `checkParameters`, and the same in the lists of old-style function-typed parameters.
fn check_params(rel: &mut Reporter<'_>, ps: &[Parameter], wildcard: bool) {
    let mut scope = Scope::new();
    for p in ps {
        if let Some(n) = p.name {
            let name = rel.name(n);
            // `super._` with a wildcard; and a plain `_`, which declares nothing.
            if !(is_wildcard(wildcard, &name) && !p.this_) {
                // `getDiagnostic` (3.13, `duplicate_definition_verifier.dart:442-451`
                // of the main checkout): only two syntactic `FieldFormalParameter`s;
                // the declaring one is a `SimpleFormalParameter`.
                let e = Elem { field_formal: p.this_ && !p.declaring, ..OTHER };
                rel.check(&mut scope, None, &name, n.span, e);
            }
        }
        check_type_params(rel, &p.function_type_params, wildcard);
        if let Some(fp) = &p.function_parameters {
            check_params(rel, fp, wildcard);
        }
    }
}

fn check_type_params(rel: &mut Reporter<'_>, tps: &[TypeParameter], wildcard: bool) {
    let mut scope = Scope::new();
    for t in tps {
        let name = rel.name(t.name);
        if !is_wildcard(wildcard, &name) {
            rel.check(&mut scope, None, &name, t.name.span, OTHER);
        }
    }
}

// ---------------------------------------------------------------------------
// Members of classes, mixins, enums, extensions and extension types.

#[derive(Default)]
struct DeclContext {
    /// The declaration's name (`None` for an anonymous extension).
    name: Option<String>,
    kind: DeclKindTag,
    /// The kind of the analyzer's element (`ElementKind`): class, mixin, enum,
    /// extension, extension type. With the name, it is the element's location
    /// in the unit.
    element_kind: &'static str,
    constructors: HashSet<String>,
    ig: Scope,
    is: Scope,
    sg: Scope,
    ss: Scope,
    members: Vec<MemberId>,
    /// The name of an extension type's primary constructor
    /// (`extension type A.foo(int it)`), which is one of the element's
    /// `constructors` for `_checkConflictingConstructorAndStatic`.
    named_primary: Option<(String, Span)>,
}

/// The member names of an element (`_InstanceElementContext`).
#[derive(Default)]
struct Scopes {
    constructors: HashSet<String>,
    ig: Scope,
    is: Scope,
    sg: Scope,
    ss: Scope,
}

/// The element key: 6.11's location (`ElementImpl.==` compares the kind and
/// the location, `an611:src/dart/element/element.dart:2877-2884`), which is
/// the chain of names — two declarations with the same name and kind in the
/// same unit are **the same** element, and the `DuplicateDefinitionVerifier`
/// merges their members (`_getElementContext`,
/// `src/error/duplicate_definition_verifier.dart:808-811`). An anonymous
/// extension does not merge (each one has its own reference).
type ElementKey = (&'static str, String);

impl DeclContext {
    fn key(&self) -> Option<ElementKey> {
        self.name.clone().map(|n| (self.element_kind, n))
    }

    /// The key, when declarations with the same location merge.
    fn key_if(&self, merge: bool) -> Option<ElementKey> {
        // 6.11 does not merge two extensions with the same name (`augment
        // extension E` without the experiment becomes a second `extension E`):
        // only the repeated name is reported, not the members of one against the other.
        if merge && self.element_kind != "extension" { self.key() } else { None }
    }

    /// Receives the names already accumulated by the element.
    fn resume(&mut self, e: Scopes) {
        self.constructors.extend(e.constructors);
        self.ig.extend(e.ig);
        self.is.extend(e.is);
        self.sg.extend(e.sg);
        self.ss.extend(e.ss);
    }

    /// Returns the accumulated names for the next declaration of the same element.
    fn save(&mut self) -> Scopes {
        Scopes {
            constructors: std::mem::take(&mut self.constructors),
            ig: std::mem::take(&mut self.ig),
            is: std::mem::take(&mut self.is),
            sg: std::mem::take(&mut self.sg),
            ss: std::mem::take(&mut self.ss),
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DeclKindTag {
    #[default]
    Class,
    Enum,
    Extension,
}

/// The members of the unit's declarations, with the names accumulated per
/// element (see [`ElementKey`]); returns each declaration's context and each
/// element's final names.
fn unit_members(rel: &mut Reporter<'_>, u: &Unidade<'_>) -> (Vec<DeclContext>, HashMap<ElementKey, Scopes>) {
    let mut v = Vec::new();
    let mut elements: HashMap<ElementKey, Scopes> = HashMap::new();
    for &d in &u.unit.declarations {
        let mut ctx = DeclContext::default();
        // The kind and the name first: the element's names come before any
        // member of this declaration.
        let (kind, name) = match &u.ast.decl(d).kind {
            DeclKind::Class(x) => ("class", Some(x.name)),
            DeclKind::Mixin(x) => ("mixin", Some(x.name)),
            DeclKind::Extension(x) => ("extension", x.name),
            DeclKind::ExtensionType(x) => ("extension type", Some(x.name)),
            DeclKind::Enum(x) => ("enum", Some(x.name)),
            _ => continue,
        };
        ctx.element_kind = kind;
        ctx.name = name.map(|n| rel.name(n));
        if let Some(e) = ctx.key_if(rel.merge).and_then(|k| elements.remove(&k)) {
            ctx.resume(e);
        }
        match &u.ast.decl(d).kind {
            DeclKind::Class(x) => {
                ctx.members = x.members.clone();
            }
            DeclKind::Mixin(x) => {
                ctx.members = x.members.clone();
            }
            DeclKind::Extension(x) => {
                ctx.kind = DeclKindTag::Extension;
                ctx.members = x.members.clone();
            }
            DeclKind::ExtensionType(x) => {
                // `A.new(int it)` is the unnamed constructor.
                ctx.constructors.insert(x.constructor.map(|n| rel.name(n)).filter(|n| n != "new").unwrap_or_default());
                if let Some(n) = x.constructor {
                    let name = rel.name(n);
                    if name != "new" {
                        ctx.named_primary = Some((name, n.span));
                    }
                }
                let n = x.representation_name;
                ctx.ig.insert(rel.name(n), Elem { kind: ElemKind::Getter, from_field: true, at: n.span, unit: rel.unit, ..OTHER });
                ctx.members = x.members.clone();
            }
            DeclKind::Enum(x) => {
                let name = rel.name(x.name);
                ctx.kind = DeclKindTag::Enum;
                for k in &x.constants {
                    let n = rel.name(k.name);
                    if n == name {
                        rel.report(c::ENUM_CONSTANT_SAME_NAME_AS_ENCLOSING, k.name.span, &[]);
                    }
                    let e = Elem { kind: ElemKind::Getter, from_field: true, ..OTHER };
                    rel.check(&mut ctx.sg, None, &n, k.name.span, e);
                    if n == "values" {
                        rel.report(c::VALUES_DECLARATION_IN_ENUM, k.name.span, &[]);
                    }
                }
                ctx.name = Some(name);
                ctx.members = x.members.clone();
            }
            _ => continue,
        }
        members(rel, u.ast, &mut ctx);
        if let DeclKind::Enum(x) = &u.ast.decl(d).kind {
            if ctx.name.as_deref() == Some("values") {
                rel.report(c::ENUM_WITH_NAME_VALUES, x.name.span, &[]);
            }
        }
        if let Some(k) = ctx.key_if(rel.merge) {
            elements.insert(k, ctx.save());
        }
        v.push(ctx);
    }
    (v, elements)
}

/// `_checkClassMembers`.
fn members(rel: &mut Reporter<'_>, ast: &Ast, ctx: &mut DeclContext) {
    let is_enum = ctx.kind == DeclKindTag::Enum;
    let mut named: Vec<(String, Span)> = ctx.named_primary.take().into_iter().collect();
    for &m in &ctx.members {
        match &ast.member(m).kind {
            MemberKind::Constructor(k) => {
                if k.primary_body_part || ctx.name.as_deref() != Some(rel.name(k.class_name).as_str()) {
                    continue;
                }
                let mut name = k.name.map(|n| rel.name(n)).unwrap_or_default();
                if name == "new" {
                    name.clear();
                }
                if let Some(n) = k.name {
                    if !name.is_empty() {
                        named.push((name.clone(), n.span));
                    }
                }
                if !ctx.constructors.insert(name.clone()) {
                    // `atConstructorDeclaration`: from the class name to the end of the name.
                    let end = k.name.unwrap_or(k.class_name).span.end;
                    let span = Span { start: k.class_name.span.start, end };
                    if name.is_empty() {
                        rel.report(c::DUPLICATE_CONSTRUCTOR_DEFAULT, span, &[]);
                    } else {
                        rel.report(c::DUPLICATE_CONSTRUCTOR_NAME, span, &[&name]);
                    }
                }
            }
            MemberKind::Field(vl) => {
                for v in vl.variables.iter() {
                    let name = rel.name(v.name);
                    let (g, s) = if vl.static_ { (&mut ctx.sg, &mut ctx.ss) } else { (&mut ctx.ig, &mut ctx.is) };
                    rel.check_property(g, Some(s), &name, v.name.span, !vl.final_ && !vl.const_);
                    if is_enum && name == "values" {
                        rel.report(c::VALUES_DECLARATION_IN_ENUM, v.name.span, &[]);
                    }
                }
            }
            MemberKind::Method(f) => {
                let f = ast.function(*f);
                let Some(n) = f.name else { continue };
                let mut name = rel.name(n);
                // `operator -()` is the analyzer's `unary-`, a name other than
                // the binary `operator -(x)`: both coexist.
                if f.kind == FunctionKind::Operator && name == "-" && f.parameters.as_ref().is_some_and(|p| p.is_empty()) {
                    name = "unary-".to_string();
                }
                let kind = function_elem_kind(f.kind);
                let (g, s) = if f.static_ { (&mut ctx.sg, &mut ctx.ss) } else { (&mut ctx.ig, &mut ctx.is) };
                rel.check(g, Some(s), &name, n.span, Elem { kind, ..OTHER });
                if is_enum && name == "values" && !(f.static_ && kind == ElemKind::Setter) {
                    rel.report(c::VALUES_DECLARATION_IN_ENUM, n.span, &[]);
                }
            }
        }
    }
    // `_checkConflictingConstructorAndStatic` (types with an interface).
    if ctx.kind != DeclKindTag::Extension {
        for (name, span) in named {
            let is_static = ctx.sg.get(&name).or_else(|| ctx.ss.get(&name)).copied();
            let code = match is_static {
                Some(e) if e.from_field => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_FIELD),
                Some(Elem { kind: ElemKind::Getter, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_GETTER),
                Some(Elem { kind: ElemKind::Setter, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_SETTER),
                Some(Elem { kind: ElemKind::Method, .. }) => Some(c::CONFLICTING_CONSTRUCTOR_AND_STATIC_METHOD),
                _ => None,
            };
            if let Some(code) = code {
                rel.report(code, span, &[&name]);
            }
        }
    }
}

/// `_checkClassStatic` / `_checkExtensionStatic`: a static member with the
/// name of an instance member declared there. (The `_checkEnumStatic`, through
/// the enum's interface, is `types::sobrescritas::estaticos_de_enum`.)
fn unit_statics(rel: &mut Reporter<'_>, u: &Unidade<'_>, contexts: &[DeclContext], elements: &HashMap<ElementKey, Scopes>) {
    let empty = Scopes::default();
    for ctx in contexts {
        if ctx.kind == DeclKindTag::Enum {
            continue;
        }
        // The instance names of the whole element (every declaration with the
        // same location), as in the analyzer, which checks the static members
        // after every instance member.
        let (ig, is) = match ctx.key_if(rel.merge) {
            Some(k) => {
                let e = elements.get(&k).unwrap_or(&empty);
                (&e.ig, &e.is)
            }
            None => (&ctx.ig, &ctx.is),
        };
        for &m in &ctx.members {
            let names: Vec<ast::Name> = match &u.ast.member(m).kind {
                MemberKind::Field(vl) if vl.static_ => vl.variables.iter().map(|v| v.name).collect(),
                MemberKind::Method(f) if u.ast.function(*f).static_ => u.ast.function(*f).name.into_iter().collect(),
                _ => continue,
            };
            for n in names {
                let name = rel.name(n);
                if ig.contains_key(&name) || is.contains_key(&name) {
                    match (ctx.kind, &ctx.name) {
                        (DeclKindTag::Extension, _) => rel.report(c::EXTENSION_CONFLICTING_STATIC_AND_INSTANCE, n.span, &[&name]),
                        (_, Some(class)) => {
                            let class = class.clone();
                            rel.report(c::CONFLICTING_STATIC_AND_INSTANCE, n.span, &[&class, &name, &class]);
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> Vec<(String, usize, usize)> {
        let mut interner = Interner::new();
        let p = dartforge_frontend::parser::parse(source, &mut interner);
        let u = Unidade { ast: &p.ast, unit: &p.unit, fonte: source };
        let mut v: Vec<_> = duplicatas(&[u], &interner, false, true)
            .into_iter()
            .map(|(_, d)| (d.code.unwrap().info().name.to_string(), d.span.start, d.span.end))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn top_level_members_and_locals() {
        let f = "int a = 1;\nvoid a() {}\nint get g => 1;\nset g(int v) {}\nclass C {\n  int x = 0;\n  void x() {}\n  C();\n  C();\n  static int y = 0;\n  int get y => 1;\n}\nvoid m(int p, int p) {\n  var q = 1;\n  var q = 2;\n  try {} catch (e, e) {}\n}\n";
        let r = run(f);
        let codigos: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(
            codigos.iter().filter(|c| **c == "duplicate_definition").count(),
            5,
            "{r:?}"
        );
        assert!(codigos.contains(&"duplicate_constructor"), "{r:?}");
        assert!(codigos.contains(&"conflicting_static_and_instance"), "{r:?}");
        // the error falls on the second declaration: `void a()` (offset 16)
        assert!(r.contains(&("duplicate_definition".to_string(), 16, 17)), "{r:?}");
    }

    #[test]
    fn getter_and_setter_do_not_collide() {
        assert!(run("int get g => 1;\nset g(int v) {}\nclass C { int get x => 1; set x(int v) {} }\n").is_empty());
        let r = run("class C { int x = 0; set x(int v) {} }\n");
        assert_eq!(r.len(), 1, "{r:?}");
    }

    #[test]
    fn unary_and_binary_minus_coexist() {
        // pointycastle `ECFieldElement`: `operator -(b)` and `operator -()`.
        assert!(run("abstract class E { E operator -(E b); E operator -(); }\n").is_empty());
        assert_eq!(run("abstract class E { E operator -(); E operator -(); }\n").len(), 1);
    }

    /// Two `class A` in the same unit are the same element in 6.11: their
    /// members are checked against each other (checked against `dart analyze`
    /// 3.6.2: `augment class A` without the experiment leaves two `class A`).
    #[test]
    fn same_named_declarations_merge_members() {
        let f = "class A {
  static void foo() {}
}
class A {
  void foo() {}
  int bar = 0;
}
class A {
  int bar = 1;
  A();
  A();
}
";
        let r = run(f);
        // static foo (offset 24) against the second one's instance foo.
        assert!(r.contains(&("conflicting_static_and_instance".to_string(), 24, 27)), "{r:?}");
        let bar2 = f.rfind("bar").unwrap();
        assert!(r.contains(&("duplicate_definition".to_string(), bar2, bar2 + 3)), "{r:?}");
        assert_eq!(r.iter().filter(|x| x.0 == "duplicate_constructor").count(), 1, "{r:?}");
        // A mixin with the same name is another kind: it does not merge.
        assert!(run("class B { void m() {} }
mixin B { static void m() {} }
").iter().all(|x| x.0 != "conflicting_static_and_instance"));
    }
}
