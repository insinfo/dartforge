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
    /// Início da marca do fim do SDK (`minificar::MARCA_FIM_DO_SDK`).
    fim_sdk: u32,
    /// Valores das `t$S…` da deduplicação de *strings*.
    strings: std::collections::HashMap<String, String>,
    /// Chamadas `t$R("receita")` do programa: (início, fim, receita).
    receitas: Vec<(u32, u32, String)>,
}

impl<'a> Visit<'a> for Achados<'_> {
    fn visit_expression_statement(&mut self, it: &ExpressionStatement<'a>) {
        if let Some(t) = prototipo(self.fonte, it) {
            self.trocas.extend(t);
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
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        if it.span.start > self.fim_sdk
            && matches!(&it.callee, Expression::Identifier(i) if i.name == "t$R")
            && it.arguments.len() == 1
        {
            let receita = match it.arguments[0].as_expression() {
                Some(Expression::StringLiteral(l)) => Some(l.value.to_string()),
                Some(Expression::Identifier(i)) => self.strings.get(i.name.as_str()).cloned(),
                _ => None,
            };
            if let Some(r) = receita {
                self.receitas.push((it.span.start, it.span.end, r));
                return;
            }
        }
        walk::walk_call_expression(self, it);
    }
}

/// `(C.n = function(…){…}).prototype = C.prototype;` → `t$P(C.n = function(…){…}, C);`,
/// em duas trocas (o `(` do começo e o `).prototype = C.prototype;` do fim),
/// para que as de dentro da função continuem valendo.
fn prototipo(fonte: &str, it: &ExpressionStatement<'_>) -> Option<[(u32, u32, String); 2]> {
    let Expression::AssignmentExpression(a) = &it.expression else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(esq) = &a.left else {
        return None;
    };
    let Expression::StaticMemberExpression(dir) = &a.right else {
        return None;
    };
    if esq.property.name != "prototype" || dir.property.name != "prototype" {
        return None;
    }
    let Expression::ParenthesizedExpression(p) = &esq.object else {
        return None;
    };
    let Expression::AssignmentExpression(interna) = &p.expression else {
        return None;
    };
    if interna.operator != AssignmentOperator::Assign
        || !matches!(interna.right, Expression::FunctionExpression(_))
    {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(alvo) = &interna.left else {
        return None;
    };
    let texto = |s: oxc_span::Span| &fonte[s.start as usize..s.end as usize];
    let classe = texto(dir.object.span());
    // A classe tem de ser a mesma expressão, e simples (sem efeito ao ler).
    if texto(alvo.object.span()) != classe
        || !classe
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'$' | b'.'))
    {
        return None;
    }
    if !texto(p.span).starts_with('(') || !texto(p.span).ends_with(')') {
        return None;
    }
    Some([
        (p.span.start, p.span.start + 1, "t$P(".to_string()),
        (p.span.end - 1, it.span.end, format!(", {classe});")),
    ])
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
    let fim_sdk = crate::minificar::inicio_da_marca(&lido.program);
    let mut a = Achados {
        fonte,
        trocas: Vec::new(),
        fim_sdk,
        strings: std::collections::HashMap::new(),
        receitas: Vec::new(),
    };
    a.visit_program(&lido.program);
    // Receitas fechadas do programa avaliadas uma vez: `t$R("R")` repetido
    // vira a variável `t$T…`, declarada logo depois do SDK (o universo rti
    // já existe, e o resultado é o mesmo objeto canônico).
    // `DARTFORGE_JSPROD_RECEITAS_FIXAS=0` desliga.
    if fim_sdk != u32::MAX
        && std::env::var("DARTFORGE_JSPROD_RECEITAS_FIXAS").map_or(true, |v| v != "0")
    {
        let mut contagem: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for (_, _, r) in &a.receitas {
            *contagem.entry(r.as_str()).or_default() += 1;
        }
        let mut repetidas: Vec<&str> = contagem
            .iter()
            .filter(|(_, n)| **n >= 2)
            .map(|(r, _)| *r)
            .collect();
        repetidas.sort_unstable();
        let nomes: std::collections::HashMap<&str, String> = repetidas
            .iter()
            .enumerate()
            .map(|(k, r)| (*r, format!("t$T{k}")))
            .collect();
        let mut trocas: Vec<(u32, u32, String)> = Vec::new();
        for (i, f, r) in &a.receitas {
            if let Some(n) = nomes.get(r.as_str()) {
                trocas.push((*i, *f, n.clone()));
            }
        }
        if !repetidas.is_empty() {
            // Logo depois da marca (`"dartforge:fim-do-sdk";`).
            let depois = fonte[fim_sdk as usize..]
                .find(';')
                .map(|k| fim_sdk + k as u32 + 1);
            if let Some(d) = depois {
                let decl: Vec<String> = repetidas
                    .iter()
                    .map(|r| {
                        format!(
                            "{} = t$R({})",
                            nomes[r],
                            dartforge_emit_js::js::string_literal(r)
                        )
                    })
                    .collect();
                trocas.push((d, d, format!("\nvar {};", decl.join(",\n"))));
                a.trocas.extend(trocas);
            }
        }
    }
    if a.trocas.is_empty() {
        return Ok(fonte.to_string());
    }
    a.trocas.sort_by_key(|t| (t.0, t.1));
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
        assert!(
            s.contains("t$P(C.new = function(x) {\n  this.x = x;\n}, C);"),
            "{s}"
        );
        assert!(
            s.contains("(C.n = function() {}).prototype = D.prototype;"),
            "{s}"
        );
        assert!(s.contains(AUXILIAR), "{s}");
    }
}
