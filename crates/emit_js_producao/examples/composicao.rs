//! Composição de um arquivo JS por categoria de token, medida sobre a AST do
//! `oxc`: quanto do arquivo são *strings* (e de que tipo: receita rti, JSON
//! de regras, outras), nomes de propriedade, identificadores e o resto
//! (pontuação, palavras-chave, números). Mede o que ocupa o arquivo de
//! produção antes de especificar o que encurtar
//! (`docs/JS-PRODUCAO-TAMANHO.md`).
//!
//! `cargo run --release -p dartforge-emit-js-producao --example composicao -- <arquivo.js> [N]`
use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

#[derive(Default)]
struct Contagem {
    strings_rti: usize,
    strings_json: usize,
    strings_outras: usize,
    n_strings: usize,
    templates: usize,
    propriedades: usize,
    chaves: usize,
    refs: usize,
    bindings: usize,
    por_propriedade: HashMap<String, usize>,
    por_string: HashMap<String, usize>,
}

impl<'a> Visit<'a> for Contagem {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        let n = it.span.size() as usize;
        self.n_strings += 1;
        let v = it.value.as_str();
        if v.starts_with('{') && v.contains("\":") {
            self.strings_json += n;
        } else if v.contains('|') && !v.contains(' ') {
            self.strings_rti += n;
        } else {
            self.strings_outras += n;
        }
        *self.por_string.entry(v.chars().take(60).collect()).or_default() += n;
    }
    fn visit_template_literal(&mut self, it: &TemplateLiteral<'a>) {
        if it.expressions.is_empty() {
            // Sem interpolação: o `oxc` imprime *strings* como `...`.
            let n = it.span.size() as usize;
            self.n_strings += 1;
            let v = it.quasis.first().map(|q| q.value.raw.as_str()).unwrap_or("");
            if v.starts_with('{') && v.contains("\":") {
                self.strings_json += n;
            } else if v.contains('|') && !v.contains(' ') {
                self.strings_rti += n;
            } else {
                self.strings_outras += n;
            }
            *self.por_string.entry(v.chars().take(60).collect()).or_default() += n;
            return;
        }
        for q in &it.quasis {
            self.templates += q.span.size() as usize;
        }
        walk::walk_template_literal(self, it);
    }
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        let n = it.property.span.size() as usize + 1;
        self.propriedades += n;
        *self.por_propriedade.entry(it.property.name.to_string()).or_default() += n;
        walk::walk_static_member_expression(self, it);
    }
    fn visit_property_key(&mut self, it: &PropertyKey<'a>) {
        if let PropertyKey::StaticIdentifier(id) = it {
            let n = id.span.size() as usize;
            self.chaves += n;
            *self.por_propriedade.entry(id.name.to_string()).or_default() += n;
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
    let caminho = args.get(1).expect("uso: composicao <arquivo.js> [N]");
    let top: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    let fonte = std::fs::read_to_string(caminho).expect("leitura");
    let alloc = Allocator::default();
    let lido = Parser::new(&alloc, &fonte, SourceType::cjs()).parse();
    if !lido.diagnostics.is_empty() {
        eprintln!("{} erro(s) de parse; o primeiro: {:?}", lido.diagnostics.len(), lido.diagnostics[0]);
    }
    let mut c = Contagem::default();
    c.visit_program(&lido.program);
    let total = fonte.len();
    let pct = |n: usize| 100.0 * n as f64 / total as f64;
    let linha = |rot: &str, n: usize| println!("{rot:<38} {n:>12} {:>6.1}%", pct(n));
    linha("total", total);
    linha("strings: receitas rti (a|B<...>)", c.strings_rti);
    linha("strings: JSON (regras rti)", c.strings_json);
    linha("strings: outras", c.strings_outras);
    linha("templates com interpolação (texto)", c.templates);
    linha("nomes de propriedade (.x)", c.propriedades);
    linha("chaves de objeto/classe (x: / x(){})", c.chaves);
    linha("identificadores (referências)", c.refs);
    linha("identificadores (declarações)", c.bindings);
    let resto = total - c.strings_rti - c.strings_json - c.strings_outras - c.templates - c.propriedades - c.chaves - c.refs - c.bindings;
    linha("resto (pontuação, palavras-chave...)", resto);
    println!("strings: {}", c.n_strings);
    let mut p: Vec<_> = c.por_propriedade.into_iter().collect();
    p.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nnomes de propriedade/chave mais caros:");
    for (n, b) in p.iter().take(top) {
        println!("  {b:>10}  {n}");
    }
    let mut s: Vec<_> = c.por_string.into_iter().collect();
    s.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nstrings mais caras (somadas por valor):");
    for (n, b) in s.iter().take(top) {
        println!("  {b:>10}  {n}");
    }
}
