//! Compact forms of constructs the emitter repeats
//! (`docs/JS-PRODUCAO-TAMANHO.md` §9.8).
//!
//! A Dart constructor in the DDC contract is a function with the class's
//! prototype: `(C.name = function(…) {…}).prototype = C.prototype;`, one per
//! constructor (4 thousand in `new_sali`). Here it becomes
//! `t$P(C.name = function(…) {…}, C);`, with `t$P` declared once at the top
//! of the file: the same assignment, in the same order (the argument is
//! evaluated before the call), and the same prototype.
//!
//! The replacement is textual, through `oxc`'s spans, as in `flatten.rs`.
//! `DARTFORGE_JSPROD_SHAPES=0` turns it off.

use oxc_ast::ast::{AssignmentOperator, AssignmentTarget, Expression, ExpressionStatement};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;

/// The helper function, declared at the top of the file.
const HELPERS: &str = "var t$P = function(f, c) { f.prototype = c.prototype; };\nvar t$C = function(c, f) { c.new = f; f.prototype = c.prototype; return c; };\nvar t$O = function(o, k, d) { return o && k in o ? o[k] : d; };\nvar t$G = function(s, g, c, f) { s(c, () => Object.setPrototypeOf(f(), g(c.__proto__))); };\n";

struct Finder<'s> {
    source: &'s str,
    /// (start, end, new text) of each replaced statement.
    edits: Vec<(u32, u32, String)>,
    /// Start of the SDK end marker (`minify::SDK_END_MARKER`).
    sdk_end: u32,
    /// Values of the `t$S…` from *string* deduplication.
    strings: std::collections::HashMap<String, String>,
    /// The program's `t$R("recipe")` calls: (start, end, recipe).
    recipes: Vec<(u32, u32, String)>,
    /// The `X.$ti = X.$ti || t || G(X);` statements: (start, end, receiver,
    /// type argument, text of `G`).
    type_args: Vec<(u32, u32, String, String, String)>,
    /// Starts of the constructor statements merged into their class
    /// definition ([`class_with_constructor`]).
    merged: std::collections::HashSet<u32>,
    /// `DARTFORGE_JSPROD_CLASS_CONSTRUCTOR=0` turns the merge off.
    merge_constructors: bool,
}

impl<'a> Visit<'a> for Finder<'_> {
    fn visit_statements(&mut self, it: &oxc_allocator::Vec<'a, oxc_ast::ast::Statement<'a>>) {
        if self.merge_constructors {
            for pair in it.windows(2) {
                if let (
                    oxc_ast::ast::Statement::ExpressionStatement(class),
                    oxc_ast::ast::Statement::ExpressionStatement(ctor),
                ) = (&pair[0], &pair[1])
                    && !self.merged.contains(&class.span.start)
                    && let Some(t) = class_with_constructor(self.source, class, ctor)
                {
                    self.edits.extend(t);
                    self.merged.insert(ctor.span.start);
                }
            }
        }
        walk::walk_statements(self, it);
    }
    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        if self.merged.contains(&it.span.start) {
            // Merged into the class definition: only what is inside the
            // function is still rewritten.
        } else if let Some(t) = prototype_form(self.source, it) {
            self.edits.extend(t);
        } else if let Some(t) = signature_form(self.source, it) {
            self.edits.push(t);
        } else if let Some(t) = type_args_form(self.source, it) {
            self.type_args.push(t);
        }
        walk::walk_expression_statement(self, it);
    }
    fn visit_variable_declarator(&mut self, it: &oxc_ast::ast::VariableDeclarator<'a>) {
        if let Some(id) = it.id.get_binding_identifier()
            && id.name.starts_with("t$S")
            && let Some(Expression::StringLiteral(l)) = &it.init
        {
            self.strings
                .insert(id.name.to_string(), l.value.to_string());
        }
        walk::walk_variable_declarator(self, it);
    }
    fn visit_conditional_expression(&mut self, it: &oxc_ast::ast::ConditionalExpression<'a>) {
        if let Some(new) = named_prologue(self.source, it) {
            self.edits.push((it.span.start, it.span.end, new));
            return;
        }
        walk::walk_conditional_expression(self, it);
    }
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if it.span.start > self.sdk_end
            && matches!(&it.callee, Expression::Identifier(i) if i.name == "t$R")
            && it.arguments.len() == 1
        {
            let recipe = match it.arguments[0].as_expression() {
                Some(Expression::StringLiteral(l)) => Some(l.value.to_string()),
                Some(Expression::Identifier(i)) => self.strings.get(i.name.as_str()).cloned(),
                _ => None,
            };
            if let Some(r) = recipe {
                self.recipes.push((it.span.start, it.span.end, r));
                return;
            }
        }
        walk::walk_call_expression(self, it);
    }
}

