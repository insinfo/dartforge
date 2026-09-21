//! Validação do controle de fluxo: try/catch/finally, throw, assert, for-in,
//! rótulos e o operador condicional.
//!
//! Todo o estado extra que estas formas precisam é barato de clonar: a pilha de
//! rótulos é um `Vec<&str>` vazio no caminho comum — um `Vec` vazio não aloca —
//! e a profundidade de `catch` é um contador. O `Validator` é copiado a cada
//! ramificação de fluxo, então nenhuma tabela nova pode entrar nele.
use super::*;
use dartforge_syntax::CatchClause;

/// Indica que a expressão é um `throw`, cujo valor nunca chega a existir.
///
/// Serve para tipar `c ? throw e : v` e `a ?? (throw e)` com o tipo do outro
/// lado, do mesmo modo que Dart usa `Never` como subtipo de tudo.
pub(super) fn is_throw(expression: &Expr<'_>) -> bool {
    matches!(expression.kind, ExprKind::Throw(_))
}

impl<'a> Validator<'a> {
    /// Valida `try`, suas cláusulas e o bloco final, sem deixar promoções vazarem.
    ///
    /// O corpo pode ser interrompido em qualquer instrução, então nada que ele
    /// promova vale nas cláusulas nem depois do `try`. As escritas do corpo e
    /// das cláusulas invalidam promoções externas, exatamente como num laço.
    /// O `finally` roda sempre e por isso é analisado no estado que segue.
    pub(super) fn try_statement(
        &mut self,
        body: &[Statement<'a>],
        catches: &[CatchClause<'a>],
        finally_body: Option<&Vec<Statement<'a>>>,
    ) -> Result<(), Diagnostic> {
        let before = self.clone();
        self.block(body)?;
        *self = before;
        self.invalidate_writes(body);
        for clause in catches {
            let mut branch = self.clone();
            branch.catch_depth += 1;
            let mut scope = HashMap::new();
            let caught = match clause.exception_type {
                Some(ty) => {
                    branch.runtime_type(ty, clause.span)?;
                    ty
                }
                None => Type::Object,
            };
            if let Some(name) = clause.exception
                && !is_wildcard(name)
            {
                scope.insert(
                    name,
                    Binding {
                        constant: None,
                        ty: Some(caught),
                        is_final: true,
                        promoted: None,
                    },
                );
            }
            if let Some(name) = clause.stack_trace
                && !is_wildcard(name)
                && scope
                    .insert(
                        name,
                        Binding {
                            constant: None,
                            // StackTrace de Dart não existe neste subconjunto;
                            // o rastro é opaco e só satisfaz Object.
                            ty: Some(Type::Object),
                            is_final: true,
                            promoted: None,
                        },
                    )
                    .is_some()
            {
                return Err(Diagnostic::new(
                    "Duplicate catch variable name",
                    clause.span,
                ));
            }
            branch.scopes.push(scope);
            branch.block(&clause.body)?;
            self.invalidate_writes(&clause.body);
        }
        if let Some(finally_body) = finally_body {
            self.block(finally_body)?;
        }
        Ok(())
    }
    /// Valida `for (final x in iterável)`; o iterável é avaliado uma única vez.
    pub(super) fn for_in(
        &mut self,
        name: &'a str,
        annotation: Option<Type>,
        is_final: bool,
        iterable: &Expr<'a>,
        body: &[Statement<'a>],
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.invalidate_writes(body);
        let source = self.value(iterable)?;
        let element = self
            .element(source)
            .ok_or_else(|| Diagnostic::new("for-in requires a List or Iterable", iterable.span))?;
        let declared = match annotation {
            Some(expected) => {
                self.check_type_name(expected, span)?;
                self.require_type(element, expected, iterable.span)?;
                expected
            }
            None => element,
        };
        if declared == Type::Void || declared == Type::Inferred {
            return Err(Diagnostic::new("Unsupported for-in element type", span));
        }
        let before = self.clone();
        let mut scope = HashMap::new();
        if !is_wildcard(name) {
            scope.insert(
                name,
                Binding {
                    constant: None,
                    ty: Some(declared),
                    is_final,
                    promoted: None,
                },
            );
        }
        self.scopes.push(scope);
        let result = self.loop_body(body);
        self.scopes.pop();
        result?;
        *self = before;
        self.invalidate_writes(body);
        Ok(())
    }
    /// Tipa o operador condicional promovendo cada ramo pelo valor da condição.
    ///
    /// As promoções valem apenas dentro do respectivo ramo: a junção não herda
    /// nenhuma delas, o que mantém a análise conservadora como nos `if`.
    pub(super) fn conditional(
        &self,
        condition: &Expr<'a>,
        then_value: &Expr<'a>,
        else_value: &Expr<'a>,
        expected: Option<Type>,
        span: Span,
    ) -> Result<Type, Diagnostic> {
        self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
        let mut yes = self.clone();
        yes.promote(condition, true);
        let mut no = self.clone();
        no.promote(condition, false);
        let left = yes.value_expected(then_value, expected)?;
        let right = no.value_expected(else_value, expected)?;
        match (is_throw(then_value), is_throw(else_value)) {
            (true, true) => Ok(expected.unwrap_or(Type::Object)),
            (true, false) => Ok(right),
            (false, true) => Ok(left),
            (false, false) => self.common(left, right, span),
        }
    }
    /// Valida o operando de `throw`; Dart proíbe lançar um valor possivelmente nulo.
    pub(super) fn throw_expression(
        &self,
        value: &Expr<'a>,
        expected: Option<Type>,
    ) -> Result<Type, Diagnostic> {
        let ty = self.value(value)?;
        if ty == Type::Null || self.may_be_null(ty) {
            return Err(Diagnostic::new(
                "Cannot throw a value that may be null",
                value.span,
            ));
        }
        Ok(expected.unwrap_or(Type::Object))
    }
    /// Valida `assert(condição)` e `assert(condição, mensagem)`.
    ///
    /// A condição não promove nada: a asserção é uma verificação, não um teste
    /// de fluxo, e manter a junção conservadora evita depender da política de
    /// emissão descrita em `docs/FLUXO.md`.
    pub(super) fn assert_statement(
        &self,
        condition: &Expr<'a>,
        message: Option<&Expr<'a>>,
    ) -> Result<(), Diagnostic> {
        self.require_type(self.value(condition)?, Type::Bool, condition.span)?;
        if let Some(message) = message {
            self.require_type(self.value(message)?, Type::String, message.span)?;
        }
        Ok(())
    }
}
