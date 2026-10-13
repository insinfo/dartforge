//! Document symbols from the same AST used by the diagnostics. The tree is
//! temporary: no previous revision stays in the server.

use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{Ast, DeclKind, DirectiveKind, ExprKind, MemberKind, Name, StmtKind, TypeKind, TypedefKind};
use dartforge_frontend::LibraryFeatures;
use dartforge_intern::Interner;
use serde_json::{Value, json};

use crate::utf16::LineTable;

pub(super) fn of_document(text: &str, features: LibraryFeatures) -> Vec<Value> {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
    let lines = LineTable::build(text);
    let mut output = Vec::new();
    // The `group`/`test` symbols: only in a unit that imports a `…test.dart`
    // file and declares no top-level function with one of those names.
    let imports_tests = parsed.unit.directives.iter().any(|d| match &d.kind {
        DirectiveKind::Import { uri, .. } => dartforge_elements::load::string_lit_value(uri).is_some_and(|u| u.ends_with("test.dart")),
        _ => false,
    });
    let declares_homonym = parsed.unit.declarations.iter().any(|d| match &parsed.ast.decl(*d).kind {
        DeclKind::Function(f) => parsed.ast.function(*f).name.is_some_and(|n| matches!(names.resolve(n.sym), "group" | "test")),
        _ => false,
    });
    let tests = imports_tests && !declares_homonym;
    for id in &parsed.unit.declarations {
        let decl = parsed.ast.decl(*id);
        let (name, kind, members) = match &decl.kind {
            DeclKind::Class(c) => (Some(c.name), 5, Some(c.members.as_slice())),
            DeclKind::Mixin(m) => (Some(m.name), 5, Some(m.members.as_slice())),
            DeclKind::Enum(e) => {
                let mut children = e.constants.iter().map(|c| {
                    symbol(text, &lines, &names, c.name, c.span, 22, Vec::new())
                }).collect::<Vec<_>>();
                children.extend(member_symbols(
                    text, &lines, &names, &parsed.ast, &e.members, tests,
                ));
                output.push(symbol(text, &lines, &names, e.name, decl.span, 10, children));
                continue;
            }
            // Extensions and extension types are `Namespace` in the Dart server.
            DeclKind::Extension(e) => (e.name, 3, Some(e.members.as_slice())),
            DeclKind::ExtensionType(e) => (Some(e.name), 3, Some(e.members.as_slice())),
            // `typedef F = void Function();` and the old form are FUNCTION_TYPE_ALIAS
            // (5); `typedef A = int;` is TYPE_ALIAS, which Dart's
            // `elementKindToSymbolKind` does not handle and comes out as 19.
            DeclKind::Typedef(t) => {
                let of_function = match &t.kind {
                    TypedefKind::Legacy { .. } => true,
                    TypedefKind::Alias(ty) => matches!(parsed.ast.ty(*ty).kind, TypeKind::Function { .. }),
                };
                (Some(t.name), if of_function { 5 } else { 19 }, None)
            }
            DeclKind::Function(f) => {
                let func = parsed.ast.function(*f);
                let kind = match func.kind {
                    dartforge_frontend::ast::FunctionKind::Getter | dartforge_frontend::ast::FunctionKind::Setter => 7,
                    _ => 12,
                };
                (func.name, kind, None)
            }
            DeclKind::Variables(v) => {
                for variable in v.variables.iter() {
                    output.push(symbol(
                        text, &lines, &names, variable.name, decl.span,
                        13, Vec::new(),
                    ));
                }
                continue;
            }
        };
        let Some(name) = name else { continue };
        let children = match members {
            Some(ids) => member_symbols(text, &lines, &names, &parsed.ast, ids, tests),
            // Top-level function: the local functions of the body, recursively.
            None if matches!(decl.kind, DeclKind::Function(_)) => local_functions(text, &lines, &names, &parsed.ast, decl.span, tests),
            None => Vec::new(),
        };
        output.push(symbol(text, &lines, &names, name, decl.span, kind, children));
    }
    output
}

fn member_symbols(
    text: &str,
    lines: &LineTable,
    names: &Interner,
    ast: &Ast,
    members: &[dartforge_frontend::ast::MemberId],
    tests: bool,
) -> Vec<Value> {
    let mut output = Vec::new();
    for id in members {
        let member = ast.member(*id);
        match &member.kind {
            MemberKind::Method(f) => {
                let func = ast.function(*f);
                if let Some(name) = func.name {
                    let kind = match func.kind {
                        dartforge_frontend::ast::FunctionKind::Getter
                        | dartforge_frontend::ast::FunctionKind::Setter => 7,
                        _ => 6,
                    };
                    let locals = local_functions(text, lines, names, ast, member.span, tests);
                    output.push(symbol(text, lines, names, name, member.span, kind, locals));
                }
            }
            MemberKind::Constructor(c) => {
                // Like the Dart outline: `Class.name` (or `Class`).
                let name = c.name.unwrap_or(c.class_name);
                let locals = local_functions(text, lines, names, ast, member.span, tests);
                let mut s = symbol(text, lines, names, name, member.span, 9, locals);
                if let Some(n) = c.name {
                    s["name"] = json!(format!("{}.{}", names.resolve(c.class_name.sym), names.resolve(n.sym)));
                }
                output.push(s);
            }
            MemberKind::Field(v) => {
                for variable in v.variables.iter() {
                    output.push(symbol(
                        text, lines, names, variable.name, member.span,
                        8, Vec::new(),
                    ));
                }
            }
        }
    }
    output
}