/// `(C.n = function(…){…}).prototype = C.prototype;` → `t$P(C.n = function(…){…}, C);`,
/// in two replacements (the leading `(` and the trailing
/// `).prototype = C.prototype;`), so that the ones inside the function still
/// apply.
fn prototype_form(source: &str, it: &ExpressionStatement<'_>) -> Option<[(u32, u32, String); 2]> {
    let Expression::AssignmentExpression(a) = &it.expression else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(left) = &a.left else {
        return None;
    };
    let Expression::StaticMemberExpression(right) = &a.right else {
        return None;
    };
    if left.property.name != "prototype" || right.property.name != "prototype" {
        return None;
    }
    let Expression::ParenthesizedExpression(p) = &left.object else {
        return None;
    };
    let Expression::AssignmentExpression(inner) = &p.expression else {
        return None;
    };
    if inner.operator != AssignmentOperator::Assign
        || !matches!(inner.right, Expression::FunctionExpression(_))
    {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(target) = &inner.left else {
        return None;
    };
    let text = |s: oxc_span::Span| &source[s.start as usize..s.end as usize];
    let class = text(right.object.span());
    // The class must be the same expression, and a simple one (no effect
    // when read).
    if text(target.object.span()) != class
        || !class
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.'))
    {
        return None;
    }
    if !text(p.span).starts_with('(') || !text(p.span).ends_with(')') {
        return None;
    }
    Some([
        (p.span.start, p.span.start + 1, "t$P(".to_string()),
        (p.span.end - 1, it.span.end, format!(", {class});")),
    ])
}

/// A class definition followed by its default constructor,
/// `C = class … {…};` and `(C.new = function(…) {…}).prototype = C.prototype;`
/// (most Dart classes): `C = t$C(class … {…}, function(…) {…});`, whose
/// function makes the same assignments and returns the class. The class name
/// is written once instead of three times. The constructor function is
/// created before `C` is assigned instead of after, which nothing observes
/// (it is not called until the class exists). Three replacements around the
/// class and the function, so that the ones inside them still apply.
fn class_with_constructor(
    source: &str,
    class: &ExpressionStatement<'_>,
    ctor: &ExpressionStatement<'_>,
) -> Option<[(u32, u32, String); 3]> {
    let Expression::AssignmentExpression(def) = &class.expression else {
        return None;
    };
    if def.operator != AssignmentOperator::Assign
        || !matches!(def.right, Expression::ClassExpression(_))
    {
        return None;
    }
    let text = |s: oxc_span::Span| &source[s.start as usize..s.end as usize];
    let name = match &def.left {
        AssignmentTarget::AssignmentTargetIdentifier(i) => i.name.as_str(),
        AssignmentTarget::StaticMemberExpression(m) => text(m.span),
        _ => return None,
    };
    let Expression::AssignmentExpression(a) = &ctor.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(left) = &a.left else {
        return None;
    };
    let Expression::StaticMemberExpression(right) = &a.right else {
        return None;
    };
    let Expression::ParenthesizedExpression(p) = &left.object else {
        return None;
    };
    let Expression::AssignmentExpression(inner) = &p.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &inner.left else {
        return None;
    };
    let Expression::FunctionExpression(f) = &inner.right else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign
        || inner.operator != AssignmentOperator::Assign
        || left.property.name != "prototype"
        || right.property.name != "prototype"
        || target.property.name != "new"
        || text(target.object.span()) != name
        || text(right.object.span()) != name
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.'))
        || !text(class.span).ends_with(';')
    {
        return None;
    }
    let value = def.right.span();
    Some([
        (value.start, value.start, "t$C(".to_string()),
        (value.end, f.span.start, ", ".to_string()),
        (f.span.end, ctor.span.end, ");".to_string()),
    ])
}

/// A class signature, `S(C, () => ({__proto__: G(C.__proto__), m: …}))`
/// (`dart.setMethodSignature(C, …)` with `dart.getMethods`, already
/// flattened to variables): `t$G(S, G, C, () => ({m: …}))`, whose function
/// sets the same prototype on the object the thunk builds
/// (`Object.setPrototypeOf` returns it). `G(C.__proto__)` now runs after the
/// entries instead of before; both only read signatures and evaluate rtis.
/// One replacement, up to the second property, so that the ones inside the
/// entries still apply.
fn signature_form(source: &str, it: &ExpressionStatement<'_>) -> Option<(u32, u32, String)> {
    use oxc_ast::ast::{ObjectPropertyKind, PropertyKey};
    let Expression::CallExpression(call) = &it.expression else {
        return None;
    };
    let Expression::Identifier(setter) = &call.callee else {
        return None;
    };
    let [class, thunk] = &call.arguments[..] else {
        return None;
    };
    let (Some(class), Some(Expression::ArrowFunctionExpression(arrow))) =
        (class.as_expression(), thunk.as_expression())
    else {
        return None;
    };
    if arrow.r#async || !arrow.params.items.is_empty() {
        return None;
    }
    let object = match arrow.body.as_expression()? {
        Expression::ParenthesizedExpression(p) => &p.expression,
        e => e,
    };
    let Expression::ObjectExpression(object) = object else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(proto), second, ..] = &object.properties[..] else {
        return None;
    };
    if proto.computed
        || proto.shorthand
        || !matches!(&proto.key, PropertyKey::StaticIdentifier(k) if k.name == "__proto__")
    {
        return None;
    }
    let Expression::CallExpression(get) = &proto.value else {
        return None;
    };
    let Expression::Identifier(getter) = &get.callee else {
        return None;
    };
    let [arg] = &get.arguments[..] else {
        return None;
    };
    let Some(Expression::StaticMemberExpression(sup)) = arg.as_expression() else {
        return None;
    };
    let text = |s: oxc_span::Span| &source[s.start as usize..s.end as usize];
    let c = text(class.span());
    // The class must be the same expression, and a simple one (no effect
    // when read twice).
    if sup.property.name != "__proto__"
        || text(sup.object.span()) != c
        || !c
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'$' | b'.'))
    {
        return None;
    }
    Some((
        call.span.start,
        second.span().start,
        format!("t$G({}, {}, {c}, () => ({{", setter.name, getter.name),
    ))
}

