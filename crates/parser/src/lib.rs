//! Análise sintática do subconjunto Dart 3.6.2, com limites explícitos de complexidade.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    BinaryOp, Class, Expr, ExprKind, Extension, Field, Function, Parameter, Program, Statement,
    StatementKind, Token, TokenKind, Type, TypeShape, UnaryOp,
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
    let mut program = parse_unit(tokens, source_len, index_classes(tokens)?)?;
    let mut main = None;
    let mut functions = Vec::new();
    for function in program.functions {
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
    program.statements = main.ok_or_else(|| {
        Diagnostic::new(
            "expected void main() entrypoint",
            Span {
                start: source_len,
                end: source_len,
            },
        )
    })?;
    program.functions = functions;
    Ok(program)
}

/// Analisa uma biblioteca com IDs nominais fornecidos pelo resolvedor de imports.
///
/// Todas as funções, inclusive main, permanecem em functions; statements fica vazio.
/// O chamador remove diretivas dos tokens, mantendo os spans do arquivo original.
///
/// # Exemplos
///
///     let fonte = "int valor() { return 1; }";
///     let tokens = dartforge_lexer::lex(fonte).unwrap();
///     let unidade = dartforge_parser::parse_unit(&tokens, fonte.len(), Default::default()).unwrap();
///     assert_eq!(unidade.functions[0].name, "valor");
///
/// # Erros
///
/// Retorna diagnóstico para sintaxe inválida, limites excedidos ou classe própria
/// ausente no ambiente nominal. A validação da entrada pertence ao chamador.
pub fn parse_unit<'a>(
    tokens: &[Token<'a>],
    source_len: usize,
    class_ids: std::collections::BTreeMap<&'a str, u32>,
) -> Result<Program<'a>, Diagnostic> {
    let mut cursor = Cursor {
        tokens,
        index: 0,
        source_len,
        expr_nodes: 0,
        class_ids,
        types: Vec::new(),
        closure_depth: 0,
    };
    let mut classes = Vec::new();
    let mut extensions = Vec::new();
    let mut functions = Vec::new();
    while cursor.peek().is_some() {
        if matches!(
            cursor.peek(),
            Some(TokenKind::Word("class" | "abstract" | "interface" | "enum"))
        ) {
            classes.push(cursor.class()?);
        } else if cursor.peek() == Some(TokenKind::Word("extension")) {
            let id =
                u32::try_from(extensions.len()).map_err(|_| cursor.error("too many extensions"))?;
            extensions.push(cursor.extension(id)?);
        } else {
            functions.push(cursor.function()?);
        }
    }
    Ok(Program {
        types: cursor.types,
        extensions,
        classes,
        functions,
        statements: Vec::new(),
    })
}

/// Nome declarado no topo de uma biblioteca, com intervalo do identificador.
#[derive(Debug)]
pub struct TopLevelName<'a> {
    pub name: &'a str,
    pub span: Span,
}

/// Nomes exportáveis de uma unidade antes da resolução de tipos importados.
#[derive(Debug, Default)]
pub struct UnitDeclarations<'a> {
    pub classes: Vec<TopLevelName<'a>>,
    pub functions: Vec<TopLevelName<'a>>,
}

