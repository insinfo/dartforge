//! Relevância dos itens do completar como o servidor do Dart 3.6.2 a calcula
//! (`services/completion/dart/relevance_computer.dart` e
//! `feature_computer.dart`): uma média ponderada de características,
//! levada a `0..1000`, e `sortText = 9999 − relevância`.
//!
//! * **tipo de contexto** — o tipo que a posição espera (parâmetro do
//!   argumento, alvo da atribuição, tipo escrito da variável, retorno da
//!   função, condição `bool`) contra o tipo do item: igual 1,0; subtipo 0,40;
//!   supertipo 0,02; sem relação 0,13; sem um dos dois, 0;
//! * **espécie do elemento** — a faixa de probabilidade da espécie (classe,
//!   local, parâmetro, campo, método, função…) no local do completar
//!   (`Block_statement`, `ArgumentList_method_unnamed`,
//!   `PropertyAccess_propertyName`…), das tabelas geradas do Dart
//!   (`relevance_tables.rs`); o meio da faixa, ou, com distância
//!   (locais pela proximidade, membros pela herança), `meio + (alto −
//!   baixo) × distância / 2`;
//! * **palavra-chave** — o alto da faixa da palavra no local;
//! * **não importado** −1, **começa com `$`** −1 (peso 0,5), **noSuchMethod**
//!   −1, **distância do local** `0,9^d`.
//!
//! `relevância = ⌊((Σ vᵢ·pᵢ / Σ pᵢ) + 1) / 2 × 1000⌋`, com os pesos do Dart
//! (todos 1, menos depreciado e `$`, 0,5). Argumentos nomeados têm valor
//! fixo (900; 950 se obrigatório), como no Dart.

use dartforge_frontend::ast::{self, ExprId};

/// Espécie de elemento das tabelas do Dart (`computeElementKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Class,
    Constructor,
    Enum,
    Field,
    Function,
    FunctionAlias,
    Local,
    Method,
    Mixin,
    Parameter,
    Prefix,
    TopLevelVariable,
    TypeParameter,
    Unknown,
    /// Espécies sem tabela (constante de enum, extensão, alias de tipo):
    /// característica de espécie 0.
    NoTable,
}

/// O que se sabe de um item para pontuá-lo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Rel {
    /// Espécie, quando o item é um elemento.
    pub kind: Option<Kind>,
    /// O tipo do item (variável, retorno, getter), para o tipo de contexto.
    pub ty: Option<dartforge_types::TypeId>,
    /// Distância (`d`, que vira `0,9^d`): local pela proximidade, membro
    /// pela herança.
    pub distance: Option<u32>,
    /// Local ou parâmetro: a distância entra também como característica.
    pub local: bool,
    pub not_imported: bool,
    /// Palavra-chave (o texto).
    pub word: Option<&'static str>,
    /// Relevância fixa (argumentos nomeados).
    pub fixed: Option<i32>,
    pub starts_with_dollar: bool,
    pub no_such_method: bool,
    /// `hasOrInheritsDeprecated` (`hasDeprecated`, peso 0,5).
    pub deprecated: bool,
    /// `isConstantFeature`: construtor `const`, campo estático `const`,
    /// variável de topo `const` (só vale com `preferConstants`).
    pub constant: bool,
    /// `superMatches`: o nome é o do método que contém o `super.▮`.
    pub super_matches: bool,
}

/// `distanceToPercent`.
pub(crate) fn percentage(distance: usize) -> f64 {
    0.9f64.powi(distance as i32)
}

/// A característica de espécie no `local`.
fn kind_feature(kind: Kind, local: Option<&str>, distance: Option<f64>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let Some((_, low, high)) = super::relevance_tables::kinds(local).iter().find(|(e, _, _)| *e == kind) else {
        return 0.0;
    };
    let middle = (low + high) / 2.0;
    match distance {
        None => middle,
        Some(d) => middle + ((high - low) * d / 2.0),
    }
}

/// A característica de palavra-chave no `local` (o alto da faixa).
fn word_feature(word: &str, local: Option<&str>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let table = super::relevance_tables::words(local);
    let find = |p: &str| table.iter().find(|(k, _, _)| *k == p).map(|(_, _, high)| *high);
    find(word)
        .or_else(|| word.find(|c: char| !c.is_ascii_lowercase()).filter(|i| *i > 0).and_then(|i| find(&word[..i])))
        .unwrap_or(0.0)
}

/// A relevância (`0..1000`) de um item.
pub(crate) fn relevance(rel: &Rel, local: Option<&str>, context_type: f64, prefer_constants: bool) -> i32 {
    if let Some(f) = rel.fixed {
        return f;
    }
    let distance = rel.distance.map(|d| percentage(d as usize));
    // `_computeFormalParameterRelevance`: a espécie do parâmetro é sem a
    // distância (só o local a usa, `_computeLocalVariableRelevance`).
    let kind = rel.kind.map_or(0.0, |e| kind_feature(e, local, if e == Kind::Parameter { None } else { distance }));
    let word = rel.word.map_or(0.0, |p| word_feature(p, local));
    let not_imported = if rel.not_imported { -1.0 } else { 0.0 };
    let dollar = if rel.starts_with_dollar { -1.0 } else { 0.0 };
    let nsm = if rel.no_such_method { -1.0 } else { 0.0 };
    let local_distance = if rel.local { distance.unwrap_or(0.0) } else { 0.0 };
    let deprecated = if rel.deprecated { -1.0 } else { 0.0 };
    let constant = if prefer_constants && rel.constant { 1.0 } else { 0.0 };
    let super_matches = if rel.super_matches { 1.0 } else { 0.0 };
    // Pesos do Dart: contexto 1, espécie 1, depreciado 0,5, constante 1,
    // noSuchMethod 1, não importado 1, palavra 1, `$` 0,5, super 1,
    // distância do local 1 — total 9.
    let sum = context_type + kind + 0.5 * deprecated + constant + nsm + not_imported + word + 0.5 * dollar + super_matches + local_distance;
    let media = sum / 9.0;
    (((media + 1.0) / 2.0) * 1000.0) as i32
}

fn elements_contains(elements: &[ast::CollectionElement], target: ExprId) -> bool {
    elements.iter().any(|e| match e {
        ast::CollectionElement::Expression(x) | ast::CollectionElement::NullAwareExpression(x) => *x == target,
        ast::CollectionElement::MapEntry { key, value, .. } => *key == target || *value == target,
        _ => false,
    })
}

fn elements_value(elements: &[ast::CollectionElement], target: ExprId) -> bool {
    elements.iter().any(|e| matches!(e, ast::CollectionElement::MapEntry { value, .. } if *value == target))
}
