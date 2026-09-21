//! Tipos estruturais, inferência contextual e capturas conservadoras de closures.
use super::*;
impl<'a> Validator<'a> {
    /// Limita impressão recursiva aos valores cuja representação Dart está implementada.
    pub(super) fn printable_type(&self, ty: Type) -> bool {
        match ty {
            Type::Parameter(_) | Type::NullableParameter(_) => {
                self.printable_type(self.upper_bound(ty))
            }
            Type::Int
            | Type::Bool
            | Type::String
            | Type::Null
            | Type::NullableInt
            | Type::NullableBool
            | Type::NullableString => true,
            Type::Applied(_) => match self.shape(ty) {
                Some(TypeShape::Map { key, value }) => {
                    self.printable_type(key) && self.printable_type(value)
                }
                Some(TypeShape::Record { positional, named }) => positional
                    .iter()
                    .chain(named.iter().map(|(_, ty)| ty))
                    .all(|ty| self.printable_type(*ty)),
                Some(
                    TypeShape::List(element)
                    | TypeShape::Iterable(element)
                    | TypeShape::Nullable(element),
                ) => self.printable_type(element),
                _ => false,
            },
            _ => false,
        }
    }
    /// Recupera uma forma sem manter empréstimo da arena durante análise recursiva.
    pub(super) fn shape(&self, ty: Type) -> Option<TypeShape> {
        if let Type::Applied(id) = ty {
            self.resolution.borrow().types.get(id as usize).cloned()
        } else {
            None
        }
    }
    /// Acrescenta formas inferidas sem alterar IDs originalmente fornecidos pelo parser.
    pub(super) fn intern(&self, shape: TypeShape) -> Type {
        let mut resolution = self.resolution.borrow_mut();
        if let Some(id) = resolution.types.iter().position(|s| *s == shape) {
            return Type::Applied(id as u32);
        }
        let id = resolution.types.len() as u32;
        resolution.types.push(shape);
        Type::Applied(id)
    }
    /// Valida a arena original e impede ciclos ou IDs fora de seus limites.
    pub(super) fn validate_shapes(&self) -> Result<(), Diagnostic> {
        let types = self.resolution.borrow().types.clone();
        for start in 0..types.len() {
            let mut active = HashSet::new();
            let mut done = HashSet::new();
            let mut stack = vec![(start, false)];
            while let Some((id, exit)) = stack.pop() {
                if exit {
                    active.remove(&id);
                    done.insert(id);
                    continue;
                }
                if done.contains(&id) {
                    continue;
                }
                if !active.insert(id) {
                    return Err(Diagnostic::new(
                        "Recursive structural types are unsupported",
                        Span { start: 0, end: 0 },
                    ));
                }
                let shape = types.get(id).ok_or_else(|| {
                    Diagnostic::new("Unknown structural type ID", Span { start: 0, end: 0 })
                })?;
                stack.push((id, true));
                let children = match shape {
                    TypeShape::Future(t) => vec![*t],
                    TypeShape::Map { key, value } => vec![*key, *value],
                    TypeShape::Record { positional, named } => {
                        self.record_shape(positional.len(), named, Span { start: 0, end: 0 })?;
                        positional
                            .iter()
                            .chain(named.iter().map(|(_, ty)| ty))
                            .copied()
                            .collect()
                    }
                    TypeShape::List(t) | TypeShape::Iterable(t) | TypeShape::Nullable(t) => {
                        vec![*t]
                    }
                    TypeShape::Function { result, parameters } => {
                        let mut v = parameters.clone();
                        v.push(*result);
                        v
                    }
                };
                for ty in children {
                    if let Type::Applied(child) = ty {
                        stack.push((child as usize, false));
                    }
                }
            }
        }
        Ok(())
    }
    /// Compara estruturas; escritas em listas covariantes são checadas pelo descritor original.
    pub(super) fn require_structural(
        &self,
        actual: Type,
        expected: Type,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let fail = || Diagnostic::new("Incompatible structural type", span);
        match (self.shape(actual), self.shape(expected)) {
            (Some(TypeShape::Future(a)), Some(TypeShape::Future(b))) => {
                if b == Type::Void {
                    Ok(())
                } else {
                    self.require_type(a, b, span)
                }
            }
            (
                Some(TypeShape::Map { key: ak, value: av }),
                Some(TypeShape::Map { key: bk, value: bv }),
            ) => {
                self.require_type(ak, bk, span)?;
                self.require_type(av, bv, span)
            }
            (
                Some(TypeShape::Record {
                    positional: a,
                    named: an,
                }),
                Some(TypeShape::Record {
                    positional: b,
                    named: bn,
                }),
            ) => {
                if a.len() != b.len()
                    || an.len() != bn.len()
                    || an.iter().zip(&bn).any(|(a, b)| a.0 != b.0)
                {
                    return Err(fail());
                }
                for (a, b) in a
                    .into_iter()
                    .chain(an.into_iter().map(|(_, ty)| ty))
                    .zip(b.into_iter().chain(bn.into_iter().map(|(_, ty)| ty)))
                {
                    self.require_type(a, b, span)?;
                }
                Ok(())
            }
            (Some(TypeShape::List(a)), Some(TypeShape::List(b))) => self.require_type(a, b, span),
            (Some(TypeShape::List(a) | TypeShape::Iterable(a)), Some(TypeShape::Iterable(b))) => {
                self.require_type(a, b, span)
            }
            (
                Some(TypeShape::Function {
                    result: a,
                    parameters: ap,
                }),
                Some(TypeShape::Function {
                    result: b,
                    parameters: bp,
                }),
            ) => {
                if ap.len() != bp.len() {
                    return Err(fail());
                }
                for (a, b) in ap.into_iter().zip(bp) {
                    self.require_type(b, a, span)?;
                }
                if b == Type::Void || b == Type::Inferred {
                    Ok(())
                } else {
                    self.require_type(a, b, span)
                }
            }
            _ => Err(fail()),
        }
    }
    /// Confere recursivamente nomes de tipos e proíbe inferência dinâmica em anotações.
    pub(super) fn check_shape_name(&self, ty: Type, span: Span) -> Result<(), Diagnostic> {
        let shape = self
            .shape(ty)
            .ok_or_else(|| Diagnostic::new("Unknown structural type", span))?;
        let (name, children) = match shape {
            TypeShape::Future(t) => {
                self.future_name(span)?;
                self.check_type_name(t, span)?;
                return Ok(());
            }
            TypeShape::Map { key, value } => {
                if key != Type::String {
                    return Err(Diagnostic::new("Only String Map keys are supported", span));
                }
                ("Map", vec![key, value])
            }
            TypeShape::Record { positional, named } => {
                self.record_shape(positional.len(), &named, span)?;
                for child in positional
                    .into_iter()
                    .chain(named.into_iter().map(|(_, ty)| ty))
                {
                    if matches!(child, Type::Void | Type::Inferred) {
                        return Err(Diagnostic::new("Unsupported record field type", span));
                    }
                    self.check_type_name(child, span)?;
                }
                return Ok(());
            }
            TypeShape::Nullable(inner) => {
                if matches!(inner, Type::Void | Type::Inferred) {
                    return Err(Diagnostic::new(
                        "Unsupported nullable structural type",
                        span,
                    ));
                }
                return self.check_type_name(inner, span);
            }
            TypeShape::List(t) => ("List", vec![t]),
            TypeShape::Iterable(t) => ("Iterable", vec![t]),
            TypeShape::Function { result, parameters } => {
                for p in parameters {
                    if p == Type::Void {
                        return Err(Diagnostic::new("Void function parameter", span));
                    }
                    self.check_type_name(p, span)?;
                }
                return self.check_type_name(result, span);
            }
        };
        if self.lookup(name).is_some()
            || self.has_implicit_member(name)
            || self.functions.contains_key(name)
            || self.classes.values().any(|c| c.name == name)
        {
            return Err(Diagnostic::new("Declaration shadows collection type", span));
        }
        for t in children {
            if t == Type::Void {
                return Err(Diagnostic::new(
                    "Void collection elements are unsupported",
                    span,
                ));
            }
            self.check_type_name(t, span)?;
        }
        Ok(())
    }
    /// Obtém o elemento de uma lista ou iterable; funções não são coleções.
    pub(super) fn element(&self, ty: Type) -> Option<Type> {
        match self.shape(self.upper_bound(ty)) {
            Some(TypeShape::List(t) | TypeShape::Iterable(t)) => Some(t),
            _ => None,
        }
    }
    /// Determina um supertipo comum, preservando anulabilidade e formas já representadas.
    pub(super) fn common(&self, a: Type, b: Type, span: Span) -> Result<Type, Diagnostic> {
        if a == b {
            return Ok(a);
        }
        if matches!(a, Type::Void | Type::Inferred) || matches!(b, Type::Void | Type::Inferred) {
            return Err(Diagnostic::new(
                "No common value type for void or unresolved expressions",
                span,
            ));
        }
        if a == Type::Null || b == Type::Null {
            return Ok(self.nullable(if a == Type::Null { b } else { a }));
        }
        if self.require_type(a, b, span).is_ok() {
            Ok(b)
        } else if self.require_type(b, a, span).is_ok() {
            Ok(a)
        } else {
            if self.may_be_null(a) || self.may_be_null(b) {
                let left = self.without_null(a);
                let right = self.without_null(b);
                if left != a || right != b {
                    return Ok(self.nullable(self.common(left, right, span)?));
                }
            }
            match (self.shape(a), self.shape(b)) {
                (Some(TypeShape::Future(a)), Some(TypeShape::Future(b))) => {
                    return Ok(self.intern(TypeShape::Future(self.common(a, b, span)?)));
                }
                (
                    Some(TypeShape::Map { key: ak, value: av }),
                    Some(TypeShape::Map { key: bk, value: bv }),
                ) => {
                    return Ok(self.intern(TypeShape::Map {
                        key: self.common(ak, bk, span)?,
                        value: self.common(av, bv, span)?,
                    }));
                }
                (
                    Some(TypeShape::Record {
                        positional: a,
                        named: an,
                    }),
                    Some(TypeShape::Record {
                        positional: b,
                        named: bn,
                    }),
                ) => {
                    if a.len() != b.len()
                        || an.len() != bn.len()
                        || an.iter().zip(&bn).any(|(a, b)| a.0 != b.0)
                    {
                        return Err(Diagnostic::new(
                            "Least upper bound of different record shapes requires unsupported Record type",
                            span,
                        ));
                    }
                    let positional = a
                        .into_iter()
                        .zip(b)
                        .map(|(a, b)| self.common(a, b, span))
                        .collect::<Result<Vec<_>, _>>()?;
                    let named = an
                        .into_iter()
                        .zip(bn)
                        .map(|((name, a), (_, b))| self.common(a, b, span).map(|ty| (name, ty)))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(self.intern(TypeShape::Record { positional, named }));
                }
                (Some(TypeShape::List(x)), Some(TypeShape::List(y))) => {
                    return Ok(self.intern(TypeShape::List(self.common(x, y, span)?)));
                }
                (
                    Some(TypeShape::List(x) | TypeShape::Iterable(x)),
                    Some(TypeShape::List(y) | TypeShape::Iterable(y)),
                ) => return Ok(self.intern(TypeShape::Iterable(self.common(x, y, span)?))),
                (
                    Some(TypeShape::Function {
                        result: x,
                        parameters: xp,
                    }),
                    Some(TypeShape::Function {
                        result: y,
                        parameters: yp,
                    }),
                ) => {
                    if xp.len() != yp.len() {
                        return Err(Diagnostic::new(
                            "Function LUB with different arity is unsupported",
                            span,
                        ));
                    }
                    let mut parameters = Vec::new();
                    for (x, y) in xp.into_iter().zip(yp) {
                        parameters.push(if self.require_type(x, y, span).is_ok() {
                            x
                        } else if self.require_type(y, x, span).is_ok() {
                            y
                        } else {
                            return Err(Diagnostic::new(
                                "Function parameter intersection is unsupported",
                                span,
                            ));
                        });
                    }
                    return Ok(self.intern(TypeShape::Function {
                        result: self.common(x, y, span)?,
                        parameters,
                    }));
                }
                _ => {}
            }
            if let (Type::Class(x), Type::Class(y)) = (a, b) {
                let left = self.ancestors(x);
                let right = self.ancestors(y);
                let common = left.intersection(&right).copied().collect::<Vec<_>>();
                let closest = common
                    .iter()
                    .copied()
                    .filter(|id| {
                        !common
                            .iter()
                            .any(|other| other != id && self.ancestors(*other).contains(id))
                    })
                    .collect::<Vec<_>>();
                if closest.len() == 1 {
                    return Ok(Type::Class(closest[0]));
                }
                if closest.len() > 1 {
                    return Err(Diagnostic::new(
                        "Ambiguous nominal least upper bound is unsupported",
                        span,
                    ));
                }
            }
            if matches!(a, Type::Parameter(_) | Type::NullableParameter(_))
                || matches!(b, Type::Parameter(_) | Type::NullableParameter(_))
            {
                return Err(Diagnostic::new(
                    "Least upper bound of unrelated type parameters requires explicit type arguments",
                    span,
                ));
            }
            Ok(if self.may_be_null(a) || self.may_be_null(b) {
                Type::NullableObject
            } else {
                Type::Object
            })
        }
    }
    /// Registra tipos resolvidos inclusive para closures e referências a funções.
    pub(super) fn expression_expected(
        &self,
        e: &Expr<'a>,
        expected: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        if let ExprKind::Identifier(name)
        | ExprKind::Call { name, .. }
        | ExprKind::GenericCall { name, .. } = &e.kind
        {
            self.reject_factory_instance(name, e.span)?;
        }
        let ty = match &e.kind {
            ExprKind::FutureValue { value, value_type } => self.future_value(
                value.as_deref(),
                value_type.or_else(|| {
                    if let Some(TypeShape::Future(t)) = expected.and_then(|t| self.shape(t)) {
                        Some(t)
                    } else {
                        None
                    }
                }),
                e.span,
            )?,
            ExprKind::FutureDelayed {
                duration,
                computation,
                value_type,
            } => self.future_delayed(
                duration,
                computation.as_deref(),
                value_type.or_else(|| {
                    if let Some(TypeShape::Future(t)) = expected.and_then(|t| self.shape(t)) {
                        Some(t)
                    } else {
                        None
                    }
                }),
                e.span,
            )?,
            ExprKind::Map {
                key_type,
                value_type,
                entries,
            } => self.map_expression(*key_type, *value_type, entries, expected, e.span)?,
            ExprKind::Cascade {
                receiver,
                null_aware,
                sections,
            } => self.cascade(receiver, *null_aware, sections, expected, e.span)?,
            ExprKind::Record { fields } => self.record_expression(fields, expected, e.span)?,
            ExprKind::Const(inner) => {
                let ty = self.expression_expected(inner, expected)?;
                self.evaluate_constant(e)?;
                ty
            }
            ExprKind::Switch { scrutinee, arms } => {
                self.switch_expression(scrutinee, arms, e.span, expected)?
            }
            ExprKind::Conditional {
                condition,
                then_value,
                else_value,
            } => self.conditional(condition, then_value, else_value, expected, e.span)?,
            ExprKind::Throw(value) => self.throw_expression(value, expected)?,
            ExprKind::GenericCall {
                name,
                type_arguments,
                arguments,
            } => self.generic_call(name, type_arguments, arguments, e.span, expected)?,
            ExprKind::Call { name, arguments }
                if self.lookup(name).is_none()
                    && !self.has_implicit_member(name)
                    && self
                        .functions
                        .get(name)
                        .is_some_and(|s| s.generic_count > 0) =>
            {
                self.generic_call(name, &[], arguments, e.span, expected)?
            }
            ExprKind::DotShorthand { name, arguments } => {
                self.dot_shorthand(name, arguments.as_deref(), expected, e.span)?
            }
            ExprKind::Closure {
                parameters,
                return_type,
                body,
                is_arrow,
                is_async,
            } => self.closure(
                parameters,
                *return_type,
                body,
                (*is_arrow, *is_async),
                expected,
                e.span,
            )?,
            ExprKind::List {
                element_type,
                elements,
            } => {
                let context = element_type.or_else(|| expected.and_then(|t| self.element(t)));
                if let Some(t) = element_type {
                    self.check_type_name(*t, e.span)?;
                }
                let mut inferred = context;
                for value in elements {
                    let actual = self.collection_element(value, context)?;
                    if let Some(target) = context {
                        self.require_type(actual, target, value.span)?;
                    }
                    inferred = Some(match inferred {
                        Some(previous) if context.is_none() => {
                            self.common(previous, actual, value.span)?
                        }
                        Some(t) => t,
                        None => actual,
                    });
                }
                let element = inferred.ok_or_else(|| {
                    Diagnostic::new(
                        "Empty List requires explicit element type or context",
                        e.span,
                    )
                })?;
                if element == Type::Void || element == Type::Inferred {
                    return Err(Diagnostic::new("Unsupported list element type", e.span));
                }
                self.intern(TypeShape::List(element))
            }
            _ => self.expression_inner(e)?,
        };
        self.resolution
            .borrow_mut()
            .expr_types
            .insert((e.span.start, e.span.end), ty);
        Ok(ty)
    }
    /// Analisa um elemento de coleção e remove a nulabilidade aceita por `?`.
    ///
    /// Dart 3.8 omite o elemento quando `?valor` avalia para null, portanto o
    /// tipo do elemento é o tipo do operando sem null. O operando recebe o
    /// contexto anulável correspondente e é avaliado uma única vez.
    pub(super) fn collection_element(
        &self,
        e: &Expr<'a>,
        context: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        let ExprKind::NullAwareElement(inner) = &e.kind else {
            return self.value_expected(e, context);
        };
        let nullable = context.map(|t| self.nullable(t));
        let actual = self.value_expected(inner, nullable)?;
        let ty = self.without_null(actual);
        if ty == Type::Void || ty == Type::Inferred || ty == Type::Null {
            return Err(Diagnostic::new(
                "Unsupported null-aware collection element type",
                e.span,
            ));
        }
        self.resolution
            .borrow_mut()
            .expr_types
            .insert((e.span.start, e.span.end), ty);
        Ok(ty)
    }
    /// Analisa valor com contexto e rejeita ausência de resultado.
    pub(super) fn value_expected(
        &self,
        e: &Expr<'a>,
        expected: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        let ty = self.expression_expected(e, expected)?;
        if ty == Type::Void {
            Err(Diagnostic::new(
                "A void expression cannot be used as a value",
                e.span,
            ))
        } else {
            Ok(ty)
        }
    }
    /// Analisa parâmetros inferidos e um corpo independente de retorno e laços externos.
    pub(super) fn closure(
        &self,
        parameters: &[dartforge_syntax::Parameter<'a>],
        annotation: Type,
        body: &[Statement<'a>],
        modifiers: (bool, bool),
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let (is_arrow, is_async) = modifiers;
        let contextual = expected.and_then(|t| self.shape(t));
        let (context_result, context_params) = match contextual {
            Some(TypeShape::Function { result, parameters }) => (Some(result), parameters),
            _ => (None, vec![]),
        };
        if !context_params.is_empty() && context_params.len() != parameters.len() {
            return Err(Diagnostic::new("Incorrect closure parameter count", span));
        }
        let mut nested = self.clone();
        nested.in_async = is_async;
        nested.in_arrow = is_arrow;
        let context_result = if is_async {
            context_result
                .map(|t| self.async_result(t, span))
                .transpose()?
        } else {
            context_result
        };
        let annotation = if is_async && annotation != Type::Inferred {
            self.async_result(annotation, span)?
        } else {
            annotation
        };
        for scope in &mut nested.scopes {
            for binding in scope.values_mut() {
                if !binding.is_final {
                    binding.promoted = None;
                }
            }
        }
        let mut scope = HashMap::new();
        let mut types = Vec::new();
        for (index, p) in parameters.iter().enumerate() {
            let ty = if p.ty == Type::Inferred {
                context_params.get(index).copied().ok_or_else(|| {
                    Diagnostic::new(
                        "Untyped closure parameter requires function context",
                        p.span,
                    )
                })?
            } else {
                self.check_type_name(p.ty, p.span)?;
                p.ty
            };
            if ty == Type::Void || ty == Type::Inferred {
                return Err(Diagnostic::new(
                    "Unsupported closure parameter type",
                    p.span,
                ));
            }
            if crate::is_wildcard(p.name) {
                // Dart 3.7: `_` em closures preserva a aridade sem declarar nome.
                types.push(ty);
                continue;
            }
            if scope
                .insert(
                    p.name,
                    Binding {
                        constant: None,
                        ty: Some(ty),
                        is_final: false,
                        promoted: None,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new("Duplicate closure parameter", p.span));
            }
            types.push(ty);
        }
        nested.scopes.push(scope);
        nested.loop_depth = 0;
        nested.in_constructor = false;
        nested.switch_depth = 0;
        // Rótulos e cláusulas catch não atravessam a fronteira de uma closure.
        nested.labels.clear();
        nested.catch_depth = 0;
        nested.return_type = if annotation != Type::Inferred {
            annotation
        } else {
            context_result.unwrap_or(Type::Inferred)
        };
        let returns = Rc::new(RefCell::new(Vec::new()));
        nested.inferred_returns = Some(returns.clone());
        nested.block(body)?;
        let mut values = returns.borrow().clone();
        if context_result == Some(Type::Void) && annotation == Type::Inferred {
            if !is_arrow && values.iter().any(|ty| *ty != Type::Void) {
                return Err(Diagnostic::new(
                    "A block closure in void context cannot return a value",
                    span,
                ));
            }
            values = vec![Type::Void];
        }
        if values.is_empty() {
            values.push(Type::Void);
        } else if !nested.returns(body) && values.iter().any(|t| *t != Type::Void) {
            values.push(Type::Null);
        }
        let mut result = values[0];
        for value in &values[1..] {
            result = self.common(result, *value, span)?;
        }
        if annotation != Type::Inferred {
            self.require_type(result, annotation, span)?;
            result = annotation;
        }
        if is_async {
            result = self.intern(TypeShape::Future(result));
        }
        let ty = self.intern(TypeShape::Function {
            result,
            parameters: types,
        });
        if let Some(expected) = expected {
            self.require_type(ty, expected, span)?;
        }
        Ok(ty)
    }
    /// Invoca valor funcional aplicando contexto aos argumentos e checando aridade.
    pub(super) fn invoke(
        &self,
        ty: Type,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let Some(TypeShape::Function { result, parameters }) = self.shape(self.upper_bound(ty))
        else {
            return Err(Diagnostic::new(
                "Calling a non-function value is unsupported",
                span,
            ));
        };
        if args.len() != parameters.len() {
            return Err(Diagnostic::new("Incorrect function argument count", span));
        }
        for (argument, expected) in args.iter().zip(parameters) {
            self.require_type(
                self.value_expected(argument, Some(expected))?,
                expected,
                argument.span,
            )?;
        }
        Ok(result)
    }
    /// Tipagem dos membros de coleção incluídos neste marco, com callbacks contextuais.
    pub(super) fn collection_call(
        &self,
        receiver: Type,
        name: &str,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let element = self.element(receiver).expect("coleção conhecida");
        if name == "toList" {
            if !args.is_empty() {
                return Err(Diagnostic::new(
                    "toList accepts no arguments in this subset",
                    span,
                ));
            }
            return Ok(self.intern(TypeShape::List(element)));
        }
        if name == "add" && matches!(self.shape(receiver), Some(TypeShape::List(_))) {
            if args.len() != 1 {
                return Err(Diagnostic::new("add expects one element", span));
            }
            self.require_type(
                self.value_expected(&args[0], Some(element))?,
                element,
                args[0].span,
            )?;
            return Ok(Type::Void);
        }
        let result = match name {
            "where" | "any" => Type::Bool,
            "forEach" => Type::Void,
            "map" => Type::Inferred,
            _ => return Err(Diagnostic::new("Unsupported collection method", span)),
        };
        if args.len() != 1 {
            return Err(Diagnostic::new(
                "Collection callback requires one argument",
                span,
            ));
        }
        let callback = self.intern(TypeShape::Function {
            result,
            parameters: vec![element],
        });
        let actual = self.value_expected(&args[0], Some(callback))?;
        self.require_type(actual, callback, args[0].span)?;
        match name {
            "where" => Ok(self.intern(TypeShape::Iterable(element))),
            "any" => Ok(Type::Bool),
            "forEach" => Ok(Type::Void),
            "map" => {
                let Some(TypeShape::Function { result, .. }) = self.shape(actual) else {
                    unreachable!()
                };
                if result == Type::Void {
                    return Err(Diagnostic::new("Mapping to void is unsupported", span));
                }
                Ok(self.intern(TypeShape::Iterable(result)))
            }
            _ => unreachable!(),
        }
    }
}
/// Coleta escritas dentro de closures; nomes homônimos são invalidados conservadoramente.
pub(super) fn captured_writes<'a>(program: &Program<'a>) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    for f in &program.functions {
        scan_body(&f.body, false, &mut names);
    }
    for c in &program.classes {
        for f in &c.fields {
            if let Some(initializer) = &f.initializer {
                scan_expr(initializer, &mut names);
            }
        }
        if let Some(constructor) = &c.constructor {
            scan_body(&constructor.body, false, &mut names);
        }
        for m in c.methods.iter().chain(&c.factories) {
            scan_body(&m.body, false, &mut names);
        }
    }
    for e in &program.extensions {
        for m in &e.methods {
            scan_body(&m.body, false, &mut names);
        }
    }
    scan_body(&program.statements, false, &mut names);
    names
}
/// Percorre instruções e posições de expressão sem interpretar fluxo executável.
fn scan_body<'a>(body: &[Statement<'a>], inside: bool, names: &mut HashSet<&'a str>) {
    for s in body {
        match &s.kind {
            StatementKind::Switch { scrutinee, cases } => {
                scan_expr(scrutinee, names);
                for case in cases {
                    if let Some(guard) = &case.guard {
                        scan_expr(guard, names);
                    }
                    scan_body(&case.body, inside, names);
                }
            }
            StatementKind::Assign { name, value } => {
                if inside {
                    names.insert(name);
                }
                scan_expr(value, names);
            }
            StatementKind::Variable { initializer, .. }
            | StatementKind::RecordDestructure { initializer, .. }
            | StatementKind::Print(initializer)
            | StatementKind::Expression(initializer) => scan_expr(initializer, names),
            StatementKind::FieldAssign {
                receiver, value, ..
            } => {
                scan_expr(receiver, names);
                scan_expr(value, names);
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                scan_expr(receiver, names);
                scan_expr(index, names);
                scan_expr(value, names);
            }
            StatementKind::Return(Some(e)) => scan_expr(e, names),
            StatementKind::Block(b) => scan_body(b, inside, names),
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                scan_expr(condition, names);
                scan_body(then_body, inside, names);
                if let Some(b) = else_body {
                    scan_body(b, inside, names);
                }
            }
            StatementKind::While { condition, body }
            | StatementKind::DoWhile { condition, body } => {
                scan_expr(condition, names);
                scan_body(body, inside, names);
            }
            StatementKind::ForIn { iterable, body, .. } => {
                scan_expr(iterable, names);
                scan_body(body, inside, names);
            }
            StatementKind::Labeled { body, .. } => {
                scan_body(std::slice::from_ref(body), inside, names);
            }
            StatementKind::Assert { condition, message } => {
                scan_expr(condition, names);
                if let Some(message) = message {
                    scan_expr(message, names);
                }
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => {
                scan_body(body, inside, names);
                for clause in catches {
                    scan_body(&clause.body, inside, names);
                }
                if let Some(body) = finally_body {
                    scan_body(body, inside, names);
                }
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                if let Some(s) = initializer {
                    scan_body(std::slice::from_ref(s.as_ref()), inside, names);
                }
                if let Some(e) = condition {
                    scan_expr(e, names);
                }
                if let Some(s) = update {
                    scan_body(std::slice::from_ref(s.as_ref()), inside, names);
                }
                scan_body(body, inside, names);
            }
            _ => {}
        }
    }
}
/// Visita closures aninhadas e coleta também efeitos em argumentos e receptores.
fn scan_expr<'a>(e: &Expr<'a>, names: &mut HashSet<&'a str>) {
    match &e.kind {
        ExprKind::Await(value) => scan_expr(value, names),
        ExprKind::FutureValue {
            value: Some(value), ..
        } => scan_expr(value, names),
        ExprKind::FutureValue { value: None, .. } => {}
        ExprKind::FutureDelayed {
            duration,
            computation,
            ..
        } => {
            scan_expr(duration, names);
            if let Some(value) = computation {
                scan_expr(value, names);
            }
        }
        ExprKind::Duration { parts } => {
            for (_, part) in parts {
                scan_expr(part, names);
            }
        }
        ExprKind::Map { entries, .. } => {
            for (key, value) in entries {
                scan_expr(key, names);
                scan_expr(value, names);
            }
        }
        ExprKind::Cascade {
            receiver, sections, ..
        } => {
            scan_expr(receiver, names);
            scan_body(sections, false, names);
        }
        ExprKind::Record { fields } => {
            for (_, value) in fields {
                scan_expr(value, names);
            }
        }
        ExprKind::TypeTest { operand, .. } | ExprKind::Cast { operand, .. } => {
            scan_expr(operand, names)
        }
        ExprKind::Const(inner) => scan_expr(inner, names),
        ExprKind::Switch { scrutinee, arms } => {
            scan_expr(scrutinee, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    scan_expr(guard, names);
                }
                scan_expr(&arm.value, names);
            }
        }
        ExprKind::GenericCall { arguments, .. } => {
            for arg in arguments {
                scan_expr(arg, names);
            }
        }
        ExprKind::Closure { body, .. } => scan_body(body, true, names),
        ExprKind::Construct {
            arguments: elements,
            ..
        }
        | ExprKind::NamedConstruct {
            arguments: elements,
            ..
        }
        | ExprKind::List { elements, .. }
        | ExprKind::Call {
            arguments: elements,
            ..
        } => {
            for e in elements {
                scan_expr(e, names);
            }
        }
        ExprKind::Invoke { callee, arguments }
        | ExprKind::MethodCall {
            receiver: callee,
            arguments,
            ..
        } => {
            scan_expr(callee, names);
            for e in arguments {
                scan_expr(e, names);
            }
        }
        ExprKind::Index { receiver, index }
        | ExprKind::Binary {
            left: receiver,
            right: index,
            ..
        } => {
            scan_expr(receiver, names);
            scan_expr(index, names);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Throw(operand)
        | ExprKind::Member {
            receiver: operand, ..
        } => scan_expr(operand, names),
        ExprKind::Conditional {
            condition,
            then_value,
            else_value,
        } => {
            scan_expr(condition, names);
            scan_expr(then_value, names);
            scan_expr(else_value, names);
        }
        _ => {}
    }
}
