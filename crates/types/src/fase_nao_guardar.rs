//! `assignment_of_do_not_store` and `return_of_do_not_store`
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, batch II.7): the 3.6.2
//! `BestPracticesVerifier` (`_checkForAssignmentOfDoNotStore`,
//! `_checkForReturnOfDoNotStore` and `_getSubExpressionsMarkedDoNotStore`,
//! `analyzer/lib/src/error/best_practices_verifier.dart`).
//!
//! The "marked" subexpressions: the property (`PropertyAccess` and
//! `PrefixedIdentifier`), the identifier and the method call
//! (`MethodInvocation`, not instance creation) whose element has
//! `hasOrInheritsDoNotStore` (on it; on the enclosing class, mixin, enum,
//! extension type or extension; or, for what is in a unit, on the library),
//! except the tear-off of a function or method; a synthetic accessor counts
//! through the variable. `?:` and a binary operator descend into both sides;
//! the function expression `=> e` descends into the body; a parenthesis does
//! not descend. The reported name is the element's `name` (the written
//! identifier).
//!
//! The initializer of a top-level variable and of a field gives
//! `assignment_of_do_not_store`; `return e;` and `=> e`, outside
//! `_inDoNotStoreMember`, give `return_of_do_not_store` with the name of the
//! nearest function or method declaration that contains them (without one,
//! nothing). `_inDoNotStoreMember` starts with the library and is turned on
//! only by the marked class (`visitClassDeclaration`), top-level or local
//! function (`visitFunctionDeclaration`) and method
//! (`visitMethodDeclaration`). Nothing in a unit in a test directory
//! (`inTestDir`: `test`, `integration_test`, `test_driver`, `testing`).
//!
//! The annotation is that of `package:meta`, resolved through the scope of
//! the unit in which it is written (`fase_resultado::meta_annotation`). The
//! library metadata is that of the `library` directive or, without it, that
//! of the first directive (`ElementBuilder._buildLibraryMetadata`). That of
//! a local variable and of a local function comes from
//! `Ast::metadados_locais`; that of a parameter, from the node.
//! Written without compiling or running (2026-10-05).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codes::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{Element, Program, UnitId};
use dartforge_frontend::ast::{self, DeclKind, DirectiveKind, ExprId, ExprKind, ForInTarget, FunctionBody, MemberKind, StmtId, StmtKind};
use dartforge_intern::Interner;

/// The test directories of `CompilationUnitExtension.inTestDir`.
const TEST_DIRECTORIES: [&str; 4] = ["test", "integration_test", "test_driver", "testing"];

fn inside(a: Span, b: Span) -> bool {
    a.start >= b.start && a.end <= b.end
}

/// The list, written in unit `unit`, has the `@doNotStore` of `package:meta`.
fn the_marked(program: &Program, interner: &Interner, unit: UnitId, metadata: &[ast::Annotation]) -> bool {
    metadata.iter().any(|m| crate::fase_resultado::meta_annotation(program, interner, unit, m, "doNotStore"))
}

/// `LibraryElement.hasDoNotStore` of the library of unit `u`: the metadata of
/// the `library` directive of the unit that defines the library or, without
/// it, that of the first directive.
fn marked_library(program: &Program, interner: &Interner, u: UnitId) -> bool {
    let lib = program.library(program.unit(u).library);
    lib.units.first().is_some_and(|&d| {
        let ds = &program.unit(d).unit.directives;
        let tgt = ds.iter().find(|x| matches!(x.kind, DirectiveKind::Library { .. })).or_else(|| ds.first());
        tgt.is_some_and(|x| the_marked(program, interner, d, &x.metadata))
    })
}

/// What the declaration of a local is: the metadata and whether it is a local
/// function.
struct LocalDeclaration<'a> {
    metadata: &'a [ast::Annotation],
    function: bool,
}

/// The metadata of a statement (`Ast::metadados_locais`).
fn statement_metadata(a: &ast::Ast, s: StmtId) -> &[ast::Annotation] {
    a.metadados_locais.iter().find(|(x, _)| *x == s).map(|(_, m)| &**m).unwrap_or(&[])
}

/// The parameter of `ps` declared at offset `decl`.
fn parameter_at(ps: &[ast::Parameter], decl: usize) -> Option<&ast::Parameter> {
    ps.iter().find(|p| p.name.is_some_and(|n| n.span.start == decl))
}