/// The prologue of a generic constructor, `X.$ti = X.$ti || t || G(X);`
/// (`G` is `dart.getReifiedType`, maybe flattened), with `X` and `t` plain
/// names: (start, end, `X`, `t`, text of `G`). [`apply`] turns the most
/// common `G` into `t$Q(X, t)`.
fn type_args_form(
    source: &str,
    it: &ExpressionStatement<'_>,
) -> Option<(u32, u32, String, String, String)> {
    use oxc_ast::ast::LogicalOperator;
    let Expression::AssignmentExpression(a) = &it.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(left) = &a.left else {
        return None;
    };
    let Expression::LogicalExpression(outer) = &a.right else {
        return None;
    };
    let Expression::LogicalExpression(inner) = &outer.left else {
        return None;
    };
    let Expression::CallExpression(call) = &outer.right else {
        return None;
    };
    let Expression::StaticMemberExpression(read) = &inner.left else {
        return None;
    };
    let Expression::Identifier(t) = &inner.right else {
        return None;
    };
    let [arg] = &call.arguments[..] else {
        return None;
    };
    let text = |s: oxc_span::Span| &source[s.start as usize..s.end as usize];
    let x = text(left.object.span());
    let simple =
        |e: &Expression<'_>| matches!(e, Expression::Identifier(_) | Expression::ThisExpression(_));
    if a.operator != AssignmentOperator::Assign
        || outer.operator != LogicalOperator::Or
        || inner.operator != LogicalOperator::Or
        || left.property.name != "$ti"
        || read.property.name != "$ti"
        || !simple(&left.object)
        || text(read.object.span()) != x
        || arg
            .as_expression()
            .is_none_or(|e| !simple(e) || text(e.span()) != x)
        || !matches!(
            &call.callee,
            Expression::Identifier(_) | Expression::StaticMemberExpression(_)
        )
    {
        return None;
    }
    Some((
        it.span.start,
        it.span.end,
        x.to_string(),
        t.name.to_string(),
        text(call.callee.span()).to_string(),
    ))
}

