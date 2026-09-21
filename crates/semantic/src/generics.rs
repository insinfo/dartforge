//! Substituição de parâmetros de tipo e inferência limitada de funções genéricas.
use super::*;
impl<'a> Validator<'a> {
    /// Restringe enums aprimorados a campos escalares finais ligados ao construtor const.
    pub(super) fn validate_enum(
        &self,
        class: &dartforge_syntax::Class<'a>,
    ) -> Result<(), Diagnostic> {
        if class.enum_values.is_empty() {
            return Ok(());
        }
        if class.enum_arguments.len() != class.enum_values.len()
            && !(class.enum_arguments.is_empty() && class.fields.is_empty())
        {
            return Err(Diagnostic::new(
                "Enum argument lists must match enum values",
                class.span,
            ));
        }
        let mut names = HashSet::new();
        for field in &class.fields {
            if !field.is_final
                || !matches!(
                    field.ty,
                    Type::Int
                        | Type::String
                        | Type::Bool
                        | Type::NullableInt
                        | Type::NullableString
                        | Type::NullableBool
                )
                || matches!(field.name, "name" | "index")
            {
                return Err(Diagnostic::new(
                    "Enhanced enum fields must be final scalar values with supported names",
                    field.span,
                ));
            }
            if !class.enum_constructor_fields.contains(&field.name) {
                return Err(Diagnostic::new(
                    "Enum field is not initialized by its const constructor",
                    field.span,
                ));
            }
        }
        for &name in &class.enum_constructor_fields {
            if !names.insert(name) || !class.fields.iter().any(|f| f.name == name) {
                return Err(Diagnostic::new(
                    "Invalid enum constructor field",
                    class.span,
                ));
            }
        }
        for arguments in &class.enum_arguments {
            if arguments.len() != class.enum_constructor_fields.len() {
                return Err(Diagnostic::new(
                    "Incorrect enum constructor argument count",
                    class.span,
                ));
            }
            for (arg, name) in arguments.iter().zip(&class.enum_constructor_fields) {
                let ty = class
                    .fields
                    .iter()
                    .find(|f| f.name == *name)
                    .expect("campo validado")
                    .ty;
                self.require_type(self.value_expected(arg, Some(ty))?, ty, arg.span)?;
                if !matches!(
                    self.evaluate_constant(arg)?,
                    ConstValue::Int(_)
                        | ConstValue::Bool(_)
                        | ConstValue::String(_)
                        | ConstValue::Null
                ) {
                    return Err(Diagnostic::new(
                        "Only scalar enum constructor constants are supported",
                        arg.span,
                    ));
                }
            }
        }
        for method in &class.methods {
            if method.name == "index" || (method.name == "name" && !method.is_getter) {
                return Err(Diagnostic::new(
                    "Overriding enum metadata properties is unsupported",
                    method.span,
                ));
            }
        }
        Ok(())
    }
    /// Avalia e registra constante somente após sua tipagem normal.
    pub(super) fn evaluate_constant(
        &self,
        expression: &Expr<'a>,
    ) -> Result<ConstValue, Diagnostic> {
        let value = {
            let resolution = self.resolution.borrow();
            self.const_evaluate(expression, &resolution)?
        };
        self.resolution
            .borrow_mut()
            .constant_values
            .insert((expression.span.start, expression.span.end), value.clone());
        Ok(value)
    }
    /// Avalia uma expressão const contra uma resolução já emprestada.
    ///
    /// Separar a avaliação do registro evita manter dois empréstimos da mesma
    /// `RefCell` quando uma instância const precisa avaliar seus argumentos.
    ///
    /// # Erros
    /// Propaga a recusa do avaliador de constantes, já com o intervalo exato.
    pub(super) fn const_evaluate(
        &self,
        expression: &Expr<'a>,
        resolution: &Resolution,
    ) -> Result<ConstValue, Diagnostic> {
        constants::evaluate(
            expression,
            resolution,
            &|name| {
                self.lookup(name)
                    .and_then(|b| b.constant.map(|v| (*v).clone()))
                    .or_else(|| {
                        // Uma variável de topo `const` só é visível quando nenhum
                        // local nem membro da instância oculta o nome escrito.
                        (self.lookup(name).is_none() && !self.has_implicit_member(name))
                            .then(|| self.global(name))
                            .flatten()
                            .and_then(|global| global.constant.as_deref().cloned())
                    })
            },
            &|inner| self.const_instance(inner, resolution),
        )
    }
    /// Substitui parâmetros dentro de funções e coleções, mantendo IDs originais intactos.
    pub(super) fn substitute(&self, ty: Type, bindings: &[Option<Type>], unknown: Type) -> Type {
        match ty {
            Type::NullableParameter(id) => self.nullable(
                bindings
                    .get(id as usize)
                    .copied()
                    .flatten()
                    .unwrap_or(unknown),
            ),
            Type::Parameter(id) => bindings
                .get(id as usize)
                .copied()
                .flatten()
                .unwrap_or(unknown),
            Type::Applied(_) => match self.shape(ty) {
                Some(TypeShape::Future(t)) => {
                    self.intern(TypeShape::Future(self.substitute(t, bindings, unknown)))
                }
                Some(TypeShape::Map { key, value }) => self.intern(TypeShape::Map {
                    key: self.substitute(key, bindings, unknown),
                    value: self.substitute(value, bindings, unknown),
                }),
                Some(TypeShape::Record { positional, named }) => self.intern(TypeShape::Record {
                    positional: positional
                        .into_iter()
                        .map(|t| self.substitute(t, bindings, unknown))
                        .collect(),
                    named: named
                        .into_iter()
                        .map(|(n, t)| (n, self.substitute(t, bindings, unknown)))
                        .collect(),
                }),
                Some(TypeShape::Nullable(t)) => {
                    self.nullable(self.substitute(t, bindings, unknown))
                }
                Some(TypeShape::List(t)) => {
                    self.intern(TypeShape::List(self.substitute(t, bindings, unknown)))
                }
                Some(TypeShape::Iterable(t)) => {
                    self.intern(TypeShape::Iterable(self.substitute(t, bindings, unknown)))
                }
                Some(TypeShape::Function { result, parameters }) => {
                    self.intern(TypeShape::Function {
                        result: self.substitute(result, bindings, unknown),
                        parameters: parameters
                            .into_iter()
                            .map(|t| self.substitute(t, bindings, unknown))
                            .collect(),
                    })
                }
                None => ty,
            },
            _ => ty,
        }
    }
    /// Detecta tipos ainda sem informação suficiente para um contexto de expressão.
    fn has_inferred(&self, ty: Type) -> bool {
        match ty {
            Type::Inferred => true,
            Type::Applied(_) => match self.shape(ty) {
                Some(TypeShape::Future(t)) => self.has_inferred(t),
                Some(TypeShape::Map { key, value }) => {
                    self.has_inferred(key) || self.has_inferred(value)
                }
                Some(TypeShape::Record { positional, named }) => positional
                    .into_iter()
                    .chain(named.into_iter().map(|(_, t)| t))
                    .any(|t| self.has_inferred(t)),
                Some(TypeShape::List(t) | TypeShape::Iterable(t) | TypeShape::Nullable(t)) => {
                    self.has_inferred(t)
                }
                Some(TypeShape::Function { result, parameters }) => {
                    self.has_inferred(result)
                        || parameters.into_iter().any(|t| self.has_inferred(t))
                }
                None => true,
            },
            _ => false,
        }
    }
    /// Recolhe restrições de argumentos concretos sem inventar dynamic ou bounds.
    fn infer(
        &self,
        formal: Type,
        actual: Type,
        bindings: &mut [Option<Type>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        match formal {
            Type::NullableParameter(id) => {
                if actual != Type::Null {
                    self.infer(
                        Type::Parameter(id),
                        self.without_null(actual),
                        bindings,
                        span,
                    )?;
                }
            }
            Type::Parameter(id) => {
                let slot = bindings
                    .get_mut(id as usize)
                    .ok_or_else(|| Diagnostic::new("Unknown generic parameter", span))?;
                *slot = Some(if let Some(old) = *slot {
                    self.common(old, actual, span)?
                } else {
                    actual
                });
            }
            Type::Applied(_) => match (self.shape(formal), self.shape(actual)) {
                (Some(TypeShape::Future(f)), Some(TypeShape::Future(a))) => {
                    self.infer(f, a, bindings, span)?
                }
                (
                    Some(TypeShape::Map { key: fk, value: fv }),
                    Some(TypeShape::Map { key: ak, value: av }),
                ) => {
                    self.infer(fk, ak, bindings, span)?;
                    self.infer(fv, av, bindings, span)?;
                }
                (
                    Some(TypeShape::Record {
                        positional: fp,
                        named: fnames,
                    }),
                    Some(TypeShape::Record {
                        positional: ap,
                        named: anames,
                    }),
                ) if fp.len() == ap.len()
                    && fnames
                        .iter()
                        .map(|(n, _)| n)
                        .eq(anames.iter().map(|(n, _)| n)) =>
                {
                    for (f, a) in fp.into_iter().zip(ap).chain(
                        fnames
                            .into_iter()
                            .map(|(_, t)| t)
                            .zip(anames.into_iter().map(|(_, t)| t)),
                    ) {
                        self.infer(f, a, bindings, span)?;
                    }
                }
                (Some(TypeShape::Nullable(inner)), _) => {
                    if actual != Type::Null {
                        self.infer(inner, self.without_null(actual), bindings, span)?;
                    }
                }
                (
                    Some(TypeShape::List(f) | TypeShape::Iterable(f)),
                    Some(TypeShape::List(a) | TypeShape::Iterable(a)),
                ) => self.infer(f, a, bindings, span)?,
                (
                    Some(TypeShape::Function {
                        result: f,
                        parameters: fp,
                    }),
                    Some(TypeShape::Function {
                        result: a,
                        parameters: ap,
                    }),
                ) if fp.len() == ap.len() => {
                    for (f, a) in fp.into_iter().zip(ap) {
                        self.infer(f, a, bindings, span)?;
                    }
                    self.infer(f, a, bindings, span)?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
    /// Resolve chamada genérica top-level; argumentos explícitos fixam cada parâmetro.
    pub(super) fn generic_call(
        &self,
        name: &str,
        type_arguments: &[Type],
        arguments: &[Expr<'a>],
        span: Span,
        context: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        if self.lookup(name).is_some() || self.has_implicit_member(name) {
            return Err(Diagnostic::new("Generic call target is shadowed", span));
        }
        let signature = self
            .functions
            .get(name)
            .ok_or_else(|| Diagnostic::new("Unknown generic function", span))?;
        if signature.generic_count == 0 || arguments.len() != signature.parameters.len() {
            return Err(Diagnostic::new("Incorrect generic call arity", span));
        }
        if !type_arguments.is_empty() && type_arguments.len() != signature.generic_count {
            return Err(Diagnostic::new("Incorrect type argument count", span));
        }
        let explicit = !type_arguments.is_empty();
        let mut bindings = vec![None; signature.generic_count];
        for (i, &ty) in type_arguments.iter().enumerate() {
            self.check_type_name(ty, span)?;
            if matches!(ty, Type::Void | Type::Inferred) {
                return Err(Diagnostic::new("Unsupported generic type argument", span));
            }
            bindings[i] = Some(ty);
        }
        if !explicit && let Some(context) = context {
            self.infer(signature.result, context, &mut bindings, span)?;
        }
        // Primeiro argumentos que não dependem do contexto de callbacks.
        for (arg, &formal) in arguments.iter().zip(&signature.parameters) {
            if matches!(arg.kind, ExprKind::Closure { .. }) {
                continue;
            }
            let expected = self.substitute(formal, &bindings, Type::Inferred);
            let actual = self.value_expected(
                arg,
                if self.has_inferred(expected) {
                    None
                } else {
                    Some(expected)
                },
            )?;
            if !explicit {
                self.infer(formal, actual, &mut bindings, arg.span)?;
            }
        }
        for (arg, &formal) in arguments.iter().zip(&signature.parameters) {
            if !matches!(arg.kind, ExprKind::Closure { .. }) {
                continue;
            }
            let expected = self.substitute(formal, &bindings, Type::Inferred);
            let actual = self.value_expected(arg, Some(expected))?;
            if !explicit {
                self.infer(formal, actual, &mut bindings, arg.span)?;
            }
        }
        while bindings.iter().any(Option::is_none) {
            let mut progressed = false;
            for i in 0..bindings.len() {
                if bindings[i].is_none() {
                    let candidate = self.substitute(signature.bounds[i], &bindings, Type::Inferred);
                    if !self.has_inferred(candidate) {
                        bindings[i] = Some(candidate);
                        progressed = true;
                    }
                }
            }
            if !progressed {
                return Err(Diagnostic::new(
                    "Cannot instantiate dependent generic bounds",
                    span,
                ));
            }
        }
        for (actual, bound) in bindings.iter().zip(&signature.bounds) {
            self.require_type(
                actual.expect("argumento inferido"),
                self.substitute(*bound, &bindings, Type::NullableObject),
                span,
            )?;
        }
        for (arg, &formal) in arguments.iter().zip(&signature.parameters) {
            let expected = self.substitute(formal, &bindings, Type::Inferred);
            self.require_type(
                self.value_expected(arg, Some(expected))?,
                expected,
                arg.span,
            )?;
        }
        self.resolution.borrow_mut().generic_arguments.insert(
            (span.start, span.end),
            bindings
                .iter()
                .map(|t| t.expect("argumento inferido"))
                .collect(),
        );
        Ok(self.substitute(signature.result, &bindings, Type::Inferred))
    }
    /// Registra nomes resolvidos sobre this sem modificar a AST emprestada.
    pub(super) fn implicit(&self, span: Span) {
        self.resolution
            .borrow_mut()
            .implicit_members
            .insert((span.start, span.end));
    }
    /// Registra leitura ou escrita resolvida para uma variável de topo.
    pub(super) fn global_access(&self, span: Span) {
        self.resolution
            .borrow_mut()
            .global_accesses
            .insert((span.start, span.end));
    }
    /// Registra acesso de propriedade com getter, distinto de tear-off de método.
    pub(super) fn getter(&self, span: Span) {
        self.resolution
            .borrow_mut()
            .getter_accesses
            .insert((span.start, span.end));
    }
}
