//! Quatro assistências só sintáticas do servidor do Dart 3.6.2, pelas
//! regras de docs/LSP-ESPECIFICACAO.md §13.8.2 e §13.10:
//!
//! | Assistência | Espécie |
//! | --- | --- |
//! | `Add digit separators` | `refactor.add.digitSeparators` |
//! | `Remove digit separators` | `refactor.remove.digitSeparators` |
//! | `Convert to is!` | `refactor.convert.isNot` |
//! | `Exchange operands` | `refactor.exchangeOperands` |
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::acoes::AcaoDeCodigo;
use crate::projeto::Projeto;
use crate::Edicao;
use dartforge_diagnostics::Span;
use dartforge_elements::model::UnitId;
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, UnaryOp};

fn acao(uri: &str, titulo: &str, especie: &str, edicoes: Vec<(Span, String)>) -> AcaoDeCodigo {
    AcaoDeCodigo {
        titulo: titulo.into(),
        especie: especie.into(),
        edicoes: edicoes.into_iter().map(|(span, texto)| Edicao { uri: uri.to_string(), span, texto }).collect(),
        diagnostico: None,
        criar_arquivo: None,
    }
}

/// `addSep` (`add_digit_separators.dart:169-185`): os dígitos em grupos de
/// `grupo`, alinhados à direita; `None` com menos de `minimo` dígitos.
fn separar(digitos: &str, grupo: usize, minimo: usize) -> Option<String> {
    let n = digitos.len();
    if n < minimo || !digitos.is_ascii() {
        return None;
    }
    let mut k = n % grupo;
    if k == 0 {
        k = grupo;
    }
    let mut saida = digitos[..k].to_string();
    while k + grupo <= n {
        saida.push('_');
        saida.push_str(&digitos[k..k + grupo]);
        k += grupo;
    }
    Some(saida)
}

/// O literal numérico `lexema` com os separadores refeitos, ou `None` se a
/// assistência não se aplica (poucos dígitos, ou o resultado é o próprio
/// lexema).
fn com_separadores(lexema: &str, de_ponto_flutuante: bool) -> Option<String> {
    let fonte: String = lexema.chars().filter(|c| *c != '_').collect();
    let resultado = if fonte.starts_with("0x") || fonte.starts_with("0X") {
        // Hexadecimal: de dois em dois, com pelo menos quatro dígitos.
        format!("{}{}", &fonte[..2], separar(&fonte[2..], 2, 4)?)
    } else if !de_ponto_flutuante {
        separar(&fonte, 3, 5)?
    } else {
        let expoente_em = fonte.find('e').or_else(|| fonte.find('E'));
        let ponto = fonte.find('.');
        let fim_da_mantissa = expoente_em.unwrap_or(fonte.len());
        let (inteiro, fracao) = match ponto {
            Some(p) => (&fonte[..p], Some(&fonte[p + 1..fim_da_mantissa])),
            None => (&fonte[..fim_da_mantissa], None),
        };
        let mut r = separar(inteiro, 3, 5).unwrap_or_else(|| inteiro.to_string());
        if let Some(f) = fracao {
            // Na fração os grupos se alinham à esquerda.
            let invertida: String = f.chars().rev().collect();
            let separada = separar(&invertida, 3, 5).map(|s| s.chars().rev().collect::<String>());
            r.push('.');
            r.push_str(&separada.unwrap_or_else(|| f.to_string()));
        }
        if let Some(e) = expoente_em {
            let depois = &fonte[e + 1..];
            let (negativo, expoente) = match depois.strip_prefix('-') {
                Some(resto) => (true, resto),
                None => (false, depois),
            };
            r.push_str(&fonte[e..e + 1]);
            if negativo {
                r.push('-');
            }
            r.push_str(&separar(expoente, 3, 5).unwrap_or_else(|| expoente.to_string()));
        }
        r
    };
    (resultado != lexema).then_some(resultado)
}

/// `getExpressionParentPrecedence(filho) >= Precedence.relational`: o pai
/// de `filho` é uma expressão que liga pelo menos tão forte quanto um
/// operador relacional.
fn pai_relacional_ou_mais(a: &ast::Ast, filho: ExprId) -> bool {
    a.exprs.iter().any(|e| match &e.kind {
        ExprKind::Binary { op, left, right } if *left == filho || *right == filho => {
            !matches!(op, BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::And | BinaryOp::Or | BinaryOp::IfNull)
        }
        ExprKind::Is { value, .. } | ExprKind::As { value, .. } => *value == filho,
        ExprKind::Unary { operand, .. } => *operand == filho,
        ExprKind::Await(x) => *x == filho,
        ExprKind::Property { target, .. } | ExprKind::Call { target, .. } | ExprKind::TypeArguments { target, .. } => *target == filho,
        // O alvo de um índice é pós-fixo; o índice em si conta como atribuição.
        ExprKind::Index { target, .. } => *target == filho,
        _ => false,
    })
}

