//! Liga bibliotecas Dart do grafo por símbolos e ASTs, sem concatenar fontes.
//! Cada arquivo constitui uma biblioteca, com imports diretos e privacidade por unidade.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_packages::{Combinator, GraphError, SourceGraph};
use dartforge_syntax::{
    Class, Expr, ExprKind, Function, Program, Statement, StatementKind, TokenKind, Type, TypeShape,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// Símbolo declarado por uma biblioteca antes da análise de corpos.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Symbol {
    owner: usize,
    class_id: Option<u32>,
}
/// Metadados imutáveis usados para verificar resolução antes da renomeação.
struct ClassNames<'a> {
    supports_implicit_members: bool,
    mixins: Vec<u32>,
    owner: usize,
    superclass: Option<u32>,
    interfaces: Vec<u32>,
    members: HashSet<&'a str>,
}

/// Compila todas as unidades com namespaces isolados e uma única entrada executável.
///
/// Os nomes globais e membros privados são renomeados na AST. O lexer não aceita
/// `$` em identificadores do usuário; o namespace gerado usa esse caractere reservado.
/// Imports repetidos são idempotentes. Ambiguidades entre imports são rejeitadas
/// mesmo quando não utilizadas, salvo quando uma declaração local da biblioteca prevalece.
///
/// # Erros
/// Preserva arquivo e intervalo local para falhas léxicas, sintáticas, de resolução
/// ou semânticas. A entrada exige void main(); bibliotecas podem declarar outro main.
/// O grafo deve usar apenas diretivas suportadas por dartforge_packages::load.
///
/// ```no_run
/// let graph = dartforge_packages::load(std::path::Path::new("main.dart"))?;
/// let js = dartforge_linker::compile_graph(&graph, false)?;
/// assert!(js.contains("export function main"));
/// # Ok::<(), dartforge_packages::GraphError>(())
/// ```
pub fn compile_graph(graph: &SourceGraph, optimize_constants: bool) -> Result<String, GraphError> {
    compile_graph_with(graph, optimize_constants, |module| {
        Ok(dartforge_codegen::emit(module))
    })
}