/// The local functions and the tests declared inside `inside` (the body of a
/// function, method or constructor), as children: the Dart outline lists
/// them recursively, including those inside closures
/// (`computer_outline.dart:450-548`). The tree comes from range containment:
/// an item is a child of the smallest one that contains it.
///
/// With `tests`, a `group(...)` or `test(...)` call becomes the symbol
/// `group("description")` (kind 6), with the call's range and the selection
/// on the name; what is inside a `group` is its child, and nothing inside a
/// `test` is listed. The original decides by the resolved element (the
/// top-level function of a `…test.dart` file, or a function with `@isTest`
/// or `@isTestGroup`); here the unprefixed call counts in a unit that
/// imports a `…test.dart` and declares no top-level function with that name.
fn local_functions(text: &str, lines: &LineTable, names: &Interner, ast: &Ast, inside: Span, tests: bool) -> Vec<Value> {
    /// An outline item of a body.
    struct Item {
        span: Span,
        name: Name,
        /// The displayed name of a test (`group("x")`); `None` for a function.
        test_only: Option<String>,
        /// A `test`: nothing inside it is included.
        leaf: bool,
    }
    let mut found: Vec<Item> = Vec::new();
    for s in ast.stmts.iter() {
        let StmtKind::Function(f) = &s.kind else { continue };
        if s.span.start < inside.start || s.span.end > inside.end {
            continue;
        }
        if let Some(name) = ast.function(*f).name {
            found.push(Item { span: s.span, name, test_only: None, leaf: false });
        }
    }
    if tests {
        for e in ast.exprs.iter() {
            let ExprKind::Call { target, arguments } = &e.kind else { continue };
            if e.span.start < inside.start || e.span.end > inside.end {
                continue;
            }
            let ExprKind::Identifier(name) = &ast.expr(*target).kind else { continue };
            let call = names.resolve(name.sym);
            if call != "group" && call != "test" {
                continue;
            }
            // `extractString`: the value of the string literal, else the text of the
            // first argument, else `unnamed`.
            let description = match arguments.args.first() {
                None => "unnamed".to_string(),
                Some(first) => {
                    let expr = ast.expr(first.value);
                    let value = match &expr.kind {
                        ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                        _ => None,
                    };
                    value.unwrap_or_else(|| text.get(expr.span.start..expr.span.end).unwrap_or("").to_string())
                }
            };
            found.push(Item { span: e.span, name: *name, test_only: Some(format!("{call}(\"{description}\")")), leaf: call == "test" });
        }
    }
    // Pre-order: by start and, on a tie, the larger first.
    found.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(b.span.end.cmp(&a.span.end)));
    fn build(text: &str, lines: &LineTable, names: &Interner, found: &[Item], i: &mut usize, limit: usize) -> Vec<Value> {
        let mut output = Vec::new();
        while *i < found.len() && found[*i].span.start < limit {
            let item = &found[*i];
            *i += 1;
            let children = if item.leaf {
                // Skips everything inside the test.
                while *i < found.len() && found[*i].span.start < item.span.end {
                    *i += 1;
                }
                Vec::new()
            } else {
                build(text, lines, names, found, i, item.span.end)
            };
            let mut s = symbol(text, lines, names, item.name, item.span, if item.test_only.is_some() { 6 } else { 12 }, children);
            if let Some(shown) = &item.test_only {
                s["name"] = json!(shown);
            }
            output.push(s);
        }
        output
    }
    let mut i = 0;
    build(text, lines, names, &found, &mut i, inside.end)
}

fn symbol(
    text: &str,
    lines: &LineTable,
    names: &Interner,
    name: Name,
    span: Span,
    kind: u32,
    children: Vec<Value>,
) -> Value {
    let full_range = range(text, lines, span);
    let selection_range = range(text, lines, name.span);
    json!({
        "name": names.resolve(name.sym),
        "kind": kind,
        "range": full_range,
        "selectionRange": selection_range,
        "children": children,
    })
}

fn range(text: &str, lines: &LineTable, span: Span) -> Value {
    let start = span.start.min(text.len());
    let end = span.end.max(start).min(text.len());
    let (l0, c0) = lines.offset_position(text, start);
    let (l1, c1) = lines.offset_position(text, end);
    json!({
        "start": {"line": l0, "character": c0},
        "end": {"line": l1, "character": c1},
    })
}
