//! Resolve nomes, escopos e tipos do subconjunto de Dart 3.6.2.
//! Rejeita programas incompatíveis antes da geração de JavaScript.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Program, Statement, StatementKind, Type, UnaryOp,
};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Binding {
    ty: Option<Type>,
    is_final: bool,
}
#[derive(Clone)]
struct Signature {
    parameters: Vec<Type>,
    result: Type,
}

struct Validator<'a> {
    scopes: Vec<HashMap<&'a str, Binding>>,
    functions: HashMap<&'a str, Signature>,
    return_type: Type,
    loop_depth: usize,
}

/// Valida nomes, tipos, chamadas, retornos e controle de laços antes da geração de código.
///
/// # Exemplos
/// ```
/// use dartforge_syntax::Program;
/// let programa = Program { functions: vec![], statements: vec![] };
/// assert!(dartforge_semantic::validate(&programa).is_ok());
/// ```
///
/// # Erros
/// Retorna o primeiro diagnóstico de nome desconhecido, incompatibilidade de tipos,
/// retorno ausente ou controle de fluxo inválido. A análise conservadora de retorno
/// não considera laços, mesmo infinitos, como prova de retorno obrigatório.
pub fn validate(program: &Program<'_>) -> Result<(), Diagnostic> {
    let mut validator = Validator {
        scopes: Vec::new(),
        functions: HashMap::new(),
        return_type: Type::Void,
        loop_depth: 0,
    };
    validator.functions.insert(
        "main",
        Signature {
            parameters: vec![],
            result: Type::Void,
        },
    );
    for function in &program.functions {
        if function.name == "print" {
            return Err(Diagnostic::new(
                "Declaring a top-level function named 'print' is unsupported",
                function.span,
            ));
        }
        if validator
            .functions
            .insert(
                function.name,
                Signature {
                    parameters: function
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect(),
                    result: function.return_type,
                },
            )
            .is_some()
        {
            return Err(Diagnostic::new(
                format!("Duplicate function '{}'", function.name),
                function.span,
            ));
        }
    }
    for function in &program.functions {
        validator.check_type_name(function.return_type, function.span)?;
        let mut parameters = HashMap::new();
        for parameter in &function.parameters {
            validator.check_type_name(parameter.ty, parameter.span)?;
            if parameter.ty == Type::Void {
                return Err(Diagnostic::new(
                    "Void parameters are unsupported",
                    parameter.span,
                ));
            }
            if parameters
                .insert(
                    parameter.name,
                    Binding {
                        ty: Some(parameter.ty),
                        is_final: false,
                    },
                )
                .is_some()
            {
                return Err(Diagnostic::new(
                    format!("Duplicate parameter '{}'", parameter.name),
                    parameter.span,
                ));
            }
        }
        validator.scopes.push(parameters);
        validator.return_type = function.return_type;
        validator.loop_depth = 0;
        validator.block(&function.body)?;
        validator.scopes.pop();
        if function.return_type != Type::Void && !definitely_returns(&function.body) {
            return Err(Diagnostic::new(
                format!(
                    "Function '{}' may complete without returning a value",
                    function.name
                ),
                function.span,
            ));
        }
    }
    validator.return_type = Type::Void;
    validator.loop_depth = 0;
    validator.block(&program.statements)
}
impl<'a> Validator<'a> {
    /// Procura o nome do escopo mais interno até o mais externo.
    fn lookup(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
    /// Exige que o nome exista e tenha concluído sua inicialização.
    fn initialized(&self, name: &str, span: Span) -> Result<Binding, Diagnostic> {
        let binding = self
            .lookup(name)
            .ok_or_else(|| Diagnostic::new(format!("Unknown identifier '{name}'"), span))?;
        if binding.ty.is_none() {
            return Err(Diagnostic::new(
                format!(
                    "Local variable '{name}' is referenced before its declaration or in its own initializer"
                ),
                span,
            ));
        }
        Ok(binding)
    }
    /// Pré-declara os locais e valida as instruções em um novo escopo.
    fn block(&mut self, statements: &[Statement<'a>]) -> Result<(), Diagnostic> {
        let mut scope = HashMap::new();
        // Locais ocultam nomes externos em todo o bloco, inclusive antes da declaração.
        for statement in statements {
            if let StatementKind::Variable { name, is_final, .. } = statement.kind
                && scope.insert(name, Binding { ty: None, is_final }).is_some()
            {
                return Err(Diagnostic::new(
                    format!("Duplicate local variable '{name}'"),
                    statement.span,
                ));
            }
        }
        self.scopes.push(scope);
        let result = statements
            .iter()
            .try_for_each(|statement| self.statement(statement));
        self.scopes.pop();
        result
    }
    /// Valida uma instrução e seus efeitos sobre o escopo semântico.
    fn statement(&mut self, statement: &Statement<'a>) -> Result<(), Diagnostic> {
        match &statement.kind {
            StatementKind::Variable {
                name,
                annotation,
                initializer,
                ..
            } => {
                let actual = self.value(initializer)?;
                if let Some(expected) = annotation {
                    self.check_type_name(*expected, statement.span)?;
                    require_type(actual, *expected, initializer.span)?;
                }
                self.scopes
                    .last_mut()
                    .expect("current block scope")
                    .get_mut(name)
                    .expect("predeclared local")
                    .ty = Some(actual);
                Ok(())
            }
            StatementKind::Assign { name, value } => {
                let binding = self.initialized(name, statement.span)?;
                if binding.is_final {
                    return Err(Diagnostic::new(
                        format!("Cannot assign to final variable '{name}'"),
                        statement.span,
                    ));
                }
                require_type(
                    self.value(value)?,
                    binding.ty.expect("initialized binding"),
                    value.span,
                )
            }
            StatementKind::Print(expression) => {
                if self.lookup("print").is_some() {
                    return Err(Diagnostic::new(
                        "Invocation of local 'print' is unsupported; it shadows the built-in function",
                        statement.span,
                    ));
                }
                self.value(expression).map(|_| ())
            }
            StatementKind::Return(value) => match (self.return_type, value) {
                (Type::Void, None) => Ok(()),
                (Type::Void, Some(expression)) => {
                    require_type(self.expression(expression)?, Type::Void, expression.span)
                }
                (_, None) => Err(Diagnostic::new(
                    "A value must be returned from this function",
                    statement.span,
                )),
                (expected, Some(expression)) => {
                    require_type(self.value(expression)?, expected, expression.span)
                }
            },
            StatementKind::Expression(expression) => self.expression(expression).map(|_| ()),
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                require_type(self.value(condition)?, Type::Bool, condition.span)?;
                self.block(then_body)?;
                if let Some(body) = else_body {
                    self.block(body)?;
                }
                Ok(())
            }
            StatementKind::While { condition, body } => {
                require_type(self.value(condition)?, Type::Bool, condition.span)?;
                self.loop_body(body)
            }
            StatementKind::DoWhile { body, condition } => {
                self.loop_body(body)?;
                require_type(self.value(condition)?, Type::Bool, condition.span)
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => self.for_loop(
                initializer.as_deref(),
                condition.as_ref(),
                update.as_deref(),
                body,
            ),
            StatementKind::Break | StatementKind::Continue => {
                if self.loop_depth == 0 {
                    Err(Diagnostic::new(
                        "break and continue require an enclosing loop",
                        statement.span,
                    ))
                } else {
                    Ok(())
                }
            }
            StatementKind::Block(statements) => self.block(statements),
        }
    }
    /// Valida o corpo em escopo próprio e restaura a profundidade inclusive em caso de erro.
    fn loop_body(&mut self, body: &[Statement<'a>]) -> Result<(), Diagnostic> {
        self.loop_depth += 1;
        let result = self.block(body);
        self.loop_depth -= 1;
        result
    }
    /// Mantém o inicializador visível à condição, atualização e corpo do laço clássico.
    fn for_loop(
        &mut self,
        initializer: Option<&Statement<'a>>,
        condition: Option<&Expr<'a>>,
        update: Option<&Statement<'a>>,
        body: &[Statement<'a>],
    ) -> Result<(), Diagnostic> {
        if let Some(initializer) = initializer
            && !matches!(
                initializer.kind,
                StatementKind::Variable { .. }
                    | StatementKind::Assign { .. }
                    | StatementKind::Expression(_)
            )
        {
            return Err(Diagnostic::new(
                "Unsupported for initializer statement",
                initializer.span,
            ));
        }
        if let Some(update) = update
            && !matches!(
                update.kind,
                StatementKind::Assign { .. } | StatementKind::Expression(_)
            )
        {
            return Err(Diagnostic::new(
                "Unsupported for update statement",
                update.span,
            ));
        }
        let mut scope = HashMap::new();
        if let Some(Statement {
            kind: StatementKind::Variable { name, is_final, .. },
            ..
        }) = initializer
        {
            scope.insert(
                *name,
                Binding {
                    ty: None,
                    is_final: *is_final,
                },
            );
        }
        self.scopes.push(scope);
        let result = (|| {
            if let Some(initializer) = initializer {
                self.statement(initializer)?;
            }
            if let Some(condition) = condition {
                require_type(self.value(condition)?, Type::Bool, condition.span)?;
            }
            if let Some(update) = update {
                self.statement(update)?;
            }
            self.loop_body(body)
        })();
        self.scopes.pop();
        result
    }
    /// Rejeita anotações cujo nome de tipo foi ocultado por uma declaração.
    fn check_type_name(&self, ty: Type, span: Span) -> Result<(), Diagnostic> {
        let name = match ty {
            Type::Void => return Ok(()),
            Type::Int => "int",
            Type::String => "String",
            Type::Bool => "bool",
        };
        if self.lookup(name).is_some() || self.functions.contains_key(name) {
            Err(Diagnostic::new(
                format!(
                    "Declaration '{name}' shadows the type name; using it as a type is unsupported"
                ),
                span,
            ))
        } else {
            Ok(())
        }
    }
    /// Exige uma expressão que produza um valor diferente de void.
    fn value(&self, expression: &Expr<'a>) -> Result<Type, Diagnostic> {
        let ty = self.expression(expression)?;
        if ty == Type::Void {
            Err(Diagnostic::new(
                "A void expression cannot be used as a value",
                expression.span,
            ))
        } else {
            Ok(ty)
        }
    }
    /// Determina o tipo da expressão e valida operadores e chamadas.
    fn expression(&self, expression: &Expr<'a>) -> Result<Type, Diagnostic> {
        match &expression.kind {
            ExprKind::Int(_) => Ok(Type::Int),
            ExprKind::String(_) | ExprKind::OwnedString(_) => Ok(Type::String),
            ExprKind::Bool(_) => Ok(Type::Bool),
            ExprKind::Identifier(name) => Ok(self
                .initialized(name, expression.span)?
                .ty
                .expect("initialized binding")),
            ExprKind::Call { name, arguments } => {
                if self.lookup(name).is_some() {
                    return Err(Diagnostic::new(
                        format!(
                            "Calling local '{name}' is unsupported; it shadows a function name"
                        ),
                        expression.span,
                    ));
                }
                if *name == "print" {
                    if arguments.len() != 1 {
                        return Err(Diagnostic::new(
                            "print expects one argument",
                            expression.span,
                        ));
                    }
                    self.value(&arguments[0])?;
                    return Ok(Type::Void);
                }
                let signature = self.functions.get(name).ok_or_else(|| {
                    Diagnostic::new(format!("Unknown function '{name}'"), expression.span)
                })?;
                if arguments.len() != signature.parameters.len() {
                    return Err(Diagnostic::new(
                        format!(
                            "Function '{name}' expects {} arguments, received {}",
                            signature.parameters.len(),
                            arguments.len()
                        ),
                        expression.span,
                    ));
                }
                for (argument, expected) in arguments.iter().zip(&signature.parameters) {
                    require_type(self.value(argument)?, *expected, argument.span)?;
                }
                Ok(signature.result)
            }
            ExprKind::Unary { op, operand } => {
                let expected = match op {
                    UnaryOp::Negate => Type::Int,
                    UnaryOp::Not => Type::Bool,
                };
                require_type(self.value(operand)?, expected, operand.span)?;
                Ok(expected)
            }
            ExprKind::Binary { op, left, right } => {
                let lhs = self.value(left)?;
                let rhs = self.value(right)?;
                match op {
                    BinaryOp::Equal | BinaryOp::NotEqual => Ok(Type::Bool),
                    BinaryOp::Add => {
                        if matches!(lhs, Type::Int | Type::String) && lhs == rhs {
                            Ok(lhs)
                        } else {
                            Err(Diagnostic::new(
                                "Operator '+' requires two int operands or two String operands",
                                expression.span,
                            ))
                        }
                    }
                    BinaryOp::Subtract | BinaryOp::Multiply => {
                        require_type(lhs, Type::Int, left.span)?;
                        require_type(rhs, Type::Int, right.span)?;
                        Ok(Type::Int)
                    }
                    BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        require_type(lhs, Type::Int, left.span)?;
                        require_type(rhs, Type::Int, right.span)?;
                        Ok(Type::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or => {
                        require_type(lhs, Type::Bool, left.span)?;
                        require_type(rhs, Type::Bool, right.span)?;
                        Ok(Type::Bool)
                    }
                }
            }
        }
    }
}
/// Prova retorno apenas por return, bloco ou ambas as alternativas de if; laços não provam retorno.
fn definitely_returns(statements: &[Statement<'_>]) -> bool {
    statements.iter().any(|statement| match &statement.kind {
        StatementKind::Return(_) => true,
        StatementKind::Block(body) => definitely_returns(body),
        StatementKind::If {
            then_body,
            else_body: Some(else_body),
            ..
        } => definitely_returns(then_body) && definitely_returns(else_body),
        _ => false,
    })
}
/// Compara tipos e associa a incompatibilidade ao trecho indicado.
fn require_type(actual: Type, expected: Type, span: Span) -> Result<(), Diagnostic> {
    if actual == expected {
        Ok(())
    } else {
        Err(Diagnostic::new(
            format!("Type mismatch: expected {expected:?}, found {actual:?}"),
            span,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SPAN: Span = Span { start: 4, end: 9 };
    /// Constrói uma expressão para os testes semânticos.
    fn expr(kind: ExprKind<'static>) -> Expr<'static> {
        Expr { kind, span: SPAN }
    }
    /// Constrói um literal inteiro para os testes.
    fn int() -> Expr<'static> {
        expr(ExprKind::Int(1))
    }
    /// Constrói uma referência a nome para os testes.
    fn id(name: &'static str) -> Expr<'static> {
        expr(ExprKind::Identifier(name))
    }
    /// Constrói uma instrução com posição conhecida.
    fn stmt(kind: StatementKind<'static>) -> Statement<'static> {
        Statement { kind, span: SPAN }
    }
    /// Constrói uma declaração local mutável sem anotação.
    fn local(name: &'static str, initializer: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Variable {
            name,
            annotation: None,
            is_final: false,
            initializer,
        })
    }
    /// Constrói uma chamada de impressão para os testes.
    fn print(value: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Print(value))
    }
    /// Valida um corpo de main sem funções auxiliares.
    fn check(statements: Vec<Statement<'static>>) -> Result<(), Diagnostic> {
        validate(&Program {
            functions: vec![],
            statements,
        })
    }
    /// Constrói uma operação binária para os testes.
    fn binary(op: BinaryOp, left: Expr<'static>, right: Expr<'static>) -> Expr<'static> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    /// Constrói uma chamada com argumentos posicionais.
    fn call(name: &'static str, arguments: Vec<Expr<'static>>) -> Expr<'static> {
        expr(ExprKind::Call { name, arguments })
    }
    /// Constrói uma função tipada para os testes.
    fn function(
        name: &'static str,
        result: Type,
        parameters: Vec<(&'static str, Type)>,
        body: Vec<Statement<'static>>,
    ) -> dartforge_syntax::Function<'static> {
        dartforge_syntax::Function {
            name,
            return_type: result,
            parameters: parameters
                .into_iter()
                .map(|(name, ty)| dartforge_syntax::Parameter {
                    name,
                    ty,
                    span: SPAN,
                })
                .collect(),
            body,
            span: SPAN,
        }
    }
    /// Constrói uma instrução de retorno com valor.
    fn ret(value: Expr<'static>) -> Statement<'static> {
        stmt(StatementKind::Return(Some(value)))
    }
    /// Constrói um laço clássico com inicializador local e condição booleana.
    fn for_statement(
        initializer: Statement<'static>,
        update: Option<Statement<'static>>,
        body: Vec<Statement<'static>>,
    ) -> Statement<'static> {
        stmt(StatementKind::For {
            initializer: Some(Box::new(initializer)),
            condition: Some(expr(ExprKind::Bool(true))),
            update: update.map(Box::new),
            body,
        })
    }
    /// Verifica controle de laços aninhados e restauração da profundidade fora deles.
    #[test]
    fn loop_control_requires_enclosing_loop() {
        for kind in [StatementKind::Break, StatementKind::Continue] {
            assert!(check(vec![stmt(kind)]).is_err());
        }
        assert!(
            check(vec![stmt(StatementKind::While {
                condition: expr(ExprKind::Bool(true)),
                body: vec![
                    stmt(StatementKind::DoWhile {
                        body: vec![stmt(StatementKind::Continue)],
                        condition: expr(ExprKind::Bool(false))
                    }),
                    stmt(StatementKind::Break),
                ]
            })])
            .is_ok()
        );
        assert!(
            check(vec![
                stmt(StatementKind::While {
                    condition: expr(ExprKind::Bool(true)),
                    body: vec![]
                }),
                stmt(StatementKind::Break)
            ])
            .is_err()
        );
        assert!(
            validate(&Program {
                functions: vec![
                    function(
                        "f",
                        Type::Void,
                        vec![],
                        vec![stmt(StatementKind::While {
                            condition: expr(ExprKind::Bool(true)),
                            body: vec![stmt(StatementKind::Break)]
                        })]
                    ),
                    function("g", Type::Void, vec![], vec![stmt(StatementKind::Continue)])
                ],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Verifica condições booleanas em todas as formas de laço.
    #[test]
    fn loop_conditions_require_bool() {
        for kind in [
            StatementKind::While {
                condition: int(),
                body: vec![],
            },
            StatementKind::DoWhile {
                condition: int(),
                body: vec![],
            },
            StatementKind::For {
                initializer: None,
                condition: Some(int()),
                update: None,
                body: vec![],
            },
        ] {
            assert!(check(vec![stmt(kind)]).is_err());
        }
        assert!(
            check(vec![stmt(StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                body: vec![stmt(StatementKind::Break)]
            })])
            .is_ok()
        );
    }
    /// Verifica o alcance do inicializador e separação entre o corpo e a atualização.
    #[test]
    fn for_scope_keeps_body_shadow_out_of_update() {
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })),
                vec![local("x", expr(ExprKind::String("body"))), print(id("x"))]
            )])
            .is_ok()
        );
        assert!(
            check(vec![
                for_statement(local("x", int()), None, vec![]),
                print(id("x"))
            ])
            .is_err()
        );
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "body",
                    value: int()
                })),
                vec![local("body", int())]
            )])
            .is_err()
        );
        assert!(
            check(vec![
                local("x", int()),
                for_statement(local("x", id("x")), None, vec![])
            ])
            .is_err()
        );
        assert!(
            check(vec![stmt(StatementKind::DoWhile {
                body: vec![local("flag", expr(ExprKind::Bool(true)))],
                condition: id("flag")
            })])
            .is_err()
        );
    }
    /// Verifica tipos e imutabilidade das atribuições na atualização.
    #[test]
    fn for_update_obeys_final_and_type_rules() {
        assert!(
            check(vec![for_statement(
                stmt(StatementKind::Variable {
                    name: "x",
                    annotation: None,
                    is_final: true,
                    initializer: int()
                }),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })),
                vec![]
            )])
            .is_err()
        );
        assert!(
            check(vec![for_statement(
                local("x", int()),
                Some(stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Bool(false))
                })),
                vec![]
            )])
            .is_err()
        );
    }
    /// Impede que ASTs construídas diretamente insiram controle de fluxo nos cabeçalhos.
    #[test]
    fn for_headers_reject_non_expression_statements() {
        for initializer in [
            stmt(StatementKind::Break),
            ret(int()),
            stmt(StatementKind::Block(vec![])),
            print(int()),
        ] {
            assert!(check(vec![for_statement(initializer, None, vec![])]).is_err());
        }
        for update in [
            local("y", int()),
            stmt(StatementKind::Continue),
            stmt(StatementKind::Return(None)),
            print(int()),
        ] {
            assert!(check(vec![for_statement(local("x", int()), Some(update), vec![])]).is_err());
        }
        // O parser representa print nos cabeçalhos como expressão de chamada.
        assert!(
            check(vec![for_statement(
                stmt(StatementKind::Expression(call("print", vec![int()]))),
                Some(stmt(StatementKind::Expression(call("print", vec![int()])))),
                vec![stmt(StatementKind::Break)],
            )])
            .is_ok()
        );
    }
    /// Não infere retorno obrigatório a partir de laços nem de retornos após break.
    #[test]
    fn loop_returns_remain_conservative() {
        for body in [
            vec![stmt(StatementKind::While {
                condition: expr(ExprKind::Bool(true)),
                body: vec![ret(int())],
            })],
            vec![stmt(StatementKind::DoWhile {
                condition: expr(ExprKind::Bool(true)),
                body: vec![stmt(StatementKind::Break), ret(int())],
            })],
        ] {
            assert!(
                validate(&Program {
                    functions: vec![function("f", Type::Int, vec![], body)],
                    statements: vec![]
                })
                .is_err()
            );
        }
        assert!(
            validate(&Program {
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![],
                    vec![
                        stmt(StatementKind::While {
                            condition: expr(ExprKind::Bool(true)),
                            body: vec![stmt(StatementKind::Break)]
                        }),
                        ret(int())
                    ]
                )],
                statements: vec![]
            })
            .is_ok()
        );
    }
    #[test]
    /// Verifica referências antecipadas, recursão e parâmetros mutáveis.
    fn forward_calls_recursion_and_mutable_parameters() {
        let program = Program {
            functions: vec![
                function(
                    "first",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![ret(call("second", vec![id("x")]))],
                ),
                function(
                    "second",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![
                        stmt(StatementKind::Assign {
                            name: "x",
                            value: int(),
                        }),
                        stmt(StatementKind::If {
                            condition: expr(ExprKind::Bool(true)),
                            then_body: vec![ret(id("x"))],
                            else_body: Some(vec![ret(call("first", vec![id("x")]))]),
                        }),
                    ],
                ),
            ],
            statements: vec![print(call("first", vec![int()]))],
        };
        assert!(validate(&program).is_ok());
    }
    #[test]
    /// Verifica nomes de funções, quantidade e tipos dos argumentos.
    fn calls_require_known_function_correct_arity_and_types() {
        for arguments in [vec![], vec![int(), int()], vec![expr(ExprKind::Bool(true))]] {
            assert!(
                validate(&Program {
                    functions: vec![function(
                        "f",
                        Type::Int,
                        vec![("x", Type::Int)],
                        vec![ret(id("x"))]
                    )],
                    statements: vec![print(call("f", arguments))]
                })
                .is_err()
            );
        }
        assert!(check(vec![print(call("missing", vec![]))]).is_err());
        assert!(check(vec![stmt(StatementKind::Expression(call("main", vec![])))]).is_ok());
        assert!(
            check(vec![
                local("main", int()),
                stmt(StatementKind::Expression(call("main", vec![])))
            ])
            .is_err()
        );
    }
    #[test]
    /// Verifica compatibilidade e presença de retorno em todos os caminhos.
    fn returns_are_typed_and_required_on_every_path() {
        for body in [
            vec![],
            vec![stmt(StatementKind::Return(None))],
            vec![ret(expr(ExprKind::Bool(false)))],
            vec![stmt(StatementKind::If {
                condition: expr(ExprKind::Bool(true)),
                then_body: vec![ret(int())],
                else_body: None,
            })],
        ] {
            assert!(
                validate(&Program {
                    functions: vec![function("f", Type::Int, vec![], body)],
                    statements: vec![]
                })
                .is_err()
            );
        }
        assert!(
            validate(&Program {
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![],
                    vec![stmt(StatementKind::Block(vec![ret(int())]))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(check(vec![ret(int())]).is_err());
        assert!(check(vec![stmt(StatementKind::Return(None))]).is_ok());
    }
    #[test]
    /// Verifica que chamadas void não escapam para contextos de valor.
    fn void_calls_are_statements_not_values() {
        assert!(
            check(vec![
                stmt(StatementKind::Expression(call("print", vec![int()]))),
                ret(call("print", vec![int()]))
            ])
            .is_ok()
        );
        for statement in [
            print(call("main", vec![])),
            local("x", call("main", vec![])),
            print(binary(
                BinaryOp::Equal,
                call("main", vec![]),
                call("main", vec![]),
            )),
        ] {
            assert!(check(vec![statement]).is_err());
        }
    }
    #[test]
    /// Rejeita funções, parâmetros duplicados e nomes especiais.
    fn duplicate_functions_parameters_and_reserved_main_are_rejected() {
        for functions in [
            vec![
                function("f", Type::Void, vec![], vec![]),
                function("f", Type::Void, vec![], vec![]),
            ],
            vec![function("main", Type::Void, vec![], vec![])],
            vec![function("print", Type::Void, vec![], vec![])],
            vec![function(
                "f",
                Type::Void,
                vec![("x", Type::Int), ("x", Type::Bool)],
                vec![],
            )],
        ] {
            assert!(
                validate(&Program {
                    functions,
                    statements: vec![]
                })
                .is_err()
            );
        }
    }
    #[test]
    /// Verifica o alcance dos parâmetros sobre nomes de tipos.
    fn parameters_shadow_body_types_but_not_signature_types() {
        assert!(
            validate(&Program {
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("int", Type::Int), ("x", Type::Int)],
                    vec![ret(id("int"))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("x", Type::Int)],
                    vec![local("x", int()), ret(id("x"))]
                )],
                statements: vec![]
            })
            .is_ok()
        );
        assert!(
            validate(&Program {
                functions: vec![function(
                    "f",
                    Type::Int,
                    vec![("int", Type::Int)],
                    vec![
                        stmt(StatementKind::Variable {
                            name: "x",
                            annotation: Some(Type::Int),
                            is_final: false,
                            initializer: int()
                        }),
                        ret(int())
                    ]
                )],
                statements: vec![]
            })
            .is_err()
        );
        assert!(
            check(vec![stmt(StatementKind::If {
                condition: int(),
                then_body: vec![],
                else_body: None
            })])
            .is_err()
        );
        assert!(
            check(vec![
                stmt(StatementKind::If {
                    condition: expr(ExprKind::Bool(true)),
                    then_body: vec![local("x", int())],
                    else_body: None
                }),
                print(id("x"))
            ])
            .is_err()
        );
    }
    #[test]
    /// Verifica ocultação de nomes e atribuições a escopos externos.
    fn nested_scopes_allow_shadowing_and_outer_assignment() {
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Block(vec![
                    stmt(StatementKind::Assign {
                        name: "x",
                        value: int()
                    }),
                    local("y", id("x")),
                    stmt(StatementKind::Block(vec![
                        local("x", expr(ExprKind::String("s"))),
                        print(id("x"))
                    ])),
                ])),
                print(id("x"))
            ])
            .is_ok()
        );
    }
    #[test]
    /// Rejeita uso antecipado de local que oculta um nome externo.
    fn later_local_shadows_outer_even_before_declaration() {
        let error = check(vec![
            local("x", int()),
            stmt(StatementKind::Block(vec![
                print(id("x")),
                local("x", int()),
            ])),
        ])
        .unwrap_err();
        assert!(error.message.contains("before its declaration"));
        assert_eq!(error.span, SPAN);
    }
    #[test]
    /// Rejeita leitura do próprio nome durante sua inicialização.
    fn self_initializer_cannot_read_outer_with_same_name() {
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Block(vec![local("x", id("x"))]))
            ])
            .unwrap_err()
            .message
            .contains("own initializer")
        );
    }
    #[test]
    /// Rejeita nomes duplicados e referências fora do escopo.
    fn duplicate_and_out_of_scope_names_are_rejected() {
        assert!(
            check(vec![local("x", int()), local("x", int())])
                .unwrap_err()
                .message
                .contains("Duplicate")
        );
        assert!(
            check(vec![
                stmt(StatementKind::Block(vec![local("x", int())])),
                print(id("x"))
            ])
            .unwrap_err()
            .message
            .contains("Unknown")
        );
    }
    #[test]
    /// Verifica imutabilidade de final e compatibilidade nas atribuições.
    fn final_and_assignment_types_are_enforced() {
        assert!(
            check(vec![
                stmt(StatementKind::Variable {
                    name: "x",
                    annotation: None,
                    is_final: true,
                    initializer: int()
                }),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: int()
                })
            ])
            .unwrap_err()
            .message
            .contains("final")
        );
        assert!(
            check(vec![
                local("x", int()),
                stmt(StatementKind::Assign {
                    name: "x",
                    value: expr(ExprKind::Bool(true))
                })
            ])
            .unwrap_err()
            .message
            .contains("Type mismatch")
        );
        assert!(
            check(vec![stmt(StatementKind::Variable {
                name: "x",
                annotation: Some(Type::Bool),
                is_final: false,
                initializer: int()
            })])
            .is_err()
        );
    }
    #[test]
    /// Verifica que strings decodificadas e emprestadas compartilham o mesmo tipo.
    fn owned_strings_match_borrowed_strings_in_operators_and_calls() {
        assert!(
            check(vec![
                print(binary(
                    BinaryOp::Add,
                    expr(ExprKind::String("texto")),
                    expr(ExprKind::OwnedString("\n🦀".into()))
                )),
                print(binary(
                    BinaryOp::Equal,
                    expr(ExprKind::OwnedString("a".into())),
                    expr(ExprKind::String("a"))
                )),
                local("s", expr(ExprKind::OwnedString("b".into()))),
                stmt(StatementKind::Assign {
                    name: "s",
                    value: expr(ExprKind::String("c"))
                }),
            ])
            .is_ok()
        );
        assert!(
            validate(&Program {
                functions: vec![
                    function(
                        "identity",
                        Type::String,
                        vec![("s", Type::String)],
                        vec![ret(id("s"))]
                    ),
                    function(
                        "decoded",
                        Type::String,
                        vec![],
                        vec![ret(expr(ExprKind::OwnedString("\n".into())))]
                    ),
                ],
                statements: vec![
                    print(call("identity", vec![expr(ExprKind::String("borrowed"))])),
                    print(call(
                        "identity",
                        vec![expr(ExprKind::OwnedString("owned".into()))]
                    )),
                    print(call("decoded", vec![])),
                ],
            })
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                expr(ExprKind::OwnedString("text".into())),
                int()
            ))])
            .is_err()
        );
        assert!(
            validate(&Program {
                functions: vec![function(
                    "wrong",
                    Type::Int,
                    vec![],
                    vec![ret(expr(ExprKind::OwnedString("text".into())))]
                )],
                statements: vec![]
            })
            .is_err()
        );
    }
    /// Verifica tipos dos operadores e igualdade entre tipos distintos.
    #[test]
    fn typed_operators_and_cross_type_equality() {
        for op in [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::Less,
            BinaryOp::LessEqual,
            BinaryOp::Greater,
            BinaryOp::GreaterEqual,
            BinaryOp::Equal,
            BinaryOp::NotEqual,
        ] {
            assert!(check(vec![print(binary(op, int(), int()))]).is_ok());
        }
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                expr(ExprKind::String("a")),
                expr(ExprKind::String("b"))
            ))])
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Equal,
                int(),
                expr(ExprKind::Bool(false))
            ))])
            .is_ok()
        );
        assert!(
            check(vec![print(binary(
                BinaryOp::Add,
                int(),
                expr(ExprKind::String("b"))
            ))])
            .is_err()
        );
        for op in [BinaryOp::And, BinaryOp::Or] {
            assert!(
                check(vec![print(binary(
                    op,
                    expr(ExprKind::Bool(true)),
                    expr(ExprKind::Bool(false))
                ))])
                .is_ok()
            );
            assert!(check(vec![print(binary(op, int(), int()))]).is_err());
        }
        for (op, valid, invalid) in [
            (UnaryOp::Negate, int(), expr(ExprKind::Bool(false))),
            (UnaryOp::Not, expr(ExprKind::Bool(false)), int()),
        ] {
            assert!(
                check(vec![print(expr(ExprKind::Unary {
                    op,
                    operand: Box::new(valid)
                }))])
                .is_ok()
            );
            assert!(
                check(vec![print(expr(ExprKind::Unary {
                    op,
                    operand: Box::new(invalid)
                }))])
                .is_err()
            );
        }
    }
    #[test]
    /// Verifica anotações quando variáveis ocultam nomes de tipos.
    fn annotations_respect_shadowed_type_names() {
        /// Constrói uma declaração com anotação e inicializador compatíveis.
        fn annotated(ty: Type) -> Statement<'static> {
            let initializer = match ty {
                Type::Void => unreachable!(),
                Type::Int => int(),
                Type::String => expr(ExprKind::String("text")),
                Type::Bool => expr(ExprKind::Bool(true)),
            };
            stmt(StatementKind::Variable {
                name: "value",
                annotation: Some(ty),
                is_final: false,
                initializer,
            })
        }
        for (name, ty) in [
            ("int", Type::Int),
            ("String", Type::String),
            ("bool", Type::Bool),
        ] {
            for statements in [
                vec![local(name, int()), annotated(ty)],
                vec![annotated(ty), local(name, int())],
                vec![
                    local(name, int()),
                    stmt(StatementKind::Block(vec![annotated(ty)])),
                ],
            ] {
                assert!(
                    check(statements)
                        .unwrap_err()
                        .message
                        .contains("shadows the type name")
                );
            }
            // Um local aninhado não oculta o tipo usado na anotação externa.
            assert!(
                check(vec![
                    annotated(ty),
                    stmt(StatementKind::Block(vec![local(name, int())]))
                ])
                .is_ok()
            );
            assert!(check(vec![local(name, int()), print(id(name))]).is_ok());
        }
    }
    #[test]
    /// Impede que print local seja tratado como a função nativa.
    fn shadowed_print_is_never_treated_as_builtin() {
        assert!(
            check(vec![local("print", int()), print(int())])
                .unwrap_err()
                .message
                .contains("shadows")
        );
        assert!(check(vec![print(int()), local("print", int())]).is_err());
        assert!(
            check(vec![
                stmt(StatementKind::Block(vec![local("print", int())])),
                print(int())
            ])
            .is_ok()
        );
    }
}
