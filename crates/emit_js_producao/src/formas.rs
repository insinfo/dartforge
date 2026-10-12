//! Formas compactas de construções que o emissor repete
//! (`docs/JS-PRODUCAO-TAMANHO.md` §9.8).
//!
//! O construtor Dart no contrato do DDC é uma função com o protótipo da
//! classe: `(C.nome = function(…) {…}).prototype = C.prototype;`, uma por
//! construtor (4 mil no `new_sali`). Aqui ela vira
//! `t$P(C.nome = function(…) {…}, C);`, com `t$P` declarada uma vez no topo do
//! arquivo: a mesma atribuição, na mesma ordem (o argumento é avaliado antes
//! da chamada), e o mesmo protótipo.
//!
//! A troca é de texto, pelos trechos do `oxc`, como no `achatar.rs`.
//! `DARTFORGE_JSPROD_FORMAS=0` desliga.

use oxc_ast::ast::{AssignmentOperator, AssignmentTarget, Expression, ExpressionStatement};
use oxc_ast_visit::{Visit, walk};
use oxc_span::GetSpan;

/// A função auxiliar, declarada no topo do arquivo.
const AUXILIAR: &str = "var t$P = function(f, c) { f.prototype = c.prototype; };\n";

struct Achados<'s> {
    fonte: &'s str,
    /// (início, fim, texto novo) de cada instrução trocada.
    trocas: Vec<(u32, u32, String)>,
}

impl<'a> Visit<'a> for Achados<'_> {
    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        if let Some(novo) = prototipo(self.fonte, it) {
            self.trocas.push((it.span.start, it.span.end, novo));
        }
        walk::walk_expression_statement(self, it);
    }
}

/// `(C.n = function(…){…}).prototype = C.prototype;` → `t$P(C.n = function(…){…}, C);`.
fn prototipo(fonte: &str, it: &ExpressionStatement<'_>) -> Option<String> {
    let Expression::AssignmentExpression(a) = &it.expression else { return None };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(esq) = &a.left else { return None };
    let Expression::StaticMemberExpression(dir) = &a.right else { return None };
    if esq.property.name != "prototype" || dir.property.name != "prototype" {
        return None;
    }
    let Expression::ParenthesizedExpression(p) = &esq.object else { return None };
    let Expression::AssignmentExpression(interna) = &p.expression else { return None };
    if interna.operator != AssignmentOperator::Assign || !matches!(interna.right, Expression::FunctionExpression(_)) {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(alvo) = &interna.left else { return None };
    let texto = |s: oxc_span::Span| &fonte[s.start as usize..s.end as usize];
    let classe = texto(dir.object.span());
    // A classe tem de ser a mesma expressão, e simples (sem efeito ao ler).
    if texto(alvo.object.span()) != classe || !classe.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.')) {
        return None;
    }
    Some(format!("t$P({}, {classe});", texto(interna.span)))
}

/// Aplica as formas compactas a `fonte` (o arquivo já embrulhado em
/// `(function () {…})();`).
///
/// # Erros
///
/// O texto não é JS válido para o parser.
pub fn aplicar(fonte: &str) -> Result<String, String> {
    let alocador = oxc_allocator::Allocator::default();
    let lido = oxc_parser::Parser::new(&alocador, fonte, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = lido.diagnostics.first() {
        return Err(format!("oxc não leu o arquivo de produção: {e}"));
    }
    let cabeca = "(function () {\n";
    if !fonte.starts_with(cabeca) || fonte.contains("t$P") {
        return Ok(fonte.to_string());
    }
    let mut a = Achados { fonte, trocas: Vec::new() };
    a.visit_program(&lido.program);
    if a.trocas.is_empty() {
        return Ok(fonte.to_string());
    }
    a.trocas.sort_by_key(|t| t.0);
    let mut out = String::with_capacity(fonte.len());
    out.push_str(cabeca);
    out.push_str(AUXILIAR);
    let mut pos = cabeca.len();
    for (i, f, novo) in a.trocas {
        let (i, f) = (i as usize, f as usize);
        if i < pos {
            continue;
        }
        out.push_str(&fonte[pos..i]);
        out.push_str(&novo);
        pos = f;
    }
    out.push_str(&fonte[pos..]);
    Ok(out)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn construtor_com_prototipo() {
        let js = "(function () {\nvar C = class {};\n(C.new = function(x) {\n  this.x = x;\n}).prototype = C.prototype;\n(C.n = function() {}).prototype = D.prototype;\n})();\n";
        let s = aplicar(js).unwrap();
        assert!(s.contains("t$P(C.new = function(x) {\n  this.x = x;\n}, C);"), "{s}");
        assert!(s.contains("(C.n = function() {}).prototype = D.prototype;"), "{s}");
        assert!(s.contains(AUXILIAR), "{s}");
    }
}
