//! Restrições de modificadores por biblioteca e propagação nominal transitiva.
use super::*;
impl<'a> Validator<'a> {
    /// Confere arestas explícitas e exige base/final/sealed em todo subtipo restrito.
    pub(super) fn validate_modifiers(
        &self,
        class: &dartforge_syntax::Class<'a>,
    ) -> Result<(), Diagnostic> {
        let error = |message| Diagnostic::new(message, class.span);
        if !class.mixins.is_empty() {
            return Err(error(
                "Mixin applications must be lowered before semantic analysis",
            ));
        }
        if class.kind != ClassKind::Class
            && (class.is_interface
                || !matches!(class.modifier, ClassModifier::None | ClassModifier::Base))
        {
            return Err(error(
                "Mixins and mixin classes only support the base modifier",
            ));
        }
        if class.kind != ClassKind::Class && class.superclass.is_some() {
            return Err(error(
                "A mixin declaration cannot have an explicit superclass",
            ));
        }
        if class.is_interface && class.modifier != ClassModifier::None {
            return Err(error("Incompatible class modifiers"));
        }
        if let Some(id) = class.superclass {
            let parent = &self.classes[&id];
            if parent.kind == ClassKind::Mixin {
                return Err(error("A mixin cannot be extended"));
            }
            if parent.library_id != class.library_id
                && (parent.is_interface
                    || matches!(
                        parent.modifier,
                        ClassModifier::Final | ClassModifier::Sealed
                    ))
            {
                return Err(Diagnostic::new(
                    format!(
                        "Superclass modifier {} prohibits extension from another library",
                        if parent.is_interface {
                            "interface".into()
                        } else {
                            format!("{:?}", parent.modifier)
                        }
                    ),
                    class.span,
                ));
            }
        }
        for &id in &class.interfaces {
            let parent = &self.classes[&id];
            if parent.library_id != class.library_id
                && matches!(
                    parent.modifier,
                    ClassModifier::Final | ClassModifier::Sealed
                )
            {
                return Err(Diagnostic::new(
                    format!(
                        "Interface modifier {:?} prohibits implementation from another library",
                        parent.modifier
                    ),
                    class.span,
                ));
            }
            // A aplicação efetiva de um mixin base fornece sua implementação, não a imita.
            if class.is_mixin_application && class.mixin_origin == Some(id) {
                continue;
            }
            if self.ancestors(id).iter().any(|ancestor| {
                let info = &self.classes[ancestor];
                info.library_id != class.library_id
                    && matches!(info.modifier, ClassModifier::Base | ClassModifier::Final)
            }) {
                return Err(error(
                    "Cannot implement a foreign base/final supertype, including indirectly",
                ));
            }
        }
        let restricted = self.ancestors(class.id).into_iter().any(|id| {
            id != class.id
                && matches!(
                    self.classes[&id].modifier,
                    ClassModifier::Base | ClassModifier::Final
                )
        });
        if restricted
            && !class.is_mixin_application
            && class.enum_values.is_empty()
            && !matches!(
                class.modifier,
                ClassModifier::Base | ClassModifier::Final | ClassModifier::Sealed
            )
        {
            return Err(error(
                "A subtype of base/final must be base, final or sealed",
            ));
        }
        Ok(())
    }
}
