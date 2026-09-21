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
        entries: &[(Expr<'a>, Option<Expr<'a>>)],
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
            // `None` é elemento de controle (`...`, `if`, `for`): contribui o
            // par inteiro, não só o valor. A ordem escrita é preservada.
            let (actual, span) = match v {
                Some(v) => {
                    self.require_type(self.collection_element(k, Some(key))?, key, k.span)?;
                    (self.collection_element(v, fixed)?, v.span)
                }
                None => (self.map_element(k, key, fixed)?.1, k.span),
            };
            if let Some(expected) = fixed {
                self.require_type(actual, expected, span)?;
            } else {
                inferred = Some(if let Some(previous) = inferred {
                    self.common(previous, actual, span)?
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
        // `C.nome(...)` designa, nesta ordem, fábrica, construtor nomeado ou
        // método estático. Os três espaços de nome são disjuntos por construção.
        if let Some(signature) = class.factories.get(name) {
            self.check_call_arguments(
                arguments,
                &signature.parameters,
                signature.required_positional,
                &signature.named,
                span,
                |_| Diagnostic::new("Incorrect factory argument count", span),
            )?;
            return Ok(Type::Class(class_id));
        }
        if let Some(declared) = find_by_name(&class.named_constructors, name, |item| item.name) {
            if class.is_abstract {
                return Err(Diagnostic::new(
                    "Cannot construct an abstract class or enum",
                    span,
                ));
            }
            self.check_call_arguments(
                arguments,
                &declared.parameters,
                declared.required,
                &declared.named,
                span,
                |_| Diagnostic::new("Incorrect constructor argument count", span),
            )?;
            return Ok(Type::Class(class_id));
        }
        if let Some(signature) = self.static_method(class_id, name) {
            self.check_call_arguments(
                arguments,
                &signature.parameters,
                signature.required_positional,
                &signature.named,
                span,
                |_| Diagnostic::new("Incorrect static method argument count", span),
            )?;
            return Ok(signature.result);
        }
        self.reject_inherited_static(class_id, name, span)?;
        Err(Diagnostic::new(
            "Unknown named factory, named constructor or static method",
            span,
        ))
    }
}
