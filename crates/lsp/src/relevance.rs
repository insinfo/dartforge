//! Relevance of completion items as the Dart 3.6.2 server computes it
//! (`services/completion/dart/relevance_computer.dart` and
//! `feature_computer.dart`): a weighted mean of features, scaled to
//! `0..1000`, and `sortText = 9999 − relevance`.
//!
//! * **context type** — the type the position expects (the argument's
//!   parameter, the assignment target, the variable's written type, the
//!   function's return, a `bool` condition) against the item's type: equal
//!   1.0; subtype 0.40; supertype 0.02; unrelated 0.13; missing either, 0;
//! * **element kind** — the probability range of the kind (class, local,
//!   parameter, field, method, function…) at the completion location
//!   (`Block_statement`, `ArgumentList_method_unnamed`,
//!   `PropertyAccess_propertyName`…), from Dart's generated tables
//!   (`relevance_tables.rs`); the middle of the range, or, with a distance
//!   (locals by proximity, members by inheritance), `middle + (high − low) ×
//!   distance / 2`;
//! * **keyword** — the top of the keyword's range at the location;
//! * **not imported** −1, **starts with `$`** −1 (weight 0.5),
//!   **noSuchMethod** −1, **local distance** `0.9^d`.
//!
//! `relevance = ⌊((Σ vᵢ·pᵢ / Σ pᵢ) + 1) / 2 × 1000⌋`, with Dart's weights
//! (all 1, except deprecated and `$`, 0.5). Named arguments have a fixed
//! value (900; 950 if required), as in Dart.

use dartforge_frontend::ast::{self, ExprId};

/// Element kind of the Dart tables (`computeElementKind`).
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
    /// Kinds without a table (enum constant, extension, type alias): kind
    /// feature 0.
    NoTable,
}

/// What is known about an item in order to score it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Rel {
    /// Kind, when the item is an element.
    pub kind: Option<Kind>,
    /// The item's type (variable, return, getter), for the context type.
    pub ty: Option<dartforge_types::TypeId>,
    /// Distance (`d`, which becomes `0.9^d`): a local by proximity, a member by
    /// inheritance.
    pub distance: Option<u32>,
    /// Local or parameter: the distance also enters as a feature.
    pub local: bool,
    pub not_imported: bool,
    /// Keyword (the text).
    pub word: Option<&'static str>,
    /// Fixed relevance (named arguments).
    pub fixed: Option<i32>,
    pub starts_with_dollar: bool,
    pub no_such_method: bool,
    /// `hasOrInheritsDeprecated` (`hasDeprecated`, weight 0.5).
    pub deprecated: bool,
    /// `isConstantFeature`: `const` constructor, `static const` field, `const`
    /// top-level variable (only counts with `preferConstants`).
    pub constant: bool,
    /// `superMatches`: the name is that of the method containing the `super.▮`.
    pub super_matches: bool,
}

/// `distanceToPercent`.
pub(crate) fn percentage(distance: usize) -> f64 {
    0.9f64.powi(distance as i32)
}

/// The kind feature at `local`.
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

/// The keyword feature at `local` (the top of the range).
fn word_feature(word: &str, local: Option<&str>) -> f64 {
    let Some(local) = local else { return 0.0 };
    let table = super::relevance_tables::words(local);
    let find = |p: &str| table.iter().find(|(k, _, _)| *k == p).map(|(_, _, high)| *high);
    find(word)
        .or_else(|| word.find(|c: char| !c.is_ascii_lowercase()).filter(|i| *i > 0).and_then(|i| find(&word[..i])))
        .unwrap_or(0.0)
}

/// The relevance (`0..1000`) of an item.
pub(crate) fn relevance(rel: &Rel, local: Option<&str>, context_type: f64, prefer_constants: bool) -> i32 {
    if let Some(f) = rel.fixed {
        return f;
    }
    let distance = rel.distance.map(|d| percentage(d as usize));
    // `_computeFormalParameterRelevance`: the parameter's kind is without the
    // distance (only the local uses it, `_computeLocalVariableRelevance`).
    let kind = rel.kind.map_or(0.0, |e| kind_feature(e, local, if e == Kind::Parameter { None } else { distance }));
    let word = rel.word.map_or(0.0, |p| word_feature(p, local));
    let not_imported = if rel.not_imported { -1.0 } else { 0.0 };
    let dollar = if rel.starts_with_dollar { -1.0 } else { 0.0 };
    let nsm = if rel.no_such_method { -1.0 } else { 0.0 };
    let local_distance = if rel.local { distance.unwrap_or(0.0) } else { 0.0 };
    let deprecated = if rel.deprecated { -1.0 } else { 0.0 };
    let constant = if prefer_constants && rel.constant { 1.0 } else { 0.0 };
    let super_matches = if rel.super_matches { 1.0 } else { 0.0 };
    // Dart's weights: context 1, kind 1, deprecated 0.5, constant 1,
    // noSuchMethod 1, not imported 1, keyword 1, `$` 0.5, super 1, local
    // distance 1 — total 9.
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
