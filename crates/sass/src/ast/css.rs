use codemap::Span;

use crate::selector::ExtendedSelector;

use super::{MediaRule, Style, UnknownAtRule};

#[derive(Debug, Clone)]
pub(crate) enum CssStmt {
    RuleSet {
        selector: ExtendedSelector,
        body: Vec<Self>,
        is_group_end: bool,
        /// O `span` do seletor (`CssStyleRule.selector.span`, que o mapa de
        /// fontes usa) e o da regra inteira (`CssStyleRule.span`).
        selector_span: Span,
        span: Span,
    },
    Style(Style),
    Media(MediaRule, bool),
    UnknownAtRule(UnknownAtRule, bool),
    Supports(SupportsRule, bool),
    Comment(String, Span),
    KeyframesRuleSet(KeyframesRuleSet),
    /// A plain import such as `@import "foo.css";` or
    /// `@import url(https://fonts.google.com/foo?bar);`
    // todo: named fields, 0: url, 1: modifiers
    Import(String, Option<String>, Span),
}

impl CssStmt {
    pub fn is_style_rule(&self) -> bool {
        matches!(self, CssStmt::RuleSet { .. })
    }

    pub fn set_group_end(&mut self) {
        match self {
            CssStmt::Media(_, is_group_end)
            | CssStmt::UnknownAtRule(_, is_group_end)
            | CssStmt::Supports(_, is_group_end)
            | CssStmt::RuleSet { is_group_end, .. } => *is_group_end = true,
            CssStmt::Style(_)
            | CssStmt::Comment(_, _)
            | CssStmt::KeyframesRuleSet(_)
            | CssStmt::Import(..) => {}
        }
    }

    /// O `span` do nó CSS no dart-sass (`CssNode.span`): a regra ou a
    /// declaração inteira na fonte.
    pub fn span(&self) -> Span {
        match self {
            CssStmt::RuleSet { span, .. } => *span,
            CssStmt::Style(s) => s.span,
            CssStmt::Media(m, _) => m.span,
            CssStmt::UnknownAtRule(u, _) => u.span,
            CssStmt::Supports(s, _) => s.span,
            CssStmt::Comment(_, span) => *span,
            CssStmt::KeyframesRuleSet(k) => k.span,
            CssStmt::Import(_, _, span) => *span,
        }
    }

    pub fn is_group_end(&self) -> bool {
        match self {
            CssStmt::Media(_, is_group_end)
            | CssStmt::UnknownAtRule(_, is_group_end)
            | CssStmt::Supports(_, is_group_end)
            | CssStmt::RuleSet { is_group_end, .. } => *is_group_end,
            _ => false,
        }
    }

    /// `equalsIgnoringChildren` do dart-sass.
    pub fn equals_ignoring_children(&self, other: &Self) -> bool {
        match (self, other) {
            (CssStmt::RuleSet { selector: a, .. }, CssStmt::RuleSet { selector: b, .. }) => a == b,
            (CssStmt::Media(a, _), CssStmt::Media(b, _)) => a.query == b.query,
            (CssStmt::Supports(a, _), CssStmt::Supports(b, _)) => a.params == b.params,
            (CssStmt::UnknownAtRule(a, _), CssStmt::UnknownAtRule(b, _)) => {
                a.name == b.name && a.params == b.params && a.has_body == b.has_body
            }
            (CssStmt::KeyframesRuleSet(a), CssStmt::KeyframesRuleSet(b)) => {
                a.selector.len() == b.selector.len()
                    && a.selector
                        .iter()
                        .zip(&b.selector)
                        .all(|(x, y)| x.to_string() == y.to_string())
            }
            _ => false,
        }
    }

    pub fn copy_without_children(&self) -> Self {
        match self {
            CssStmt::RuleSet {
                selector,
                is_group_end,
                selector_span,
                span,
                ..
            } => CssStmt::RuleSet {
                selector: selector.clone(),
                body: Vec::new(),
                is_group_end: *is_group_end,
                selector_span: *selector_span,
                span: *span,
            },
            CssStmt::Style(..) | CssStmt::Comment(..) | CssStmt::Import(..) => unreachable!(),
            CssStmt::Media(media, is_group_end) => CssStmt::Media(
                MediaRule {
                    query: media.query.clone(),
                    body: Vec::new(),
                    span: media.span,
                },
                *is_group_end,
            ),
            CssStmt::UnknownAtRule(at_rule, is_group_end) => CssStmt::UnknownAtRule(
                UnknownAtRule {
                    name: at_rule.name.clone(),
                    params: at_rule.params.clone(),
                    body: Vec::new(),
                    has_body: at_rule.has_body,
                    span: at_rule.span,
                },
                *is_group_end,
            ),
            CssStmt::Supports(supports, is_group_end) => CssStmt::Supports(
                SupportsRule {
                    params: supports.params.clone(),
                    body: Vec::new(),
                    span: supports.span,
                },
                *is_group_end,
            ),
            CssStmt::KeyframesRuleSet(keyframes) => CssStmt::KeyframesRuleSet(KeyframesRuleSet {
                selector: keyframes.selector.clone(),
                body: Vec::new(),
                selector_span: keyframes.selector_span,
                span: keyframes.span,
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct KeyframesRuleSet {
    pub selector: Vec<KeyframesSelector>,
    pub body: Vec<CssStmt>,
    pub selector_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum KeyframesSelector {
    To,
    From,
    Percent(Box<str>),
}

#[derive(Debug, Clone)]
pub(crate) struct SupportsRule {
    pub params: String,
    pub body: Vec<CssStmt>,
    pub span: Span,
}
