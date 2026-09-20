use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Function, Parameter, Program, Statement, StatementKind, Token,
    TokenKind, Type, UnaryOp,
};

/// Limits recursive nesting and AST depth, including long left-associative chains.
const MAX_DEPTH: usize = 64;
const MAX_EXPR_NODES: usize = 128;

pub fn parse<'a>(tokens: &[Token<'a>], source_len: usize) -> Result<Program<'a>, Diagnostic> {
    let mut cursor = Cursor {
        tokens,
        index: 0,
        source_len,
        expr_nodes: 0,
    };
    let mut functions = Vec::new();
    let mut main = None;
    while cursor.peek().is_some() {
        let function = cursor.function()?;
        if function.name == "main" {
            if main.is_some() {
                return Err(Diagnostic::new("duplicate main function", function.span));
            }
            if function.return_type != Type::Void || !function.parameters.is_empty() {
                return Err(Diagnostic::new(
                    "main must have signature void main()",
                    function.span,
                ));
            }
            main = Some(function.body);
        } else {
            functions.push(function);
        }
    }
    let statements = main.ok_or_else(|| cursor.error("expected void main() entrypoint"))?;
    Ok(Program {
        functions,
        statements,
    })
}
struct Cursor<'t, 'a> {
    tokens: &'t [Token<'a>],
    index: usize,
    source_len: usize,
    expr_nodes: usize,
}
impl<'a> Cursor<'_, 'a> {
    fn peek(&self) -> Option<TokenKind<'a>> {
        self.tokens.get(self.index).map(|t| t.kind)
    }
    fn position(&self) -> usize {
        self.tokens
            .get(self.index)
            .map_or(self.source_len, |t| t.span.start)
    }
    fn end(&self) -> usize {
        self.tokens[self.index - 1].span.end
    }
    fn error(&self, message: &str) -> Diagnostic {
        Diagnostic::new(
            message,
            self.tokens.get(self.index).map(|t| t.span).unwrap_or(Span {
                start: self.source_len,
                end: self.source_len,
            }),
        )
    }
    fn expect(&mut self, kind: TokenKind<'_>) -> Result<(), Diagnostic> {
        if self.peek() != Some(kind) {
            return Err(self.error(&format!("expected {kind:?}; supported subset only")));
        }
        self.index += 1;
        Ok(())
    }
    fn take(&mut self, kind: TokenKind<'_>) -> bool {
        if self.peek() == Some(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn name(&mut self) -> Result<&'a str, Diagnostic> {
        match self.peek() {
            Some(TokenKind::Word(name)) if !reserved(name) => {
                self.index += 1;
                Ok(name)
            }
            _ => Err(self.error("expected a non-reserved identifier")),
        }
    }
    fn ty(&mut self, allow_void: bool) -> Result<Type, Diagnostic> {
        let ty = match self.peek() {
            Some(TokenKind::Word("int")) => Type::Int,
            Some(TokenKind::Word("String")) => Type::String,
            Some(TokenKind::Word("bool")) => Type::Bool,
            Some(TokenKind::Word("void")) if allow_void => Type::Void,
            _ => return Err(self.error("expected an explicitly supported type")),
        };
        self.index += 1;
        Ok(ty)
    }
    fn function(&mut self) -> Result<Function<'a>, Diagnostic> {
        let start = self.position();
        let return_type = self.ty(true)?;
        let name = self.name()?;
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        if self.peek() != Some(TokenKind::Symbol(')')) {
            loop {
                let start = self.position();
                let ty = self.ty(false)?;
                let name = self.name()?;
                parameters.push(Parameter {
                    name,
                    ty,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
                if !self.take(TokenKind::Symbol(',')) || self.peek() == Some(TokenKind::Symbol(')'))
                {
                    break;
                }
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        let body = self.block(0)?;
        Ok(Function {
            name,
            return_type,
            parameters,
            body,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    fn arguments(&mut self, depth: usize) -> Result<Vec<Expr<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut arguments = Vec::new();
        if self.peek() != Some(TokenKind::Symbol(')')) {
            loop {
                // Share the expression budget with all arguments; never reset here.
                arguments.push(self.binary(0, depth + 1)?);
                if !self.take(TokenKind::Symbol(',')) || self.peek() == Some(TokenKind::Symbol(')'))
                {
                    break;
                }
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        Ok(arguments)
    }
    fn block(&mut self, depth: usize) -> Result<Vec<Statement<'a>>, Diagnostic> {
        if depth >= MAX_DEPTH {
            return Err(self.error("block nesting limit exceeded"));
        }
        self.expect(TokenKind::Symbol('{'))?;
        let mut statements = Vec::new();
        while self.peek() != Some(TokenKind::Symbol('}')) {
            statements.push(self.statement(depth + 1)?);
        }
        self.expect(TokenKind::Symbol('}'))?;
        Ok(statements)
    }
    fn statement(&mut self, depth: usize) -> Result<Statement<'a>, Diagnostic> {
        let start = self.position();
        if self.peek() == Some(TokenKind::Symbol('{')) {
            let kind = StatementKind::Block(self.block(depth)?);
            return Ok(Statement {
                kind,
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        if self.take(TokenKind::Word("if")) {
            self.expect(TokenKind::Symbol('('))?;
            let condition = self.expression()?;
            self.expect(TokenKind::Symbol(')'))?;
            let then_body = self.block(depth)?;
            let else_body = if self.take(TokenKind::Word("else")) {
                Some(self.block(depth)?)
            } else {
                None
            };
            return Ok(Statement {
                kind: StatementKind::If {
                    condition,
                    then_body,
                    else_body,
                },
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        let kind = if self.take(TokenKind::Word("return")) {
            let value = if self.peek() == Some(TokenKind::Symbol(';')) {
                None
            } else {
                Some(self.expression()?)
            };
            StatementKind::Return(value)
        } else if self.take(TokenKind::Word("print")) {
            self.expect(TokenKind::Symbol('('))?;
            let value = self.expression()?;
            self.expect(TokenKind::Symbol(')'))?;
            StatementKind::Print(value)
        } else if matches!(
            self.peek(),
            Some(TokenKind::Word("var" | "final" | "int" | "String" | "bool"))
        ) {
            let is_final = self.take(TokenKind::Word("final"));
            let inferred = self.take(TokenKind::Word("var"));
            if is_final && inferred {
                return Err(self.error("final var is not supported; use final name = expression"));
            }
            let annotation = if !inferred {
                match self.peek() {
                    Some(TokenKind::Word("int")) => {
                        self.index += 1;
                        Some(Type::Int)
                    }
                    Some(TokenKind::Word("String")) => {
                        self.index += 1;
                        Some(Type::String)
                    }
                    Some(TokenKind::Word("bool")) => {
                        self.index += 1;
                        Some(Type::Bool)
                    }
                    _ => None,
                }
            } else {
                None
            };
            let name = self.name()?;
            self.expect(TokenKind::Operator("="))?;
            let initializer = self.expression()?;
            StatementKind::Variable {
                name,
                annotation,
                is_final,
                initializer,
            }
        } else if matches!(self.peek(), Some(TokenKind::Word(_)))
            && self.tokens.get(self.index + 1).map(|t| t.kind) == Some(TokenKind::Operator("="))
        {
            let name = self.name()?;
            self.expect(TokenKind::Operator("="))?;
            StatementKind::Assign {
                name,
                value: self.expression()?,
            }
        } else {
            let value = self.expression()?;
            if !matches!(value.kind, ExprKind::Call { .. }) {
                return Err(Diagnostic::new(
                    "only function calls are supported as expression statements",
                    value.span,
                ));
            }
            StatementKind::Expression(value)
        };
        self.expect(TokenKind::Symbol(';'))?;
        Ok(Statement {
            kind,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    fn expression(&mut self) -> Result<Expr<'a>, Diagnostic> {
        self.expr_nodes = 0;
        self.binary(0, 0)
    }
    fn charge(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.expr_nodes += 1;
        if depth >= MAX_DEPTH || self.expr_nodes > MAX_EXPR_NODES {
            return Err(self.error("expression complexity limit exceeded"));
        }
        Ok(())
    }
    fn binary(&mut self, min: u8, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let mut left = self.primary(depth)?;
        let mut comparison = None;
        while let Some(TokenKind::Operator(symbol)) = self.peek() {
            let Some((precedence, op)) = binary_op(symbol) else {
                break;
            };
            if precedence < min {
                break;
            }
            if matches!(precedence, 3 | 4) {
                if comparison == Some(precedence) {
                    return Err(
                        self.error("comparison operators cannot be chained without parentheses")
                    );
                }
                comparison = Some(precedence);
            }
            self.charge(depth)?;
            self.index += 1;
            let right = self.binary(precedence + 1, depth + 1)?;
            let span = Span {
                start: left.span.start,
                end: right.span.end,
            };
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }
    fn primary(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        self.charge(depth)?;
        let start = self.position();
        let kind = match self.peek() {
            Some(TokenKind::Number(text)) => {
                let value = text.parse::<i32>().map_err(|_| {
                    self.error("integer literal outside supported signed 32-bit range")
                })?;
                self.index += 1;
                ExprKind::Int(value)
            }
            Some(TokenKind::String(text)) => {
                self.index += 1;
                ExprKind::String(text)
            }
            Some(TokenKind::Word("true")) => {
                self.index += 1;
                ExprKind::Bool(true)
            }
            Some(TokenKind::Word("false")) => {
                self.index += 1;
                ExprKind::Bool(false)
            }
            Some(TokenKind::Word("print")) => {
                self.index += 1;
                ExprKind::Call {
                    name: "print",
                    arguments: self.arguments(depth)?,
                }
            }
            Some(TokenKind::Word(name)) if !reserved(name) => {
                self.index += 1;
                if self.peek() == Some(TokenKind::Symbol('(')) {
                    ExprKind::Call {
                        name,
                        arguments: self.arguments(depth)?,
                    }
                } else {
                    ExprKind::Identifier(name)
                }
            }
            Some(TokenKind::Operator(symbol @ ("-" | "!"))) => {
                self.index += 1;
                if symbol == "-"
                    && matches!(self.peek(), Some(TokenKind::Number(n)) if n.parse::<u64>() == Ok(2147483648))
                {
                    self.index += 1;
                    ExprKind::Int(i32::MIN)
                } else {
                    let operand = self.primary(depth + 1)?;
                    ExprKind::Unary {
                        op: if symbol == "-" {
                            UnaryOp::Negate
                        } else {
                            UnaryOp::Not
                        },
                        operand: Box::new(operand),
                    }
                }
            }
            Some(TokenKind::Symbol('(')) => {
                self.index += 1;
                let mut value = self.binary(0, depth + 1)?;
                self.expect(TokenKind::Symbol(')'))?;
                value.span = Span {
                    start,
                    end: self.end(),
                };
                return Ok(value);
            }
            _ => return Err(self.error("expected a supported expression")),
        };
        Ok(Expr {
            kind,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
}

fn binary_op(symbol: &str) -> Option<(u8, BinaryOp)> {
    Some(match symbol {
        "||" => (1, BinaryOp::Or),
        "&&" => (2, BinaryOp::And),
        "==" => (3, BinaryOp::Equal),
        "!=" => (3, BinaryOp::NotEqual),
        "<" => (4, BinaryOp::Less),
        "<=" => (4, BinaryOp::LessEqual),
        ">" => (4, BinaryOp::Greater),
        ">=" => (4, BinaryOp::GreaterEqual),
        "+" => (5, BinaryOp::Add),
        "-" => (5, BinaryOp::Subtract),
        "*" => (6, BinaryOp::Multiply),
        _ => return None,
    })
}

// Conservative keyword restriction for the bootstrap subset, including builtin types.
fn reserved(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "augment"
            | "base"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "covariant"
            | "default"
            | "deferred"
            | "do"
            | "dynamic"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "extension"
            | "external"
            | "factory"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "Function"
            | "get"
            | "hide"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "interface"
            | "is"
            | "late"
            | "library"
            | "mixin"
            | "native"
            | "new"
            | "null"
            | "of"
            | "on"
            | "operator"
            | "part"
            | "required"
            | "rethrow"
            | "return"
            | "sealed"
            | "set"
            | "show"
            | "static"
            | "super"
            | "switch"
            | "sync"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typedef"
            | "var"
            | "void"
            | "when"
            | "while"
            | "with"
            | "yield"
            | "int"
            | "String"
            | "bool"
            | "print"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parsed(source: &str) -> Program<'_> {
        parse(&dartforge_lexer::lex(source).unwrap(), source.len()).unwrap()
    }
    fn rejected(body: &str) {
        let source = format!("void main() {{ {body} }}");
        assert!(
            parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
            "{body}"
        );
    }
    #[test]
    fn declarations_assignment_and_scopes() {
        let p = parsed(
            "void main() { var x = 1; final String s = 'olá'; bool b = true; x = x + 2; { final y = x; print(y); } }",
        );
        assert_eq!(p.statements.len(), 5);
        assert!(matches!(
            &p.statements[1].kind,
            StatementKind::Variable {
                annotation: Some(Type::String),
                is_final: true,
                ..
            }
        ));
        assert!(matches!(
            &p.statements[3].kind,
            StatementKind::Assign { name: "x", .. }
        ));
        assert!(matches!(&p.statements[4].kind, StatementKind::Block(v) if v.len() == 2));
    }
    #[test]
    fn precedence_and_associativity() {
        let p =
            parsed("void main() { print(1 + 2 * 3 == 7 && !false || false); print(8 - 3 - 1); }");
        let StatementKind::Print(e) = &p.statements[0].kind else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::Or,
            left,
            ..
        } = &e.kind
        else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::And,
            left,
            ..
        } = &left.kind
        else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::Equal,
            left,
            ..
        } = &left.kind
        else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::Add,
            right,
            ..
        } = &left.kind
        else {
            panic!()
        };
        assert!(matches!(
            right.kind,
            ExprKind::Binary {
                op: BinaryOp::Multiply,
                ..
            }
        ));
        let StatementKind::Print(e) = &p.statements[1].kind else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::Subtract,
            left,
            ..
        } = &e.kind
        else {
            panic!()
        };
        assert!(matches!(
            left.kind,
            ExprKind::Binary {
                op: BinaryOp::Subtract,
                ..
            }
        ));
    }
    #[test]
    fn spans_and_integer_bounds() {
        let source = "void main() { print('olá'); print(-2147483648); }";
        let p = parsed(source);
        let StatementKind::Print(e) = &p.statements[0].kind else {
            panic!()
        };
        assert_eq!(&source[e.span.start..e.span.end], "'olá'");
        let StatementKind::Print(e) = &p.statements[1].kind else {
            panic!()
        };
        assert!(matches!(e.kind, ExprKind::Int(i32::MIN)));
        rejected("print(2147483648);");
        rejected("print(-2147483649);");
        rejected("print(99999999999999999999999999999999999);");
    }
    #[test]
    fn strict_subset_and_missing_tokens() {
        for body in [
            "var class = 1;",
            "final var x = 1;",
            "int x;",
            "print();",
            "print(1)",
            "foo(,);",
            "return 1 2;",
            "var x = null;",
            "print(1 2);",
            "print(1 +);",
            "print(1 == 2 == false);",
            "print(1 < 2 < 3);",
            "{",
        ] {
            rejected(body);
        }
        assert!(parse(&[], 0).is_err());
        let s = "void main() {} void main() {}";
        assert!(parse(&dartforge_lexer::lex(s).unwrap(), s.len()).is_err());
    }
    #[test]
    fn functions_calls_returns_and_conditionals() {
        let source = "int sum(int a, int b,) { if (a > 0) { return a + b; } else { return b; } } void main() { print(sum(sum(1, 2), 3,)); greet(); return; } void greet() { print('olá'); }";
        let p = parsed(source);
        assert_eq!(p.functions.len(), 2);
        let f = &p.functions[0];
        assert_eq!(f.name, "sum");
        assert_eq!(f.return_type, Type::Int);
        assert_eq!(f.parameters.len(), 2);
        assert_eq!(
            &source[f.parameters[0].span.start..f.parameters[0].span.end],
            "int a"
        );
        assert!(source[f.span.start..f.span.end].starts_with("int sum("));
        let StatementKind::If {
            condition,
            then_body,
            else_body,
        } = &f.body[0].kind
        else {
            panic!()
        };
        assert!(matches!(
            condition.kind,
            ExprKind::Binary {
                op: BinaryOp::Greater,
                ..
            }
        ));
        assert!(matches!(then_body[0].kind, StatementKind::Return(Some(_))));
        assert!(else_body.is_some());
        let StatementKind::Print(value) = &p.statements[0].kind else {
            panic!()
        };
        let ExprKind::Call { name, arguments } = &value.kind else {
            panic!()
        };
        assert_eq!(*name, "sum");
        assert!(matches!(
            arguments[0].kind,
            ExprKind::Call { name: "sum", .. }
        ));
        assert_eq!(
            &source[value.span.start..value.span.end],
            "sum(sum(1, 2), 3,)"
        );
        assert!(matches!(p.statements[1].kind, StatementKind::Expression(_)));
        assert!(matches!(p.statements[2].kind, StatementKind::Return(None)));
        assert!(matches!(
            parsed("void main() { if (true) {} }").statements[0].kind,
            StatementKind::If {
                else_body: None,
                ..
            }
        ));
    }
    #[test]
    fn function_signatures_and_unsupported_forms() {
        for source in [
            "int main() { return 0; }",
            "void main(int a) {}",
            "void f() {}",
            "void main() {} void main() {}",
            "void f(a) {} void main() {}",
            "void f(void a) {} void main() {}",
            "void f(int a = 1) {} void main() {}",
            "void f(int a,,) {} void main() {}",
            "void main() { int f() {} }",
            "void main() { if(true) print(1); }",
            "void main() { if(true) {} else if(false) {} }",
            "void main() { f(1,,2); }",
            "void main() { 1 + 2; }",
            "void main() { return 1 2; }",
            "void main() { f(1);",
        ] {
            assert!(
                parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
    }
    #[test]
    fn builtin_print_call_expressions() {
        let p = parsed("void f() { return print(1); } void main() { print(print(1)); }");
        let StatementKind::Return(Some(value)) = &p.functions[0].body[0].kind else {
            panic!()
        };
        assert!(matches!(value.kind, ExprKind::Call { name: "print", .. }));
        let StatementKind::Print(value) = &p.statements[0].kind else {
            panic!()
        };
        assert!(matches!(value.kind, ExprKind::Call { name: "print", .. }));
        rejected("var x = print;");
        rejected("var print = 1;");
    }
    #[test]
    fn calls_and_if_share_complexity_guards() {
        rejected(&format!(
            "print({}1{});",
            "f(".repeat(1000),
            ")".repeat(1000)
        ));
        rejected(&format!("f({});", vec!["1"; 1000].join(",")));
        rejected(&format!("{}{}", "if(true){".repeat(1000), "}".repeat(1000)));
        // A large argument tree may not bypass the shared node count.
        rejected(&format!("f({});", vec!["1+2+3"; 50].join(",")));
    }
    #[test]
    fn pathological_depth_is_rejected() {
        rejected(&format!(
            "print({}1{});",
            "(".repeat(1000),
            ")".repeat(1000)
        ));
        rejected(&format!("print({}true);", "!".repeat(1000)));
        rejected(&format!("{}{}", "{".repeat(1000), "}".repeat(1000)));
        rejected(&format!("print(1{});", "+1".repeat(1000)));
    }
}
