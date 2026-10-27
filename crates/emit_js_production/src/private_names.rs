//! The runtime's private-name declarations grouped per library.
//!
//! Each private member of the SDK is a symbol created at load time by its own
//! declarator, `$P__rti__as = dart$$privateName(dart_rti, "_as")`; minified,
//! every one of them repeats the call around its name (`ie=v(h,`_as`),`). A
//! run of such declarators at the top of the file becomes one destructuring
//! per library:
//!
//! ```js
//! var t$V = (l, s) => s.split(" ").map(n => dart$$privateName(l, n)),
//!   [$P__rti__as, $P__rti__eval] = t$V(dart_rti, "_as _eval");
//! ```
//!
//! The same calls, in the same library order of first appearance, with the
//! same arguments: `privateName` caches one symbol per library and name, so
//! a variable that named an already declared pair becomes an alias of the
//! first one (`_as = $P__rti__as`) and still holds the same symbol. It runs on
//! the compressed tree, after `oxc` dropped the declarators nobody reads, and
//! only touches consecutive declarators of one `var` at the top level of the
//! file whose description has no space. `t$` is a prefix the emitter
//! reserves. `DARTFORGE_JSPROD_GROUP_PRIVATE_NAMES=0` turns it off.

use oxc_allocator::Allocator;
use oxc_ast::ast::{Argument, BindingPattern, Expression, Program, Statement, VariableDeclarator};

/// Name of the helper the grouped declarators call.
const HELPER: &str = "t$V";

/// Smallest run worth grouping: the helper costs about as much as five
/// declarators save.
const MIN_RUN: usize = 6;

/// `NAME = F(LIB, "DESC")`: the texts of a groupable declarator.
struct Declaration {
    name: String,
    callee: String,
    library: String,
    description: String,
}

/// Groups the private-name declarators of `program` in place. Returns the
/// descriptions of the runtime's declarators (those that start before
/// `runtime_end`, the SDK end marker): their literals reserved them as
/// property names, and the caller keeps them reserved.
pub fn group<'a>(
    allocator: &'a Allocator,
    program: &mut Program<'a>,
    runtime_end: u32,
) -> Vec<String> {
    let mut descriptions = Vec::new();
    let Some(body) = outer_body(program) else {
        return descriptions;
    };
    if mentions(body, HELPER) {
        return descriptions;
    }
    let mut helper: Option<String> = None;
    for s in body.iter_mut() {
        let Statement::VariableDeclaration(v) = s else {
            continue;
        };
        let found: Vec<Option<Declaration>> = v.declarations.iter().map(declaration).collect();
        // Runs of consecutive groupable declarators with the same callee.
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < found.len() {
            let Some(first) = &found[i] else {
                i += 1;
                continue;
            };
            let mut j = i + 1;
            while j < found.len() && found[j].as_ref().is_some_and(|d| d.callee == first.callee) {
                j += 1;
            }
            if j - i >= MIN_RUN && helper.as_ref().is_none_or(|h| *h == first.callee) {
                runs.push((i, j));
            }
            i = j;
        }
        if runs.is_empty() {
            continue;
        }
        let old = std::mem::replace(&mut v.declarations, oxc_allocator::Vec::new_in(&allocator));
        let mut next_run = 0;
        for (k, d) in old.into_iter().enumerate() {
            let Some(&(start, end)) = runs.get(next_run) else {
                v.declarations.push(d);
                continue;
            };
            if k < start {
                v.declarations.push(d);
                continue;
            }
            if k == start {
                let run: Vec<&Declaration> = found[start..end].iter().flatten().collect();
                let mut snippet = String::new();
                if helper.is_none() {
                    helper = Some(run[0].callee.clone());
                    snippet.push_str(&format!(
                        "{HELPER} = (l, s) => s.split(\" \").map(n => {}(l, n)), ",
                        run[0].callee
                    ));
                }
                snippet.push_str(&grouped(
                    &run,
                    d.span.start < runtime_end,
                    &mut descriptions,
                ));
                for g in parse_declarators(allocator, &snippet) {
                    v.declarations.push(g);
                }
            }
            if k + 1 == end {
                next_run += 1;
            }
        }
    }
    descriptions
}