impl Projeto {
    /// As assistências desta lista com o cursor em `offset` de `unidade`.
    pub(crate) fn assistencias_sintaticas(&self, uri: &str, unidade: UnitId, offset: usize) -> Vec<AcaoDeCodigo> {
        let mut saida = Vec::new();
        let u = self.programa().unit(unidade);
        let a = &u.ast;
        let fonte = u.source.as_str();
        let contem = |s: Span| s.start <= offset && offset <= s.end;

        // Os separadores de dígitos: o literal numérico sob o cursor.
        for e in a.exprs.iter() {
            let (span, flutuante) = match &e.kind {
                ExprKind::Int(s) => (*s, false),
                ExprKind::Double(s) => (*s, true),
                _ => continue,
            };
            if !contem(span) {
                continue;
            }
            let Some(lexema) = fonte.get(span.start..span.end) else { continue };
            if let Some(novo) = com_separadores(lexema, flutuante) {
                saida.push(acao(uri, "Add digit separators", "refactor.add.digitSeparators", vec![(span, novo)]));
            }
            if lexema.contains('_') {
                let sem: String = lexema.chars().filter(|c| *c != '_').collect();
                saida.push(acao(uri, "Remove digit separators", "refactor.remove.digitSeparators", vec![(span, sem)]));
            }
            break;
        }

        // `Convert to is!`: `!(x is T)` com o cursor no `!`, num parêntese
        // ou dentro do `is`.
        for (i, e) in a.exprs.iter().enumerate() {
            let ExprKind::Unary { op: UnaryOp::Not, operand } = &e.kind else { continue };
            let entre = a.expr(*operand);
            let ExprKind::Parenthesized(dentro) = &entre.kind else { continue };
            let teste = a.expr(*dentro);
            let ExprKind::Is { value, negated: false, .. } = &teste.kind else { continue };
            let no_teste = contem(teste.span);
            let no_prefixo = offset == e.span.start || offset == entre.span.start || offset == entre.span.end;
            if !(no_teste || no_prefixo) {
                continue;
            }
            // O `is` fica depois do valor: a palavra, e o ponto logo depois dela.
            let depois_do_valor = a.expr(*value).span.end;
            let Some(k) = fonte.get(depois_do_valor..teste.span.end).and_then(|t| t.find("is")) else { continue };
            let fim_do_is = depois_do_valor + k + 2;
            let mut edicoes: Vec<(Span, String)> = Vec::new();
            if pai_relacional_ou_mais(a, ExprId(i as u32)) {
                // Os parênteses ficam: sai só o `!`.
                edicoes.push((Span { start: e.span.start, end: e.span.start + 1 }, String::new()));
            } else {
                // Saem o `!(` e o `)`.
                edicoes.push((Span { start: e.span.start, end: entre.span.start + 1 }, String::new()));
                edicoes.push((Span { start: entre.span.end.saturating_sub(1), end: e.span.end }, String::new()));
            }
            edicoes.push((Span { start: fim_do_is, end: fim_do_is }, "!".to_string()));
            saida.push(acao(uri, "Convert to is!", "refactor.convert.isNot", edicoes));
            break;
        }

        // `Exchange operands`: a expressão binária com o cursor no operador.
        let selecionada = a.exprs.iter().enumerate().find_map(|(i, e)| match &e.kind {
            ExprKind::Binary { op, left, right } => {
                let (de, ate) = (a.expr(*left).span.end, a.expr(*right).span.start);
                let texto = fonte.get(de..ate)?;
                let inicio = de + (texto.len() - texto.trim_start().len());
                let fim = de + texto.trim_end().len();
                (inicio <= offset && offset <= fim).then_some((ExprId(i as u32), *op, *left, *right, Span { start: inicio, end: fim }))
            }
            _ => None,
        });
        if let Some((id, op, esquerda, direita, operador)) = selecionada {
            // Alarga pelos pais de mesmo operador.
            let mut larga = id;
            loop {
                let pai = a.exprs.iter().position(|e| matches!(&e.kind, ExprKind::Binary { op: o, left, right } if *o == op && (*left == larga || *right == larga)));
                match pai {
                    Some(p) => larga = ExprId(p as u32),
                    None => break,
                }
            }
            let larga = a.expr(larga).span;
            let (fim_da_esquerda, inicio_da_direita) = (a.expr(esquerda).span.end, a.expr(direita).span.start);
            if let (Some(texto_esquerdo), Some(texto_direito)) = (fonte.get(larga.start..fim_da_esquerda), fonte.get(inicio_da_direita..larga.end)) {
                let mut edicoes = vec![
                    (Span { start: larga.start, end: fim_da_esquerda }, texto_direito.to_string()),
                    (Span { start: inicio_da_direita, end: larga.end }, texto_esquerdo.to_string()),
                ];
                // Um operador relacional troca de lado.
                let invertido = match op {
                    BinaryOp::Lt => Some(">"),
                    BinaryOp::LtEq => Some(">="),
                    BinaryOp::Gt => Some("<"),
                    BinaryOp::GtEq => Some("<="),
                    _ => None,
                };
                if let Some(novo) = invertido {
                    edicoes.push((operador, novo.to_string()));
                }
                edicoes.sort_by_key(|(s, _)| s.start);
                saida.push(acao(uri, "Exchange operands", "refactor.exchangeOperands", edicoes));
            }
        }
        saida
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Os exemplos da especificação (§13.8.2).
    #[test]
    fn separadores_de_digitos() {
        assert_eq!(com_separadores("1234567", false).as_deref(), Some("1_234_567"));
        assert_eq!(com_separadores("1234", false), None);
        assert_eq!(com_separadores("12_34567", false).as_deref(), Some("1_234_567"));
        assert_eq!(com_separadores("0xFFFFFF", false).as_deref(), Some("0xFF_FF_FF"));
        assert_eq!(com_separadores("0xFFF", false), None);
        assert_eq!(com_separadores("12345.67891", true).as_deref(), Some("12_345.678_91"));
        assert_eq!(com_separadores("1_000_000", false), None);
    }
}
