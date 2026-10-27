//! Library IIFEs without declarations of their own, unwrapped, and the
//! top-level names declared where they are first assigned
//! (`docs/JS-PRODUCAO-TAMANHO.md` §15).
//!
//! Once the module preludes are hoisted (`own_sdk::hoist_preludes`) and the
//! library members are variables (`flatten.rs`), most library IIFEs only
//! assign names declared at the top of the file: `(function () { core$$print
//! = function …; })();`. Such an IIFE changes nothing in scope and its body
//! can take its place. The names declared without a value at the top (`var
//! a, b, c, …`: 3,500 random names in a large application, which gzip cannot
//! compress) are then declared by their first assignment in the file's own
//! scope (`var core$$print = function …;`), which is the same `var`, hoisted
//! the same way.
//!
//! An IIFE stays when its body declares a name that also occurs outside it
//! (the declaration would leak into the shared scope), has directives, or
//! returns at its top level. `DARTFORGE_JSPROD_UNWRAP=0` turns it off.

use oxc_ast::ast::{
    AssignmentOperator, AssignmentTarget, Expression, Statement, VariableDeclarationKind,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;
use std::collections::HashMap;

/// Whether a statement list returns at its own level (not inside a nested
/// function).
struct Returns(bool);

impl<'a> Visit<'a> for Returns {
    fn visit_return_statement(&mut self, _it: &oxc_ast::ast::ReturnStatement<'a>) {
        self.0 = true;
    }
    fn visit_function(
        &mut self,
        _it: &oxc_ast::ast::Function<'a>,
        _flags: oxc_syntax::scope::ScopeFlags,
    ) {
    }
    fn visit_arrow_function_expression(&mut self, _it: &oxc_ast::ast::ArrowFunctionExpression<'a>) {
    }
    fn visit_class(&mut self, _it: &oxc_ast::ast::Class<'a>) {}
}

/// The names a statement list declares at its own level.
fn declared_names<'a>(statements: &[Statement<'a>]) -> Option<Vec<&'a str>> {
    let mut out = Vec::new();
    for s in statements {
        match s {
            Statement::VariableDeclaration(d) => {
                for x in &d.declarations {
                    // A pattern (`let {a} = …`): not worth reading.
                    out.push(x.id.get_binding_identifier()?.name.as_str());
                }
            }
            Statement::FunctionDeclaration(f) => out.push(f.id.as_ref()?.name.as_str()),
            Statement::ClassDeclaration(c) => out.push(c.id.as_ref()?.name.as_str()),
            _ => {}
        }
    }
    Some(out)
}

/// How many times each identifier-shaped word occurs in `source` (strings
/// and property names included: more occurrences only make the check
/// below more cautious).
fn word_counts(source: &str) -> HashMap<&str, usize> {
    let mut count: HashMap<&str, usize> = HashMap::new();
    let b = source.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let mut i = 0;
    while i < b.len() {
        if id(b[i]) {
            let s = i;
            while i < b.len() && id(b[i]) {
                i += 1;
            }
            *count.entry(&source[s..i]).or_default() += 1;
        } else {
            i += 1;
        }
    }
    count
}

/// The body of `(function () { … })();` when it may replace the call.
fn unwrappable<'b, 'a>(s: &'b Statement<'a>) -> Option<&'b oxc_ast::ast::FunctionBody<'a>> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    let Expression::CallExpression(c) = &e.expression else {
        return None;
    };
    if !c.arguments.is_empty() {
        return None;
    }
    let mut callee = &c.callee;
    while let Expression::ParenthesizedExpression(p) = callee {
        callee = &p.expression;
    }
    let Expression::FunctionExpression(f) = callee else {
        return None;
    };
    if f.r#async || f.generator || !f.params.items.is_empty() || f.params.rest.is_some() {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let mut r = Returns(false);
    for st in &body.statements {
        walk::walk_statement(&mut r, st);
    }
    (!r.0).then_some(body)
}

/// `name = …;` at statement level: the assigned name.
fn assigned_name<'a>(s: &Statement<'a>) -> Option<&'a str> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &e.expression else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    match &a.left {
        AssignmentTarget::AssignmentTargetIdentifier(i) => Some(i.name.as_str()),
        _ => None,
    }
}

