//! Composition of a JS file by token category, measured over the `oxc` AST:
//! how much of the file is *strings* (and of which kind: rti recipe, rules
//! JSON, others), property names, identifiers and the rest (punctuation,
//! keywords, numbers). Measures what takes up the production file before
//! specifying what to shorten (`docs/JS-PRODUCAO-TAMANHO.md`).
//!
//! `cargo run --release -p dartforge-emit-js-production --example composition -- <file.js> [N]`
use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

#[derive(Default)]
struct Count {
    strings_rti: usize,
    strings_json: usize,
    strings_other: usize,
    n_strings: usize,
    templates: usize,
    properties: usize,
    keys: usize,
    refs: usize,
    bindings: usize,
    by_property: HashMap<String, usize>,
    by_string: HashMap<String, usize>,
}

impl<'a> Visit<'a> for Count {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        let n = it.span.size() as usize;
        self.n_strings += 1;
        let v = it.value.as_str();
        if v.starts_with('{') && v.contains("\":") {
            self.strings_json += n;
        } else if v.contains('|') && !v.contains(' ') {
            self.strings_rti += n;
        } else {
            self.strings_other += n;
        }
        *self
            .by_string
            .entry(v.chars().take(60).collect())
            .or_default() += n;
    }
    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        if it.expressions.is_empty() {
            // No interpolation: `oxc` prints *strings* as `...`.
            let n = it.span.size() as usize;
            self.n_strings += 1;
            let v = it
                .quasis
                .first()
                .map(|q| q.value.raw.as_str())
                .unwrap_or("");
            if v.starts_with('{') && v.contains("\":") {
                self.strings_json += n;
            } else if v.contains('|') && !v.contains(' ') {
                self.strings_rti += n;
            } else {
                self.strings_other += n;
            }
            *self
                .by_string
                .entry(v.chars().take(60).collect())
                .or_default() += n;
            return;
        }
        for q in &it.quasis {
            self.templates += q.span.size() as usize;
        }
        walk::walk_template_literal(self, it);
    }
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        let n = it.property.span.size() as usize + 1;
        self.properties += n;
        *self
            .by_property
            .entry(it.property.name.to_string())
            .or_default() += n;
        walk::walk_static_member_expression(self, it);
    }
    fn visit_property_key(&mut self, it: &PropertyKey<'a>) {
        if let PropertyKey::StaticIdentifier(id) = it {
            let n = id.span.size() as usize;
            self.keys += n;
            *self.by_property.entry(id.name.to_string()).or_default() += n;
            return;
        }
        walk::walk_property_key(self, it);
    }
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.refs += it.span().size() as usize;
    }
    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        self.bindings += it.span.size() as usize;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).expect("usage: composition <file.js> [N]");
    let top: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    let source = std::fs::read_to_string(path).expect("read");
    let alloc = Allocator::default();
    let parsed = Parser::new(&alloc, &source, SourceType::cjs()).parse();
    if !parsed.diagnostics.is_empty() {
        eprintln!(
            "{} parse error(s); the first: {:?}",
            parsed.diagnostics.len(),
            parsed.diagnostics[0]
        );
    }
    let mut c = Count::default();
    c.visit_program(&parsed.program);
    let total = source.len();
    let pct = |n: usize| 100.0 * n as f64 / total as f64;
    let line = |label: &str, n: usize| println!("{label:<38} {n:>12} {:>6.1}%", pct(n));
    line("total", total);
    line("strings: rti recipes (a|B<...>)", c.strings_rti);
    line("strings: JSON (rti rules)", c.strings_json);
    line("strings: other", c.strings_other);
    line("templates with interpolation (text)", c.templates);
    line("property names (.x)", c.properties);
    line("object/class keys (x: / x(){})", c.keys);
    line("identifiers (references)", c.refs);
    line("identifiers (declarations)", c.bindings);
    let rest = total
        - c.strings_rti
        - c.strings_json
        - c.strings_other
        - c.templates
        - c.properties
        - c.keys
        - c.refs
        - c.bindings;
    line("rest (punctuation, keywords...)", rest);
    println!("strings: {}", c.n_strings);
    let mut p: Vec<_> = c.by_property.into_iter().collect();
    p.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("\nmost expensive property/key names:");
    for (n, b) in p.iter().take(top) {
        println!("  {b:>10}  {n}");
    }
    let mut s: Vec<_> = c.by_string.into_iter().collect();
    s.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("\nmost expensive strings (summed by value):");
    for (n, b) in s.iter().take(top) {
        println!("  {b:>10}  {n}");
    }
}
