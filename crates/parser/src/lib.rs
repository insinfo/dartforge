//! Análise sintática do subconjunto Dart 3.6.2, com limites explícitos de complexidade.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Expr, ExprKind, Function, Parameter, Program, Statement, StatementKind, Token,
    TokenKind, Type, UnaryOp,
};

/// Limita o aninhamento recursivo, inclusive cadeias associativas à esquerda.
const MAX_DEPTH: usize = 64;
const MAX_EXPR_NODES: usize = 128;

/// Analisa tokens do subconjunto Dart e exige uma única entrada void main().
///
/// # Exemplos
///
///     let fonte = "void main() {}";
///     let tokens = dartforge_lexer::lex(fonte).unwrap();
///     let programa = dartforge_parser::parse(&tokens, fonte.len()).unwrap();
///     assert!(programa.statements.is_empty());
///
/// # Erros
///
/// Retorna diagnóstico para sintaxe não suportada, entrada inválida ou limites
/// de aninhamento e complexidade excedidos.
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
    /// Consulta o próximo token sem avançar o cursor.
    fn peek(&self) -> Option<TokenKind<'a>> {
        self.tokens.get(self.index).map(|t| t.kind)
    }
    /// Obtém o deslocamento inicial do token atual em bytes.
    fn position(&self) -> usize {
        self.tokens
            .get(self.index)
            .map_or(self.source_len, |t| t.span.start)
    }
    /// Obtém o deslocamento final do último token consumido.
    fn end(&self) -> usize {
        self.tokens[self.index - 1].span.end
    }
    /// Cria um diagnóstico localizado no token atual ou no fim do arquivo.
    fn error(&self, message: &str) -> Diagnostic {
        Diagnostic::new(
            message,
            self.tokens.get(self.index).map(|t| t.span).unwrap_or(Span {
                start: self.source_len,
                end: self.source_len,
            }),
        )
    }
    /// Consome o token esperado ou informa o desvio sintático.
    fn expect(&mut self, kind: TokenKind<'_>) -> Result<(), Diagnostic> {
        if self.peek() != Some(kind) {
            return Err(self.error(&format!("expected {kind:?}; supported subset only")));
        }
        self.index += 1;
        Ok(())
    }
    /// Consome um token opcional e informa se ele estava presente.
    fn take(&mut self, kind: TokenKind<'_>) -> bool {
        if self.peek() == Some(kind) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    /// Lê um identificador permitido pelo subconjunto.
    fn name(&mut self) -> Result<&'a str, Diagnostic> {
        match self.peek() {
            Some(TokenKind::Word(name)) if !reserved(name) => {
                self.index += 1;
                Ok(name)
            }
            _ => Err(self.error("expected a non-reserved identifier")),
        }
    }
    /// Lê um tipo explícito, aceitando void somente quando autorizado.
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
    /// Lê a assinatura tipada e o corpo de uma função de topo.
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
    /// Lê argumentos posicionais mantendo o limite compartilhado da expressão.
    fn arguments(&mut self, depth: usize) -> Result<Vec<Expr<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut arguments = Vec::new();
        if self.peek() != Some(TokenKind::Symbol(')')) {
            loop {
                // Compartilha o limite de nós entre todos os argumentos, sem reiniciá-lo.
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
    /// Lê um bloco entre chaves e verifica o limite de aninhamento.
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
    /// Lê uma instrução e preserva seu intervalo completo no código-fonte.
    fn statement(&mut self, depth: usize) -> Result<Statement<'a>, Diagnostic> {
        let start = self.position();
        let kind = if self.peek() == Some(TokenKind::Symbol('{')) {
            StatementKind::Block(self.block(depth)?)
        } else if self.take(TokenKind::Word("if")) {
            let condition = self.condition()?;
            let then_body = self.block(depth)?;
            let else_body = if self.take(TokenKind::Word("else")) {
                Some(self.block(depth)?)
            } else {
                None
            };
            StatementKind::If {
                condition,
                then_body,
                else_body,
            }
        } else if self.take(TokenKind::Word("while")) {
            let condition = self.condition()?;
            let body = self.block(depth)?;
            StatementKind::While { condition, body }
        } else if self.take(TokenKind::Word("do")) {
            let body = self.block(depth)?;
            self.expect(TokenKind::Word("while"))?;
            let condition = self.condition()?;
            self.expect(TokenKind::Symbol(';'))?;
            StatementKind::DoWhile { body, condition }
        } else if self.take(TokenKind::Word("for")) {
            self.expect(TokenKind::Symbol('('))?;
            let initializer = if self.peek() == Some(TokenKind::Symbol(';')) {
                None
            } else {
                Some(Box::new(self.simple(true)?))
            };
            self.expect(TokenKind::Symbol(';'))?;
            let condition = if self.peek() == Some(TokenKind::Symbol(';')) {
                None
            } else {
                Some(self.expression()?)
            };
            self.expect(TokenKind::Symbol(';'))?;
            let update = if self.peek() == Some(TokenKind::Symbol(')')) {
                None
            } else {
                Some(Box::new(self.simple(false)?))
            };
            self.expect(TokenKind::Symbol(')'))?;
            let body = self.block(depth)?;
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            }
        } else {
            let kind = if self.take(TokenKind::Word("break")) {
                StatementKind::Break
            } else if self.take(TokenKind::Word("continue")) {
                StatementKind::Continue
            } else if self.take(TokenKind::Word("return")) {
                let value = if self.peek() == Some(TokenKind::Symbol(';')) {
                    None
                } else {
                    Some(self.expression()?)
                };
                StatementKind::Return(value)
            } else if self.take(TokenKind::Word("print")) {
                let value = self.condition()?;
                StatementKind::Print(value)
            } else {
                self.simple(true)?.kind
            };
            self.expect(TokenKind::Symbol(';'))?;
            kind
        };
        Ok(Statement {
            kind,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê uma expressão delimitada por parênteses.
    fn condition(&mut self) -> Result<Expr<'a>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let condition = self.expression()?;
        self.expect(TokenKind::Symbol(')'))?;
        Ok(condition)
    }
    /// Lê declaração, atribuição ou chamada sem consumir o ponto e vírgula.
    fn simple(&mut self, allow_declaration: bool) -> Result<Statement<'a>, Diagnostic> {
        let start = self.position();
        let kind = if matches!(
            self.peek(),
            Some(TokenKind::Word("var" | "final" | "int" | "String" | "bool"))
        ) {
            if !allow_declaration {
                return Err(self.error("declarations are not supported in for updates"));
            }
            let is_final = self.take(TokenKind::Word("final"));
            let inferred = self.take(TokenKind::Word("var"));
            if is_final && inferred {
                return Err(self.error("final var is not supported; use final name = expression"));
            }
            let annotation = if !inferred
                && matches!(
                    self.peek(),
                    Some(TokenKind::Word("int" | "String" | "bool"))
                ) {
                Some(self.ty(false)?)
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
        } else if let Some(TokenKind::Operator(operator @ ("++" | "--"))) = self.peek() {
            self.index += 1;
            let name_start = self.position();
            let name = self.name()?;
            let name_span = Span {
                start: name_start,
                end: self.end(),
            };
            self.updated(name, name_span, operator, start)?
        } else if matches!(self.peek(), Some(TokenKind::Word(_)))
            && matches!(
                self.tokens.get(self.index + 1).map(|t| t.kind),
                Some(TokenKind::Operator("=" | "+=" | "-=" | "*=" | "++" | "--"))
            )
        {
            let name = self.name()?;
            let name_span = Span {
                start,
                end: self.end(),
            };
            let Some(TokenKind::Operator(operator)) = self.peek() else {
                unreachable!()
            };
            self.index += 1;
            if operator == "=" {
                StatementKind::Assign {
                    name,
                    value: self.expression()?,
                }
            } else {
                self.updated(name, name_span, operator, start)?
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
        Ok(Statement {
            kind,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Converte atualizações de uma variável simples em atribuições binárias.
    fn updated(
        &mut self,
        name: &'a str,
        name_span: Span,
        operator: &str,
        start: usize,
    ) -> Result<StatementKind<'a>, Diagnostic> {
        self.expr_nodes = 0;
        let right = if matches!(operator, "++" | "--") {
            Expr {
                kind: ExprKind::Int(1),
                span: name_span,
            }
        } else {
            self.expression()?
        };
        self.charge(0)?;
        self.charge(0)?;
        let op = match operator {
            "++" | "+=" => BinaryOp::Add,
            "--" | "-=" => BinaryOp::Subtract,
            "*=" => BinaryOp::Multiply,
            _ => unreachable!(),
        };
        let value = Expr {
            kind: ExprKind::Binary {
                op,
                left: Box::new(Expr {
                    kind: ExprKind::Identifier(name),
                    span: name_span,
                }),
                right: Box::new(right),
            },
            span: Span {
                start,
                end: self.end(),
            },
        };
        Ok(StatementKind::Assign { name, value })
    }
    /// Inicia uma expressão com um novo limite de complexidade.
    fn expression(&mut self) -> Result<Expr<'a>, Diagnostic> {
        self.expr_nodes = 0;
        self.binary(0, 0)
    }
    /// Contabiliza um nó e rejeita expressões que excedam os limites.
    fn charge(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.expr_nodes += 1;
        if depth >= MAX_DEPTH || self.expr_nodes > MAX_EXPR_NODES {
            return Err(self.error("expression complexity limit exceeded"));
        }
        Ok(())
    }
    /// Lê operadores binários respeitando precedência e associatividade.
    fn binary(&mut self, min: u8, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let mut left = self.primary(depth)?;
        if matches!(self.peek(), Some(TokenKind::Operator("++" | "--"))) {
            return Err(
                self.error("increment and decrement are supported only as standalone statements")
            );
        }
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
    /// Lê literais, referências, chamadas e operadores unários.
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
                let span = self.tokens[self.index].span;
                self.index += 1;
                if text.contains('\\') {
                    ExprKind::OwnedString(decode_string(text, span)?)
                } else {
                    ExprKind::String(text)
                }
            }
            Some(TokenKind::RawString(text)) => {
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
            Some(TokenKind::Operator("++" | "--")) => {
                return Err(self
                    .error("increment and decrement are supported only as standalone statements"));
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

/// Decodifica escapes Dart em unidades UTF-16 e rejeita surrogates isolados.
fn decode_string(text: &str, span: Span) -> Result<String, Diagnostic> {
    let mut chars = text.chars();
    let mut units = Vec::with_capacity(text.len());
    while let Some(mut ch) = chars.next() {
        if ch == '\\' {
            ch = chars
                .next()
                .ok_or_else(|| Diagnostic::new("truncated string escape", span))?;
            match ch {
                'b' => ch = '\u{8}',
                'f' => ch = '\u{c}',
                'n' => ch = '\n',
                'r' => ch = '\r',
                't' => ch = '\t',
                'v' => ch = '\u{b}',
                'x' | 'u' => {
                    let value = if ch == 'x' {
                        hex_digits(&mut chars, 2, span)?
                    } else if chars.clone().next() == Some('{') {
                        chars.next();
                        let mut value = 0u32;
                        let mut count = 0;
                        loop {
                            let c = chars.next().ok_or_else(|| {
                                Diagnostic::new("unterminated Unicode escape", span)
                            })?;
                            if c == '}' {
                                if count == 0 {
                                    return Err(Diagnostic::new(
                                        "Unicode escape requires 1 to 6 hexadecimal digits",
                                        span,
                                    ));
                                }
                                break;
                            }
                            let digit =
                                c.to_digit(16).filter(|_| c.is_ascii()).ok_or_else(|| {
                                    Diagnostic::new(
                                        "invalid hexadecimal digit in Unicode escape",
                                        span,
                                    )
                                })?;
                            count += 1;
                            if count > 6 {
                                return Err(Diagnostic::new(
                                    "Unicode escape requires 1 to 6 hexadecimal digits",
                                    span,
                                ));
                            }
                            value = value * 16 + digit;
                        }
                        value
                    } else {
                        hex_digits(&mut chars, 4, span)?
                    };
                    if value > 0x10FFFF {
                        return Err(Diagnostic::new("Unicode escape exceeds U+10FFFF", span));
                    }
                    if value <= 0xFFFF {
                        units.push(value as u16);
                    } else {
                        let value = value - 0x10000;
                        units.push(0xD800 + (value >> 10) as u16);
                        units.push(0xDC00 + (value & 0x3FF) as u16);
                    }
                    continue;
                }
                // Dart remove a barra invertida de escapes desconhecidos.
                _ => {}
            }
        }
        let mut encoded = [0u16; 2];
        units.extend_from_slice(ch.encode_utf16(&mut encoded));
    }
    String::from_utf16(&units)
        .map_err(|_| Diagnostic::new("isolated UTF-16 surrogates are not supported", span))
}

/// Lê a quantidade exata de dígitos hexadecimais de um escape fixo.
fn hex_digits(
    chars: &mut std::str::Chars<'_>,
    count: usize,
    span: Span,
) -> Result<u32, Diagnostic> {
    let mut value = 0;
    for _ in 0..count {
        let digit = chars
            .next()
            .and_then(|c| if c.is_ascii() { c.to_digit(16) } else { None })
            .ok_or_else(|| {
                Diagnostic::new(
                    "escape requires the exact number of hexadecimal digits",
                    span,
                )
            })?;
        value = value * 16 + digit;
    }
    Ok(value)
}
/// Traduz um operador textual para sua precedência e operação sintática.
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

// Restringe palavras reservadas e nomes especiais no subconjunto inicial.
/// Identifica nomes indisponíveis como identificadores neste subconjunto.
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
    /// Verifica truncamentos em limites UTF-8, inclusive após barras e aspas escapadas.
    #[test]
    fn string_prefixes_never_panic_or_produce_invalid_spans() {
        let sources = [
            r#"void main(){print('é😀\'fim');}"#,
            r#"void main(){print("é\\\"\$\q\x41\u0042\u{1f600}");}"#,
            r#"void main(){print(r'é😀\$fim\');}"#,
            r#"void main(){print('\uD83D\uDE00');}"#,
            r#"void main(){print('\u{123456789}');}"#,
            r#"void main(){print('\xé');}"#,
            r#"void main(){print('\u{é}');}"#,
            "void main(){print('é\\\r\n');}",
        ];
        for source in sources {
            for end in source
                .char_indices()
                .map(|(i, _)| i)
                .chain(std::iter::once(source.len()))
            {
                let prefix = &source[..end];
                let result = match dartforge_lexer::lex(prefix) {
                    Ok(tokens) => {
                        for token in &tokens {
                            assert!(prefix.get(token.span.start..token.span.end).is_some());
                        }
                        parse(&tokens, prefix.len()).map(|_| ())
                    }
                    Err(error) => Err(error),
                };
                if let Err(error) = result {
                    assert!(
                        prefix.get(error.span.start..error.span.end).is_some(),
                        "{prefix:?}: {error:?}"
                    );
                }
            }
        }
    }
    #[test]
    fn escaped_and_raw_strings() {
        let p = parsed(
            r#"void main() { print('\b\f\n\r\t\v\\\"\'\$\q\0\é'); print('\x41\u0042\u{1f600}\uD83D\uDE00'); print(r'\n$x\'); print('plain'); }"#,
        );
        let StatementKind::Print(e) = &p.statements[0].kind else {
            panic!()
        };
        assert!(
            matches!(&e.kind, ExprKind::OwnedString(text) if text == "\u{8}\u{c}\n\r\t\u{b}\\\"'$q0é")
        );
        let StatementKind::Print(e) = &p.statements[1].kind else {
            panic!()
        };
        assert!(matches!(&e.kind, ExprKind::OwnedString(text) if text == "AB😀😀"));
        let StatementKind::Print(e) = &p.statements[2].kind else {
            panic!()
        };
        assert!(matches!(&e.kind, ExprKind::String(r"\n$x\")));
        let StatementKind::Print(e) = &p.statements[3].kind else {
            panic!()
        };
        assert!(matches!(&e.kind, ExprKind::String("plain")));
        let p = parsed(r#"void main() { print('\x414\u00414\u{000041}\u{D83D}\u{DE00}'); }"#);
        let StatementKind::Print(e) = &p.statements[0].kind else {
            panic!()
        };
        assert!(matches!(&e.kind, ExprKind::OwnedString(text) if text == "A4A4A😀"));
    }
    #[test]
    fn malformed_escapes_have_valid_byte_spans() {
        for literal in [
            r"\x4",
            r"\xGG",
            r"\u123",
            r"\uGGGG",
            r"\u{}",
            r"\u{110000}",
            r"\u{1234567}",
            r"\u{41 42}",
            r"\uD800",
            r"\uDC00",
            r"\uD800x",
            r"\u{D800}",
        ] {
            let source = format!("void main() {{ print('olá{literal}'); }}");
            let error = parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).unwrap_err();
            assert!(source.get(error.span.start..error.span.end).is_some());
            assert!(error.span.end <= source.len());
        }
        rejected(r#"print(R'\n');"#);
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
    fn loops_and_updates() {
        let p = parsed(
            "void main() { while(true) { break; } do { continue; } while(false); for(var i=0; i<3; i++) { print(i); } for(;;) { break; } for(f(); true; f()) {} ++i; --i; i--; i += 2; i -= 3; i *= 4; }",
        );
        assert!(matches!(p.statements[0].kind, StatementKind::While { .. }));
        assert!(matches!(
            p.statements[1].kind,
            StatementKind::DoWhile { .. }
        ));
        let StatementKind::For {
            initializer,
            update,
            ..
        } = &p.statements[2].kind
        else {
            panic!()
        };
        assert!(matches!(
            initializer.as_ref().unwrap().kind,
            StatementKind::Variable { .. }
        ));
        let StatementKind::Assign { value, .. } = &update.as_ref().unwrap().kind else {
            panic!()
        };
        assert!(matches!(
            value.kind,
            ExprKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
        assert!(matches!(
            p.statements[3].kind,
            StatementKind::For {
                initializer: None,
                condition: None,
                update: None,
                ..
            }
        ));
        assert!(matches!(
            p.statements[10].kind,
            StatementKind::Assign { .. }
        ));
    }
    #[test]
    fn invalid_loops_and_expression_updates() {
        for body in [
            "while(true) break;",
            "do {} while(true)",
            "for(var i=0;true;var x=1) {}",
            "for(i=0;true;i++,j++) {}",
            "for(break;true;i++) {}",
            "break label;",
            "continue label;",
            "var x = i++;",
            "print(++i);",
            "i++ + 1;",
            "for(i++;true;++i) print(i);",
            "for(var i=0,i=1;true;i++) {}",
        ] {
            rejected(body);
        }
        for opening in ["while(true){", "for(;;){"] {
            rejected(&format!("{}{}", opening.repeat(1000), "}".repeat(1000)));
        }
        rejected(&format!(
            "{}{}",
            "do{".repeat(1000),
            "}while(true);".repeat(1000)
        ));
        let p = parsed(
            "void main() { for(int i=0;i<2;++i) {} for(i=0;i<2;i+=1) {} for(final i=1;false;i=i) {} }",
        );
        assert_eq!(p.statements.len(), 3);
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
        // Uma árvore extensa de argumentos não pode contornar o limite compartilhado de nós.
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
