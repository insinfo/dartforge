//! Análise sintática do subconjunto Dart 3.6.2, com limites explícitos de complexidade.
//! Mixins aceitam implements e aplicações with, mas restrições on e padrões de
//! objeto com extração de campos permanecem explicitamente fora do subconjunto.
//! O parser limita estruturas a 64 níveis e primárias simultâneas a 16, contando
//! reentradas por corpos de closures; expressões aninhadas compartilham 128 nós.
//! Async admite Future, await, value/delayed e Duration; geradores, const Duration
//! e construtores Future adicionais permanecem fora desta etapa.
//! Metadados limitam-se a override/deprecated/Deprecated e Native escalar em
//! funções external de topo; campos, parâmetros e anotações customizadas são rejeitados.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_syntax::{
    Annotation, AnnotationKind, BinaryOp, CatchClause, Class, ClassKind, ClassModifier,
    Constructor, ConstructorAssert, ConstructorExtras, ConstructorParameter, DurationUnit, Expr,
    ExprKind, Extension,
    Field, FieldInitializer, Function, GenericParameter, NamedConstructor, NativeBinding,
    NativeType, Parameter, ParameterKind, Pattern, Program, RedirectCall, Statement, StatementKind,
    StaticField, StringPart, SuperCall, SwitchArm, SwitchCase, Token, TokenKind, Type, TypeShape,
    UnaryOp,
};

/// Recusa de `const` sem anotação cujo tipo o inicializador não decide sozinho.
///
/// A mensagem diz a razão, o que a dedução cobre e a saída, no mesmo formato da
/// recusa de `dynamic`: uma recusa que não ensina o que escrever no lugar
/// obriga quem lê a adivinhar o recorte aceito.
const UNTYPED_CONST: &str = "a const declaration without a type annotation takes its type from the initializer, and this initializer does not decide it syntactically: only literals, constant operators over them, collection literals with a written or uniform element type, constructor invocations and references to const declarations written earlier are inferred here; write the type before the name";

/// Limita o aninhamento recursivo, inclusive cadeias associativas à esquerda.
///
/// A contagem cobre a profundidade da **árvore**, não a da leitura: `a+b+c+d`
/// é lido num laço, sem recursão, mas produz uma espinha esquerda tão funda
/// quanto a cadeia é longa, e é essa espinha que a análise semântica, os
/// otimizadores, a emissão e até o `Drop` da árvore percorrem recursivamente.
/// Por isso `binary` e `postfix` aprofundam o nível a cada iteração.
const MAX_DEPTH: usize = 64;
/// Teto absoluto de nós de uma expressão isolada.
///
/// Este orçamento **não** é a proteção de pilha — quem protege a pilha é
/// [`MAX_DEPTH`], que desde esta versão conta também as cadeias associativas à
/// esquerda. O que resta aqui é um teto de tamanho, e o valor antigo de 128
/// confundia largura com profundidade: `type1_fonts.dart` do pacote `pdf`
/// declara `const List<double>` com 255 elementos, uma árvore larga e de
/// profundidade 1, sem risco nenhum de estouro, e ainda assim recusada. Um
/// literal de tabela é código real e precisa caber; o teto novo é dimensionado
/// para isso e continua finito para entrada arbitrária.
const MAX_EXPR_NODES: usize = 65_536;
/// Limita frames simultâneos de expressão, inclusive reentrada por corpos de closures.
/// O teto estrutural de 64 continua separado; este protege pilhas pequenas em debug.
const MAX_ACTIVE_PRIMARIES: usize = 16;

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
    let class_ids = index_classes(tokens)?;
    // A unidade isolada reserva o ID seguinte para a declaração sintética que
    // agrupa as variáveis de topo; nenhum nome de usuário pode alcançá-lo.
    let globals_id = u32::try_from(class_ids.len()).ok();
    let mut program = parse_unit_with_globals(tokens, source_len, class_ids, globals_id)?;
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
            let future_void = matches!(function.return_type, Type::Applied(id) if program.types.get(id as usize) == Some(&TypeShape::Future(Type::Void)));
            if (function.return_type != Type::Void && !(function.is_async && future_void))
                || !function.parameters.is_empty()
                || !function.type_parameters.is_empty()
            {
                return Err(Diagnostic::new(
                    "main requires void main() or async Future<void> main()",
                    function.span,
                ));
            }
            program.main_is_arrow = function.is_arrow;
            program.main_is_async = function.is_async;
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
    parse_unit_with_globals(tokens, source_len, class_ids, None)
}

