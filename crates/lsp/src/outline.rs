//! The document outline in the Dart server's format
//! (`dart/textDocument/publishOutline`): the `Outline` of
//! `DartUnitOutlineComputer` converted by `toOutline` and `toElement`
//! (`AS:src/computer/computer_outline.dart` and
//! `AS:src/lsp/mapping.dart:1197-1317` of 3.6.2, read in full;
//! docs/LSP-ESPECIFICACAO.md §3.4): `{element: {name, kind, range,
//! parameters, typeParameters, returnType}, range, codeRange, children}`.
//!
//! `range` is the node with its documentation comment and annotations;
//! `codeRange` starts at the first token after them. The texts of
//! parameters, type parameters and types are the source's, without the
//! `toSource` normalization. The tree is the parser's (no resolution): the
//! `group`/`test` nodes follow the rule of `crate::symbols`, and there are
//! no Flutter widget creation nodes.
//! Written without compiling or running (2026-10-05).

use crate::utf16::LineTable;
use dartforge_diagnostics::Span;
use dartforge_frontend::ast::{
    Annotation, Ast, DeclKind, DirectiveKind, ExprKind, FunctionId, FunctionKind, MemberId, MemberKind, Name, StmtKind, TypeKind,
    TypedefKind, VariableList,
};
use dartforge_intern::Interner;
use serde_json::{Value, json};

struct Ctx<'a> {
    text: &'a str,
    lines: LineTable,
    names: &'a Interner,
    ast: &'a Ast,
    tests: bool,
}

/// The balanced pair `opens`…`closes` that starts at `de` (after
/// whitespace): the range including the delimiters.
fn balanced(text: &str, de: usize, opens: u8, closes: u8) -> Option<Span> {
    let b = text.as_bytes();
    let mut i = de;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if b.get(i) != Some(&opens) {
        return None;
    }
    let start = i;
    let mut level = 0usize;
    while i < b.len() {
        if b[i] == opens {
            level += 1;
        } else if b[i] == closes {
            level -= 1;
            if level == 0 {
                return Some(Span { start: start, end: i + 1 });
            }
        }
        i += 1;
    }
    None
}

