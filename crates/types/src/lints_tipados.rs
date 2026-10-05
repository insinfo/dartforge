//! As regras de lint que pedem os tipos estáticos da inferência
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8). Devolvem achados neutros (a
//! posição, o nome único do código e os argumentos): quem chama os guarda
//! e só os emite com a regra ligada.
//!
//! Aqui: `prefer_is_empty` (`linter/lib/src/rules/prefer_is_empty.dart`,
//! lido no `main` do SDK; a conferir contra a 3.6.2), com os quatro
//! códigos. O "contexto constante" do original é aproximado: a expressão
//! dentro de uma lista de variáveis `const`, de uma anotação, ou de uma
//! coleção ou criação `const`.
//! Escrito sem compilar nem executar (2026-10-05).

use crate::resolve::OutlineTypes;
use crate::resolved::UnitBodyTypes;
use crate::table::{CoreTypes, Type, TypeTable};
use dartforge_diagnostics::Span;
use dartforge_elements::model::{Program, UnitId};
use dartforge_frontend::ast::{self, BinaryOp, CreationKeyword, DeclKind, ExprId, ExprKind, MemberKind, StmtKind, UnaryOp};

/// Um achado: a posição, o nome único do código de lint e os argumentos.
pub type Achado = (Span, &'static str, Vec<String>);

/// `getIntValue`: o valor de um literal inteiro, com o `-` na frente.
fn valor_inteiro(a: &ast::Ast, fonte: &str, e: ExprId) -> Option<i64> {
    let literal = |x: ExprId| match &a.expr(x).kind {
        ExprKind::Int(s) => {
            let texto: String = fonte.get(s.start..s.end)?.chars().filter(|c| *c != '_').collect();
            match texto.strip_prefix("0x").or_else(|| texto.strip_prefix("0X")) {
                Some(hex) => i64::from_str_radix(hex, 16).ok(),
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

/// Os achados das regras tipadas na unidade `u`.
pub fn achados(program: &Program, table: &TypeTable, core: &CoreTypes, outline: &OutlineTypes, corpo: &UnitBodyTypes, u: UnitId) -> Vec<Achado> {
    let mut out: Vec<Achado> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    let fonte = unidade.source.as_str();

    // Os trechos em contexto constante.
    let mut constantes: Vec<Span> = Vec::new();
    let mut de_lista = |l: &ast::VariableList| {
        if l.const_ {
            constantes.extend(l.variables.iter().filter_map(|v| v.initializer).map(|e| a.expr(e).span));
        }
    };
    for d in a.decls.iter() {
        if let DeclKind::Variables(l) = &d.kind {
            de_lista(l);
        }
    }
    for m in a.members.iter() {
        if let MemberKind::Field(l) = &m.kind {
            de_lista(l);
        }
    }
    for s in a.stmts.iter() {
        if let StmtKind::Variables(l) = &s.kind {
            de_lista(l);
        }
    }
    constantes.extend(a.decls.iter().flat_map(|d| d.metadata.iter()).chain(a.members.iter().flat_map(|m| m.metadata.iter())).map(|m| m.span));
    for e in a.exprs.iter() {
        let constante = match &e.kind {
            ExprKind::List { const_, .. } | ExprKind::SetOrMap { const_, .. } | ExprKind::Record { const_, .. } => *const_,
            ExprKind::InstanceCreation { keyword, .. } => *keyword == Some(CreationKeyword::Const),
            _ => false,
        };
        if constante {
            constantes.push(e.span);
        }
    }
    let em_constante = |s: Span| constantes.iter().any(|k| k.start <= s.start && s.end <= k.end);

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

    for e in a.exprs.iter() {
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
        if em_constante(e.span) {
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
