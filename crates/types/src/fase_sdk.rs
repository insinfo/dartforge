//! O `SdkConstraintVerifier` do analyzer
//! (`analyzer/lib/src/hint/sdk_constraint_verifier.dart`, lido por inteiro
//! na 6.11.0; docs/ANALYZER-ESPECIFICACAO-INFRA.md §6.4 e lote II.8):
//!
//! * `sdk_version_gt_gt_gt_operator`: o operador `>>>`, usado ou declarado,
//!   quando a restrição de SDK do `pubspec.yaml` admite versão anterior à
//!   2.14.0;
//! * `sdk_version_since`: a referência a um elemento do SDK anotado com
//!   `@Since('x.y')` (nele ou na classe que o contém) que a restrição não
//!   garante.
//!
//! Fora: os argumentos posicionais passados a parâmetros com `@Since`, os
//! operadores de índice e de atribuição, a invocação de expressão de
//! função, e a propriedade `index` (o original só a relata quando o alvo é
//! exatamente `Enum`). A restrição em união (`||`) não é lida.
//! Escrito sem compilar nem executar (2026-10-04).

use crate::resolved::{MemberRef, Resolved, UnitBodyTypes};
use dartforge_diagnostics::codigos::warning as w;
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::model::{ClassId, Element, FunctionElementId, FunctionRef, Program, UnitId, VariableId, VariableRef};
use dartforge_frontend::ast::{self, BinaryOp, DeclKind, ExprKind, FunctionKind, MemberKind, TypeKind};
use dartforge_intern::Interner;

/// Uma versão `maior.menor.correção`, sem pré-lançamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Versao(pub u32, pub u32, pub u32);

impl Versao {
    /// `2.15`, `2.15.0`, `3.0.0-dev.1`: o que falta é zero; o que vem depois
    /// de `-` ou `+` é descartado.
    pub fn de_texto(texto: &str) -> Option<Versao> {
        let nucleo = texto.trim().split(['-', '+']).next()?;
        let mut partes = nucleo.split('.').map(|p| p.parse::<u32>());
        let maior = partes.next()?.ok()?;
        let menor = partes.next().unwrap_or(Ok(0)).ok()?;
        let correcao = partes.next().unwrap_or(Ok(0)).ok()?;
        Some(Versao(maior, menor, correcao))
    }
}

impl std::fmt::Display for Versao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// A restrição `environment: sdk:` do `pubspec.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestricaoDeSdk {
    /// O mínimo (de `^v`, `>=v`, `>v` ou da versão exata); `None` sem piso.
    pub minimo: Option<Versao>,
    /// O texto da restrição, com os espaços normalizados (o `toString`).
    pub texto: String,
}

impl RestricaoDeSdk {
    /// `^3.6.0`, `>=2.12.0 <4.0.0`, `any`, `3.6.0`. `None` para o que não se
    /// lê (uniões com `||`, texto inválido): o verificador não roda.
    pub fn de_texto(texto: &str) -> Option<RestricaoDeSdk> {
        let normal = texto.split_whitespace().collect::<Vec<_>>().join(" ");
        if normal.contains("||") {
            return None;
        }
        if normal.is_empty() || normal == "any" {
            return Some(RestricaoDeSdk { minimo: None, texto: "any".to_string() });
        }
        let mut minimo = None;
        for parte in normal.split(' ') {
            if let Some(v) = parte.strip_prefix('^').or_else(|| parte.strip_prefix(">=")).or_else(|| parte.strip_prefix('>')) {
                minimo = Some(Versao::de_texto(v)?);
            } else if let Some(v) = parte.strip_prefix("<=").or_else(|| parte.strip_prefix('<')) {
                Versao::de_texto(v)?;
            } else {
                minimo = Some(Versao::de_texto(parte)?);
            }
        }
        Some(RestricaoDeSdk { minimo, texto: normal })
    }

    /// `requiresAtLeast`: o mínimo da restrição é pelo menos `versao`.
    fn garante(&self, versao: Versao) -> bool {
        self.minimo.is_some_and(|m| m >= versao)
    }