impl Ctx<'_> {
    fn snippet(&self, s: Span) -> &str {
        self.text.get(s.start..s.end).unwrap_or("")
    }

    fn range(&self, s: Span) -> Value {
        let (l0, c0) = self.lines.offset_position(self.text, s.start.min(self.text.len()));
        let (l1, c1) = self.lines.offset_position(self.text, s.end.clamp(s.start, self.text.len()));
        json!({"start": {"line": l0, "character": c0}, "end": {"line": l1, "character": c1}})
    }

    /// The start of the node with the documentation comment that precedes it:
    /// the contiguous `///` lines, or a `/** */` block.
    fn with_documentation(&self, start: usize) -> usize {
        let mut current = start;
        loop {
            let before = self.text[..current].trim_end();
            if before.ends_with("*/") {
                return match before[..before.len() - 2].rfind("/**") {
                    Some(k) => k,
                    None => current,
                };
            }
            let line_start_ = before.rfind('\n').map_or(0, |i| i + 1);
            let line = &before[line_start_..];
            let indent = line.len() - line.trim_start().len();
            if line.trim_start().starts_with("///") {
                current = line_start_ + indent;
            } else {
                return current;
            }
        }
    }

    /// `firstTokenAfterCommentAndMetadata`: after the last annotation.
    fn after_annotations(&self, start: usize, metadata: &[Annotation]) -> usize {
        let end = metadata.iter().map(|m| m.span.end).max().unwrap_or(start).max(start);
        let rest = &self.text[end.min(self.text.len())..];
        end + (rest.len() - rest.trim_start().len())
    }

    /// `_nodeOutline` of an annotated node.
    fn no(&self, element: Value, span: Span, metadata: &[Annotation], children: Vec<Value>) -> Value {
        let start = self.with_documentation(span.start);
        let code = self.after_annotations(span.start, metadata);
        let mut v = json!({
            "element": element,
            "range": self.range(Span { start: start, end: span.end }),
            "codeRange": self.range(Span { start: code.min(span.end), end: span.end }),
        });
        if !children.is_empty() {
            v["children"] = json!(children);
        }
        v
    }

    /// `toElement`: null fields are not emitted.
    fn element(&self, kind: &str, name: &str, local: Option<Span>, parameters: Option<&str>, of_type: Option<&str>, return_: Option<&str>) -> Value {
        let shown = if !name.is_empty() {
            name
        } else if kind == "EXTENSION" {
            "<unnamed extension>"
        } else {
            "<unnamed>"
        };
        let mut v = json!({"name": shown, "kind": kind});
        if let Some(s) = local {
            v["range"] = self.range(s);
        }
        if let Some(p) = parameters {
            v["parameters"] = json!(p);
        }
        if let Some(t) = of_type {
            v["typeParameters"] = json!(t);
        }
        if let Some(r) = return_ {
            v["returnType"] = json!(r);
        }
        v
    }

    /// The type parameters `<…>` written right after `after`.
    fn type_parameters(&self, after: usize) -> Option<Span> {
        balanced(self.text, after, b'<', b'>')
    }

    /// The `(…)` list written after the name and the type parameters.
    fn parameter_list(&self, after: usize) -> Option<Span> {
        let de = self.type_parameters(after).map_or(after, |s| s.end);
        balanced(self.text, de, b'(', b')')
    }

    fn ty(&self, t: Option<dartforge_frontend::ast::TypeId>) -> &str {
        t.map_or("", |t| self.snippet(self.ast.ty(t).span))
    }

    /// The local functions and tests of a body (`_FunctionBodyOutlinesVisitor`).
    fn of_body(&self, inside: Span) -> Vec<Value> {
        struct Item {
            span: Span,
            value: Value,
            leaf: bool,
        }
        let a = self.ast;
        let mut items: Vec<Item> = Vec::new();
        for s in a.stmts.iter() {
            let StmtKind::Function(f) = &s.kind else { continue };
            // Only those strictly inside: the node itself is not its own child.
            if s.span.start < inside.start || s.span.end > inside.end || s.span == inside {
                continue;
            }
            if let Some(v) = self.function(*f, s.span, &[], "FUNCTION", false) {
                items.push(Item { span: s.span, value: v, leaf: false });
            }
        }
        if self.tests {
            for e in a.exprs.iter() {
                let ExprKind::Call { target, arguments } = &e.kind else { continue };
                if e.span.start < inside.start || e.span.end > inside.end {
                    continue;
                }
                let ExprKind::Identifier(name) = &a.expr(*target).kind else { continue };
                let call = self.names.resolve(name.sym);
                if call != "group" && call != "test" {
                    continue;
                }
                let description = match arguments.args.first() {
                    None => "unnamed".to_string(),
                    Some(first) => {
                        let expr = a.expr(first.value);
                        let value = match &expr.kind {
                            ExprKind::String(lit) => dartforge_elements::load::string_lit_value(lit),
                            _ => None,
                        };
                        value.unwrap_or_else(|| self.snippet(expr.span).to_string())
                    }
                };
                let kind = if call == "group" { "UNIT_TEST_GROUP" } else { "UNIT_TEST_TEST" };
                let element = self.element(kind, &format!("{call}(\"{description}\")"), Some(name.span), None, None, None);
                let value = json!({"element": element, "range": self.range(e.span), "codeRange": self.range(e.span)});
                items.push(Item { span: e.span, value, leaf: call == "test" });
            }
        }
        // Pre-order; the local functions already carry their own children, so only
        // test groups nest here.
        items.sort_by(|x, y| x.span.start.cmp(&y.span.start).then(y.span.end.cmp(&x.span.end)));
        fn build(items: &mut std::iter::Peekable<std::vec::IntoIter<Item>>, limit: usize) -> Vec<Value> {
            let mut output = Vec::new();
            while let Some(item) = items.next_if(|i| i.span.start < limit) {
                let mut value = item.value;
                let is_group = value.pointer("/element/kind").and_then(Value::as_str) == Some("UNIT_TEST_GROUP");
                if is_group {
                    let children = build(items, item.span.end);
                    if !children.is_empty() {
                        value["children"] = json!(children);
                    }
                } else {
                    // Local function (children already computed) or test (leaf): what is
                    // inside is not a sibling.
                    let _ = item.leaf;
                    while items.next_if(|i| i.span.start < item.span.end).is_some() {}
                }
                output.push(value);
            }
            output
        }
        build(&mut items.into_iter().peekable(), inside.end)
    }

    /// `_newFunctionOutline` and `_newMethodOutline`.
    fn function(&self, f: FunctionId, span: Span, metadata: &[Annotation], common_kind: &str, of_method: bool) -> Option<Value> {
        let func = self.ast.function(f);
        let name = func.name?;
        let kind = match func.kind {
            FunctionKind::Getter => "GETTER",
            FunctionKind::Setter => "SETTER",
            _ => common_kind,
        };
        let parameters = self.parameter_list(name.span.end).map(|s| self.snippet(s));
        // A function without a list (getter) has `''`; a method, null.
        let parameters = if of_method { parameters } else { Some(parameters.unwrap_or("")) };
        let of_type = self.type_parameters(name.span.end).map(|s| self.snippet(s));
        let element = self.element(kind, self.names.resolve(name.sym), Some(name.span), parameters, of_type, Some(self.ty(func.return_type)));
        Some(self.no(element, span, metadata, self.of_body(span)))
    }

    /// `_newVariableOutline` of each variable of a list.
    fn variables(&self, list: &VariableList, declaration: Span, metadata: &[Annotation], kind: &str, output: &mut Vec<Value>) {
        let total = list.variables.len();
        for (i, v) in list.variables.iter().enumerate() {
            let variable_end = v.initializer.map_or(v.name.span.end, |e| self.ast.expr(e).span.end);
            // The first one starts at the declaration (with documentation and
            // annotations); the last one ends with it.
            let start = if i == 0 { self.with_documentation(declaration.start) } else { v.name.span.start };
            let end = if i + 1 == total { declaration.end } else { variable_end };
            let _ = metadata;
            let element = self.element(kind, self.names.resolve(v.name.sym), Some(v.name.span), None, None, Some(self.ty(list.ty)));
            output.push(json!({
                "element": element,
                "range": self.range(Span { start: start, end: end }),
                "codeRange": self.range(Span { start: v.name.span.start, end: variable_end }),
            }));
        }
    }

    /// `_outlinesForMembers`.
    fn members(&self, ids: &[MemberId]) -> Vec<Value> {
        let mut output = Vec::new();
        for id in ids {
            let m = self.ast.member(*id);
            match &m.kind {
                MemberKind::Constructor(k) => {
                    let class = self.names.resolve(k.class_name.sym);
                    let (name, local) = match k.name {
                        Some(n) => (format!("{class}.{}", self.names.resolve(n.sym)), n.span),
                        None => (class.to_string(), k.class_name.span),
                    };
                    let parameters = self.parameter_list(local.end).map(|s| self.snippet(s)).unwrap_or("");
                    let element = self.element("CONSTRUCTOR", &name, Some(local), Some(parameters), None, None);
                    output.push(self.no(element, m.span, &m.metadata, self.of_body(m.span)));
                }
                MemberKind::Field(l) => self.variables(l, m.span, &m.metadata, "FIELD", &mut output),
                MemberKind::Method(f) => output.extend(self.function(*f, m.span, &m.metadata, "METHOD", true)),
            }
        }
        output
    }
}

