//! Gera módulos JavaScript para o subconjunto Dart validado semanticamente.
//!
//! Literais inteiros são i32, mas operações usam Number do JavaScript; precisão
//! e transbordamento ainda não equivalem à semântica completa de inteiros Dart.
//! Parênteses preservam a árvore e a ordem de avaliação. Negação e multiplicação
//! normalizam zero inteiro para zero positivo. Não reproduzimos a variação de
//! impressão de zero negativo observada no dart2js 3.6.2 conforme outros tipos
//! impressos no mesmo programa. Laços nativos preservam break e continue.
use dartforge_hir::Module;
use dartforge_syntax::{BinaryOp, Expr, ExprKind, Statement, StatementKind, UnaryOp};
use std::fmt::Write;

/// Gera um módulo ES com funções, exportação de main e chamada inicial de main.
///
/// O módulo deve ter passado pela análise semântica: nomes válidos, tipos
/// compatíveis, break/continue dentro de laços e cabeçalhos de for válidos.
/// Não inclui runtime Dart completo, mapas de origem ou otimizações globais.
///
/// # Pânicos
///
/// Falha se um cabeçalho de for contém uma instrução incompatível. A inicialização
/// aceita variável, atribuição ou expressão; a atualização aceita somente as
/// duas últimas. Esta falha indica uma violação do contrato interno da AST.
///
/// ```
/// use dartforge_hir::lower;
/// use dartforge_syntax::Program;
/// let module = lower(Program { functions: vec![], statements: vec![] });
/// let javascript = dartforge_codegen::emit(&module);
/// assert!(javascript.contains("export function main()"));
/// assert!(javascript.ends_with("main();\n"));
/// ```
pub fn emit(module: &Module<'_>) -> String {
    let mut output = String::from("// Saída do subconjunto DartForge\n");
    for function in &module.functions {
        output.push_str("function ");
        identifier(function.name, &mut output);
        output.push('(');
        for (index, parameter) in function.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            identifier(parameter.name, &mut output);
        }
        output.push_str(") {\n");
        // Dart permite que um local do corpo sombreie um parâmetro. Em JS,
        // parâmetros e let no mesmo bloco conflitam; o bloco interno mantém
        // os dois escopos distintos sem mudar retornos ou referências externas.
        indent(1, &mut output);
        block(&function.body, 1, &mut output);
        output.push_str("\n}\n");
    }
    output.push_str("export function main() {\n");
    statements(&module.statements, 1, &mut output);
    output.push_str("}\nmain();\n");
    output
}

/// Aplica o prefixo estável usado em declarações e referências.
fn identifier(name: &str, output: &mut String) {
    // O prefixo injetivo evita palavras reservadas e globais do JavaScript.
    // Blocos léxicos preservam sombreamento; referências recebem o mesmo prefixo.
    output.push_str("$df_");
    output.push_str(name);
}

/// Acrescenta dois espaços por nível léxico.
fn indent(depth: usize, output: &mut String) {
    for _ in 0..depth {
        output.push_str("  ");
    }
}