/// Applies the transformation to `source` (the file already wrapped in
/// `(function () {…})();`).
///
/// # Errors
///
/// The text is not valid JS for the parser.
pub fn apply(source: &str) -> Result<String, String> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let Some(outer) = parsed.program.body.first().and_then(unwrappable_any) else {
        return Ok(source.to_string());
    };
    let mut edits: Vec<(u32, u32, String)> = Vec::new();
    // The statements now at the file scope, in order: the outer body's own
    // and those of each unwrapped IIFE.
    let mut first_assignment: HashMap<&str, u32> = HashMap::new();
    fn note<'a>(s: &Statement<'a>, first: &mut HashMap<&'a str, u32>) {
        if let Some(n) = assigned_name(s) {
            first.entry(n).or_insert(s.span().start);
        }
    }
    // An IIFE that declares names may still be unwrapped when every
    // occurrence of each of those names is inside it, and no other unwrapped
    // IIFE declares them: they become file-scope names nobody else sees.
    let counts = word_counts(source);
    let mut taken: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if let Some(names) = declared_names(&outer.statements) {
        taken.extend(names);
    }
    for s in &outer.statements {
        let body = unwrappable(s).filter(|body| {
            let Some(names) = declared_names(&body.statements) else {
                return false;
            };
            if names.is_empty() {
                return true;
            }
            let span = s.span();
            let inside = word_counts(&source[span.start as usize..span.end as usize]);
            let private = names.iter().all(|n| {
                !taken.contains(n) && counts.get(n) == inside.get(n) && inside.contains_key(n)
            });
            if private {
                taken.extend(names);
            }
            private
        });
        match body {
            Some(body) => {
                let span = s.span();
                let (Some(first), Some(last)) = (body.statements.first(), body.statements.last())
                else {
                    // An empty IIFE: nothing.
                    edits.push((span.start, span.end, String::new()));
                    continue;
                };
                let inner_start = first.span().start;
                let inner_end = last.span().end;
                // A leading `(`/`[`/template would continue the previous
                // statement if it had no `;`.
                let guard = matches!(
                    source.as_bytes().get(inner_start as usize),
                    Some(b'(' | b'[' | b'`' | b'+' | b'-' | b'/')
                );
                edits.push((
                    span.start,
                    inner_start,
                    if guard {
                        ";".to_string()
                    } else {
                        String::new()
                    },
                ));
                edits.push((inner_end, span.end, "\n".to_string()));
                for st in &body.statements {
                    note(st, &mut first_assignment);
                }
            }
            None => note(s, &mut first_assignment),
        }
    }
    // `var a, b, c;` at the file scope: each name with a first assignment
    // there is declared by it.
    for s in &outer.statements {
        let Statement::VariableDeclaration(d) = s else {
            continue;
        };
        if d.kind != VariableDeclarationKind::Var || d.declarations.iter().any(|x| x.init.is_some())
        {
            continue;
        }
        let mut kept: Vec<&str> = Vec::new();
        let mut moved = false;
        for x in &d.declarations {
            let Some(id) = x.id.get_binding_identifier() else {
                kept.push(&source[x.span.start as usize..x.span.end as usize]);
                continue;
            };
            match first_assignment.remove(id.name.as_str()) {
                Some(at) => {
                    edits.push((at, at, "var ".to_string()));
                    moved = true;
                }
                None => kept.push(id.name.as_str()),
            }
        }
        if moved {
            let span = d.span;
            let text = if kept.is_empty() {
                String::new()
            } else {
                format!("var {};", kept.join(", "))
            };
            edits.push((span.start, span.end, text));
        }
    }
    if edits.is_empty() {
        return Ok(source.to_string());
    }
    edits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut out = String::with_capacity(source.len());
    let mut pos = 0usize;
    for (start, end, text) in edits {
        let (start, end) = (start as usize, end as usize);
        if start < pos {
            continue;
        }
        out.push_str(&source[pos..start]);
        out.push_str(&text);
        pos = end;
    }
    out.push_str(&source[pos..]);
    Ok(out)
}

/// The outer wrapper's body (the whole file is one IIFE); unlike
/// [`unwrappable`], it may declare anything.
pub(crate) fn unwrappable_any<'b, 'a>(
    s: &'b Statement<'a>,
) -> Option<&'b oxc_ast::ast::FunctionBody<'a>> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    let Expression::CallExpression(c) = &e.expression else {
        return None;
    };
    let mut callee = &c.callee;
    while let Expression::ParenthesizedExpression(p) = callee {
        callee = &p.expression;
    }
    let Expression::FunctionExpression(f) = callee else {
        return None;
    };
    f.body.as_deref()
}