/// The declarators that replace one run (without `var`), separated by commas.
fn grouped(run: &[&Declaration], runtime: bool, descriptions: &mut Vec<String>) -> String {
    // Libraries in order of first appearance; a repeated pair is an alias of
    // its first variable.
    let mut libraries: Vec<(&str, Vec<&str>, Vec<&str>)> = Vec::new();
    let mut first: std::collections::HashMap<(&str, &str), &str> = std::collections::HashMap::new();
    let mut aliases: Vec<(&str, &str)> = Vec::new();
    for d in run {
        let key = (d.library.as_str(), d.description.as_str());
        if let Some(f) = first.get(&key) {
            aliases.push((&d.name, f));
            continue;
        }
        first.insert(key, &d.name);
        match libraries.iter_mut().find(|(l, _, _)| *l == d.library) {
            Some((_, names, descs)) => {
                names.push(&d.name);
                descs.push(&d.description);
            }
            None => libraries.push((&d.library, vec![&d.name], vec![&d.description])),
        }
        if runtime {
            descriptions.push(d.description.clone());
        }
    }
    let mut parts: Vec<String> = libraries
        .iter()
        .map(|(library, names, descs)| {
            format!(
                "[{}] = {HELPER}({library}, {})",
                names.join(", "),
                dartforge_emit_js::js::string_literal(&descs.join(" "))
            )
        })
        .collect();
    parts.extend(aliases.iter().map(|(name, to)| format!("{name} = {to}")));
    parts.join(", ")
}

/// Parses `var <snippet>;` in the program's allocator and returns its
/// declarators.
fn parse_declarators<'a>(allocator: &'a Allocator, snippet: &str) -> Vec<VariableDeclarator<'a>> {
    let text = allocator.alloc_str(&format!("var {snippet};"));
    let parsed = oxc_parser::Parser::new(allocator, text, oxc_span::SourceType::cjs()).parse();
    debug_assert!(parsed.diagnostics.is_empty(), "{text}");
    let mut out = Vec::new();
    for s in parsed.program.body {
        if let Statement::VariableDeclaration(v) = s {
            out.extend(v.unbox().declarations);
        }
    }
    out
}

/// The statements of the function that wraps the whole file.
fn outer_body<'b, 'a>(
    program: &'b mut Program<'a>,
) -> Option<&'b mut oxc_allocator::Vec<'a, Statement<'a>>> {
    let Some(Statement::ExpressionStatement(e)) = program.body.first_mut() else {
        return None;
    };
    let Expression::CallExpression(c) = &mut e.expression else {
        return None;
    };
    let mut callee = &mut c.callee;
    while let Expression::ParenthesizedExpression(p) = callee {
        callee = &mut p.expression;
    }
    match callee {
        Expression::FunctionExpression(f) => f.body.as_mut().map(|b| &mut b.statements),
        _ => None,
    }
}

/// Whether an identifier named `name` appears among the top-level
/// declarations (the helper's name must be free).
fn mentions(body: &oxc_allocator::Vec<'_, Statement<'_>>, name: &str) -> bool {
    body.iter().any(|s| match s {
        Statement::VariableDeclaration(v) => v
            .declarations
            .iter()
            .any(|d| matches!(&d.id, BindingPattern::BindingIdentifier(b) if b.name == name)),
        Statement::FunctionDeclaration(f) => f.id.as_ref().is_some_and(|i| i.name == name),
        _ => false,
    })
}

/// `NAME = dart$$privateName(LIB, "DESC")` (or `dart.privateName`): an
/// identifier library and a description without spaces.
fn declaration(d: &VariableDeclarator<'_>) -> Option<Declaration> {
    let BindingPattern::BindingIdentifier(id) = &d.id else {
        return None;
    };
    let Some(Expression::CallExpression(c)) = &d.init else {
        return None;
    };
    if crate::unwrap::dart_member(&c.callee) != Some("privateName") || c.arguments.len() != 2 {
        return None;
    }
    let callee = match &c.callee {
        Expression::Identifier(i) => i.name.to_string(),
        Expression::StaticMemberExpression(m) => format!("dart.{}", m.property.name),
        _ => return None,
    };
    let Argument::Identifier(library) = &c.arguments[0] else {
        return None;
    };
    let Argument::StringLiteral(text) = &c.arguments[1] else {
        return None;
    };
    let description = text.value.as_str();
    if description.is_empty()
        || description
            .chars()
            .any(|c| !c.is_ascii_graphic() || c == '"' || c == '\\')
    {
        return None;
    }
    Some(Declaration {
        name: id.name.to_string(),
        callee,
        library: library.name.to_string(),
        description: description.to_string(),
    })
}

/// Name of the helper of [`group_extension_symbols`].
const SYMBOLS_HELPER: &str = "t$Y";