    /// `checkTripleShift`: a restrição admite versão anterior à 2.14.0.
    fn antes_do_deslocamento_triplo(&self) -> bool {
        self.minimo.is_none_or(|m| m < Versao(2, 14, 0))
    }
}

/// A versão do `@Since('x.y')` de uma lista de anotações.
fn desde(metadata: &[ast::Annotation], a: &ast::Ast, interner: &Interner) -> Option<Versao> {
    metadata.iter().find_map(|m| {
        let args = m.arguments.as_ref()?;
        if m.name.last().map(|n| interner.resolve(n.sym)) != Some("Since") {
            return None;
        }
        let primeiro = args.args.iter().find(|x| x.name.is_none())?;
        match &a.expr(primeiro.value).kind {
            ExprKind::String(lit) => Versao::de_texto(&dartforge_elements::load::string_lit_value(lit)?),
            _ => None,
        }
    })
}

fn desde_da_classe(program: &Program, interner: &Interner, c: ClassId) -> Option<Versao> {
    let e = program.class(c);
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let d = e.decl?;
    let a = &program.unit(d.unit).ast;
    desde(&a.decl(d.decl).metadata, a, interner)
}

fn desde_da_variavel(program: &Program, interner: &Interner, v: VariableId) -> Option<Versao> {
    let e = program.variable(v);
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let propria = match e.node {
        VariableRef::TopLevel { unit, decl, .. } => {
            let a = &program.unit(unit).ast;
            desde(&a.decl(decl).metadata, a, interner)
        }
        VariableRef::Field { unit, member, .. } => {
            let a = &program.unit(unit).ast;
            desde(&a.member(member).metadata, a, interner)
        }
        _ => None,
    };
    // `sinceSdkVersion`: o maior entre o do elemento e o de quem o contém.
    propria.max(e.class.and_then(|c| desde_da_classe(program, interner, c)))
}

fn desde_da_funcao(program: &Program, interner: &Interner, f: FunctionElementId) -> Option<Versao> {
    let e = program.function(f);
    if let Some(v) = e.variable {
        return desde_da_variavel(program, interner, v);
    }
    if !program.library(e.library).uri.starts_with("dart:") {
        return None;
    }
    let propria = match e.node {
        FunctionRef::Constructor { unit, member } => {
            let a = &program.unit(unit).ast;
            desde(&a.member(member).metadata, a, interner)
        }
        FunctionRef::Function { unit, function } => {
            let a = &program.unit(unit).ast;
            let do_membro = a.members.iter().find(|m| matches!(&m.kind, MemberKind::Method(g) if *g == function)).map(|m| &m.metadata[..]);
            let da_declaracao = || a.decls.iter().find(|d| matches!(&d.kind, DeclKind::Function(g) if *g == function)).map(|d| &d.metadata[..]);
            do_membro.or_else(da_declaracao).and_then(|m| desde(m, a, interner))
        }
        FunctionRef::None => None,
    };
    propria.max(e.class.and_then(|c| desde_da_classe(program, interner, c)))
}

