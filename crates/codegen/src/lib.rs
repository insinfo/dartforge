//! JavaScript ES module output for the semantically validated Dart subset.
//!
//! Integer literals are i32, but operations currently use JavaScript Number.
//! This is not a claim of complete Dart integer semantics (especially overflow
//! and precision). Expression trees are parenthesized to preserve evaluation.
//! Integer negation and multiplication canonicalize zero to positive zero. In
//! Dart 3.6.2 dart2js, negative-zero printing was observed to change depending on
//! other print argument types in the program; we do not emulate that artifact.
use dartforge_hir::Module;
use dartforge_syntax::{BinaryOp, Expr, ExprKind, Statement, StatementKind, UnaryOp};
use std::fmt::Write;

pub fn emit(module: &Module<'_>) -> String {
    let mut output = String::from("// DartForge subset output\n");
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
        statements(&function.body, 1, &mut output);
        output.push_str("}\n");
    }
    output.push_str("export function main() {\n");
    statements(&module.statements, 1, &mut output);
    output.push_str("}\nmain();\n");
    output
}

fn identifier(name: &str, output: &mut String) {
    // A uniform injective prefix avoids JavaScript keywords and runtime globals.
    // Lexical blocks preserve shadowing; references use the same transformation.
    output.push_str("$df_");
    output.push_str(name);
}

fn indent(depth: usize, output: &mut String) {
    for _ in 0..depth {
        output.push_str("  ");
    }
}

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
            StatementKind::Block(body) => {
                output.push_str("{\n");
                statements(body, depth + 1, output);
                indent(depth, output);
                output.push_str("}\n");
            }
        }
    }
}

fn expression(value: &Expr<'_>, output: &mut String) {
    match &value.kind {
        ExprKind::Int(value) => write!(output, "{value}").expect("writing to String cannot fail"),
        ExprKind::String(value) => output
            .push_str(&serde_json::to_string(value).expect("serializing a string cannot fail")),
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
                // The supported operand is int: normalize JS's negative zero.
                // Adding zero preserves Number precision/overflow behavior.
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
                // A negative int multiplied by zero remains integer zero.
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
    #[ignore = "requires Node.js on PATH"]
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
            .expect("Node.js required");
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        // Keep int zero positive, including when used in further operations.
        // Dart 3.6.2 dart2js prints 0 for int-only programs, but -0.0 for
        // negation in mixed-print programs because of print specialization.
        // We intentionally provide stable integer-zero semantics instead of
        // reproducing that whole-program optimization artifact. Large-number
        // baselines below match dart2js, not the Dart VM's 64-bit arithmetic.
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
        assert!(output.contains("if (($df_n <= 1)) {\n    return 1;\n  } else {"));
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
    #[ignore = "requires Node.js on PATH"]
    fn functions_execute_recursion_ordered_arguments_and_early_return() {
        let output = functions_fixture();
        let run = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &output])
            .output()
            .expect("Node.js required");
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
}