/// The bootstrap's `for (const n of [...]) dartx[n] = Symbol("dartx." + n)`
/// and the top-level aliases `$n = dartx.n` (`own_sdk::hoist_preludes`)
/// become one destructuring at the loop's place:
/// `var [$a, , $c] = t$Y(dartx, "a b c")`, with `t$Y` creating each symbol in
/// the same order and returning them. The aliases are `var`s of the same
/// scope, now initialized earlier — and never read before the loop, which
/// precedes every class. A name with a second alias keeps that one as it
/// was. Runs with [`group`], on the compressed tree; the names stay
/// reserved because their literals were seen before. Returns whether it
/// changed anything.
pub fn group_extension_symbols<'a>(allocator: &'a Allocator, program: &mut Program<'a>) -> bool {
    let Some(body) = outer_body(program) else {
        return false;
    };
    if mentions(body, SYMBOLS_HELPER) {
        return false;
    }
    // The loop: its index, the names and the `dartx` object's name.
    let mut found: Option<(usize, Vec<String>, String)> = None;
    for (i, s) in body.iter().enumerate() {
        if let Some((names, g)) = symbol_loop(s) {
            if found.is_some() {
                return false;
            }
            found = Some((i, names, g));
        }
    }
    let Some((at, names, g)) = found else {
        return false;
    };
    if names
        .iter()
        .any(|n| n.is_empty() || n.contains([' ', '"', '\\']))
    {
        return false;
    }
    // The aliases: `var` declarators `X = g.n` / `X = g["n"]` at the top
    // level, the first one per name.
    let mut alias: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for s in body.iter() {
        let Statement::VariableDeclaration(v) = s else {
            continue;
        };
        if v.kind != oxc_ast::ast::VariableDeclarationKind::Var {
            continue;
        }
        for d in &v.declarations {
            if let Some((x, n)) = alias_of(d, &g)
                && names.contains(&n)
                && !alias.contains_key(&n)
            {
                alias.insert(n, x);
            }
        }
    }
    if alias.is_empty() {
        return false;
    }
    let chosen: std::collections::HashSet<String> = alias.values().cloned().collect();
    for s in body.iter_mut() {
        let Statement::VariableDeclaration(v) = s else {
            continue;
        };
        if v.kind != oxc_ast::ast::VariableDeclarationKind::Var {
            continue;
        }
        let old = std::mem::replace(&mut v.declarations, oxc_allocator::Vec::new_in(&allocator));
        for d in old {
            let drop = alias_of(&d, &g)
                .is_some_and(|(x, n)| chosen.contains(&x) && alias.get(&n) == Some(&x));
            if !drop {
                v.declarations.push(d);
            }
        }
    }
    // Every name with its alias (a hole when it has none): the trailing
    // holes are left out.
    let mut pattern: Vec<&str> = names
        .iter()
        .map(|n| alias.get(n).map_or("", String::as_str))
        .collect();
    while pattern.last() == Some(&"") {
        pattern.pop();
    }
    let snippet = format!(
        "{SYMBOLS_HELPER} = (g, s) => s.split(\" \").map(n => g[n] = Symbol(\"dartx.\" + n)), [{}] = {SYMBOLS_HELPER}({g}, {})",
        pattern.join(", "),
        dartforge_emit_js::js::string_literal(&names.join(" "))
    );
    let text = allocator.alloc_str(&format!("var {snippet};"));
    let parsed = oxc_parser::Parser::new(allocator, text, oxc_span::SourceType::cjs()).parse();
    let Some(statement) = parsed.program.body.into_iter().next() else {
        return false;
    };
    body[at] = statement;
    // A declaration left without declarators is not valid JS.
    body.retain(|s| !matches!(s, Statement::VariableDeclaration(v) if v.declarations.is_empty()));
    true
}

/// `for (const n of <names>) g[n] = Symbol("dartx." + n)`, with `<names>` an
/// array of strings or `"a.b".split(".")` (compression's form): the names and
/// `g`.
fn symbol_loop(s: &Statement<'_>) -> Option<(Vec<String>, String)> {
    let Statement::ForOfStatement(f) = s else {
        return None;
    };
    let names: Vec<String> = match &f.right {
        Expression::ArrayExpression(a) => a
            .elements
            .iter()
            .map(|e| match e {
                oxc_ast::ast::ArrayExpressionElement::StringLiteral(l) => Some(l.value.to_string()),
                _ => None,
            })
            .collect::<Option<_>>()?,
        Expression::CallExpression(c) => {
            let Expression::StaticMemberExpression(m) = &c.callee else {
                return None;
            };
            let (Expression::StringLiteral(text), [Argument::StringLiteral(sep)]) =
                (&m.object, &c.arguments[..])
            else {
                return None;
            };
            if m.property.name != "split" || sep.value.is_empty() {
                return None;
            }
            text.value
                .split(sep.value.as_str())
                .map(str::to_string)
                .collect()
        }
        _ => return None,
    };
    let Statement::ExpressionStatement(e) = &f.body else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &e.expression else {
        return None;
    };
    let oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
        return None;
    };
    let Expression::Identifier(g) = &m.object else {
        return None;
    };
    let Expression::CallExpression(sym) = &a.right else {
        return None;
    };
    if !matches!(&sym.callee, Expression::Identifier(i) if i.name == "Symbol") {
        return None;
    }
    Some((names, g.name.to_string()))
}