/// Os relatos do `SdkConstraintVerifier` na unidade `u`.
pub fn restricao_de_sdk(
    program: &Program,
    interner: &Interner,
    corpo: Option<&UnitBodyTypes>,
    u: UnitId,
    restricao: &RestricaoDeSdk,
) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let unidade = program.unit(u);
    let a = &unidade.ast;
    // `>>>`: o uso e a declaração do operador.
    if restricao.antes_do_deslocamento_triplo() {
        for e in a.exprs.iter() {
            if let ExprKind::Binary { op: BinaryOp::UShr, left, right } = &e.kind {
                let (de, ate) = (a.expr(*left).span.end, a.expr(*right).span.start);
                if let Some(k) = unidade.source.get(de..ate).and_then(|t| t.find(">>>")) {
                    out.push(Diagnostic::com_codigo(w::SDK_VERSION_GT_GT_GT_OPERATOR, Span { start: de + k, end: de + k + 3 }, std::iter::empty::<&str>()));
                }
            }
        }
        for f in a.functions.iter() {
            if let (FunctionKind::Operator, Some(n)) = (f.kind, f.name)
                && interner.resolve(n.sym) == ">>>"
            {
                out.push(Diagnostic::com_codigo(w::SDK_VERSION_GT_GT_GT_OPERATOR, n.span, std::iter::empty::<&str>()));
            }
        }
    }
    // `_checkSinceSdkVersion`.
    let mut relatar = |versao: Option<Versao>, span: Span| {
        let Some(v) = versao else { return };
        if restricao.garante(v) {
            return;
        }
        // O ouvinte é um conjunto: um relato por posição.
        if !out.iter().any(|x| x.code == Some(w::SDK_VERSION_SINCE) && x.span == span) {
            let texto = v.to_string();
            out.push(Diagnostic::com_codigo(w::SDK_VERSION_SINCE, span, [texto.as_str(), restricao.texto.as_str()]));
        }
    };
    // Tipos nomeados: a classe do SDK, no nome.
    for t in a.types.iter() {
        let TypeKind::Named { name, .. } = &t.kind else { continue };
        let ligacao = match &name[..] {
            [n] => program.lookup_na_unidade(u, n.sym),
            [p, n] => program.lookup_prefixed_na_unidade(u, p.sym, n.sym),
            _ => None,
        };
        if let (Some(Element::Class(c)), Some(ultimo)) = (ligacao.and_then(|b| b.getter), name.last()) {
            relatar(desde_da_classe(program, interner, c), ultimo.span);
        }
    }
    if let Some(corpo) = corpo {
        for (i, expr) in a.exprs.iter().enumerate() {
            let Some(Some(r)) = corpo.resolved.get(i) else { continue };
            let no_nome = match &expr.kind {
                ExprKind::Identifier(n) => n,
                ExprKind::Property { name, .. } => name,
                _ => continue,
            };
            let versao = match r {
                Resolved::Constructor(f) => desde_da_funcao(program, interner, *f),
                Resolved::Element(Element::Class(c)) => desde_da_classe(program, interner, *c),
                Resolved::Element(Element::Function(f)) => desde_da_funcao(program, interner, *f),
                Resolved::Element(Element::Variable(v)) => desde_da_variavel(program, interner, *v),
                // `_shouldReportEnumIndex`: `index` só com alvo exatamente
                // `Enum`, o que aqui não se confere.
                Resolved::Member { .. } | Resolved::ExtensionMember { .. } if interner.resolve(no_nome.sym) == "index" => None,
                Resolved::Member { member: MemberRef::Function(f), .. } | Resolved::ExtensionMember { member: f, .. } => {
                    desde_da_funcao(program, interner, *f)
                }
                Resolved::Member { member: MemberRef::Variable(v), .. } => desde_da_variavel(program, interner, *v),
                _ => None,
            };
            relatar(versao, no_nome.span);
        }
    }
    out
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn restricoes() {
        let caret = RestricaoDeSdk::de_texto("^3.6.0").unwrap();
        assert_eq!(caret.minimo, Some(Versao(3, 6, 0)));
        assert!(caret.garante(Versao(2, 15, 0)) && !caret.garante(Versao(3, 7, 0)));
        assert!(!caret.antes_do_deslocamento_triplo());
        let faixa = RestricaoDeSdk::de_texto(">=2.12.0   <4.0.0").unwrap();
        assert_eq!(faixa.texto, ">=2.12.0 <4.0.0");
        assert!(faixa.antes_do_deslocamento_triplo() && !faixa.garante(Versao(2, 15, 0)));
        assert!(RestricaoDeSdk::de_texto("any").unwrap().antes_do_deslocamento_triplo());
        assert!(RestricaoDeSdk::de_texto(">=2.0.0 <3.0.0 || >=3.1.0").is_none());
        assert_eq!(Versao::de_texto("2.15"), Some(Versao(2, 15, 0)));
        assert_eq!(Versao::de_texto("3.0.0-dev.1"), Some(Versao(3, 0, 0)));
    }
}