/// Emite instruções na ordem original e preserva os respectivos escopos.
fn statements(body: &[Statement<'_>], depth: usize, output: &mut String) {
    for statement in body {
        indent(depth, output);
        match &statement.kind {
            StatementKind::Variable {
                name,
                is_final,
                initializer,
                ..
            } => {
                output.push_str(if *is_final { "const " } else { "let " });
                identifier(name, output);
                output.push_str(" = ");
                expression(initializer, output);
                output.push_str(";\n");
            }
            StatementKind::Assign { name, value } => {
                identifier(name, output);
                output.push_str(" = ");
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::Print(value) => {
                output.push_str("console.log(");
                expression(value, output);
                output.push_str(");\n");
            }
            StatementKind::Return(value) => {
                output.push_str("return");
                if let Some(value) = value {
                    output.push(' ');
                    expression(value, output);
                }
                output.push_str(";\n");
            }
            StatementKind::Expression(value) => {
                expression(value, output);
                output.push_str(";\n");
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                output.push_str("if (");
                expression(condition, output);
                output.push_str(") {\n");
                statements(then_body, depth + 1, output);
                indent(depth, output);
                output.push('}');
                if let Some(else_body) = else_body {
                    output.push_str(" else {\n");
                    statements(else_body, depth + 1, output);
                    indent(depth, output);
                    output.push('}');
                }
                output.push('\n');
            }
            StatementKind::While { condition, body } => {
                output.push_str("while (");
                expression(condition, output);
                output.push_str(") ");
                block(body, depth, output);
                output.push('\n');
            }
            StatementKind::DoWhile { body, condition } => {
                output.push_str("do ");
                block(body, depth, output);
                output.push_str(" while (");
                expression(condition, output);
                output.push_str(");\n");
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                output.push_str("for (");
                if let Some(initializer) = initializer {
                    for_clause(initializer, true, output);
                }
                output.push_str("; ");
                if let Some(condition) = condition {
                    expression(condition, output);
                }
                output.push_str("; ");
                if let Some(update) = update {
                    for_clause(update, false, output);
                }
                output.push_str(") ");
                block(body, depth, output);
                output.push('\n');
            }
            StatementKind::Break => output.push_str("break;\n"),
            StatementKind::Continue => output.push_str("continue;\n"),
            StatementKind::Block(body) => {
                output.push_str("{\n");
                statements(body, depth + 1, output);
                indent(depth, output);
                output.push_str("}\n");
            }
        }
    }
}

/// Emite um bloco sem recuo inicial ou quebra de linha final.
fn block(body: &[Statement<'_>], depth: usize, output: &mut String) {
    output.push_str("{\n");
    statements(body, depth + 1, output);
    indent(depth, output);
    output.push('}');
}

/// Emite uma cláusula de for validada, sem separadores ou quebras de linha.
fn for_clause(statement: &Statement<'_>, allow_variable: bool, output: &mut String) {
    match &statement.kind {
        StatementKind::Variable {
            name,
            is_final,
            initializer,
            ..
        } if allow_variable => {
            output.push_str(if *is_final { "const " } else { "let " });
            identifier(name, output);
            output.push_str(" = ");
            expression(initializer, output);
        }
        StatementKind::Assign { name, value } => {
            identifier(name, output);
            output.push_str(" = ");
            expression(value, output);
        }
        StatementKind::Expression(value) => expression(value, output),
        _ => panic!("AST inválida: instrução incompatível com cabeçalho de for"),
    }
}

/// Serializa texto já decodificado, preservando controles, Unicode e aspas.
///
/// O escape JSON impede que conteúdo da string seja interpretado como código
/// JavaScript. Strings emprestadas e alocadas seguem exatamente o mesmo caminho.
fn string_literal(value: &str, output: &mut String) {
    output.push_str(&serde_json::to_string(value).expect("serializar uma string não falha"));
}

/// Emite uma expressão sem duplicar a avaliação de operandos.
fn expression(value: &Expr<'_>, output: &mut String) {
    match &value.kind {
        ExprKind::Int(value) => write!(output, "{value}").expect("escrever em String não falha"),
        ExprKind::String(value) => string_literal(value, output),
        ExprKind::OwnedString(value) => string_literal(value, output),
        ExprKind::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        ExprKind::Identifier(name) => identifier(name, output),
        ExprKind::Call { name, arguments } => {
            match *name {
                "main" => output.push_str("main"),
                "print" => output.push_str("console.log"),
                _ => identifier(name, output),
            }
            output.push('(');
            for (index, argument) in arguments.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                expression(argument, output);
            }
            output.push(')');
        }
        ExprKind::Unary { op, operand } => {
            output.push('(');
            output.push_str(match op {
                UnaryOp::Negate => "-",
                UnaryOp::Not => "!",
            });
            output.push(' ');
            expression(operand, output);
            if *op == UnaryOp::Negate {
                // O operando é int: normaliza o zero negativo do JavaScript.
                // Somar zero preserva precisão e transbordamento de Number.
                output.push_str(" + 0");
            }
            output.push(')');
        }
        ExprKind::Binary { op, left, right } => {
            output.push('(');
            expression(left, output);
            output.push_str(match op {
                BinaryOp::Add => " + ",
                BinaryOp::Subtract => " - ",
                BinaryOp::Multiply => " * ",
                BinaryOp::Equal => " === ",
                BinaryOp::NotEqual => " !== ",
                BinaryOp::Less => " < ",
                BinaryOp::LessEqual => " <= ",
                BinaryOp::Greater => " > ",
                BinaryOp::GreaterEqual => " >= ",
                BinaryOp::And => " && ",
                BinaryOp::Or => " || ",
            });
            expression(right, output);
            if *op == BinaryOp::Multiply {
                // Um inteiro negativo multiplicado por zero continua sendo zero inteiro.
                output.push_str(" + 0");
            }
            output.push(')');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dartforge_diagnostics::Span;
    use dartforge_syntax::{Function, Parameter, Program, Type};

    fn expr(kind: ExprKind<'_>) -> Expr<'_> {
        Expr {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn statement(kind: StatementKind<'_>) -> Statement<'_> {
        Statement {
            kind,
            span: Span { start: 0, end: 0 },
        }
    }
    fn print(value: Expr<'_>) -> Statement<'_> {
        statement(StatementKind::Print(value))
    }
    fn binary<'a>(op: BinaryOp, left: Expr<'a>, right: Expr<'a>) -> Expr<'a> {
        expr(ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    fn variable<'a>(name: &'a str, is_final: bool, initializer: Expr<'a>) -> Statement<'a> {
        statement(StatementKind::Variable {
            name,
            annotation: None,
            is_final,
            initializer,
        })
    }
    fn compile(statements: Vec<Statement<'_>>) -> String {
        emit(&dartforge_hir::lower(Program {
            functions: vec![],
            statements,
        }))
    }

    #[test]
    fn preserves_nested_expression_tree_and_unary_tokens() {
        let value = binary(
            BinaryOp::Multiply,
            binary(
                BinaryOp::Add,
                expr(ExprKind::Int(2)),
                expr(ExprKind::Int(3)),
            ),
            expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(-4))),
            }),
        );
        assert!(compile(vec![print(value)]).contains("console.log(((2 + 3) * (- -4 + 0) + 0));"));
    }

    #[test]
    fn protects_globals_keywords_and_nested_shadowing() {
        let output = compile(vec![
            variable("console", false, expr(ExprKind::Int(1))),
            variable("main", true, expr(ExprKind::Int(2))),
            variable("function", false, expr(ExprKind::Bool(true))),
            variable("$df_console", true, expr(ExprKind::Int(3))),
            statement(StatementKind::Block(vec![
                variable("console", false, expr(ExprKind::Int(4))),
                statement(StatementKind::Assign {
                    name: "console",
                    value: expr(ExprKind::Int(5)),
                }),
                print(expr(ExprKind::Identifier("console"))),
            ])),
            print(expr(ExprKind::Identifier("console"))),
        ]);
        assert!(output.contains("let $df_console = 1;"));
        assert!(output.contains("const $df_main = 2;"));
        assert!(output.contains("let $df_function = true;"));
        assert!(output.contains("const $df_$df_console = 3;"));
        assert!(output.contains("  {\n    let $df_console = 4;\n    $df_console = 5;\n    console.log($df_console);\n  }\n  console.log($df_console);"));
        assert!(output.ends_with("}\nmain();\n"));
    }

    #[test]
    fn strings_cannot_break_out_of_javascript_literal() {
        let value = "\"\\\n\r\t'); throw Error('bad'); // 🦀";
        let output = compile(vec![print(expr(ExprKind::String(value)))]);
        let serialized = output
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(serialized).unwrap(), value);
    }

    #[test]
    fn maps_equality_and_short_circuit_operators() {
        for (op, text) in [
            (BinaryOp::Equal, " === "),
            (BinaryOp::NotEqual, " !== "),
            (BinaryOp::And, " && "),
            (BinaryOp::Or, " || "),
            (BinaryOp::LessEqual, " <= "),
            (BinaryOp::GreaterEqual, " >= "),
        ] {
            let output = compile(vec![print(binary(
                op,
                expr(ExprKind::Bool(true)),
                expr(ExprKind::Bool(false)),
            ))]);
            assert!(output.contains(&format!("(true{text}false)")));
        }
    }
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn numeric_int_zero_is_canonical_and_large_values_match_js() {
        let output = compile(vec![
            variable("zero", false, expr(ExprKind::Int(0))),
            variable("large", false, expr(ExprKind::Int(i32::MAX))),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Identifier("zero"))),
            })),
            print(expr(ExprKind::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(expr(ExprKind::Int(0))),
            })),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Identifier("large")),
                expr(ExprKind::Identifier("large")),
            )),
            print(binary(
                BinaryOp::Multiply,
                binary(
                    BinaryOp::Multiply,
                    expr(ExprKind::Identifier("large")),
                    expr(ExprKind::Identifier("large")),
                ),
                expr(ExprKind::Identifier("large")),
            )),
            variable(
                "negativeZero",
                false,
                expr(ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand: Box::new(expr(ExprKind::Int(0))),
                }),
            ),
            print(binary(
                BinaryOp::Multiply,
                expr(ExprKind::Int(-1)),
                expr(ExprKind::Identifier("negativeZero")),
            )),
            print(expr(ExprKind::Bool(true))),
            print(expr(ExprKind::String("plain"))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Mantém zero inteiro positivo também nas operações seguintes.
        // dart2js 3.6.2 imprime 0 em programas que imprimem apenas int, mas pode
        // imprimir -0.0 se outros tipos também forem impressos. Preservamos zero
        // inteiro estável sem reproduzir esse efeito da otimização global.
        // Os valores grandes seguem dart2js, não a aritmética de 64 bits da VM.
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "0\n0\n4611686014132420600\n9.903520300447984e+27\n0\ntrue\nplain\n"
        );
    }
    fn call(name: &'static str, arguments: Vec<Expr<'static>>) -> Expr<'static> {
        expr(ExprKind::Call { name, arguments })
    }

    fn function(
        name: &'static str,
        return_type: Type,
        parameters: &[(&'static str, Type)],
        body: Vec<Statement<'static>>,
    ) -> Function<'static> {
        Function {
            name,
            return_type,
            parameters: parameters
                .iter()
                .map(|&(name, ty)| Parameter {
                    name,
                    ty,
                    span: Span { start: 0, end: 0 },
                })
                .collect(),
            body,
            span: Span { start: 0, end: 0 },
        }
    }

    fn functions_fixture() -> String {
        let functions = vec![
            function(
                "returnedPrint",
                Type::Void,
                &[],
                vec![statement(StatementKind::Return(Some(call(
                    "print",
                    vec![expr(ExprKind::String("returned print"))],
                ))))],
            ),
            function(
                "factorial",
                Type::Int,
                &[("n", Type::Int)],
                vec![statement(StatementKind::If {
                    condition: binary(
                        BinaryOp::LessEqual,
                        expr(ExprKind::Identifier("n")),
                        expr(ExprKind::Int(1)),
                    ),
                    then_body: vec![statement(StatementKind::Return(Some(expr(ExprKind::Int(
                        1,
                    )))))],
                    else_body: Some(vec![statement(StatementKind::Return(Some(binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("n")),
                        call(
                            "factorial",
                            vec![binary(
                                BinaryOp::Subtract,
                                expr(ExprKind::Identifier("n")),
                                expr(ExprKind::Int(1)),
                            )],
                        ),
                    ))))]),
                })],
            ),
            function(
                "mark",
                Type::Int,
                &[("console", Type::Int)],
                vec![
                    print(expr(ExprKind::Identifier("console"))),
                    statement(StatementKind::Return(Some(expr(ExprKind::Identifier(
                        "console",
                    ))))),
                ],
            ),
            function(
                "function",
                Type::Int,
                &[("main", Type::Int), ("second", Type::Int)],
                vec![statement(StatementKind::Return(Some(binary(
                    BinaryOp::Add,
                    binary(
                        BinaryOp::Multiply,
                        expr(ExprKind::Identifier("main")),
                        expr(ExprKind::Int(10)),
                    ),
                    expr(ExprKind::Identifier("second")),
                ))))],
            ),
            function(
                "stop",
                Type::Void,
                &[("flag", Type::Bool)],
                vec![
                    statement(StatementKind::If {
                        condition: expr(ExprKind::Identifier("flag")),
                        then_body: vec![
                            print(expr(ExprKind::String("stop"))),
                            statement(StatementKind::Return(None)),
                        ],
                        else_body: None,
                    }),
                    print(expr(ExprKind::String("go"))),
                ],
            ),
        ];
        let statements = vec![
            statement(StatementKind::Expression(call("returnedPrint", vec![]))),
            print(call("factorial", vec![expr(ExprKind::Int(5))])),
            print(call(
                "function",
                vec![
                    call("mark", vec![expr(ExprKind::Int(1))]),
                    call("mark", vec![expr(ExprKind::Int(2))]),
                ],
            )),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(true))],
            ))),
            statement(StatementKind::Expression(call(
                "stop",
                vec![expr(ExprKind::Bool(false))],
            ))),
        ];
        emit(&dartforge_hir::lower(Program {
            functions,
            statements,
        }))
    }

    #[test]
    fn emits_named_functions_parameters_returns_and_branches() {
        let output = functions_fixture();
        assert!(output.contains("function $df_function($df_main, $df_second) {"));
        assert!(output.contains("function $df_mark($df_console) {"));
        assert!(output.contains("if (($df_n <= 1)) {\n      return 1;\n    } else {"));
        assert!(output.contains("    return;\n"));
        assert!(output.contains("$df_stop(true);"));
        assert!(output.contains("return console.log(\"returned print\");"));
        assert!(!output.contains("$df_print("));
        let main_call = compile(vec![statement(StatementKind::Expression(call(
            "main",
            vec![],
        )))]);
        assert!(main_call.contains("  main();\n"));
        assert!(!main_call.contains("$df_main("));
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn functions_execute_recursion_ordered_arguments_and_early_return() {
        let output = functions_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "returned print\n120\n1\n2\n12\nstop\ngo\n"
        );
    }
    fn assign(name: &'static str, value: Expr<'static>) -> Statement<'static> {
        statement(StatementKind::Assign { name, value })
    }
    fn name(value: &'static str) -> Expr<'static> {
        expr(ExprKind::Identifier(value))
    }
    fn int(value: i32) -> Expr<'static> {
        expr(ExprKind::Int(value))
    }
    fn when(condition: Expr<'static>, body: Vec<Statement<'static>>) -> Statement<'static> {
        statement(StatementKind::If {
            condition,
            then_body: body,
            else_body: None,
        })
    }
    fn loops_fixture() -> String {
        let functions = vec![
            function(
                "tick",
                Type::Int,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("update"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Add,
                        name("n"),
                        int(1),
                    )))),
                ],
            ),
            function(
                "check",
                Type::Bool,
                &[("n", Type::Int)],
                vec![
                    print(expr(ExprKind::String("condition"))),
                    print(name("n")),
                    statement(StatementKind::Return(Some(binary(
                        BinaryOp::Less,
                        name("n"),
                        int(2),
                    )))),
                ],
            ),
        ];
        let statements = vec![
            variable("i", false, int(77)),
            statement(StatementKind::For {
                initializer: Some(Box::new(variable("i", false, int(0)))),
                condition: Some(binary(BinaryOp::Less, name("i"), int(3))),
                update: Some(Box::new(assign("i", call("tick", vec![name("i")])))),
                body: vec![
                    variable("j", false, int(0)),
                    statement(StatementKind::While {
                        condition: binary(BinaryOp::Less, name("j"), int(3)),
                        body: vec![
                            assign("j", binary(BinaryOp::Add, name("j"), int(1))),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(1)),
                                vec![statement(StatementKind::Continue)],
                            ),
                            when(
                                binary(BinaryOp::Equal, name("j"), int(3)),
                                vec![statement(StatementKind::Break)],
                            ),
                            print(binary(
                                BinaryOp::Add,
                                binary(BinaryOp::Multiply, name("i"), int(10)),
                                name("j"),
                            )),
                        ],
                    }),
                    when(
                        binary(BinaryOp::Equal, name("i"), int(1)),
                        vec![statement(StatementKind::Continue)],
                    ),
                    print(name("i")),
                ],
            }),
            print(name("i")),
            variable("d", false, int(0)),
            statement(StatementKind::DoWhile {
                body: vec![
                    assign("d", binary(BinaryOp::Add, name("d"), int(1))),
                    statement(StatementKind::Continue),
                ],
                condition: call("check", vec![name("d")]),
            }),
            print(name("d")),
            statement(StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                body: vec![statement(StatementKind::Break)],
            }),
        ];
        emit(&dartforge_hir::lower(Program {
            functions,
            statements,
        }))
    }

    #[test]
    fn loops_keep_native_control_flow_and_header_scope() {
        let output = loops_fixture();
        assert!(output.contains("for (let $df_i = 0; ($df_i < 3); $df_i = $df_tick($df_i)) {"));
        assert!(output.contains("while (($df_j < 3)) {"));
        assert!(output.contains("} while ($df_check($df_d));"));
        assert!(output.contains("for (; ; ) {"));
    }

    #[test]
    #[should_panic(expected = "AST inválida")]
    fn rejects_invalid_for_header_instead_of_emitting_broken_javascript() {
        compile(vec![statement(StatementKind::For {
            initializer: None,
            condition: None,
            update: Some(Box::new(variable("invalid", false, int(0)))),
            body: vec![],
        })]);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn loops_execute_nested_break_continue_updates_and_conditions() {
        let output = loops_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            String::from_utf8(run.stdout).unwrap().replace("\r\n", "\n"),
            "2\n0\nupdate\n0\n12\nupdate\n1\n22\n2\nupdate\n2\n77\ncondition\n1\ncondition\n2\n2\n"
        );
    }
    #[test]
    fn borrowed_and_decoded_strings_emit_identical_literals() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let borrowed = compile(vec![print(expr(ExprKind::String(value)))]);
        let owned = compile(vec![print(expr(ExprKind::OwnedString(value.to_owned())))]);
        assert_eq!(borrowed, owned);
        let literal = owned
            .split("console.log(")
            .nth(1)
            .unwrap()
            .split(");\n")
            .next()
            .unwrap();
        assert_eq!(serde_json::from_str::<String>(literal).unwrap(), value);
    }

    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn decoded_strings_execute_without_injection_or_unicode_loss() {
        let value = "aspas: \"' barra: \\ controles: \n\r\t\0 Unicode: 😀\u{2028}\u{2029}\"); process.exit(17); //";
        let output = compile(vec![
            print(expr(ExprKind::OwnedString(value.to_owned()))),
            print(expr(ExprKind::String(value))),
        ]);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Compara bytes: normalizar novas linhas esconderia corrupção de \r.
        assert_eq!(run.stdout, format!("{value}\n{value}\n").as_bytes());
    }
    /// Confere sombreamento de parâmetro sem declaração JavaScript duplicada.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn function_body_locals_can_shadow_parameters() {
        let module = dartforge_hir::lower(Program {
            functions: vec![function(
                "f",
                Type::Int,
                &[("x", Type::Int)],
                vec![
                    variable("x", false, int(1)),
                    statement(StatementKind::Return(Some(name("x")))),
                ],
            )],
            statements: vec![print(call("f", vec![int(99)]))],
        });
        let output = emit(&module);
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js necessário");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(run.stdout, b"1\n");
    }
}