/// The `dart` runtime function a callee names (`dart.x` or, flattened,
/// `dart$$x`).
pub(crate) fn dart_member<'a>(callee: &'a Expression<'_>) -> Option<&'a str> {
    match callee {
        Expression::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == "dart") => {
            Some(m.property.name.as_str())
        }
        Expression::Identifier(i) => crate::flatten::member(i.name.as_str(), "dart"),
        _ => None,
    }
}

/// The class a metadata statement registers, as source text: the class of
/// `dart.lateFields(C.prototype, …)`, of `dart.setXSignature(C, …)` and of
/// the compact `t$G(S, G, C, …)` (`shapes.rs`).
fn metadata_class<'s>(source: &'s str, s: &Statement<'_>) -> Option<&'s str> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    let Expression::CallExpression(c) = &e.expression else {
        return None;
    };
    let arg = match (&c.callee, dart_member(&c.callee)) {
        (_, Some("lateFields")) => c.arguments.first()?,
        (_, Some(m)) if m.starts_with("set") && m.ends_with("Signature") => c.arguments.first()?,
        (Expression::Identifier(i), _) if i.name == "t$G" => c.arguments.get(2)?,
        _ => return None,
    };
    let span = arg.span();
    let text = &source[span.start as usize..span.end as usize];
    Some(text.strip_suffix(".prototype").unwrap_or(text))
}

/// Source texts of the arguments of the runtime calls that copy a class's
/// members or signatures when the library loads (`applyMixin`,
/// `registerExtension`, `defineExtensionMethods`/`Accessors`) and of the
/// classes with a `[dart.mixinOn]`: their metadata must stay where it is.
struct LoadTimeCopies<'s> {
    source: &'s str,
    classes: std::collections::HashSet<&'s str>,
}

impl<'a> Visit<'a> for LoadTimeCopies<'_> {
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if matches!(
            dart_member(&it.callee),
            Some(
                "applyMixin"
                    | "registerExtension"
                    | "defineExtensionMethods"
                    | "defineExtensionAccessors"
            )
        ) {
            for a in &it.arguments {
                let span = a.span();
                self.classes
                    .insert(&self.source[span.start as usize..span.end as usize]);
            }
        }
        walk::walk_call_expression(self, it);
    }
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(m) = &it.left
            && let Expression::Identifier(k) = &m.expression
            && (k.name == "dart$$mixinOn")
        {
            let span = m.object.span();
            self.classes
                .insert(&self.source[span.start as usize..span.end as usize]);
        }
        if let AssignmentTarget::ComputedMemberExpression(m) = &it.left
            && let Expression::StaticMemberExpression(k) = &m.expression
            && k.property.name == "mixinOn"
        {
            let span = m.object.span();
            self.classes
                .insert(&self.source[span.start as usize..span.end as usize]);
        }
        walk::walk_assignment_expression(self, it);
    }
}

/// Moves the file-scope metadata statements of the classes (`lateFields`
/// and the signatures) to just before the last statement (the call of
/// `main`), in their order: alike statements next to each other compress
/// far better than scattered between class bodies (limitless_ui: the late
/// field lists alone, -6.8 KB gzip). Nothing reads them while the libraries
/// load (the signatures are lazy, the late accessors serve instances,
/// created from `main` on), except the load-time copies of
/// [`LoadTimeCopies`], whose classes keep their statements in place.
/// `DARTFORGE_JSPROD_GROUP_METADATA=0` turns it off.
///
/// # Errors
///
/// The text is not valid JS for the parser.
pub fn group_metadata(source: &str) -> Result<String, String> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let Some(outer) = parsed.program.body.first().and_then(unwrappable_any) else {
        return Ok(source.to_string());
    };
    let Some(last) = outer.statements.last() else {
        return Ok(source.to_string());
    };
    let mut copies = LoadTimeCopies {
        source,
        classes: std::collections::HashSet::new(),
    };
    copies.visit_program(&parsed.program);
    let mut moved: Vec<(u32, u32)> = Vec::new();
    for s in &outer.statements[..outer.statements.len() - 1] {
        if metadata_class(source, s).is_some_and(|c| !copies.classes.contains(c)) {
            let span = s.span();
            moved.push((span.start, span.end));
        }
    }
    // The extension aliases after the moved metadata, in one call
    // (`extension_batch`).
    let (extensions, batch) =
        if std::env::var("DARTFORGE_JSPROD_BATCH_EXTENSIONS").map_or(true, |v| v != "0") {
            extension_batch(source, &parsed.program, &outer.statements)
        } else {
            (Vec::new(), String::new())
        };
    if moved.is_empty() && extensions.is_empty() {
        return Ok(source.to_string());
    }
    let mut removed: Vec<(u32, u32, bool)> = moved.iter().map(|&(a, b)| (a, b, true)).collect();
    removed.extend(extensions.iter().map(|&(a, b)| (a, b, false)));
    removed.sort_unstable();
    let at = last.span().start as usize;
    let mut out = String::with_capacity(source.len() + moved.len());
    let mut pos = 0usize;
    let mut group = String::new();
    for &(a, b, keep) in &removed {
        let (a, b) = (a as usize, b as usize);
        out.push_str(&source[pos..a]);
        if keep {
            group.push_str(&source[a..b]);
            group.push('\n');
        }
        pos = b;
    }
    out.push_str(&source[pos..at]);
    out.push_str(&group);
    out.push_str(&batch);
    out.push_str(&source[at..]);
    Ok(out)
}

