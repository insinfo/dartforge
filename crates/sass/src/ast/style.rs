use codemap::Spanned;

use crate::{interner::InternedString, value::Value};

/// A style: `color: red`
#[derive(Clone, Debug)]
pub(crate) struct Style {
    pub property: InternedString,
    pub value: Box<Spanned<Value>>,
    pub declared_as_custom_property: bool,
    /// `CssDeclaration.name.span`, `span` e `valueSpanForMap` do dart-sass.
    pub name_span: codemap::Span,
    pub span: codemap::Span,
    pub value_span_for_map: codemap::Span,
}