/// The declaration of the local declared at offset `decl`: parameter, local
/// variable, local function or `for-in` variable.
fn local_declaration(a: &ast::Ast, decl: usize) -> Option<LocalDeclaration<'_>> {
    for f in a.functions.iter() {
        if let Some(p) = f.parameters.as_deref().and_then(|ps| parameter_at(ps, decl)) {
            return Some(LocalDeclaration { metadata: &p.metadata, function: false });
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && let Some(p) = parameter_at(&k.parameters, decl)
        {
            return Some(LocalDeclaration { metadata: &p.metadata, function: false });
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        let id = StmtId(i as u32);
        match &s.kind {
            StmtKind::Variables(l) if l.variables.iter().any(|v| v.name.span.start == decl) => {
                return Some(LocalDeclaration { metadata: statement_metadata(a, id), function: false });
            }
            StmtKind::Function(f) if a.function(*f).name.is_some_and(|n| n.span.start == decl) => {
                return Some(LocalDeclaration { metadata: statement_metadata(a, id), function: true });
            }
            StmtKind::ForIn { target: ForInTarget::Declared { metadata, name, .. }, .. } if name.span.start == decl => {
                return Some(LocalDeclaration { metadata, function: false });
            }
            _ => {}
        }
    }
    None
}

/// The parameter `the_name` of the innermost function or constructor that
/// contains `e` and declares it (`Resolved::Parameter`).
fn parameter_declaration(a: &ast::Ast, e: Span, the_name: dartforge_intern::SymbolId) -> Option<LocalDeclaration<'_>> {
    let mut best: Option<(usize, &ast::Parameter)> = None;
    let mut lists: Vec<(Span, &[ast::Parameter])> = Vec::new();
    for f in a.functions.iter() {
        if let Some(ps) = f.parameters.as_deref() {
            lists.push((f.span, ps));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind {
            lists.push((m.span, &k.parameters[..]));
        }
    }
    for (sp, ps) in lists {
        if !inside(e, sp) {
            continue;
        }
        if let Some(p) = ps.iter().find(|p| p.name.is_some_and(|n| n.sym == the_name)) {
            let size = sp.end - sp.start;
            if best.is_none_or(|(t, _)| size < t) {
                best = Some((size, p));
            }
        }
    }
    best.map(|(_, p)| LocalDeclaration { metadata: &p.metadata, function: false })
}

/// `hasOrInheritsDoNotStore` of the resolved element `r` of expression `e`
/// (`u` is the unit of the expression, for the locals).
fn marked_element(program: &Program, interner: &Interner, u: UnitId, body: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let a = &program.unit(u).ast;
    match r {
        // A local element: only its own metadata (what contains it is a function,
        // neither a class nor a unit).
        Resolved::Local(_) => body
            .declaracao_local(e)
            .and_then(|d| local_declaration(a, d))
            .is_some_and(|d| the_marked(program, interner, u, d.metadata)),
        Resolved::Parameter { name, .. } => {
            parameter_declaration(a, a.expr(e).span, *name).is_some_and(|d| the_marked(program, interner, u, d.metadata))
        }
        _ => {
            let (metadata, where_) = crate::fase_resultado::element_annotations(program, r);
            if where_.is_some_and(|w| the_marked(program, interner, w, metadata)) {
                return true;
            }
            // The class or extension that contains it; then, the library.
            match r {
                Resolved::Member { class, .. } => match program.class(*class).decl {
                    Some(d) => {
                        let a = &program.unit(d.unit).ast;
                        the_marked(program, interner, d.unit, &a.decl(d.decl).metadata) || marked_library(program, interner, d.unit)
                    }
                    None => false,
                },
                Resolved::ExtensionMember { extension, .. } => {
                    let d = program.extension(*extension).decl;
                    let a = &program.unit(d.unit).ast;
                    the_marked(program, interner, d.unit, &a.decl(d.decl).metadata) || marked_library(program, interner, d.unit)
                }
                _ => where_.is_some_and(|w| marked_library(program, interner, w)),
            }
        }
    }
}

/// The element is a function or a method (`FunctionElement`,
/// `MethodElement`; not a getter, setter or variable accessor): the
/// identifier and the property that reference it are a tear-off.
fn e_tear_off(program: &Program, a: &ast::Ast, body: &UnitBodyTypes, e: ExprId, r: &Resolved) -> bool {
    let f = match r {
        Resolved::Element(Element::Function(f)) | Resolved::Member { member: MemberRef::Function(f), .. } => *f,
        Resolved::ExtensionMember { member, .. } => *member,
        // A local function is a `FunctionElement`.
        Resolved::Local(_) => return body.declaracao_local(e).and_then(|d| local_declaration(a, d)).is_some_and(|d| d.function),
        _ => return false,
    };
    let g = program.function(f);
    g.variable.is_none() && matches!(g.kind, dartforge_elements::model::FunctionKind::Function | dartforge_elements::model::FunctionKind::Operator)
}

/// `_getSubExpressionsMarkedDoNotStore`: the marked subexpressions, with the
/// name to report.
fn marked_(program: &Program, interner: &Interner, u: UnitId, body: &UnitBodyTypes, e: ExprId, output: &mut Vec<(Span, String)>) {
    let a = &program.unit(u).ast;
    let resolved = |x: ExprId| body.get_resolved(x);
    let expr = a.expr(e);
    // (the expression whose element counts, the element, the name)
    let finding: Option<(ExprId, &Resolved, String)> = match &expr.kind {
        ExprKind::Property { name, .. } => {
            resolved(e).filter(|r| !e_tear_off(program, a, body, e, r)).map(|r| (e, r, interner.resolve(name.sym).to_string()))
        }
        ExprKind::Identifier(n) => {
            resolved(e).filter(|r| !e_tear_off(program, a, body, e, r)).map(|r| (e, r, interner.resolve(n.sym).to_string()))
        }
        // `MethodInvocation`: `f(…)`, `o.m(…)`; an instance creation without `new`
        // (`C()`, `C.the_name()`, `p.C()`) is an `InstanceCreationExpression` in the
        // analyzer and does not count.
        ExprKind::Call { target, .. } if !matches!(resolved(e), Some(Resolved::Constructor(_))) => match &a.expr(*target).kind {
            ExprKind::Identifier(n) | ExprKind::Property { name: n, .. } => {
                match resolved(*target) {
                    Some(r) => Some((*target, r, interner.resolve(n.sym).to_string())),
                    None => resolved(e).map(|r| (e, r, interner.resolve(n.sym).to_string())),
                }
            }
            _ => None,
        },
        ExprKind::Conditional { then, else_, .. } => {
            marked_(program, interner, u, body, *else_, output);
            marked_(program, interner, u, body, *then, output);
            None
        }
        ExprKind::Binary { left, right, .. } => {
            marked_(program, interner, u, body, *left, output);
            marked_(program, interner, u, body, *right, output);
            None
        }
        ExprKind::FunctionExpression(f) => {
            if let FunctionBody::Expression(x) = &a.function(*f).body {
                marked_(program, interner, u, body, *x, output);
            }
            None
        }
        _ => None,
    };
    if let Some((x, r, the_name)) = finding
        && marked_element(program, interner, u, body, x, r)
    {
        output.push((expr.span, the_name));
    }
}

/// The reports of unit `u`.
pub fn guardados_e_devolvidos(program: &Program, interner: &Interner, body: &UnitBodyTypes, u: UnitId) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if interner.lookup("doNotStore").is_none() {
        return out;
    }
    let unit_id = program.unit(u);
    // `inTestDir`: a directory from the list in the path.
    let in_test = unit_id
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .is_some_and(|d| d.components().any(|c| TEST_DIRECTORIES.iter().any(|t| c.as_os_str() == *t)));
    if in_test {
        return out;
    }
    let a = &unit_id.ast;

    // `assignment_of_do_not_store`: the top-level and field initializers.
    let mut initial: Vec<ExprId> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            initial.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            initial.extend(l.variables.iter().filter_map(|v| v.initializer));
        }
    }
    for e in initial {
        let mut findings = Vec::new();
        marked_(program, interner, u, body, e, &mut findings);
        for (s, the_name) in findings {
            out.push(Diagnostic::with_code(w::ASSIGNMENT_OF_DO_NOT_STORE, s, [the_name.as_str()]));
        }
    }

    // `return_of_do_not_store`. The function (top-level and local) and method
    // declarations: the span, the `displayName` and whether they turn on
    // `_inDoNotStoreMember`.
    let name_of = |g: &ast::Function| g.name.map(|n| interner.resolve(n.sym).to_string()).unwrap_or_default();
    let mut declarations: Vec<(Span, String, bool)> = Vec::new();
    for d in a.decls.iter() {
        if let DeclKind::Function(f) = &d.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, &d.metadata)));
        }
    }
    for (i, s) in a.stmts.iter().enumerate() {
        if let StmtKind::Function(f) = &s.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, statement_metadata(a, StmtId(i as u32)))));
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Method(f) = &m.kind {
            let g = a.function(*f);
            declarations.push((g.span, name_of(g), the_marked(program, interner, u, &m.metadata)));
        }
    }
    // The spans with `_inDoNotStoreMember` on: the marked declarations and the
    // marked classes (only `visitClassDeclaration` turns it on).
    let mut marked: Vec<Span> = declarations.iter().filter(|x| x.2).map(|x| x.0).collect();
    for d in a.decls.iter() {
        if matches!(d.kind, DeclKind::Class(_)) && the_marked(program, interner, u, &d.metadata) {
            marked.push(d.span);
        }
    }
    if marked_library(program, interner, u) {
        return out;
    }
    let mut returned: Vec<ExprId> = Vec::new();
    for s in a.stmts.iter() {
        if let StmtKind::Return(Some(e)) = &s.kind {
            returned.push(*e);
        }
    }
    for f in a.functions.iter() {
        if let FunctionBody::Expression(e) = &f.body {
            returned.push(*e);
        }
    }
    for e in returned {
        let sp = a.expr(e).span;
        if marked.iter().any(|&m| inside(sp, m)) {
            continue;
        }
        let mut findings = Vec::new();
        marked_(program, interner, u, body, e, &mut findings);
        if findings.is_empty() {
            continue;
        }
        // The nearest function or method declaration that contains the return.
        let Some(owner) = declarations.iter().filter(|x| inside(sp, x.0)).min_by_key(|x| x.0.end - x.0.start) else { continue };
        for (s, the_name) in findings {
            out.push(Diagnostic::with_code(w::RETURN_OF_DO_NOT_STORE, s, [the_name.as_str(), owner.1.as_str()]));
        }
    }
    out
}