/// Name of the helper of [`extension_batch`].
const EXTENSIONS_HELPER: &str = "t$X";

/// The top-level `dart.defineExtensionMethods(C, ["m", …])` and
/// `…Accessors(C, ["a", …])` statements that can wait for the end of the
/// load, and the call that replaces them: a flat list `C, ["m", …], ["a",
/// …]` (`0` for a missing list) handed to a helper that makes the same calls
/// in the same order. The aliases are first read when a method is called
/// through its `dartx` symbol, which loading a library does not do, except
/// in the load-time copies: `applyMixin` and `registerExtension` copy the
/// symbol-keyed members of their classes (a native type's whole Dart chain),
/// so those classes, their superclasses and the classes with a
/// `[dart.mixinOn]` keep their statements. The call goes after the moved
/// metadata, so every late accessor already exists. Without a readable
/// superclass for a copied class nothing moves.
/// `DARTFORGE_JSPROD_BATCH_EXTENSIONS=0` turns it off.
fn extension_batch(
    source: &str,
    program: &oxc_ast::ast::Program<'_>,
    statements: &[Statement<'_>],
) -> (Vec<(u32, u32)>, String) {
    let none = (Vec::new(), String::new());
    if source.contains(EXTENSIONS_HELPER) {
        return none;
    }
    let text = |e: &Expression<'_>| -> Option<&str> {
        match e {
            Expression::Identifier(_) | Expression::StaticMemberExpression(_) => {
                let span = e.span();
                Some(&source[span.start as usize..span.end as usize])
            }
            _ => None,
        }
    };
    // Superclass of each class defined at the top level (`None`: no
    // `extends`).
    let mut supers: HashMap<&str, Option<&str>> = HashMap::new();
    let class_of = |e: &Expression<'_>| -> Option<Option<&str>> {
        let class = match e {
            Expression::ClassExpression(c) => c,
            Expression::CallExpression(c) => match c.arguments.first()?.as_expression()? {
                Expression::ClassExpression(c) => c,
                _ => return None,
            },
            _ => return None,
        };
        Some(match &class.heritage {
            None => None,
            Some(h) => Some(text(&h.expression)?),
        })
    };
    for s in statements {
        match s {
            Statement::VariableDeclaration(v) => {
                for d in &v.declarations {
                    if let (Some(id), Some(init)) = (d.id.get_binding_identifier(), &d.init)
                        && let Some(sup) = class_of(init)
                    {
                        let span = id.span;
                        supers.insert(&source[span.start as usize..span.end as usize], sup);
                    }
                }
            }
            Statement::ExpressionStatement(e) => {
                if let Expression::AssignmentExpression(a) = &e.expression
                    && let Some(sup) = class_of(&a.right)
                {
                    let span = a.left.span();
                    supers.insert(&source[span.start as usize..span.end as usize], sup);
                }
            }
            _ => {}
        }
    }
    let mut copies = CopiedClasses {
        source,
        classes: Vec::new(),
    };
    copies.visit_program(program);
    let mut pinned: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut queue = copies.classes;
    while let Some(c) = queue.pop() {
        if !pinned.insert(c) {
            continue;
        }
        match supers.get(c) {
            Some(Some(s)) => queue.push(s),
            Some(None) => {}
            None => return none,
        }
    }
    // (class, methods, accessors) in order of first statement.
    let mut found: Vec<(u32, u32)> = Vec::new();
    let mut entries: Vec<(&str, Option<&str>, Option<&str>)> = Vec::new();
    let mut callees: (Option<&str>, Option<&str>) = (None, None);
    for s in statements {
        let Statement::ExpressionStatement(e) = s else {
            continue;
        };
        let Expression::CallExpression(c) = &e.expression else {
            continue;
        };
        let accessors = match dart_member(&c.callee) {
            Some("defineExtensionMethods") => false,
            Some("defineExtensionAccessors") => true,
            _ => continue,
        };
        let [class, list] = &c.arguments[..] else {
            continue;
        };
        let (Some(class), Some(Expression::ArrayExpression(items))) =
            (class.as_expression().and_then(text), list.as_expression())
        else {
            continue;
        };
        if pinned.contains(class)
            || !items
                .elements
                .iter()
                .all(|x| matches!(x, oxc_ast::ast::ArrayExpressionElement::StringLiteral(_)))
        {
            continue;
        }
        let callee = text(&c.callee);
        let slot = if accessors {
            &mut callees.1
        } else {
            &mut callees.0
        };
        if slot.is_some_and(|x| Some(x) != callee) {
            continue;
        }
        *slot = callee;
        let span = items.span;
        let list = &source[span.start as usize..span.end as usize];
        match entries.last_mut() {
            Some((c, m, a))
                if *c == class && (if accessors { a.is_none() } else { m.is_none() }) =>
            {
                if accessors {
                    *a = Some(list);
                } else {
                    *m = Some(list);
                }
            }
            _ => entries.push(if accessors {
                (class, None, Some(list))
            } else {
                (class, Some(list), None)
            }),
        }
        let span = s.span();
        found.push((span.start, span.end));
    }
    // The helper costs about as much as twenty statements save (measured on
    // the corpus).
    if found.len() < 28 {
        return none;
    }
    let call = |f: Option<&str>, k: usize| {
        f.map(|f| format!("a[i + {k}] && {f}(a[i], a[i + {k}]); "))
            .unwrap_or_default()
    };
    let mut flat: Vec<String> = Vec::new();
    for (c, m, a) in &entries {
        flat.push(c.to_string());
        flat.push(m.unwrap_or("0").to_string());
        flat.push(a.unwrap_or("0").to_string());
    }
    let batch = format!(
        "var {EXTENSIONS_HELPER} = function(a) {{ for (let i = 0; i < a.length; i += 3) {{ {}{}}} }};\n{EXTENSIONS_HELPER}([{}]);\n",
        call(callees.0, 1),
        call(callees.1, 2),
        flat.join(", ")
    );
    (found, batch)
}