/// The prologue of a named parameter, `o && "k" in o ? o.k : d`, with `d` a
/// literal or a variable (no effect, so evaluating it earlier changes
/// nothing; a Dart parameter's default value is constant): `t$O(o, "k", d)`.
fn named_prologue(source: &str, it: &oxc_ast::ast::ConditionalExpression<'_>) -> Option<String> {
    use oxc_ast::ast::{BinaryOperator, LogicalOperator};
    let Expression::LogicalExpression(l) = &it.test else {
        return None;
    };
    if l.operator != LogicalOperator::And {
        return None;
    }
    let Expression::Identifier(o) = &l.left else {
        return None;
    };
    let Expression::BinaryExpression(b) = &l.right else {
        return None;
    };
    if b.operator != BinaryOperator::In {
        return None;
    }
    let Expression::StringLiteral(k) = &b.left else {
        return None;
    };
    if !matches!(&b.right, Expression::Identifier(o2) if o2.name == o.name) {
        return None;
    }
    let read = match &it.consequent {
        Expression::StaticMemberExpression(m) => {
            matches!(&m.object, Expression::Identifier(o3) if o3.name == o.name)
                && m.property.name == k.value
        }
        Expression::ComputedMemberExpression(m) => {
            matches!(&m.object, Expression::Identifier(o3) if o3.name == o.name)
                && matches!(&m.expression, Expression::StringLiteral(k2) if k2.value == k.value)
        }
        _ => false,
    };
    let simple = matches!(
        &it.alternate,
        Expression::NullLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::Identifier(_)
    ) || matches!(&it.alternate, Expression::UnaryExpression(u) if u.operator == oxc_ast::ast::UnaryOperator::Void && matches!(u.argument, Expression::NumericLiteral(_)))
        || matches!(&it.alternate, Expression::UnaryExpression(u) if u.operator == oxc_ast::ast::UnaryOperator::UnaryNegation && matches!(u.argument, Expression::NumericLiteral(_)));
    if !read || !simple {
        return None;
    }
    let text = |s: oxc_span::Span| &source[s.start as usize..s.end as usize];
    Some(format!(
        "t$O({}, {}, {})",
        o.name,
        text(k.span),
        text(it.alternate.span())
    ))
}