/// Resolve e valida bibliotecas antes de entregar a HIR a um backend selecionado.
///
/// A HIR empresta nomes das arenas locais e só pode ser consumida durante a chamada.
/// Diagnósticos do backend são traduzidos de spans virtuais para arquivo e span original.
///
/// # Erros
/// Retorna falhas de resolução, análise ou recursos rejeitados pelo backend.
pub fn compile_graph_with(
    graph: &SourceGraph,
    optimize_constants: bool,
    emit: impl FnOnce(&dartforge_hir::Module<'_>) -> Result<String, Diagnostic>,
) -> Result<String, GraphError> {
    compile_graph_with_options(graph, optimize_constants, false, emit)
}
/// Resolve bibliotecas e aplica passes independentes antes da emissão.
/// # Erros
/// Retorna falhas localizadas de resolução, análise e backend.
pub fn compile_graph_with_options(
    graph: &SourceGraph,
    optimize_constants: bool,
    merge_identical: bool,
    emit: impl FnOnce(&dartforge_hir::Module<'_>) -> Result<String, Diagnostic>,
) -> Result<String, GraphError> {
    if graph.entry >= graph.units.len() {
        return Err(GraphError {
            path: std::path::PathBuf::new(),
            span: None,
            message: "entrada do grafo inválida".into(),
        });
    }
    let mut tokens = Vec::new();
    let mut declarations = Vec::new();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        let all = dartforge_lexer::lex(&unit.source)
            .map_err(|error| source_error(graph, unit_id, error))?;
        let prefix = unit
            .imports
            .iter()
            .chain(&unit.exports)
            .map(|import| import.span.end)
            .max()
            .unwrap_or(0);
        let body = all
            .into_iter()
            .filter(|token| token.span.start >= prefix)
            .collect::<Vec<_>>();
        declarations.push(
            dartforge_parser::index_unit(&body)
                .map_err(|error| source_error(graph, unit_id, error))?,
        );
        tokens.push(body);
    }
    let mut own = Vec::new();
    let mut next_class = 0u32;
    let mut class_origins = HashMap::new();
    for (owner, declaration) in declarations.iter().enumerate() {
        let mut symbols = HashMap::new();
        for (names, is_class) in [
            (&declaration.classes, true),
            (&declaration.functions, false),
        ] {
            for item in names {
                if matches!(item.name, "print" | "int" | "String" | "bool") {
                    return Err(source_error(
                        graph,
                        owner,
                        Diagnostic::new(
                            "declaração oculta um nome nativo fora do subconjunto",
                            item.span,
                        ),
                    ));
                }
                let class_id = if is_class {
                    let id = next_class;
                    next_class = next_class.checked_add(1).ok_or_else(|| {
                        source_error(
                            graph,
                            owner,
                            Diagnostic::new("excesso de classes", item.span),
                        )
                    })?;
                    class_origins.insert(id, (owner, item.name));
                    Some(id)
                } else {
                    None
                };
                if symbols
                    .insert(item.name, Symbol { owner, class_id })
                    .is_some()
                {
                    return Err(source_error(
                        graph,
                        owner,
                        Diagnostic::new("símbolo top-level duplicado", item.span),
                    ));
                }
            }
        }
        own.push(symbols);
    }
    let exported = exported_namespaces(graph, &own)?;
    let mut visible = own.clone();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        for import in &unit.imports {
            let imported = exported.get(import.target).ok_or_else(|| {
                source_error(
                    graph,
                    unit_id,
                    Diagnostic::new("destino de import inválido", import.span),
                )
            })?;
            // O BTreeMap mantém a ordem lexical sem alocação ou ordenação adicional.
            for (&name, &symbol) in imported {
                if name.starts_with('_')
                    || own[unit_id].contains_key(name)
                    || !allows_name(name, &import.combinators)
                {
                    continue;
                }
                if let Some(previous) = visible[unit_id].get(name)
                    && previous.owner != symbol.owner
                {
                    return Err(source_error(
                        graph,
                        unit_id,
                        Diagnostic::new(
                            format!("import ambíguo: {name}; use show/hide para desambiguar"),
                            import.span,
                        ),
                    ));
                }
                visible[unit_id].insert(name, symbol);
            }
        }
    }
    // A arena permanece imóvel até emissão; referências da AST nunca escapam desta função.
    let mut names = HashMap::new();
    for (owner, unit_tokens) in tokens.iter().enumerate() {
        for token in unit_tokens {
            if let TokenKind::Word(name) = token.kind
                && (own[owner].contains_key(name) || name.starts_with('_'))
            {
                names
                    .entry((owner, name))
                    .or_insert_with(|| format!("$lib{owner}${name}"));
            }
        }
    }
    let mut programs = Vec::new();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        let env: BTreeMap<_, _> = visible[unit_id]
            .iter()
            .filter_map(|(&name, symbol)| symbol.class_id.map(|id| (name, id)))
            .collect();
        programs.push(
            dartforge_parser::parse_unit(&tokens[unit_id], unit.source.len(), env)
                .map_err(|error| source_error(graph, unit_id, error))?,
        );
    }
    let mut classes = HashMap::new();
    for (owner, program) in programs.iter_mut().enumerate() {
        for class in &mut program.classes {
            class.library_id = owner;
            classes.insert(
                class.id,
                ClassNames {
                    supports_implicit_members: !class.enum_values.is_empty()
                        || class.kind != dartforge_syntax::ClassKind::Class
                        || !class.mixins.is_empty(),
                    mixins: class.mixins.clone(),
                    owner,
                    superclass: class.superclass,
                    interfaces: class.interfaces.clone(),
                    members: class
                        .fields
                        .iter()
                        .map(|field| field.name)
                        .chain(class.methods.iter().map(|method| method.name))
                        .chain(class.abstract_methods.iter().map(|method| method.name))
                        .chain(class.enum_values.iter().copied())
                        .collect(),
                },
            );
        }
    }
    let entry_function = programs[graph.entry]
        .functions
        .iter()
        .find(|function| function.name == "main")
        .ok_or_else(|| {
            source_error(
                graph,
                graph.entry,
                Diagnostic::new("entrada exige void main()", Span { start: 0, end: 0 }),
            )
        })?;
    if entry_function.return_type != Type::Void || !entry_function.parameters.is_empty() {
        return Err(source_error(
            graph,
            graph.entry,
            Diagnostic::new(
                "entrada exige void main() sem parâmetros",
                entry_function.span,
            ),
        ));
    }
    let entry_span = entry_function.span;
    let mut offsets = Vec::new();
    let mut offset = 0usize;
    for unit in &graph.units {
        offsets.push(offset);
        offset = offset.checked_add(unit.source.len() + 1).ok_or_else(|| {
            source_error(
                graph,
                graph.entry,
                Diagnostic::new("grafo excede o espaço de spans", entry_span),
            )
        })?;
    }
    let mut linked_types = Vec::new();
    for (unit_id, program) in programs.iter_mut().enumerate() {
        let type_offset = u32::try_from(linked_types.len()).map_err(|_| {
            source_error(graph, unit_id, Diagnostic::new("tipos demais", entry_span))
        })?;
        let mut resolver = Resolver {
            types: &program.types,
            type_offset,
            unit: unit_id,
            graph,
            visible: &visible,
            names: &names,
            classes: &classes,
            class_origins: &class_origins,
            scopes: vec![],
            current_class: None,
            offset: offsets[unit_id],
        };
        for class in &mut program.classes {
            resolver.class(class)?;
        }
        for function in &mut program.functions {
            resolver.function(function, true)?;
        }
        for shape in &program.types {
            linked_types.push(match shape {
                TypeShape::List(t) => TypeShape::List(remap_type(*t, type_offset)),
                TypeShape::Iterable(t) => TypeShape::Iterable(remap_type(*t, type_offset)),
                TypeShape::Function { result, parameters } => TypeShape::Function {
                    result: remap_type(*result, type_offset),
                    parameters: parameters
                        .iter()
                        .map(|t| remap_type(*t, type_offset))
                        .collect(),
                },
            });
        }
    }
    let entry_name = names[&(graph.entry, "main")].as_str();
    let mut linked = Program {
        types: linked_types,
        extensions: vec![],
        classes: vec![],
        functions: vec![],
        statements: vec![Statement {
            kind: StatementKind::Expression(Expr {
                kind: ExprKind::Call {
                    name: entry_name,
                    arguments: vec![],
                },
                span: Span {
                    start: entry_span.start + offsets[graph.entry],
                    end: entry_span.end + offsets[graph.entry],
                },
            }),
            span: Span {
                start: entry_span.start + offsets[graph.entry],
                end: entry_span.end + offsets[graph.entry],
            },
        }],
    };
    for program in programs {
        linked.classes.extend(program.classes);
        linked.functions.extend(program.functions);
    }
    let resolution = dartforge_hir::expand_mixins(&mut linked)
        .and_then(|()| dartforge_semantic::analyze(&linked))
        .map_err(|error| {
            let unit_id = offsets
                .iter()
                .rposition(|&start| start <= error.span.start)
                .unwrap_or(graph.entry);
            source_error(
                graph,
                unit_id,
                Diagnostic::new(
                    error.message,
                    Span {
                        start: error.span.start.saturating_sub(offsets[unit_id]),
                        end: error.span.end.saturating_sub(offsets[unit_id]),
                    },
                ),
            )
        })?;
    if optimize_constants {
        dartforge_optimizer::fold_constants(&mut linked);
    }
    if merge_identical {
        dartforge_optimizer::merge_identical_functions(&mut linked, &resolution);
    }
    emit(&dartforge_hir::lower_resolved(linked, resolution)).map_err(|error| {
        let unit_id = offsets
            .iter()
            .rposition(|&start| start <= error.span.start)
            .unwrap_or(graph.entry);
        source_error(
            graph,
            unit_id,
            Diagnostic::new(
                error.message,
                Span {
                    start: error.span.start.saturating_sub(offsets[unit_id]),
                    end: error.span.end.saturating_sub(offsets[unit_id]),
                },
            ),
        )
    })
}

