//! `textDocument/prepareRename` and `textDocument/rename` like the Dart
//! 3.6.2 server (docs/LSP-ESPECIFICACAO.md §12.1 to §12.10): the element
//! under the cursor through `getElementToRename`, the refactoring class of
//! `RenameRefactoring.create`, the `RefactoringStatus` with the exact
//! messages (`checkInitialConditions`, `checkNewName`,
//! `checkFinalConditions`) and the edits of each class (`fillChange`). The
//! whole project is loaded and the references come from the common model of
//! [`crate::project`] (the same as `references`).
//! Written without compiling or running (2026-10-05).

use crate::project::{Target, Concrete, Owner, Project, base_name, word};
use crate::refactorings::Context;
use crate::refactorings_exec::{State, Severity};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{ClassId, ClassKind, Element, FunctionElementId, FunctionKind, LibraryId, UnitId, VariableId};
use dartforge_frontend::ast::{self, DeclKind, ExprKind};
use dartforge_intern::SymbolId;
use std::collections::BTreeSet;
use url::Url;

/// A text edit: replace `span` (bytes) of `uri` with `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub uri: String,
    pub span: Span,
    pub text: String,
}

/// The file that goes with the renamed class (the `RenameFile` at the end of
/// `documentChanges`, §12.1 step 17).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameFile {
    /// Current URI of the file.
    pub de: String,
    /// New URI (same directory, `toFileName` of the new name).
    pub para: String,
    /// Dart does not edit the directives here (they come from
    /// `willRenameFiles`): empty.
    pub directives: Vec<Edit>,
}

/// The result of `rename`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rename {
    pub edits: Vec<Edit>,
    pub file: Option<RenameFile>,
    /// `checkFinalConditions` with an error or a warning (not fatal): the message
    /// of the `Rename Anyway`/`Cancel` prompt, or of the `-32010` without a
    /// prompt.
    pub warning: Option<String>,
    /// Nothing to rename: the response is `null`.
    pub null: bool,
}

/// The refactoring class (`RenameRefactoring.create`, §12.2).
#[derive(Debug, Clone)]
enum Class {
    /// `RenameUnitMemberRefactoringImpl`: the top-level element (the top-level
    /// accessor already replaced by the variable) or the prefix of a type.
    UnitMember(Target),
    Constructor(FunctionElementId),
    /// `RenameImportRefactoringImpl`: the import (by position in `imports`).
    Import { library: LibraryId, index: usize },
    Label { unit: UnitId, declaration: usize },
    Library(LibraryId),
    /// `RenameParameterRefactoringImpl`; `field`: the `this.x` with a field.
    Parameter { unit: UnitId, declaration: usize, field: Option<VariableId> },
    Local { unit: UnitId, declaration: usize },
    TypeParameter { unit: UnitId, declaration: usize },
    ClassMember { target: Target, class: ClassId, concrete: Option<Concrete> },
    ExtensionMember { target: Target, concrete: Option<Concrete> },
}

/// What the cursor renames: the class, the `prepareRename` range, the old
/// name, the kind and the qualified name (messages), and where the element
/// is (SDK, workspace).
#[derive(Debug, Clone)]
struct Request {
    class: Class,
    range: Span,
    old: String,
    kind: String,
    qualified: String,
    library: Option<LibraryId>,
    unit: Option<UnitId>,
}

/// The reserved words (`KeywordStyle.reserved`).
const RESERVED: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally", "for", "if",
    "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while", "with",
];

/// The built-in and the pseudo ones (`isBuiltInOrPseudo`).
const INLINE: &[&str] = &[
    "Function", "abstract", "as", "augment", "covariant", "deferred", "dynamic", "export", "extension", "external", "factory", "get", "implements",
    "import", "interface", "late", "library", "mixin", "operator", "part", "required", "set", "static", "typedef", "async", "await", "base", "hide",
    "inout", "native", "of", "on", "out", "patch", "sealed", "show", "source", "sync", "when", "yield",
];

/// `_validateIdentifier`.
fn validate_identifier(id: &str, desc: &str, start: &str, inline_allowed: bool) -> State {
    if id != id.trim() {
        return State::fatal(format!("{desc} must not start or end with a blank."));
    }
    if id.is_empty() {
        return State::fatal(format!("{desc} must not be empty."));
    }
    let inlined = INLINE.contains(&id);
    if inlined || RESERVED.contains(&id) {
        if inlined && inline_allowed {
            return State::warning("Avoid using built-in identifiers as names.");
        }
        return State::fatal(format!("{desc} must not be a keyword."));
    }
    for c in id.chars() {
        if !(c.is_ascii_alphanumeric() || c == '_' || c == '$') {
            return State::fatal(format!("{desc} must not contain '{c}'."));
        }
    }
    let b = id.as_bytes()[0];
    if !(b.is_ascii_alphabetic() || b == b'_' || b == b'$') {
        return State::fatal(format!("{desc} must begin with {start}."));
    }
    State::default()
}

/// `_validateLowerCamelCase`.
fn validate_lowercase(id: &str, desc: &str, inline_allowed: bool) -> State {
    let desc = format!("{desc} name");
    let e = validate_identifier(id, &desc, "a lowercase letter or underscore", inline_allowed);
    if !e.ok() {
        return e;
    }
    let b = id.as_bytes()[0];
    if b == b'_' || b == b'$' {
        return State::default();
    }
    if !b.is_ascii_lowercase() {
        return State::warning(format!("{desc} should start with a lowercase letter."));
    }
    State::default()
}

/// `_validateUpperCamelCase`.
fn validate_uppercase(id: &str, desc: &str) -> State {
    let desc = format!("{desc} name");
    let e = validate_identifier(id, &desc, "an uppercase letter or underscore", false);
    if !e.ok() {
        return e;
    }
    let b = id.as_bytes()[0];
    if b == b'_' || b == b'$' {
        return State::default();
    }
    if !b.is_ascii_uppercase() {
        return State::warning(format!("{desc} should start with an uppercase letter."));
    }
    State::default()
}

/// `validateLibraryName`.
fn validate_library(name: &str) -> State {
    if name.trim_matches([' ', '\t']).is_empty() {
        return State::fatal("Library name must not be blank.");
    }
    for c in name.split('.') {
        let e = validate_identifier(c, "Library name identifier", "a lowercase letter or underscore", false);
        if !e.ok() {
            return e;
        }
    }
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        return State::warning("Library name should consist of lowercase identifier separated by dots.");
    }
    State::default()
}

/// `String.toFileName`.
pub(crate) fn file_name(name: &str) -> String {
    crate::refactorings::file_name(name)
}

/// The parameter (of a function or constructor) whose range is in `[a, b]`.
fn parameter_in_range(ast: &ast::Ast, a: usize, b: usize) -> Option<&ast::Parameter> {
    let inside = |q: &&ast::Parameter| q.span.start >= a && q.span.end <= b;
    for f in &ast.functions {
        if let Some(q) = f.parameters.iter().flatten().find(inside) {
            return Some(q);
        }
    }
    for m in &ast.members {
        if let ast::MemberKind::Constructor(k) = &m.kind
            && let Some(q) = k.parameters.iter().find(inside)
        {
            return Some(q);
        }
    }
    None
}

impl Project {
    // -- The element under the cursor --------------------------------------------