/// `X = g.n` or `X = g["n"]`: `(X, n)`.
fn alias_of(d: &VariableDeclarator<'_>, g: &str) -> Option<(String, String)> {
    let BindingPattern::BindingIdentifier(id) = &d.id else {
        return None;
    };
    let n = match d.init.as_ref()? {
        Expression::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == g) => {
            m.property.name.to_string()
        }
        Expression::ComputedMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == g) => {
            match &m.expression {
                Expression::StringLiteral(l) => l.value.to_string(),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((id.name.to_string(), n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> (String, Vec<String>) {
        let allocator = Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, src, oxc_span::SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty());
        let mut program = parsed.program;
        let d = group(&allocator, &mut program, u32::MAX);
        (oxc_codegen::Codegen::new().build(&program).code, d)
    }

    fn decls(lines: &[(&str, &str, &str)]) -> String {
        lines
            .iter()
            .map(|(n, l, d)| format!("{n} = dart$$privateName({l}, \"{d}\")"))
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[test]
    fn a_run_becomes_one_destructuring_per_library() {
        let body = decls(&[
            ("a", "core", "_a"),
            ("b", "rti", "_b"),
            ("c", "core", "_c"),
            ("d", "rti", "_d"),
            ("e", "core", "_e"),
            ("f", "rti", "_b"),
        ]);
        let src = format!(
            "(function () {{\nvar dart$$privateName = function (l, n) {{}}, {body}, z = 1;\nuse(a, f);\n}})();\n"
        );
        let (out, descs) = run(&src);
        assert!(
            out.contains("t$V = (l, s) => s.split(\" \").map((n) => dart$$privateName(l, n))"),
            "{out}"
        );
        assert!(out.contains("[a, c, e] = t$V(core, \"_a _c _e\")"), "{out}");
        assert!(out.contains("[b, d] = t$V(rti, \"_b _d\")"), "{out}");
        assert!(out.contains("f = b, z = 1"), "{out}");
        assert_eq!(descs, ["_a", "_b", "_c", "_d", "_e"]);
        let allocator = Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, &out, oxc_span::SourceType::cjs()).parse();
        assert!(parsed.diagnostics.is_empty(), "{out}");
    }

    #[test]
    fn extension_symbols_and_their_aliases_become_one_destructuring() {
        let src = "(function () {\nvar dartx = {};\nfor (const n of \"a.b.c.d\".split(\".\")) dartx[n] = Symbol(\"dartx.\" + n);\nvar x = 1, $a = dartx.a, $c = dartx[\"c\"], $a2 = dartx.a;\nvar $d = dartx.d;\nuse($a, $c, $a2, $d);\n})();\n";
        let allocator = Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, src, oxc_span::SourceType::cjs()).parse();
        let mut program = parsed.program;
        assert!(group_extension_symbols(&allocator, &mut program));
        let out = oxc_codegen::Codegen::new().build(&program).code;
        assert!(
            out.contains("[$a, , $c, $d] = t$Y(dartx, \"a b c d\")"),
            "{out}"
        );
        let allocator2 = Allocator::default();
        let reparsed =
            oxc_parser::Parser::new(&allocator2, &out, oxc_span::SourceType::cjs()).parse();
        assert!(reparsed.diagnostics.is_empty(), "{out}");
        assert!(!out.contains("for ("), "{out}");
        assert!(out.contains("var x = 1, $a2 = dartx.a;"), "{out}");
    }

    #[test]
    fn short_runs_nested_scopes_and_odd_descriptions_stay() {
        let short = decls(&[("a", "core", "_a"), ("b", "core", "_b")]);
        let (out, _) = run(&format!("(function () {{\nvar {short};\n}})();\n"));
        assert!(!out.contains("t$V"), "{out}");
        let six = decls(&[
            ("a", "core", "_a"),
            ("b", "core", "_b"),
            ("c", "core", "_c"),
            ("d", "core", "_d"),
            ("e", "core", "_e"),
            ("f", "core", "_f"),
        ]);
        let (out, _) = run(&format!(
            "(function () {{\n(function () {{\nvar {six};\n}})();\n}})();\n"
        ));
        assert!(!out.contains("t$V"), "{out}");
        let (out, _) = run(&format!(
            "(function () {{\nvar {six}, g = dart$$privateName(core, \"a b\");\n}})();\n"
        ));
        assert!(
            out.contains("g = dart$$privateName(core, \"a b\")"),
            "{out}"
        );
        assert!(out.contains("[a, b, c, d, e, f] = t$V(core"), "{out}");
    }
}
