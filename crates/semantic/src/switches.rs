//! Tipagem e cobertura conservadora de switch com padrões simples e guardas.
use super::*;
use dartforge_syntax::{Pattern, SwitchArm, SwitchCase};
impl<'a> Validator<'a> {
    /// Introduz bindings do padrão e avalia constantes sem promover variáveis externas.
    fn pattern(
        &mut self,
        pattern: &Pattern<'a>,
        scrutinee: Type,
        span: Span,
    ) -> Result<Option<ConstValue>, Diagnostic> {
        match pattern {
            Pattern::Wildcard => Ok(None),
            Pattern::Type(ty) => {
                if !matches!(ty, Type::Class(_)) {
                    return Err(Diagnostic::new(
                        "Empty object patterns require a non-null nominal type",
                        span,
                    ));
                }
                self.check_type_name(*ty, span)?;
                Ok(None)
            }
            Pattern::Binding { ty, name } => {
                if matches!(
                    ty,
                    Type::Applied(_)
                        | Type::Parameter(_)
                        | Type::Inferred
                        | Type::Void
                        | Type::Null
                ) {
                    return Err(Diagnostic::new(
                        "Binding patterns require a supported scalar or nominal type",
                        span,
                    ));
                }
                let ty = if *ty == Type::Inferred {
                    scrutinee
                } else {
                    self.check_type_name(*ty, span)?;
                    *ty
                };
                if self
                    .require_type(non_null(scrutinee), non_null(ty), span)
                    .is_err()
                    && self
                        .require_type(non_null(ty), non_null(scrutinee), span)
                        .is_err()
                {
                    return Err(Diagnostic::new(
                        "Pattern type does not overlap the scrutinee type",
                        span,
                    ));
                }
                self.scopes.last_mut().expect("escopo padrão").insert(
                    name,
                    Binding {
                        constant: None,
                        ty: Some(ty),
                        is_final: false,
                        promoted: None,
                    },
                );
                Ok(None)
            }
            Pattern::Constant(e) => {
                let ty = self.value(e)?;
                if ty != Type::Null {
                    self.require_type(ty, non_null(scrutinee), e.span)?;
                }
                Ok(Some(self.evaluate_constant(e)?))
            }
        }
    }
    /// Calcula o conjunto finito exigido para bool e enum, incluindo null se necessário.
    fn finite_values(&self, ty: Type) -> Option<Vec<ConstValue>> {
        let mut values = match non_null(ty) {
            Type::Bool => vec![ConstValue::Bool(false), ConstValue::Bool(true)],
            Type::Class(id) => {
                let class = self.classes.get(&id)?;
                if class.enum_values.is_empty() {
                    return None;
                }
                class
                    .enum_values
                    .iter()
                    .map(|name| ConstValue::Enum {
                        class_id: id,
                        name: (*name).into(),
                    })
                    .collect()
            }
            Type::Null => vec![],
            _ => return None,
        };
        if is_nullable(ty) || ty == Type::Null {
            values.push(ConstValue::Null);
        }
        Some(values)
    }
    /// Considera cobertura de padrões não guardados; guardas nunca tornam o switch exaustivo.
    fn covers(
        &self,
        ty: Type,
        patterns: &[(&Pattern<'a>, bool, Option<ConstValue>)],
        span: Span,
    ) -> bool {
        let mut values = Vec::new();
        for (pattern, guarded, value) in patterns {
            if *guarded {
                continue;
            }
            match pattern {
                Pattern::Wildcard => return true,
                Pattern::Binding { ty: binding, .. } | Pattern::Type(binding) => {
                    if is_nullable(*binding) {
                        values.push(ConstValue::Null);
                    }
                    if *binding == Type::Inferred || self.require_type(ty, *binding, span).is_ok() {
                        return true;
                    }
                    if self.require_type(non_null(ty), *binding, span).is_ok()
                        && let Some(all) = self.finite_values(non_null(ty))
                    {
                        values.extend(all);
                    }
                }
                Pattern::Constant(_) => {
                    if let Some(value) = value {
                        values.push(value.clone());
                    }
                }
            }
        }
        if self
            .finite_values(ty)
            .is_some_and(|needed| needed.iter().all(|v| values.contains(v)))
        {
            return true;
        }
        if let Type::Class(id) = non_null(ty)
            && self.classes[&id].modifier == ClassModifier::Sealed
        {
            return (!is_nullable(ty) || values.contains(&ConstValue::Null))
                && self.covers_cone(id, patterns, span, &mut HashSet::new());
        }
        false
    }
    /// Expande apenas cones fechados; um subtipo aberto exige padrão que cubra o próprio tipo.
    fn covers_cone(
        &self,
        id: u32,
        patterns: &[(&Pattern<'a>, bool, Option<ConstValue>)],
        span: Span,
        active: &mut HashSet<u32>,
    ) -> bool {
        if patterns.iter().any(|(pattern, guarded, _)| {
            !guarded
                && match pattern {
                    Pattern::Wildcard => true,
                    Pattern::Type(ty) | Pattern::Binding { ty, .. } => {
                        self.require_type(Type::Class(id), *ty, span).is_ok()
                    }
                    _ => false,
                }
        }) {
            return true;
        }
        let class = &self.classes[&id];
        if !class.enum_values.is_empty() {
            return self.finite_values(Type::Class(id)).is_some_and(|needed| {
                needed.iter().all(|v| {
                    patterns
                        .iter()
                        .any(|(_, guarded, value)| !guarded && value.as_ref() == Some(v))
                })
            });
        }
        if class.modifier != ClassModifier::Sealed && !class.is_mixin_application {
            return false;
        }
        if !active.insert(id) {
            return false;
        }
        let result = self
            .classes
            .iter()
            .filter(|(_, child)| child.superclass == Some(id) || child.interfaces.contains(&id))
            .all(|(&child, _)| self.covers_cone(child, patterns, span, active));
        active.remove(&id);
        result
    }
    /// Valida os braços com tipos contextuais e exige cobertura completa da expressão.
    pub(super) fn switch_expression(
        &self,
        scrutinee: &Expr<'a>,
        arms: &[SwitchArm<'a>],
        span: Span,
        context: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        let ty = self.value(scrutinee)?;
        let mut patterns = Vec::new();
        let mut result = None;
        for arm in arms {
            let mut branch = self.clone();
            branch.scopes.push(HashMap::new());
            let constant = branch.pattern(&arm.pattern, ty, arm.span)?;
            if let Some(guard) = &arm.guard {
                branch.require_type(branch.value(guard)?, Type::Bool, guard.span)?;
                branch.promote(guard, true);
            }
            let actual = branch.value_expected(&arm.value, context)?;
            if let Some(expected) = context {
                self.require_type(actual, expected, arm.value.span)?;
            }
            result = Some(if let Some(previous) = result {
                self.common(previous, actual, arm.value.span)?
            } else {
                actual
            });
            patterns.push((&arm.pattern, arm.guard.is_some(), constant));
        }
        if !self.covers(ty, &patterns, span) {
            return Err(Diagnostic::new(
                "Non-exhaustive switch expression; guarded patterns do not provide coverage",
                span,
            ));
        }
        result.ok_or_else(|| Diagnostic::new("Switch expression requires at least one arm", span))
    }
    /// Valida casos com escopos separados; invalida escritas sem ganhar promoções entre casos.
    pub(super) fn switch_statement(
        &mut self,
        scrutinee: &Expr<'a>,
        cases: &[SwitchCase<'a>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        let ty = self.value(scrutinee)?;
        let mut patterns = Vec::new();
        for case in cases {
            let mut branch = self.clone();
            branch.switch_depth += 1;
            branch.scopes.push(HashMap::new());
            let constant = branch.pattern(&case.pattern, ty, case.span)?;
            if let Some(guard) = &case.guard {
                branch.require_type(branch.value(guard)?, Type::Bool, guard.span)?;
                branch.promote(guard, true);
            }
            branch.block(&case.body)?;
            self.invalidate_writes(&case.body);
            patterns.push((&case.pattern, case.guard.is_some(), constant));
        }
        if self.covers(ty, &patterns, span) {
            self.exhaustive_switches
                .borrow_mut()
                .insert((span.start, span.end));
        } else if self.finite_values(ty).is_some()
            || matches!(non_null(ty), Type::Class(id) if self.classes[&id].modifier == ClassModifier::Sealed)
        {
            return Err(Diagnostic::new(
                "Non-exhaustive switch statement over enum, bool or sealed type",
                span,
            ));
        }
        Ok(())
    }
    /// Prova retorno sem considerar laços e sem atravessar break ou continue inalcançáveis.
    pub(super) fn returns(&self, statements: &[Statement<'a>]) -> bool {
        for s in statements {
            match &s.kind {
                StatementKind::Return(_) => return true,
                StatementKind::Block(body) if self.returns(body) => return true,
                StatementKind::If {
                    then_body,
                    else_body: Some(other),
                    ..
                } if self.returns(then_body) && self.returns(other) => return true,
                StatementKind::Switch { cases, .. }
                    if self
                        .exhaustive_switches
                        .borrow()
                        .contains(&(s.span.start, s.span.end))
                        && cases.iter().all(|c| self.returns(&c.body)) =>
                {
                    return true;
                }
                StatementKind::Break | StatementKind::Continue => return false,
                _ => {}
            }
        }
        false
    }
}