    /// `getElementOfNode` + `getElementToRename` + `RenameRefactoring.create`.
    fn rename_request(&self, unit: UnitId, offset: usize) -> Option<Request> {
        let p = self.program();
        let u = p.unit(unit);
        let source = u.source.as_str();
        let cx = Context::new(self, unit);
        let n = cx.tree.localizar(offset, offset)?;
        let node_kind = cx.kind(n);
        let no = &cx.tree.nos[n];
        let node_range = Span { start: no.inicio, end: no.fim };
        match node_kind {
            // Outside the `getElementToRename` table.
            "SuperFormalParameter" | "FunctionTypedFormalParameter" => return None,
            // A directive's URI cannot be renamed.
            "SimpleStringLiteral" | "StringInterpolation" | "AdjacentStrings"
                if cx.parent(n).is_some_and(|x| matches!(cx.kind(x), "ImportDirective" | "ExportDirective" | "PartDirective" | "PartOfDirective")) =>
            {
                return None;
            }
            "ImportDirective" => {
                // The directive's import: with a prefix, its identifier; without one, the
                // empty range at the cursor.
                let lib = u.library;
                let index = p.library(lib).imports.iter().position(|i| {
                    i.unit == unit
                        && p.unit(i.unit).unit.directives.get(i.directive).is_some_and(|d| no.inicio <= d.span.start && d.span.end <= no.fim)
                })?;
                let imp = &p.library(lib).imports[index];
                let (range, old) = match &p.unit(imp.unit).unit.directives[imp.directive].kind {
                    ast::DirectiveKind::Import { prefix: Some(pr), .. } => (pr.span, source[pr.span.start..pr.span.end].to_string()),
                    _ => (Span { start: offset, end: offset }, String::new()),
                };
                return Some(Request {
                    class: Class::Import { library: lib, index },
                    range,
                    old,
                    kind: "import directive".to_string(),
                    qualified: String::new(),
                    library: Some(lib),
                    unit: Some(unit),
                });
            }
            "ImportPrefixReference" => {
                let text = cx.node_text(n).trim_end_matches('.').trim().to_string();
                let symbol = self.query.names.lookup(&text)?;
                return Some(Request {
                    class: Class::UnitMember(Target::Prefix { library: u.library, name: symbol }),
                    range: node_range,
                    old: text.clone(),
                    kind: "import prefix".to_string(),
                    qualified: text,
                    library: Some(u.library),
                    unit: Some(unit),
                });
            }
            "LibraryDirective" => {
                let name = source[no.inicio..no.fim].trim_start_matches("library").trim().trim_end_matches(';').trim().to_string();
                return Some(Request {
                    class: Class::Library(u.library),
                    range: node_range,
                    old: name.clone(),
                    kind: "library".to_string(),
                    qualified: name,
                    library: Some(u.library),
                    unit: Some(unit),
                });
            }
            "InstanceCreationExpression" => {
                // `new`/`const`: the class, on the type name.
                let cn = cx.children(n).iter().copied().find(|&k| cx.kind(k) == "ConstructorName")?;
                let ty = *cx.children(cn).first()?;
                let start = cx.children(ty).iter().copied().find(|&k| cx.kind(k) == "ImportPrefixReference").map_or(cx.tree.nos[ty].inicio, |k| cx.tree.nos[k].fim);
                let d = self.identify(unit, start).ok()??;
                let Target::Top(Element::Class(c)) = d.target else { return None };
                return self.target_request(Target::Top(Element::Class(c)), None, d.name, unit);
            }
            "ConstructorDeclaration" => {
                let f = (0..p.functions.len()).map(|i| FunctionElementId(i as u32)).find(|&f| {
                    matches!(p.function(f).kind, FunctionKind::Constructor)
                        && self.function_name(f).is_some_and(|(uu, s)| uu == unit && no.inicio <= s.start && s.end <= no.fim)
                })?;
                let mut request = self.target_request(Target::Constructor(f), Some(Concrete::Function(f)), node_range, unit)?;
                request.range = node_range;
                return Some(request);
            }
            "FieldFormalParameter" => {
                let q = parameter_in_range(&u.ast, no.inicio, no.fim)?;
                let name = q.name?;
                let field = (0..p.classes.len()).map(|i| ClassId(i as u32)).find(|&c| {
                    p.class(c).decl.is_some_and(|d| d.unit == unit && {
                        let s = u.ast.decl(d.decl).span;
                        s.start <= no.inicio && no.fim <= s.end
                    })
                });
                let field = field.and_then(|c| p.class(c).fields.iter().copied().find(|v| p.variable(*v).name == name.sym));
                let old = source[name.span.start..name.span.end].to_string();
                return Some(Request {
                    class: Class::Parameter { unit, declaration: name.span.start, field },
                    range: name.span,
                    old: old.clone(),
                    kind: "parameter".to_string(),
                    qualified: old,
                    library: Some(u.library),
                    unit: Some(unit),
                });
            }
            _ => {}
        }
        // Statement label, and that of a `break`/`continue`.
        if node_kind == "SimpleIdentifier"
            && let Some(parent) = cx.parent(n)
        {
            let command_label = cx.kind(parent) == "Label"
                && cx.parent(parent).is_some_and(|g| matches!(cx.kind(g), "LabeledStatement" | "SwitchCase" | "SwitchDefault" | "SwitchPatternCase"));
            let declaration = if command_label {
                Some(no.inicio)
            } else if matches!(cx.kind(parent), "BreakStatement" | "ContinueStatement") {
                let name = cx.node_text(n);
                let mut found = None;
                'outside: for k in cx.with_parents(parent) {
                    for &l in cx.children(k) {
                        if cx.kind(l) == "Label"
                            && let Some(&i) = cx.children(l).first()
                            && cx.node_text(i) == name
                        {
                            found = Some(cx.tree.nos[i].inicio);
                            break 'outside;
                        }
                    }
                }
                Some(found?)
            } else {
                None
            };
            if let Some(declaration) = declaration {
                let name = cx.node_text(n).to_string();
                return Some(Request {
                    class: Class::Label { unit, declaration },
                    range: node_range,
                    old: name.clone(),
                    kind: "label".to_string(),
                    qualified: name,
                    library: Some(u.library),
                    unit: Some(unit),
                });
            }
        }
        let d = self.identify(unit, offset).ok()??;
        // The prefix used in an expression: the `PrefixElement`, a member of the
        // unit (`RenameUnitMemberRefactoringImpl`); the directive's `as p` keeps
        // the `LibraryImportElement` (the `ImportDirective` branch).
        if let Target::Prefix { library, name } = d.target {
            let old = self.name(name).to_string();
            // On the directive's own `as p`: the `LibraryImportElement`.
            let in_directive = p.library(library).imports.iter().position(|i| {
                i.unit == unit
                    && matches!(
                        p.unit(i.unit).unit.directives.get(i.directive).map(|x| &x.kind),
                        Some(ast::DirectiveKind::Import { prefix: Some(pr), .. }) if pr.span.start <= offset && offset <= pr.span.end
                    )
            });
            // Used in an expression (a `SimpleIdentifier` with the `PrefixElement`):
            // `getElementOfNode` swaps it for the import (`getImportElement`), and the
            // rename is the import's.
            let in_directive = in_directive.or_else(|| self.prefix_import(unit, offset, library, name));
            if let Some(index) = in_directive {
                return Some(Request {
                    class: Class::Import { library, index },
                    range: d.name,
                    old,
                    kind: "import directive".to_string(),
                    qualified: String::new(),
                    library: Some(library),
                    unit: Some(unit),
                });
            }
            return Some(Request {
                class: Class::UnitMember(Target::Prefix { library, name }),
                range: d.name,
                old: old.clone(),
                kind: "import prefix".to_string(),
                qualified: old,
                library: Some(library),
                unit: Some(unit),
            });
        }
        self.target_request(d.target, d.concrete, d.name, unit)
    }

    /// The request for an `identify` target (rule 1 of `create` already
    /// applied: accessor → variable).
    fn target_request(&self, target: Target, concrete: Option<Concrete>, range: Span, unit: UnitId) -> Option<Request> {
        let p = self.program();
        let request = match &target {
            Target::Top(el) => {
                let (kind, name) = self.top_kind(*el)?;
                Request {
                    class: Class::UnitMember(target.clone()),
                    range,
                    old: name.clone(),
                    kind,
                    qualified: name,
                    library: Some(self.element_library(*el)),
                    unit: self.top_element_name(*el).map(|(u, _)| u),
                }
            }
            Target::Constructor(f) => {
                let fe = p.function(*f);
                let class = fe.class.map(|c| self.name(p.class(c).name).to_string()).unwrap_or_default();
                let name = self.name(fe.name).to_string();
                let displayed = if name.is_empty() { class } else { format!("{class}.{name}") };
                Request {
                    class: Class::Constructor(*f),
                    range,
                    old: name,
                    kind: "constructor".to_string(),
                    qualified: displayed,
                    library: Some(fe.library),
                    unit: self.function_name(*f).map(|(u, _)| u),
                }
            }
            Target::Local { unit: lu, declaration } => {
                let un = p.unit(*lu);
                let name = word(&un.source, *declaration).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
                let (class, kind) = if crate::project::parameter_in(&un.ast, *declaration).is_some() {
                    (Class::Parameter { unit: *lu, declaration: *declaration, field: None }, "parameter")
                } else if un.ast.functions.iter().any(|f| f.name.is_some_and(|x| x.span.start == *declaration)) {
                    (Class::Local { unit: *lu, declaration: *declaration }, "function")
                } else {
                    (Class::Local { unit: *lu, declaration: *declaration }, "local variable")
                };
                Request {
                    class,
                    range,
                    old: name.clone(),
                    kind: kind.to_string(),
                    qualified: name,
                    library: Some(un.library),
                    unit: Some(*lu),
                }
            }
            Target::TypeParameter { unit: lu, declaration } => {
                let un = p.unit(*lu);
                let name = word(&un.source, *declaration).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
                Request {
                    class: Class::TypeParameter { unit: *lu, declaration: *declaration },
                    range,
                    old: name.clone(),
                    kind: "type parameter".to_string(),
                    qualified: name,
                    library: Some(un.library),
                    unit: Some(*lu),
                }
            }
            Target::Prefix { library, name } => Request {
                class: Class::UnitMember(target.clone()),
                range,
                old: self.name(*name).to_string(),
                kind: "import prefix".to_string(),
                qualified: self.name(*name).to_string(),
                library: Some(*library),
                unit: Some(unit),
            },
            Target::Member { owner, name, .. } => {
                let (kind, lib, uni) = match concrete {
                    Some(Concrete::Function(f)) => {
                        let fe = p.function(f);
                        let kind = match fe.kind {
                            FunctionKind::Function | FunctionKind::Operator => "method",
                            _ => "field",
                        };
                        (kind, fe.library, self.function_name(f).map(|(u, _)| u))
                    }
                    Some(Concrete::Variable(v)) => ("field", p.variable(v).library, self.variable_name(v).map(|(u, _)| u)),
                    None => ("field", p.unit(unit).library, None),
                };
                let (class, qualified) = match owner {
                    Owner::Class(c) => (
                        Class::ClassMember { target: target.clone(), class: *c, concrete },
                        format!("{}.{name}", self.name(p.class(*c).name)),
                    ),
                    Owner::Extension(x) => (
                        Class::ExtensionMember { target: target.clone(), concrete },
                        format!("{}.{name}", p.extension(*x).name.map(|s| self.name(s)).unwrap_or("")),
                    ),
                };
                Request { class, range, old: name.clone(), kind: kind.to_string(), qualified, library: Some(lib), unit: uni }
            }
        };
        Some(request)
    }

    /// The kind (`kind.displayName`) and the name of a top-level element (a
    /// top-level accessor becomes the variable).
    fn top_kind(&self, el: Element) -> Option<(String, String)> {
        let p = self.program();
        Some(match el {
            Element::Class(c) => (self.class_kind(c), self.name(p.class(c).name).to_string()),
            Element::Extension(x) => ("extension".to_string(), p.extension(x).name.map(|s| self.name(s).to_string()).unwrap_or_default()),
            Element::Typedef(t) => {
                let d = p.typedef(t);
                let legacy = matches!(&p.unit(d.decl.unit).ast.decl(d.decl.decl).kind, DeclKind::Typedef(x) if matches!(x.kind, ast::TypedefKind::Legacy { .. }));
                (if legacy { "function type alias" } else { "type alias" }.to_string(), self.name(d.name).to_string())
            }
            Element::Function(f) => {
                let fe = p.function(f);
                match fe.kind {
                    FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor => {
                        ("top level variable".to_string(), base_name(self.name(fe.name)).to_string())
                    }
                    _ => ("function".to_string(), self.name(fe.name).to_string()),
                }
            }
            Element::Variable(v) => ("top level variable".to_string(), self.name(p.variable(v).name).to_string()),
            Element::Prefix(_, s) => ("import prefix".to_string(), self.name(s).to_string()),
        })
    }

    /// `workspace.containsElement`: the element's file is under the root.
    fn in_workspace(&self, unit: Option<UnitId>) -> bool {
        let Some(u) = unit else { return false };
        self.program().unit(u).path.as_deref().is_some_and(|c| crate::project::inside(c, &self.root))
    }

    // -- The conditions -----------------------------------------------------------

    /// `checkInitialConditions`.
    fn initial_conditions(&self, request: &Request) -> State {
        let mut e = State::default();
        let p = self.program();
        if request.library.is_some_and(|l| p.library(l).is_sdk) {
            e.add(Severity::Fatal, format!("The {} '{}' is defined in the SDK, so cannot be renamed.", request.kind, request.qualified));
        }
        if !matches!(request.class, Class::Import { .. }) && !self.in_workspace(request.unit) {
            e.add(Severity::Fatal, format!("The {} '{}' is defined outside of the project, so cannot be renamed.", request.kind, request.qualified));
        }
        let operator = match &request.class {
            Class::ClassMember { concrete: Some(Concrete::Function(f)), .. } | Class::ExtensionMember { concrete: Some(Concrete::Function(f)), .. } => {
                p.function(*f).kind == FunctionKind::Operator
            }
            _ => false,
        };
        if operator {
            e.add(Severity::Fatal, "Cannot rename operator.");
        }
        e
    }

    /// `checkNewName`.
    fn verify_new_name(&self, request: &Request, new: &str) -> State {
        let p = self.program();
        let mut e = State::default();
        if new == request.old {
            e.add(Severity::Fatal, "The new name must be different than the current name.");
        }
        match &request.class {
            Class::UnitMember(Target::Top(el)) => match el {
                Element::Class(c) => {
                    if p.class(*c).kind == ClassKind::ExtensionType {
                        e.absorb(validate_uppercase(new, "Extension type"));
                    }
                    e.absorb(validate_uppercase(new, "Class"));
                }
                Element::Function(f) if matches!(p.function(*f).kind, FunctionKind::Function) => e.absorb(validate_lowercase(new, "Function", true)),
                Element::Function(_) | Element::Variable(_) => e.absorb(validate_lowercase(new, "Variable", true)),
                Element::Typedef(_) => e.absorb(validate_uppercase(new, "Type alias")),
                Element::Extension(_) | Element::Prefix(..) => {}
            },
            Class::UnitMember(_) => {}
            Class::Constructor(f) => {
                if !new.is_empty() {
                    e.absorb(validate_lowercase(new, "Constructor", true));
                }
                // `_analyzePossibleConflicts`.
                if let Some(c) = p.function(*f).class {
                    let cl = p.class(c);
                    if self.name(cl.name) == new {
                        e.add(Severity::Error, "The constructor should not have the same name as the name of the enclosing class.");
                    }
                    for (kind, _) in self.children_with_name(c, new) {
                        e.add(
                            Severity::Error,
                            format!("{} '{}' already declares {kind} with name '{new}'.", capitalize(&self.class_kind(c)), self.name(cl.name)),
                        );
                    }
                }
            }
            Class::Import { .. } => {
                if !new.is_empty() {
                    e.absorb(validate_identifier(new, "Import prefix name", "a lowercase letter or underscore", false));
                }
            }
            Class::Label { .. } => e.absorb(validate_lowercase(new, "Label", true)),
            Class::Library(_) => e.absorb(validate_library(new)),
            Class::Parameter { .. } => e.absorb(validate_lowercase(new, "Parameter", true)),
            Class::Local { unit, declaration } => {
                let function = p.unit(*unit).ast.functions.iter().any(|f| f.name.is_some_and(|x| x.span.start == *declaration));
                e.absorb(validate_lowercase(new, if function { "Function" } else { "Variable" }, true));
            }
            Class::TypeParameter { .. } => e.absorb(validate_uppercase(new, "Type parameter")),
            Class::ClassMember { concrete, .. } | Class::ExtensionMember { concrete, .. } => match concrete {
                Some(Concrete::Function(f)) if matches!(p.function(*f).kind, FunctionKind::Function | FunctionKind::Operator) => {
                    e.absorb(validate_lowercase(new, "Method", true))
                }
                _ => e.absorb(validate_lowercase(new, "Field", true)),
            },
        }
        e
    }

    /// The kind (`kind.displayName`) of a class.
    fn class_kind(&self, c: ClassId) -> String {
        match self.program().class(c).kind {
            ClassKind::Mixin => "mixin",
            ClassKind::Enum => "enum",
            ClassKind::ExtensionType => "extension type",
            _ => "class",
        }
        .to_string()
    }

    /// The direct children of `c` with base name `name` (`getChildren`):
    /// accessors, fields, constructors, methods, type parameters; the kind and
    /// the qualified name.
    fn children_with_name(&self, c: ClassId, name: &str) -> Vec<(String, String)> {
        let p = self.program();
        let cl = p.class(c);
        let class = self.name(cl.name).to_string();
        let mut v = Vec::new();
        let mut members: Vec<(SymbolId, FunctionElementId)> = cl.instance_members.iter().chain(cl.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
        members.sort_by_key(|(_, f)| f.0);
        for (s, f) in &members {
            let fe = p.function(*f);
            if base_name(self.name(*s)) == name && matches!(fe.kind, FunctionKind::Getter | FunctionKind::Setter | FunctionKind::ImplicitAccessor) {
                let setter = fe.kind == FunctionKind::Setter || self.name(*s).ends_with('=');
                v.push((if setter { "setter" } else { "getter" }.to_string(), name.to_string()));
            }
        }
        for f in cl.fields.iter() {
            if self.name(p.variable(*f).name) == name {
                v.push(("field".to_string(), format!("{class}.{name}")));
            }
        }
        for (s, _) in cl.constructors.iter() {
            if self.name(*s) == name {
                v.push(("constructor".to_string(), format!("{class}.{name}")));
            }
        }
        for (s, f) in &members {
            if self.name(*s) == name && matches!(p.function(*f).kind, FunctionKind::Function | FunctionKind::Operator) {
                v.push(("method".to_string(), format!("{class}.{name}")));
            }
        }
        for tp in cl.type_params.iter() {
            if self.name(tp.name) == name {
                v.push(("type parameter".to_string(), name.to_string()));
            }
        }
        v
    }

    /// The references (without the declarations) of a target, as (unit, range).
    fn target_references(&self, target: &Target) -> Vec<(UnitId, Span)> {
        let declarations = self.declarations(target);
        let Ok(oc) = self.occurrences(target, false) else { return Vec::new() };
        // `searchReferences` goes through `Search._addResults`: only the candidate
        // files (those that cite the name and those of the library), with Dart's
        // gaps (a factory redirection does not cite the name).
        let candidates = self.search_name(target).map(|name| {
            let mut units: Vec<UnitId> = declarations.iter().map(|(u, _, _)| *u).collect();
            units.dedup();
            self.candidate_files(&name, &units)
        });
        oc.into_iter()
            .filter(|o| !declarations.contains(o))
            .filter(|(u, _, _)| candidates.as_ref().is_none_or(|c| c.contains(u)))
            .map(|(u, a, b)| (u, Span { start: a, end: b }))
            .collect()
    }

    /// The library path relative to the root (`getElementQualifiedName` of a
    /// library).
    fn library_path(&self, l: LibraryId) -> String {
        let p = self.program();
        let Some(&u) = p.library(l).units.first() else { return String::new() };
        match p.unit(u).path.as_deref() {
            // `pathContext.relative`: with the platform separator.
            Some(c) => c.strip_prefix(&self.root).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| c.to_string_lossy().into_owned()),
            None => p.library(l).uri.clone(),
        }
    }

    /// `checkFinalConditions`.
    fn final_conditions(&self, request: &Request, new: &str) -> State {
        let p = self.program();
        let mut e = State::default();
        match &request.class {
            Class::UnitMember(target) => {
                // The prefix (`PrefixElement`) is also a member of the unit.
                if !matches!(target, Target::Top(_) | Target::Prefix { .. }) {
                    return e;
                }
                let lib = request.library.unwrap_or(LibraryId(0));
                // `_validateWillConflict`.
                for &u in p.library(lib).units.iter() {
                    let cx = Context::new(self, u);
                    for (kind, name) in crate::refactorings_method::top_elements(&cx, u) {
                        if name == new {
                            e.add(Severity::Error, format!("Library already declares {kind} with name '{new}'."));
                        }
                    }
                }
                let references = self.target_references(target);
                // `_validateWillBeInvisible`.
                if new.starts_with('_') {
                    for (u, _) in &references {
                        let l = p.unit(*u).library;
                        if l != lib {
                            e.add(Severity::Error, format!("Renamed {} will be invisible in '{}'.", request.kind, self.library_path(l)));
                        }
                    }
                }
                // `_validateWillBeShadowed`.
                for (u, s) in &references {
                    if let Some(c) = self.containing_class(*u, s.start) {
                        for (kind, qualified) in self.children_with_name(c, new) {
                            e.add(Severity::Error, format!("Reference to renamed {} will be shadowed by {kind} '{qualified}'.", request.kind));
                        }
                    }
                }
                // `_validateWillShadow`.
                for (kind, qualified, owning, functions) in self.member_declarations(new) {
                    for (u, s) in self.uses_not_qualified(&functions) {
                        if self.containing_class(u, s.start) == Some(owning) {
                            continue;
                        }
                        let visible = p.unit(u).library == lib || self.visible_by_import_without_prefix(u, target);
                        if !visible {
                            continue;
                        }
                        e.add(Severity::Error, format!("Renamed {} will shadow {kind} '{qualified}'.", request.kind));
                    }
                }
            }
            Class::ClassMember { target, class, .. } => {
                let class = *class;
                // `_checkClassAlreadyDeclares`.
                for (kind, _) in self.children_with_name(class, new) {
                    e.add(
                        Severity::Error,
                        format!(
                            "{} '{}' already declares {kind} with name '{new}'.",
                            capitalize(&self.class_kind(class)),
                            self.name(p.class(class).name)
                        ),
                    );
                }
                let Target::Member { owner, name, is_static } = target else { return e };
                let family = self.family(*owner, name, *is_static, false).ok();
                let references = self.target_references(target);
                let subclasses: Vec<ClassId> = self.subtypes(class, false);
                // The owning class of an element is named like the new name.
                if let Some(fam) = &family {
                    let mut owning_: BTreeSet<u32> = BTreeSet::new();
                    for f in &fam.functions {
                        if let Some(c) = p.function(*f).class {
                            owning_.insert(c.0);
                        }
                    }
                    for c in owning_ {
                        let c = ClassId(c);
                        if self.name(p.class(c).name) == new {
                            e.add(
                                Severity::Error,
                                format!("Renamed {} has the same name as the declaring {} '{new}'.", request.kind, self.class_kind(c)),
                            );
                        }
                    }
                }
                // `_getShadowingLocalElement`: the first shadowing by a local.
                if let Some((local_kind, local_name)) = self.shadow_by_local(&references, new) {
                    e.add(Severity::Error, format!("Usage of renamed {} will be shadowed by {local_kind} '{local_name}'.", request.kind));
                }
                // `_checkHierarchy(isRename: true)`.
                let supers = self.supertypes(class);
                for (kind, qualified, owning, _) in self.member_declarations(new) {
                    if supers.contains(&owning) {
                        e.add(Severity::Error, format!("Renamed {} will shadow {kind} '{qualified}'.", request.kind));
                    }
                    if subclasses.contains(&owning) {
                        e.add(Severity::Error, format!("Renamed {} will be shadowed by {kind} '{qualified}'.", request.kind));
                    }
                }
                // `_validateWillBeInvisible`.
                if new.starts_with('_') {
                    let lib = p.class(class).library;
                    for (u, _) in &references {
                        let l = p.unit(*u).library;
                        if l != lib {
                            e.add(Severity::Error, format!("Renamed {} will be invisible in '{}'.", request.kind, self.library_path(l)));
                        }
                    }
                }
            }
            Class::ExtensionMember { target, .. } => {
                if let Target::Member { owner: Owner::Extension(x), .. } = target {
                    let ext = p.extension(*x);
                    let mut members: Vec<(SymbolId, FunctionElementId)> = ext.instance_members.iter().chain(ext.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
                    members.sort_by_key(|(_, f)| f.0);
                    let name_ext = ext.name.map(|s| self.name(s).to_string()).unwrap_or_default();
                    for (s, f) in members {
                        if base_name(self.name(s)) != new {
                            continue;
                        }
                        let kind = match p.function(f).kind {
                            FunctionKind::Function | FunctionKind::Operator => "method",
                            FunctionKind::Getter => "getter",
                            FunctionKind::Setter => "setter",
                            _ => "field",
                        };
                        e.add(Severity::Error, format!("Extension '{name_ext}' already declares {kind} with name '{new}'."));
                    }
                    let references = self.target_references(target);
                    if let Some((local_kind, local_name)) = self.shadow_by_local(&references, new) {
                        e.add(Severity::Error, format!("Usage of renamed {} will be shadowed by {local_kind} '{local_name}'.", request.kind));
                    }
                }
            }
            Class::Parameter { unit, declaration, .. } => {
                let un = p.unit(*unit);
                let named = crate::project::parameter_in(&un.ast, *declaration).is_some_and(|(q, _)| q.kind == ast::ParameterKind::Named);
                if new.starts_with('_') && named {
                    e.add(Severity::Error, format!("The parameter '{}' is named and can not be private.", request.old));
                    return e;
                }
                e.absorb(self.local_conflicts(*unit, *declaration, new, &request.kind));
            }
            Class::Local { unit, declaration } => {
                e.absorb(self.local_conflicts(*unit, *declaration, new, &request.kind));
            }
            Class::TypeParameter { unit, declaration } => {
                let ast = &p.unit(*unit).ast;
                let siblings = type_parameter_lists(ast).into_iter().find(|l| l.iter().any(|t| t.name.span.start == *declaration)).unwrap_or_default();
                if siblings.iter().any(|t| self.name(t.name.sym) == new) {
                    e.add(Severity::Error, format!("Duplicate type parameter '{new}'."));
                }
            }
            Class::Constructor(_) | Class::Import { .. } | Class::Label { .. } | Class::Library(_) => {}
        }
        e
    }

    /// The class, mixin, enum or extension type that contains the offset.
    fn containing_class(&self, u: UnitId, offset: usize) -> Option<ClassId> {
        let p = self.program();
        p.classes
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let d = c.decl?;
                let s = p.unit(d.unit).ast.decl(d.decl).span;
                (d.unit == u && s.start <= offset && offset < s.end).then_some((s.end - s.start, ClassId(i as u32)))
            })
            .min_by_key(|(t, _)| *t)
            .map(|(_, c)| c)
    }

    /// `searchMemberDeclarations(name)`: fields (non-synthetic), declared
    /// accessors and methods with the name, of every class of the program: the
    /// kind, the qualified name, the owner and the functions whose references
    /// count.
    fn member_declarations(&self, name: &str) -> Vec<(String, String, ClassId, Vec<FunctionElementId>)> {
        let p = self.program();
        let mut v = Vec::new();
        for (i, c) in p.classes.iter().enumerate() {
            let owning = ClassId(i as u32);
            let mut members: Vec<(SymbolId, FunctionElementId)> = c.instance_members.iter().chain(c.static_members.iter()).map(|(s, f)| (*s, *f)).collect();
            members.sort_by_key(|(_, f)| f.0);
            for (s, f) in &members {
                let key = self.name(*s);
                if base_name(key) != name {
                    continue;
                }
                let fe = p.function(*f);
                let qualified = format!("{}.{name}", self.name(c.name));
                match fe.kind {
                    FunctionKind::Function | FunctionKind::Operator => v.push(("method".to_string(), qualified, owning, vec![*f])),
                    FunctionKind::ImplicitAccessor if !key.ends_with('=') => {
                        let mut fs = vec![*f];
                        if let Some((_, g)) = members.iter().find(|(s2, _)| self.name(*s2) == format!("{key}=")) {
                            fs.push(*g);
                        }
                        v.push(("field".to_string(), qualified, owning, fs));
                    }
                    FunctionKind::Getter => v.push(("getter".to_string(), name.to_string(), owning, vec![*f])),
                    FunctionKind::Setter => v.push(("setter".to_string(), name.to_string(), owning, vec![*f])),
                    _ => {}
                }
            }
        }
        v
    }

    /// The unqualified uses (bare identifier) resolved to one of the functions,
    /// in the project's libraries.
    fn uses_not_qualified(&self, functions: &[FunctionElementId]) -> Vec<(UnitId, Span)> {
        let p = self.program();
        let mut v = Vec::new();
        for u in self.units() {
            let ast = &p.unit(u).ast;
            let bodies = &self.query.bodies.units[u.0 as usize];
            for (i, e) in ast.exprs.iter().enumerate() {
                let ExprKind::Identifier(n) = &e.kind else { continue };
                if let Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(g), .. }) = bodies.get_resolved(ast::ExprId(i as u32))
                    && functions.contains(g)
                {
                    v.push((u, n.span));
                }
            }
        }
        v
    }

    /// `_isVisibleAt`: some unprefixed import of the reference's library brings
    /// the element.
    fn visible_by_import_without_prefix(&self, u: UnitId, target: &Target) -> bool {
        let p = self.program();
        let Target::Top(el) = target else { return false };
        let lib = p.unit(u).library;
        p.library(lib)
            .imports
            .iter()
            .filter(|i| i.prefix.is_none())
            .any(|i| p.library(i.library).exported.values().any(|b| b.getter == Some(*el) || b.setter == Some(*el)))
    }

    /// The first shadowing by a local (`_getShadowingLocalElement`): for each
    /// unqualified reference, a local named `new` (top-level or local function,
    /// simple parameter, local variable) visible at it.
    fn shadow_by_local(&self, references: &[(UnitId, Span)], new: &str) -> Option<(String, String)> {
        let p = self.program();
        for (u, s) in references {
            let un = p.unit(*u);
            let ast = &un.ast;
            let loose = ast.exprs.iter().any(|e| matches!(&e.kind, ExprKind::Identifier(n) if n.span == *s));
            if !loose {
                continue;
            }
            let cx = Context::new(self, *u);
            for (name, (a, b)) in crate::refactorings_inline::visible_ranges(&cx, 0) {
                if name == new && !(b <= s.start) && !(a >= s.end) {
                    let function = ast.functions.iter().any(|f| f.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == new && x.span.start >= a));
                    let kind = if function { "function" } else if ast.functions.iter().any(|f| f.parameters.iter().flatten().any(|q| q.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == new))) { "parameter" } else { "local variable" };
                    return Some((kind.to_string(), name));
                }
            }
            // Top-level functions are also `FunctionDeclaration`s with a
            // `FunctionElement`, visible in the whole unit.
            for d in ast.decls.iter() {
                if let DeclKind::Function(f) = &d.kind
                    && let Some(n) = ast.function(*f).name
                    && &un.source[n.span.start..n.span.end] == new
                    && matches!(ast.function(*f).kind, ast::FunctionKind::Function)
                {
                    return Some(("function".to_string(), new.to_string()));
                }
            }
        }
        None
    }

    /// The `ConflictValidatorVisitor` (§12.9): a duplicate local function or
    /// variable visible together with the target, and a use of another element
    /// named `new` inside the target's range.
    fn local_conflicts(&self, u: UnitId, declaration: usize, new: &str, kind_target: &str) -> State {
        let p = self.program();
        let un = p.unit(u);
        let cx = Context::new(self, u);
        let visible = crate::refactorings_inline::visible_ranges(&cx, 0);
        let name_target = word(&un.source, declaration).map(|s| un.source[s.start..s.end].to_string()).unwrap_or_default();
        let target_range = visible.iter().find(|(n, (a, b))| *n == name_target && *a <= declaration && declaration <= *b).map(|(_, f)| *f);
        let mut e = State::default();
        let Some((ai, af)) = target_range else { return e };
        let mut conflicting: Vec<(usize, usize)> = Vec::new();
        // The duplicate declarations (local functions and local variables; the
        // parameters are not visited).
        for (name, (a, b)) in &visible {
            if name != new || *b <= ai || *a >= af {
                continue;
            }
            let function = un.ast.functions.iter().any(|f| f.name.is_some_and(|x| &un.source[x.span.start..x.span.end] == new && x.span.start >= *a && x.span.end <= *b));
            let variable = un.ast.stmts.iter().any(|s| match &s.kind {
                ast::StmtKind::Variables(l) => l.variables.iter().any(|v| &un.source[v.name.span.start..v.name.span.end] == new && v.name.span.start >= *a && v.name.span.end <= *b),
                _ => false,
            });
            if !function && !variable {
                continue;
            }
            e.add(Severity::Error, format!("Duplicate {} '{new}'.", if function { "function" } else { "local variable" }));
            conflicting.push((*a, *b));
        }
        // The uses of another element named `new` in the target's range.
        let bodies = &self.query.bodies.units[u.0 as usize];
        let file_of = |x: Option<UnitId>| {
            x.and_then(|x| p.unit(x).path.as_deref().and_then(|c| c.file_name()).map(|n| n.to_string_lossy().into_owned())).unwrap_or_default()
        };
        for (i, x) in un.ast.exprs.iter().enumerate() {
            let ExprKind::Identifier(n) = &x.kind else { continue };
            if self.name(n.sym) != new || n.span.start < ai || n.span.start > af {
                continue;
            }
            let id = ast::ExprId(i as u32);
            if let Some(d) = bodies.declaracao_local(id)
                && conflicting.iter().any(|(a, b)| *a <= d && d <= *b)
            {
                continue;
            }
            let (kind, qualified, file) = match bodies.get_resolved(id) {
                Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Function(f), .. }) => {
                    let fe = p.function(*f);
                    let c = fe.class.map(|c| self.name(p.class(c).name).to_string()).unwrap_or_default();
                    let kind = match fe.kind {
                        FunctionKind::Function | FunctionKind::Operator => "method",
                        _ => "field",
                    };
                    (kind, format!("{c}.{new}"), file_of(self.function_name(*f).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Member { member: dartforge_types::MemberRef::Variable(v), .. }) => {
                    let c = p.variable(*v).class.map(|c| self.name(p.class(c).name).to_string()).unwrap_or_default();
                    ("field", format!("{c}.{new}"), file_of(self.variable_name(*v).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Element(el)) => {
                    let (kind, _) = self.top_kind(*el).unwrap_or_default();
                    let kind: &'static str = match kind.as_str() {
                        "function" => "function",
                        "top level variable" => "top level variable",
                        "class" => "class",
                        "mixin" => "mixin",
                        "enum" => "enum",
                        _ => "type alias",
                    };
                    (kind, new.to_string(), file_of(self.top_element_name(*el).map(|(u, _)| u)))
                }
                Some(dartforge_types::Resolved::Parameter { .. }) => ("parameter", new.to_string(), file_of(Some(u))),
                Some(dartforge_types::Resolved::Local(_)) => {
                    // `ElementKind.displayName`: a local declared in a parameter list is
                    // `parameter`.
                    let parameter = bodies.declaracao_local(id).is_some_and(|d| {
                        un.ast.functions.iter().any(|f| f.parameters.iter().flatten().any(|q| q.name.is_some_and(|x| x.span.start == d)))
                    });
                    (if parameter { "parameter" } else { "local variable" }, new.to_string(), file_of(Some(u)))
                }
                _ => continue,
            };
            e.add(Severity::Error, format!("Usage of {kind} \"{qualified}\" declared in \"{file}\" will be shadowed by renamed {kind_target}."));
        }
        e
    }

    // -- The edits -------------------------------------------------------------

    /// `fillChange` of each class.
    fn rename_edits(&self, request: &Request, new: &str) -> Result<Vec<Edit>, String> {
        let p = self.program();
        let mut edits: Vec<Edit> = Vec::new();
        let por = |u: UnitId, s: Span, text: &str, edits: &mut Vec<Edit>| {
            if !self.in_workspace(Some(u)) {
                return;
            }
            if let Some(uri) = self.unit_uri(u)
                && !edits.iter().any(|e| e.uri == uri && e.span == s)
            {
                edits.push(Edit { uri, span: s, text: text.to_string() });
            }
        };
        match &request.class {
            Class::Constructor(f) => return self.constructor_edits(*f, new),
            Class::Import { library, index } => return Ok(self.import_edits(*library, *index, new)),
            Class::UnitMember(target) | Class::ExtensionMember { target, .. } => {
                for (u, a, b) in self.declarations(target) {
                    por(u, Span { start: a, end: b }, new, &mut edits);
                }
                for (u, s) in self.target_references(target) {
                    por(u, s, new, &mut edits);
                }
            }
            Class::ClassMember { target, .. } => {
                for (u, a, b) in self.declarations(target) {
                    por(u, Span { start: a, end: b }, new, &mut edits);
                }
                for (u, s) in self.target_references(target) {
                    // The named `this.x` that becomes private: `{T x}` and the initializer
                    // `_new = x`.
                    if new.starts_with('_')
                        && let Some(e) = self.private_parameter(u, s, new)
                    {
                        edits.extend(e);
                        continue;
                    }
                    // The references of the `this.x`s (the start of an optional positional
                    // argument, of length 0, which becomes an insertion, and the `super.x`s)
                    // only count with a public new name (`fillChange`).
                    if new.starts_with('_') && (s.start == s.end || self.is_super_formal(u, s)) {
                        continue;
                    }
                    por(u, s, new, &mut edits);
                }
            }
            Class::Parameter { unit, declaration, field } => {
                if let Some(v) = field {
                    // The field: the declaration and all its references.
                    let target = self.variable_member(*v);
                    if let Some((u, s)) = self.variable_name(*v) {
                        por(u, s, new, &mut edits);
                    }
                    for (u, s) in self.target_references(&target) {
                        por(u, s, new, &mut edits);
                    }
                } else {
                    let target = Target::Local { unit: *unit, declaration: *declaration };
                    let un = p.unit(*unit);
                    let named = crate::project::parameter_in(&un.ast, *declaration).is_some_and(|(q, _)| q.kind == ast::ParameterKind::Named);
                    let end = word(&un.source, *declaration).map_or(*declaration, |s| s.end);
                    por(*unit, Span { start: *declaration, end: end }, new, &mut edits);
                    // `RenameParameterRefactoringImpl._getElements`: the named one of the
                    // overrides is renamed with the declaration (the occurrence rename mode,
                    // which includes the homonyms).
                    let references: Vec<(UnitId, Span)> = match self.occurrences(&target, true) {
                        Ok(oc) => oc
                            .into_iter()
                            .filter(|&(u, a, _)| !(u == *unit && a == *declaration))
                            .map(|(u, a, b)| (u, Span { start: a, end: b }))
                            .collect(),
                        Err(_) => self.target_references(&target),
                    };
                    for (u, s) in references {
                        // Without the implicit zero-length ones; for a positional, without the
                        // `super.x`s.
                        if s.start == s.end {
                            continue;
                        }
                        if !named && self.is_super_formal(u, s) {
                            continue;
                        }
                        por(u, s, new, &mut edits);
                    }
                    // The `[x]` of the member's doc.
                    for (u, s) in self.parameter_docs(*unit, *declaration) {
                        por(u, s, new, &mut edits);
                    }
                }
            }
            Class::Local { unit, declaration } | Class::TypeParameter { unit, declaration } => {
                let target = match &request.class {
                    Class::Local { .. } => Target::Local { unit: *unit, declaration: *declaration },
                    _ => Target::TypeParameter { unit: *unit, declaration: *declaration },
                };
                for (u, a, b) in self.occurrences(&target, true)? {
                    por(u, Span { start: a, end: b }, new, &mut edits);
                }
            }
            Class::Label { unit, declaration } => {
                let un = p.unit(*unit);
                let cx = Context::new(self, *unit);
                let end = word(&un.source, *declaration).map_or(*declaration, |s| s.end);
                por(*unit, Span { start: *declaration, end: end }, new, &mut edits);
                let name_ = &un.source[*declaration..end];
                for (k, no) in cx.tree.nos.iter().enumerate() {
                    if no.especie == "SimpleIdentifier"
                        && cx.parent(k).is_some_and(|x| matches!(cx.kind(x), "BreakStatement" | "ContinueStatement"))
                        && cx.node_text(k) == name_
                    {
                        por(*unit, Span { start: no.inicio, end: no.fim }, new, &mut edits);
                    }
                }
            }
            Class::Library(l) => {
                for &u in p.library(*l).units.iter() {
                    for d in p.unit(u).unit.directives.iter() {
                        let name_: &[ast::Name] = match &d.kind {
                            ast::DirectiveKind::Library { name } => name,
                            ast::DirectiveKind::PartOf { name, .. } => name,
                            _ => continue,
                        };
                        if let (Some(a), Some(b)) = (name_.first(), name_.last()) {
                            por(u, Span { start: a.span.start, end: b.span.end }, new, &mut edits);
                        }
                    }
                }
            }
        }
        Ok(edits)
    }

    /// The `super.x` in `s`.
    fn is_super_formal(&self, u: UnitId, s: Span) -> bool {
        let ast = &self.program().unit(u).ast;
        ast.members.iter().any(|m| match &m.kind {
            ast::MemberKind::Constructor(k) => k.parameters.iter().any(|q| q.super_ && q.name.is_some_and(|n| n.span == s)),
            _ => false,
        })
    }

    /// The `[x]`s of the documentation comment of the member that declares the
    /// parameter.
    fn parameter_docs(&self, unit: UnitId, declaration: usize) -> Vec<(UnitId, Span)> {
        let u = self.program().unit(unit);
        let name = word(&u.source, declaration).map(|s| u.source[s.start..s.end].to_string()).unwrap_or_default();
        let mut v = Vec::new();
        for c in crate::dartdoc::comments(&u.source) {
            for r in &c.references {
                if r.len() != 1 || r[0].1 != name {
                    continue;
                }
                // The comment documents the declaration right after it (member, top-level
                // declaration or local function), which declares the parameter.
                let declares = |ps: &[ast::Parameter]| ps.iter().any(|q| q.name.is_some_and(|n| n.span.start == declaration));
                let member = u.ast.members.iter().filter(|m| m.span.start >= c.span.end).min_by_key(|m| m.span.start);
                let top = u.ast.decls.iter().filter(|d| d.span.start >= c.span.end).min_by_key(|d| d.span.start);
                let function = u.ast.functions.iter().filter(|f| f.span.start >= c.span.end).min_by_key(|f| f.span.start);
                let pm = member.map_or(usize::MAX, |m| m.span.start);
                let pt = top.map_or(usize::MAX, |d| d.span.start);
                let pf = function.map_or(usize::MAX, |f| f.span.start);
                let documents = if pm <= pt && pm <= pf {
                    member.is_some_and(|m| match &m.kind {
                        ast::MemberKind::Constructor(k) => declares(&k.parameters),
                        ast::MemberKind::Method(f) => declares(u.ast.function(*f).parameters.as_deref().unwrap_or(&[])),
                        ast::MemberKind::Field(_) => false,
                    })
                } else if pt <= pf {
                    top.is_some_and(|d| matches!(&d.kind, ast::DeclKind::Function(f) if declares(u.ast.function(*f).parameters.as_deref().unwrap_or(&[]))))
                } else {
                    function.is_some_and(|f| declares(f.parameters.as_deref().unwrap_or(&[])))
                };
                if documents {
                    v.push((unit, r[0].0));
                }
            }
        }
        v
    }

    /// `_addPrivateNamedFormalParameterEdit` for reference `s` (the `x` of a
    /// `{this.x}`): `{T x}` and the initializer `_new = x`.
    fn private_parameter(&self, u: UnitId, s: Span, new: &str) -> Option<Vec<Edit>> {
        let p = self.program();
        let un = p.unit(u);
        let uri = self.unit_uri(u)?;
        for (mi, m) in un.ast.members.iter().enumerate() {
            let ast::MemberKind::Constructor(k) = &m.kind else { continue };
            let Some(q) = k.parameters.iter().find(|q| q.this_ && q.kind == ast::ParameterKind::Named && q.name.is_some_and(|n| n.span == s)) else { continue };
            let name = q.name?;
            let field = self
                .node_constructor(u, ast::MemberId(mi as u32))
                .and_then(|f| p.function(f).class)
                .and_then(|c| p.class(c).fields.iter().copied().find(|v| p.variable(*v).name == name.sym))?;
            let ty = self.query.variable_type(field).map(|t| self.query.format(t)).unwrap_or_else(|| "dynamic".to_string());
            let this_start = un.source[..name.span.start].rfind("this")?;
            let mut v = vec![Edit { uri: uri.clone(), span: Span { start: this_start, end: name.span.start }, text: format!("{ty} ") }];
            let x = &un.source[name.span.start..name.span.end];
            let end = k.parameters.last().map_or(name.span.end, |q| q.span.end);
            if k.initializers.is_empty() {
                let closes = un.source[end..].find(')').map_or(end, |i| end + i + 1);
                v.push(Edit { uri, span: Span { start: closes, end: closes }, text: format!(" : {new} = {x}") });
            } else {
                let colon = un.source[end..].find(':').map_or(end, |i| end + i + 1);
                v.push(Edit { uri, span: Span { start: colon, end: colon }, text: format!(" {new} = {x},") });
            }
            return Some(v);
        }
        None
    }

    /// `RenameImportRefactoringImpl.fillChange` (§12.8).
    fn import_edits(&self, lib: LibraryId, index: usize, new: &str) -> Vec<Edit> {
        let p = self.program();
        let imp = &p.library(lib).imports[index];
        let mut v = Vec::new();
        let Some(uri) = self.unit_uri(imp.unit) else { return v };
        let un = p.unit(imp.unit);
        let Some(d) = un.unit.directives.get(imp.directive) else { return v };
        let ast::DirectiveKind::Import { uri: lit, prefix, .. } = &d.kind else { return v };
        match (prefix, new.is_empty()) {
            (Some(pr), true) => v.push(Edit { uri: uri.clone(), span: Span { start: lit.span.end, end: pr.span.end }, text: String::new() }),
            (None, false) => v.push(Edit { uri: uri.clone(), span: Span { start: lit.span.end, end: lit.span.end }, text: format!(" as {new}") }),
            (Some(pr), false) => v.push(Edit { uri: uri.clone(), span: pr.span, text: new.to_string() }),
            (None, true) => {}
        }
        // The uses of the elements of the import's namespace.
        let namespace = &p.library(imp.library).exported;
        let visible = |s: SymbolId| {
            namespace.contains_key(&s)
                && imp.combinators.iter().all(|c| match c {
                    ast::Combinator::Show(ns) => ns.iter().any(|n| n.sym == s),
                    ast::Combinator::Hide(ns) => !ns.iter().any(|n| n.sym == s),
                })
        };
        let replacement = if new.is_empty() { String::new() } else { format!("{new}.") };
        let of_directive = v.len();
        for &x in p.library(lib).units.iter() {
            let ux = p.unit(x);
            let Some(uri_x) = self.unit_uri(x) else { continue };
            let fx = ux.source.as_str();
            match prefix {
                Some(pr) => {
                    // `RenameImportRefactoringImpl`: the `p.` of each use (with the dot)
                    // becomes `new.` (or disappears).
                    let until_next = |s: Span| {
                        let dot = fx[s.end..].find('.').map_or(s.end, |k| s.end + k + 1);
                        let spaces = fx[dot..].len() - fx[dot..].trim_start().len();
                        Span { start: s.start, end: dot + spaces }
                    };
                    let use_ = |pn: &ast::Name, v: &mut Vec<Edit>| {
                        v.push(Edit { uri: uri_x.clone(), span: until_next(pn.span), text: replacement.clone() });
                    };
                    for annotation in dartforge_frontend::pais::todas_as_anotacoes(&ux.ast, &ux.unit) {
                        if let [pn, n, ..] = &annotation.name[..]
                            && pn.sym == pr.sym
                            && visible(n.sym)
                        {
                            use_(pn, &mut v);
                        }
                    }
                    for e in ux.ast.exprs.iter() {
                        if let ExprKind::Property { target, name, .. } = &e.kind
                            && let ExprKind::Identifier(pn) = &ux.ast.expr(*target).kind
                            && pn.sym == pr.sym
                            && visible(name.sym)
                        {
                            use_(pn, &mut v);
                        }
                    }
                    for t in ux.ast.types.iter() {
                        if let ast::TypeKind::Named { name, .. } = &t.kind
                            && let [pn, n] = &name[..]
                            && pn.sym == pr.sym
                            && visible(n.sym)
                        {
                            use_(pn, &mut v);
                        }
                    }
                    // `[p.x]` in the documentation: the index also cites the import.
                    let prefix_name = &un.source[pr.span.start..pr.span.end];
                    if fx.contains('[') {
                        for c in crate::dartdoc::comments(fx) {
                            for r in &c.references {
                                if r.len() >= 2
                                    && r[0].1 == prefix_name
                                    && self.query.names.lookup(&r[1].1).is_some_and(visible)
                                {
                                    v.push(Edit { uri: uri_x.clone(), span: until_next(r[0].0), text: replacement.clone() });
                                }
                            }
                        }
                    }
                }
                None => {
                    // Without a prefix: `new.` before each unprefixed use of an element of the
                    // import (`"$x"` → `"${new.x}"`).
                    let bodies = &self.query.bodies.units[x.0 as usize];
                    for (i, e) in ux.ast.exprs.iter().enumerate() {
                        let ExprKind::Identifier(n) = &e.kind else { continue };
                        let Some(dartforge_types::Resolved::Element(el)) = bodies.get_resolved(ast::ExprId(i as u32)) else { continue };
                        if !namespace.get(&n.sym).is_some_and(|b| b.getter == Some(*el) || b.setter == Some(*el)) || !visible(n.sym) {
                            continue;
                        }
                        let interpolated = n.span.start > 0 && fx.as_bytes()[n.span.start - 1] == b'$';
                        if interpolated {
                            v.push(Edit { uri: uri_x.clone(), span: n.span, text: format!("{{{new}.{}}}", &fx[n.span.start..n.span.end]) });
                        } else {
                            v.push(Edit { uri: uri_x.clone(), span: Span { start: n.span.start, end: n.span.start }, text: replacement.clone() });
                        }
                    }
                    for t in ux.ast.types.iter() {
                        if let ast::TypeKind::Named { name, .. } = &t.kind
                            && let [n] = &name[..]
                            && visible(n.sym)
                            && import_type(p, x, n.sym, imp.library)
                        {
                            v.push(Edit { uri: uri_x.clone(), span: Span { start: n.span.start, end: n.span.start }, text: replacement.clone() });
                        }
                    }
                }
            }
        }
        // The uses in search order: by unit and by position.
        v[of_directive..].sort_by_key(|e| e.span.start);
        v
    }

    /// The file of the renamed class (§12.1 step 17): only a class, mixin, enum
    /// or extension type whose file is named `toFileName` of the old name.
    fn class_file(&self, request: &Request, new: &str) -> Option<RenameFile> {
        let Class::UnitMember(Target::Top(Element::Class(c))) = &request.class else { return None };
        let d = self.program().class(*c).decl?;
        let path = self.program().unit(d.unit).path.clone()?;
        if path.file_name()?.to_str()? != file_name(&request.old) {
            return None;
        }
        let target = path.with_file_name(file_name(new));
        Some(RenameFile { de: Url::from_file_path(&path).ok()?.to_string(), para: Url::from_file_path(&target).ok()?.to_string(), directives: Vec::new() })
    }

    /// The edits of `RenameConstructorRefactoringImpl.fillChange` (§12.7): each
    /// reference (`.name` or zero-length) becomes `.new` (unnamed: `.new` on a
    /// tear-off, nothing on the others; an enum constant without arguments gets
    /// `()`); an implicit call to the super constructor gets `super.new()`; the
    /// declaration replaces `.name` (or the empty span at the end of the class
    /// name) with `.new`.
    pub(crate) fn constructor_edits(&self, f: FunctionElementId, new: &str) -> Result<Vec<Edit>, String> {
        let p = self.program();
        let mut edits = Vec::new();
        let declaration = self.function_name(f);
        let new_dot = if new.is_empty() { String::new() } else { format!(".{new}") };
        for (u, start, end) in self.occurrences(&Target::Constructor(f), false)? {
            if !self.in_workspace(Some(u)) {
                continue;
            }
            let Some(uri) = self.unit_uri(u) else { continue };
            let unit = p.unit(u);
            let source = unit.source.as_str();
            let ast = &unit.ast;
            // The declaration.
            if declaration == Some((u, Span { start: start, end: end })) {
                let unnamed = self.name(p.function(f).name).is_empty();
                let span = if unnamed {
                    Span { start: end, end: end }
                } else {
                    let before = source[..start].trim_end();
                    Span { start: if before.ends_with('.') { before.len() - 1 } else { start }, end: end }
                };
                edits.push(Edit { uri, span, text: new_dot.clone() });
                continue;
            }
            // A subclass without a declared constructor: the class name.
            let class_without_constructor = ast.decls.iter().find_map(|d| match &d.kind {
                DeclKind::Class(c) if c.name.span.start == start && c.name.span.end == end => Some(d),
                _ => None,
            });
            if let Some(d) = class_without_constructor {
                let DeclKind::Class(c) = &d.kind else { continue };
                let key = source[c.name.span.end..d.span.end].find('{').map(|k| c.name.span.end + k + 1);
                if let Some(o) = key {
                    let name = &source[c.name.span.start..c.name.span.end];
                    let call = if new.is_empty() { "super()".to_string() } else { format!("super.{new}()") };
                    edits.push(Edit { uri, span: Span { start: o, end: o }, text: format!("\n  {name}() : {call};") });
                }
                continue;
            }
            // The subclass constructor without an explicit `super(...)`.
            let implicit = ast.members.iter().find_map(|m| match &m.kind {
                ast::MemberKind::Constructor(k) if k.class_name.span.start == start => Some(k),
                _ => None,
            });
            if let Some(k) = implicit {
                let call = if new.is_empty() { "super()".to_string() } else { format!("super.{new}()") };
                let (o, text) = match k.initializers.last() {
                    Some(last) => {
                        let s = match last {
                            ast::Initializer::Field { span, .. }
                            | ast::Initializer::Super { span, .. }
                            | ast::Initializer::Redirect { span, .. }
                            | ast::Initializer::Assert { span, .. } => *span,
                        };
                        (s.end, format!(", {call}"))
                    }
                    None => {
                        let parameter_end = k.parameters.last().map_or(k.name.map_or(k.class_name.span.end, |n| n.span.end), |q| q.span.end);
                        let closes = source[parameter_end..].find(')').map_or(parameter_end, |x| parameter_end + x + 1);
                        (closes, format!(" : {call}"))
                    }
                };
                edits.push(Edit { uri, span: Span { start: o, end: o }, text });
                continue;
            }
            // The other references.
            let tear_off = source[start..end] == *".new" && !source[end..].trim_start().starts_with('(');
            let mut text = if new.is_empty() {
                if tear_off { ".new".to_string() } else { String::new() }
            } else {
                new_dot.clone()
            };
            let constant_without_arguments = start == end
                && ast.decls.iter().any(|d| match &d.kind {
                    DeclKind::Enum(e) => e.constants.iter().any(|c| c.name.span.end == start && c.arguments.is_none()),
                    _ => false,
                });
            if constant_without_arguments {
                text.push_str("()");
            }
            edits.push(Edit { uri, span: Span { start: start, end: end }, text });
        }
        Ok(edits)
    }
}

