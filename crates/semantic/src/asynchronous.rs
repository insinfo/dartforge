//! Tipagem do recorte assíncrono; Future mantém uma camada explícita por await.
use super::*;
impl<'a> Validator<'a> {
    /// Nomes intrínsecos não atravessam declarações que ocultam a classe Future.
    pub(super) fn future_name(&self, span: Span) -> Result<(), Diagnostic> {
        if self.lookup("Future").is_some()
            || self.has_implicit_member("Future")
            || self.functions.contains_key("Future")
            || self.classes.values().any(|c| c.name == "Future")
        {
            return Err(Diagnostic::new("Declaration shadows Future", span));
        }
        Ok(())
    }
    /// Extrai o resultado declarado do corpo async sem aceitar assinaturas síncronas.
    pub(super) fn async_result(&self, ty: Type, span: Span) -> Result<Type, Diagnostic> {
        if ty == Type::Void {
            return Ok(Type::Void);
        }
        if let Some(TypeShape::Future(result)) = self.shape(ty) {
            return Ok(result);
        }
        Err(Diagnostic::new(
            "Async functions require Future<T> or void return type in this subset",
            span,
        ))
    }
    /// Await aceita valores escalares e retira somente a camada externa do Future.
    pub(super) fn await_type(&self, ty: Type) -> Type {
        let upper = self.upper_bound(ty);
        if let Some(TypeShape::Future(result)) = self.shape(upper) {
            return result;
        }
        if let Some(TypeShape::Nullable(inner)) = self.shape(upper)
            && let Some(TypeShape::Future(result)) = self.shape(inner)
        {
            return self.nullable(result);
        }
        ty
    }
    /// Valida a união de retorno FutureOr<T> sem apagar Futures que já constituem T.
    pub(super) fn future_or(
        &self,
        actual: Type,
        expected: Type,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.require_type(actual, expected, span).is_ok() {
            return Ok(());
        }
        if let Some(TypeShape::Future(inner)) = self.shape(actual) {
            return self.require_type(inner, expected, span);
        }
        self.require_type(actual, expected, span)
    }
    /// Retornos explícitos void não podem descartar valores como uma função callback void.
    pub(super) fn async_return(&self, value: &Expr<'a>, expected: Type) -> Result<(), Diagnostic> {
        let actual = self.expression_expected(
            value,
            if expected == Type::Void {
                None
            } else {
                Some(expected)
            },
        )?;
        if expected == Type::Void && self.in_arrow {
            return Ok(());
        }
        self.future_or(actual, expected, value.span)
    }
    /// Resolve Future.value com inferência escalar, contexto e adoção de Future<T>.
    pub(super) fn future_value(
        &self,
        value: Option<&Expr<'a>>,
        explicit: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        self.future_name(span)?;
        if explicit.is_none() && value.is_none() {
            return Err(Diagnostic::new(
                "Future.value without a value requires explicit or contextual type",
                span,
            ));
        }
        if let Some(t) = explicit {
            self.check_type_name(t, span)?;
        }
        let actual = if let Some(value) = value {
            self.expression_expected(value, explicit)?
        } else {
            Type::Null
        };
        let result = explicit.unwrap_or_else(|| self.await_type(actual));
        if result != Type::Void {
            self.future_or(actual, result, span)?;
        }
        Ok(self.intern(TypeShape::Future(result)))
    }
    /// Future.delayed exige duração e callback sem argumentos; ausência produz null.
    pub(super) fn future_delayed(
        &self,
        duration: &Expr<'a>,
        computation: Option<&Expr<'a>>,
        explicit: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        self.future_name(span)?;
        if explicit.is_none() && computation.is_none() {
            return Err(Diagnostic::new(
                "Future.delayed without computation requires explicit or contextual type",
                span,
            ));
        }
        self.require_type(self.value(duration)?, Type::Duration, duration.span)?;
        if let Some(t) = explicit {
            self.check_type_name(t, span)?;
        }
        let result = if let Some(callback) = computation {
            let ty = self.value(callback)?;
            let actual = self.invoke(ty, &[], callback.span)?;
            let result = explicit.unwrap_or_else(|| self.await_type(actual));
            if result != Type::Void {
                self.future_or(actual, result, callback.span)?;
            }
            result
        } else {
            let result = explicit.unwrap_or(Type::Null);
            if result != Type::Void {
                self.require_type(Type::Null, result, span)?;
            }
            result
        };
        Ok(self.intern(TypeShape::Future(result)))
    }
    /// Rotinas de dart:async são intrínsecas somente quando nenhum nome léxico as oculta.
    pub(super) fn async_builtin(
        &self,
        name: &str,
        args: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if !self.async_library {
            return Err(Diagnostic::new(
                "Async scheduling requires dart:async",
                span,
            ));
        }
        self.resolution
            .borrow_mut()
            .async_builtins
            .insert((span.start, span.end));
        let (callback, result) = if name == "Timer" {
            if args.len() != 2 {
                return Err(Diagnostic::new(
                    "Timer requires duration and callback",
                    span,
                ));
            }
            self.require_type(self.value(&args[0])?, Type::Duration, args[0].span)?;
            (&args[1], Type::Timer)
        } else {
            if args.len() != 1 {
                return Err(Diagnostic::new(
                    "scheduleMicrotask requires one callback",
                    span,
                ));
            }
            (&args[0], Type::Void)
        };
        let expected = self.intern(TypeShape::Function {
            result: Type::Void,
            parameters: vec![],
        });
        self.require_type(
            self.value_expected(callback, Some(expected))?,
            expected,
            callback.span,
        )?;
        Ok(result)
    }
}