/// Aplica combinadores na ordem declarada sem tornar nomes privados públicos.
fn allows_name(name: &str, combinators: &[Combinator]) -> bool {
    !name.starts_with('_')
        && combinators.iter().all(|combinator| match combinator {
            Combinator::Show(names) => names.iter().any(|candidate| candidate == name),
            Combinator::Hide(names) => !names.iter().any(|candidate| candidate == name),
        })
}

/// Calcula exports transitivos por ponto fixo monotônico de origens declarativas.
/// Declarações próprias prevalecem antes da união; imports nunca entram nesse conjunto.
fn exported_namespaces<'a>(
    graph: &SourceGraph,
    own: &[HashMap<&'a str, Symbol>],
) -> Result<Vec<BTreeMap<&'a str, Symbol>>, GraphError> {
    let initial: Vec<BTreeMap<&'a str, BTreeSet<Symbol>>> = own
        .iter()
        .map(|symbols| {
            symbols
                .iter()
                .filter(|(name, _)| !name.starts_with('_'))
                .map(|(&name, &symbol)| (name, BTreeSet::from([symbol])))
                .collect()
        })
        .collect();
    let mut exported = initial;
    let mut dependents = vec![Vec::new(); graph.units.len()];
    for (owner, unit) in graph.units.iter().enumerate() {
        for (index, directive) in unit.exports.iter().enumerate() {
            let Some(parents) = dependents.get_mut(directive.target) else {
                return Err(source_error(
                    graph,
                    owner,
                    Diagnostic::new("destino de export inválido", directive.span),
                ));
            };
            parents.push((owner, index));
        }
    }
    let mut pending: VecDeque<_> = (0..graph.units.len()).collect();
    let mut queued = vec![true; graph.units.len()];
    while let Some(target) = pending.pop_front() {
        queued[target] = false;
        if dependents[target].is_empty() {
            continue;
        }
        // A cópia fica restrita ao namespace que mudou, sem recomputar o grafo inteiro.
        let namespace = exported[target].clone();
        for &(owner, index) in &dependents[target] {
            let directive = &graph.units[owner].exports[index];
            let mut changed = false;
            for (&name, symbols) in &namespace {
                if own[owner].contains_key(name) || !allows_name(name, &directive.combinators) {
                    continue;
                }
                let origins = exported[owner].entry(name).or_default();
                let previous = origins.len();
                origins.extend(symbols.iter().copied());
                changed |= origins.len() != previous;
            }
            if changed && !queued[owner] {
                queued[owner] = true;
                pending.push_back(owner);
            }
        }
    }
    let mut result = Vec::with_capacity(exported.len());
    for (unit_id, namespace) in exported.iter().enumerate() {
        let mut resolved = BTreeMap::new();
        for (&name, symbols) in namespace {
            if symbols.len() > 1 {
                let span = graph.units[unit_id]
                    .exports
                    .iter()
                    .find(|directive| {
                        allows_name(name, &directive.combinators)
                            && exported[directive.target].contains_key(name)
                    })
                    .map_or(Span { start: 0, end: 0 }, |directive| directive.span);
                return Err(source_error(
                    graph,
                    unit_id,
                    Diagnostic::new(
                        format!("export ambíguo: {name}; declarações de origens diferentes"),
                        span,
                    ),
                ));
            }
            if let Some(&symbol) = symbols.first() {
                resolved.insert(name, symbol);
            }
        }
        result.push(resolved);
    }
    Ok(result)
}

/// Constrói erro associado ao arquivo sem alterar o intervalo original.
fn source_error(graph: &SourceGraph, unit: usize, error: Diagnostic) -> GraphError {
    GraphError {
        path: graph.units[unit].path.clone(),
        span: Some(error.span),
        message: error.message,
    }
}

/// Resolve nomes originais antes de substituir símbolos e membros privados na AST.
fn remap_type(ty: Type, offset: u32) -> Type {
    match ty {
        Type::Applied(id) => Type::Applied(id.checked_add(offset).expect("IDs de tipos excedidos")),
        _ => ty,
    }
}