/// Analisa uma unidade aceitando variáveis de topo sob um ID nominal reservado.
///
/// `globals_id` precisa ser um identificador de classe livre: as variáveis de
/// topo viram membros estáticos de uma declaração sintética, porque `Program`
/// não pode ganhar um campo sem quebrar o linker, que o constrói por literal.
/// Com `None`, qualquer variável de topo é recusada com diagnóstico explícito.
///
/// # Exemplos
///
///     let fonte = "int contador = 1;";
///     let tokens = dartforge_lexer::lex(fonte).unwrap();
///     let unidade =
///         dartforge_parser::parse_unit_with_globals(&tokens, fonte.len(), Default::default(), Some(0))
///             .unwrap();
///     assert_eq!(unidade.classes[0].static_fields[0].name, "contador");
///
/// # Erros
///
/// Retorna diagnóstico para sintaxe inválida, limites excedidos, classe própria
/// ausente no ambiente nominal ou variável de topo sem ID reservado.
pub fn parse_unit_with_globals<'a>(
    tokens: &[Token<'a>],
    source_len: usize,
    class_ids: std::collections::BTreeMap<&'a str, u32>,
    globals_id: Option<u32>,
) -> Result<Program<'a>, Diagnostic> {
    let mut cursor = Cursor {
        tokens,
        index: 0,
        source_len,
        expr_nodes: 0,
        class_ids,
        types: Vec::new(),
        closure_depth: 0,
        active_primaries: 0,
        expression_frames: 0,
        type_parameters: Vec::new(),
        typedefs: std::collections::BTreeMap::new(),
        class_type_parameters: Vec::new(),
        class_type_bounds: Vec::new(),
        guard_start: None,
        in_type_test: false,
        inferred_constants: std::collections::BTreeMap::new(),
    };
    let mut classes = Vec::new();
    let mut extensions = Vec::new();
    let mut functions = Vec::new();
    let mut globals: Vec<StaticField<'a>> = Vec::new();
    while cursor.peek().is_some() {
        let declaration_index = skip_metadata(tokens, cursor.index)?;
        let declaration_kind = tokens.get(declaration_index).map(|token| token.kind);
        if declaration_kind == Some(TokenKind::Word("macro")) {
            return Err(
                cursor.error("macro declarations and generated annotations are not supported")
            );
        }
        if starts_nominal(tokens, declaration_index) {
            classes.push(cursor.class()?);
        } else if declaration_kind == Some(TokenKind::Word("typedef")) {
            cursor.typedef_decl()?;
        } else if starts_global_variable(tokens, declaration_index) {
            let global = cursor.global_variable()?;
            if globals_id.is_none() {
                return Err(Diagnostic::new(
                    "top-level variables require a single compilation unit in this subset",
                    global.span,
                ));
            }
            globals.push(global);
        } else if declaration_kind == Some(TokenKind::Word("extension")) {
            if cursor.peek() == Some(TokenKind::Symbol('@')) {
                return Err(cursor.error("annotations on extensions are not supported yet"));
            }
            if tokens.get(declaration_index + 1).map(|token| token.kind)
                == Some(TokenKind::Word("type"))
            {
                cursor.extension_type(&mut extensions, &mut functions)?;
                continue;
            }
            let id =
                u32::try_from(extensions.len()).map_err(|_| cursor.error("too many extensions"))?;
            extensions.push(cursor.extension(id)?);
        } else {
            functions.push(cursor.function()?);
        }
    }
    if !globals.is_empty() {
        let span = globals[0].span;
        classes.push(globals_class(
            globals_id.expect("ID reservado exigido antes de coletar variáveis"),
            globals,
            span,
        ));
    }
    let program = Program {
        main_is_arrow: false,
        main_is_async: false,
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
            AnnotationKind::JsonCodable => "JsonCodable",
            AnnotationKind::DataClass => "DataClass",
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

        if first.kind == TokenKind::Word("typedef")
            || starts_global_variable(tokens, index)
            || (matches!(
                first.kind,
                TokenKind::Word("const" | "final" | "var" | "late")
            ) && !starts_nominal(tokens, index))
        {
            index = skip_until_semicolon(tokens, index, first.span)?;
            continue;
        }

        let is_class = starts_nominal(tokens, index);
        if is_class {
            index = nominal_header(tokens, index)?.name_index;
        } else {
            index += 1;
        }

        if !matches!(first.kind, TokenKind::Word(_) | TokenKind::Symbol('(')) {
            return Err(Diagnostic::new(
                "expected top-level declaration",
                first.span,
            ));
        }
        if !is_class {
            if first.kind == TokenKind::Symbol('(') {
                index = skip_delimited(tokens, index - 1, '(', ')', first.span)?;
            }
            index = skip_type_arguments(tokens, index, first.span)?;
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("?")) {
                index += 1;
            }
            while tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("Function")) {
                index = skip_delimited(tokens, index + 1, '(', ')', first.span)?;
            }
            if matches!(
                tokens.get(index).map(|t| t.kind),
                Some(TokenKind::Word("get" | "set"))
            ) {
                index += 1;
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
            index = skip_type_arguments(tokens, index, first.span)?;
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("extends")) {
                index += 1;
                if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                    return Err(Diagnostic::new("expected superclass name", first.span));
                }
                index += 1;
                if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Symbol('.')) {
                    index += 1;
                    if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                        return Err(Diagnostic::new(
                            "expected qualified superclass name",
                            first.span,
                        ));
                    }
                    index += 1;
                }
                index = skip_type_arguments(tokens, index, first.span)?;
            }
            // `mixin M on Base`: o índice só precisa atravessar a cláusula;
            // quem resolve a restrição é a leitura formal da declaração.
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word("on")) {
                index += 1;
                loop {
                    if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                        return Err(Diagnostic::new("expected mixin constraint name", first.span));
                    }
                    index += 1;
                    if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Symbol('.')) {
                        index += 1;
                        if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                            return Err(Diagnostic::new(
                                "expected qualified mixin constraint name",
                                first.span,
                            ));
                        }
                        index += 1;
                    }
                    index = skip_type_arguments(tokens, index, first.span)?;
                    if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol(',')) {
                        break;
                    }
                    index += 1;
                }
            }
            for clause in ["with", "implements"] {
                if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Word(clause)) {
                    index += 1;
                    loop {
                        if !matches!(tokens.get(index).map(|t| t.kind), Some(TokenKind::Word(_))) {
                            return Err(Diagnostic::new("expected interface name", first.span));
                        }
                        index += 1;
                        if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Symbol('.')) {
                            index += 1;
                            if !matches!(
                                tokens.get(index).map(|t| t.kind),
                                Some(TokenKind::Word(_))
                            ) {
                                return Err(Diagnostic::new(
                                    "expected qualified interface name",
                                    first.span,
                                ));
                            }
                            index += 1;
                        }
                        index = skip_type_arguments(tokens, index, first.span)?;
                        if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Symbol(',')) {
                            break;
                        }
                        index += 1;
                    }
                }
            }
            if tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("=")) {
                index = skip_until_semicolon(tokens, index, first.span)?;
                continue;
            }
        } else {
            declarations.functions.push(item);
            index = skip_type_arguments(tokens, index, first.span)?;
            index = skip_delimited(tokens, index, '(', ')', first.span)?;
            if matches!(
                tokens.get(index).map(|token| token.kind),
                Some(TokenKind::Word("async" | "sync"))
            ) {
                index += 1;
                if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Operator("*")) {
                    index += 1;
                }
            }
            if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Symbol(';')) {
                index += 1;
                continue;
            }
        }
        if !is_class && tokens.get(index).map(|t| t.kind) == Some(TokenKind::Operator("=>")) {
            index = skip_until_semicolon(tokens, index, first.span)?;
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

/// Avança sobre delimitadores balanceados até o ponto e vírgula delimitador.
fn skip_until_semicolon(
    tokens: &[Token<'_>],
    mut index: usize,
    fallback: Span,
) -> Result<usize, Diagnostic> {
    let mut delimiters = Vec::new();
    while let Some(token) = tokens.get(index) {
        match token.kind {
            TokenKind::Symbol(';') if delimiters.is_empty() => return Ok(index + 1),
            TokenKind::Symbol(open @ ('(' | '[' | '{')) => delimiters.push(open),
            TokenKind::Symbol(close @ (')' | ']' | '}')) => {
                let expected = match close {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if delimiters.pop() != Some(expected) {
                    return Err(Diagnostic::new("unbalanced delimiters", token.span));
                }
            }
            _ => {}
        }
        index += 1;
    }
    Err(Diagnostic::new("expected semicolon", fallback))
}

/// Avança sobre argumentos ou parâmetros de tipo balanceados `<...>`.
fn skip_type_arguments(
    tokens: &[Token<'_>],
    mut index: usize,
    fallback: Span,
) -> Result<usize, Diagnostic> {
    if tokens.get(index).map(|t| t.kind) != Some(TokenKind::Operator("<")) {
        return Ok(index);
    }
    let mut depth = 1usize;
    index += 1;
    while depth > 0 {
        let token = tokens
            .get(index)
            .ok_or_else(|| Diagnostic::new("unterminated type arguments", fallback))?;
        match token.kind {
            TokenKind::Operator("<") => depth += 1,
            TokenKind::Operator(">") => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    Ok(index)
}
struct Cursor<'t, 'a> {
    tokens: &'t [Token<'a>],
    index: usize,
    source_len: usize,
    expr_nodes: usize,
    class_ids: std::collections::BTreeMap<&'a str, u32>,
    types: Vec<TypeShape>,
    closure_depth: usize,
    active_primaries: usize,
    expression_frames: usize,
    type_parameters: Vec<&'a str>,
    /// Apelidos `typedef` (nome direto para o tipo, já com erasure aplicado).
    typedefs: std::collections::BTreeMap<&'a str, Type>,
    /// Parâmetros da classe em análise; usos de `T` viram o bound (erasure).
    class_type_parameters: Vec<&'a str>,
    /// Bounds na mesma ordem de `class_type_parameters`.
    class_type_bounds: Vec<Type>,
    guard_start: Option<usize>,
    /// Marca a leitura do tipo de `is`/`as`, onde `?` pode abrir um condicional.
    in_type_test: bool,
    /// Tipos já inferidos de `const` sem anotação, pelo nome escrito.
    ///
    /// Um `const` sem anotação toma o tipo do próprio inicializador, e um
    /// inicializador pode citar outro `const` declarado antes dele — a mesma
    /// ordem que a avaliação constante já exige. Vazio em todo arquivo que
    /// escreve a anotação, que é o caminho comum e não aloca nada aqui.
    inferred_constants: std::collections::BTreeMap<&'a str, Type>,
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
    /// Lê o tipo de `is`/`as`, onde um `?` final pode abrir um condicional.
    ///
    /// Dart lê `x is int ? a : b` como `(x is int) ? a : b`, e `x is int? && y`
    /// como um teste de `int?`. A decisão é do token seguinte ao `?`.
    fn test_type(&mut self) -> Result<Type, Diagnostic> {
        self.in_type_test = true;
        let result = self.type_at(false, 0);
        self.in_type_test = false;
        result
    }
    /// Consome o `?` de nulabilidade e informa se ele pertencia ao tipo.
    ///
    /// Dentro de `is`/`as` no nível externo, um `?` seguido de algo que possa
    /// iniciar uma expressão pertence ao operador condicional, não ao tipo.
    fn nullable_marker(&mut self, depth: usize) -> bool {
        if self.peek() != Some(TokenKind::Operator("?")) {
            return false;
        }
        if self.in_type_test
            && depth == 0
            && matches!(
                self.tokens.get(self.index + 1).map(|token| token.kind),
                Some(
                    TokenKind::Word(_)
                        | TokenKind::Number(_)
                        | TokenKind::String(_)
                        | TokenKind::RawString(_)
                        | TokenKind::MultilineString { .. }
                        | TokenKind::StringStart { .. }
                        | TokenKind::InterpolatedName(_)
                        | TokenKind::Symbol('(' | '[' | '{')
                        | TokenKind::Operator("-" | "!")
                )
            )
        {
            return false;
        }
        self.index += 1;
        true
    }
    /// Analisa List/Iterable e tipos de função sem generics definidos pelo usuário.
    fn type_at(&mut self, allow_void: bool, depth: usize) -> Result<Type, Diagnostic> {
        if depth >= MAX_DEPTH {
            return Err(self.error("type nesting limit exceeded"));
        }
        let word = self.peek();
        let mut ty = match word {
            Some(TokenKind::Symbol('(')) => self.record_type(depth)?,
            Some(TokenKind::Word("int")) => Type::Int,
            Some(TokenKind::Word("double")) => Type::Double,
            Some(TokenKind::Word("num")) => Type::Num,
            Some(TokenKind::Word("String")) => Type::String,
            Some(TokenKind::Word("bool")) => Type::Bool,
            Some(TokenKind::Word("Object")) => Type::Object,
            Some(TokenKind::Word("Duration")) => Type::Duration,
            Some(TokenKind::Word("Timer")) if !self.class_ids.contains_key("Timer") => Type::Timer,
            Some(TokenKind::Word("Null")) => Type::Null,
            Some(TokenKind::Word("void")) => Type::Void,
            // `dynamic` é recusado de propósito, e não por falta de trabalho:
            // implementá-lo pela metade é pior do que não tê-lo. Um único
            // receptor dinâmico desliga o tree shaking de membros e obriga o
            // runtime a preservar todo método de mesmo nome do programa.
            Some(TokenKind::Word("dynamic")) => {
                return Err(self.error(
                    "dynamic is unsupported: this subset resolves every member statically, and dynamic dispatch would disable tree shaking and force the runtime to keep every same-named member; use an explicit type, Object or Object?",
                ));
            }
            Some(TokenKind::Word("Map")) => {
                self.index += 1;
                self.expect(TokenKind::Operator("<"))?;
                let key = self.type_at(false, depth + 1)?;
                self.expect(TokenKind::Symbol(','))?;
                let value = self.type_at(false, depth + 1)?;
                self.expect(TokenKind::Operator(">"))?;
                self.intern(TypeShape::Map { key, value })
            }
            Some(TokenKind::Word("List" | "Set" | "Iterable" | "Future")) => {
                self.index += 1;
                self.expect(TokenKind::Operator("<"))?;
                let element = self.type_at(word == Some(TokenKind::Word("Future")), depth + 1)?;
                self.expect(TokenKind::Operator(">"))?;
                self.intern(if word == Some(TokenKind::Word("List")) {
                    TypeShape::List(element)
                } else if word == Some(TokenKind::Word("Set")) {
                    TypeShape::Set(element)
                } else if word == Some(TokenKind::Word("Future")) {
                    TypeShape::Future(element)
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
            // Erasure de classe genérica: `T` vira seu bound; sem descritor
            // reificado, `C<int>` e `C<String>` usam a mesma representação.
            Some(TokenKind::Word(name)) if self.class_type_parameters.contains(&name) => {
                self.class_type_bounds[self
                    .class_type_parameters
                    .iter()
                    .position(|p| *p == name)
                    .expect("parâmetro de classe validado")]
            }
            // Apelido `typedef`/representation de extension type: já resolvido
            // no parse; `is`/`as` enxergam diretamente o tipo subjacente.
            Some(TokenKind::Word(name)) if self.typedefs.contains_key(name) => self.typedefs[name],
            Some(TokenKind::Word(name)) if self.class_ids.contains_key(name) => {
                Type::Class(self.class_ids[name])
            }
            _ => return Err(self.error("expected an explicitly supported type")),
        };
        if !matches!(
            word,
            Some(
                TokenKind::Word("List" | "Set" | "Iterable" | "Map" | "Future")
                    | TokenKind::Symbol('(')
            )
        ) {
            self.index += 1;
        }
        // Argumentos de classe genérica têm erasure: `C<int>` valida e descarta,
        // antes do `?` para que `C<int>?` forme o anulável da classe crua.
        if matches!(ty, Type::Class(_) | Type::NullableClass(_))
            || matches!(word, Some(TokenKind::Word(name)) if self.typedefs.contains_key(name))
        {
            self.discard_type_arguments();
        }
        if self.nullable_marker(depth) {
            ty = match ty {
                Type::Int => Type::NullableInt,
                Type::Double => Type::NullableDouble,
                Type::Num => Type::NullableNum,
                Type::Bool => Type::NullableBool,
                Type::String => Type::NullableString,
                Type::Class(id) => Type::NullableClass(id),
                Type::Object => Type::NullableObject,
                Type::Parameter(id) => Type::NullableParameter(id),
                Type::Applied(_) | Type::Duration | Type::Timer => {
                    self.intern(TypeShape::Nullable(ty))
                }
                // Nulabilidade é idempotente: `T?` com `T` já anulável é `T?`.
                // O caso aparece pelo erasure do parâmetro de classe, cujo bound
                // implícito é `Object?`: em `class A<T> { T? v; }` o `T` já vale
                // `Object?` quando o `?` é lido, e recusá-lo rejeitaria a forma
                // mais comum de campo genérico anulável.
                Type::NullableInt
                | Type::NullableDouble
                | Type::NullableNum
                | Type::NullableBool
                | Type::NullableString
                | Type::NullableObject
                | Type::NullableClass(_)
                | Type::NullableParameter(_) => ty,
                Type::Null => Type::Null,
                _ => return Err(self.error("type cannot be nullable")),
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
            if self.take(TokenKind::Operator("?")) {
                ty = self.intern(TypeShape::Nullable(ty));
            }
        }
        if ty == Type::Void && !allow_void {
            return Err(self.error("void value type is unsupported"));
        }
        Ok(ty)
    }
    /// Lê tipo estrutural de record e ordena somente os nomes de sua identidade.
    fn record_type(&mut self, depth: usize) -> Result<Type, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut labels = Vec::new();
        let mut comma = false;
        while self.peek() != Some(TokenKind::Symbol(')')) {
            if self.take(TokenKind::Symbol('{')) {
                if self.peek() == Some(TokenKind::Symbol('}')) {
                    return Err(self.error("named record types require at least one field"));
                }
                while self.peek() != Some(TokenKind::Symbol('}')) {
                    let ty = self.type_at(false, depth + 1)?;
                    let label = self.name()?;
                    labels.push(label);
                    let name = label.to_owned();
                    if named.iter().any(|(existing, _)| existing == &name) {
                        return Err(self.error("duplicate named record field"));
                    }
                    named.push((name, ty));
                    if !self.take(TokenKind::Symbol(',')) {
                        break;
                    }
                }
                self.expect(TokenKind::Symbol('}'))?;
                break;
            }
            positional.push(self.type_at(false, depth + 1)?);
            if matches!(self.peek(), Some(TokenKind::Word(name)) if !reserved(name)) {
                labels.push(self.name()?);
            }
            comma = self.take(TokenKind::Symbol(','));
            if !comma {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        if positional.len() == 1 && named.is_empty() && !comma {
            return Err(self.error("single-field record types require a trailing comma"));
        }
        let mut seen = std::collections::HashSet::new();
        for label in labels {
            if !seen.insert(label) || invalid_record_field_name(label, positional.len()) {
                return Err(self.error("invalid or duplicate record type field name"));
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(self.intern(TypeShape::Record { positional, named }))
    }
    /// Testa uma anotação local e restaura cursor e arena após a sondagem.
    fn starts_annotation(&mut self) -> bool {
        // `dynamic` começa uma anotação válida em Dart. Reconhecê-lo aqui faz
        // o diagnóstico específico de `type_at` chegar ao usuário; sem isto a
        // sondagem falharia em silêncio e a declaração viraria uma expressão,
        // com a mensagem genérica de expressão inválida.
        if self.peek() == Some(TokenKind::Word("dynamic")) {
            return true;
        }
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
        if (is_enum || kind != ClassKind::Class)
            && annotations
                .iter()
                .any(|annotation| matches!(annotation.kind, AnnotationKind::JsonCodable))
        {
            return Err(self.error("JsonCodable applies only to classes"));
        }
        let name = self.name()?;
        let id = *self.class_ids.get(name).ok_or_else(|| {
            self.error("declared class is missing from the supplied class environment")
        })?;
        if is_enum {
            return self.enumeration(id, name, start, annotations);
        }
        // Parâmetros de classe genérica com erasure: `T` resolve para o bound
        // durante todo o corpo; a representação em runtime é única por classe.
        let previous_class_params = std::mem::take(&mut self.class_type_parameters);
        let previous_class_bounds = std::mem::take(&mut self.class_type_bounds);
        let mut type_parameters = Vec::new();
        if self.take(TokenKind::Operator("<")) {
            loop {
                let param_start = self.position();
                let param_name = self.name()?;
                if self.class_type_parameters.contains(&param_name) {
                    return Err(self.error("duplicate class type parameter"));
                }
                self.class_type_parameters.push(param_name);
                self.class_type_bounds.push(Type::NullableObject);
                let bound = if self.take(TokenKind::Word("extends")) {
                    self.ty(false)?
                } else {
                    Type::NullableObject
                };
                if matches!(bound, Type::Void | Type::Inferred | Type::Null) {
                    return Err(self.error("Unsupported class generic bound"));
                }
                *self
                    .class_type_bounds
                    .last_mut()
                    .expect("bound recém-empilhado") = bound;
                type_parameters.push(GenericParameter {
                    name: param_name,
                    bound,
                    span: Span {
                        start: param_start,
                        end: self.end(),
                    },
                });
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Operator(">"))?;
        }
        // Dart 3.13: a lista primaria declara campos e o construtor sem nome.
        let primary = if self.peek() == Some(TokenKind::Symbol('(')) {
            if kind != ClassKind::Class {
                return Err(
                    self.error("primary constructors require an ordinary class declaration")
                );
            }
            Some(self.primary_constructor()?)
        } else {
            None
        };
        let superclass = if self.take(TokenKind::Word("extends")) {
            let parent = self.name()?;
            let base = *self
                .class_ids
                .get(parent)
                .ok_or_else(|| self.error("unknown superclass"))?;
            // Herança parametrizada tem erasure: `extends C<int>` valida e descarta.
            self.discard_type_arguments();
            Some(base)
        } else {
            None
        };
        // `mixin M on Base` restringe onde o mixin pode ser aplicado e dá
        // acesso aos membros de `Base` dentro do corpo. Não é herança: o mixin
        // continua sem construtor próprio e sem `super`.
        let mixin_constraint = if self.peek() == Some(TokenKind::Word("on")) {
            if kind != ClassKind::Mixin {
                return Err(self.error("only a mixin declaration accepts an on constraint"));
            }
            self.index += 1;
            let constraint = self.name()?;
            let constraint = *self.class_ids.get(constraint).ok_or_else(|| {
                self.error("unknown mixin constraint")
            })?;
            self.discard_type_arguments();
            if self.peek() == Some(TokenKind::Symbol(',')) {
                return Err(self.error(
                    "this subset accepts a single on constraint per mixin: several constraints would need a synthesized intersection type to resolve members against; declare the shared supertype and constrain on it",
                ));
            }
            Some(constraint)
        } else {
            None
        };
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
                self.discard_type_arguments();
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
        }
        // Corpo vazio abreviado por `;` so existe com construtor primario.
        let empty_body = primary.is_some() && self.take(TokenKind::Symbol(';'));
        if !empty_body {
            self.expect(TokenKind::Symbol('{'))?;
        }
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut abstract_methods = Vec::new();
        let mut constructor = None;
        let mut constructor_extras: Option<Box<ConstructorExtras<'a>>> = None;
        let mut named_constructors: Vec<NamedConstructor<'a>> = Vec::new();
        let mut static_fields: Vec<StaticField<'a>> = Vec::new();
        let mut static_methods = Vec::new();
        let mut factories = Vec::new();
        while !empty_body && self.peek() != Some(TokenKind::Symbol('}')) {
            let index = self.index;
            let (metadata, native) = self.metadata()?;
            if self.peek() == Some(TokenKind::Word("factory")) {
                if kind != ClassKind::Class {
                    return Err(
                        self.error("factory declarations require an ordinary class declaration")
                    );
                }
                if native.is_some() {
                    return Err(self.error(
                        "Native annotations belong to a top-level external function, not to a factory",
                    ));
                }
                // As macros nativas geram membros a partir da classe inteira e
                // não têm o que fazer numa fábrica; `@override` não se aplica a
                // construtor algum. `@Deprecated` e os metadados sem semântica
                // atravessam sem efeito, como em qualquer outro membro.
                if let Some(annotation) = metadata.iter().find(|annotation| {
                    !matches!(annotation.kind, AnnotationKind::Deprecated { .. })
                }) {
                    return Err(Diagnostic::new(
                        "this annotation targets a class or a method, not a factory; a factory accepts Deprecated and the semantics-free annotations of package:meta",
                        annotation.span,
                    ));
                }
                factories.push(self.factory(name, id)?);
                continue;
            }
            // `static` não participa de herança: o membro pertence à declaração.
            if self.peek() == Some(TokenKind::Word("static")) {
                if kind != ClassKind::Class {
                    return Err(self.error("static members require an ordinary class declaration"));
                }
                if !metadata.is_empty() || native.is_some() {
                    return Err(self.error("annotations on static members are not supported yet"));
                }
                self.index += 1;
                self.static_member(&mut static_fields, &mut static_methods, false)?;
                continue;
            }
            // `const C(...)` declara construtor const; `C.nome(...)`, um nomeado.
            let const_constructor = self.peek() == Some(TokenKind::Word("const"))
                && self.tokens.get(self.index + 1).map(|token| token.kind)
                    == Some(TokenKind::Word(name));
            if const_constructor
                || (self.peek() == Some(TokenKind::Word(name))
                    && matches!(
                        self.tokens.get(self.index + 1).map(|token| token.kind),
                        Some(TokenKind::Symbol('(' | '.'))
                    ))
            {
                if kind != ClassKind::Class {
                    return Err(
                        self.error("mixin declarations cannot declare generative constructors")
                    );
                }
                if !metadata.is_empty() || native.is_some() {
                    return Err(self.error("constructor annotations are not supported yet"));
                }
                if const_constructor {
                    self.index += 1;
                }
                let (member, declared, extras) = self.constructor(name, const_constructor)?;
                match member {
                    None => {
                        if constructor.is_some() {
                            return Err(self.error("only one unnamed constructor is supported"));
                        }
                        constructor = Some(declared);
                        if !extras.is_plain() {
                            constructor_extras = Some(Box::new(extras));
                        }
                    }
                    Some(member) => {
                        if named_constructors
                            .iter()
                            .any(|existing| existing.name == member)
                        {
                            return Err(self.error("duplicate named constructor"));
                        }
                        named_constructors.push(NamedConstructor {
                            name: member,
                            constructor: declared,
                            extras,
                        });
                    }
                }
                continue;
            }
            let field_start = self.position();
            // `set nome(T v)` sem tipo de retorno escrito: a leitura do tipo
            // precisa ser evitada porque `set` não é um tipo.
            if self.peek() == Some(TokenKind::Word("set")) {
                methods.push(self.instance_setter(field_start, Type::Void, metadata, native)?);
                continue;
            }
            let is_late = self.take(TokenKind::Word("late"));
            let is_final = self.take(TokenKind::Word("final"));
            let ty = self.ty(!is_final)?;
            if self.peek() == Some(TokenKind::Word("operator")) {
                if is_final || is_late {
                    return Err(self.error("operator methods cannot be late or final"));
                }
                methods.push(self.equality_operator(field_start, ty, metadata, native)?);
                continue;
            }
            if self.peek() == Some(TokenKind::Word("set")) {
                if is_final || is_late {
                    return Err(self.error("setters cannot be late or final"));
                }
                if ty != Type::Void {
                    return Err(self.error("a setter declares no return type or 'void'"));
                }
                methods.push(self.instance_setter(field_start, ty, metadata, native)?);
                continue;
            }
            let getter = self.take(TokenKind::Word("get"));
            let field_name = self.name()?;
            if getter || self.peek() == Some(TokenKind::Symbol('(')) {
                if is_final || is_late {
                    return Err(self.error("methods and getters cannot be late or final"));
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
                let initializer = if self.peek() == Some(TokenKind::Operator("=")) {
                    if is_late {
                        return Err(late_initializer_error(field_start));
                    }
                    self.index += 1;
                    Some(self.expression()?)
                } else {
                    None
                };
                self.expect(TokenKind::Symbol(';'))?;
                fields.push(Field {
                    name: field_name,
                    ty,
                    is_final,
                    is_late,
                    initializer,
                    span: Span {
                        start: field_start,
                        end: self.end(),
                    },
                });
            }
        }
        if !empty_body {
            self.expect(TokenKind::Symbol('}'))?;
        }
        if let Some(primary) = primary {
            if constructor.is_some() {
                return Err(self.error(
                    "a class with a primary constructor cannot declare another unnamed constructor",
                ));
            }
            constructor = Some(self.apply_primary(primary, &mut fields)?);
        }
        for declared in constructor
            .iter_mut()
            .chain(named_constructors.iter_mut().map(|c| &mut c.constructor))
        {
            for parameter in &mut declared.parameters {
                if let Some(name) = parameter.field
                    && let Some(field) = fields.iter().find(|field| field.name == name)
                {
                    parameter.ty = field.ty;
                }
            }
        }
        self.class_type_parameters = previous_class_params;
        self.class_type_bounds = previous_class_bounds;
        Ok(Class {
            mixin_constraint,
            factories,
            constructor,
            constructor_extras,
            named_constructors,
            static_fields,
            static_methods,
            is_library_globals: false,
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
            type_parameters,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê a lista primária de uma classe, que declara campos e o construtor.
    ///
    /// Aceita `Tipo nome` e `final Tipo nome`, ambos declarando campo final, e
    /// `var Tipo nome`, que declara campo mutável. A forma `this.nome` exige que
    /// o campo já exista no corpo da classe, de onde vem o tipo.
    fn primary_constructor(&mut self) -> Result<Vec<PrimaryParameter<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters: Vec<PrimaryParameter<'a>> = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let start = self.position();
            if self.take(TokenKind::Word("this")) {
                self.expect(TokenKind::Symbol('.'))?;
                let name = self.name()?;
                parameters.push(PrimaryParameter {
                    name,
                    declared: None,
                    is_final: true,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
            } else {
                let mutable = self.take(TokenKind::Word("var"));
                if !mutable {
                    self.take(TokenKind::Word("final"));
                }
                let ty = self.ty(false)?;
                if ty == Type::Void {
                    return Err(self.error("primary constructor fields cannot have void type"));
                }
                let name = self.name()?;
                parameters.push(PrimaryParameter {
                    name,
                    declared: Some(ty),
                    is_final: !mutable,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
            }
            let last = parameters[parameters.len() - 1].name;
            if parameters.iter().filter(|p| p.name == last).count() > 1 {
                return Err(self.error("duplicate primary constructor parameter"));
            }
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        if parameters.is_empty() {
            return Err(self.error("a primary constructor requires at least one parameter"));
        }
        Ok(parameters)
    }
    /// Materializa campos e o construtor sem nome a partir da lista primária.
    ///
    /// Os campos declarados pela lista precedem os do corpo, preservando a ordem
    /// de inicialização escrita na declaração da classe.
    fn apply_primary(
        &mut self,
        primary: Vec<PrimaryParameter<'a>>,
        fields: &mut Vec<Field<'a>>,
    ) -> Result<Constructor<'a>, Diagnostic> {
        let mut parameters = Vec::new();
        let mut declared: Vec<Field<'a>> = Vec::new();
        for parameter in primary {
            let existing = fields.iter().find(|field| field.name == parameter.name);
            let ty = match (parameter.declared, existing) {
                (Some(_), Some(_)) => {
                    return Err(self.error(
                        "a primary constructor field cannot be redeclared in the class body",
                    ));
                }
                (Some(ty), None) => {
                    declared.push(Field {
                        name: parameter.name,
                        ty,
                        is_final: parameter.is_final,
                        is_late: false,
                        initializer: None,
                        span: parameter.span,
                    });
                    ty
                }
                (None, Some(field)) => {
                    if field.initializer.is_some() {
                        return Err(self.error(
                            "a this. primary parameter requires a field without initializer",
                        ));
                    }
                    field.ty
                }
                (None, None) => {
                    return Err(self.error(
                        "a this. primary parameter requires the field declared in the class body",
                    ));
                }
            };
            parameters.push(ConstructorParameter::required(
                parameter.name,
                ty,
                Some(parameter.name),
                parameter.span,
            ));
        }
        let span = parameters
            .first()
            .map(|p| p.span)
            .expect("lista primária não vazia");
        declared.append(fields);
        *fields = declared;
        Ok(Constructor {
            parameters,
            body: Vec::new(),
            span,
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
                        is_late: false,
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
            mixin_constraint: None,
            factories: Vec::new(),
            constructor: None,
            constructor_extras: None,
            named_constructors: Vec::new(),
            static_fields: Vec::new(),
            static_methods: Vec::new(),
            is_library_globals: false,
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
            type_parameters: Vec::new(),
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê construtor posicional sem nome; this.campo obtém o tipo após ler a classe.
    fn constructor(
        &mut self,
        name: &'a str,
        is_const: bool,
    ) -> Result<(Option<&'a str>, Constructor<'a>, ConstructorExtras<'a>), Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word(name))?;
        let member = if self.take(TokenKind::Symbol('.')) {
            Some(self.name()?)
        } else {
            None
        };
        let parameters = self.constructor_parameter_list()?;
        let mut extras = ConstructorExtras {
            is_const,
            ..ConstructorExtras::default()
        };
        if self.take(TokenKind::Symbol(':')) {
            self.initializer_list(&mut extras)?;
        }
        let body = if self.take(TokenKind::Symbol(';')) {
            Vec::new()
        } else {
            self.block(0)?
        };
        if let Some(redirect) = &extras.redirect {
            if !body.is_empty() {
                return Err(Diagnostic::new(
                    "a redirecting constructor delegates entirely and cannot declare a body",
                    redirect.span,
                ));
            }
            if extras.is_const {
                return Err(Diagnostic::new(
                    "a const redirecting constructor is unsupported: the canonical instance is built from a single recipe of fields, and following a redirection would need a second recipe per target; declare the const constructor that initializes the fields directly",
                    redirect.span,
                ));
            }
        }
        Ok((
            member,
            Constructor {
                parameters,
                body,
                span: Span {
                    start,
                    end: self.end(),
                },
            },
            extras,
        ))
    }
    /// Lê a lista de inicialização, exigindo `super` como última entrada.
    ///
    /// Dart avalia as entradas na ordem escrita e só depois constrói a base; a
    /// exigência sintática de `super` no fim preserva essa ordem sem reordenar
    /// nada durante a emissão.
    fn initializer_list(&mut self, extras: &mut ConstructorExtras<'a>) -> Result<(), Diagnostic> {
        loop {
            let start = self.position();
            // `assert` na lista roda antes do corpo e antes de `super`. O
            // índice guardado preserva a ordem escrita entre asserções e
            // entradas `campo = valor`, que é a ordem de avaliação do Dart.
            if self.peek() == Some(TokenKind::Word("assert")) {
                self.index += 1;
                self.expect(TokenKind::Symbol('('))?;
                let condition = self.expression()?;
                let mut message = None;
                if self.take(TokenKind::Symbol(','))
                    && self.peek() != Some(TokenKind::Symbol(')'))
                {
                    message = Some(self.expression()?);
                    self.take(TokenKind::Symbol(','));
                }
                self.expect(TokenKind::Symbol(')'))?;
                extras.asserts.push(ConstructorAssert {
                    condition,
                    message,
                    before: extras.initializers.len(),
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
                if !self.take(TokenKind::Symbol(',')) {
                    return Ok(());
                }
                continue;
            }
            if self.peek() == Some(TokenKind::Word("super")) {
                if extras.redirect.is_some() {
                    return Err(self.error(
                        "a redirecting constructor delegates entirely and cannot also call super",
                    ));
                }
                self.index += 1;
                let name = if self.take(TokenKind::Symbol('.')) {
                    Some(self.name()?)
                } else {
                    None
                };
                if self.peek() != Some(TokenKind::Symbol('(')) {
                    return Err(
                        self.error("super in an initializer list requires an argument list")
                    );
                }
                let arguments = self.arguments(0)?;
                extras.super_call = Some(SuperCall {
                    name,
                    arguments,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
                if self.take(TokenKind::Symbol(',')) {
                    return Err(self.error("super must be the last entry of an initializer list"));
                }
                return Ok(());
            }
            if self.peek() == Some(TokenKind::Word("this")) {
                let next = self.tokens.get(self.index + 1).map(|token| token.kind);
                let redirect = next == Some(TokenKind::Symbol('('))
                    || (next == Some(TokenKind::Symbol('.'))
                        && self.tokens.get(self.index + 3).map(|token| token.kind)
                            == Some(TokenKind::Symbol('(')));
                if redirect {
                    // `C.nome() : this(0)` delega inteiramente: não inicializa
                    // campo, não chama `super` e não executa corpo próprio.
                    self.index += 1;
                    let name = if self.take(TokenKind::Symbol('.')) {
                        Some(self.name()?)
                    } else {
                        None
                    };
                    if !extras.initializers.is_empty() || extras.super_call.is_some() {
                        return Err(self.error(
                            "a redirecting constructor delegates entirely: it cannot also initialize fields or call super; move those to the target constructor",
                        ));
                    }
                    let arguments = self.arguments(0)?;
                    extras.redirect = Some(RedirectCall {
                        name,
                        arguments,
                        span: Span {
                            start,
                            end: self.end(),
                        },
                    });
                    if self.take(TokenKind::Symbol(',')) {
                        return Err(self.error(
                            "the redirection must be the last entry of an initializer list",
                        ));
                    }
                    return Ok(());
                }
                self.index += 1;
                self.expect(TokenKind::Symbol('.'))?;
            }
            let field = self.name()?;
            self.expect(TokenKind::Operator("="))?;
            let value = self.expression()?;
            extras.initializers.push(FieldInitializer {
                field,
                value,
                span: Span {
                    start,
                    end: self.end(),
                },
            });
            if !self.take(TokenKind::Symbol(',')) {
                return Ok(());
            }
        }
    }
    /// Deduz o tipo de um `const` escrito sem anotação, pelo inicializador.
    ///
    /// A dedução é sintática e deliberadamente estreita: o subconjunto resolve
    /// todo membro estaticamente, então o tipo de uma declaração de topo precisa
    /// ficar decidido antes da análise semântica, que recebe o programa por
    /// referência imutável e não teria onde gravar a resposta. Cobre literais,
    /// operadores constantes sobre eles, literais de coleção com elemento
    /// escrito ou uniforme, invocação de construtor e referência a um `const`
    /// declarado antes — que é a mesma ordem que a avaliação constante exige.
    ///
    /// # Erros
    /// Recusa qualquer outra forma, pedindo a anotação explícita.
    fn constant_type(&mut self, value: &Expr<'a>) -> Result<Type, Diagnostic> {
        let ty = match &value.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Double(_) => Type::Double,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::String(_) | ExprKind::OwnedString(_) | ExprKind::Interpolation(_) => {
                Type::String
            }
            ExprKind::Const(inner) => self.constant_type(inner)?,
            ExprKind::Construct { class_id, .. }
            | ExprKind::NamedConstruct { class_id, .. }
            | ExprKind::EnumValue { class_id, .. } => Type::Class(*class_id),
            ExprKind::Identifier(name) => *self
                .inferred_constants
                .get(name)
                .ok_or_else(|| Diagnostic::new(UNTYPED_CONST, value.span))?,
            ExprKind::Unary { op, operand } => match op {
                UnaryOp::Not => Type::Bool,
                UnaryOp::BitNot => Type::Int,
                UnaryOp::Negate => match self.constant_type(operand)? {
                    numeric @ (Type::Int | Type::Double | Type::Num) => numeric,
                    _ => return Err(Diagnostic::new(UNTYPED_CONST, value.span)),
                },
                UnaryOp::NullAssert => return Err(Diagnostic::new(UNTYPED_CONST, value.span)),
            },
            ExprKind::Binary { op, left, right } => match op {
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
                | BinaryOp::And
                | BinaryOp::Or => Type::Bool,
                // `/` produz double mesmo entre inteiros; `~/` e os bit a bit
                // produzem int mesmo entre doubles, como no oráculo Dart.
                BinaryOp::Divide => Type::Double,
                BinaryOp::TruncDivide
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::ShiftLeft
                | BinaryOp::ShiftRight
                | BinaryOp::ShiftRightUnsigned => Type::Int,
                BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Remainder => {
                    let left = self.constant_type(left)?;
                    let right = self.constant_type(right)?;
                    match (left, right) {
                        (Type::String, Type::String) if *op == BinaryOp::Add => Type::String,
                        (Type::Int, Type::Int) => Type::Int,
                        (
                            Type::Int | Type::Double | Type::Num,
                            Type::Int | Type::Double | Type::Num,
                        ) if left == Type::Num || right == Type::Num => Type::Num,
                        (Type::Int | Type::Double, Type::Int | Type::Double) => Type::Double,
                        _ => return Err(Diagnostic::new(UNTYPED_CONST, value.span)),
                    }
                }
                BinaryOp::IfNull => return Err(Diagnostic::new(UNTYPED_CONST, value.span)),
            },
            ExprKind::List {
                element_type,
                elements,
            } => {
                let element = self.uniform_element(*element_type, elements, value.span)?;
                self.intern(TypeShape::List(element))
            }
            ExprKind::Set {
                element_type,
                elements,
            } => {
                let element = self.uniform_element(*element_type, elements, value.span)?;
                self.intern(TypeShape::Set(element))
            }
            ExprKind::Map {
                key_type,
                value_type,
                entries,
            } => {
                let (Some(key), Some(element)) = (*key_type, *value_type) else {
                    let mut keys = Vec::with_capacity(entries.len());
                    let mut values = Vec::with_capacity(entries.len());
                    for (key, element) in entries {
                        let Some(element) = element else {
                            return Err(Diagnostic::new(UNTYPED_CONST, value.span));
                        };
                        keys.push(key.clone());
                        values.push(element.clone());
                    }
                    let key = self.uniform_element(None, &keys, value.span)?;
                    let element = self.uniform_element(None, &values, value.span)?;
                    return Ok(self.intern(TypeShape::Map {
                        key,
                        value: element,
                    }));
                };
                self.intern(TypeShape::Map {
                    key,
                    value: element,
                })
            }
            _ => return Err(Diagnostic::new(UNTYPED_CONST, value.span)),
        };
        Ok(ty)
    }
    /// Devolve o tipo de elemento escrito ou o comum a todos os elementos.
    ///
    /// # Erros
    /// Recusa literal vazio sem tipo escrito e literal com elementos de tipos
    /// diferentes: escolher um supertipo aqui repetiria, pela metade, a
    /// inferência que a análise semântica faz com o contexto inteiro.
    fn uniform_element(
        &mut self,
        written: Option<Type>,
        elements: &[Expr<'a>],
        span: Span,
    ) -> Result<Type, Diagnostic> {
        if let Some(written) = written {
            return Ok(written);
        }
        let mut common: Option<Type> = None;
        for element in elements {
            let ty = self.constant_type(element)?;
            match common {
                Some(previous) if previous != ty => {
                    return Err(Diagnostic::new(UNTYPED_CONST, span));
                }
                _ => common = Some(ty),
            }
        }
        common.ok_or_else(|| Diagnostic::new(UNTYPED_CONST, span))
    }
    /// Lê um membro estático já sem a palavra `static`, campo ou método.
    fn static_member(
        &mut self,
        fields: &mut Vec<StaticField<'a>>,
        methods: &mut Vec<Function<'a>>,
        is_top_level: bool,
    ) -> Result<(), Diagnostic> {
        let start = self.position();
        let is_late = self.take(TokenKind::Word("late"));
        let is_const = self.take(TokenKind::Word("const"));
        let is_final = !is_const && self.take(TokenKind::Word("final"));
        if is_late && is_const {
            return Err(self.error("late const is not supported"));
        }
        let signature = self.index;
        // `const kIndentSize = 2;` e `static const padrao = 'x';` omitem o tipo
        // e o tomam do inicializador constante. A sondagem é a mesma de uma
        // declaração local: só há anotação quando um tipo é seguido de um nome.
        let infer_constant = is_const && !self.starts_annotation();
        let ty = if infer_constant {
            Type::Inferred
        } else {
            self.ty(!is_const && !is_final)?
        };
        if self.peek() == Some(TokenKind::Word("set")) {
            return Err(self.error(if is_top_level {
                "top-level setters are not supported yet"
            } else {
                "static setters are not supported yet"
            }));
        }
        if self.peek() == Some(TokenKind::Word("get")) {
            return Err(self.error(if is_top_level {
                "top-level getters are not supported yet"
            } else {
                "static getters are not supported yet"
            }));
        }
        let name = self.name()?;
        if matches!(
            self.peek(),
            Some(TokenKind::Symbol('(') | TokenKind::Operator("<"))
        ) {
            if is_const || is_final || is_late {
                return Err(self.error("a static method cannot be const, final, or late"));
            }
            self.index = signature;
            let (method, _) = self.function_with_abstract(false, false)?;
            methods.push(method);
            return Ok(());
        }
        if ty == Type::Void {
            return Err(self.error("fields cannot have void type"));
        }
        let initializer = if self.peek() == Some(TokenKind::Operator("=")) {
            if is_late {
                return Err(late_initializer_error(start));
            }
            self.index += 1;
            Some(self.expression()?)
        } else {
            None
        };
        self.expect(TokenKind::Symbol(';'))?;
        let ty = if infer_constant {
            let span = Span {
                start,
                end: self.end(),
            };
            let initializer = initializer.as_ref().ok_or_else(|| {
                Diagnostic::new("a const declaration requires an initializer", span)
            })?;
            let ty = self.constant_type(initializer)?;
            if is_top_level {
                self.inferred_constants.insert(name, ty);
            }
            ty
        } else {
            ty
        };
        fields.push(StaticField {
            name,
            ty,
            is_final,
            is_const,
            is_late,
            initializer,
            span: Span {
                start,
                end: self.end(),
            },
        });
        Ok(())
    }
    /// Lê uma variável de topo `[const|final] Tipo nome [= valor];`.
    fn global_variable(&mut self) -> Result<StaticField<'a>, Diagnostic> {
        if self.peek() == Some(TokenKind::Symbol('@')) {
            return Err(self.error("annotations on top-level variables are not supported yet"));
        }
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        self.static_member(&mut fields, &mut methods, true)?;
        if let Some(method) = methods.first() {
            return Err(Diagnostic::new(
                "expected a top-level variable declaration",
                method.span,
            ));
        }
        Ok(fields.pop().expect("membro estático sem método é um campo"))
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
                self.discard_type_arguments();
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
        }
        Ok(ids)
    }
    /// Lê `typedef` moderno (`typedef F = int Function(int);`) ou legado
    /// (`typedef int F(int);`); o apelido resolve para o tipo já no parse.
    fn typedef_decl(&mut self) -> Result<(), Diagnostic> {
        self.expect(TokenKind::Word("typedef"))?;
        let mark = self.index;
        let alias_start = self.position();
        if let Ok(alias) = self.name() {
            let alias_span = Span {
                start: alias_start,
                end: self.end(),
            };
            if matches!(self.peek(), Some(TokenKind::Operator("=" | "<"))) {
                let previous_class_params = std::mem::take(&mut self.class_type_parameters);
                let previous_class_bounds = std::mem::take(&mut self.class_type_bounds);
                if self.take(TokenKind::Operator("<")) {
                    loop {
                        let param_name = self.name()?;
                        if self.class_type_parameters.contains(&param_name) {
                            self.class_type_parameters = previous_class_params;
                            self.class_type_bounds = previous_class_bounds;
                            return Err(self.error("duplicate typedef type parameter"));
                        }
                        self.class_type_parameters.push(param_name);
                        self.class_type_bounds.push(Type::NullableObject);
                        let bound = if self.take(TokenKind::Word("extends")) {
                            self.ty(false)?
                        } else {
                            Type::NullableObject
                        };
                        *self
                            .class_type_bounds
                            .last_mut()
                            .expect("bound recém-empilhado") = bound;
                        if !self.take(TokenKind::Symbol(',')) {
                            break;
                        }
                    }
                    self.expect(TokenKind::Operator(">"))?;
                }
                if self.class_ids.contains_key(alias) || self.typedefs.contains_key(alias) {
                    self.class_type_parameters = previous_class_params;
                    self.class_type_bounds = previous_class_bounds;
                    return Err(Diagnostic::new("duplicate typedef name", alias_span));
                }
                self.expect(TokenKind::Operator("="))?;
                let target = self.ty(true);
                self.class_type_parameters = previous_class_params;
                self.class_type_bounds = previous_class_bounds;
                let target = target?;
                self.expect(TokenKind::Symbol(';'))?;
                self.typedefs.insert(alias, target);
                return Ok(());
            }
        }
        self.index = mark;
        let result = self.ty(true)?;
        let alias_start = self.position();
        let alias = self.name()?;
        let alias_span = Span {
            start: alias_start,
            end: self.end(),
        };
        let previous_class_params = std::mem::take(&mut self.class_type_parameters);
        let previous_class_bounds = std::mem::take(&mut self.class_type_bounds);
        if self.take(TokenKind::Operator("<")) {
            loop {
                let param_name = self.name()?;
                if self.class_type_parameters.contains(&param_name) {
                    self.class_type_parameters = previous_class_params;
                    self.class_type_bounds = previous_class_bounds;
                    return Err(self.error("duplicate typedef type parameter"));
                }
                self.class_type_parameters.push(param_name);
                self.class_type_bounds.push(Type::NullableObject);
                let bound = if self.take(TokenKind::Word("extends")) {
                    self.ty(false)?
                } else {
                    Type::NullableObject
                };
                *self
                    .class_type_bounds
                    .last_mut()
                    .expect("bound recém-empilhado") = bound;
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Operator(">"))?;
        }
        if self.class_ids.contains_key(alias) || self.typedefs.contains_key(alias) {
            self.class_type_parameters = previous_class_params;
            self.class_type_bounds = previous_class_bounds;
            return Err(Diagnostic::new("duplicate typedef name", alias_span));
        }
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(')')) {
            parameters.push(self.ty(false)?);
            if matches!(self.peek(), Some(TokenKind::Word(param)) if !reserved(param)) {
                self.name()?;
            }
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        self.expect(TokenKind::Symbol(';'))?;
        self.class_type_parameters = previous_class_params;
        self.class_type_bounds = previous_class_bounds;
        let target = self.intern(TypeShape::Function { result, parameters });
        self.typedefs.insert(alias, target);
        Ok(())
    }
    /// Lê `extension type` com erasure para a representação e despacho estático.
    ///
    /// O tipo vira a representação (`is` checa a representação, custo zero em
    /// runtime); os membros viram uma extension sobre a representação e o
    /// construtor primário vira uma função-identidade com o nome declarado.
    fn extension_type(
        &mut self,
        extensions: &mut Vec<Extension<'a>>,
        functions: &mut Vec<Function<'a>>,
    ) -> Result<(), Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word("extension"))?;
        self.expect(TokenKind::Word("type"))?;
        let name = self.name()?;
        if self.class_ids.contains_key(name) || self.typedefs.contains_key(name) {
            return Err(self.error("duplicate extension type name"));
        }
        if self.peek() == Some(TokenKind::Operator("<")) {
            return Err(self.error("generic extension types are not supported yet"));
        }
        self.expect(TokenKind::Symbol('('))?;
        let representation = self.ty(false)?;
        let field = self.name()?;
        self.expect(TokenKind::Symbol(')'))?;
        if self.take(TokenKind::Word("implements")) {
            return Err(self.error("extension type implements clauses are not supported yet"));
        }
        self.expect(TokenKind::Symbol('{'))?;
        let mut methods = Vec::new();
        while self.peek() != Some(TokenKind::Symbol('}')) {
            if (self.peek() == Some(TokenKind::Word(name))
                && matches!(
                    self.tokens.get(self.index + 1).map(|token| token.kind),
                    Some(TokenKind::Symbol('(' | '.'))
                ))
                || (self.peek() == Some(TokenKind::Word("const"))
                    && self.tokens.get(self.index + 1).map(|token| token.kind)
                        == Some(TokenKind::Word(name)))
            {
                return Err(self.error(
                    "extension type constructors are not supported yet: use the primary representation constructor",
                ));
            }
            let method = self.function_with_abstract(false, false)?.0;
            if method.is_getter {
                return Err(Diagnostic::new(
                    "extension type getters are not supported yet",
                    method.span,
                ));
            }
            methods.push(method);
        }
        self.expect(TokenKind::Symbol('}'))?;
        let end = self.end();
        let span = Span { start, end };
        self.typedefs.insert(name, representation);
        let id = u32::try_from(extensions.len()).map_err(|_| self.error("too many extensions"))?;
        extensions.push(Extension {
            id,
            name,
            on_type: representation,
            methods,
            span,
        });
        functions.push(Function {
            is_arrow: false,
            is_async: false,
            annotations: Vec::new(),
            native_binding: None,
            type_parameters: Vec::new(),
            is_getter: false,
            name,
            return_type: representation,
            parameters: vec![Parameter {
                name: field,
                ty: representation,
                kind: ParameterKind::RequiredPositional,
                default: None,
                span,
            }],
            body: vec![Statement {
                kind: StatementKind::Return(Some(Expr {
                    kind: ExprKind::Identifier(field),
                    span,
                })),
                span,
            }],
            span,
        });
        Ok(())
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
            if prefix.is_some() && !ignorable_metadata(name) {
                return Err(self.error("prefixed metadata other than Native is not supported"));
            }
            if ignorable_metadata(name) {
                // Metadado sem efeito em geração de código: consome e descarta.
                // Aferido em código de produção, 65% dos arquivos do pacote `pdf`
                // param aqui, e nenhuma dessas anotações muda o programa emitido.
                if self.peek() == Some(TokenKind::Symbol('(')) {
                    let fallback = self.tokens[self.index].span;
                    self.index = skip_delimited(self.tokens, self.index, '(', ')', fallback)?;
                }
                continue;
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
                "JsonCodable" => {
                    self.expect(TokenKind::Symbol('('))?;
                    self.expect(TokenKind::Symbol(')'))?;
                    AnnotationKind::JsonCodable
                },
                "DataClass" => {
                    self.expect(TokenKind::Symbol('('))?;
                    self.expect(TokenKind::Symbol(')'))?;
                    AnnotationKind::DataClass
                },
                // `pragma` dirige o compilador: tolerá-la em silêncio seria
                // prometer honrar uma diretiva que não é lida. Erro próprio.
                "pragma" => {
                    return Err(self.error(
                        "@pragma directs the compiler and cannot be ignored; it is not implemented",
                    ));
                }
                _ => return Err(self.error("unsupported annotation; supported metadata: override, deprecated, Deprecated, JsonCodable, DataClass, Native and the semantics-free annotations of package:meta")),
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
            TokenKind::String(value) => decode_string(value, false, token.span)?,
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
    /// Lê uma fábrica nomeada com parâmetros posicionais e corpo de expressão.
    fn factory(&mut self, class_name: &'a str, class_id: u32) -> Result<Function<'a>, Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word("factory"))?;
        self.expect(TokenKind::Word(class_name))?;
        self.expect(TokenKind::Symbol('.'))?;
        let name = self.name()?;
        let parameters = self.parameter_list()?;
        // A fábrica com redirecionamento (`= Alvo` ou `= Alvo.nome`) vira um
        // corpo sintetizado que repassa cada parâmetro e converte o resultado
        // para a classe da fábrica; o `as` mantém a verificação em runtime e
        // dispensa sub-tipagem na análise semântica.
        if self.take(TokenKind::Operator("=")) {
            let target = self.name()?;
            let target_id = *self
                .class_ids
                .get(target)
                .ok_or_else(|| self.error("unknown factory redirect target"))?;
            let member = if self.take(TokenKind::Symbol('.')) {
                Some(self.name()?)
            } else {
                None
            };
            let call_start = start;
            let mut arguments = Vec::with_capacity(parameters.len());
            for parameter in &parameters {
                let value = Expr {
                    kind: ExprKind::Identifier(parameter.name),
                    span: parameter.span,
                };
                arguments.push(if parameter.kind.is_named() {
                    Expr {
                        kind: ExprKind::NamedArgument {
                            label: parameter.label(),
                            value: Box::new(value),
                        },
                        span: parameter.span,
                    }
                } else {
                    value
                });
            }
            let call_span = Span {
                start: call_start,
                end: self.end(),
            };
            let target_kind = match member {
                Some(member) => ExprKind::NamedConstruct {
                    class_id: target_id,
                    name: member,
                    arguments,
                },
                None => ExprKind::Construct {
                    class_id: target_id,
                    arguments,
                },
            };
            self.expect(TokenKind::Symbol(';'))?;
            let body = vec![Statement {
                kind: StatementKind::Return(Some(Expr {
                    kind: ExprKind::Cast {
                        operand: Box::new(Expr {
                            kind: target_kind,
                            span: call_span,
                        }),
                        ty: Type::Class(class_id),
                    },
                    span: call_span,
                })),
                span: call_span,
            }];
            return Ok(Function {
                is_arrow: false,
                is_async: false,
                name,
                return_type: Type::Class(class_id),
                parameters,
                body,
                span: Span {
                    start,
                    end: self.end(),
                },
                type_parameters: Vec::new(),
                is_getter: false,
                annotations: Vec::new(),
                native_binding: None,
            });
        }
        // A fábrica aceita corpo de expressão ou bloco, como qualquer função.
        let is_arrow = self.take(TokenKind::Operator("=>"));
        let body = if is_arrow {
            let value = self.expression()?;
            let span = value.span;
            self.expect(TokenKind::Symbol(';'))?;
            vec![Statement {
                kind: StatementKind::Return(Some(value)),
                span,
            }]
        } else {
            self.block(0)?
        };
        Ok(Function {
            is_arrow,
            is_async: false,
            name,
            return_type: Type::Class(class_id),
            parameters,
            body,
            span: Span {
                start,
                end: self.end(),
            },
            type_parameters: Vec::new(),
            is_getter: false,
            annotations: Vec::new(),
            native_binding: None,
        })
    }
    /// Lê o corpo em bloco ou em flecha de um acessor ou de `operator ==`.
    ///
    /// Reproduz a regra de descarte do corpo em flecha: com retorno `void`, o
    /// valor é avaliado e descartado, como em `void f() => 42` no Dart 3.6.2.
    ///
    /// # Erros
    /// Recusa corpo assíncrono e assinatura abstrata terminada por `;`.
    fn member_body(&mut self, return_type: Type) -> Result<(bool, Vec<Statement<'a>>), Diagnostic> {
        if self.peek() == Some(TokenKind::Word("async")) {
            return Err(self.error("setters and operator == cannot be async in this subset"));
        }
        if self.peek() == Some(TokenKind::Symbol(';')) {
            return Err(
                self.error("abstract setters and operator declarations are not supported yet")
            );
        }
        if !self.take(TokenKind::Operator("=>")) {
            return Ok((false, self.block(0)?));
        }
        let start = self.position();
        let value = self.expression()?;
        self.expect(TokenKind::Symbol(';'))?;
        let span = Span {
            start,
            end: self.end(),
        };
        let body = if return_type == Type::Void {
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
        };
        Ok((true, body))
    }
    /// Lê `set nome(T valor)` e devolve o acessor com um único parâmetro.
    ///
    /// O setter viaja em `Class::methods` marcado por `is_getter` com um
    /// parâmetro; a nota de [`dartforge_syntax::Function::is_getter`] explica
    /// por que a marca não é um campo novo da árvore.
    ///
    /// # Erros
    /// Recusa `@Native`, aridade diferente de um posicional obrigatório e as
    /// formas de corpo que [`Cursor::member_body`] já rejeita.
    fn instance_setter(
        &mut self,
        start: usize,
        return_type: Type,
        annotations: Vec<Annotation>,
        native: Option<NativeBinding<'a>>,
    ) -> Result<Function<'a>, Diagnostic> {
        if native.is_some() {
            return Err(self.error("Native is supported only on top-level functions"));
        }
        self.expect(TokenKind::Word("set"))?;
        let name = self.name()?;
        let parameters = self.parameter_list()?;
        if parameters.len() != 1 || parameters[0].kind != ParameterKind::RequiredPositional {
            return Err(Diagnostic::new(
                "a setter takes exactly one required positional parameter",
                Span {
                    start,
                    end: self.end(),
                },
            ));
        }
        let (is_arrow, body) = self.member_body(return_type)?;
        Ok(Function {
            is_arrow,
            is_async: false,
            annotations,
            native_binding: None,
            type_parameters: Vec::new(),
            is_getter: true,
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
    /// Lê `operator ==(Object outro)`, o único operador declarável do subconjunto.
    ///
    /// O nome interno passa a ser `==`, que nenhum identificador Dart pode ter:
    /// a declaração atravessa linker, otimizadores e emissão como um método
    /// comum, com herança e contrato de override já validados.
    ///
    /// # Erros
    /// Recusa qualquer outro operador com o intervalo do símbolo escrito.
    fn equality_operator(
        &mut self,
        start: usize,
        return_type: Type,
        annotations: Vec<Annotation>,
        native: Option<NativeBinding<'a>>,
    ) -> Result<Function<'a>, Diagnostic> {
        if native.is_some() {
            return Err(self.error("Native is supported only on top-level functions"));
        }
        self.expect(TokenKind::Word("operator"))?;
        if self.peek() != Some(TokenKind::Operator("==")) {
            return Err(self.error(
                "only 'operator ==' is supported; other operator declarations are not supported yet",
            ));
        }
        self.index += 1;
        let parameters = self.parameter_list()?;
        let (is_arrow, body) = self.member_body(return_type)?;
        Ok(Function {
            is_arrow,
            is_async: false,
            annotations,
            native_binding: None,
            type_parameters: Vec::new(),
            is_getter: false,
            name: dartforge_syntax::EQUALS_OPERATOR,
            return_type,
            parameters,
            body,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Distingue assinatura abstrata terminada por ponto e vírgula de corpo concreto.
    fn function_with_abstract(
        &mut self,
        allow_abstract: bool,
        allow_generic: bool,
    ) -> Result<(Function<'a>, bool), Diagnostic> {
        let start = self.position();
        let (annotations, mut native_binding) = self.metadata()?;
        if annotations
            .iter()
            .any(|annotation| matches!(annotation.kind, AnnotationKind::JsonCodable))
        {
            return Err(self.error("JsonCodable applies only to classes"));
        }
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
        let getter_start = self.position();
        let is_getter = self.take(TokenKind::Word("get"));
        if is_getter && allow_generic {
            return Err(Diagnostic::new(
                "top-level getters are not supported yet",
                Span {
                    start: getter_start,
                    end: self.end(),
                },
            ));
        }
        if allow_generic && self.peek() == Some(TokenKind::Word("set")) {
            return Err(self.error("top-level setters are not supported yet"));
        }
        let name = self.name()?;
        if let Some(binding) = native_binding.as_mut()
            && binding.symbol.is_empty()
        {
            binding.symbol = name.to_owned();
        }
        let mut generic_parameters = Vec::new();
        if self.take(TokenKind::Operator("<")) {
            if !allow_generic {
                return Err(self.error("generic methods are not supported yet"));
            }
            if self.type_parameters.is_empty() {
                return Err(self.error("generic declarations require named type parameters"));
            }
            for index in 0..self.type_parameters.len() {
                let start = self.position();
                let name = self.name()?;
                if name != self.type_parameters[index] {
                    return Err(self.error("unsupported type parameter declaration"));
                }
                let bound = if self.take(TokenKind::Word("extends")) {
                    self.ty(false)?
                } else {
                    Type::NullableObject
                };
                generic_parameters.push(GenericParameter {
                    name,
                    bound,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
                if index + 1 < self.type_parameters.len() {
                    self.expect(TokenKind::Symbol(','))?;
                } else {
                    self.take(TokenKind::Symbol(','));
                }
            }
            self.expect(TokenKind::Operator(">"))?;
        }
        let parameters = if is_getter {
            Vec::new()
        } else {
            self.parameter_list()?
        };
        let is_async = self.take(TokenKind::Word("async"));
        if is_async && (external || self.peek() == Some(TokenKind::Symbol(';'))) {
            return Err(self.error("async requires a concrete function body"));
        }
        let abstract_body = !external && allow_abstract && self.take(TokenKind::Symbol(';'));
        let is_arrow = !external && !abstract_body && self.take(TokenKind::Operator("=>"));
        let body = if external {
            self.expect(TokenKind::Symbol(';'))?;
            Vec::new()
        } else if abstract_body {
            Vec::new()
        } else if is_arrow {
            let start = self.position();
            let value = self.expression()?;
            self.expect(TokenKind::Symbol(';'))?;
            let span = Span {
                start,
                end: self.end(),
            };
            if return_type == Type::Void && !is_async {
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
        self.type_parameters = previous_parameters;
        let type_parameters = generic_parameters;
        Ok((
            Function {
                is_arrow,
                is_async,
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
    /// Pré-indexa nomes genéricos, ignorando limites balanceados até a assinatura formal.
    fn scan_type_parameters(&self) -> Result<Vec<&'a str>, Diagnostic> {
        let mut return_parentheses = 0usize;
        for index in self.index..self.tokens.len() {
            if return_parentheses == 0
                && matches!(
                    self.tokens[index].kind,
                    TokenKind::Symbol('{' | ';') | TokenKind::Operator("=>")
                )
            {
                break;
            }
            match self.tokens[index].kind {
                TokenKind::Symbol('(') => return_parentheses += 1,
                TokenKind::Symbol(')') => return_parentheses = return_parentheses.saturating_sub(1),
                _ => {}
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
                if self.tokens.get(cursor).map(|t| t.kind) == Some(TokenKind::Word("extends")) {
                    cursor += 1;
                    let mut angles = 0usize;
                    let mut parentheses = 0usize;
                    while let Some(token) = self.tokens.get(cursor) {
                        match token.kind {
                            TokenKind::Operator(">") if angles == 0 && parentheses == 0 => break,
                            TokenKind::Symbol(',') if angles == 0 && parentheses == 0 => break,
                            TokenKind::Operator("<") => angles += 1,
                            TokenKind::Operator(">") => angles = angles.saturating_sub(1),
                            TokenKind::Symbol('(') => parentheses += 1,
                            TokenKind::Symbol(')') => parentheses = parentheses.saturating_sub(1),
                            TokenKind::Symbol('{') if parentheses == 0 => break,
                            TokenKind::Symbol(';') | TokenKind::Operator("=>") => break,
                            _ => {}
                        }
                        cursor += 1;
                    }
                }
                if self.tokens.get(cursor).map(|t| t.kind) == Some(TokenKind::Symbol(',')) {
                    cursor += 1;
                    if self.tokens.get(cursor).map(|t| t.kind) != Some(TokenKind::Operator(">")) {
                        continue;
                    }
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
    /// Lê o valor padrão `= expressão` quando presente.
    ///
    /// A expressão só é validada como constante pela análise semântica; aqui
    /// apenas preservamos a árvore e o intervalo de origem.
    fn parameter_default(&mut self) -> Result<Option<Box<Expr<'a>>>, Diagnostic> {
        if !self.take(TokenKind::Operator("=")) {
            return Ok(None);
        }
        Ok(Some(Box::new(self.expression()?)))
    }
    /// Lê um parâmetro comum `Tipo nome [= padrão]` com a forma de passagem dada.
    fn parameter(&mut self, kind: ParameterKind) -> Result<Parameter<'a>, Diagnostic> {
        let start = self.position();
        self.parameter_metadata()?;
        if self.peek() == Some(TokenKind::Word("covariant")) {
            return Err(self.error("covariant parameters are not supported yet"));
        }
        let ty = self.ty(false)?;
        let name = self.name()?;
        let default = self.parameter_default()?;
        Ok(Parameter {
            name,
            ty,
            kind,
            default,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Consome os metadados escritos antes de um parâmetro.
    ///
    /// Um parâmetro não carrega anotação na árvore porque nenhuma das que o
    /// Dart de produção escreve ali muda o código emitido: `@Deprecated` é
    /// documentação e os metadados de `package:meta` são, por definição, sem
    /// semântica. Tolerar não é ignorar em silêncio — uma anotação cujo alvo
    /// não é um parâmetro continua recusada, com o span da própria anotação.
    ///
    /// # Erros
    /// Recusa `@Native` e as anotações cujo alvo é uma declaração, não um
    /// parâmetro.
    fn parameter_metadata(&mut self) -> Result<(), Diagnostic> {
        if self.peek() != Some(TokenKind::Symbol('@')) {
            return Ok(());
        }
        let start = self.position();
        let (annotations, native) = self.metadata()?;
        if native.is_some() {
            return Err(Diagnostic::new(
                "Native annotations belong to a top-level external function, not to a parameter",
                Span {
                    start,
                    end: self.end(),
                },
            ));
        }
        for annotation in &annotations {
            if !matches!(annotation.kind, AnnotationKind::Deprecated { .. }) {
                return Err(Diagnostic::new(
                    "this annotation targets a declaration, not a parameter; a parameter accepts Deprecated and the semantics-free annotations of package:meta",
                    annotation.span,
                ));
            }
        }
        Ok(())
    }
    /// Lê a lista completa de parâmetros de função, método ou fábrica.
    ///
    /// A ordem aceita é posicionais obrigatórios seguidos, no máximo, de um
    /// grupo `[...]` de posicionais opcionais **ou** de um grupo `{...}` de
    /// nomeados. Dart não permite os dois grupos na mesma assinatura e o
    /// diagnóstico explícito preserva essa regra.
    fn parameter_list(&mut self) -> Result<Vec<Parameter<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        while !matches!(self.peek(), Some(TokenKind::Symbol(')' | '[' | '{')) | None) {
            parameters.push(self.parameter(ParameterKind::RequiredPositional)?);
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        if self.take(TokenKind::Symbol('[')) {
            while self.peek() != Some(TokenKind::Symbol(']')) {
                parameters.push(self.parameter(ParameterKind::OptionalPositional)?);
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Symbol(']'))?;
            self.take(TokenKind::Symbol(','));
        } else if self.take(TokenKind::Symbol('{')) {
            while self.peek() != Some(TokenKind::Symbol('}')) {
                // Dart admite o metadado antes e depois de `required`.
                self.parameter_metadata()?;
                let required = self.take(TokenKind::Word("required"));
                parameters.push(self.parameter(ParameterKind::Named { required })?);
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Symbol('}'))?;
            self.take(TokenKind::Symbol(','));
        }
        if matches!(self.peek(), Some(TokenKind::Symbol('[' | '{'))) {
            return Err(self.error("a signature accepts a single optional or named group"));
        }
        self.expect(TokenKind::Symbol(')'))?;
        Ok(parameters)
    }
    /// Lê um parâmetro de construtor, aceitando o initializing formal `this.campo`.
    fn constructor_parameter(
        &mut self,
        kind: ParameterKind,
    ) -> Result<ConstructorParameter<'a>, Diagnostic> {
        let start = self.position();
        self.parameter_metadata()?;
        if self.peek() == Some(TokenKind::Word("covariant")) {
            return Err(self.error("covariant parameters are not supported yet"));
        }
        if self.peek() == Some(TokenKind::Word("super")) {
            return Err(self.error("super parameters are not supported yet"));
        }
        let (name, ty, field) = if self.take(TokenKind::Word("this")) {
            self.expect(TokenKind::Symbol('.'))?;
            let name = self.name()?;
            (name, Type::Inferred, Some(name))
        } else {
            let ty = self.ty(false)?;
            (self.name()?, ty, None)
        };
        let default = self.parameter_default()?;
        Ok(ConstructorParameter {
            name,
            ty,
            field,
            kind,
            default,
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê a lista de parâmetros de um construtor generativo, com os mesmos grupos.
    fn constructor_parameter_list(&mut self) -> Result<Vec<ConstructorParameter<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut parameters = Vec::new();
        while !matches!(self.peek(), Some(TokenKind::Symbol(')' | '[' | '{')) | None) {
            parameters.push(self.constructor_parameter(ParameterKind::RequiredPositional)?);
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        if self.take(TokenKind::Symbol('[')) {
            while self.peek() != Some(TokenKind::Symbol(']')) {
                parameters.push(self.constructor_parameter(ParameterKind::OptionalPositional)?);
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Symbol(']'))?;
            self.take(TokenKind::Symbol(','));
        } else if self.take(TokenKind::Symbol('{')) {
            while self.peek() != Some(TokenKind::Symbol('}')) {
                // Dart admite o metadado antes e depois de `required`.
                self.parameter_metadata()?;
                let required = self.take(TokenKind::Word("required"));
                parameters.push(self.constructor_parameter(ParameterKind::Named { required })?);
                if !self.take(TokenKind::Symbol(',')) {
                    break;
                }
            }
            self.expect(TokenKind::Symbol('}'))?;
            self.take(TokenKind::Symbol(','));
        }
        if matches!(self.peek(), Some(TokenKind::Symbol('[' | '{'))) {
            return Err(self.error("a signature accepts a single optional or named group"));
        }
        self.expect(TokenKind::Symbol(')'))?;
        Ok(parameters)
    }
    /// Lê argumentos posicionais e nomeados mantendo o limite compartilhado da expressão.
    ///
    /// Os nomeados devem vir depois de todos os posicionais; a ordem escrita é
    /// preservada na lista e é também a ordem de avaliação emitida.
    fn arguments(&mut self, depth: usize) -> Result<Vec<Expr<'a>>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut arguments: Vec<Expr<'a>> = Vec::new();
        let mut named = false;
        if self.peek() != Some(TokenKind::Symbol(')')) {
            loop {
                // Compartilha o limite de nós entre todos os argumentos, sem reiniciá-lo.
                let start = self.position();
                let label = match (self.peek(), self.tokens.get(self.index + 1).map(|t| t.kind)) {
                    (Some(TokenKind::Word(label)), Some(TokenKind::Symbol(':')))
                        if !reserved(label) =>
                    {
                        self.index += 2;
                        Some(label)
                    }
                    _ => None,
                };
                if label.is_none() && named {
                    return Err(self.error("positional arguments must precede named arguments"));
                }
                named |= label.is_some();
                let value = self.cascade(depth + 1)?;
                arguments.push(match label {
                    Some(label) => Expr {
                        kind: ExprKind::NamedArgument {
                            label,
                            value: Box::new(value),
                        },
                        span: Span {
                            start,
                            end: self.end(),
                        },
                    },
                    None => value,
                });
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
        let kind = if let Some(TokenKind::Word(label)) = self.peek()
            && !reserved(label)
            && self.tokens.get(self.index + 1).map(|token| token.kind)
                == Some(TokenKind::Symbol(':'))
        {
            self.index += 2;
            let body = self.statement(depth + 1)?;
            if !matches!(
                body.kind,
                StatementKind::While { .. }
                    | StatementKind::DoWhile { .. }
                    | StatementKind::For { .. }
                    | StatementKind::ForIn { .. }
                    | StatementKind::Labeled { .. }
            ) {
                return Err(Diagnostic::new(
                    "labels are supported only on loops",
                    body.span,
                ));
            }
            StatementKind::Labeled {
                label,
                body: Box::new(body),
            }
        } else if self.take(TokenKind::Word("try")) {
            self.try_statement(depth)?
        } else if self.take(TokenKind::Word("switch")) {
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
            if self.for_in_ahead() {
                return self.for_in(depth, start);
            }
            let initializer = if self.peek() == Some(TokenKind::Symbol(';')) {
                None
            } else {
                let initializer = self.simple(true)?;
                if matches!(initializer.kind, StatementKind::RecordDestructure { .. }) {
                    return Err(Diagnostic::new(
                        "record destructuring in for initializers is not supported yet",
                        initializer.span,
                    ));
                }
                Some(Box::new(initializer))
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
                match self.peek() {
                    Some(TokenKind::Word(label)) if !reserved(label) => {
                        self.index += 1;
                        StatementKind::BreakLabel(label)
                    }
                    _ => StatementKind::Break,
                }
            } else if self.take(TokenKind::Word("continue")) {
                match self.peek() {
                    Some(TokenKind::Word(label)) if !reserved(label) => {
                        self.index += 1;
                        StatementKind::ContinueLabel(label)
                    }
                    _ => StatementKind::Continue,
                }
            } else if self.take(TokenKind::Word("rethrow")) {
                StatementKind::Rethrow
            } else if self.peek() == Some(TokenKind::Word("throw")) {
                StatementKind::Expression(self.expression()?)
            } else if self.take(TokenKind::Word("assert")) {
                self.expect(TokenKind::Symbol('('))?;
                let condition = self.expression()?;
                let message = if self.take(TokenKind::Symbol(','))
                    && self.peek() != Some(TokenKind::Symbol(')'))
                {
                    Some(self.expression()?)
                } else {
                    None
                };
                self.take(TokenKind::Symbol(','));
                self.expect(TokenKind::Symbol(')'))?;
                StatementKind::Assert { condition, message }
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
    /// Lê as cláusulas `on`/`catch` e o bloco `finally` de um `try` já aberto.
    ///
    /// As cláusulas são conservadas na ordem escrita, que é a ordem de teste em
    /// execução. `on T { ... }` sem `catch` não liga variáveis; `catch (e, s)`
    /// liga o valor lançado e o rastro de pilha. Um `try` sem nenhuma cláusula
    /// e sem `finally` é rejeitado aqui, e não silenciosamente aceito.
    fn try_statement(&mut self, depth: usize) -> Result<StatementKind<'a>, Diagnostic> {
        let body = self.block(depth)?;
        let mut catches = Vec::new();
        loop {
            let start = self.position();
            let exception_type = if self.take(TokenKind::Word("on")) {
                Some(self.ty(false)?)
            } else {
                None
            };
            let (exception, stack_trace) = if self.take(TokenKind::Word("catch")) {
                self.expect(TokenKind::Symbol('('))?;
                let exception = self.name()?;
                let stack_trace = if self.take(TokenKind::Symbol(',')) {
                    Some(self.name()?)
                } else {
                    None
                };
                self.expect(TokenKind::Symbol(')'))?;
                (Some(exception), stack_trace)
            } else if exception_type.is_some() {
                (None, None)
            } else {
                break;
            };
            let body = self.block(depth)?;
            catches.push(CatchClause {
                exception_type,
                exception,
                stack_trace,
                body,
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        let finally_body = if self.take(TokenKind::Word("finally")) {
            Some(self.block(depth)?)
        } else {
            None
        };
        if catches.is_empty() && finally_body.is_none() {
            return Err(self.error("try requires at least one on, catch or finally clause"));
        }
        Ok(StatementKind::Try {
            body,
            catches,
            finally_body,
        })
    }
    /// Indica se o cabeçalho de `for` já aberto é da forma `for (... in ...)`.
    ///
    /// A varredura para no primeiro `;` ou no fecha-parêntese do próprio
    /// cabeçalho, de modo que um `in` dentro de uma closure aninhada não muda
    /// a decisão.
    fn for_in_ahead(&self) -> bool {
        let mut depth = 0usize;
        for token in &self.tokens[self.index..] {
            match token.kind {
                TokenKind::Symbol('(' | '[' | '{') => depth += 1,
                TokenKind::Symbol(')' | ']' | '}') => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                TokenKind::Symbol(';') if depth == 0 => return false,
                TokenKind::Word("in") if depth == 0 => return true,
                _ => {}
            }
        }
        false
    }
    /// Lê `for (declaração in expressão) { ... }` com o parêntese já consumido.
    ///
    /// Só a forma declarativa é aceita: `for (x in lista)` sobre uma variável
    /// existente é rejeitada explicitamente, porque a emissão precisaria de uma
    /// atribuição por iteração que este subconjunto ainda não modela.
    fn for_in(&mut self, depth: usize, start: usize) -> Result<Statement<'a>, Diagnostic> {
        if self.peek() == Some(TokenKind::Word("const")) {
            return Err(self.error("const is not allowed in a for-in variable"));
        }
        let is_final = self.take(TokenKind::Word("final"));
        let inferred = self.take(TokenKind::Word("var"));
        if is_final && inferred {
            return Err(self.error("final var is not supported; use final name in expression"));
        }
        let annotation = if !inferred && !self.at_for_in_name() {
            Some(self.ty(false)?)
        } else {
            None
        };
        if !is_final && !inferred && annotation.is_none() {
            return Err(self.error(
                "for-in requires final, var or a type; assigning to an existing variable is not supported",
            ));
        }
        let name = self.name()?;
        self.expect(TokenKind::Word("in"))?;
        let iterable = self.expression()?;
        self.expect(TokenKind::Symbol(')'))?;
        let body = self.block(depth)?;
        Ok(Statement {
            kind: StatementKind::ForIn {
                is_final,
                name,
                annotation,
                iterable,
                body,
            },
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Indica que o token atual já é o nome da variável de um `for-in`.
    fn at_for_in_name(&self) -> bool {
        self.tokens.get(self.index + 1).map(|token| token.kind) == Some(TokenKind::Word("in"))
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
        let is_late = self.take(TokenKind::Word("late"));
        if is_late && self.peek() == Some(TokenKind::Word("const")) {
            return Err(self.error("late const is not supported"));
        }
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
            if self.peek() == Some(TokenKind::Symbol('(')) && !self.starts_annotation() {
                if is_const {
                    return Err(self.error("const record destructuring is not supported"));
                }
                let kind = self.record_destructure(is_final)?;
                return Ok(Statement {
                    kind,
                    span: Span {
                        start,
                        end: self.end(),
                    },
                });
            }
            let annotation = if !inferred && self.starts_annotation() {
                Some(self.ty(false)?)
            } else {
                None
            };
            let name = self.name()?;
            // `bool? b;` é válido em Dart 3.6.2 e inicializa com null; só
            // vale para tipo anulável e não-final, desaçucarado para `= null`.
            // `final` sem inicializador e tipo não-anulável sem inicializador
            // continuam exigindo `=`.
            let nullable = match annotation {
                Some(
                    Type::NullableInt
                    | Type::NullableBool
                    | Type::NullableString
                    | Type::NullableObject
                    | Type::NullableClass(_)
                    | Type::NullableParameter(_),
                ) => true,
                Some(Type::Applied(id)) => {
                    matches!(self.types.get(id as usize), Some(TypeShape::Nullable(_)))
                }
                _ => false,
            };
            let initializer = if self.peek() == Some(TokenKind::Operator("=")) {
                if is_late {
                    return Err(late_initializer_error(start));
                }
                self.index += 1;
                self.expression()?
            } else if (!is_final && nullable) || is_late {
                let end = self.end();
                Expr {
                    kind: ExprKind::Null,
                    span: Span { start: end, end },
                }
            } else {
                self.expect(TokenKind::Operator("="))?;
                unreachable!("o `=` foi testado acima");
            };
            StatementKind::Variable {
                is_late,
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
                    // `C.x = v` chega aqui como acesso estático; a escrita
                    // exige resolução de estáticos na análise semântica (ver
                    // gaps documentados), então a rejeição é explícita.
                    ExprKind::EnumValue { .. } => {
                        return Err(Diagnostic::new(
                            "static field assignments are not supported yet",
                            value.span,
                        ));
                    }
                    _ => return Err(self.error("assignment requires a member or index target")),
                }
            } else {
                if !matches!(
                    value.kind,
                    ExprKind::Call { .. }
                        | ExprKind::GenericCall { .. }
                        | ExprKind::MethodCall { .. }
                        | ExprKind::Construct { .. }
                        | ExprKind::NamedConstruct { .. }
                        | ExprKind::Await(_)
                        | ExprKind::FutureValue { .. }
                        | ExprKind::FutureDelayed { .. }
                        | ExprKind::Duration { .. }
                        | ExprKind::Invoke { .. }
                        | ExprKind::Cascade { .. }
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
    /// Reinicia o orçamento somente fora de outra expressão, inclusive corpos de closures.
    fn expression(&mut self) -> Result<Expr<'a>, Diagnostic> {
        if self.expression_frames == 0 {
            self.expr_nodes = 0;
        }
        self.expression_frames += 1;
        let result = self.throw_or(0, false);
        self.expression_frames -= 1;
        result
    }
    /// Lê `throw expressão` ou delega à cascata; `plain` proíbe a cascata.
    ///
    /// Dart admite `throw` apenas no topo de uma expressão e nos ramos do
    /// operador condicional. Por isso `a ?? throw e` exige parênteses, e
    /// `a ?? (throw e)` chega aqui pelo grupo entre parênteses.
    fn throw_or(&mut self, depth: usize, plain: bool) -> Result<Expr<'a>, Diagnostic> {
        if self.peek() == Some(TokenKind::Word("throw")) {
            let start = self.position();
            self.charge(depth)?;
            self.index += 1;
            let value = self.throw_or(depth + 1, plain)?;
            let end = value.span.end;
            return Ok(Expr {
                kind: ExprKind::Throw(Box::new(value)),
                span: Span { start, end },
            });
        }
        if plain {
            self.conditional(depth)
        } else {
            self.cascade(depth)
        }
    }
    /// Lê o operador condicional, associativo à direita e acima da cascata.
    ///
    /// A condição é uma expressão de `??` completa, então `a ?? b ? c : d`
    /// agrupa `(a ?? b) ? c : d`, como em Dart 3.6.2. Os dois ramos aceitam
    /// `throw` e outro condicional, mas não cascatas.
    fn conditional(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let condition = self.binary(0, depth)?;
        if self.peek() != Some(TokenKind::Operator("?")) {
            return Ok(condition);
        }
        self.charge(depth)?;
        self.index += 1;
        let then_value = self.throw_or(depth + 1, true)?;
        self.expect(TokenKind::Symbol(':'))?;
        let else_value = self.throw_or(depth + 1, true)?;
        let span = Span {
            start: condition.span.start,
            end: else_value.span.end,
        };
        Ok(Expr {
            kind: ExprKind::Conditional {
                condition: Box::new(condition),
                then_value: Box::new(then_value),
                else_value: Box::new(else_value),
            },
            span,
        })
    }
    /// Lê cascatas sem absorver a próxima seção no lado direito de atribuições.
    fn cascade(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let receiver = self.conditional(depth)?;
        if !matches!(self.peek(), Some(TokenKind::Operator(".." | "?.."))) {
            return Ok(receiver);
        }
        self.charge(depth)?;
        let start = receiver.span.start;
        let null_aware = self.peek() == Some(TokenKind::Operator("?.."));
        let mut sections = Vec::new();
        while matches!(self.peek(), Some(TokenKind::Operator(".." | "?.."))) {
            if !sections.is_empty() && self.peek() == Some(TokenKind::Operator("?..")) {
                return Err(self.error("only the first cascade section may be null-aware"));
            }
            self.charge(depth + 1)?;
            let span = self.tokens[self.index].span;
            self.index += 1;
            let target = Expr {
                kind: ExprKind::CascadeReceiver,
                span,
            };
            let target = if self.peek() == Some(TokenKind::Symbol('[')) {
                self.postfix(target, depth + 1)?
            } else {
                let name = self.name()?;
                let kind = if self.peek() == Some(TokenKind::Symbol('(')) {
                    ExprKind::MethodCall {
                        receiver: Box::new(target),
                        name,
                        arguments: self.arguments(depth + 1)?,
                    }
                } else {
                    ExprKind::Member {
                        receiver: Box::new(target),
                        name,
                    }
                };
                let value = Expr {
                    kind,
                    span: Span {
                        start: span.start,
                        end: self.end(),
                    },
                };
                self.postfix(value, depth + 1)?
            };
            if matches!(
                self.peek(),
                Some(TokenKind::Operator("+=" | "-=" | "*=" | "++" | "--"))
            ) {
                return Err(
                    self.error("compound assignments and updates in cascades are not supported")
                );
            }
            let kind = if self.take(TokenKind::Operator("=")) {
                let value = self.binary(0, depth + 1)?;
                match target.kind {
                    ExprKind::Member { receiver, name } => StatementKind::FieldAssign {
                        receiver: *receiver,
                        name,
                        value,
                    },
                    ExprKind::Index { receiver, index } => StatementKind::IndexAssign {
                        receiver: *receiver,
                        index: *index,
                        value,
                    },
                    _ => {
                        return Err(
                            self.error("cascade assignment requires a member or index target")
                        );
                    }
                }
            } else {
                StatementKind::Expression(target)
            };
            sections.push(Statement {
                kind,
                span: Span {
                    start: span.start,
                    end: self.end(),
                },
            });
        }
        Ok(Expr {
            kind: ExprKind::Cascade {
                receiver: Box::new(receiver),
                null_aware,
                sections,
            },
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Reconhece o início de um seletor null-aware `?.` ou `?[`.
    ///
    /// A adjacência em bytes é obrigatória, como no scanner do Dart: só assim
    /// `c ? .a : .b`, que combina o condicional com os atalhos de ponto do
    /// Dart 3.10, continua sendo um condicional. `?..` é a cascata null-aware
    /// e chega como um token próprio, portanto nunca cai aqui.
    fn null_aware_selector(&self) -> Option<NullAwareSelector> {
        let question = self.tokens.get(self.index)?;
        if question.kind != TokenKind::Operator("?") {
            return None;
        }
        let next = self.tokens.get(self.index + 1)?;
        if next.span.start != question.span.end {
            return None;
        }
        match next.kind {
            TokenKind::Symbol('.') => Some(NullAwareSelector::Member),
            TokenKind::Symbol('[') => Some(NullAwareSelector::Index),
            _ => None,
        }
    }
    /// Reconhece `>>` e `>>>` como `>` adjacentes e informa quantos consumir.
    ///
    /// O lexer não junta `>` para que `List<List<int>>` continue fechando dois
    /// argumentos de tipo com o mesmo token esperado em toda a leitura de
    /// tipos. A distinção fica aqui e é puramente léxica: só há deslocamento
    /// quando os tokens são adjacentes em bytes, sem espaço nem comentário
    /// entre eles, exatamente como no scanner do Dart.
    fn shift_ahead(&self) -> Option<(u8, BinaryOp, usize)> {
        let first = self.tokens.get(self.index)?;
        if first.kind != TokenKind::Operator(">") {
            return None;
        }
        let second = self.tokens.get(self.index + 1)?;
        if second.kind != TokenKind::Operator(">") || second.span.start != first.span.end {
            return None;
        }
        if let Some(third) = self.tokens.get(self.index + 2)
            && third.kind == TokenKind::Operator(">")
            && third.span.start == second.span.end
        {
            return Some((8, BinaryOp::ShiftRightUnsigned, 3));
        }
        Some((8, BinaryOp::ShiftRight, 2))
    }
    /// Contabiliza um nó e rejeita expressões que excedam os limites.
    ///
    /// `depth` é a profundidade da árvore no ponto do nó, e não a da recursão
    /// do parser: as cadeias associativas à esquerda passam aqui o nível já
    /// aprofundado pela própria iteração.
    fn charge(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.expr_nodes += 1;
        if depth >= MAX_DEPTH || self.expr_nodes > MAX_EXPR_NODES {
            return Err(self.error("expression complexity limit exceeded"));
        }
        Ok(())
    }
    /// Lê operadores binários respeitando precedência e associatividade.
    ///
    /// Cada operador consumido no laço acrescenta um nível à árvore resultante,
    /// porque a associatividade à esquerda pendura o nó anterior como operando
    /// esquerdo do novo. `level` acompanha essa profundidade real e é o que vai
    /// ao orçamento: sem ele, `1+1+1+…` seria plano para o limite e fundo para
    /// quem percorre a árvore depois.
    fn binary(&mut self, min: u8, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let mut left = self.primary(depth)?;
        if let Some(TokenKind::Operator(operator @ ("++" | "--"))) = self.peek() {
            left = self.increment_suffix(left, operator, depth)?;
        }
        let mut level = depth;
        let mut comparison = None;
        let mut type_comparison = false;
        loop {
            if let Some(TokenKind::Word(operator @ ("is" | "as"))) = self.peek() {
                if 4 < min {
                    break;
                }
                if comparison == Some(4) {
                    return Err(self.error(
                        "type tests and relational operators cannot be chained without parentheses",
                    ));
                }
                comparison = Some(4);
                type_comparison = true;
                level += 1;
                self.charge(level)?;
                self.index += 1;
                let negated = operator == "is" && self.take(TokenKind::Operator("!"));
                let ty = self.test_type()?;
                let span = Span {
                    start: left.span.start,
                    end: self.end(),
                };
                left = Expr {
                    kind: if operator == "is" {
                        ExprKind::TypeTest {
                            operand: Box::new(left),
                            ty,
                            negated,
                        }
                    } else {
                        ExprKind::Cast {
                            operand: Box::new(left),
                            ty,
                        }
                    },
                    span,
                };
                continue;
            }
            let Some(TokenKind::Operator(symbol)) = self.peek() else {
                break;
            };
            // `>>` e `>>>` chegam como `>` adjacentes; ver `shift_ahead`.
            let (precedence, op, width) = match self.shift_ahead() {
                Some(shift) => shift,
                None => match binary_op(symbol) {
                    Some((precedence, op)) => (precedence, op, 1),
                    None => break,
                },
            };
            if precedence < min {
                break;
            }
            if type_comparison && precedence >= 4 {
                return Err(self.error("operators after a type test or cast require parentheses"));
            }
            if matches!(precedence, 3 | 4) {
                if comparison == Some(precedence) {
                    return Err(
                        self.error("comparison operators cannot be chained without parentheses")
                    );
                }
                comparison = Some(precedence);
            }
            level += 1;
            self.charge(level)?;
            self.index += width;
            let next_min = if op == BinaryOp::IfNull {
                precedence
            } else {
                precedence + 1
            };
            let right = self.binary(next_min, level)?;
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
    /// Lê um literal de string, juntando literais adjacentes em tempo de compilação.
    ///
    /// Dart concatena `'a' 'b'` sem operador, e qualquer uma das partes pode ser
    /// interpolada. Sem nenhuma interpolação o resultado volta a ser um literal
    /// simples, que continua emprestado da fonte quando não houve escape nem
    /// junção. Com interpolação, as partes ficam na ordem escrita e cada
    /// expressão aparece exatamente uma vez.
    fn string_literal(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
        let mut parts = Vec::new();
        while matches!(
            self.peek(),
            Some(
                TokenKind::String(_)
                    | TokenKind::RawString(_)
                    | TokenKind::MultilineString { .. }
                    | TokenKind::StringStart { .. }
            )
        ) {
            self.string_piece(depth, &mut parts)?;
        }
        if parts
            .iter()
            .any(|part| matches!(part, StringPart::Expression(_)))
        {
            return Ok(ExprKind::Interpolation(parts));
        }
        Ok(match parts.pop() {
            None => ExprKind::String(""),
            Some(StringPart::Borrowed(text)) => ExprKind::String(text),
            Some(StringPart::Owned(text)) => ExprKind::OwnedString(text),
            Some(StringPart::Expression(_)) => unreachable!("nenhuma expressão nesta posição"),
        })
    }
    /// Consome um literal completo, que pode conter várias interpolações.
    fn string_piece(
        &mut self,
        depth: usize,
        parts: &mut Vec<StringPart<'a>>,
    ) -> Result<(), Diagnostic> {
        let token = self.tokens[self.index];
        self.index += 1;
        match token.kind {
            TokenKind::String(text) => {
                push_literal(parts, decode_chunk(text, false, false, token.span)?);
                return Ok(());
            }
            TokenKind::RawString(text) => {
                push_literal(parts, std::borrow::Cow::Borrowed(text));
                return Ok(());
            }
            TokenKind::MultilineString { text, raw } => {
                push_literal(parts, decode_chunk(text, true, raw, token.span)?);
                return Ok(());
            }
            TokenKind::StringStart { text, multiline } => {
                push_literal(parts, decode_chunk(text, multiline, false, token.span)?);
            }
            _ => unreachable!("chamada sem token de string à frente"),
        }
        loop {
            parts.push(StringPart::Expression(self.interpolated_expression(depth)?));
            let token = self
                .tokens
                .get(self.index)
                .copied()
                .ok_or_else(|| self.error("unterminated string interpolation"))?;
            match token.kind {
                TokenKind::StringMid { text, multiline } => {
                    self.index += 1;
                    push_literal(parts, decode_chunk(text, multiline, false, token.span)?);
                }
                TokenKind::StringEnd { text, multiline } => {
                    self.index += 1;
                    push_literal(parts, decode_chunk(text, multiline, false, token.span)?);
                    return Ok(());
                }
                _ => {
                    return Err(Diagnostic::new(
                        "string interpolation accepts a single expression",
                        token.span,
                    ));
                }
            }
        }
    }
    /// Lê a expressão de uma interpolação, distinguindo `$nome` de `${expressão}`.
    ///
    /// `'$obj.campo'` interpola apenas `obj`: o lexer já separou `.campo` como
    /// texto, então a forma simples nunca continua em acesso a membro.
    fn interpolated_expression(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        if let Some(token) = self.tokens.get(self.index).copied()
            && let TokenKind::InterpolatedName(name) = token.kind
        {
            self.index += 1;
            if reserved(name) {
                return Err(Diagnostic::new(
                    format!("'{name}' can't be used as an identifier because it's a keyword"),
                    token.span,
                ));
            }
            return Ok(Expr {
                kind: ExprKind::Identifier(name),
                span: token.span,
            });
        }
        self.cascade(depth + 1)
    }
    /// Lê literais, referências, chamadas e operadores unários.
    fn primary(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        if self.active_primaries >= MAX_ACTIVE_PRIMARIES {
            return Err(self.error("active expression nesting limit exceeded"));
        }
        self.active_primaries += 1;
        let result = self.primary_inner(depth);
        self.active_primaries -= 1;
        result
    }
    /// Analisa a primária sob o orçamento global já reservado pelo invólucro.
    fn primary_inner(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        self.charge(depth)?;
        let start = self.position();
        let kind = match self.peek() {
            Some(TokenKind::Word("await")) => {
                self.index += 1;
                ExprKind::Await(Box::new(self.primary(depth + 1)?))
            }
            Some(TokenKind::Word("Future")) => self.future_expression(depth)?,
            Some(TokenKind::Word("Duration")) => self.duration_expression(depth)?,
            Some(TokenKind::Word("const")) => {
                self.index += 1;
                if self.peek() == Some(TokenKind::Word("Duration")) {
                    return Err(self.error(
                        "const Duration is not supported yet; use Duration.zero or Duration(...)",
                    ));
                }
                // `const C(...)` e `const C.nome(...)` viram o mesmo nó de
                // construção envolto em `Const`; a canonicalização é decidida
                // pela receita const da análise semântica.
                let invoked = match self.peek() {
                    Some(TokenKind::Word(name)) if self.class_ids.contains_key(name) => Some(name),
                    _ => None,
                };
                if let Some(name) = invoked {
                    let class_id = self.class_ids[name];
                    self.index += 1;
                    let kind = if self.take(TokenKind::Symbol('.')) {
                        let member = self.name()?;
                        ExprKind::NamedConstruct {
                            class_id,
                            name: member,
                            arguments: self.arguments(depth)?,
                        }
                    } else {
                        ExprKind::Construct {
                            class_id,
                            arguments: self.arguments(depth)?,
                        }
                    };
                    ExprKind::Const(Box::new(Expr {
                        kind,
                        span: Span {
                            start,
                            end: self.end(),
                        },
                    }))
                } else {
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
            }
            Some(TokenKind::Word("switch")) => {
                self.index += 1;
                self.expect(TokenKind::Symbol('('))?;
                let scrutinee = self.cascade(depth + 1)?;
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
                    let value = self.cascade(depth + 1)?;
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
                if text.bytes().any(|b| matches!(b, b'.' | b'e' | b'E')) {
                    let value = text.parse::<f64>().map_err(|_| {
                        self.error("invalid double literal outside supported 64-bit range")
                    })?;
                    self.index += 1;
                    ExprKind::Double(value)
                } else {
                    let value = text.parse::<i32>().map_err(|_| {
                        self.error("integer literal outside supported signed 32-bit range")
                    })?;
                    self.index += 1;
                    ExprKind::Int(value)
                }
            }
            Some(
                TokenKind::String(_)
                | TokenKind::RawString(_)
                | TokenKind::MultilineString { .. }
                | TokenKind::StringStart { .. },
            ) => self.string_literal(depth)?,
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
                    // `C<int>(...)` com `C` sendo classe é construção genérica,
                    // não chamada de função genérica. Quem decide é a análise
                    // semântica, que tem os bounds de cada parâmetro e valida os
                    // argumentos antes de apagá-los; o parser não os tem.
                    ExprKind::GenericCall {
                        name,
                        type_arguments,
                        arguments,
                    }
                } else if self.class_ids.contains_key(name) && self.take(TokenKind::Symbol('.')) {
                    let member = self.name()?;
                    if self.peek() == Some(TokenKind::Symbol('(')) {
                        ExprKind::NamedConstruct {
                            class_id: self.class_ids[name],
                            name: member,
                            arguments: self.arguments(depth)?,
                        }
                    } else {
                        ExprKind::EnumValue {
                            class_id: self.class_ids[name],
                            name: member,
                        }
                    }
                } else if self.peek() == Some(TokenKind::Symbol('(')) {
                    let arguments = self.arguments(depth)?;
                    // `identical` deixou de virar `==` no parse. Enquanto
                    // `operator ==` não existia, `==` sempre emitia `===` e a
                    // troca era exata; com o operador declarável, reescrever
                    // `identical` em `==` passaria a chamar o operador do
                    // usuário, que é justamente o que `identical` não faz. A
                    // chamada segue intacta e a análise semântica a trata como
                    // intrínseco de identidade.
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
            Some(TokenKind::Operator(symbol @ ("-" | "!" | "~"))) => {
                self.index += 1;
                if symbol == "-"
                    && matches!(self.peek(), Some(TokenKind::Number(n)) if n.parse::<u64>() == Ok(2147483648))
                {
                    self.index += 1;
                    ExprKind::Int(i32::MIN)
                } else {
                    let operand = self.primary(depth + 1)?;
                    ExprKind::Unary {
                        op: match symbol {
                            "-" => UnaryOp::Negate,
                            "~" => UnaryOp::BitNot,
                            _ => UnaryOp::Not,
                        },
                        operand: Box::new(operand),
                    }
                }
            }
            Some(TokenKind::Symbol('.')) => {
                // Dart 3.10: `.nome` só é válido onde o contexto determina o tipo.
                self.index += 1;
                // `.new` designa o construtor sem nome e é a única palavra
                // reservada aceita como membro de um atalho de ponto.
                let name = if self.take(TokenKind::Word("new")) {
                    "new"
                } else {
                    self.name()?
                };
                let arguments = if self.peek() == Some(TokenKind::Symbol('(')) {
                    Some(self.arguments(depth)?)
                } else {
                    None
                };
                if name == "new" && arguments.is_none() {
                    return Err(self.error("`.new` requires an argument list"));
                }
                ExprKind::DotShorthand { name, arguments }
            }
            Some(TokenKind::Symbol('[' | '{')) | Some(TokenKind::Operator("<")) => {
                self.list_literal(depth)?
            }
            Some(TokenKind::Symbol('(')) if self.starts_closure() => self.closure(depth)?,
            Some(TokenKind::Symbol('(')) => {
                let mut value = self.record_or_group(depth)?;
                value.span = Span {
                    start,
                    end: self.end(),
                };
                return self.postfix(value, depth);
            }
            Some(TokenKind::Operator(operator @ ("++" | "--"))) => {
                // `++x` produz o valor já atualizado. O alvo é lido fora de
                // `postfix` de propósito: em Dart `++x.y` é `++(x.y)`, e deixar
                // o resultado seguir para `postfix` produziria `(++x).y`.
                self.index += 1;
                let target = self.increment_target(operator)?;
                let span = Span {
                    start,
                    end: self.end(),
                };
                return Ok(Expr {
                    kind: ExprKind::Increment {
                        target: Box::new(target),
                        increase: operator == "++",
                        prefix: true,
                    },
                    span,
                });
            }
            Some(TokenKind::Word("super")) => {
                if self.tokens.get(self.index + 1).map(|token| token.kind)
                    == Some(TokenKind::Symbol('.'))
                {
                    return Err(self.error("super method calls are not supported yet"));
                }
                return Err(self.error("expected a supported expression"));
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
    /// Distingue agrupamento de expressão de records, preservando a ordem total dos campos.
    fn record_or_group(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Symbol('('))?;
        let mut fields = Vec::new();
        let mut comma = false;
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let name = if matches!(self.peek(), Some(TokenKind::Word(_)))
                && self.tokens.get(self.index + 1).map(|token| token.kind)
                    == Some(TokenKind::Symbol(':'))
            {
                let name = self.name()?;
                self.expect(TokenKind::Symbol(':'))?;
                if fields.iter().any(|(existing, _)| *existing == Some(name)) {
                    return Err(self.error("duplicate named record field"));
                }
                Some(name)
            } else {
                None
            };
            // O interior aceita `throw` para que `(throw e)` agrupe o
            // lançamento; `(throw e, 1)` é um record válido (Dart 3.6.2).
            fields.push((name, self.throw_or(depth + 1, false)?));
            comma = self.take(TokenKind::Symbol(','));
            if !comma {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        let span = Span {
            start,
            end: self.end(),
        };
        if fields.len() == 1 && fields[0].0.is_none() && !comma {
            let mut expression = fields.pop().unwrap().1;
            expression.span = span;
            Ok(expression)
        } else {
            Ok(Expr {
                kind: ExprKind::Record { fields },
                span,
            })
        }
    }
    /// Lê desestruturação rasa com nomes simples; '_' permanece sentinela de wildcard.
    fn record_destructure(&mut self, is_final: bool) -> Result<StatementKind<'a>, Diagnostic> {
        self.expect(TokenKind::Symbol('('))?;
        let mut positional = Vec::new();
        let mut named = Vec::new();
        let mut comma = false;
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let shorthand = self.take(TokenKind::Symbol(':'));
            let start = self.position();
            let name = self.name().map_err(|_| self.error("record destructuring supports only simple untyped names, without nested patterns"))?;
            let mut binding = name;
            let mut span = Span {
                start,
                end: self.end(),
            };
            if shorthand || self.take(TokenKind::Symbol(':')) {
                if !shorthand {
                    let start = self.position();
                    binding = self.name().map_err(|_| {
                        self.error("record destructuring supports only simple untyped bindings")
                    })?;
                    span = Span {
                        start,
                        end: self.end(),
                    };
                }
                if name == "_" {
                    return Err(self.error("record field name cannot be private"));
                }
                if named.iter().any(|(field, _, _)| *field == name) {
                    return Err(self.error("duplicate named record pattern field"));
                }
                named.push((name, binding, span));
            } else {
                positional.push((binding, span));
            }
            comma = self.take(TokenKind::Symbol(','));
            if !comma {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        if positional.len() == 1 && named.is_empty() && !comma {
            return Err(self.error("single-field record patterns require a trailing comma"));
        }
        self.expect(TokenKind::Operator("="))?;
        let initializer = self.expression()?;
        Ok(StatementKind::RecordDestructure {
            is_final,
            positional,
            named,
            initializer,
        })
    }
    /// Mantém o separador de braço fora de closures parentetizadas no nível da guarda.
    fn guard(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let previous = self.guard_start.replace(self.index);
        let result = self.cascade(depth);
        self.guard_start = previous;
        result
    }
    /// Lê uma lista tipada ou inferida sem absorver acessos pós-fixos no contexto const.
    fn list_literal(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
        let mut value_type = None;
        let element_type = if self.take(TokenKind::Operator("<")) {
            let ty = self.ty(false)?;
            if self.take(TokenKind::Symbol(',')) {
                value_type = Some(self.ty(false)?);
            }
            self.expect(TokenKind::Operator(">"))?;
            Some(ty)
        } else {
            None
        };
        if self.take(TokenKind::Symbol('{')) {
            return self.brace_literal(element_type, value_type, depth);
        }
        if value_type.is_some() {
            return Err(self.error("list literals require exactly one type argument"));
        }
        self.expect(TokenKind::Symbol('['))?;
        let mut elements = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(']')) {
            elements.push(self.collection_element(depth + 1, false)?);
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
    /// Distingue literal de mapa de literal de conjunto, como os SDKs 3.6.2/3.13.4.
    ///
    /// A decisão é sintática e nesta ordem: dois argumentos de tipo dizem mapa,
    /// um argumento diz conjunto, `{}` sem argumentos é o mapa vazio e, no
    /// restante, a presença de uma entrada `chave: valor` no nível de topo diz
    /// mapa. Um literal formado só por espalhamentos não é decidível sem os
    /// tipos estáticos dos operandos e é recusado com pedido de anotação.
    fn brace_literal(
        &mut self,
        element_type: Option<Type>,
        value_type: Option<Type>,
        depth: usize,
    ) -> Result<ExprKind<'a>, Diagnostic> {
        let declared_map = value_type.is_some();
        let declared_set = element_type.is_some() && value_type.is_none();
        let mut entries: Vec<(Expr<'a>, Option<Expr<'a>>)> = Vec::new();
        // Guarda apenas o veredito e um span por categoria: nada por elemento.
        let mut decision: Option<bool> = None;
        let mut map_span: Option<Span> = None;
        let mut set_span: Option<Span> = None;
        while self.peek() != Some(TokenKind::Symbol('}')) {
            let element = self.collection_element(depth + 1, declared_map)?;
            let control = matches!(
                element.kind,
                ExprKind::CollectionIf { .. } | ExprKind::CollectionFor { .. }
            );
            let verdict = if let ExprKind::MapEntry { key, value } = element.kind {
                let span = key.span;
                entries.push((*key, Some(*value)));
                map_span.get_or_insert(span);
                Some(true)
            } else if !declared_map && !declared_set && self.peek() == Some(TokenKind::Symbol(':'))
            {
                if control {
                    return Err(Diagnostic::new(
                        "if and for elements in map literals require explicit <K, V> type arguments",
                        element.span,
                    ));
                }
                self.index += 1;
                map_span.get_or_insert(element.span);
                let value = self.collection_element(depth + 1, false)?;
                entries.push((element, Some(value)));
                Some(true)
            } else {
                let verdict = decides_map(&element);
                if verdict == Some(true) {
                    map_span.get_or_insert(element.span);
                } else if verdict == Some(false) {
                    set_span.get_or_insert(element.span);
                }
                entries.push((element, None));
                verdict
            };
            if decision.is_none() {
                decision = verdict;
            }
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol('}'))?;
        let is_map =
            declared_map || (!declared_set && (decision == Some(true) || entries.is_empty()));
        if is_map {
            if let Some(span) = set_span {
                return Err(Diagnostic::new(
                    "map literals require `key: value` entries",
                    span,
                ));
            }
            return Ok(ExprKind::Map {
                key_type: element_type,
                value_type,
                entries,
            });
        }
        if let Some(span) = map_span {
            return Err(Diagnostic::new(
                "set literals do not accept `key: value` entries",
                span,
            ));
        }
        if !declared_set && decision.is_none() {
            return Err(self.error(
                "a literal built only from spreads needs explicit <T> or <K, V> type arguments",
            ));
        }
        Ok(ExprKind::Set {
            element_type,
            elements: entries.into_iter().map(|(element, _)| element).collect(),
        })
    }
    /// Lê um elemento de literal de coleção em todas as formas do subconjunto.
    ///
    /// Aceita `...`/`...?`, `if`/`if-else`, `for` clássico e `for-in`, o prefixo
    /// `?` do Dart 3.8 e uma expressão simples. Com `map`, o elemento simples
    /// precisa ser a entrada `chave: valor`, e os ramos de `if` e o corpo do
    /// `for` recebem o mesmo contexto — por isso `{if (c) 'a': 1}` é uma entrada
    /// condicional, e não uma entrada cuja chave é um `if`.
    ///
    /// Cada operando aparece uma única vez na árvore: nenhuma subexpressão é
    /// duplicada aqui, nem reanalisada para decidir a forma do literal.
    fn collection_element(&mut self, depth: usize, map: bool) -> Result<Expr<'a>, Diagnostic> {
        let start = self.position();
        if let Some(TokenKind::Operator(spread @ ("..." | "...?"))) = self.peek() {
            self.charge(depth)?;
            self.index += 1;
            let operand = self.cascade(depth + 1)?;
            return Ok(Expr {
                kind: ExprKind::Spread {
                    operand: Box::new(operand),
                    null_aware: spread == "...?",
                },
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        if self.peek() == Some(TokenKind::Word("if")) {
            self.charge(depth)?;
            self.index += 1;
            self.expect(TokenKind::Symbol('('))?;
            let condition = self.expression()?;
            self.expect(TokenKind::Symbol(')'))?;
            let then_element = self.collection_element(depth + 1, map)?;
            // `else` liga-se sempre ao `if` mais interno, como em Dart.
            let else_element = if self.take(TokenKind::Word("else")) {
                Some(Box::new(self.collection_element(depth + 1, map)?))
            } else {
                None
            };
            return Ok(Expr {
                kind: ExprKind::CollectionIf {
                    condition: Box::new(condition),
                    then_element: Box::new(then_element),
                    else_element,
                },
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        if self.peek() == Some(TokenKind::Word("for")) {
            self.charge(depth)?;
            let header = self.collection_for_header()?;
            let element = self.collection_element(depth + 1, map)?;
            return Ok(Expr {
                kind: ExprKind::CollectionFor {
                    header: Box::new(header),
                    element: Box::new(element),
                },
                span: Span {
                    start,
                    end: self.end(),
                },
            });
        }
        let value = self.null_aware_element(depth)?;
        if !map {
            return Ok(value);
        }
        self.expect(TokenKind::Symbol(':'))?;
        let entry = self.null_aware_element(depth)?;
        Ok(Expr {
            kind: ExprKind::MapEntry {
                key: Box::new(value),
                value: Box::new(entry),
            },
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê o prefixo `?` do Dart 3.8, que omite o elemento quando avalia para null.
    ///
    /// O prefixo não se aninha: `??valor` é rejeitado explicitamente.
    fn null_aware_element(&mut self, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        let start = self.position();
        if !self.take(TokenKind::Operator("?")) {
            return self.cascade(depth);
        }
        if self.peek() == Some(TokenKind::Operator("?")) {
            return Err(self.error("nested null-aware collection elements are not supported"));
        }
        let value = self.cascade(depth)?;
        Ok(Expr {
            kind: ExprKind::NullAwareElement(Box::new(value)),
            span: Span {
                start,
                end: self.end(),
            },
        })
    }
    /// Lê o cabeçalho de um `for` de literal, devolvendo-o com corpo vazio.
    ///
    /// Reaproveita as mesmas formas das instruções `for` e `for-in`, para que a
    /// análise e a emissão não dupliquem regras de escopo nem de iteração.
    /// `await for` e as cláusulas separadas por vírgula ficam fora do subconjunto.
    fn collection_for_header(&mut self) -> Result<Statement<'a>, Diagnostic> {
        let start = self.position();
        self.expect(TokenKind::Word("for"))?;
        if self.peek() == Some(TokenKind::Word("await")) {
            return Err(self.error("await for in collection literals is not supported"));
        }
        self.expect(TokenKind::Symbol('('))?;
        let kind = if self.for_in_ahead() {
            if self.peek() == Some(TokenKind::Word("const")) {
                return Err(self.error("const is not allowed in a for-in variable"));
            }
            let is_final = self.take(TokenKind::Word("final"));
            let inferred = self.take(TokenKind::Word("var"));
            if is_final && inferred {
                return Err(self.error("final var is not supported; use final name in expression"));
            }
            let annotation = if !inferred && !self.at_for_in_name() {
                Some(self.ty(false)?)
            } else {
                None
            };
            if !is_final && !inferred && annotation.is_none() {
                return Err(self.error(
                    "for-in requires final, var or a type; assigning to an existing variable is not supported",
                ));
            }
            let name = self.name()?;
            self.expect(TokenKind::Word("in"))?;
            let iterable = self.expression()?;
            self.expect(TokenKind::Symbol(')'))?;
            StatementKind::ForIn {
                is_final,
                name,
                annotation,
                iterable,
                body: Vec::new(),
            }
        } else {
            let initializer = if self.peek() == Some(TokenKind::Symbol(';')) {
                None
            } else {
                let initializer = self.simple(true)?;
                if matches!(initializer.kind, StatementKind::RecordDestructure { .. }) {
                    return Err(Diagnostic::new(
                        "record destructuring in for initializers is not supported yet",
                        initializer.span,
                    ));
                }
                Some(Box::new(initializer))
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
            StatementKind::For {
                initializer,
                condition,
                update,
                body: Vec::new(),
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
    /// Analisa construtores Future.value/delayed com tipo explícito opcional.
    fn future_expression(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
        self.expect(TokenKind::Word("Future"))?;
        let value_type = if self.take(TokenKind::Operator("<")) {
            let ty = self.ty(true)?;
            self.expect(TokenKind::Operator(">"))?;
            Some(ty)
        } else {
            None
        };
        self.expect(TokenKind::Symbol('.'))?;
        let name = self.name()?;
        if !matches!(name, "value" | "delayed") {
            return Err(self.error("only Future.value and Future.delayed are supported"));
        }
        let arguments = self.arguments(depth)?;
        if name == "value" {
            if arguments.len() > 1 {
                return Err(self.error("Future.value accepts at most one argument"));
            }
            Ok(ExprKind::FutureValue {
                value: arguments.into_iter().next().map(Box::new),
                value_type,
            })
        } else {
            if !(1..=2).contains(&arguments.len()) {
                return Err(self.error("Future.delayed requires duration and optional computation"));
            }
            let mut arguments = arguments.into_iter();
            Ok(ExprKind::FutureDelayed {
                duration: Box::new(arguments.next().unwrap()),
                computation: arguments.next().map(Box::new),
                value_type,
            })
        }
    }
    /// Preserva a ordem das unidades nomeadas de Duration sem avaliar expressões no parser.
    fn duration_expression(&mut self, depth: usize) -> Result<ExprKind<'a>, Diagnostic> {
        self.expect(TokenKind::Word("Duration"))?;
        if self.take(TokenKind::Symbol('.')) {
            self.expect(TokenKind::Word("zero"))?;
            return Ok(ExprKind::Duration { parts: Vec::new() });
        }
        self.expect(TokenKind::Symbol('('))?;
        let mut parts = Vec::new();
        while self.peek() != Some(TokenKind::Symbol(')')) {
            let name = self.name()?;
            let unit = match name {
                "days" => DurationUnit::Days,
                "hours" => DurationUnit::Hours,
                "minutes" => DurationUnit::Minutes,
                "seconds" => DurationUnit::Seconds,
                "milliseconds" => DurationUnit::Milliseconds,
                "microseconds" => DurationUnit::Microseconds,
                _ => return Err(self.error("unsupported Duration unit")),
            };
            if parts.iter().any(|(existing, _)| *existing == unit) {
                return Err(self.error("duplicate Duration unit"));
            }
            self.expect(TokenKind::Symbol(':'))?;
            parts.push((unit, self.cascade(depth + 1)?));
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        Ok(ExprKind::Duration { parts })
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
            | ExprKind::Double(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::OwnedString(_)
            | ExprKind::Identifier(_)
            | ExprKind::EnumValue { .. } => true,
            ExprKind::Unary {
                op: UnaryOp::Negate,
                operand,
            } => matches!(operand.kind, ExprKind::Int(_) | ExprKind::Double(_)),
            _ => false,
        };
        if !supported {
            return Err(self.error(
                "only literal, named constant, enum and typed binding patterns are supported",
            ));
        }
        Ok(Pattern::Constant(value))
    }
    /// Descarta `<A, B>` após um tipo classe (erasure); devolve se consumiu.
    ///
    /// A tentativa é restaurada quando `<` não abre argumentos válidos, para
    /// não confundir outros usos do operador. Tipos internos são validados.
    fn discard_type_arguments(&mut self) -> bool {
        if self.peek() != Some(TokenKind::Operator("<")) {
            return false;
        }
        let index = self.index;
        let count = self.types.len();
        let mut ok = false;
        if self.take(TokenKind::Operator("<")) {
            ok = self.ty(false).is_ok();
            while ok && self.take(TokenKind::Symbol(',')) {
                ok = self.ty(false).is_ok();
            }
            ok = ok && self.take(TokenKind::Operator(">"));
        }
        if !ok {
            self.index = index;
            self.types.truncate(count);
            return false;
        }
        true
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
                            Some(
                                TokenKind::Operator("=>")
                                    | TokenKind::Symbol('{')
                                    | TokenKind::Word("async")
                            )
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
            // Closures permanecem limitadas a posicionais obrigatórios; a inferência
            // contextual de tipos de função ainda não modela grupos opcionais.
            if matches!(
                self.peek(),
                Some(TokenKind::Symbol('[' | '{') | TokenKind::Word("required"))
            ) {
                return Err(self.error("closures support only required positional parameters"));
            }
            let start = self.position();
            let ty = if self.starts_annotation() {
                self.ty(false)?
            } else {
                Type::Inferred
            };
            let name = self.name()?;
            if self.peek() == Some(TokenKind::Operator("=")) {
                return Err(self.error("closures support only required positional parameters"));
            }
            parameters.push(Parameter::required(
                name,
                ty,
                Span {
                    start,
                    end: self.end(),
                },
            ));
            if !self.take(TokenKind::Symbol(',')) {
                break;
            }
        }
        self.expect(TokenKind::Symbol(')'))?;
        let is_async = self.take(TokenKind::Word("async"));
        let is_arrow = self.take(TokenKind::Operator("=>"));
        let body = if is_arrow {
            let value = self.cascade(depth + 1)?;
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
            is_async,
            is_arrow,
            parameters,
            return_type: Type::Inferred,
            body,
        })
    }
    /// Aplica asserções pós-fixas sem permitir que contornem o limite de nós.
    /// Lê o alvo de um `++`/`--` prefixo, que precisa ser um nome simples.
    ///
    /// # Erros
    /// Recusa alvo composto (`a[i]`, `o.campo`) com a razão e a alternativa:
    /// atualizar um alvo composto exige guardar receptor e índice em
    /// temporários para avaliá-los uma única vez, e este subconjunto ainda não
    /// tem essa forma.
    fn increment_target(&mut self, operator: &str) -> Result<Expr<'a>, Diagnostic> {
        let start = self.position();
        let Some(TokenKind::Word(name)) = self.peek() else {
            return Err(self.error("increment and decrement require a simple variable target"));
        };
        if reserved(name) {
            return Err(self.error("increment and decrement require a simple variable target"));
        }
        self.index += 1;
        let span = Span {
            start,
            end: self.end(),
        };
        if matches!(
            self.peek(),
            Some(TokenKind::Symbol('.' | '[') | TokenKind::Operator("?."))
        ) {
            return Err(Diagnostic::new(
                format!(
                    "`{operator}` accepts only a simple variable as target: updating `a[i]` or `o.field` has to evaluate receiver and index exactly once, which needs temporaries this subset does not emit yet; write `a[i] = a[i] {} 1` as a statement instead",
                    if operator == "++" { "+" } else { "-" }
                ),
                span,
            ));
        }
        Ok(Expr {
            kind: ExprKind::Identifier(name),
            span,
        })
    }
    /// Converte `alvo++`/`alvo--` lido depois de uma primária em `Increment`.
    ///
    /// # Erros
    /// Recusa alvo que não seja um nome simples, pela mesma razão documentada
    /// em [`Parser::increment_target`].
    fn increment_suffix(
        &mut self,
        value: Expr<'a>,
        operator: &'a str,
        depth: usize,
    ) -> Result<Expr<'a>, Diagnostic> {
        if !matches!(value.kind, ExprKind::Identifier(_)) {
            return Err(Diagnostic::new(
                format!(
                    "`{operator}` accepts only a simple variable as target: updating `a[i]` or `o.field` has to evaluate receiver and index exactly once, which needs temporaries this subset does not emit yet; write `a[i] = a[i] {} 1` as a statement instead",
                    if operator == "++" { "+" } else { "-" }
                ),
                value.span,
            ));
        }
        self.charge(depth)?;
        self.index += 1;
        let span = Span {
            start: value.span.start,
            end: self.end(),
        };
        Ok(Expr {
            kind: ExprKind::Increment {
                target: Box::new(value),
                increase: operator == "++",
                prefix: false,
            },
            span,
        })
    }
    fn postfix(&mut self, mut value: Expr<'a>, depth: usize) -> Result<Expr<'a>, Diagnostic> {
        // Cada seletor pendura o valor anterior como receptor: a cadeia é lida
        // num laço, mas a árvore que ela produz fica um nível mais funda a cada
        // volta. `level` é essa profundidade real, e é a que o orçamento vê.
        let mut level = depth;
        loop {
            if let Some(selector) = self.null_aware_selector() {
                // A cadeia inteira curto-circuita: `a?.b.c` só avalia `.c`
                // quando `a` não é null. O receptor sai da posição de operando
                // e o restante dos seletores passa a pender do alvo sintético,
                // que a emissão liga a um único temporário.
                level += 1;
                self.charge(level)?;
                let operator = self.tokens[self.index].span;
                self.index += 1;
                let target = Expr {
                    kind: ExprKind::NullShortTarget,
                    span: operator,
                };
                let selected = match selector {
                    NullAwareSelector::Member => {
                        self.expect(TokenKind::Symbol('.'))?;
                        let name = self.name()?;
                        let kind = if self.peek() == Some(TokenKind::Symbol('(')) {
                            ExprKind::MethodCall {
                                receiver: Box::new(target),
                                name,
                                arguments: self.arguments(level)?,
                            }
                        } else {
                            ExprKind::Member {
                                receiver: Box::new(target),
                                name,
                            }
                        };
                        Expr {
                            kind,
                            span: Span {
                                start: operator.start,
                                end: self.end(),
                            },
                        }
                    }
                    NullAwareSelector::Index => {
                        self.expect(TokenKind::Symbol('['))?;
                        let index = self.cascade(level + 1)?;
                        self.expect(TokenKind::Symbol(']'))?;
                        Expr {
                            kind: ExprKind::Index {
                                receiver: Box::new(target),
                                index: Box::new(index),
                            },
                            span: Span {
                                start: operator.start,
                                end: self.end(),
                            },
                        }
                    }
                };
                let chain = self.postfix(selected, level)?;
                let span = Span {
                    start: value.span.start,
                    end: self.end(),
                };
                return Ok(Expr {
                    kind: ExprKind::NullShort {
                        receiver: Box::new(value),
                        chain: Box::new(chain),
                        target: operator,
                    },
                    span,
                });
            }
            let kind = if self.peek() == Some(TokenKind::Symbol('(')) {
                level += 1;
                self.charge(level)?;
                ExprKind::Invoke {
                    callee: Box::new(value),
                    arguments: self.arguments(level)?,
                }
            } else if self.take(TokenKind::Symbol('[')) {
                level += 1;
                self.charge(level)?;
                let index = self.cascade(level + 1)?;
                self.expect(TokenKind::Symbol(']'))?;
                ExprKind::Index {
                    receiver: Box::new(value),
                    index: Box::new(index),
                }
            } else if self.take(TokenKind::Operator("!")) {
                level += 1;
                self.charge(level)?;
                ExprKind::Unary {
                    op: UnaryOp::NullAssert,
                    operand: Box::new(value),
                }
            } else if self.take(TokenKind::Symbol('.')) {
                level += 1;
                self.charge(level)?;
                let name = self.name()?;
                if self.peek() == Some(TokenKind::Symbol('(')) {
                    let arguments = self.arguments(level)?;
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

/// Forma do seletor que abre uma cadeia null-aware.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NullAwareSelector {
    /// `?.nome` e `?.nome(args)`.
    Member,
    /// `?[índice]`.
    Index,
}

/// Decide pela sintaxe se um elemento força mapa, força conjunto ou nada diz.
///
/// `Some(true)` é uma entrada `chave: valor`, `Some(false)` é um valor solto e
/// `None` é um espalhamento, cuja forma depende do tipo estático do operando.
/// Os ramos de `if` e o corpo de `for` são inspecionados na ordem escrita, que
/// é a mesma ordem em que Dart procura o primeiro elemento decisivo.
fn decides_map(element: &Expr<'_>) -> Option<bool> {
    match &element.kind {
        ExprKind::MapEntry { .. } => Some(true),
        ExprKind::Spread { .. } => None,
        ExprKind::CollectionIf {
            then_element,
            else_element,
            ..
        } => decides_map(then_element).or_else(|| else_element.as_deref().and_then(decides_map)),
        ExprKind::CollectionFor { element, .. } => decides_map(element),
        _ => Some(false),
    }
}

/// Valida nomes de campos, inclusive rótulos posicionais que não integram o tipo estrutural.
fn invalid_record_field_name(name: &str, positional: usize) -> bool {
    name.starts_with('_')
        || matches!(
            name,
            "hashCode" | "runtimeType" | "toString" | "noSuchMethod"
        )
        || name.strip_prefix('$').is_some_and(|index| {
            !index.starts_with('0')
                && index
                    .parse::<usize>()
                    .is_ok_and(|index| index > 0 && index <= positional)
        })
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
/// Anotações que documentam intenção sem alterar o programa emitido.
///
/// São os metadados de `package:meta` e equivalentes: o compilador e o analisador
/// as usam para avisar o programador, e nenhuma delas muda a semântica nem a
/// geração de código. Aceitá-las é o que permite compilar código Dart de
/// produção, onde elas aparecem em dois terços dos arquivos.
///
/// A lista é deliberadamente fechada. Tolerar **qualquer** anotação desconhecida
/// esconderia o caso oposto — uma anotação que o compilador precisa honrar,
/// como `@pragma`, que continua com diagnóstico próprio. Aceitar em silêncio
/// algo que dirige o compilador é pior do que rejeitar.
fn ignorable_metadata(name: &str) -> bool {
    matches!(
        name,
        "immutable"
            | "protected"
            | "mustCallSuper"
            | "visibleForTesting"
            | "visibleForOverriding"
            | "experimental"
            | "internal"
            | "nonVirtual"
            | "useResult"
            | "doNotStore"
            | "doNotSubmit"
            | "alwaysThrows"
            | "literal"
            | "optionalTypeArgs"
            | "awaitNotRequired"
            | "redeclare"
            | "reopen"
            | "widgetFactory"
            | "pragma"
            | "factory"
            | "sealed"
            | "required"
            | "isTest"
            | "isTestGroup"
            | "GenerateMocks"
            | "RecordUse"
            | "Target"
            | "Category"
    )
}

/// Parâmetro da lista primária antes de virar campo e parâmetro de construtor.
struct PrimaryParameter<'a> {
    name: &'a str,
    /// Tipo escrito na lista; `None` indica a forma `this.nome`.
    declared: Option<Type>,
    is_final: bool,
    span: Span,
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
/// Distingue um cabeçalho nominal de uma variável de topo `final` ou `const`.
///
/// `final class C {}` é declaração nominal; `final int x = 1;` é variável. Os
/// dois começam pela mesma palavra, então só a palavra seguinte decide.
fn starts_nominal(tokens: &[Token<'_>], start: usize) -> bool {
    let mut index = start;
    while matches!(
        tokens.get(index).map(|token| token.kind),
        Some(TokenKind::Word(
            "abstract" | "base" | "final" | "sealed" | "interface"
        ))
    ) {
        index += 1;
    }
    matches!(
        tokens.get(index).map(|token| token.kind),
        Some(TokenKind::Word("class" | "enum" | "mixin"))
    )
}

/// Decide entre variável de topo e função pelo primeiro delimitador da declaração.
///
/// Uma assinatura de função sempre abre parênteses antes de `=` ou `;`; uma
/// variável sempre alcança `=` ou `;` antes de qualquer parêntese. Um tipo de
/// função escrito na anotação (`int Function(int) f = ...`) cai no lado errado
/// dessa decisão e é recusado como assinatura inválida, o que documenta o limite.
fn starts_global_variable(tokens: &[Token<'_>], start: usize) -> bool {
    for token in &tokens[start.min(tokens.len())..] {
        match token.kind {
            TokenKind::Symbol('(' | '{') | TokenKind::Operator("=>") => return false,
            TokenKind::Operator("=") | TokenKind::Symbol(';') => return true,
            _ => {}
        }
    }
    false
}

/// Monta a declaração sintética que carrega as variáveis de topo da unidade.
///
/// O nome começa por `#`, que nenhum identificador Dart aceita, então a
/// declaração nunca colide com uma classe do usuário nem vira um tipo.
fn globals_class<'a>(id: u32, static_fields: Vec<StaticField<'a>>, span: Span) -> Class<'a> {
    Class {
        mixin_constraint: None,
        factories: vec![],
        constructor: None,
        constructor_extras: None,
        named_constructors: vec![],
        static_fields,
        static_methods: vec![],
        is_library_globals: true,
        annotations: vec![],
        is_mixin_application: false,
        mixin_origin: None,
        modifier: ClassModifier::None,
        kind: ClassKind::Class,
        mixins: vec![],
        enum_arguments: vec![],
        enum_constructor_fields: vec![],
        is_interface: false,
        library_id: 0,
        is_abstract: true,
        interfaces: vec![],
        abstract_methods: vec![],
        enum_values: vec![],
        id,
        name: "#globals",
        superclass: None,
        fields: vec![],
        methods: vec![],
        type_parameters: vec![],
        span,
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
/// Acrescenta um trecho literal, descartando vazios e juntando o anterior.
///
/// A junção mantém a lista com no máximo um literal entre duas expressões, o que
/// também faz `'a' 'b'` voltar a ser um literal simples sem passar pelo emissor.
fn push_literal<'a>(parts: &mut Vec<StringPart<'a>>, text: std::borrow::Cow<'a, str>) {
    if text.is_empty() {
        return;
    }
    match parts.last_mut() {
        Some(StringPart::Owned(previous)) => {
            previous.push_str(&text);
            return;
        }
        Some(part @ StringPart::Borrowed(_)) => {
            let StringPart::Borrowed(previous) = *part else {
                unreachable!("braço filtrado pelo padrão")
            };
            let mut merged = String::with_capacity(previous.len() + text.len());
            merged.push_str(previous);
            merged.push_str(&text);
            *part = StringPart::Owned(merged);
            return;
        }
        _ => {}
    }
    parts.push(match text {
        std::borrow::Cow::Borrowed(text) => StringPart::Borrowed(text),
        std::borrow::Cow::Owned(text) => StringPart::Owned(text),
    });
}

/// Decide se um trecho literal precisa de alocação antes de virar valor.
///
/// Um trecho sem escapes e sem CR continua emprestado do texto original; só os
/// demais pagam a cópia. Strings raw preservam as barras invertidas, mas ainda
/// normalizam o fim de linha quando são de aspas triplas.
fn decode_chunk(
    text: &str,
    multiline: bool,
    raw: bool,
    span: Span,
) -> Result<std::borrow::Cow<'_, str>, Diagnostic> {
    let carriage = multiline && text.contains('\r');
    if raw {
        return Ok(if carriage {
            std::borrow::Cow::Owned(normalize_line_endings(text))
        } else {
            std::borrow::Cow::Borrowed(text)
        });
    }
    if !carriage && !text.contains('\\') {
        return Ok(std::borrow::Cow::Borrowed(text));
    }
    Ok(std::borrow::Cow::Owned(decode_string(
        text, multiline, span,
    )?))
}

/// Converte CRLF e CR isolado em LF, como o lexer do Dart faz em strings triplas.
fn normalize_line_endings(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find('\r') {
        result.push_str(&rest[..index]);
        result.push('\n');
        rest = &rest[index + 1..];
        if let Some(tail) = rest.strip_prefix('\n') {
            rest = tail;
        }
    }
    result.push_str(rest);
    result
}

/// Decodifica escapes Dart em unidades UTF-16 e rejeita surrogates isolados.
///
/// `multiline` liga a normalização de fim de linha das strings de aspas triplas;
/// ela vale para o CR escrito na fonte, nunca para o produzido por `\r`.
fn decode_string(text: &str, multiline: bool, span: Span) -> Result<String, Diagnostic> {
    let mut chars = text.chars();
    let mut units = Vec::with_capacity(text.len());
    while let Some(mut ch) = chars.next() {
        // CR e CRLF escritos na fonte viram LF; o escape \r segue adiante intacto.
        if multiline && ch == '\r' {
            if chars.clone().next() == Some('\n') {
                chars.next();
            }
            units.push(u16::from(b'\n'));
            continue;
        }
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
        // Precedências 5 a 8 seguem a especificação Dart: `|` liga mais frouxo
        // que `^`, que liga mais frouxo que `&`, que liga mais frouxo que os
        // deslocamentos. Todos ligam mais frouxo que `+`/`-`.
        "|" => (5, BinaryOp::BitOr),
        "^" => (6, BinaryOp::BitXor),
        "&" => (7, BinaryOp::BitAnd),
        "<<" => (8, BinaryOp::ShiftLeft),
        "+" => (9, BinaryOp::Add),
        "-" => (9, BinaryOp::Subtract),
        "*" => (10, BinaryOp::Multiply),
        "/" => (10, BinaryOp::Divide),
        "~/" => (10, BinaryOp::TruncDivide),
        "%" => (10, BinaryOp::Remainder),
        _ => return None,
    })
}

/// Recusa `late` com inicializador, cuja célula preguiçosa não é emitida.
///
/// Em Dart, `late T x = init` não avalia `init` na declaração: o inicializador
/// roda na **primeira leitura**, e uma escrita anterior à primeira leitura o
/// cancela sem jamais executá-lo. Emitir a avaliação na declaração daria a
/// ordem de efeitos errada em silêncio, então a forma é recusada. O span cobre
/// a palavra `late`, que é o que torna a declaração preguiçosa.
fn late_initializer_error(start: usize) -> Diagnostic {
    Diagnostic::new(
        "late with an initializer is not supported: in Dart the initializer runs on the first read and a write before that read cancels it; declare `late T name;` and assign before reading",
        Span {
            start,
            end: start + "late".len(),
        },
    )
}

// Restringe palavras reservadas e nomes especiais no subconjunto inicial.
/// Identifica nomes indisponíveis como identificadores neste subconjunto.
fn reserved(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "Map"
            | "Future"
            | "Duration"
            | "List"
            | "Iterable"
            | "Object"
            | "Null"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "augment"
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
            | "double"
            | "num"
            | "String"
            | "bool"
            | "print"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Uma classe Timer declarada pelo usuário prevalece sobre o tipo intrínseco.
    #[test]
    fn user_timer_class_preserves_nominal_type() {
        let program = parsed("class Timer{}Timer make()=>Timer();void main(){Timer timer=make();}");
        assert_eq!(
            program.functions[0].return_type,
            Type::Class(program.classes[0].id)
        );
        let StatementKind::Variable { annotation, .. } = &program.statements[0].kind else {
            panic!()
        };
        assert_eq!(*annotation, Some(Type::Class(program.classes[0].id)));
    }
    /// Preserva async sem await e distingue descarte arrow de retorno explícito em bloco.
    #[test]
    fn async_functions_closures_and_main_preserve_flags() {
        let source = "Future<int> value() async=>1;Future<void> discard() async=>2;class C{Future<int> method() async{return await value();}}Future<void> main() async{var f=() async=>await Future<int>.value(3);var g=() async{return 4;};await Future<void>.delayed(Duration.zero);}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 3);
        let program = parse(&tokens, source.len()).unwrap();
        assert!(program.main_is_async);
        assert!(program.functions.iter().all(|f| f.is_async));
        assert!(program.classes[0].methods[0].is_async);
        assert!(program.functions[1].is_arrow);
        assert!(matches!(
            program.functions[1].body[0].kind,
            StatementKind::Return(Some(_))
        ));
        let StatementKind::Variable { initializer, .. } = &program.statements[0].kind else {
            panic!()
        };
        assert!(matches!(
            initializer.kind,
            ExprKind::Closure {
                is_async: true,
                is_arrow: true,
                ..
            }
        ));
        assert!(matches!(
            program.statements[2].kind,
            StatementKind::Expression(Expr {
                kind: ExprKind::Await(_),
                ..
            })
        ));
        assert!(parsed("void main() async{}").main_is_async);
        let arrow_main = parsed("Future<void> main() async=>Future<int>.value(1);");
        assert!(arrow_main.main_is_arrow && arrow_main.main_is_async);
        assert!(matches!(
            arrow_main.statements[0].kind,
            StatementKind::Return(Some(_))
        ));
        assert!(!parsed("void main(){}").main_is_async);
    }
    /// Construtores assíncronos retêm tipos opcionais e unidades na ordem da fonte.
    #[test]
    fn future_and_duration_intrinsic_arguments() {
        let program = parsed(
            "void main(){var duration=Duration(seconds:f(),days:g(),microseconds:3);var future=Future<int>.delayed(duration,() async=>2);Future.value();Timer(Duration.zero,()=>print(1));scheduleMicrotask(()=>print(2));}",
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[0].kind else {
            panic!()
        };
        let ExprKind::Duration { parts } = &initializer.kind else {
            panic!()
        };
        assert_eq!(
            parts.iter().map(|(unit, _)| *unit).collect::<Vec<_>>(),
            [
                DurationUnit::Seconds,
                DurationUnit::Days,
                DurationUnit::Microseconds
            ]
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[1].kind else {
            panic!()
        };
        assert!(matches!(
            initializer.kind,
            ExprKind::FutureDelayed {
                value_type: Some(Type::Int),
                computation: Some(_),
                ..
            }
        ));
        for body in [
            "var x=Duration(seconds:1,seconds:2);",
            "var x=Duration(1);",
            "var x=const Duration(seconds:1);",
            "var x=Future<int>.value(1,2);",
            "var x=Future.delayed();",
            "var x=Future.error(1);",
            "var f=() async*{};",
        ] {
            rejected(body);
        }
        for source in [
            "Future<void> main()=>Future<void>.value();",
            "Future<int> main() async=>1;",
            "Future<int> f() async;void main(){}",
        ] {
            assert!(parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err());
        }
    }
    /// Mapas e fábricas preservam tipos, argumentos e metadados para expansão posterior.
    #[test]
    fn maps_named_factories_and_json_metadata() {
        let source = "@JsonCodable() class User{final String name;User(this.name);factory User.fromJson(Map<String,Object?> json)=>User(json['name'] as String);} void main(){Map<String,Object?> json=<String,Object?>{'name':'Ana','age':1};var empty={};var user=User.fromJson(json);var inferred={'name':'Bia'};}";
        let program = parsed(source);
        assert!(matches!(
            program.classes[0].annotations[0].kind,
            AnnotationKind::JsonCodable
        ));
        let factory = &program.classes[0].factories[0];
        assert_eq!(factory.name, "fromJson");
        assert_eq!(factory.return_type, Type::Class(program.classes[0].id));
        let Type::Applied(id) = factory.parameters[0].ty else {
            panic!()
        };
        assert_eq!(
            program.types[id as usize],
            TypeShape::Map {
                key: Type::String,
                value: Type::NullableObject
            }
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[0].kind else {
            panic!()
        };
        assert!(
            matches!(&initializer.kind, ExprKind::Map { key_type: Some(Type::String), value_type: Some(Type::NullableObject), entries } if entries.len()==2)
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[2].kind else {
            panic!()
        };
        assert!(matches!(
            initializer.kind,
            ExprKind::NamedConstruct {
                name: "fromJson",
                ..
            }
        ));
    }
    /// A infraestrutura não aceita sets, fábricas redirecionadas ou macros em outros alvos.
    #[test]
    fn unsupported_map_factory_and_macro_forms() {
        for body in ["var x=<String,int>[];", "var x=Map<String,int,bool>();"] {
            rejected(body);
        }
        for source in [
            "@JsonCodable() enum E{a}void main(){}",
            "@JsonCodable() mixin M{}void main(){}",
            "class C{@JsonCodable() void f(){}}void main(){}",
            "class JsonCodable{} @JsonCodable() class C{}void main(){}",
            "@JsonCodable(1) class C{}void main(){}",
        ] {
            assert!(
                parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
        // Fábrica com corpo em bloco é válida: equipara-se às demais funções.
        let bloco = "class C{factory C.a(){return C();}}void main(){}";
        let programa = parse(&dartforge_lexer::lex(bloco).unwrap(), bloco.len()).unwrap();
        let fabrica = &programa.classes[0].factories[0];
        assert_eq!(fabrica.name, "a");
        assert!(!fabrica.is_arrow);
    }
    /// Cascatas preservam precedência, receptor sintético e limites entre seções.
    #[test]
    fn cascades_preserve_sections_and_receiver_spans() {
        let source = "void main(){var x=a ?? b?..value=1+2..child.run()..[0]=3; a..child=(b..value=4)..run(); print([a..run()]);}";
        let program = parsed(source);
        let StatementKind::Variable { initializer, .. } = &program.statements[0].kind else {
            panic!()
        };
        let ExprKind::Cascade {
            receiver,
            null_aware,
            sections,
        } = &initializer.kind
        else {
            panic!()
        };
        assert!(*null_aware);
        assert!(matches!(
            receiver.kind,
            ExprKind::Binary {
                op: BinaryOp::IfNull,
                ..
            }
        ));
        assert_eq!(sections.len(), 3);
        let StatementKind::FieldAssign {
            receiver, value, ..
        } = &sections[0].kind
        else {
            panic!()
        };
        assert!(matches!(receiver.kind, ExprKind::CascadeReceiver));
        assert_eq!(&source[receiver.span.start..receiver.span.end], "?..");
        assert!(matches!(
            value.kind,
            ExprKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
        assert!(matches!(
            sections[2].kind,
            StatementKind::IndexAssign { .. }
        ));
        let StatementKind::Expression(Expr {
            kind: ExprKind::Cascade { sections, .. },
            ..
        }) = &program.statements[1].kind
        else {
            panic!()
        };
        assert_eq!(sections.len(), 2);
        assert!(matches!(
            sections[0].kind,
            StatementKind::FieldAssign {
                value: Expr {
                    kind: ExprKind::Cascade { .. },
                    ..
                },
                ..
            }
        ));
    }
    /// Sintaxe parcial e árvores extensas não podem contornar os limites do parser.
    #[test]
    fn cascades_reject_unsupported_updates_and_incomplete_sections() {
        for body in [
            "a..;",
            "a..run()?..run();",
            "a?..run()?..run();",
            "a..x++;",
            "a..x+=1;",
            "a..run()=1;",
            "a..[0]=;",
            "a..x=1=2;",
        ] {
            rejected(body);
        }
        rejected(&format!("a{};", "..run()".repeat(1000)));
    }
    /// Records preservam avaliação mista e tipos equivalentes ordenam apenas nomes.
    #[test]
    fn record_literals_types_and_destructuring() {
        let source = "(int,{String z,int a}) first()=> (1,z:'x',a:2); (int,{int a,String z}) second()=> (a:2,1,z:'x'); (T,{T value}) pair<T>(T value)=>(value,value:value); void main(){var empty=();var single=(1,);var grouped=(1);var mixed=(1,label:2,3);var (x,:label,y)=mixed;final (a,)=single;var (_,_)=(1,2);print(mixed.$1);}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 4);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(
            program.functions[0].return_type,
            program.functions[1].return_type
        );
        let Type::Applied(id) = program.functions[0].return_type else {
            panic!()
        };
        let TypeShape::Record { named, .. } = &program.types[id as usize] else {
            panic!()
        };
        assert_eq!(
            named
                .iter()
                .map(|field| field.0.as_str())
                .collect::<Vec<_>>(),
            ["a", "z"]
        );
        let StatementKind::Variable { initializer, .. } = &program.statements[3].kind else {
            panic!()
        };
        let ExprKind::Record { fields } = &initializer.kind else {
            panic!()
        };
        assert_eq!(
            fields.iter().map(|field| field.0).collect::<Vec<_>>(),
            [None, Some("label"), None]
        );
        let StatementKind::RecordDestructure {
            positional,
            named,
            is_final,
            ..
        } = &program.statements[4].kind
        else {
            panic!()
        };
        assert!(!is_final);
        assert_eq!(
            positional.iter().map(|field| field.0).collect::<Vec<_>>(),
            ["x", "y"]
        );
        assert_eq!((named[0].0, named[0].1), ("label", "label"));
        assert_eq!(&source[named[0].2.start..named[0].2.end], "label");
        assert!(matches!(
            program.statements[5].kind,
            StatementKind::RecordDestructure { is_final: true, .. }
        ));
        assert!(matches!(
            &program.statements[7].kind,
            StatementKind::Print(Expr {
                kind: ExprKind::Member { name: "$1", .. },
                ..
            })
        ));
        parsed("void main(){({int x}) r=(x:1);var (x:y)=r;print(y);}");
    }
    /// Desestruturação aninhada/tipada, nomes inválidos e macros recebem diagnóstico.
    #[test]
    fn unsupported_record_patterns_and_invalid_record_types() {
        for body in [
            "for(var (x,y)=(1,2);x<3;x++){}",
            "for(final (x,y)=(1,2);true;){}",
            "var ((x,y),z)=((1,2),3);",
            "var (int x,y)=(1,2);",
            "var (x)=1;",
            "const (x,y)=(1,2);",
            "var (:_)=(x:1);",
            "var (a:x,a:y)=(a:1);",
            "var value=(x:1,x:2);",
            "(int) value=(1,);",
            "({}) value=();",
            "(int x,int x) value=(1,2);",
            "(int _x,) value=(1,);",
            "(int hashCode,) value=(1,);",
        ] {
            rejected(body);
        }
        for source in [
            "macro class Example{} void main(){}",
            "@JsonCodable() int f()=>1; void main(){}",
        ] {
            assert!(parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err());
        }
    }
    /// Limites omitidos são Object? e testes/casts preservam tipos reificados e spans.
    #[test]
    fn bounded_functions_nullable_parameters_and_runtime_type_syntax() {
        let source = "class LoginService{int login()=>1;} T identity<T>(T value)=>value; T? nullable<T extends Object>(T? value)=>value; int use<T extends LoginService>(T value)=>value.login(); bool test<T extends Object?>(Object? value)=>value is T; void main(){Object? value=null;print(value is! int);print(value as Object?);List<int>? xs=null;}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 5);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(
            program.functions[0].type_parameters[0].bound,
            Type::NullableObject
        );
        assert_eq!(program.functions[1].type_parameters[0].bound, Type::Object);
        assert_eq!(program.functions[1].return_type, Type::NullableParameter(0));
        assert_eq!(
            program.functions[2].type_parameters[0].bound,
            Type::Class(0)
        );
        let StatementKind::Return(Some(value)) = &program.functions[3].body[0].kind else {
            panic!()
        };
        assert!(matches!(
            value.kind,
            ExprKind::TypeTest {
                ty: Type::Parameter(0),
                negated: false,
                ..
            }
        ));
        assert_eq!(&source[value.span.start..value.span.end], "value is T");
        assert!(matches!(
            &program.statements[1].kind,
            StatementKind::Print(Expr {
                kind: ExprKind::TypeTest { negated: true, .. },
                ..
            })
        ));
        assert!(matches!(
            &program.statements[2].kind,
            StatementKind::Print(Expr {
                kind: ExprKind::Cast {
                    ty: Type::NullableObject,
                    ..
                },
                ..
            })
        ));
        let source =
            "T first<T extends List<int>,U extends T>(T value,U other)=>value; void main(){}";
        let tokens = dartforge_lexer::lex(source).unwrap();
        assert_eq!(index_unit(&tokens).unwrap().functions.len(), 2);
        let program = parse(&tokens, source.len()).unwrap();
        assert_eq!(
            program.functions[0].type_parameters[1].bound,
            Type::Parameter(0)
        );
    }
    /// Classes/métodos genéricos e encadeamentos relacionais ainda não são aceitos.
    #[test]
    fn generic_bounds_and_type_operator_rejections() {
        for declaration in [
            "class C{T f<T>(T value)=>value;}",
            "T f<T extends>(T value)=>value;",
            "T f<T extends Object,T>(T value)=>value;",
            "T f<T extends void>(T value)=>value;",
        ] {
            let source = format!("{declaration} void main(){{}}");
            assert!(
                parse(&dartforge_lexer::lex(&source).unwrap(), source.len()).is_err(),
                "{source}"
            );
        }
        for body in [
            "print(1 as int + 1);",
            "print(1 as int is int);",
            "print(1 is int is bool);",
            "print(1 as int as Object);",
            "print(1 is!);",
            "print(1 as void);",
        ] {
            rejected(body);
        }
        parsed("void main(){print((1 as int)+1);print(1 is int == true);}");
        let source = "void main(){print(1+2 is int && true);}";
        let program = parsed(source);
        let StatementKind::Print(Expr {
            kind:
                ExprKind::Binary {
                    op: BinaryOp::And,
                    left,
                    ..
                },
            ..
        }) = &program.statements[0].kind
        else {
            panic!()
        };
        let ExprKind::TypeTest { operand, .. } = &left.kind else {
            panic!()
        };
        assert!(matches!(
            operand.kind,
            ExprKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        ));
        let source = "void main<T>(){}";
        assert!(parse(&dartforge_lexer::lex(source).unwrap(), source.len()).is_err());
    }
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
    /// Formas de construtor fora do subconjunto continuam rejeitadas.
    ///
    /// Nomeados, `const`, listas de inicialização e `super` explícito fazem
    /// parte do subconjunto e são exercitados em `crates/compiler/tests/classes_adv.rs.
    #[test]
    fn rejects_constructor_forms_outside_subset() {
        for declaration in [
            "class C{C();C(){}}",
            "class C{factory C();}",
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
        assert_eq!(program.functions[0].type_parameters[0].name, "T");
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
            "T f<T extends>(T x)=>x;",
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
            "List<void>? xs=null;",
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
    /// Pilha de um MiB rejeita reentrada de closures sem ampliar a pilha de execução.
    #[test]
    fn closure_expression_reentry_is_bounded_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                let mut body = "return 1;".to_owned();
                for _ in 0..70 {
                    body = format!("return (){{{body}}};");
                }
                rejected(&format!("var f=(){{{body}}};"));
                let mut expression = "1".to_owned();
                for _ in 0..70 {
                    expression = format!("{{'x':()=>({expression})}}");
                }
                rejected(&format!("var x={expression};"));
                // Corpos simples não reiniciam o orçamento da cascata envolvente.
                rejected(&format!("a{};", "..run((){print(1);})".repeat(1000)));
                parsed("void main(){var f=(){return (){return {'x':1};};};}");
            })
            .unwrap()
            .join()
            .unwrap();
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
            "class C { void x=1; } void main(){}",
            "class C extends Missing {} void main(){}",
            "class C {} class C {} void main(){}",
            "class int {} void main(){}",
            "class C {} void main(){ new C(); }",
            "class C {} void main(){ class Nested{} }",
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
