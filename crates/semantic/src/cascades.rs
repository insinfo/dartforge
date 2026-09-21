//! Cascatas conservam o tipo do receptor e isolam a remoção local de null.
use super::*;

impl<'a> Validator<'a> {
    /// Verifica todas as seções, mesmo quando um receptor constante null impede sua execução.
    pub(super) fn cascade(
        &self,
        receiver: &Expr<'a>,
        null_aware: bool,
        sections: &[Statement<'a>],
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let ty = self.value_expected(receiver, expected)?;
        if sections.is_empty() {
            return Err(Diagnostic::new(
                "A cascade requires at least one section",
                span,
            ));
        }
        if !null_aware && self.may_be_null(ty) {
            return Err(Diagnostic::new(
                "Ordinary cascades require a non-null receiver",
                receiver.span,
            ));
        }
        let mut nested = self.clone();
        nested.in_async = false;
        nested.cascade_receiver = Some(if null_aware {
            self.without_null(self.upper_bound(ty))
        } else {
            ty
        });
        for section in sections {
            if !matches!(
                section.kind,
                StatementKind::Expression(_)
                    | StatementKind::FieldAssign { .. }
                    | StatementKind::IndexAssign { .. }
            ) {
                return Err(Diagnostic::new("Unsupported cascade section", section.span));
            }
            nested.statement(section)?;
        }
        Ok(ty)
    }
}