/// The `Outline` of the text unit `text` (the `<unit>` node with the children).
pub(crate) fn of_document(text: &str) -> Value {
    let mut names = Interner::new();
    let parsed = dartforge_frontend::parser::parse(text, &mut names);
    let a = &parsed.ast;
    // The same rule as `crate::symbols` for test nodes.
    let imports_tests = parsed.unit.directives.iter().any(|d| match &d.kind {
        DirectiveKind::Import { uri, .. } => dartforge_elements::load::string_lit_value(uri).is_some_and(|u| u.ends_with("test.dart")),
        _ => false,
    });
    let declares_homonym = parsed.unit.declarations.iter().any(|d| match &a.decl(*d).kind {
        DeclKind::Function(f) => a.function(*f).name.is_some_and(|n| matches!(names.resolve(n.sym), "group" | "test")),
        _ => false,
    });
    let ctx = Ctx { text, lines: LineTable::build(text), names: &names, ast: a, tests: imports_tests && !declares_homonym };
    let name_of = |n: Name| names.resolve(n.sym);
    let mut children: Vec<Value> = Vec::new();
    for id in &parsed.unit.declarations {
        let decl = a.decl(*id);
        let of_type = |n: Name| ctx.type_parameters(n.span.end).map(|s| ctx.snippet(s));
        match &decl.kind {
            DeclKind::Class(x) => {
                let kind = if x.mixin_application { "CLASS_TYPE_ALIAS" } else { "CLASS" };
                let element = ctx.element(kind, name_of(x.name), Some(x.name.span), None, of_type(x.name), None);
                let internal = if x.mixin_application { Vec::new() } else { ctx.members(&x.members) };
                children.push(ctx.no(element, decl.span, &decl.metadata, internal));
            }
            DeclKind::Mixin(x) => {
                let element = ctx.element("MIXIN", name_of(x.name), Some(x.name.span), None, of_type(x.name), None);
                children.push(ctx.no(element, decl.span, &decl.metadata, ctx.members(&x.members)));
            }
            DeclKind::Enum(x) => {
                // All constants before the members.
                let mut internal: Vec<Value> = x
                    .constants
                    .iter()
                    .map(|k| ctx.no(ctx.element("ENUM_CONSTANT", name_of(k.name), Some(k.name.span), None, None, None), k.span, &k.metadata, Vec::new()))
                    .collect();
                internal.extend(ctx.members(&x.members));
                let element = ctx.element("ENUM", name_of(x.name), Some(x.name.span), None, None, None);
                children.push(ctx.no(element, decl.span, &decl.metadata, internal));
            }
            DeclKind::Extension(x) => {
                // Without a name, the position is that of the extended type.
                let (name, local) = match x.name {
                    Some(n) => (name_of(n), n.span),
                    None => ("", a.ty(x.on).span),
                };
                let element = ctx.element("EXTENSION", name, Some(local), None, x.name.and_then(of_type), None);
                children.push(ctx.no(element, decl.span, &decl.metadata, ctx.members(&x.members)));
            }
            DeclKind::ExtensionType(x) => {
                let element = ctx.element("EXTENSION_TYPE", name_of(x.name), Some(x.name.span), None, of_type(x.name), None);
                children.push(ctx.no(element, decl.span, &decl.metadata, ctx.members(&x.members)));
            }
            DeclKind::Variables(l) => ctx.variables(l, decl.span, &decl.metadata, "TOP_LEVEL_VARIABLE", &mut children),
            DeclKind::Function(f) => children.extend(ctx.function(*f, decl.span, &decl.metadata, "FUNCTION", false)),
            DeclKind::Typedef(x) => {
                let element = match &x.kind {
                    // `typedef R F(params);`
                    TypedefKind::Legacy { return_type, .. } => {
                        let parameters = ctx.parameter_list(x.name.span.end).map(|s| ctx.snippet(s)).unwrap_or("");
                        ctx.element("FUNCTION_TYPE_ALIAS", name_of(x.name), Some(x.name.span), Some(parameters), of_type(x.name), Some(ctx.ty(*return_type)))
                    }
                    TypedefKind::Alias(t) => match &a.ty(*t).kind {
                        // `typedef F = R Function(params);`
                        TypeKind::Function { return_type, .. } => {
                            let written = ctx.snippet(a.ty(*t).span);
                            // The list is the pair of parentheses that closes at the type's last `)`.
                            let parameters = {
                                let b = written.as_bytes();
                                let mut found: Option<(usize, usize)> = None;
                                if let Some(closure) = written.rfind(')') {
                                    let mut level = 0usize;
                                    for i in (0..=closure).rev() {
                                        if b[i] == b')' {
                                            level += 1;
                                        } else if b[i] == b'(' {
                                            level -= 1;
                                            if level == 0 {
                                                found = Some((i, closure + 1));
                                                break;
                                            }
                                        }
                                    }
                                }
                                found.map_or("", |(x, y)| &written[x..y])
                            };
                            ctx.element("FUNCTION_TYPE_ALIAS", name_of(x.name), Some(x.name.span), Some(parameters), of_type(x.name), Some(ctx.ty(*return_type)))
                        }
                        _ => ctx.element("TYPE_ALIAS", name_of(x.name), Some(x.name.span), None, of_type(x.name), None),
                    },
                };
                children.push(ctx.no(element, decl.span, &decl.metadata, Vec::new()));
            }
        }
    }
    // The unit node: from the first to the last token.
    let start = parsed
        .unit
        .directives
        .first()
        .map(|d| d.span.start)
        .into_iter()
        .chain(parsed.unit.declarations.first().map(|d| a.decl(*d).span.start))
        .min()
        .unwrap_or(0);
    let end = parsed
        .unit
        .directives
        .last()
        .map(|d| d.span.end)
        .into_iter()
        .chain(parsed.unit.declarations.last().map(|d| a.decl(*d).span.end))
        .max()
        .unwrap_or(0);
    let unit = Span { start: start, end: end.max(start) };
    let mut root = json!({
        "element": ctx.element("COMPILATION_UNIT", "<unit>", Some(unit), None, None, None),
        "range": ctx.range(unit),
        "codeRange": ctx.range(unit),
    });
    if !children.is_empty() {
        root["children"] = json!(children);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_with_documentation_and_annotation() {
        let source = "/// Doc.\n@a\nclass A<T> {\n  int x = 1, y;\n  A.n(int p);\n  void m() {}\n}\nvoid f(int a) {}\n";
        let root = of_document(source);
        assert_eq!(root["element"]["kind"], "COMPILATION_UNIT");
        let class = &root["children"][0];
        assert_eq!(class["element"]["name"], "A");
        assert_eq!(class["element"]["typeParameters"], "<T>");
        // `range` starts at the comment; `codeRange`, at the `class` keyword.
        assert_eq!(class["range"]["start"]["line"], 0);
        assert_eq!(class["codeRange"]["start"]["line"], 2);
        let members = class["children"].as_array().unwrap();
        assert_eq!(members.len(), 4);
        assert_eq!(members[0]["element"]["kind"], "FIELD");
        assert_eq!(members[0]["element"]["returnType"], "int");
        assert_eq!(members[2]["element"]["name"], "A.n");
        assert_eq!(members[2]["element"]["parameters"], "(int p)");
        assert_eq!(members[3]["element"]["kind"], "METHOD");
        let function = &root["children"][1];
        assert_eq!(function["element"]["parameters"], "(int a)");
        assert_eq!(function["element"]["returnType"], "void");
    }
}
