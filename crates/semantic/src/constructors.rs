//! Inicialização definida de campos e escopo de construtores generativos simples.
use super::*;
impl<'a> Validator<'a> {
    /// Confere campos próprios inicializados por declaração ou formal e a chamada super implícita.
    pub(super) fn validate_constructor_fields(
        &mut self,
        class: &dartforge_syntax::Class<'a>,
    ) -> Result<(), Diagnostic> {
        if !class.enum_values.is_empty() {
            return Ok(());
        }
        if class.named_constructors.is_empty() && !self.classes[&class.id].has_generative {
            return Ok(());
        }
        if (class.constructor.is_some() || !class.named_constructors.is_empty())
            && (class.kind != ClassKind::Class || class.is_mixin_application)
        {
            return Err(Diagnostic::new(
                "Explicit constructors on mixin declarations are unsupported",
                class.span,
            ));
        }
        let plain = dartforge_syntax::ConstructorExtras::default();
        if let Some(constructor) = &class.constructor {
            let extras = class.constructor_extras.as_deref().unwrap_or(&plain);
            self.validate_constructor(class, None, Some(constructor), extras)?;
        } else if class.named_constructors.is_empty() {
            self.validate_constructor(class, None, None, &plain)?;
        }
        for declared in &class.named_constructors {
            self.validate_constructor(
                class,
                Some(declared.name),
                Some(&declared.constructor),
                &declared.extras,
            )?;
        }
        Ok(())
    }
    /// Confere um construtor: parâmetros, lista de inicialização, super e campos.
    ///
    /// A ordem de verificação acompanha a ordem de execução do Dart: formais,
    /// lista de inicialização, `super` e, só então, a exigência de que todo
    /// campo tenha origem definida.
    fn validate_constructor(
        &mut self,
        class: &dartforge_syntax::Class<'a>,
        name: Option<&'a str>,
        constructor: Option<&dartforge_syntax::Constructor<'a>>,
        extras: &dartforge_syntax::ConstructorExtras<'a>,
    ) -> Result<(), Diagnostic> {
        let span = constructor.map_or(class.span, |declared| declared.span);
        let parameters = constructor.map_or(&[][..], |declared| &declared.parameters);
        // Listas curtas: varrer é mais barato do que montar tabela alguma.
        let mut names: Vec<&str> = Vec::new();
        let mut labels: Vec<&str> = Vec::new();
        let mut initialized: Vec<&str> = Vec::new();
        for parameter in parameters {
            self.check_type_name(parameter.ty, parameter.span)?;
            if matches!(parameter.ty, Type::Void | Type::Inferred)
                || names.contains(&parameter.name)
            {
                return Err(Diagnostic::new(
                    "Invalid or duplicate constructor parameter",
                    parameter.span,
                ));
            }
            names.push(parameter.name);
            if parameter.kind.is_named() {
                // Dart 3.12: só o initializing formal pode ter nome privado,
                // e o rótulo externo perde o sublinhado inicial.
                if parameter.name.starts_with('_') && parameter.field.is_none() {
                    return Err(Diagnostic::new(
                        "A named parameter accepts a private name only as an initializing formal",
                        parameter.span,
                    ));
                }
                if labels.contains(&parameter.label()) {
                    return Err(Diagnostic::new(
                        format!("Duplicate named parameter '{}'", parameter.label()),
                        parameter.span,
                    ));
                }
                labels.push(parameter.label());
            }
            self.validate_parameter_default(
                parameter.kind,
                parameter.ty,
                parameter.default.as_deref(),
                parameter.span,
            )?;
            if let Some(name) = parameter.field {
                // Um redirecionador delega a inicialização inteira ao alvo, e
                // por isso não pode gravar campo nenhum: `this.campo` num
                // redirecionador é erro de compilação também no Dart.
                if extras.redirect.is_some() {
                    return Err(Diagnostic::new(
                        format!(
                            "A redirecting constructor cannot declare the initializing formal 'this.{name}': it delegates the whole initialization to the target; declare a plain parameter and pass it in the redirection"
                        ),
                        parameter.span,
                    ));
                }
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
                if initialized.contains(&name) || (field.is_final && field.initializer.is_some()) {
                    return Err(Diagnostic::new(
                        "Field is initialized more than once",
                        parameter.span,
                    ));
                }
                initialized.push(name);
                self.require_type(parameter.ty, field.ty, parameter.span)?;
            }
        }
        self.with_initializer_scope(parameters, |validator| {
            // A ordem é a escrita: `assert` e `campo = valor` são entradas da
            // mesma lista e o Dart avalia uma depois da outra. `before` diz
            // quantas entradas de campo precedem cada asserção.
            for (index, entry) in extras.initializers.iter().enumerate() {
                validator.validate_initializer_asserts(extras, index)?;
                let field = class
                    .fields
                    .iter()
                    .find(|field| field.name == entry.field)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "An initializer list entry requires an own instance field",
                            entry.span,
                        )
                    })?;
                if initialized.contains(&entry.field)
                    || (field.is_final && field.initializer.is_some())
                {
                    return Err(Diagnostic::new(
                        "Field is initialized more than once",
                        entry.span,
                    ));
                }
                initialized.push(entry.field);
                let actual = validator.value_expected(&entry.value, Some(field.ty))?;
                validator.require_type(actual, field.ty, entry.value.span)?;
            }
            validator.validate_initializer_asserts(extras, extras.initializers.len())?;
            // Um redirecionador delega inteiramente: não há `super` próprio nem
            // campo para exigir, porque o alvo é que inicializa o objeto.
            if let Some(call) = &extras.redirect {
                return validator.validate_redirect(class, name, call);
            }
            validator.validate_super_call(class, parameters, extras, span)
        })?;
        if extras.redirect.is_some() {
            if extras.is_const {
                return Err(Diagnostic::new(
                    "A const redirecting constructor is unsupported in this subset",
                    span,
                ));
            }
            return Ok(());
        }
        for field in &class.fields {
            if field.initializer.is_none()
                && !field.is_late
                && !initialized.contains(&field.name)
                && (field.is_final || !self.may_be_null(field.ty))
            {
                return Err(Diagnostic::new(
                    "Instance field requires an initializer or an initializing formal",
                    field.span,
                ));
            }
        }
        if extras.is_const && constructor.is_some_and(|declared| !declared.body.is_empty()) {
            return Err(Diagnostic::new(
                "A const constructor cannot have a body",
                span,
            ));
        }
        Ok(())
    }
    /// Confere as asserções da lista de inicialização escritas nesta posição.
    ///
    /// Uma asserção da lista roda antes do corpo e antes de `super`; o escopo
    /// é o mesmo das entradas `campo = valor`, sem `this`.
    ///
    /// # Erros
    /// Recusa condição não booleana e mensagem que não seja `String`.
    fn validate_initializer_asserts(
        &self,
        extras: &dartforge_syntax::ConstructorExtras<'a>,
        position: usize,
    ) -> Result<(), Diagnostic> {
        for entry in extras
            .asserts
            .iter()
            .filter(|entry| entry.before == position)
        {
            self.assert_statement(&entry.condition, entry.message.as_ref())?;
        }
        Ok(())
    }
    /// Resolve `: this(...)` ou `: this.nome(...)` para um construtor da classe.
    ///
    /// # Erros
    /// Recusa alvo inexistente, redirecionamento para o próprio construtor,
    /// cadeia cíclica e contagem ou rótulo de argumento incompatível.
    fn validate_redirect(
        &self,
        class: &dartforge_syntax::Class<'a>,
        from: Option<&'a str>,
        call: &dartforge_syntax::RedirectCall<'a>,
    ) -> Result<(), Diagnostic> {
        if call.name == from {
            return Err(Diagnostic::new(
                "A redirecting constructor cannot redirect to itself",
                call.span,
            ));
        }
        // A cadeia é curta e local à classe; seguir os nomes escritos detecta o
        // ciclo sem alocar tabela alguma e sem depender de ordem de declaração.
        let mut seen: Vec<Option<&'a str>> = vec![from, call.name];
        let mut current = call.name;
        while let Some(next) = redirect_of(class, current) {
            if seen.contains(&next.name) {
                return Err(Diagnostic::new(
                    "Redirecting constructors form a cycle",
                    call.span,
                ));
            }
            seen.push(next.name);
            current = next.name;
        }
        let info = self
            .classes
            .get(&class.id)
            .ok_or_else(|| Diagnostic::new("Unknown class", call.span))?;
        match call.name {
            None => {
                if !info.has_generative {
                    return Err(Diagnostic::new(
                        "The class has no unnamed generative constructor to redirect to",
                        call.span,
                    ));
                }
                self.check_call_arguments(
                    &call.arguments,
                    &info.constructor_parameters,
                    info.constructor_required,
                    &info.constructor_named,
                    call.span,
                    |_| {
                        Diagnostic::new(
                            "Incorrect redirected constructor argument count",
                            call.span,
                        )
                    },
                )
            }
            Some(name) => {
                let declared = find_by_name(&info.named_constructors, name, |item| item.name)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            format!("Unknown redirected constructor '{}.{name}'", info.name),
                            call.span,
                        )
                    })?;
                self.check_call_arguments(
                    &call.arguments,
                    &declared.parameters,
                    declared.required,
                    &declared.named,
                    call.span,
                    |_| {
                        Diagnostic::new(
                            "Incorrect redirected constructor argument count",
                            call.span,
                        )
                    },
                )
            }
        }
    }
    /// Executa a ação com os parâmetros do construtor em escopo e sem `this`.
    ///
    /// A lista de inicialização e os argumentos de `super` são avaliados antes
    /// de a base existir, então qualquer leitura de `this` é recusada ali.
    fn with_initializer_scope<T>(
        &mut self,
        parameters: &[dartforge_syntax::ConstructorParameter<'a>],
        action: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let mut scope = HashMap::new();
        for parameter in parameters {
            scope.insert(
                parameter.name,
                Binding {
                    constant: None,
                    ty: Some(parameter.ty),
                    is_final: false,
                    is_late: false,
                    promoted: None,
                },
            );
        }
        self.scopes.push(scope);
        let previous = std::mem::replace(&mut self.in_initializer_list, true);
        let result = action(self);
        self.in_initializer_list = previous;
        self.scopes.pop();
        result
    }
    /// Resolve `super(...)`, `super.nome(...)` ou a chamada implícita sem argumentos.
    fn validate_super_call(
        &self,
        class: &dartforge_syntax::Class<'a>,
        parameters: &[dartforge_syntax::ConstructorParameter<'a>],
        extras: &dartforge_syntax::ConstructorExtras<'a>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let _ = parameters;
        let Some(call) = &extras.super_call else {
            if class.superclass.is_some_and(|id| {
                !self.classes[&id].has_generative
                    || !self.classes[&id].constructor_parameters.is_empty()
                    || !self.classes[&id].constructor_named.is_empty()
            }) {
                return Err(Diagnostic::new(
                    "Implicit super() cannot invoke a constructor requiring arguments",
                    span,
                ));
            }
            return Ok(());
        };
        let Some(base) = class.superclass else {
            return Err(Diagnostic::new(
                "A class without a superclass cannot call super",
                call.span,
            ));
        };
        let info = self
            .classes
            .get(&base)
            .ok_or_else(|| Diagnostic::new("Unknown superclass", call.span))?;
        match call.name {
            None => {
                if !info.has_generative {
                    return Err(Diagnostic::new(
                        "The superclass has no unnamed generative constructor",
                        call.span,
                    ));
                }
                self.check_call_arguments(
                    &call.arguments,
                    &info.constructor_parameters,
                    info.constructor_required,
                    &info.constructor_named,
                    call.span,
                    |_| Diagnostic::new("Incorrect super constructor argument count", call.span),
                )
            }
            Some(name) => {
                let declared = find_by_name(&info.named_constructors, name, |item| item.name)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            format!("Unknown superclass constructor '{}.{name}'", info.name),
                            call.span,
                        )
                    })?;
                self.check_call_arguments(
                    &call.arguments,
                    &declared.parameters,
                    declared.required,
                    &declared.named,
                    call.span,
                    |_| Diagnostic::new("Incorrect super constructor argument count", call.span),
                )?;
                Ok(())
            }
        }
    }
    /// Monta as receitas dos construtores `const` declarados pela classe.
    ///
    /// A construção acontece antes da validação dos corpos porque um `const
    /// C(1)` escrito em qualquer ponto do programa precisa da receita pronta.
    ///
    /// # Erros
    /// Recusa construtores `const` fora do recorte aceito, sempre no intervalo
    /// da declaração que impede a canonicalização.
    pub(super) fn build_const_plans(
        &mut self,
        class: &dartforge_syntax::Class<'a>,
    ) -> Result<(), Diagnostic> {
        let mut plans: Vec<(&'a str, Rc<ConstPlan<'a>>)> = Vec::new();
        if let Some(constructor) = &class.constructor
            && let Some(extras) = class.constructor_extras.as_deref()
            && extras.is_const
        {
            plans.push((
                "",
                Rc::new(self.const_plan(
                    class,
                    &constructor.parameters,
                    extras,
                    constructor.span,
                )?),
            ));
        }
        for declared in &class.named_constructors {
            if declared.extras.is_const {
                plans.push((
                    declared.name,
                    Rc::new(self.const_plan(
                        class,
                        &declared.constructor.parameters,
                        &declared.extras,
                        declared.constructor.span,
                    )?),
                ));
            }
        }
        if plans.is_empty() {
            return Ok(());
        }
        plans.sort_unstable_by_key(|(name, _)| *name);
        Rc::make_mut(&mut self.classes)
            .get_mut(&class.id)
            .expect("classe registrada antes das receitas const")
            .const_plans = plans;
        Ok(())
    }
    /// Deriva a receita de um único construtor `const` declarado.
    fn const_plan(
        &self,
        class: &dartforge_syntax::Class<'a>,
        parameters: &[dartforge_syntax::ConstructorParameter<'a>],
        extras: &dartforge_syntax::ConstructorExtras<'a>,
        span: Span,
    ) -> Result<ConstPlan<'a>, Diagnostic> {
        if class.superclass.is_some() || !class.mixins.is_empty() {
            return Err(Diagnostic::new(
                "A const constructor requires a class without a superclass in this subset",
                span,
            ));
        }
        if !extras.initializers.is_empty() || extras.super_call.is_some() {
            return Err(Diagnostic::new(
                "A const constructor accepts only initializing formals in this subset",
                span,
            ));
        }
        let mut formals = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            let Some(field) = parameter.field else {
                return Err(Diagnostic::new(
                    "A const constructor accepts only initializing formals in this subset",
                    parameter.span,
                ));
            };
            let default = match parameter.default.as_deref() {
                Some(value) => Some(self.evaluate_constant(value)?),
                None => None,
            };
            formals.push(ConstFormal {
                field,
                label: parameter.label(),
                kind: parameter.kind,
                default,
            });
        }
        let mut fields = Vec::with_capacity(class.fields.len());
        for field in &class.fields {
            if !field.is_final {
                return Err(Diagnostic::new(
                    "A const constructor requires every instance field to be final",
                    field.span,
                ));
            }
            if formals.iter().any(|formal| formal.field == field.name) {
                fields.push((field.name, None));
                continue;
            }
            let initializer = field.initializer.as_ref().ok_or_else(|| {
                Diagnostic::new(
                    "A const class field requires an initializing formal or a constant initializer",
                    field.span,
                )
            })?;
            fields.push((field.name, Some(self.evaluate_constant(initializer)?)));
        }
        Ok(ConstPlan { formals, fields })
    }
    /// Produz o valor canônico de uma invocação de construtor em contexto const.
    ///
    /// # Erros
    /// Recusa classes sem receita const, rótulos desconhecidos e argumentos que
    /// não sejam eles próprios constantes.
    pub(super) fn const_instance(
        &self,
        expression: &Expr<'a>,
        resolution: &Resolution,
    ) -> Result<ConstValue, Diagnostic> {
        let (class_id, name, arguments) = match &expression.kind {
            ExprKind::Construct {
                class_id,
                arguments,
            } => (*class_id, "", arguments),
            ExprKind::NamedConstruct {
                class_id,
                name,
                arguments,
            } => (*class_id, *name, arguments),
            _ => {
                return Err(Diagnostic::new(
                    "Const expression: calls, getters and this expression are unsupported",
                    expression.span,
                ));
            }
        };
        let plan = self
            .classes
            .get(&class_id)
            .and_then(|info| find_by_name(&info.const_plans, name, |(key, _)| *key))
            .map(|(_, plan)| Rc::clone(plan))
            .ok_or_else(|| {
                Diagnostic::new(
                    "Const expression: the class has no matching const constructor",
                    expression.span,
                )
            })?;
        let (written, labelled) = split_arguments(arguments, expression.span)?;
        let mut values: Vec<Option<ConstValue>> = vec![None; plan.formals.len()];
        for (index, argument) in written.iter().enumerate() {
            if index >= values.len() {
                return Err(Diagnostic::new(
                    "Const expression: too many constructor arguments",
                    argument.span,
                ));
            }
            values[index] = Some(self.const_evaluate(argument, resolution)?);
        }
        for argument in labelled {
            let ExprKind::NamedArgument { label, value } = &argument.kind else {
                continue;
            };
            let index = plan
                .formals
                .iter()
                .position(|formal| formal.kind.is_named() && formal.label == *label)
                .ok_or_else(|| {
                    Diagnostic::new(
                        format!("Const expression: unknown named argument '{label}'"),
                        argument.span,
                    )
                })?;
            values[index] = Some(self.const_evaluate(value, resolution)?);
        }
        for (index, formal) in plan.formals.iter().enumerate() {
            if values[index].is_none() {
                values[index] = Some(formal.default.clone().unwrap_or(ConstValue::Null));
            }
        }
        let mut fields = Vec::with_capacity(plan.fields.len());
        for (field, declared) in &plan.fields {
            let value = match declared {
                Some(value) => value.clone(),
                None => plan
                    .formals
                    .iter()
                    .position(|formal| formal.field == *field)
                    .and_then(|index| values[index].clone())
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "Const expression: a field has no constant value",
                            expression.span,
                        )
                    })?,
            };
            fields.push(((*field).to_owned(), value));
        }
        Ok(ConstValue::Instance { class_id, fields })
    }
    /// Formais this.campo não são variáveis locais do corpo; parâmetros comuns ocultam membros.
    pub(super) fn constructor_body(
        &mut self,
        constructor: &dartforge_syntax::Constructor<'a>,
    ) -> Result<(), Diagnostic> {
        self.type_parameters = Rc::new(Vec::new());
        self.in_async = false;
        self.in_arrow = false;
        self.loop_depth = 0;
        self.switch_depth = 0;
        self.labels.clear();
        self.catch_depth = 0;
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
                        is_late: false,
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

/// Devolve o redirecionamento declarado pelo construtor de nome indicado.
fn redirect_of<'a, 'b>(
    class: &'b dartforge_syntax::Class<'a>,
    name: Option<&str>,
) -> Option<&'b dartforge_syntax::RedirectCall<'a>> {
    match name {
        None => class
            .constructor_extras
            .as_deref()
            .and_then(|extras| extras.redirect.as_ref()),
        Some(name) => class
            .named_constructors
            .iter()
            .find(|declared| declared.name == name)
            .and_then(|declared| declared.extras.redirect.as_ref()),
    }
}