/// Source texts of the classes a runtime call copies at load: the
/// arguments of `applyMixin` and `registerExtension` and the classes with a
/// `[dart.mixinOn]` (as [`LoadTimeCopies`], without the extension calls).
struct CopiedClasses<'s> {
    source: &'s str,
    classes: Vec<&'s str>,
}

impl<'a> Visit<'a> for CopiedClasses<'_> {
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if matches!(
            dart_member(&it.callee),
            Some("applyMixin" | "registerExtension")
        ) {
            for a in &it.arguments {
                if let Some(e) = a.as_expression()
                    && matches!(
                        e,
                        Expression::Identifier(_) | Expression::StaticMemberExpression(_)
                    )
                {
                    let span = e.span();
                    self.classes
                        .push(&self.source[span.start as usize..span.end as usize]);
                }
            }
        }
        walk::walk_call_expression(self, it);
    }
    fn visit_assignment_expression(&mut self, it: &oxc_ast::ast::AssignmentExpression<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(m) = &it.left {
            let mixin_on = match &m.expression {
                Expression::Identifier(k) => k.name == "dart$$mixinOn",
                Expression::StaticMemberExpression(k) => k.property.name == "mixinOn",
                _ => false,
            };
            if mixin_on {
                let span = m.object.span();
                self.classes
                    .push(&self.source[span.start as usize..span.end as usize]);
            }
        }
        walk::walk_assignment_expression(self, it);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(js: &str) -> String {
        apply(&format!("(function () {{\n{js}\n}})();\n")).unwrap()
    }

    #[test]
    fn assignment_only_iife_is_unwrapped_and_names_declared_in_place() {
        let s =
            run("var a, b, c;\n(function () {\na = function () { return 1; };\nb = 2;\n})();\nc;");
        assert!(!s.contains("(function () {\na"), "{s}");
        assert!(s.contains("var a = function"), "{s}");
        assert!(s.contains("var b = 2;"), "{s}");
        assert!(s.contains("var c;"), "{s}");
    }

    #[test]
    fn iife_with_shared_declarations_or_return_stays() {
        let s = run("var a;\nvar x = 0;\n(function () {\nlet x = 1;\na = x;\n})();");
        assert!(s.contains("let x = 1;"), "{s}");
        assert!(s.contains("(function () {"), "{s}");
        assert!(s.contains("var a;"), "{s}");
        let s = run("var a;\n(function () {\nlet y = 1;\na = y;\n})();");
        assert!(!s.contains("(function () {\nlet"), "{s}");
        assert!(s.contains("var a = y;"), "{s}");
        let s = run("var a;\n(function () {\na = 1;\nreturn;\n})();\na = 2;");
        assert!(s.contains("return;"), "{s}");
        assert!(s.contains("var a = 2;"), "{s}");
    }

    #[test]
    fn leading_parenthesis_is_guarded() {
        let s = run("var a;\nx();\n(function () {\n(a = 1);\n})();");
        assert!(s.contains(";(a = 1)"), "{s}");
    }

    #[test]
    fn metadata_moves_before_main_except_load_time_copies() {
        let s = group_metadata(
            "(function () {\nvar A = class {}, M = class {}, B = class {};\ndart$$lateFields(A.prototype, \"a\", \"\");\nt$G(S, G, A, () => ({m: T}));\ndart$$lateFields(M.prototype, \"m\", \"\");\ndart$$applyMixin(B, M);\nx();\nL$$main();\n})();\n",
        )
        .unwrap();
        let main = s.find("L$$main()").unwrap();
        let x = s.find("x();").unwrap();
        let a = s.find("dart$$lateFields(A.prototype").unwrap();
        let g = s.find("t$G(S, G, A").unwrap();
        let m = s.find("dart$$lateFields(M.prototype").unwrap();
        assert!(x < a && a < g && g < main, "{s}");
        assert!(m < x, "{s}");
    }

    #[test]
    fn extension_aliases_batch_after_the_metadata_except_load_time_copies() {
        let mut body = String::from(
            "var N = class extends P {}, P = class extends core$$Object {}, core$$Object = class {};\ndart$$defineExtensionMethods(P, [\"p\"]);\ndart$$defineExtensionMethods(N, [\"n\"]);\ndart$$registerExtension(\"Array\", N);\n",
        );
        for i in 0..30 {
            body.push_str(&format!(
                "var C{i} = class extends core$$Object {{}};\ndart$$lateFields(C{i}.prototype, \"k\", \"\");\ndart$$defineExtensionMethods(C{i}, [\"m\", \"n\"]);\ndart$$defineExtensionAccessors(C{i}, [\"a\"]);\n"
            ));
        }
        let s = group_metadata(&format!("(function () {{\n{body}L$$main();\n}})();\n")).unwrap();
        // The native peer and its superclass keep theirs, in place.
        let reg = s.find("dart$$registerExtension").unwrap();
        assert!(
            s.find("dart$$defineExtensionMethods(P, [\"p\"])").unwrap() < reg,
            "{s}"
        );
        assert!(
            s.find("dart$$defineExtensionMethods(N, [\"n\"])").unwrap() < reg,
            "{s}"
        );
        // The others: one call after the moved late fields, before `main`.
        assert!(!s.contains("dart$$defineExtensionMethods(C0"), "{s}");
        let batch = s.find("t$X([C0, [\"m\", \"n\"], [\"a\"], C1").unwrap();
        assert!(s.rfind("dart$$lateFields(").unwrap() < batch, "{s}");
        assert!(batch < s.find("L$$main();").unwrap(), "{s}");
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, &s, oxc_span::SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{s}");
        // An unreadable superclass of a copied class keeps everything.
        let s = group_metadata(&format!(
            "(function () {{\n{}L$$main();\n}})();\n",
            body.replace("class extends P", "class extends f()")
        ))
        .unwrap();
        assert!(s.contains("dart$$defineExtensionMethods(C0"), "{s}");
    }

    #[test]
    fn the_result_parses() {
        let s = run(
            "var a, b;\n(function () {\na = 1;\n})();\n(function () {\nb = a + 1;\n})();\nconsole.log(b);",
        );
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, &s, oxc_span::SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{s}");
        assert!(s.contains("var b = a + 1;"), "{s}");
    }
}
