//! Conjuntos, espalhamentos, elementos `if`/`for` e cadeias null-aware.
//!
//! Este módulo concentra as formas de literal de coleção que produzem zero ou
//! mais elementos e o curto-circuito de `?.`/`?[`. A regra que atravessa tudo
//! aqui é a ordem de avaliação do Dart: da esquerda para a direita, com cada
//! operando avaliado exatamente uma vez. Nenhuma subexpressão é analisada duas
//! vezes nem reescrita; o que varia por forma é apenas o tipo contribuído.
use super::*;

impl<'a> Validator<'a> {
    /// Analisa um literal de conjunto, inferindo o elemento como Dart 3.6.2.
    ///
    /// O contexto vem, nesta ordem, do argumento de tipo escrito e do tipo
    /// esperado; sem nenhum dos dois, o elemento é a junção dos elementos
    /// escritos, e um `<T>{}` vazio sem contexto é recusado.
    pub(super) fn set_expression(
        &self,
        element_type: Option<Type>,
        elements: &[Expr<'a>],
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let context = element_type.or_else(|| expected.and_then(|t| self.element(t)));
        if let Some(t) = element_type {
            self.check_type_name(t, span)?;
        }
        let mut inferred = context;
        for value in elements {
            let actual = self.sequence_element(value, context)?;
            if let Some(target) = context {
                self.require_type(actual, target, value.span)?;
            }
            inferred = Some(match inferred {
                Some(previous) if context.is_none() => self.common(previous, actual, value.span)?,
                Some(t) => t,
                None => actual,
            });
        }
        let element = inferred.ok_or_else(|| {
            Diagnostic::new("Empty Set requires explicit element type or context", span)
        })?;
        if element == Type::Void || element == Type::Inferred {
            return Err(Diagnostic::new("Unsupported set element type", span));
        }
        Ok(self.intern(TypeShape::Set(element)))
    }
    /// Tipo que um elemento de literal de lista ou conjunto contribui.
    ///
    /// Um valor solto contribui o próprio tipo; `?valor` contribui o tipo sem
    /// null; `...`/`...?` contribuem o elemento do operando; `if` contribui a
    /// junção dos ramos escritos; `for` contribui o tipo do elemento repetido.
    pub(super) fn sequence_element(
        &self,
        e: &Expr<'a>,
        context: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        let ty = match &e.kind {
            ExprKind::Spread {
                operand,
                null_aware,
            } => self.spread_sequence(operand, *null_aware, context, e.span)?,
            ExprKind::CollectionIf {
                condition,
                then_element,
                else_element,
            } => {
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
                let mut branch = self.clone();
                branch.promote(condition, true);
                let then_type = branch.sequence_element(then_element, context)?;
                match else_element {
                    None => then_type,
                    Some(other) => {
                        let mut otherwise = self.clone();
                        otherwise.promote(condition, false);
                        let else_type = otherwise.sequence_element(other, context)?;
                        match context {
                            Some(target) => target,
                            None => self.common(then_type, else_type, e.span)?,
                        }
                    }
                }
            }
            ExprKind::CollectionFor { header, element } => {
                self.with_for_header(header, |nested| nested.sequence_element(element, context))?
            }
            _ => return self.collection_element(e, context),
        };
        self.resolution
            .borrow_mut()
            .expr_types
            .insert((e.span.start, e.span.end), ty);
        Ok(ty)
    }
    /// Tipo do elemento produzido por `...`/`...?` numa lista ou conjunto.
    ///
    /// Como nos SDKs 3.6.2 e 3.13.4, `...` exige operando não anulável: um
    /// operando anulável é erro estático, e não um lançamento adiado. `...?`
    /// aceita o anulável e não acrescenta nada quando o valor é null.
    fn spread_sequence(
        &self,
        operand: &Expr<'a>,
        null_aware: bool,
        context: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        let expected = context.map(|element| {
            let sequence = self.intern(TypeShape::Iterable(element));
            if null_aware {
                self.nullable(sequence)
            } else {
                sequence
            }
        });
        let actual = self.value_expected(operand, expected)?;
        let source = self.spread_source(actual, null_aware, operand.span)?;
        self.element(source)
            .ok_or_else(|| Diagnostic::new("Spread requires a List, Set or Iterable", span))
    }
    /// Remove a nulabilidade aceita por `...?` e recusa a que `...` não admite.
    fn spread_source(
        &self,
        actual: Type,
        null_aware: bool,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if self.may_be_null(actual) && !null_aware {
            return Err(Diagnostic::new(
                "A nullable expression cannot be spread; use `...?`",
                span,
            ));
        }
        Ok(self.without_null(actual))
    }
    /// Par `(chave, valor)` que um elemento de literal de mapa contribui.
    pub(super) fn map_element(
        &self,
        e: &Expr<'a>,
        key: Type,
        value: Option<Type>,
    ) -> Result<(Type, Type), Diagnostic> {
        match &e.kind {
            ExprKind::MapEntry {
                key: written,
                value: entry,
            } => {
                let actual_key = self.collection_element(written, Some(key))?;
                self.require_type(actual_key, key, written.span)?;
                let actual = self.collection_element(entry, value)?;
                if let Some(expected) = value {
                    self.require_type(actual, expected, entry.span)?;
                }
                Ok((actual_key, actual))
            }
            ExprKind::Spread {
                operand,
                null_aware,
            } => {
                let expected = value.map(|value| {
                    let map = self.intern(TypeShape::Map { key, value });
                    if *null_aware { self.nullable(map) } else { map }
                });
                let actual = self.value_expected(operand, expected)?;
                let source = self.spread_source(actual, *null_aware, operand.span)?;
                match self.shape(self.upper_bound(source)) {
                    Some(TypeShape::Map {
                        key: actual_key,
                        value: actual_value,
                    }) => {
                        self.require_type(actual_key, key, operand.span)?;
                        if let Some(expected) = value {
                            self.require_type(actual_value, expected, operand.span)?;
                        }
                        Ok((actual_key, actual_value))
                    }
                    _ => Err(Diagnostic::new("Spread in a map requires a Map", e.span)),
                }
            }
            ExprKind::CollectionIf {
                condition,
                then_element,
                else_element,
            } => {
                self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
                let mut branch = self.clone();
                branch.promote(condition, true);
                let (then_key, then_value) = branch.map_element(then_element, key, value)?;
                match else_element {
                    None => Ok((then_key, then_value)),
                    Some(other) => {
                        let mut otherwise = self.clone();
                        otherwise.promote(condition, false);
                        let (else_key, else_value) = otherwise.map_element(other, key, value)?;
                        Ok((
                            self.common(then_key, else_key, e.span)?,
                            match value {
                                Some(target) => target,
                                None => self.common(then_value, else_value, e.span)?,
                            },
                        ))
                    }
                }
            }
            ExprKind::CollectionFor { header, element } => {
                self.with_for_header(header, |nested| nested.map_element(element, key, value))
            }
            _ => Err(Diagnostic::new(
                "Map literals require `key: value` entries",
                e.span,
            )),
        }
    }
    /// Analisa o cabeçalho de um `for` de literal num escopo próprio.
    ///
    /// Reaproveita as regras das instruções `for` e `for-in`: a ligação do
    /// cabeçalho só existe dentro do elemento, a condição precisa ser `bool` e
    /// a atualização é analisada depois do elemento, como numa iteração real.
    /// O `Validator` é clonado uma vez por `for` escrito — nunca por elemento
    /// produzido — porque o cabeçalho declara nomes que não podem escapar.
    fn with_for_header<T>(
        &self,
        header: &Statement<'a>,
        element: impl FnOnce(&Validator<'a>) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        let mut nested = self.clone();
        match &header.kind {
            StatementKind::ForIn {
                is_final,
                name,
                annotation,
                iterable,
                ..
            } => {
                let source = nested.value(iterable)?;
                let declared = nested.element(source).ok_or_else(|| {
                    Diagnostic::new("for-in requires a List or Iterable", iterable.span)
                })?;
                let declared = match annotation {
                    Some(expected) => {
                        nested.check_type_name(*expected, header.span)?;
                        nested.require_type(declared, *expected, iterable.span)?;
                        *expected
                    }
                    None => declared,
                };
                if declared == Type::Void || declared == Type::Inferred {
                    return Err(Diagnostic::new(
                        "Unsupported for-in element type",
                        header.span,
                    ));
                }
                let mut scope = HashMap::new();
                if !is_wildcard(name) {
                    scope.insert(
                        *name,
                        Binding {
                            constant: None,
                            ty: Some(declared),
                            is_final: *is_final,
                            is_late: false,
                            promoted: None,
                        },
                    );
                }
                nested.scopes.push(scope);
                element(&nested)
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                ..
            } => {
                let mut scope = HashMap::new();
                if let Some(Statement {
                    kind:
                        StatementKind::Variable {
                            name,
                            is_final,
                            is_const,
                            ..
                        },
                    ..
                }) = initializer.as_deref()
                    && !is_wildcard(name)
                {
                    scope.insert(
                        *name,
                        Binding {
                            constant: None,
                            ty: None,
                            is_final: *is_final || *is_const,
                            is_late: false,
                            promoted: None,
                        },
                    );
                }
                nested.scopes.push(scope);
                if let Some(initializer) = initializer {
                    nested.statement(initializer)?;
                }
                if let Some(update) = update {
                    nested.invalidate_writes(std::slice::from_ref(update));
                }
                if let Some(condition) = condition {
                    nested.require_type(nested.value(condition)?, Type::Bool, condition.span)?;
                    nested.promote(condition, true);
                }
                let result = element(&nested)?;
                if let Some(update) = update {
                    nested.statement(update)?;
                }
                Ok(result)
            }
            _ => Err(Diagnostic::new(
                "Unsupported for header in a collection literal",
                header.span,
            )),
        }
    }
    /// Analisa `a?.b`, `a?.b()` e `a?[i]`, com curto-circuito de toda a cadeia.
    ///
    /// O receptor é avaliado uma única vez: seu tipo sem null é gravado no span
    /// do operador, que é a chave do receptor sintético da cadeia. O resultado
    /// é anulável mesmo quando o seletor final não é, porque a cadeia inteira
    /// produz null quando o receptor é null. Um seletor `void` continua `void`:
    /// `a?.m();` é instrução válida e não produz valor.
    pub(super) fn null_short(
        &self,
        receiver: &Expr<'a>,
        chain: &Expr<'a>,
        target: Span,
    ) -> Result<Type, Diagnostic> {
        let actual = self.value(receiver)?;
        if actual == Type::Null {
            return Err(Diagnostic::new(
                "Null-aware access on a null-only value is unsupported",
                receiver.span,
            ));
        }
        if !self.may_be_null(actual) {
            return Err(Diagnostic::new(
                "Null-aware access requires a nullable receiver",
                receiver.span,
            ));
        }
        let non_null = self.without_null(actual);
        self.resolution
            .borrow_mut()
            .expr_types
            .insert((target.start, target.end), non_null);
        let result = self.expression_expected(chain, None)?;
        if result == Type::Void {
            return Ok(Type::Void);
        }
        Ok(self.nullable(result))
    }
    /// Recupera o tipo do receptor sintético gravado pela cadeia envolvente.
    pub(super) fn null_short_target(&self, span: Span) -> Result<Type, Diagnostic> {
        self.resolution
            .borrow()
            .expr_types
            .get(&(span.start, span.end))
            .copied()
            .ok_or_else(|| Diagnostic::new("Null-aware target outside a null-aware chain", span))
    }
}
