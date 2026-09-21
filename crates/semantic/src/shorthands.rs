//! Atalhos de ponto do Dart 3.10: `.nome` resolvido apenas pelo tipo de contexto.
//!
//! O parser não conhece a classe alvo, portanto a forma sintática chega sem
//! identidade nominal. Esta análise exige um tipo de contexto concreto e recusa
//! qualquer posição em que o alvo seria inferido a partir do próprio atalho.
use super::*;
impl<'a> Validator<'a> {
    /// Resolve `.nome` ou `.nome(args)` contra o tipo esperado da posição.
    ///
    /// Aceita valores de enum, fábricas nomeadas e `.new(...)` para o construtor
    /// sem nome. O contexto anulável é aceito: `Cor? c = .vermelho;` resolve a
    /// classe subjacente, como em Dart.
    ///
    /// # Erros
    /// Retorna diagnóstico quando não há contexto, quando o contexto não é uma
    /// classe declarada ou quando o membro não existe na classe alvo.
    pub(super) fn dot_shorthand(
        &self,
        name: &str,
        arguments: Option<&[Expr<'a>]>,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let context = expected.ok_or_else(|| {
            Diagnostic::new(
                "Dot shorthand requires a context type in this position",
                span,
            )
        })?;
        let class_id = match self.without_null(context) {
            Type::Class(id) => id,
            _ => {
                return Err(Diagnostic::new(
                    "Dot shorthand requires a class or enum context type",
                    span,
                ));
            }
        };
        let class = self
            .classes
            .get(&class_id)
            .ok_or_else(|| Diagnostic::new("Unknown dot shorthand target", span))?;
        let Some(arguments) = arguments else {
            if !class.enum_values.contains(&name) {
                return Err(Diagnostic::new(
                    format!("'{name}' is not an enum value of '{}'", class.name),
                    span,
                ));
            }
            return Ok(Type::Class(class_id));
        };
        if name == "new" {
            return self.shorthand_construct(class_id, arguments, span);
        }
        if !class.factories.contains_key(name) {
            return Err(Diagnostic::new(
                format!("'{name}' is not a named factory of '{}'", class.name),
                span,
            ));
        }
        self.factory_call(class_id, name, arguments, span)
    }
    /// Valida `.new(...)` com as mesmas regras do construtor sem nome escrito por extenso.
    fn shorthand_construct(
        &self,
        class_id: u32,
        arguments: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let class = self
            .classes
            .get(&class_id)
            .ok_or_else(|| Diagnostic::new("Unknown dot shorthand target", span))?;
        if class.is_abstract
            || !class.has_generative
            || class.kind == ClassKind::Mixin
            || class.is_mixin_application
            || !class.enum_values.is_empty()
        {
            return Err(Diagnostic::new(
                "Cannot construct an abstract class or enum",
                span,
            ));
        }
        let positional = class.constructor_parameters.clone();
        let required = class.constructor_required;
        let named = class.constructor_named.clone();
        self.check_call_arguments(arguments, &positional, required, &named, span, |_| {
            Diagnostic::new("Incorrect constructor argument count", span)
        })?;
        Ok(Type::Class(class_id))
    }
}
