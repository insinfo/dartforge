//! As regras de lint que pedem os tipos estáticos da inferência
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8). Devolvem achados neutros (a
//! posição, o nome único do código e os argumentos): quem chama os guarda
//! e só os emite com a regra ligada.
//!
//! Aqui: `prefer_is_empty` (`linter/lib/src/rules/prefer_is_empty.dart`,
//! lido no `main` do SDK; a conferir contra a 3.6.2), com os quatro
//! códigos. O contexto constante é o `inConstantContext` do analyzer
//! (`dartforge_frontend::pais`), e o inicializador de construtor `const` fica
//! de fora como no `_check` do original.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::table::{CoreTypes, Type, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, ExprId, ExprKind, Initializer, MemberKind, UnaryOp};

/// Um achado: a posição, o nome único do código de lint e os argumentos.
pub type Achado = (Span, &'static str, Vec<String>);

/// `getIntValue`: o valor de um literal inteiro, com o `-` na frente.
fn valor_inteiro(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    let literal = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Int(s) => {
            let texto: String = fonte.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
            match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
                // O hexadecimal vai até 64 bits (`0xFFFFFFFFFFFFFFFF` é -1).
                Some(hex) => u64::from_str_radix(hex, 16).ok().map(|v| v as i64),
                None => texto.parse::<i64>().ok(),
            }
        }
        _ => None,
    };
    match &a.expr(e).kind {
        ExprKind::Unary { op: UnaryOp::Neg, operand } => literal(*operand).map(|v| -v),
        _ => literal(e),
    }
}

/// Os pais das expressões da unidade `u` (`dartforge_frontend::pais`), com
/// o `case` antigo nas bibliotecas de antes da 3.0.
pub(crate) fn pais_da_unidade(program: &Program, u: UnitId) -> dartforge_frontend::pais::Pais {
    let unidade = program.unit(u);
    let versao = program.library(unidade.library).features.versao();
    let antes_de_3 = versao < dartforge_frontend::features::LanguageVersion::new(3, 0);
    dartforge_frontend::pais::Pais::novo(&unidade.ast, &unidade.unit, &unidade.source, antes_de_3)
}

/// Os achados das regras tipadas na unidade `u`.
pub fn achados(program: &Program, table: &TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();

    // `_check`: dentro de um inicializador de construtor `const`, nada.
    let mut de_const: Vec<Span> = Vec::new();
    for m in a.members.iter() {
        if let MemberKind::Constructor(k) = &m.kind
            && k.const_
        {
            de_const.extend(k.initializers.iter().map(|i| match i {
                Initializer::Field { span, .. } | Initializer::Super { span, .. } | Initializer::Redirect { span, .. } | Initializer::Assert { span, .. } => *span,
            }));
        }
    }
    // `inConstantContext`.
    let pais = pais_da_unidade(program, u);
    let em_constante = |e: ExprId| pais.em_contexto_constante(a, e) || de_const.iter().any(|k| k.start <= a.expr(e).span.start && a.expr(e).span.end <= k.end);

    // `_isLengthAccess`: `x.length` com `x` um `Iterable`, um `Map` ou uma
    // `String` (através de parênteses e de `as`).
    let de_comprimento = |mut e: ExprId| {
        loop {
            match &a.expr(e).kind {
                ExprKind::Parenthesized(x) => e = *x,
                ExprKind::As { value, .. } => e = *value,
                _ => break,
            }
        }
        let ExprKind::Property { target, name, .. } = &a.expr(e).kind else { return false };
        if fonte.get(name.span.start..name.span.end) != Some("length") {
            return false;
        }
        let Some(tipo) = corpo.get_type(*target) else { return false };
        let Type::Interface { class, .. } = table.get(tipo) else { return false };
        let implementa = |alvo: Option<dartforge_elements::model::ClassId>| {
            alvo.is_some_and(|k| *class == k || outline.hierarchy.get(*class).is_some_and(|d| d.supertypes.contains_key(&k)))
        };
        Some(*class) == core.string_class || implementa(core.iterable_class) || implementa(core.map_class)
    };

    for (k, e) in a.exprs.iter().enumerate() {
        let ExprKind::Binary { op, left, right } = &e.kind else { continue };
        // A constante de um lado e o `length` do outro.
        let (valor, a_direita) = match valor_inteiro(a, fonte, *right) {
            Some(v) if de_comprimento(*left) => (v, true),
            Some(_) => continue,
            None => match valor_inteiro(a, fonte, *left) {
                Some(v) if de_comprimento(*right) => (v, false),
                _ => continue,
            },
        };
        if em_constante(ExprId(k as u32)) {
            continue;
        }
        let (vazio, nao_vazio, falso, verdadeiro) = (
            "prefer_is_empty_use_is_empty",
            "prefer_is_empty_use_is_not_empty",
            "prefer_is_empty_always_false",
            "prefer_is_empty_always_true",
        );
        let codigo = if valor == 0 {
            match op {
                BinaryOp::Eq | BinaryOp::LtEq => Some(vazio),
                BinaryOp::Gt | BinaryOp::NotEq => Some(nao_vazio),
                BinaryOp::Lt => Some(falso),
                BinaryOp::GtEq => Some(verdadeiro),
                _ => None,
            }
        } else if valor == 1 {
            match (a_direita, op) {
                (true, BinaryOp::GtEq) | (false, BinaryOp::LtEq) => Some(nao_vazio),
                (true, BinaryOp::Lt) | (false, BinaryOp::Gt) => Some(vazio),
                _ => None,
            }
        } else if valor < 0 {
            match (a_direita, op) {
                (true, BinaryOp::Eq | BinaryOp::LtEq | BinaryOp::Lt) | (false, BinaryOp::Eq | BinaryOp::GtEq | BinaryOp::Gt) => Some(falso),
                (true, BinaryOp::NotEq | BinaryOp::GtEq | BinaryOp::Gt) | (false, BinaryOp::NotEq | BinaryOp::LtEq | BinaryOp::Lt) => Some(verdadeiro),
                _ => None,
            }
        } else {
            None
        };
        if let Some(c) = codigo {
            out.push((e.span, c, Vec::new()));
        }
    }
    out
}
