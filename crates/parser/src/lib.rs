//! Análise sintática do subconjunto Dart 3.6.2, com limites explícitos de complexidade.
//! Mixins aceitam implements e aplicações with, mas restrições on e padrões de
//! objeto com extração de campos permanecem explicitamente fora do subconjunto.
//! Metadados limitam-se a override/deprecated/Deprecated e Native escalar em
//! funções external de topo; campos, parâmetros e anotações customizadas são rejeitados.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    Annotation, AnnotationKind, BinaryOp, Class, ClassKind, ClassModifier, Constructor,
    ConstructorParameter, Expr, ExprKind, Extension, Field, Function, NativeBinding, NativeType,
    Parameter, Pattern, Program, Statement, StatementKind, SwitchArm, SwitchCase, Token, TokenKind,
    Type, TypeShape, UnaryOp,
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
            if function.native_binding.is_some() {
                return Err(Diagnostic::new(
                    "native main entrypoints are not supported",
                    function.span,
                ));
            }
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
        type_parameters: Vec::new(),
        guard_start: None,
    };
    let mut classes = Vec::new();
    let mut extensions = Vec::new();
    let mut functions = Vec::new();
    while cursor.peek().is_some() {
        let declaration_index = skip_metadata(tokens, cursor.index)?;
        let declaration_kind = tokens.get(declaration_index).map(|token| token.kind);
        if matches!(
            declaration_kind,
            Some(TokenKind::Word(
                "class" | "abstract" | "interface" | "enum" | "base" | "final" | "sealed" | "mixin"
            ))
        ) {
            classes.push(cursor.class()?);
        } else if declaration_kind == Some(TokenKind::Word("extension")) {
            if cursor.peek() == Some(TokenKind::Symbol('@')) {
                return Err(cursor.error("annotations on extensions are not supported yet"));
            }
            let id =
                u32::try_from(extensions.len()).map_err(|_| cursor.error("too many extensions"))?;
            extensions.push(cursor.extension(id)?);
        } else {
            functions.push(cursor.function()?);
        }
    }
    let program = Program {
        types: cursor.types,
        extensions,
        classes,
        functions,
        statements: Vec::new(),
    };
    validate_metadata_bindings(&program, &cursor.class_ids)?;
    Ok(program)
}