/// Indexa declarações de topo sem interpretar tipos potencialmente importados.
///
/// Não remove imports nem valida corpos: o resolvedor fornece os tokens após as
/// diretivas e usa parse_unit para a validação sintática completa.
///
/// # Exemplos
///
///     let tokens = dartforge_lexer::lex("FutureType? obter() { return null; }").unwrap();
///     let nomes = dartforge_parser::index_unit(&tokens).unwrap();
///     assert_eq!(nomes.functions[0].name, "obter");
///
/// # Erros
///
/// Retorna diagnóstico para cabeçalhos não suportados, delimitadores incompletos
/// ou extensions, cuja visibilidade em grafos de bibliotecas ainda não é suportada.
pub fn index_unit<'a>(tokens: &[Token<'a>]) -> Result<UnitDeclarations<'a>, Diagnostic> {
    let mut declarations = UnitDeclarations::default();
    let mut index = 0;
    while let Some(first) = tokens.get(index) {
        if first.kind == TokenKind::Word("extension") {
            return Err(Diagnostic::new(
                "extensions in library import graphs are not supported yet",
                first.span,
            ));
        }
        let is_class = matches!(
            first.kind,
            TokenKind::Word("class" | "abstract" | "interface" | "enum")
        );
        index += 1;
        if matches!(first.kind, TokenKind::Word("abstract" | "interface")) {
            if first.kind == TokenKind::Word("abstract")
                && tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("interface"))
            {
                index += 1;
            }
            if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Word("class")) {
                return Err(Diagnostic::new(
                    "expected class after supported modifiers",
                    first.span,
                ));
            }
            index += 1;
        }
        if !matches!(first.kind, TokenKind::Word(_)) {
            return Err(Diagnostic::new(
                "expected top-level declaration",
                first.span,
            ));
        }
        if !is_class {
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("<")) {
                let mut depth = 1usize;
                index += 1;
                while depth > 0 {
                    let token = tokens.get(index).ok_or_else(|| {
                        Diagnostic::new("unterminated type arguments", first.span)
                    })?;
                    match token.kind {
                        TokenKind::Operator("<") => depth += 1,
                        TokenKind::Operator(">") => depth -= 1,
                        _ => {}
                    }
                    index += 1;
                }
            }
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("?")) {
                index += 1;
            }
            while tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("Function")) {
                index = skip_delimited(tokens, index + 1, '(', ')', first.span)?;
            }
        }
        let name_token = tokens
            .get(index)
            .ok_or_else(|| Diagnostic::new("expected declaration name", first.span))?;
        let TokenKind::Word(name) = name_token.kind else {
            return Err(Diagnostic::new(
                "expected declaration name",
                name_token.span,
            ));
        };
        index += 1;
        let item = TopLevelName {
            name,
            span: name_token.span,
        };
        if is_class {
            declarations.classes.push(item);
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("extends")) {
                index += 1;
                if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                    return Err(Diagnostic::new("expected superclass name", first.span));
                }
                index += 1;
            }
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("implements")) {
                index += 1;
                loop {
                    if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                        return Err(Diagnostic::new("expected interface name", first.span));
                    }
                    index += 1;
                    if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol(',')) {
                        break;
                    }
                    index += 1;
                }
            }
        } else {
            declarations.functions.push(item);
            index = skip_delimited(tokens, index, '(', ')', first.span)?;
        }
        if !is_class && tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("=>")) {
            // O parser completo validará a expressão; aqui apenas indexamos nomes.
            index += 1;
            let mut delimiters = Vec::new();
            while let Some(token) = tokens.get(index) {
                match token.kind {
                    TokenKind::Symbol(';') if delimiters.is_empty() => break,
                    TokenKind::Symbol(open @ ('(' | '[' | '{')) => delimiters.push(open),
                    TokenKind::Symbol(close @ (')' | ']' | '}')) => {
                        let expected = match close {
                            ')' => '(',
                            ']' => '[',
                            _ => '{',
                        };
                        if delimiters.pop() != Some(expected) {
                            return Err(Diagnostic::new("unbalanced expression body", token.span));
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
            if tokens.get(index).is_none() {
                return Err(Diagnostic::new(
                    "expected semicolon after expression body",
                    first.span,
                ));
            }
            index += 1;
        } else {
            index = skip_delimited(tokens, index, '{', '}', first.span)?;
        }
    }
    Ok(declarations)
}

/// Avança sobre delimitadores balanceados sem recorrer nem copiar tokens.
fn skip_delimited(
    tokens: &[Token<'_>],
    start: usize,
    open: char,
    close: char,
    fallback: Span,
) -> Result<usize, Diagnostic> {
    if tokens.get(start).map(|t| t.kind) != Some(TokenKind::Symbol(open)) {
        return Err(Diagnostic::new(
            "expected declaration delimiter",
            tokens.get(start).map_or(fallback, |t| t.span),
        ));
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if token.kind == TokenKind::Symbol(open) {
            depth += 1;
        } else if token.kind == TokenKind::Symbol(close) {
            depth -= 1;
            if depth == 0 {
                return Ok(index + 1);
            }
        }
    }
    Err(Diagnostic::new(
        "unterminated declaration delimiter",
        fallback,
    ))
}
struct Cursor<'t, 'a> {
    tokens: &'t [Token<'a>],
    index: usize,
    source_len: usize,
    expr_nodes: usize,
    class_ids: std::collections::BTreeMap<&'a str, u32>,
    types: Vec<TypeShape>,
    closure_depth: usize,
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
    /// Interna uma forma estrutural e preserva IDs locais estáveis.
    fn intern(&mut self, shape: TypeShape) -> Type {
        if let Some(id) = self.types.iter().position(|item| *item == shape) {
            return Type::Applied(id as u32);
        }
        let id = u32::try_from(self.types.len()).expect("limite de tipos");
        self.types.push(shape);
        Type::Applied(id)
    }
    /// Lê anotação completa e limita recursão de formas estruturais.
    fn ty(&mut self, allow_void: bool) -> Result<Type, Diagnostic> {
        self.type_at(allow_void, 0)
    }
    /// Analisa List/Iterable e tipos de função sem generics definidos pelo usuário.
    fn type_at(&mut self, allow_void: bool, depth: usize) -> Result<Type, Diagnostic> {
        if depth >= MAX_DEPTH {
            return Err(self.error("type nesting limit exceeded"));
        }
        let word = self.peek();
        let mut ty = match word {
            Some(TokenKind::Word("int")) => Type::Int,
            Some(TokenKind::Word("String")) => Type::String,
            Some(TokenKind::Word("bool")) => Type::Bool,
            Some(TokenKind::Word("void")) => Type::Void,
            Some(TokenKind::Word("List" | "Iterable")) => {
                self.index += 1;
                self.expect(TokenKind::Operator("<"))?;
                let element = self.type_at(false, depth + 1)?;
                self.expect(TokenKind::Operator(">"))?;
                self.intern(if word == Some(TokenKind::Word("List")) {
                    TypeShape::List(element)
                } else {
                    TypeShape::Iterable(element)
                })
            }
            Some(TokenKind::Word(name)) if self.class_ids.contains_key(name) => {
                Type::Class(self.class_ids[name])
            }
            _ => return Err(self.error("expected an explicitly supported type")),
        };
        if !matches!(word, Some(TokenKind::Word("List" | "Iterable"))) {
            self.index += 1;
        }
        if self.take(TokenKind::Operator("?")) {
            ty = match ty {
                Type::Int => Type::NullableInt,
                Type::Bool => Type::NullableBool,
                Type::String => Type::NullableString,
                Type::Class(id) => Type::NullableClass(id),
                _ => return Err(self.error("nullable structural types are not supported yet")),
            };
        }
        let mut function_depth = depth;
        while self.take(TokenKind::Word("Function")) {
            function_depth += 1;
            if function_depth >= MAX_DEPTH {
                return Err(self.error("type nesting limit exceeded"));
            }
            self.expect(TokenKind::Symbol('('))?;
            let mut parameters = Vec::new();
            while self.peek() != Some(TokenKind::Symbol(')')) {
                parameters.push(self.type_at(false, depth + 1)?);
                if matches!(self.peek(),Some(TokenKind::Word(name)) if !reserved(name)) {
                    self.name()?;
                }
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Symbol(')'))?;
            ty = self.intern(TypeShape::Function {
                result: ty,
                parameters,
            });
        }
        if ty == Type::Void && !allow_void {
            return Err(self.error("void value type is unsupported"));
        }
        Ok(ty)
    }
    /// Testa uma anotação local e restaura cursor e arena após a sondagem.
    fn starts_annotation(&mut self) -> bool {
        let index = self.index;
        let count = self.types.len();
        let result = self.ty(false).is_ok()
            && matches!(self.peek(),Some(TokenKind::Word(name)) if !reserved(name));
        self.index = index;
        self.types.truncate(count);
        result
    }
    /// Lê campos inicializados e métodos de uma classe nominal.
    fn class(&mut self) -> Result<Class<'a>, Diagnostic> {
        let start = self.position();
        let is_abstract = self.take(TokenKind::Word("abstract"));
        let is_interface = self.take(TokenKind::Word("interface"));
        let is_enum = !is_abstract && !is_interface && self.take(TokenKind::Word("enum"));
        if !is_enum {
            self.expect(TokenKind::Word("class"))?;
        }
        let name = self.name()?;
        let id = *self.class_ids.get(name).ok_or_else(|| {
            self.error("declared class is missing from the supplied class environment")
        })?;
        if is_enum {
            self.expect(TokenKind::Symbol('{'))?;
            let mut enum_values = Vec::new();
            loop {
                let value = self.name()?;
                if enum_values.contains(&value) {
                    return Err(self.error("duplicate enum value"));
                }
                enum_values.push(value);
                if !self.take(TokenKind::Symbol(',')) || self.peek() == Some(TokenKind::Symbol('}'))
                {
                    break;
                }
            }
            self.expect(TokenKind::Symbol('}'))?;
            return Ok(Class {
                is_interface: false,
                library_id: 0,
                id,
                name,
                is_abstract: false,
                interfaces: vec![],
                abstract_methods: vec![],
                enum_values,
                superclass: None,
                fields: vec![],
                methods: vec![],
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        let superclass = if self.take(TokenKind::Word("extends")) {
            let parent = self.name()?;
            Some(
                *self
                    .class_ids
                    .get(parent)
                    .ok_or_else(|| self.error("unknown superclass"))?,
            )
        } else {
            None
        };
        let mut interfaces = Vec::new();
        if self.take(TokenKind::Word("implements")) {
            loop {
                let name = self.name()?;
                interfaces.push(
                    *self
                        .class_ids
                        .get(name)
                        .ok_or_else(|| self.error("unknown interface"))?,
                );
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
        }
        self.expect(TokenKind::Symbol('{'))?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut abstract_methods = Vec::new();
        while self.peek() != Some(TokenKind::Symbol('}')) {
            let index = self.index;
            let field_start = self.position();
            let is_final = self.take(TokenKind::Word("final"));
            let ty = self.ty(!is_final)?;
            let field_name = self.name()?;
            if self.peek() == Some(TokenKind::Symbol('(')) {
                if is_final {
                    return Err(self.error("final methods are not supported"));
                }
                self.index = index;
                let (method, abstract_body) = self.function_with_abstract(true)?;
                if abstract_body {
                    abstract_methods.push(method);
                } else {
                    methods.push(method);
                }
            } else {
                if ty == Type::Void {
                    return Err(self.error("fields cannot have void type"));
                }
                self.expect(TokenKind::Operator("="))?;
                let initializer = self.expression()?;
                self.expect(TokenKind::Symbol(';'))?;
                fields.push(Field {
                    name: field_name,
                    ty,
                    is_final,
                    initializer,
                    span: Span {
                        start: field_start,
                        end: self.end(),
                    },
                });
            }
        }
        self.expect(TokenKind::Symbol('}'))?;
        Ok(Class {
            is_interface,
            library_id: 0,
            is_abstract,
            interfaces,
            abstract_methods,
            enum_values: vec![],
            id,
            name,
            superclass,
            fields,
            methods,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê uma extension nomeada sobre tipo não anulável, contendo somente métodos.
    fn extension(&mut self, id: u32) -> Result<Extension<'a>, Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word("extension"))?;
        let name = self.name()?;
        self.expect(TokenKind::Word("on"))?;
        let on_type = self.ty(false)?;
        if !matches!(
            on_type,
            Type::Int | Type::String | Type::Bool | Type::Class(_)
        ) {
            return Err(self.error("extensions require a non-null primitive or class type"));
        }
        self.expect(TokenKind::Symbol('{'))?;
        let mut methods = Vec::new();
        while self.peek() != Some(TokenKind::Symbol('}')) {
            methods.push(self.function()?);
        }
        self.expect(TokenKind::Symbol('}'))?;
        Ok(Extension {
            id,
            name,
            on_type,
            methods,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê a assinatura tipada e o corpo em bloco ou expressão de uma função/método.
    fn function(&mut self) -> Result<Function<'a>, Diagnostic> {
        self.function_with_abstract(false)
            .map(|(function, _)| function)
    }
    /// Distingue assinatura abstrata terminada por ponto e vírgula de corpo concreto.
    fn function_with_abstract(
        &mut self,
        allow_abstract: bool,
    ) -> Result<(Function<'a>, bool), Diagnostic> {
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
        let abstract_body = allow_abstract && self.take(TokenKind::Symbol(';'));
        let body = if abstract_body {
            Vec::new()
        } else if self.take(TokenKind::Operator("=>")) {
            let start = self.position();
            let value = self.expression()?;
            self.expect(TokenKind::Symbol(';'))?;
            let span = Span {
                start,
                end: self.end(),
            };
            if return_type == Type::Void {
                // Dart permite `void f() => 42`: avalia o valor, mas o descarta.
                vec![
                    Statement {
                        kind: StatementKind::Expression(value),
                        span,
                    },
                    Statement {
                        kind: StatementKind::Return(None),
                        span,
                    },
                ]
            } else {
                vec![Statement {
                    kind: StatementKind::Return(Some(value)),
                    span,
                }]
            }
        } else {
            self.block(0)?
        };
        Ok((
            Function {
                name,
                return_type,
                parameters,
                body,
                span: Span {
                    start,
                    end: self.end(),
                },
            },
            abstract_body,
        ))
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
        let kind = if matches!(self.peek(), Some(TokenKind::Word("var" | "final")))
            || self.starts_annotation()
        {
            if !allow_declaration {
                return Err(self.error("declarations are not supported in for updates"));
            }
            let is_final = self.take(TokenKind::Word("final"));
            let inferred = self.take(TokenKind::Word("var"));
            if is_final && inferred {
                return Err(self.error("final var is not supported; use final name = expression"));
            }
            let annotation = if !inferred && self.starts_annotation() {
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
            if self.take(TokenKind::Operator("=")) {
                match value.kind {
                    ExprKind::Member { receiver, name } => StatementKind::FieldAssign {
                        receiver: *receiver,
                        name,
                        value: self.expression()?,
                    },
                    ExprKind::Index { receiver, index } => StatementKind::IndexAssign {
                        receiver: *receiver,
                        index: *index,
                        value: self.expression()?,
                    },
                    _ => return Err(self.error("assignment requires a member or index target")),
                }
            } else {
                if !matches!(
                    value.kind,
                    ExprKind::Call { .. }
                        | ExprKind::MethodCall { .. }
                        | ExprKind::Construct { .. }
                        | ExprKind::Invoke { .. }
                ) {
                    return Err(Diagnostic::new(
                        "only calls are supported as expression statements",
                        value.span,
                    ));
                }
                StatementKind::Expression(value)
            }
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
            let next_min = if op == BinaryOp::IfNull {
                precedence
            } else {
                precedence + 1
            };
            let right = self.binary(next_min, depth + 1)?;
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
            Some(TokenKind::Word("this")) => {
                self.index += 1;
                ExprKind::This
            }
            Some(TokenKind::Word("null")) => {
                self.index += 1;
                ExprKind::Null
            }
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
                if self.class_ids.contains_key(name) && self.take(TokenKind::Symbol('.')) {
                    ExprKind::EnumValue {
                        class_id: self.class_ids[name],
                        name: self.name()?,
                    }
                } else if self.peek() == Some(TokenKind::Symbol('(')) {
                    let arguments = self.arguments(depth)?;
                    if let Some(&class_id) = self.class_ids.get(name) {
                        if !arguments.is_empty() {
                            return Err(self.error(
                                "only implicit constructors without arguments are supported",
                            ));
                        }
                        ExprKind::Construct { class_id }
                    } else {
                        ExprKind::Call { name, arguments }
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
            Some(TokenKind::Symbol('[')) | Some(TokenKind::Operator("<")) => {
                let element_type = if self.take(TokenKind::Operator("<")) {
                    let ty = self.ty(false)?;
                    self.expect(TokenKind::Operator(">"))?;
                    Some(ty)
                } else {
                    None
                };
                self.expect(TokenKind::Symbol('['))?;
                let mut elements = Vec::new();
                while self.peek() != Some(TokenKind::Symbol(']')) {
                    elements.push(self.binary(0, depth + 1)?);
                    if !self.take(TokenKind::Symbol(',')) {
                        break;
                    }
                }
                self.expect(TokenKind::Symbol(']'))?;
                ExprKind::List {
                    element_type,
                    elements,
                }
            }
            Some(TokenKind::Symbol('(')) if self.starts_closure() => self.closure(depth)?,
            Some(TokenKind::Symbol('(')) => {
                self.index += 1;
                let mut value = self.binary(0, depth + 1)?;
                self.expect(TokenKind::Symbol(')'))?;
                value.span = Span {
                    start,
                    end: self.end(),
                };
                return self.postfix(value, depth);
            }
            Some(TokenKind::Operator("++" | "--")) => {
                return Err(self
                    .error("increment and decrement are supported only as standalone statements"));
            }
            _ => return Err(self.error("expected a supported expression")),
        };
        self.postfix(
            Expr {
                kind,
                span: Span {
                    start,
                    end: self.end(),
                },
            },
            depth,
        )
    }
    /// Reconhece parâmetros de closure sem confundir agrupamento de expressões.
    fn starts_closure(&self) -> bool {
        let mut balance = 0usize;
        for (offset, token) in self.tokens[self.index..].iter().enumerate() {
            match token.kind {
                TokenKind::Symbol('(') => balance += 1,
                TokenKind::Symbol(')') => {
                    balance = balance.saturating_sub(1);
                    if balance == 0 {
                        return matches!(
                            self.tokens.get(self.index + offset + 1).map(|t| t.kind),
                            Some(TokenKind::Operator("=>") | TokenKind::Symbol('{'))
                        );
                    }
                }
                _ => {}
            }
        }
        false
    }
    /// Lê closure com parâmetros tipados ou inferidos e limita corpos recursivos.
    fn closure(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
        self.closure_depth += 1;
        if self.closure_depth > MAX_DEPTH {
            return Err(self.error("closure nesting limit exceeded"));
        }
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let start = self.position();
            let ty = if self.starts_annotation() {
                self.ty(false)?
            } else {
                Type::Inferred
            };
            let name = self.name()?;
            parameters.push(Parameter {
                name,
                ty,
                span: Span {
                    start,
                    end: self.end(),
                },
            });
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        let is_arrow = self.take(TokenKind::Operator("=>"));
        let body = if is_arrow {
            let value = self.binary(0, depth + 1)?;
            let span = value.span;
            vec![Statement {
                kind: StatementKind::Return(Some(value)),
                span,
            }]
        } else {
            self.block(depth + 1)?
        };
        self.closure_depth -= 1;
        Ok(ExprKind::Closure {
            is_arrow,
            parameters,
            return_type: Type::Inferred,
            body,
        })
    }
    /// Aplica asserções pós-fixas sem permitir que contornem o limite de nós.
    fn postfix(&mut self, mut value: Expr<'a>, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        loop {
            let kind = if self.peek() == Some(TokenKind::Symbol('(')) {
                self.charge(depth)?;
                ExprKind::Invoke {
                    callee: Box::new(value),
                    arguments: self.arguments(depth)?,
                }
            } else if self.take(TokenKind::Symbol('[')) {
                self.charge(depth)?;
                let index = self.binary(0, depth + 1)?;
                self.expect(TokenKind::Symbol(']'))?;
                ExprKind::Index {
                    receiver: Box::new(value),
                    index: Box::new(index),
                }
            } else if self.take(TokenKind::Operator("!")) {
                self.charge(depth)?;
                ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    operand: Box::new(value),
                }
            } else if self.take(TokenKind::Symbol('.')) {
                self.charge(depth)?;
                let name = self.name()?;
                if self.peek() == Some(TokenKind::Symbol('(')) {
                    let arguments = self.arguments(depth)?;
                    ExprKind::MethodCall {
                        receiver: Box::new(value),
                        name,
                        arguments,
                    }
                } else {
                    ExprKind::Member {
                        receiver: Box::new(value),
                        name,
                    }
                }
            } else {
                break;
            };
            let span = match &kind {
                ExprKind::Unary { operand, .. } => Span {
                    start: operand.span.start,
                    end: self.end(),
                },
                ExprKind::Member { receiver, .. }
                | ExprKind::MethodCall { receiver, .. }
                | ExprKind::Index { receiver, .. }
                | ExprKind::Invoke {
                    callee: receiver, ..
                } => Span {
                    start: receiver.span.start,
                    end: self.end(),
                },
                _ => unreachable!(),
            };
            value = Expr { kind, span };
        }
        Ok(value)
    }
}

/// Pré-indexa somente classes de topo para permitir referências posteriores.
fn index_classes<'a>(
    tokens: &[Token<'a>],
) -> Result<std::collections::BTreeMap<&'a str, u32>, Diagnostic> {
    let mut result = std::collections::BTreeMap::new();
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            TokenKind::Symbol('{') => depth += 1,
            TokenKind::Symbol('}') => depth = depth.saturating_sub(1),
            TokenKind::Word("class" | "enum") if depth == 0 => {
                let next = tokens
                    .get(index + 1)
                    .ok_or_else(|| Diagnostic::new("expected class name", token.span))?;
                let TokenKind::Word(name) = next.kind else {
                    return Err(Diagnostic::new("expected class name", next.span));
                };
                if reserved(name) {
                    return Err(Diagnostic::new("unsupported class name", next.span));
                }
                let id = u32::try_from(result.len())
                    .map_err(|_| Diagnostic::new("too many classes", token.span))?;
                if result.insert(name, id).is_some() {
                    return Err(Diagnostic::new("duplicate class name", next.span));
                }
            }
            _ => {}
        }
    }
    Ok(result)
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
        "??" => (0, BinaryOp::IfNull),
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
        "%" => (6, BinaryOp::Remainder),
        _ => return None,
    })
}

// Restringe palavras reservadas e nomes especiais no subconjunto inicial.
/// Identifica nomes indisponíveis como identificadores neste subconjunto.
fn reserved(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "List"
            | "Iterable"
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
    /// Formas genéricas, closures e chamadas pós-fixas mantêm estrutura e tipos locais.
    #[test]
    fn closures_lists_function_types_and_indices() {
        let source = "int Function(int) maker(){return (int x)=>x%2;} List<List<int>> nested(){return <List<int>>[<int>[1]];} void main(){List<int> xs=<int>[1,2,3]; Iterable<int> ys=xs.where((x)=>x%2==0); int Function(int) f=(x){return x+1;}; xs[0]=f(2); print(((int x)=>x+1)(xs[0]));}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 3);
        let program = parse(&tokens, source.len()).unwrap();
        assert!(program.types.iter().any(|shape|matches!(shape,TypeShape::Function{result:Type::Int,parameters} if parameters==&vec![Type::Int])));
        assert!(matches!(
            program.statements[3].kind,
            StatementKind::IndexAssign { .. }
        ));
        let StatementKind::Print(value) = &program.statements[4].kind else {
            panic!()
        };
        assert!(matches!(value.kind, ExprKind::Invoke { .. }));
        let source = "int Function(int) make() => (int x) { return x; }; void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 2);
        assert!(parse(&tokens, source.len()).is_ok());
    }
    /// Closures e formas recursivas não contornam limites nem aceitam parâmetros nomeados.
    #[test]
    fn structural_subset_rejections_and_depth() {
        for body in [
            "var f=({int x})=>x;",
            "var f=([int x])=>x;",
            "List<int>? xs=null;",
            "var xs=<int,String>[];",
            "var xs=[1,2;",
            "var f=(x=1)=>x;",
        ] {
            rejected(body);
        }
        let mut body = "return 1;".to_owned();
        for _ in 0..70 {
            body = format!("return (){{{body}}};");
        }
        rejected(&format!("var f=(){{{body}}};"));
        let ty = format!("int{}", " Function()".repeat(80));
        rejected(&format!("{ty} f=()=>1;"));
    }
    /// Indexa contratos abstratos e enums antes de resolver implementações posteriores.
    #[test]
    fn abstract_interfaces_enums_and_import_index() {
        let source = "abstract class I { int f(); } class A implements I,J { int f()=>1; } abstract class J {} enum E { a, name, } void main(){print(E.name.index);}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let index = index_unit(&tokens).unwrap();
        assert_eq!(
            index
                .classes
                .iter()
                .map(|item| item.name)
                .collect::<Vec<_>>(),
            ["I", "A", "J", "E"]
        );
        let program = parse(&tokens, source.len()).unwrap();
        assert!(program.classes[0].is_abstract);
        assert_eq!(program.classes[0].abstract_methods.len(), 1);
        assert_eq!(program.classes[1].interfaces, [0, 2]);
        assert_eq!(program.classes[3].enum_values, ["a", "name"]);
        let StatementKind::Print(expression) = &program.statements[0].kind else {
            panic!()
        };
        let ExprKind::Member {
            receiver,
            name: "index",
        } = &expression.kind
        else {
            panic!()
        };
        assert!(matches!(
            receiver.kind,
            ExprKind::EnumValue {
                class_id: 3,
                name: "name"
            }
        ));
    }
    /// Aceita apenas a ordem Dart abstract interface class e interface class.
    #[test]
    fn interface_class_modifiers_and_index() {
        let source = "interface class I { int f()=>1; } abstract interface class J { int f(); } class C extends I implements J {} void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().classes.len(), 3);
        let program = parse(&tokens, source.len()).unwrap();
        assert!(program.classes[0].is_interface);
        assert!(!program.classes[0].is_abstract);
        assert!(program.classes[1].is_interface && program.classes[1].is_abstract);
        assert!(program.classes.iter().all(|class| class.library_id == 0));
    }
    /// Modificadores de biblioteca e enums enriquecidos permanecem fora do subconjunto.
    #[test]
    fn rejects_unsupported_nominal_forms() {
        for source in [
            "interface abstract class I {}",
            "abstract base class I {}",
            "enum E {}",
            "enum E { a, a }",
            "enum E { a; int f()=>1; }",
            "enum E implements I { a }",
            "class A implements {}",
            "int f();",
        ] {
            let source = format!("{source} void main(){{}}");
            assert!(
                parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
    }
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
    /// Confere tipos anuláveis, precedência, associatividade e intervalos pós-fixos.
    /// Valida referências nominais posteriores, herança e acesso explícito a membros.
    /// Mantém main de biblioteca e resolve IDs globais sem concatenar fontes.
    /// Analisa extensions em uma unidade e preserva seus tipos e IDs independentes.
    #[test]
    fn named_extensions_single_unit() {
        let source = "extension Numbers on int { int twice() { return this*2; } } void main() { print(3.twice()); } extension Objects on Box { int read() { return this.value; } } class Box { int value=1; } extension Text on String { String same() { return this; } } extension Flags on bool { bool flip() { return !this; } }";
        let p = parsed(source);
        assert_eq!(p.extensions.len(), 4);
        assert_eq!(p.extensions[0].on_type, Type::Int);
        assert_eq!(p.extensions[1].on_type, Type::Class(0));
        assert_eq!(p.extensions[2].on_type, Type::String);
        assert_eq!(p.extensions[3].on_type, Type::Bool);
        assert_eq!(p.extensions[3].id, 3);
        assert_eq!(p.extensions[0].methods[0].name, "twice");
        assert!(
            source[p.extensions[0].span.start..p.extensions[0].span.end]
                .starts_with("extension Numbers")
        );
        let StatementKind::Print(e) = &p.statements[0].kind else {
            panic!()
        };
        assert!(matches!(e.kind, ExprKind::MethodCall { name: "twice", .. }));
        let error = index_unit(&dartforge_lexer::lex(source).unwrap()).unwrap_err();
        assert!(
            error
                .message
                .contains("extensions in library import graphs")
        );
    }

    /// Rejeita as formas de extension fora do subconjunto definido nesta etapa.
    #[test]
    fn unsupported_extension_declarations() {
        for declaration in [
            "extension on int {}",
            "extension E<T> on int {}",
            "extension E on int? {}",
            "extension E on void {}",
            "extension E on Missing {}",
            "extension E on int { int field=1; }",
            "extension E on int { static int f(){return 1;} }",
            "extension E on int { int get value{return 1;} }",
            "extension E on int { set value(int x){} }",
            "extension E on int {",
        ] {
            let source = format!("{declaration} void main() {{}}");
            let tokens = dartforge_lexer::lex(&source).unwrap();
            assert!(parse(&tokens, source.len()).is_err(), "{declaration}");
        }
    }
    #[test]
    fn parse_library_with_imported_nominal_environment() {
        let source = "import 'base.dart'; Remote? make(Remote? x) { return x; } class Local extends Remote { Remote? field=null; } int main() { return 1; }";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let tokens = &tokens[3..];
        let names = index_unit(tokens).unwrap();
        assert_eq!(names.classes[0].name, "Local");
        assert_eq!(
            names.functions.iter().map(|n| n.name).collect::<Vec<_>>(),
            ["make", "main"]
        );
        assert_eq!(
            &source[names.classes[0].span.start..names.classes[0].span.end],
            "Local"
        );
        let env = std::collections::BTreeMap::from([("Remote", 17), ("Local", 23)]);
        let p = parse_unit(tokens, source.len(), env).unwrap();
        assert!(p.statements.is_empty());
        assert_eq!(p.classes[0].id, 23);
        assert_eq!(p.classes[0].superclass, Some(17));
        assert_eq!(p.functions[0].return_type, Type::NullableClass(17));
        assert_eq!(p.functions[1].name, "main");
        assert_eq!(p.functions[1].return_type, Type::Int);
        assert!(parse_unit(tokens, source.len(), Default::default()).is_err());
        let library = "int f() { return 1; }";
        let tokens = dartforge_lexer::lex(library).unwrap();
        assert!(parse_unit(&tokens, library.len(), Default::default()).is_ok());
        assert!(parse(&tokens, library.len()).is_err());
    }

    /// Indexa nomes sem resolver tipos externos e sem incluir membros ou locais.
    #[test]
    fn unit_index_is_top_level_and_checks_delimiters() {
        let source = "Unknown? make(Other x) { if(true) {} return null; } class Own extends Later { int method(){return 1;} } class Later {}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let names = index_unit(&tokens).unwrap();
        assert_eq!(names.functions.len(), 1);
        assert_eq!(
            names.classes.iter().map(|n| n.name).collect::<Vec<_>>(),
            ["Own", "Later"]
        );
        for bad in [
            "class",
            "class A",
            "class A extends",
            "class A {",
            "int f(",
            "int f() {",
            "int x=1;",
            "}",
        ] {
            assert!(
                index_unit(&dartforge_lexer::lex(bad).unwrap()).is_err(),
                "{bad}"
            );
        }
        let source = "class A {}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert!(parse_unit(&tokens, source.len(), Default::default()).is_err());
    }
    #[test]
    fn classes_fields_methods_and_forward_types() {
        let source = "Child? make(Child? x) { return x; } void main() { Child c=Child(); c.value=4; print(c.read()); c.self()!.value=5; } class Child extends Base { final String label='x'; Child? self() { return this; } int read() { return this.value; } } class Base { int value=1; }";
        let p = parsed(source);
        assert_eq!(p.classes.len(), 2);
        assert_eq!(p.classes[0].id, 0);
        assert_eq!(p.classes[1].id, 1);
        assert_eq!(p.classes[0].superclass, Some(1));
        assert_eq!(p.functions[0].return_type, Type::NullableClass(0));
        assert_eq!(p.classes[0].fields[0].ty, Type::String);
        assert!(p.classes[0].fields[0].is_final);
        assert_eq!(p.classes[0].methods.len(), 2);
        let StatementKind::Variable {
            annotation,
            initializer,
            ..
        } = &p.statements[0].kind
        else {
            panic!()
        };
        assert_eq!(*annotation, Some(Type::Class(0)));
        assert!(matches!(
            initializer.kind,
            ExprKind::Construct { class_id: 0 }
        ));
        assert!(matches!(
            p.statements[1].kind,
            StatementKind::FieldAssign { name: "value", .. }
        ));
        let StatementKind::Print(e) = &p.statements[2].kind else {
            panic!()
        };
        assert!(matches!(e.kind, ExprKind::MethodCall { name: "read", .. }));
        assert_eq!(&source[e.span.start..e.span.end], "c.read()");
        let StatementKind::Return(Some(e)) = &p.classes[0].methods[1].body[0].kind else {
            panic!()
        };
        assert!(matches!(e.kind, ExprKind::Member { name: "value", .. }));
        let p = parsed("class C {} void main(){ final C c=C(); C(); var x=C(); }");
        assert!(matches!(
            p.statements[1].kind,
            StatementKind::Expression(Expr {
                kind: ExprKind::Construct { .. },
                ..
            })
        ));
    }
    /// Rejeita construtores explícitos e demais formas fora do subconjunto nominal.
    #[test]
    fn invalid_class_forms_and_member_limits() {
        for source in [
            "class C { C() {} } void main(){}",
            "class C { int x; } void main(){}",
            "class C { static int x=1; } void main(){}",
            "class C { void x=1; } void main(){}",
            "class C extends Missing {} void main(){}",
            "class C {} class C {} void main(){}",
            "class int {} void main(){}",
            "class C<T> {} void main(){}",
            "class C {} void main(){ C(1); }",
            "class C {} void main(){ new C(); }",
            "class C {} void main(){ class Nested{} }",
            "void main(){ print(1.5); }",
            "void main(){ c.field+=1; }",
            "void main(){ c.field++; }",
            "void main(){ c.(); }",
            "void main(){ super.f(); }",
        ] {
            let result = dartforge_lexer::lex(source).and_then(|t| parse(&t, source.len()));
            assert!(result.is_err(), "{source}");
        }
        rejected(&format!("print(c{});", ".x".repeat(1000)));
        rejected(&format!("print(c{});", ".x()".repeat(1000)));
    }
    #[test]
    fn nullable_types_and_operators() {
        let source = "int? f(String? s, bool? b) { int? x=null; final String? y=null; return x; } void main() { bool? b=null; print(null ?? true || false ?? false); print((f(null,null))!); print(!b!); }";
        let p = parsed(source);
        assert_eq!(p.functions[0].return_type, Type::NullableInt);
        assert_eq!(p.functions[0].parameters[0].ty, Type::NullableString);
        assert_eq!(p.functions[0].parameters[1].ty, Type::NullableBool);
        assert!(matches!(
            p.functions[0].body[0].kind,
            StatementKind::Variable {
                annotation: Some(Type::NullableInt),
                ..
            }
        ));
        assert!(matches!(
            p.functions[0].body[1].kind,
            StatementKind::Variable {
                annotation: Some(Type::NullableString),
                ..
            }
        ));
        let StatementKind::Print(e) = &p.statements[1].kind else {
            panic!()
        };
        let ExprKind::Binary {
            op: BinaryOp::IfNull,
            left,
            right,
        } = &e.kind
        else {
            panic!()
        };
        assert!(matches!(left.kind, ExprKind::Null));
        let ExprKind::Binary {
            op: BinaryOp::IfNull,
            left,
            ..
        } = &right.kind
        else {
            panic!()
        };
        assert!(matches!(
            left.kind,
            ExprKind::Binary {
                op: BinaryOp::Or,
                ..
            }
        ));
        let StatementKind::Print(e) = &p.statements[2].kind else {
            panic!()
        };
        assert!(matches!(
            e.kind,
            ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            }
        ));
        assert_eq!(&source[e.span.start..e.span.end], "(f(null,null))!");
        let StatementKind::Print(e) = &p.statements[3].kind else {
            panic!()
        };
        let ExprKind::Unary {
            op: UnaryOp::Not,
            operand,
        } = &e.kind
        else {
            panic!()
        };
        assert!(matches!(
            operand.kind,
            ExprKind::Unary {
                op: UnaryOp::NullAssert,
                ..
            }
        ));
    }
    /// Rejeita tipos inválidos e impede cadeias nulas de contornarem os limites.
    #[test]
    fn nullable_syntax_rejections_and_limits() {
        for source in [
            "void? main() {}",
            "void f(void? x) {} void main() {}",
            "int?? f() {} void main() {}",
            "void main() { int?? x=null; }",
            "void main() { var x=1?2; }",
            "void main() { print(null ??); }",
        ] {
            assert!(
                parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
        rejected(&format!("print(null{});", "!".repeat(1000)));
        rejected(&format!("print({}null);", "null??".repeat(1000)));
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
            "var x = ;",
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
    fn expression_bodies_lower_to_returns_and_index_libraries() {
        let source = "int soma(int a, int b) => a + b; class C { int value() => 7; } void main() => print(soma(1, 2));";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let names = index_unit(&tokens).unwrap();
        assert_eq!(names.functions.len(), 2);
        assert_eq!(names.classes.len(), 1);
        let program = parse(&tokens, source.len()).unwrap();
        assert!(matches!(
            program.functions[0].body[0].kind,
            StatementKind::Return(Some(_))
        ));
        assert!(matches!(
            program.classes[0].methods[0].body[0].kind,
            StatementKind::Return(Some(_))
        ));
        assert!(matches!(
            program.statements[0].kind,
            StatementKind::Expression(_)
        ));
        assert!(matches!(
            program.statements[1].kind,
            StatementKind::Return(None)
        ));
        for invalid in ["int f() => ; void main() {}", "int f() => 1 void main() {}"] {
            let tokens = dartforge_lexer::lex(invalid).unwrap();
            assert!(parse(&tokens, invalid.len()).is_err());
        }
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