/// The simple name `n` of unit `u` resolves to the element that library
/// `target` exports (the unprefixed import brings it).
fn import_type(p: &dartforge_elements::model::Program, u: UnitId, n: SymbolId, target: LibraryId) -> bool {
    match (p.lookup_na_unidade(u, n).and_then(|b| b.getter), p.library(target).exported.get(&n).and_then(|b| b.getter)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// `capitalize`.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The type parameter lists of the unit.
fn type_parameter_lists(ast: &ast::Ast) -> Vec<Vec<&ast::TypeParameter>> {
    let mut v: Vec<Vec<&ast::TypeParameter>> = Vec::new();
    for d in &ast.decls {
        let ps = match &d.kind {
            DeclKind::Class(c) => &c.type_params,
            DeclKind::Mixin(m) => &m.type_params,
            DeclKind::Enum(e) => &e.type_params,
            DeclKind::Extension(x) => &x.type_params,
            DeclKind::ExtensionType(x) => &x.type_params,
            DeclKind::Typedef(t) => &t.type_params,
            _ => continue,
        };
        v.push(ps.iter().collect());
    }
    for f in &ast.functions {
        v.push(f.type_params.iter().collect());
    }
    for t in &ast.types {
        if let ast::TypeKind::Function { type_params, .. } = &t.kind {
            v.push(type_params.iter().collect());
        }
    }
    v
}

/// `prepareRename`: the range from `getElementToRename` and the `oldName`;
/// `Ok(None)` with nothing renameable.
///
/// # Errors
///
/// `checkInitialConditions` fatal: the message (`-32010`).
pub(crate) fn prepare(project: &Project, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
    let Some(unit) = project.uri_unit(uri) else { return Ok(None) };
    let Some(request) = project.rename_request(unit, offset) else { return Ok(None) };
    let initial = project.initial_conditions(&request);
    if initial.has_fatal() {
        return Err(initial.message().unwrap_or_default());
    }
    Ok(Some((request.range, request.old)))
}

/// `rename`: the edits, or the `-32010` message.
///
/// # Errors
///
/// `checkInitialConditions` fatal, `checkNewName` with an error, or
/// `checkFinalConditions` fatal: the message.
pub(crate) fn rename(project: &Project, uri: &str, offset: usize, new: &str) -> Result<Rename, String> {
    let Some(unit) = project.uri_unit(uri) else {
        return Ok(Rename { null: true, ..Default::default() });
    };
    let Some(request) = project.rename_request(unit, offset) else {
        return Ok(Rename { null: true, ..Default::default() });
    };
    let initial = project.initial_conditions(&request);
    if initial.has_fatal() {
        return Err(initial.message().unwrap_or_default());
    }
    let name = project.verify_new_name(&request, new);
    if name.has_error() {
        return Err(name.message().unwrap_or_default());
    }
    let finals = project.final_conditions(&request, new);
    if finals.has_fatal() {
        return Err(finals.message().unwrap_or_default());
    }
    let warning = if finals.ok() { None } else { finals.message() };
    let edits = project.rename_edits(&request, new)?;
    let file = project.class_file(&request, new);
    Ok(Rename { edits, file, warning, null: false })
}

#[cfg(test)]
mod tests {
    use super::file_name;

    #[test]
    fn file_names() {
        assert_eq!(file_name("MinhaClasse"), "minha_classe.dart");
        assert_eq!(file_name("HTTPServer"), "h_t_t_p_server.dart");
        assert_eq!(file_name("A"), "a.dart");
        assert_eq!(file_name("_Foo"), "__foo.dart");
    }
}