/// Applies the compact forms to `source` (the file already wrapped in
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
    let head = "(function () {\n";
    if !source.starts_with(head) || source.contains("t$P") {
        return Ok(source.to_string());
    }
    let sdk_end = crate::minify::marker_start(&parsed.program);
    let mut a = Finder {
        source,
        edits: Vec::new(),
        sdk_end,
        strings: std::collections::HashMap::new(),
        recipes: Vec::new(),
        type_args: Vec::new(),
        merged: std::collections::HashSet::new(),
        merge_constructors: std::env::var("DARTFORGE_JSPROD_CLASS_CONSTRUCTOR")
            .map_or(true, |v| v != "0"),
    };
    a.visit_program(&parsed.program);
    // The generic constructor prologue through `t$Q`, for the most common
    // reified-type function (the callee is a variable after flattening, and
    // the helper must name the same one).
    let mut helpers = HELPERS.to_string();
    let mut by_callee: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (_, _, _, _, g) in &a.type_args {
        *by_callee.entry(g.as_str()).or_default() += 1;
    }
    if let Some((g, n)) = by_callee
        .iter()
        .max_by_key(|(g, n)| (**n, std::cmp::Reverse(**g)))
        && *n >= 4
    {
        let g = g.to_string();
        helpers.push_str(&format!(
            "var t$Q = function(o, t) {{ o.$ti = o.$ti || t || {g}(o); }};\n"
        ));
        for (i, f, x, t, callee) in &a.type_args {
            if *callee == g {
                a.edits.push((*i, *f, format!("t$Q({x}, {t});")));
            }
        }
    }
    // The program's closed recipes evaluated once: a repeated `t$R("R")`
    // becomes the variable `t$T…`, declared right after the SDK (the rti
    // universe already exists, and the result is the same canonical object).
    // `DARTFORGE_JSPROD_FIXED_RECIPES=0` turns it off.
    if sdk_end != u32::MAX
        && std::env::var("DARTFORGE_JSPROD_FIXED_RECIPES").map_or(true, |v| v != "0")
    {
        let mut count: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (_, _, r) in &a.recipes {
            *count.entry(r.as_str()).or_default() += 1;
        }
        let mut repeated: Vec<&str> = count
            .iter()
            .filter(|(_, n)| **n >= 2)
            .map(|(r, _)| *r)
            .collect();
        repeated.sort_unstable();
        let names: std::collections::HashMap<&str, String> = repeated
            .iter()
            .enumerate()
            .map(|(k, r)| (*r, format!("t$T{k}")))
            .collect();
        let mut edits: Vec<(u32, u32, String)> = Vec::new();
        for (i, f, r) in &a.recipes {
            if let Some(n) = names.get(r.as_str()) {
                edits.push((*i, *f, n.clone()));
            }
        }
        if !repeated.is_empty() {
            // Right after the marker (`"dartforge:sdk-end";`).
            let after = source[sdk_end as usize..]
                .find(';')
                .map(|k| sdk_end + k as u32 + 1);
            if let Some(d) = after {
                let decl: Vec<String> = repeated
                    .iter()
                    .map(|r| {
                        format!(
                            "{} = t$R({})",
                            names[r],
                            dartforge_emit_js::js::string_literal(r)
                        )
                    })
                    .collect();
                edits.push((d, d, format!("\nvar {};", decl.join(",\n"))));
                a.edits.extend(edits);
            }
        }
    }
    if a.edits.is_empty() {
        return Ok(source.to_string());
    }
    a.edits.sort_by_key(|t| (t.0, t.1));
    let mut out = String::with_capacity(source.len());
    out.push_str(head);
    out.push_str(&helpers);
    let mut pos = head.len();
    for (i, f, new) in a.edits {
        let (i, f) = (i as usize, f as usize);
        if i < pos {
            continue;
        }
        out.push_str(&source[pos..i]);
        out.push_str(&new);
        pos = f;
    }
    out.push_str(&source[pos..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_with_prototype() {
        let js = "(function () {\nvar C = class {};\n(C.new = function(x) {\n  this.x = x;\n}).prototype = C.prototype;\n(C.n = function() {}).prototype = D.prototype;\n})();\n";
        let s = apply(js).unwrap();
        assert!(
            s.contains("t$P(C.new = function(x) {\n  this.x = x;\n}, C);"),
            "{s}"
        );
        assert!(
            s.contains("(C.n = function() {}).prototype = D.prototype;"),
            "{s}"
        );
        assert!(s.contains(HELPERS), "{s}");
    }

    #[test]
    fn class_with_default_constructor() {
        let js = "(function () {\nC = class C extends P {\n  m() { return 1; }\n};\n(C.new = function(x) {\n  this.x = x;\n}).prototype = C.prototype;\n(C.n = function() {}).prototype = C.prototype;\nD = class {};\nf();\n(D.new = function() {}).prototype = D.prototype;\n})();\n";
        let s = apply(js).unwrap();
        assert!(s.contains("C = t$C(class C extends P {\n  m() { return 1; }\n}, function(x) {\n  this.x = x;\n});"), "{s}");
        assert!(s.contains("t$P(C.n = function() {}, C);"), "{s}");
        // Not adjacent: the usual prototype form.
        assert!(
            s.contains("D = class {};\nf();\nt$P(D.new = function() {}, D);"),
            "{s}"
        );
    }

    #[test]
    fn generic_constructor_prologue() {
        let js = "(function () {\nvar C = class {};\nfunction f(_ti) { let r = this; r.$ti = r.$ti || _ti || G(r); }\nfunction g(_ti) { this.$ti = this.$ti || _ti || G(this); }\nfunction h(_ti) { this.$ti = this.$ti || _ti || G(this); }\nfunction i(_ti) { this.$ti = this.$ti || _ti || G(this); }\nfunction j(_ti) { this.$ti = this.$ti || _ti || H(this); }\n})();\n";
        let s = apply(js).unwrap();
        assert!(
            s.contains("var t$Q = function(o, t) { o.$ti = o.$ti || t || G(o); };"),
            "{s}"
        );
        assert!(s.contains("let r = this; t$Q(r, _ti); }"), "{s}");
        assert!(s.contains("{ t$Q(this, _ti); }"), "{s}");
        assert!(s.contains("this.$ti = this.$ti || _ti || H(this);"), "{s}");
    }

    #[test]
    fn class_signature() {
        let js = "(function () {\nvar C = class {};\nS(C, () => ({__proto__: G(C.__proto__), m: _ti => R, [k]: X}));\nS(C, () => ({__proto__: G(D.__proto__), m: X}));\n})();\n";
        let s = apply(js).unwrap();
        assert!(
            s.contains("t$G(S, G, C, () => ({m: _ti => R, [k]: X}));"),
            "{s}"
        );
        assert!(
            s.contains("S(C, () => ({__proto__: G(D.__proto__), m: X}));"),
            "{s}"
        );
    }
}
