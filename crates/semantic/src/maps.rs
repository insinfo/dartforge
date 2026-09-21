//! Mapas com chaves String e factories nomeadas sem receptor de instância.
use super::*;
impl<'a> Validator<'a> {
    /// O escopo estático ainda oculta globais com nomes de membros de instância.
    pub(super) fn reject_factory_instance(&self, name: &str, span: Span) -> Result<(), Diagnostic> {
        if self.lookup(name).is_none()
            && self
                .factory_class
                .is_some_and(|id| self.field(id, name).is_some() || self.method(id, name).is_some())
        {
            return Err(Diagnostic::new(
                "Instance member unavailable in factory",
                span,
            ));
        }
        Ok(())
    }
    /// Infere valores preservando contexto e verifica cada entrada na ordem fonte.
    pub(super) fn map_expression(
        &self,
        key_type: Option<Type>,
        value_type: Option<Type>,
        entries: &[(Expr<'a>, Expr<'a>)],
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let context = expected.and_then(|t| self.shape(self.without_null(t)));
        let (ck, cv) = if let Some(TypeShape::Map { key, value }) = context {
            (Some(key), Some(value))
        } else {
            (None, None)
        };
        let key = key_type.or(ck).unwrap_or(Type::String);
        if key != Type::String {
            return Err(Diagnostic::new("Only String Map keys are supported", span));
        }
        self.check_type_name(key, span)?;
        let fixed = value_type.or(cv);
        if let Some(value) = fixed {
            if matches!(value, Type::Void | Type::Inferred) {
                return Err(Diagnostic::new("Unsupported Map value type", span));
            }
            self.check_type_name(value, span)?;
        }
        let mut inferred = None;
        for (k, v) in entries {
            self.require_type(self.collection_element(k, Some(key))?, key, k.span)?;
            let actual = self.collection_element(v, fixed)?;
            if let Some(expected) = fixed {
                self.require_type(actual, expected, v.span)?;
            } else {
                inferred = Some(if let Some(previous) = inferred {
                    self.common(previous, actual, v.span)?
                } else {
                    actual
                });
            }
        }
        let value = fixed.or(inferred).ok_or_else(|| {
            Diagnostic::new(
                "Empty Map requires an explicit or contextual value type",
                span,
            )
        })?;
        Ok(self.intern(TypeShape::Map { key, value }))
    }
    /// Factories são funções da classe declaradora, não métodos herdados de instância.
    pub(super) fn factory_call(
        &self,
        class_id: u32,
        name: &str,
        arguments: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let class = self
            .classes
            .get(&class_id)
            .ok_or_else(|| Diagnostic::new("Unknown factory class", span))?;
        if self.lookup(class.name).is_some() || self.has_implicit_member(class.name) {
            return Err(Diagnostic::new("Declaration shadows factory class", span));
        }
        let signature = class
            .factories
            .get(name)
            .ok_or_else(|| Diagnostic::new("Unknown named factory", span))?;
        self.check_call_arguments(
            arguments,
            &signature.parameters,
            signature.required_positional,
            &signature.named,
            span,
            |_| Diagnostic::new("Incorrect factory argument count", span),
        )?;
        Ok(Type::Class(class_id))
    }
}