/// Resolve símbolos preservando o ambiente local de formas estruturais.
struct Resolver<'a, 'g> {
    types: &'g [TypeShape],
    type_offset: u32,
    unit: usize,
    graph: &'g SourceGraph,
    visible: &'g [HashMap<&'a str, Symbol>],
    names: &'a HashMap<(usize, &'a str), String>,
    classes: &'g HashMap<u32, ClassNames<'a>>,
    class_origins: &'g HashMap<u32, (usize, &'a str)>,
    scopes: Vec<HashSet<&'a str>>,
    current_class: Option<u32>,
    offset: usize,
}
impl<'a> Resolver<'a, '_> {
    /// Cria diagnóstico no arquivo corrente antes da mudança dos spans.
    fn error(&self, span: Span, message: impl Into<String>) -> GraphError {
        source_error(self.graph, self.unit, Diagnostic::new(message, span))
    }
    /// Converte um intervalo local para o domínio temporário usado na análise conjunta.
    fn span(&self, span: &mut Span) {
        span.start += self.offset;
        span.end += self.offset;
    }
    /// Consulta nomes locais incluindo declarações posteriores no mesmo bloco.
    fn local(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
    /// Consulta membros visíveis sem confundir privados de bibliotecas distintas.
    fn implicit_member(&self, name: &str) -> bool {
        let mut pending: Vec<u32> = self.current_class.into_iter().collect();
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(class) = self.classes.get(&id) else {
                continue;
            };
            if class.members.contains(name) && (!name.starts_with('_') || class.owner == self.unit)
            {
                return true;
            }
            pending.extend(class.superclass);
            pending.extend(class.interfaces.iter().copied());
            pending.extend(class.mixins.iter().copied());
        }
        false
    }
    /// Retorna um nome privado pertencente à biblioteca corrente.
    fn member_name(&self, name: &'a str) -> &'a str {
        if name.starts_with('_') {
            self.names[&(self.unit, name)].as_str()
        } else {
            name
        }
    }
    /// Verifica o nome lexical de tipos antes de esconder a biblioteca no nome gerado.
    fn ty(&self, ty: Type, span: Span) -> Result<(), GraphError> {
        if let Type::Applied(id) = ty {
            match self
                .types
                .get(id as usize)
                .ok_or_else(|| self.error(span, "ID de tipo inválido"))?
            {
                TypeShape::List(t) | TypeShape::Iterable(t) => self.ty(*t, span)?,
                TypeShape::Function { result, parameters } => {
                    self.ty(*result, span)?;
                    for t in parameters {
                        self.ty(*t, span)?;
                    }
                }
            }
        }
        let name = match ty {
            Type::Class(id) | Type::NullableClass(id) => {
                self.class_origins.get(&id).map(|(_, name)| *name)
            }
            Type::Int | Type::NullableInt => Some("int"),
            Type::String | Type::NullableString => Some("String"),
            Type::Bool | Type::NullableBool => Some("bool"),
            _ => None,
        };
        if let Some(name) = name
            && (self.local(name) || self.implicit_member(name))
        {
            return Err(self.error(span, format!("declaração oculta o tipo {name}")));
        }
        Ok(())
    }
    /// Resolve uma classe sem permitir que renomeação esconda colisões originais.
    fn class(&mut self, class: &mut Class<'a>) -> Result<(), GraphError> {
        self.current_class = Some(class.id);
        for name in &mut class.enum_constructor_fields {
            *name = self.member_name(name);
        }
        for args in &mut class.enum_arguments {
            for arg in args {
                self.expression(arg)?;
            }
        }
        for field in &mut class.fields {
            if field.name == class.name {
                return Err(self.error(field.span, "membro não pode ter o nome da classe"));
            }
            self.ty(field.ty, field.span)?;
            field.ty = remap_type(field.ty, self.type_offset);
            self.expression(&mut field.initializer)?;
            field.name = self.member_name(field.name);
            self.span(&mut field.span);
        }
        for interface in &class.interfaces {
            self.ty(Type::Class(*interface), class.span)?;
        }
        for method in class.methods.iter_mut().chain(&mut class.abstract_methods) {
            if method.name == class.name {
                return Err(self.error(method.span, "método não pode ter o nome da classe"));
            }
            self.function(method, false)?;
        }
        class.name = self.names[&(self.unit, class.name)].as_str();
        self.span(&mut class.span);
        self.current_class = None;
        Ok(())
    }
    /// Resolve assinatura fora do escopo de parâmetros e corpo em escopo aninhado.
    fn function(&mut self, function: &mut Function<'a>, top_level: bool) -> Result<(), GraphError> {
        self.ty(function.return_type, function.span)?;
        function.return_type = remap_type(function.return_type, self.type_offset);
        for parameter in &mut function.parameters {
            self.ty(parameter.ty, parameter.span)?;
            parameter.ty = remap_type(parameter.ty, self.type_offset);
            self.span(&mut parameter.span);
        }
        self.scopes.push(
            function
                .parameters
                .iter()
                .map(|parameter| parameter.name)
                .collect(),
        );
        self.block(&mut function.body)?;
        self.scopes.pop();
        function.name = if top_level {
            self.names[&(self.unit, function.name)].as_str()
        } else {
            self.member_name(function.name)
        };
        self.span(&mut function.span);
        Ok(())
    }
    /// Pré-declara nomes para respeitar sombreamento anterior à declaração.
    fn block(&mut self, body: &mut [Statement<'a>]) -> Result<(), GraphError> {
        self.scopes.push(
            body.iter()
                .filter_map(|statement| {
                    if let StatementKind::Variable { name, .. } = statement.kind {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect(),
        );
        for statement in body {
            self.statement(statement)?;
        }
        self.scopes.pop();
        Ok(())
    }
    /// Resolve instruções preservando os escopos próprios de corpo e cabeçalho de for.
    /// Resolve binding de padrão no escopo exclusivo do braço.
    fn pattern(
        &mut self,
        pattern: &mut dartforge_syntax::Pattern<'a>,
        span: Span,
    ) -> Result<(), GraphError> {
        match pattern {
            dartforge_syntax::Pattern::Constant(e) => self.expression(e)?,
            dartforge_syntax::Pattern::Binding { ty, name } => {
                self.ty(*ty, span)?;
                *ty = remap_type(*ty, self.type_offset);
                self.scopes.last_mut().unwrap().insert(*name);
            }
            dartforge_syntax::Pattern::Type(ty) => {
                self.ty(*ty, span)?;
                *ty = remap_type(*ty, self.type_offset);
            }
            dartforge_syntax::Pattern::Wildcard => {}
        }
        Ok(())
    }
    /// Resolve instrução e seus intervalos virtuais.
    fn statement(&mut self, statement: &mut Statement<'a>) -> Result<(), GraphError> {
        match &mut statement.kind {
            StatementKind::Switch { scrutinee, cases } => {
                self.expression(scrutinee)?;
                for case in cases {
                    self.scopes.push(HashSet::new());
                    self.pattern(&mut case.pattern, case.span)?;
                    if let Some(g) = &mut case.guard {
                        self.expression(g)?;
                    }
                    self.block(&mut case.body)?;
                    self.scopes.pop();
                    self.span(&mut case.span);
                }
            }
            StatementKind::Variable {
                annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = annotation {
                    self.ty(*ty, statement.span)?;
                    *ty = remap_type(*ty, self.type_offset);
                }
                self.expression(initializer)?;
            }
            StatementKind::Assign { name, value } => {
                if !self.local(name) {
                    if self.implicit_member(name)
                        && self
                            .current_class
                            .is_some_and(|id| self.classes[&id].supports_implicit_members)
                    {
                        *name = self.member_name(name);
                    } else {
                        return Err(self.error(
                            statement.span,
                            "atribuição exige variável local ou membro declarado",
                        ));
                    }
                }
                self.expression(value)?;
            }
            StatementKind::IndexAssign {
                receiver,
                index,
                value,
            } => {
                self.expression(receiver)?;
                self.expression(index)?;
                self.expression(value)?;
            }
            StatementKind::FieldAssign {
                receiver,
                name,
                value,
            } => {
                self.expression(receiver)?;
                self.expression(value)?;
                *name = self.member_name(name);
            }
            StatementKind::Print(value) => {
                if self.local("print") || self.implicit_member("print") {
                    return Err(self.error(statement.span, "print sombreado não é a função nativa"));
                }
                self.expression(value)?;
            }
            StatementKind::Expression(value) => self.expression(value)?,
            StatementKind::Return(value) => {
                if let Some(value) = value {
                    self.expression(value)?;
                }
            }
            StatementKind::If {
                condition,
                then_body,
                else_body,
            } => {
                self.expression(condition)?;
                self.block(then_body)?;
                if let Some(body) = else_body {
                    self.block(body)?;
                }
            }
            StatementKind::While { condition, body } => {
                self.expression(condition)?;
                self.block(body)?;
            }
            StatementKind::DoWhile { body, condition } => {
                self.block(body)?;
                self.expression(condition)?;
            }
            StatementKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                let mut scope = HashSet::new();
                if let Some(init) = initializer
                    && let StatementKind::Variable { name, .. } = init.kind
                {
                    scope.insert(name);
                }
                self.scopes.push(scope);
                if let Some(init) = initializer {
                    self.statement(init)?;
                }
                if let Some(value) = condition {
                    self.expression(value)?;
                }
                if let Some(update) = update {
                    self.statement(update)?;
                }
                self.block(body)?;
                self.scopes.pop();
            }
            StatementKind::Block(body) => self.block(body)?,
            StatementKind::Break | StatementKind::Continue => {}
        }
        self.span(&mut statement.span);
        Ok(())
    }
    /// Resolve chamadas e construtores sem reescrever texto nem capturar globais indevidos.
    fn expression(&mut self, expression: &mut Expr<'a>) -> Result<(), GraphError> {
        if let ExprKind::GenericCall { type_arguments, .. } = &mut expression.kind {
            for t in type_arguments {
                self.ty(*t, expression.span)?;
                *t = remap_type(*t, self.type_offset);
            }
        }
        match &mut expression.kind {
            ExprKind::Const(e) => self.expression(e)?,
            ExprKind::Switch { scrutinee, arms } => {
                self.expression(scrutinee)?;
                for arm in arms {
                    self.scopes.push(HashSet::new());
                    self.pattern(&mut arm.pattern, arm.span)?;
                    if let Some(g) = &mut arm.guard {
                        self.expression(g)?;
                    }
                    self.expression(&mut arm.value)?;
                    self.scopes.pop();
                    self.span(&mut arm.span);
                }
            }
            ExprKind::Identifier(name)
                if !self.local(name)
                    && self.implicit_member(name)
                    && self
                        .current_class
                        .is_some_and(|id| self.classes[&id].supports_implicit_members) =>
            {
                *name = self.member_name(name);
            }
            ExprKind::Call { name, arguments }
            | ExprKind::GenericCall {
                name, arguments, ..
            } => {
                let implicit = !self.local(name) && self.implicit_member(name);
                if implicit
                    && !self
                        .current_class
                        .is_some_and(|id| self.classes[&id].supports_implicit_members)
                {
                    return Err(self.error(
                        expression.span,
                        "chamada de membro exige receptor this explícito",
                    ));
                }
                if implicit {
                    *name = self.member_name(name);
                } else if *name != "print" && !self.local(name) {
                    let symbol = self.visible[self.unit].get(name).ok_or_else(|| {
                        self.error(
                            expression.span,
                            format!("função não visível nesta biblioteca: {name}"),
                        )
                    })?;
                    if symbol.class_id.is_some() {
                        return Err(self.error(expression.span, "classe usada como função"));
                    }
                    *name = self.names[&(symbol.owner, *name)].as_str();
                }
                for argument in arguments {
                    self.expression(argument)?;
                }
            }
            ExprKind::Closure {
                parameters,
                return_type,
                body,
                ..
            } => {
                self.ty(*return_type, expression.span)?;
                *return_type = remap_type(*return_type, self.type_offset);
                for p in parameters.iter_mut() {
                    self.ty(p.ty, p.span)?;
                    p.ty = remap_type(p.ty, self.type_offset);
                    self.span(&mut p.span);
                }
                self.scopes
                    .push(parameters.iter().map(|p| p.name).collect());
                self.block(body)?;
                self.scopes.pop();
            }
            ExprKind::List {
                element_type,
                elements,
            } => {
                if let Some(t) = element_type {
                    self.ty(*t, expression.span)?;
                    *t = remap_type(*t, self.type_offset);
                }
                for element in elements {
                    self.expression(element)?;
                }
            }
            ExprKind::Index { receiver, index } => {
                self.expression(receiver)?;
                self.expression(index)?;
            }
            ExprKind::Invoke { callee, arguments } => {
                self.expression(callee)?;
                for argument in arguments {
                    self.expression(argument)?;
                }
            }
            ExprKind::Identifier(name) if !self.local(name) && !self.implicit_member(name) => {
                if let Some(symbol) = self.visible[self.unit].get(name)
                    && symbol.class_id.is_none()
                {
                    *name = self.names[&(symbol.owner, *name)].as_str();
                }
            }
            ExprKind::Construct { class_id } => {
                let (_, name) = self
                    .class_origins
                    .get(class_id)
                    .ok_or_else(|| self.error(expression.span, "classe desconhecida"))?;
                if self.local(name) || self.implicit_member(name) {
                    return Err(self.error(
                        expression.span,
                        format!("declaração oculta construtor {name}"),
                    ));
                }
            }
            ExprKind::EnumValue { class_id, name } => {
                self.ty(Type::Class(*class_id), expression.span)?;
                let (owner, _) = self
                    .class_origins
                    .get(class_id)
                    .ok_or_else(|| self.error(expression.span, "enum desconhecido"))?;
                if name.starts_with('_') && *owner != self.unit {
                    return Err(
                        self.error(expression.span, "valor de enum privado em outra biblioteca")
                    );
                }
            }
            ExprKind::Member { receiver, name } => {
                self.expression(receiver)?;
                *name = self.member_name(name);
            }
            ExprKind::MethodCall {
                receiver,
                name,
                arguments,
            } => {
                self.expression(receiver)?;
                *name = self.member_name(name);
                for argument in arguments {
                    self.expression(argument)?;
                }
            }
            ExprKind::Unary { operand, .. } => self.expression(operand)?,
            ExprKind::Binary { left, right, .. } => {
                self.expression(left)?;
                self.expression(right)?;
            }
            _ => {}
        }
        self.span(&mut expression.span);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    /// Arenas de tipos de bibliotecas distintas não confundem List<String> e List<int>.
    #[test]
    fn structural_types_and_closure_scopes_across_libraries() {
        let graph = graph(&[
            (
                "List<String> words()=>['x']; void main(){List<int> xs=numbers(); int Function(int) f=adder(2); xs[0]=f(xs[0]); print(xs[0]);}",
                &[1],
            ),
            (
                "List<int> numbers()=>[3]; int Function(int) adder(int step)=>(int x)=>x+step;",
                &[],
            ),
        ]);
        for optimize in [false, true] {
            assert!(compile_graph(&graph, optimize).is_ok());
        }
    }
    /// Executa captura mutável e chamada de local após remapear tipos de bibliotecas.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn linked_closure_capture_and_list_index_execute() {
        let graph = graph(&[
            (
                "List<String> words()=>['x']; void main(){List<int> xs=numbers(); int Function(int) f=adder(2); xs[0]=f(xs[0]); print(xs[0]);}",
                &[1],
            ),
            (
                "List<int> numbers()=>[3]; int Function(int) adder(int step){return (int x){step=step+1;return x+step;};}",
                &[],
            ),
        ]);
        for optimize in [false, true] {
            let js = compile_graph(&graph, optimize).unwrap();
            let run = std::process::Command::new("node")
                .args(["--eval", &js])
                .output()
                .unwrap();
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "6");
        }
        let inferred = self::graph(&[("void main(){var xs=[3];print(xs[0]);}", &[])]);
        let js = compile_graph(&inferred, false).unwrap();
        let run = std::process::Command::new("node")
            .args(["--eval", &js])
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "3");
    }
    /// Preserva nomes de enum e contratos públicos através de bibliotecas distintas.
    #[test]
    fn imports_abstract_interfaces_and_enum_names() {
        let sources = graph(&[
            (
                "class A implements I { E value()=>E.name; } void main(){I a=A(); print(a.value().name);}",
                &[1],
            ),
            (
                "abstract class I { E value(); } enum E { name, _private }",
                &[],
            ),
        ]);
        let js = compile_graph(&sources, false).unwrap();
        assert!(js.contains("$df_name:\"name\""));
        let sources = graph(&[
            ("void main(){print(E._private.index);}", &[1]),
            ("enum E {_private}", &[]),
        ]);
        assert!(
            compile_graph(&sources, false)
                .unwrap_err()
                .message
                .contains("privado")
        );
    }
    /// Interface permite extends local, mas exige implements na outra biblioteca.
    #[test]
    fn interface_extends_respects_library_identity() {
        let local = graph(&[(
            "interface class I { int f()=>1; } class C extends I {} void main(){print(C().f());}",
            &[],
        )]);
        assert!(compile_graph(&local, false).is_ok());
        let external = graph(&[
            ("class C extends I {} void main(){}", &[1]),
            ("interface class I { int f()=>1; }", &[]),
        ]);
        assert!(compile_graph(&external, false).is_err());
        let implemented = graph(&[
            (
                "class C implements I { int f()=>2; } void main(){I c=C();print(c.f());}",
                &[1],
            ),
            ("abstract interface class I { int f(); }", &[]),
        ]);
        assert!(compile_graph(&implemented, false).is_ok());
    }
    /// Monta um grafo em memória com imports representados pelas arestas declaradas.
    fn graph(sources: &[(&str, &[usize])]) -> SourceGraph {
        SourceGraph {
            entry: 0,
            units: sources
                .iter()
                .enumerate()
                .map(|(id, (source, targets))| dartforge_packages::SourceUnit {
                    path: PathBuf::from(format!("library_{id}.dart")),
                    source: (*source).into(),
                    exports: vec![],
                    imports: targets
                        .iter()
                        .map(|&target| dartforge_packages::Import {
                            uri: format!("library_{target}.dart"),
                            target,
                            span: Span { start: 0, end: 0 },
                            combinators: vec![],
                        })
                        .collect(),
                })
                .collect(),
        }
    }
    /// Constrói uma diretiva resolvida para testar filtros sem alterar texto fonte.
    fn edge(target: usize, combinators: Vec<Combinator>) -> dartforge_packages::Import {
        dartforge_packages::Import {
            uri: format!("library_{target}.dart"),
            target,
            span: Span { start: 0, end: 0 },
            combinators,
        }
    }
    /// Show e hide filtram imports antes da combinação das origens visíveis.
    #[test]
    fn import_combinators_filter_ambiguity_and_compose() {
        let mut sources = graph(&[
            ("void main(){print(f());}", &[1, 2]),
            ("int f(){return 1;} int a(){return 3;}", &[]),
            ("int f(){return 2;} int b(){return 4;}", &[]),
        ]);
        sources.units[0].imports[0].combinators = vec![
            Combinator::Show(vec!["f".into(), "a".into()]),
            Combinator::Show(vec!["f".into()]),
        ];
        sources.units[0].imports[1].combinators = vec![Combinator::Hide(vec!["f".into()])];
        assert!(compile_graph(&sources, false).is_ok());
        sources.units[0].source = "void main(){print(a());}".into();
        assert!(
            compile_graph(&sources, false)
                .unwrap_err()
                .message
                .contains("não visível")
        );
        sources.units[0].source = "void main(){print(f());}".into();
        sources.units[0].imports[0].combinators = vec![
            Combinator::Hide(vec!["f".into()]),
            Combinator::Show(vec!["f".into()]),
        ];
        assert!(compile_graph(&sources, false).is_err());
    }
    /// Reexports transitivos em diamante e ciclo convergem sem duplicar a mesma origem.
    #[test]
    fn export_fixed_point_handles_diamond_and_cycles() {
        let mut sources = graph(&[
            ("void main(){print(f());}", &[1]),
            ("", &[]),
            ("", &[]),
            ("", &[]),
            ("int f(){return 7;}", &[]),
        ]);
        sources.units[1].exports = vec![edge(2, vec![]), edge(3, vec![])];
        sources.units[2].exports = vec![edge(4, vec![])];
        sources.units[3].exports = vec![edge(4, vec![])];
        sources.units[4].exports = vec![edge(1, vec![])];
        let js = compile_graph(&sources, false).unwrap();
        assert_eq!(js.matches("function $df_$lib4$f(").count(), 1);
        assert!(js.contains("console.log($df_$lib4$f())"));
        sources.units[1].exports[0].combinators = vec![Combinator::Hide(vec!["f".into()])];
        sources.units[1].exports[1].combinators = vec![Combinator::Hide(vec!["f".into()])];
        assert!(compile_graph(&sources, false).is_err());
    }
    /// Declarações próprias prevalecem sobre exports conflitantes, mas privados nunca saem.
    #[test]
    fn export_local_precedence_privacy_and_ambiguity() {
        let mut sources = graph(&[
            ("void main(){print(f());}", &[1]),
            ("int f(){return 10;}", &[]),
            ("int f(){return 2;} int _secret(){return 99;}", &[]),
            ("int f(){return 3;}", &[]),
        ]);
        sources.units[1].exports = vec![edge(2, vec![]), edge(3, vec![])];
        assert!(
            compile_graph(&sources, false)
                .unwrap()
                .contains("console.log($df_$lib1$f())")
        );
        sources.units[1].source = String::new();
        let error = compile_graph(&sources, false).unwrap_err();
        assert_eq!(error.path, PathBuf::from("library_1.dart"));
        assert!(error.message.contains("export ambíguo: f"));
        sources.units[1].exports = vec![edge(2, vec![Combinator::Show(vec!["_secret".into()])])];
        sources.units[0].source = "void main(){print(_secret());}".into();
        assert!(
            compile_graph(&sources, false)
                .unwrap_err()
                .message
                .contains("não visível")
        );
    }
    /// Export não importa nomes para o próprio corpo e import não reexporta nomes.
    #[test]
    fn exports_and_imports_have_distinct_local_namespaces() {
        let mut sources = graph(&[
            ("void main(){print(f());}", &[1]),
            ("", &[2]),
            ("int f(){return 1;}", &[]),
        ]);
        assert!(compile_graph(&sources, false).is_err());
        sources.units[1].imports.clear();
        sources.units[1].exports = vec![edge(2, vec![])];
        assert!(compile_graph(&sources, false).is_ok());
        sources.units[1].source = "int local(){return f();}".into();
        let error = compile_graph(&sources, false).unwrap_err();
        assert_eq!(error.path, PathBuf::from("library_1.dart"));
        assert!(error.message.contains("não visível"));
        sources.units[1].imports = vec![edge(2, vec![])];
        assert!(compile_graph(&sources, false).is_ok());
    }
    /// Executa uma classe reexportada e seleciona a função filtrada em ambos os modos.
    #[test]
    #[ignore = "requires Node.js on PATH"]
    fn filtered_reexports_execute_in_both_modes() {
        let mut sources = graph(&[
            ("void main(){var c=C();print(c.value()+f());}", &[1, 2]),
            ("", &[]),
            ("int f(){return 100;} int unused(){return 1;}", &[]),
            ("class C {int value(){return 5;}} int f(){return 2;}", &[]),
        ]);
        sources.units[1].exports = vec![edge(
            3,
            vec![Combinator::Show(vec!["C".into(), "f".into()])],
        )];
        sources.units[0].imports[1].combinators = vec![Combinator::Hide(vec!["f".into()])];
        for optimize in [false, true] {
            let js = compile_graph(&sources, optimize).unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &js])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
        }
    }
    /// Bibliotecas homônimas em ramos distintos recebem nomes globais diferentes.
    /// Escolhe a primeira ambiguidade lexical independentemente das sementes dos mapas.
    #[test]
    fn import_ambiguity_diagnostic_is_deterministic() {
        let graph = graph(&[
            ("void main(){}", &[1, 2]),
            ("int zeta(){return 1;} int alpha(){return 2;}", &[]),
            ("int zeta(){return 3;} int alpha(){return 4;}", &[]),
        ]);
        for _ in 0..64 {
            let error = compile_graph(&graph, false).unwrap_err();
            assert_eq!(error.path, PathBuf::from("library_0.dart"));
            assert_eq!(
                error.message,
                "import ambíguo: alpha; use show/hide para desambiguar"
            );
        }
    }
    #[test]
    fn namespace_diamond_cycles_and_library_main() {
        let graph = graph(&[
            ("void main(){print(a()+b());}", &[1, 2]),
            ("int a(){return shared();} void main(){}", &[3]),
            ("int b(){return shared();}", &[3]),
            ("int shared(){return 7;}", &[1]),
        ]);
        let js = compile_graph(&graph, false).unwrap();
        assert!(js.contains("$df_$lib3$shared"));
        assert!(js.contains("$df_$lib1$main"));
        assert_eq!(js.matches("function $df_$lib3$shared(").count(), 1);
        assert!(compile_graph(&graph, true).is_ok());
        let homonyms = self::graph(&[
            ("void main(){print(a()+b());}", &[1, 2]),
            ("int a(){return helper();} int helper(){return 1;}", &[]),
            ("int b(){return helper();} int helper(){return 2;}", &[]),
        ]);
        // A importação direta dos dois helper é ambígua, mesmo sem uso direto.
        assert!(
            compile_graph(&homonyms, false)
                .unwrap_err()
                .message
                .contains("ambíguo")
        );
    }
    /// Imports não transitivos e símbolos privados nunca escapam da biblioteca.
    #[test]
    fn direct_import_and_private_top_level_boundaries() {
        for sources in [
            vec![
                ("void main(){print(hidden());}", &[1][..]),
                ("int public(){return hidden();}", &[2][..]),
                ("int hidden(){return 1;}", &[][..]),
            ],
            vec![
                ("void main(){print(_hidden());}", &[1][..]),
                ("int _hidden(){return 1;}", &[][..]),
            ],
        ] {
            let error = compile_graph(&graph(&sources), false).unwrap_err();
            assert_eq!(error.path, PathBuf::from("library_0.dart"));
            assert!(error.message.contains("não visível"));
        }
        assert!(
            compile_graph(
                &graph(&[
                    ("void main(){print(a()+b());}", &[1, 2]),
                    ("int a(){return _same();} int _same(){return 1;}", &[]),
                    ("int b(){return _same();} int _same(){return 2;}", &[])
                ]),
                false
            )
            .is_ok()
        );
    }
    /// Classes importadas têm IDs globais e herança preservada entre bibliotecas.
    #[test]
    fn cross_library_classes_and_private_members() {
        let sources = [
            ("void main(){var c=Child(); print(c.value());}", &[1][..]),
            (
                "class Child extends Base {int value(){return this.readValue();}}",
                &[2][..],
            ),
            (
                "class Base {int _x=3; int readValue(){return this._x;}}",
                &[][..],
            ),
        ];
        let js = compile_graph(&graph(&sources), false).unwrap();
        assert!(js.contains("$lib2$_x"));
        let denied = graph(&[
            ("void main(){var c=C(); print(c._x);}", &[1]),
            ("class C {int _x=3;}", &[]),
        ]);
        let error = compile_graph(&denied, false).unwrap_err();
        assert_eq!(error.path, PathBuf::from("library_0.dart"));
        assert!(error.message.contains("Unknown field"));
        let private_twins = graph(&[
            (
                "class Child extends Base {int _x=2; int child(){return this._x;}} void main(){var c=Child();print(c.child()+c.readBase());}",
                &[1],
            ),
            (
                "class Base {int _x=1; int readBase(){return this._x;}}",
                &[],
            ),
        ]);
        assert!(compile_graph(&private_twins, false).is_ok());
    }
    /// Sombreamento é verificado antes de nomes de classes e funções serem alterados.
    #[test]
    fn lexical_shadowing_and_type_shadowing_are_not_erased() {
        for source in [
            "void main(){var f=1;print(f());}",
            "void main(){print(f());var f=1;}",
            "void main(){var C=1;var c=C();}",
            "void main(){var C=1;C c=C();}",
            "void main(){for(var f=0;f<1;f=f+1){print(f());}}",
        ] {
            assert!(
                compile_graph(
                    &graph(&[(source, &[1]), ("int f(){return 1;} class C {}", &[])]),
                    false
                )
                .is_err(),
                "{source}"
            );
        }
        assert!(
            compile_graph(
                &graph(&[
                    ("void main(){ {var f=2;print(f);} print(f());}", &[1]),
                    ("int f(){return 1;}", &[])
                ]),
                false
            )
            .is_ok()
        );
        assert!(
            compile_graph(
                &graph(&[
                    ("void main(){print(f());} int f(){return 2;}", &[1]),
                    ("int f(){return 1;}", &[])
                ]),
                false
            )
            .is_ok()
        );
    }
    /// Erros de parser e semântica mantêm caminho e intervalo local da dependência.
    #[test]
    fn imported_errors_keep_source_path_and_local_span() {
        for source in [
            "int f(){return true;}",
            "int f(){return ;}",
            "int f(){return unknown;}",
        ] {
            let graph = graph(&[("void main(){print(f());}", &[1]), (source, &[])]);
            let error = compile_graph(&graph, false).unwrap_err();
            assert_eq!(error.path, PathBuf::from("library_1.dart"));
            let span = error.span.unwrap();
            assert!(
                span.start <= span.end && span.end <= source.len(),
                "{error}"
            );
        }
        assert!(compile_graph(&graph(&[("int main(){return 1;}", &[])]), false).is_err());
        assert!(compile_graph(&graph(&[("int f(){return 1;}", &[])]), false).is_err());
    }
    /// Membros implícitos mantêm precedência sobre nomes globais antes de renomear.
    #[test]
    fn implicit_members_and_constructor_types_stay_protected() {
        for source in [
            "class A {int f=1;int x=f();} void main(){}",
            "class A {int C=1;C make(){return C();}} void main(){}",
            "class A {int f(){return 1;} int g(){return f();}} void main(){}",
        ] {
            assert!(
                compile_graph(
                    &graph(&[(source, &[1]), ("int f(){return 9;} class C{}", &[])]),
                    false
                )
                .is_err(),
                "{source}"
            );
        }
    }
    /// Interfaces transitivas ocultam globais antes da renomeação, inclusive em diamantes.
    #[test]
    fn interface_members_never_rebind_to_global_functions() {
        let graph = graph(&[
            (
                "int f()=>1; abstract class B implements Left,Right {int run()=>f();} class C extends B {int f()=>2;} void main(){print(C().run());}",
                &[1],
            ),
            (
                "abstract class I{int f();} abstract class Left implements I{} abstract class Right implements I{}",
                &[],
            ),
        ]);
        for optimize in [false, true] {
            let error = compile_graph(&graph, optimize).unwrap_err();
            assert!(
                error.message.contains("this explícito"),
                "{}",
                error.message
            );
            assert_eq!(error.path, PathBuf::from("library_0.dart"));
        }
    }
    /// Um membro privado de interface estrangeira não oculta o global privado local.
    #[test]
    fn foreign_private_interface_members_do_not_shadow_globals() {
        let graph = graph(&[
            (
                "int _f()=>1; abstract class B implements I {int run()=>_f();} void main(){}",
                &[1],
            ),
            ("abstract class I {int _f();}", &[]),
        ]);
        assert!(compile_graph(&graph, false).is_ok());
    }
    /// Executa a saída ligada para conferir campos privados e despacho entre bibliotecas.
    #[test]
    #[ignore = "requires Node.js on PATH"]
    fn linked_output_runs_with_private_fields_and_two_mains() {
        let graph = graph(&[
            (
                "class Child extends Base {int _x=2;int child(){return this._x;}} void main(){var c=Child();print(c.child()+c.readBase()+helper());}",
                &[1],
            ),
            (
                "class Base {int _x=1;int readBase(){return this._x;}} int helper(){return 4;} void main(){print(999);}",
                &[],
            ),
        ]);
        for optimize in [false, true] {
            let js = compile_graph(&graph, optimize).unwrap();
            let output = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &js])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "7");
        }
    }
}