/// Rejeita metadados conhecidos cujo nome não designa inequivocamente o builtin.
/// Membros em interfaces e mixins locais contam conservadoramente como sombra;
/// a resolução geral de constantes usadas como anotações não é implementada.
fn validate_metadata_bindings(
    program: &Program<'_>,
    class_ids: &std::collections::BTreeMap<&str, u32>,
) -> Result<(), Diagnostic> {
    // O caminho comum sem metadata não aloca tabelas nem percorre hierarquias.
    if program.functions.iter().all(|f| f.annotations.is_empty())
        && program.classes.iter().all(|c| {
            c.annotations.is_empty()
                && c.methods
                    .iter()
                    .chain(&c.abstract_methods)
                    .all(|f| f.annotations.is_empty())
        })
        && program
            .extensions
            .iter()
            .all(|e| e.methods.iter().all(|f| f.annotations.is_empty()))
    {
        return Ok(());
    }
    let global_names: std::collections::HashSet<_> = class_ids
        .keys()
        .copied()
        .chain(program.functions.iter().map(|function| function.name))
        .collect();
    let classes: std::collections::HashMap<_, _> = program
        .classes
        .iter()
        .map(|class| (class.id, class))
        .collect();
    for class in &program.classes {
        check_metadata_names(&class.annotations, &global_names, &Default::default())?;
        if class
            .methods
            .iter()
            .chain(&class.abstract_methods)
            .all(|f| f.annotations.is_empty())
        {
            continue;
        }
        let mut members = std::collections::HashSet::new();
        let mut visited = std::collections::HashSet::new();
        let mut pending = vec![class.id];
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(class) = classes.get(&id) else {
                continue;
            };
            members.extend(class.fields.iter().map(|field| field.name));
            members.extend(class.enum_values.iter().copied());
            members.extend(
                class
                    .methods
                    .iter()
                    .chain(&class.abstract_methods)
                    .map(|method| method.name),
            );
            pending.extend(class.superclass);
            pending.extend(&class.interfaces);
            pending.extend(&class.mixins);
        }
        for method in class.methods.iter().chain(&class.abstract_methods) {
            check_metadata_names(&method.annotations, &global_names, &members)?;
        }
    }
    for function in &program.functions {
        check_metadata_names(&function.annotations, &global_names, &Default::default())?;
    }
    for extension in &program.extensions {
        let members = extension.methods.iter().map(|method| method.name).collect();
        for method in &extension.methods {
            check_metadata_names(&method.annotations, &global_names, &members)?;
        }
    }
    Ok(())
}
/// Exige que o nome de cada anotação reconhecida permaneça sem sombreamento.
fn check_metadata_names(
    annotations: &[Annotation],
    globals: &std::collections::HashSet<&str>,
    members: &std::collections::HashSet<&str>,
) -> Result<(), Diagnostic> {
    for annotation in annotations {
        let name = match annotation.kind {
            AnnotationKind::Override => "override",
            AnnotationKind::Deprecated { message: Some(_) } => "Deprecated",
            AnnotationKind::Deprecated { message: None } => "deprecated",
        };
        if globals.contains(name) || members.contains(name) {
            return Err(Diagnostic::new(
                format!("shadowed annotation {name} is not supported in this subset"),
                annotation.span,
            ));
        }
    }
    Ok(())
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
    while tokens.get(index).is_some() {
        index = skip_metadata(tokens, index)?;
        if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Word("external")) {
            index += 1;
        }
        let first = tokens.get(index).ok_or_else(|| {
            Diagnostic::new(
                "expected declaration after metadata",
                tokens.last().unwrap().span,
            )
        })?;
        if first.kind == TokenKind::Word("extension") {
            return Err(Diagnostic::new(
                "extensions in library import graphs are not supported yet",
                first.span,
            ));
        }
        let is_class = matches!(
            first.kind,
            TokenKind::Word(
                "class" | "abstract" | "interface" | "enum" | "base" | "final" | "sealed" | "mixin"
            )
        );
        index += 1;
        if is_class {
            index = nominal_header(tokens, index - 1)?.name_index;
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
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("on")) {
                return Err(Diagnostic::new(
                    "mixin on constraints are not supported yet",
                    tokens[index].span,
                ));
            }
            for clause in ["with", "implements"] {
                if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word(clause)) {
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
            }
        } else {
            declarations.functions.push(item);
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("<")) {
                index += 1;
                while tokens
                    .get(index)
                    .is_some_and(|t| t.kind != TokenKind::Operator(">"))
                {
                    index += 1;
                }
                if tokens.get(index).is_none() {
                    return Err(Diagnostic::new("unterminated type parameters", first.span));
                }
                index += 1;
            }
            index = skip_delimited(tokens, index, '(', ')', first.span)?;
            if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Symbol(';')) {
                index += 1;
                continue;
            }
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
    type_parameters: Vec<&'a str>,
    guard_start: Option<usize>,
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
            Some(TokenKind::Word(name)) if self.type_parameters.contains(&name) => Type::Parameter(
                self.type_parameters
                    .iter()
                    .position(|p| *p == name)
                    .unwrap() as u32,
            ),
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
        let (annotations, native_binding) = self.metadata()?;
        if native_binding.is_some()
            || annotations
                .iter()
                .any(|annotation| matches!(annotation.kind, AnnotationKind::Override))
        {
            return Err(self.error("Native and override annotations are not valid on classes"));
        }
        let header = nominal_header(self.tokens, self.index)?;
        self.index = header.name_index;
        let NominalHeader {
            is_abstract,
            is_interface,
            is_enum,
            kind,
            modifier,
            ..
        } = header;
        let name = self.name()?;
        let id = *self.class_ids.get(name).ok_or_else(|| {
            self.error("declared class is missing from the supplied class environment")
        })?;
        if is_enum {
            return self.enumeration(id, name, start, annotations);
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
        if self.peek() == Some(TokenKind::Word("on")) {
            return Err(self.error("mixin on constraints are not supported yet"));
        }
        let mixins = self.nominal_list("with")?;
        if kind != ClassKind::Class && (superclass.is_some() || !mixins.is_empty()) {
            return Err(
                self.error("mixin declarations cannot declare extends or with in this subset")
            );
        }
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
        let mut constructor = None;
        while self.peek() != Some(TokenKind::Symbol('}')) {
            let index = self.index;
            let (metadata, native) = self.metadata()?;
            if self.peek() == Some(TokenKind::Word(name))
                && self.tokens.get(self.index + 1).map(|token| token.kind)
                    == Some(TokenKind::Symbol('('))
            {
                if kind != ClassKind::Class {
                    return Err(
                        self.error("mixin declarations cannot declare generative constructors")
                    );
                }
                if constructor.is_some() {
                    return Err(self.error("only one unnamed constructor is supported"));
                }
                if !metadata.is_empty() || native.is_some() {
                    return Err(self.error("constructor annotations are not supported yet"));
                }
                constructor = Some(self.constructor(name)?);
                continue;
            }
            let field_start = self.position();
            let is_final = self.take(TokenKind::Word("final"));
            let ty = self.ty(!is_final)?;
            let getter = self.take(TokenKind::Word("get"));
            let field_name = self.name()?;
            if getter || self.peek() == Some(TokenKind::Symbol('(')) {
                if is_final {
                    return Err(self.error("final methods are not supported"));
                }
                self.index = index;
                let (method, abstract_body) = self.function_with_abstract(true, false)?;
                if abstract_body {
                    abstract_methods.push(method);
                } else {
                    methods.push(method);
                }
            } else {
                if !metadata.is_empty() || native.is_some() {
                    return Err(self.error("annotations on fields are not supported yet"));
                }
                if ty == Type::Void {
                    return Err(self.error("fields cannot have void type"));
                }
                let initializer = if self.take(TokenKind::Operator("=")) {
                    Some(self.expression()?)
                } else {
                    None
                };
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
        if let Some(constructor) = &mut constructor {
            for parameter in &mut constructor.parameters {
                if let Some(name) = parameter.field
                    && let Some(field) = fields.iter().find(|field| field.name == name)
                {
                    parameter.ty = field.ty;
                }
            }
        }
        Ok(Class {
            constructor,
            annotations,
            is_mixin_application: false,
            mixin_origin: None,
            modifier,
            kind,
            mixins,
            is_interface,
            library_id: 0,
            is_abstract,
            interfaces,
            abstract_methods,
            enum_values: vec![],
            enum_arguments: vec![],
            enum_constructor_fields: vec![],
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
    /// Lê enum avançado limitado a campos finais, construtor this.campo e métodos.
    fn enumeration(
        &mut self,
        id: u32,
        name: &'a str,
        start: usize,
        annotations: Vec<Annotation>,
    ) -> Result<Class<'a>, Diagnostic> {
        let mixins = self.nominal_list("with")?;
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
        let mut enum_values = Vec::new();
        let mut enum_arguments = Vec::new();
        loop {
            let value = self.name()?;
            if enum_values.contains(&value) {
                return Err(self.error("duplicate enum value"));
            }
            enum_values.push(value);
            self.expr_nodes = 0;
            enum_arguments.push(if self.peek() == Some(TokenKind::Symbol('(')) {
                self.arguments(0)?
            } else {
                Vec::new()
            });
            if !self.take(TokenKind::Symbol(','))
                || matches!(self.peek(), Some(TokenKind::Symbol('}' | ';')))
            {
                break;
            }
        }
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut enum_constructor_fields = Vec::new();
        let mut constructor = false;
        if self.take(TokenKind::Symbol(';')) {
            while self.peek() != Some(TokenKind::Symbol('}')) {
                if self.take(TokenKind::Word("const")) {
                    if constructor {
                        return Err(self.error("only one enum constructor is supported"));
                    }
                    constructor = true;
                    self.expect(TokenKind::Word(name))?;
                    self.expect(TokenKind::Symbol('('))?;
                    while self.peek() != Some(TokenKind::Symbol(')')) {
                        self.expect(TokenKind::Word("this"))?;
                        self.expect(TokenKind::Symbol('.'))?;
                        enum_constructor_fields.push(self.name()?);
                        if !self.take(TokenKind::Symbol(',')) {
                            break;
                        }
                    }
                    self.expect(TokenKind::Symbol(')'))?;
                    self.expect(TokenKind::Symbol(';'))?;
                } else if self.take(TokenKind::Word("final")) {
                    let start = self.position();
                    let ty = self.ty(false)?;
                    let name = self.name()?;
                    self.expect(TokenKind::Symbol(';'))?;
                    let span = Span {
                        start,
                        end: self.end(),
                    };
                    fields.push(Field {
                        name,
                        ty,
                        is_final: true,
                        initializer: None,
                        span,
                    });
                } else {
                    methods.push(self.function_with_abstract(false, false)?.0);
                }
            }
        }
        self.expect(TokenKind::Symbol('}'))?;
        if !constructor
            && (!fields.is_empty() || enum_arguments.iter().any(|args| !args.is_empty()))
        {
            return Err(self.error("enum arguments and fields require a const constructor"));
        }
        Ok(Class {
            constructor: None,
            annotations,
            is_mixin_application: false,
            mixin_origin: None,
            modifier: ClassModifier::None,
            kind: ClassKind::Class,
            mixins,
            id,
            name,
            is_interface: false,
            library_id: 0,
            is_abstract: false,
            interfaces,
            abstract_methods: vec![],
            enum_values,
            enum_arguments,
            enum_constructor_fields,
            superclass: None,
            fields,
            methods,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê construtor posicional sem nome; this.campo obtém o tipo após ler a classe.
    fn constructor(&mut self, name: &'a str) -> Result<Constructor<'a>, Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word(name))?;
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let start = self.position();
            let (name, ty, field) = if self.take(TokenKind::Word("this")) {
                self.expect(TokenKind::Symbol('.'))?;
                let name = self.name()?;
                (name, Type::Inferred, Some(name))
            } else {
                let ty = self.ty(false)?;
                (self.name()?, ty, None)
            };
            parameters.push(ConstructorParameter {
                name,
                ty,
                field,
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
        let body = if self.take(TokenKind::Symbol(';')) {
            Vec::new()
        } else {
            self.block(0)?
        };
        Ok(Constructor {
            parameters,
            body,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Resolve os nomes de uma cláusula nominal na ordem declarada.
    fn nominal_list(&mut self, keyword: &'static str) -> Result<Vec<u32>, Diagnostic> {
        let mut ids = Vec::new();
        if self.take(TokenKind::Word(keyword)) {
            loop {
                let name = self.name()?;
                ids.push(
                    *self
                        .class_ids
                        .get(name)
                        .ok_or_else(|| self.error("unknown nominal type"))?,
                );
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
        }
        Ok(ids)
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
            let method = self.function_with_abstract(false, false)?.0;
            if method.is_getter {
                return Err(Diagnostic::new(
                    "extension getters are not supported yet",
                    method.span,
                ));
            }
            methods.push(method);
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
    /// Lê somente os metadados reconhecidos; anotações arbitrárias são rejeitadas.
    fn metadata(&mut self) -> Result<(Vec<Annotation>, Option<NativeBinding<'a>>), Diagnostic> {
        let mut annotations = Vec::new();
        let mut native = None;
        while self.peek() == Some(TokenKind::Symbol('@')) {
            let start = self.position();
            self.index += 1;
            let first = self.name()?;
            let (prefix, name) = if self.take(TokenKind::Symbol('.')) {
                (Some(first), self.name()?)
            } else {
                (None, first)
            };
            if name == "Native" {
                if native.is_some() {
                    return Err(self.error("duplicate Native annotation"));
                }
                self.expect(TokenKind::Operator("<"))?;
                let result = self.native_type(prefix)?;
                self.expect(TokenKind::Word("Function"))?;
                self.expect(TokenKind::Symbol('('))?;
                let mut parameters = Vec::new();
                while self.peek() != Some(TokenKind::Symbol(')')) {
                    parameters.push(self.native_type(prefix)?);
                    if !self.take(TokenKind::Symbol(',')) {
                        break;
                    }
                }
                self.expect(TokenKind::Symbol(')'))?;
                self.expect(TokenKind::Operator(">"))?;
                self.expect(TokenKind::Symbol('('))?;
                let mut symbol = None;
                let mut is_leaf = None;
                while self.peek() != Some(TokenKind::Symbol(')')) {
                    let option = self.name()?;
                    self.expect(TokenKind::Symbol(':'))?;
                    match option {
                        "symbol" if symbol.is_none() => {
                            let value = self.metadata_string()?;
                            if value.is_empty() {
                                return Err(self.error("Native symbol must not be empty"));
                            }
                            symbol = Some(value);
                        }
                        "isLeaf" if is_leaf.is_none() => {
                            is_leaf = Some(if self.take(TokenKind::Word("true")) {
                                true
                            } else if self.take(TokenKind::Word("false")) {
                                false
                            } else {
                                return Err(self.error("Native isLeaf requires a bool literal"));
                            });
                        }
                        _ => return Err(self.error("unsupported or repeated Native option")),
                    }
                    if !self.take(TokenKind::Symbol(',')) {
                        break;
                    }
                }
                self.expect(TokenKind::Symbol(')'))?;
                native = Some(NativeBinding {
                    prefix,
                    symbol: symbol.unwrap_or_default(),
                    result,
                    parameters,
                    is_leaf: is_leaf.unwrap_or(false),
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
                continue;
            }
            if prefix.is_some() {
                return Err(self.error("prefixed metadata other than Native is not supported"));
            }
            let kind = match name {
                "override" => AnnotationKind::Override,
                "deprecated" => AnnotationKind::Deprecated { message: None },
                "Deprecated" => {
                    self.expect(TokenKind::Symbol('('))?;
                    let message = self.metadata_string()?;
                    self.take(TokenKind::Symbol(','));
                    self.expect(TokenKind::Symbol(')'))?;
                    AnnotationKind::Deprecated { message: Some(message) }
                }
                _ => return Err(self.error("unsupported annotation; supported metadata: override, deprecated, Deprecated and Native")),
            };
            annotations.push(Annotation {
                kind,
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        Ok((annotations, native))
    }
    /// Lê um tipo ABI escalar com o mesmo prefixo usado pela anotação Native.
    fn native_type(&mut self, prefix: Option<&'a str>) -> Result<NativeType, Diagnostic> {
        if let Some(prefix) = prefix {
            self.expect(TokenKind::Word(prefix))?;
            self.expect(TokenKind::Symbol('.'))?;
        }
        let name = self.name()?;
        match name {
            "Void" => Ok(NativeType::Void),
            "Int32" => Ok(NativeType::Int32),
            "Int64" => Ok(NativeType::Int64),
            _ => Err(self.error("supported native types are Void, Int32 and Int64")),
        }
    }
    /// Decodifica strings de metadados mantendo a representação UTF-8 validada.
    fn metadata_string(&mut self) -> Result<String, Diagnostic> {
        let token = self
            .tokens
            .get(self.index)
            .ok_or_else(|| self.error("expected annotation string"))?;
        let value = match token.kind {
            TokenKind::String(value) => decode_string(value, token.span)?,
            TokenKind::RawString(value) => value.to_owned(),
            _ => return Err(self.error("expected annotation string literal")),
        };
        self.index += 1;
        Ok(value)
    }
    /// Lê a assinatura tipada e o corpo em bloco ou expressão de uma função/método.
    fn function(&mut self) -> Result<Function<'a>, Diagnostic> {
        self.function_with_abstract(false, true)
            .map(|(function, _)| function)
    }
    /// Distingue assinatura abstrata terminada por ponto e vírgula de corpo concreto.
    fn function_with_abstract(
        &mut self,
        allow_abstract: bool,
        allow_generic: bool,
    ) -> Result<(Function<'a>, bool), Diagnostic> {
        let start = self.position();
        let (annotations, mut native_binding) = self.metadata()?;
        let external = self.take(TokenKind::Word("external"));
        if external != native_binding.is_some() {
            return Err(self
                .error("external functions require Native and Native functions require external"));
        }
        if !allow_generic && native_binding.is_some() {
            return Err(self.error("Native is supported only on top-level functions"));
        }
        if allow_generic
            && annotations
                .iter()
                .any(|annotation| matches!(annotation.kind, AnnotationKind::Override))
        {
            return Err(self.error("override is supported only on methods"));
        }
        let previous_parameters = std::mem::take(&mut self.type_parameters);
        self.type_parameters = if allow_generic {
            self.scan_type_parameters()?
        } else {
            Vec::new()
        };
        let return_type = self.ty(true)?;
        let is_getter = self.take(TokenKind::Word("get"));
        if is_getter && allow_generic {
            return Err(self.error("top-level getters are not supported yet"));
        }
        let name = self.name()?;
        if let Some(binding) = native_binding.as_mut()
            && binding.symbol.is_empty()
        {
            binding.symbol = name.to_owned();
        }
        if self.take(TokenKind::Operator("<")) {
            if !allow_generic {
                return Err(self.error("generic methods are not supported yet"));
            }
            if self.type_parameters.is_empty() {
                return Err(self.error("generic declarations require named type parameters"));
            }
            for index in 0..self.type_parameters.len() {
                let name = self.name()?;
                if name != self.type_parameters[index] {
                    return Err(self.error("unsupported type parameter declaration"));
                }
                if index + 1 < self.type_parameters.len() {
                    self.expect(TokenKind::Symbol(','))?;
                }
            }
            self.expect(TokenKind::Operator(">"))?;
        }
        if !is_getter {
            self.expect(TokenKind::Symbol('('))?;
        }
        let mut parameters = Vec::new();
        if !is_getter && self.peek() != Some(TokenKind::Symbol(')')) {
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
        if !is_getter {
            self.expect(TokenKind::Symbol(')'))?;
        }
        let abstract_body = !external && allow_abstract && self.take(TokenKind::Symbol(';'));
        let body = if external {
            self.expect(TokenKind::Symbol(';'))?;
            Vec::new()
        } else if abstract_body {
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
        let type_parameters = std::mem::replace(&mut self.type_parameters, previous_parameters);
        Ok((
            Function {
                annotations,
                native_binding,
                type_parameters,
                is_getter,
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
    /// Pré-indexa parâmetros genéricos simples para resolver também o tipo de retorno.
    fn scan_type_parameters(&self) -> Result<Vec<&'a str>, Diagnostic> {
        for index in self.index..self.tokens.len() {
            if matches!(
                self.tokens[index].kind,
                TokenKind::Symbol('{' | ';') | TokenKind::Operator("=>")
            ) {
                break;
            }
            if self.tokens[index].kind != TokenKind::Operator("<") {
                continue;
            }
            let mut cursor = index + 1;
            let mut names = Vec::new();
            while let Some(TokenKind::Word(name)) = self.tokens.get(cursor).map(|t| t.kind) {
                if reserved(name) {
                    break;
                }
                names.push(name);
                cursor += 1;
                if self.tokens.get(cursor).map(|t| t.kind) == Some(TokenKind::Symbol(',')) {
                    cursor += 1;
                    continue;
                }
                if self.tokens.get(cursor).map(|t| t.kind) == Some(TokenKind::Operator(">"))
                    && self.tokens.get(cursor + 1).map(|t| t.kind) == Some(TokenKind::Symbol('('))
                {
                    let unique: std::collections::BTreeSet<_> = names.iter().collect();
                    if unique.len() != names.len() {
                        return Err(self.error("duplicate type parameter"));
                    }
                    return Ok(names);
                }
                break;
            }
        }
        Ok(Vec::new())
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
        if depth >= MAX_DEPTH {
            return Err(self.error("statement nesting limit exceeded"));
        }
        let start = self.position();
        let kind = if self.take(TokenKind::Word("switch")) {
            let scrutinee = self.condition()?;
            self.expect(TokenKind::Symbol('{'))?;
            let mut cases = Vec::new();
            while self.peek() != Some(TokenKind::Symbol('}')) {
                let start = self.position();
                let is_default = self.take(TokenKind::Word("default"));
                let pattern = if is_default {
                    Pattern::Wildcard
                } else {
                    self.expect(TokenKind::Word("case"))?;
                    self.pattern(depth + 1)?
                };
                let guard = if self.take(TokenKind::Word("when")) {
                    if is_default {
                        return Err(self.error("default cannot have a guard"));
                    }
                    Some(self.guard(0)?)
                } else {
                    None
                };
                self.expect(TokenKind::Symbol(':'))?;
                let mut body = Vec::new();
                while !matches!(
                    self.peek(),
                    Some(TokenKind::Word("case" | "default") | TokenKind::Symbol('}'))
                ) {
                    body.push(self.statement(depth + 1)?);
                }
                if body.is_empty() {
                    return Err(self.error("shared switch labels are not supported yet"));
                }
                cases.push(SwitchCase {
                    pattern,
                    guard,
                    body,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
            }
            self.expect(TokenKind::Symbol('}'))?;
            StatementKind::Switch { scrutinee, cases }
        } else if self.peek() == Some(TokenKind::Symbol('{')) {
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
            Some(TokenKind::Word("var" | "final" | "const"))
        ) || self.starts_annotation()
        {
            if !allow_declaration {
                return Err(self.error("declarations are not supported in for updates"));
            }
            let is_const = self.take(TokenKind::Word("const"));
            let is_final = is_const || self.take(TokenKind::Word("final"));
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
                is_const,
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
                        | ExprKind::GenericCall { .. }
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
            Some(TokenKind::Word("const")) => {
                self.index += 1;
                let start = self.position();
                let kind = self.list_literal(depth + 1)?;
                ExprKind::Const(Box::new(Expr {
                    kind,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                }))
            }
            Some(TokenKind::Word("switch")) => {
                self.index += 1;
                self.expect(TokenKind::Symbol('('))?;
                let scrutinee = self.binary(0, depth + 1)?;
                self.expect(TokenKind::Symbol(')'))?;
                self.expect(TokenKind::Symbol('{'))?;
                let mut arms = Vec::new();
                while self.peek() != Some(TokenKind::Symbol('}')) {
                    let start = self.position();
                    let pattern = self.pattern(depth + 1)?;
                    let guard = if self.take(TokenKind::Word("when")) {
                        Some(self.guard(depth + 1)?)
                    } else {
                        None
                    };
                    self.expect(TokenKind::Operator("=>"))?;
                    let value = self.binary(0, depth + 1)?;
                    arms.push(SwitchArm {
                        pattern,
                        guard,
                        value,
                        span: Span {
                            start,
                            end: self.end(),
                        },
                    });
                    if !self.take(TokenKind::Symbol(',')) {
                        break;
                    }
                }
                self.expect(TokenKind::Symbol('}'))?;
                ExprKind::Switch {
                    scrutinee: Box::new(scrutinee),
                    arms,
                }
            }
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
                if self.generic_call_ahead() {
                    let type_arguments = self.type_arguments()?;
                    let arguments = self.arguments(depth)?;
                    ExprKind::GenericCall {
                        name,
                        type_arguments,
                        arguments,
                    }
                } else if self.class_ids.contains_key(name) && self.take(TokenKind::Symbol('.')) {
                    ExprKind::EnumValue {
                        class_id: self.class_ids[name],
                        name: self.name()?,
                    }
                } else if self.peek() == Some(TokenKind::Symbol('(')) {
                    let arguments = self.arguments(depth)?;
                    if let Some(&class_id) = self.class_ids.get(name) {
                        ExprKind::Construct {
                            class_id,
                            arguments,
                        }
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
                self.list_literal(depth)?
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
    /// Mantém o separador de braço fora de closures parentetizadas no nível da guarda.
    fn guard(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let previous = self.guard_start.replace(self.index);
        let result = self.binary(0, depth);
        self.guard_start = previous;
        result
    }
    /// Lê uma lista tipada ou inferida sem absorver acessos pós-fixos no contexto const.
    fn list_literal(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
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
        Ok(ExprKind::List {
            element_type,
            elements,
        })
    }
    /// Analisa wildcard, binding tipado ou expressão constante do padrão simples.
    fn pattern(&mut self, depth: usize) -> Result<Pattern<'a>, Diagnostic> {
        if self.take(TokenKind::Word("_")) {
            return Ok(Pattern::Wildcard);
        }
        if self.starts_annotation() {
            let ty = self.ty(false)?;
            let name = self.name()?;
            return Ok(Pattern::Binding { ty, name });
        }
        let saved_index = self.index;
        let saved_types = self.types.len();
        if let Ok(ty) = self.ty(false)
            && self.take(TokenKind::Symbol('('))
        {
            self.expect(TokenKind::Symbol(')'))
                .map_err(|_| self.error("only empty object patterns are supported"))?;
            return Ok(Pattern::Type(ty));
        }
        self.index = saved_index;
        self.types.truncate(saved_types);
        let value = self.primary(depth + 1)?;
        let supported = match &value.kind {
            ExprKind::Int(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::OwnedString(_)
            | ExprKind::Identifier(_)
            | ExprKind::EnumValue { .. } => true,
            ExprKind::Unary {
                op: UnaryOp::Negate,
                operand,
            } => matches!(operand.kind, ExprKind::Int(_)),
            _ => false,
        };
        if !supported {
            return Err(self.error(
                "only literal, named constant, enum and typed binding patterns are supported",
            ));
        }
        Ok(Pattern::Constant(value))
    }
    /// Consome argumentos de tipo explícitos de uma chamada genérica top-level.
    fn type_arguments(&mut self) -> Result<Vec<Type>, Diagnostic> {
        self.expect(TokenKind::Operator("<"))?;
        let mut types = Vec::new();
        loop {
            types.push(self.ty(false)?);
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Operator(">"))?;
        Ok(types)
    }
    /// Distingue argumentos genéricos de operadores relacionais e restaura a arena.
    fn generic_call_ahead(&mut self) -> bool {
        if self.peek() != Some(TokenKind::Operator("<")) {
            return false;
        }
        let index = self.index;
        let count = self.types.len();
        let result = self.type_arguments().is_ok() && self.peek() == Some(TokenKind::Symbol('('));
        self.index = index;
        self.types.truncate(count);
        result
    }
    /// Reconhece parâmetros de closure sem confundir agrupamento de expressões.
    fn starts_closure(&self) -> bool {
        if let Some(start) = self.guard_start {
            let mut nesting = 0usize;
            for token in &self.tokens[start..self.index] {
                match token.kind {
                    TokenKind::Symbol('(' | '[' | '{') => nesting += 1,
                    TokenKind::Symbol(')' | ']' | '}') => nesting = nesting.saturating_sub(1),
                    _ => {}
                }
            }
            if nesting == 0 {
                return false;
            }
        }
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

/// Avança metadados balanceados durante a indexação; o parser completo valida seu conteúdo.
fn skip_metadata(tokens: &[Token<'_>], mut index: usize) -> Result<usize, Diagnostic> {
    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Symbol('@')) {
        let span = tokens[index].span;
        index += 1;
        if !matches!(
            tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Word(_))
        ) {
            return Err(Diagnostic::new("expected annotation name", span));
        }
        index += 1;
        if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Symbol('.')) {
            index += 1;
            if !matches!(
                tokens.get(index).map(|token| token.kind),
                Some(TokenKind::Word(_))
            ) {
                return Err(Diagnostic::new("expected qualified annotation name", span));
            }
            index += 1;
        }
        if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Operator("<")) {
            let mut depth = 1usize;
            index += 1;
            while depth > 0 {
                let token = tokens.get(index).ok_or_else(|| {
                    Diagnostic::new("unterminated annotation type arguments", span)
                })?;
                match token.kind {
                    TokenKind::Operator("<") => depth += 1,
                    TokenKind::Operator(">") => depth -= 1,
                    _ => {}
                }
                index += 1;
            }
        }
        if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Symbol('(')) {
            index = skip_delimited(tokens, index, '(', ')', span)?;
        }
    }
    Ok(index)
}
/// Cabeçalho compartilhado pelo índice de bibliotecas e pela análise completa.
struct NominalHeader {
    name_index: usize,
    is_abstract: bool,
    is_interface: bool,
    is_enum: bool,
    kind: ClassKind,
    modifier: ClassModifier,
}
/// Valida a ordem e as combinações dos modificadores nominais de Dart 3.6.2.
fn nominal_header(tokens: &[Token<'_>], start: usize) -> Result<NominalHeader, Diagnostic> {
    let span = tokens[start].span;
    let mut index = start;
    let abstract_written = tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("abstract"));
    index += usize::from(abstract_written);
    let mut is_interface = false;
    let modifier = match tokens.get(index).map(|t| t.kind) {
        Some(TokenKind::Word("base")) => {
            index += 1;
            ClassModifier::Base
        }
        Some(TokenKind::Word("final")) => {
            index += 1;
            ClassModifier::Final
        }
        Some(TokenKind::Word("sealed")) => {
            index += 1;
            ClassModifier::Sealed
        }
        Some(TokenKind::Word("interface")) => {
            index += 1;
            is_interface = true;
            ClassModifier::None
        }
        _ => ClassModifier::None,
    };
    let (kind, is_enum) = match tokens.get(index).map(|t| t.kind) {
        Some(TokenKind::Word("class")) => {
            index += 1;
            (ClassKind::Class, false)
        }
        Some(TokenKind::Word("enum"))
            if !abstract_written && !is_interface && modifier == ClassModifier::None =>
        {
            index += 1;
            (ClassKind::Class, true)
        }
        Some(TokenKind::Word("mixin")) => {
            index += 1;
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("class")) {
                index += 1;
                (ClassKind::MixinClass, false)
            } else {
                (ClassKind::Mixin, false)
            }
        }
        _ => {
            return Err(Diagnostic::new(
                "invalid nominal modifier order; expected class or mixin",
                span,
            ));
        }
    };
    if (modifier == ClassModifier::Sealed && abstract_written)
        || (kind != ClassKind::Class
            && (is_interface || matches!(modifier, ClassModifier::Final | ClassModifier::Sealed)))
        || (kind == ClassKind::Mixin && abstract_written)
    {
        return Err(Diagnostic::new(
            "invalid combination of nominal modifiers",
            span,
        ));
    }
    Ok(NominalHeader {
        name_index: index,
        is_abstract: abstract_written
            || modifier == ClassModifier::Sealed
            || kind == ClassKind::Mixin,
        is_interface,
        is_enum,
        kind,
        modifier,
    })
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
            TokenKind::Word("mixin")
                if depth == 0
                    && tokens.get(index + 1).map(|t| t.kind) == Some(TokenKind::Word("class")) => {}
            TokenKind::Word("class" | "enum" | "mixin") if depth == 0 => {
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
    /// Inicializadores this.campo recebem tipos próprios mesmo antes da declaração do campo.
    #[test]
    fn positional_constructors_and_initializing_formals() {
        let source = "class Usuario{Usuario(this.username,this.email){print(username);} final String username;final String email;} class Counter{int value=0;Counter(int value){this.value=value;}} void main(){var user=Usuario('ana','a@b');Counter(2);}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().classes.len(), 2);
        let program = parse(&tokens, source.len()).unwrap();
        let constructor = program.classes[0].constructor.as_ref().unwrap();
        assert_eq!(constructor.parameters.len(), 2);
        assert!(constructor.parameters.iter().all(
            |parameter| parameter.ty == Type::String && parameter.field == Some(parameter.name)
        ));
        assert!(
            program.classes[0]
                .fields
                .iter()
                .all(|field| field.initializer.is_none())
        );
        assert!(program.classes[1].fields[0].initializer.is_some());
        assert!(
            program.classes[1].constructor.as_ref().unwrap().parameters[0]
                .field
                .is_none()
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[0].kind else {
            panic!()
        };
        assert!(
            matches!(&initializer.kind, ExprKind::Construct { arguments, .. } if arguments.len() == 2)
        );
        let source = "class A{A(this.value,);int value;} class B{B();} void main(){}";
        let program = parsed(source);
        assert!(
            program.classes[0]
                .constructor
                .as_ref()
                .unwrap()
                .body
                .is_empty()
        );
        assert!(
            program.classes[1]
                .constructor
                .as_ref()
                .unwrap()
                .parameters
                .is_empty()
        );
    }
    /// Construtores nomeados, listas de inicialização e parâmetros opcionais são rejeitados.
    #[test]
    fn rejects_constructor_forms_outside_subset() {
        for declaration in [
            "class C{C();C(){}}",
            "class C{C.named();}",
            "class C{const C();}",
            "class C{factory C();}",
            "class C{C():super();}",
            "class C{C():x=1;int x;}",
            "class C{C([int x=1]);}",
            "class C{C({int x=1});}",
            "class C{C(super.x);}",
            "class C{C(int this.x);int x;}",
            "mixin class C{C();}",
            "class C{@deprecated C();}",
        ] {
            let source = format!("{declaration} void main(){{}}");
            assert!(
                parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
    }
    /// Mantém assinatura ABI, símbolo original e metadados sem depender do linker.
    #[test]
    fn native_annotations_and_metadata_are_preserved() {
        let source = "@Deprecated('use novo') @ffi.Native<ffi.Int32 Function(ffi.Int32,ffi.Int32)>(symbol:'somar_valores',isLeaf:true) external int somarValores(int a,int b); @Native<Void Function()>() external void limpar(); @deprecated class C{@override int value()=>1;} void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let index = index_unit(&tokens).unwrap();
        assert_eq!(
            index
                .functions
                .iter()
                .map(|item| item.name)
                .collect::<Vec<_>>(),
            ["somarValores", "limpar", "main"]
        );
        assert_eq!(index.classes[0].name, "C");
        let program = parse(&tokens, source.len()).unwrap();
        let binding = program.functions[0].native_binding.as_ref().unwrap();
        assert_eq!(binding.prefix, Some("ffi"));
        assert_eq!(binding.result, NativeType::Int32);
        assert_eq!(binding.parameters, [NativeType::Int32, NativeType::Int32]);
        assert_eq!(binding.symbol, "somar_valores");
        assert!(binding.is_leaf);
        assert!(program.functions[0].body.is_empty());
        assert!(
            matches!(&program.functions[0].annotations[0].kind, AnnotationKind::Deprecated { message: Some(message) } if message == "use novo")
        );
        let default = program.functions[1].native_binding.as_ref().unwrap();
        assert_eq!(default.symbol, "limpar");
        assert!(!default.is_leaf);
        assert!(matches!(
            program.classes[0].annotations[0].kind,
            AnnotationKind::Deprecated { message: None }
        ));
        assert!(matches!(
            program.classes[0].methods[0].annotations[0].kind,
            AnnotationKind::Override
        ));
        assert!(source[binding.span.start..binding.span.end].starts_with("@ffi.Native"));
    }
    /// Anotações não suportadas e vínculos externos incompletos não são ignorados.
    #[test]
    fn rejects_unsupported_annotations_and_native_declarations() {
        for declaration in [
            "class Deprecated{} @Deprecated('obsolete') void f(){}",
            "int deprecated()=>1; @deprecated class C{}",
            "class C{int override=1; @override int f()=>1;}",
            "class A{int override=1;} class C extends A{@override int f()=>1;}",
            "mixin M{int Deprecated=1;} class C with M{@Deprecated('old') int f()=>1;}",
            "@Custom() int f()=>1;",
            "@override int f()=>1;",
            "@override class C{}",
            "external int f();",
            "@Native<Int64 Function()>() int f()=>1;",
            "@Native<Int64 Function()>() external int f()=>1;",
            "@Native<Int64 Function()>(assetId:'x') external int f();",
            "@Native<Int64 Function()>(isLeaf:1) external int f();",
            "@Native<Int64 Function()>(symbol:'') external int f();",
            "@Native<Int64 Function()>(symbol:'x',symbol:'y') external int f();",
            "@Native<Double Function()>() external int f();",
            "@ffi.Native<Int64 Function()>() external int f();",
            "@Native<Void Function()>() class C{}",
            "class C{@Native<Int64 Function()>() external int f();}",
            "class C{@deprecated int field=1;}",
            "int f(@deprecated int x)=>x;",
            "@Native<Void Function()>() external void main();",
        ] {
            let source = format!("{declaration} void main(){{}}");
            assert!(
                parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
        for source in [
            "@",
            "@ffi.",
            "@Native<Int32 Function()",
            "@Deprecated('msg')",
        ] {
            let tokens = dartforge_lexer::lex(source).unwrap();
            let error = index_unit(&tokens).unwrap_err();
            assert!(error.span.start <= error.span.end && error.span.end <= source.len());
        }
    }
    /// Preserva modificadores, aplicações ordenadas e padrões vazios de objeto.
    #[test]
    fn modifiers_mixins_and_empty_object_patterns() {
        let source = "base class A{} abstract base class B{} final class C{} abstract final class D{} sealed class S{} mixin M{int f()=>1;} base mixin N{} mixin class P{} abstract base mixin class Q{} class R extends A with M,N implements B{} int f(S s)=>switch(s){C()=>1,_=>0}; void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().classes.len(), 10);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(program.classes[0].modifier, ClassModifier::Base);
        assert_eq!(program.classes[2].modifier, ClassModifier::Final);
        assert_eq!(program.classes[4].modifier, ClassModifier::Sealed);
        assert!(program.classes[4].is_abstract);
        assert_eq!(program.classes[5].kind, ClassKind::Mixin);
        assert!(program.classes[5].is_abstract);
        assert_eq!(program.classes[7].kind, ClassKind::MixinClass);
        assert!(!program.classes[7].is_abstract);
        assert!(program.classes[8].is_abstract);
        assert_eq!(
            program.classes[9].mixins,
            [program.classes[5].id, program.classes[6].id]
        );
        let StatementKind::Return(Some(value)) = &program.functions[0].body[0].kind else {
            panic!()
        };
        let ExprKind::Switch { arms, .. } = &value.kind else {
            panic!()
        };
        assert!(matches!(arms[0].pattern, Pattern::Type(Type::Class(2))));
        assert!(
            program
                .classes
                .iter()
                .all(|class| !class.is_mixin_application && class.mixin_origin.is_none())
        );
    }
    /// Rejeita combinações ilegais, restrições on e extração de campos em padrões.
    #[test]
    fn rejects_invalid_modifiers_and_nonempty_object_patterns() {
        for declaration in [
            "abstract sealed class C{}",
            "final mixin M{}",
            "interface mixin class C{}",
            "sealed mixin class C{}",
            "abstract mixin M{}",
            "base final class C{}",
            "base abstract class C{}",
            "mixin class abstract C{}",
            "mixin M on C{} class C{}",
            "class C{} mixin M extends C{}",
            "mixin M{} mixin N with M{}",
            "mixin M{} class C with M,{}",
            "class C with Missing{}",
        ] {
            let source = format!("{declaration} void main(){{}}");
            let tokens = dartforge_lexer::lex(&source).unwrap();
            assert!(parse(&tokens, source.len()).is_err(), "{source}");
        }
        let source = "class C{int x=1;} void main(){print(switch(C()){C(x:1)=>1,_=>0});}";
        assert!(parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err());
        let source = "mixin M on C{} class C{}";
        assert!(
            index_unit(&dartforge_lexer::lex(source).unwrap())
                .unwrap_err()
                .message
                .contains("on constraints")
        );
    }
    /// Combinadores de importação são identificadores fora das diretivas.
    #[test]
    fn show_and_hide_are_contextual_identifiers() {
        let source = "int show(int hide)=>hide; void main(){var hide=1;print(show(hide));}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(program.functions[0].name, "show");
        assert_eq!(program.functions[0].parameters[0].name, "hide");
    }
    /// Enums resolvem interfaces múltiplas declaradas antes ou depois deles.
    #[test]
    fn enhanced_enum_implements_interfaces() {
        let source = "abstract class I{int value();} enum E implements I,J{a(1);final int n;const E(this.n);int value()=>this.n;int get doubled=>this.n*2;} abstract class J{int get doubled;} void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().classes.len(), 3);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(
            program.classes[1].interfaces,
            [program.classes[0].id, program.classes[2].id]
        );
        assert_eq!(program.classes[1].enum_constructor_fields, ["n"]);
        for declaration in [
            "enum E implements Missing{a}",
            "class I{} enum E implements I,{a}",
            "class I{} enum E extends I{a}",
        ] {
            let source = format!("{declaration} void main(){{}}");
            assert!(parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err());
        }
    }
    /// Contratos genéricos, enum avançado, getters e switches preservam metadados.
    #[test]
    fn generic_enum_getter_const_and_switch_shapes() {
        let source = "T identity<T>(T value)=>value; enum E {a(1),b(2); final int code; const E(this.code); int get doubled=>code*2; int value()=>this.code;} void main(){const values=<int>[1,2];var x=identity<int>(1);print(switch(E.a){E.a=>1,E.b=>2});switch(x){case int n when (n>0): print(n);break;default:print(0);}}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 2);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(program.functions[0].type_parameters, ["T"]);
        assert_eq!(program.functions[0].return_type, Type::Parameter(0));
        assert_eq!(program.classes[0].enum_constructor_fields, ["code"]);
        assert_eq!(program.classes[0].enum_arguments.len(), 2);
        assert!(program.classes[0].methods[0].is_getter);
        assert!(matches!(
            program.statements[0].kind,
            StatementKind::Variable { is_const: true, .. }
        ));
        assert!(matches!(
            program.statements[3].kind,
            StatementKind::Switch { .. }
        ));
        parsed(
            "void main(){var n=1;print(switch(n){int value when (true) && (value>0)=>value,_=>0});print(const <int>[1][0]);}",
        );
    }
    /// Limita as novas formas aos contratos anunciados, sem construtores ou padrões gerais.
    #[test]
    fn increment_thirteen_rejects_unsupported_forms() {
        for source in [
            "T f<T extends int>(T x)=>x;",
            "T f<T,T>(T x)=>x;",
            "enum E{a(1);final int n; E(this.n);}",
            "enum E{a(1);final int n=1;const E(this.n);}",
            "const x=1;",
            "class C{int get x()=>1;}",
        ] {
            let source = format!("{source} void main(){{}}");
            assert!(
                parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
        rejected("var x=switch(1){[var a]=>a,_=>0};");
        rejected("switch(1){case 1:case 2:print(2);}");
        rejected("print(switch(3){1+2=>1,_=>0});");
        rejected("print(switch(3){(1+2)=>1,_=>0});");
        rejected("switch(1){default when true:print(1);}");
    }
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
    /// Combinações inválidas e formas ainda não implementadas produzem diagnósticos.
    #[test]
    fn rejects_unsupported_nominal_forms() {
        for source in [
            "interface abstract class I {}",
            "base abstract class I {}",
            "enum E {}",
            "enum E { a, a }",
            "enum E { a; static int f()=>1; }",
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
            ExprKind::Construct { class_id: 0, .. }
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
    /// Rejeita formas ainda fora do subconjunto nominal.
    #[test]
    fn invalid_class_forms_and_member_limits() {
        for source in [
            "class C { C.named() {} } void main(){}",
            "class C { late int x; } void main(){}",
            "class C { static int x=1; } void main(){}",
            "class C { void x=1; } void main(){}",
            "class C extends Missing {} void main(){}",
            "class C {} class C {} void main(){}",
            "class int {} void main(){}",
            "class C<T> {} void main(){}",
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
