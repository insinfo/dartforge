//! Inicialização definida de campos e escopo de construtores generativos simples.
use super::*;
impl<'a> Validator<'a> {
    /// Confere campos próprios inicializados por declaração ou formal e a chamada super implícita.
    pub(super) fn validate_constructor_fields(
        &self,
        class: &dartforge_syntax::Class<'a>,
    ) -> Result<(), Diagnostic> {
        if !class.enum_values.is_empty() {
            return Ok(());
        }
        if class.constructor.is_some()
            && (class.kind != ClassKind::Class || class.is_mixin_application)
        {
            return Err(Diagnostic::new(
                "Explicit constructors on mixin declarations are unsupported",
                class.span,
            ));
        }
        if class
            .superclass
            .is_some_and(|id| !self.classes[&id].constructor_parameters.is_empty())
        {
            return Err(Diagnostic::new(
                "Implicit super() cannot invoke a constructor requiring arguments",
                class.span,
            ));
        }
        let mut names = HashSet::new();
        let mut initialized = HashSet::new();
        if let Some(constructor) = &class.constructor {
            for parameter in &constructor.parameters {
                self.check_type_name(parameter.ty, parameter.span)?;
                if matches!(parameter.ty, Type::Void | Type::Inferred)
                    || !names.insert(parameter.name)
                {
                    return Err(Diagnostic::new(
                        "Invalid or duplicate constructor parameter",
                        parameter.span,
                    ));
                }
                if let Some(name) = parameter.field {
                    let field = class
                        .fields
                        .iter()
                        .find(|field| field.name == name)
                        .ok_or_else(|| {
                            Diagnostic::new(
                                "Initializing formal requires an own instance field",
                                parameter.span,
                            )
                        })?;
                    if !initialized.insert(name) || (field.is_final && field.initializer.is_some())
                    {
                        return Err(Diagnostic::new(
                            "Field is initialized more than once",
                            parameter.span,
                        ));
                    }
                    self.require_type(parameter.ty, field.ty, parameter.span)?;
                }
            }
        }
        for field in &class.fields {
            if field.initializer.is_none()
                && !initialized.contains(field.name)
                && (field.is_final || !self.may_be_null(field.ty))
            {
                return Err(Diagnostic::new(
                    "Instance field requires an initializer or an initializing formal",
                    field.span,
                ));
            }
        }
        Ok(())
    }
    /// Formais this.campo não são variáveis locais do corpo; parâmetros comuns ocultam membros.
    pub(super) fn constructor_body(
        &mut self,
        constructor: &dartforge_syntax::Constructor<'a>,
    ) -> Result<(), Diagnostic> {
        self.type_parameters.clear();
        self.loop_depth = 0;
        self.switch_depth = 0;
        self.inferred_returns = None;
        self.return_type = Type::Void;
        let mut parameters = HashMap::new();
        for parameter in &constructor.parameters {
            if parameter.field.is_none() {
                parameters.insert(
                    parameter.name,
                    Binding {
                        constant: None,
                        ty: Some(parameter.ty),
                        is_final: false,
                        promoted: None,
                    },
                );
            }
        }
        self.scopes.push(parameters);
        self.in_constructor = true;
        let result = self.block(&constructor.body);
        self.in_constructor = false;
        self.scopes.pop();
        result
    }
}
