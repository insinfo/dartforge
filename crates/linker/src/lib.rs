//! Liga bibliotecas Dart do grafo por símbolos e ASTs, sem concatenar fontes.
//! Cada arquivo constitui uma biblioteca, com imports diretos e privacidade por unidade,
//! exceto as partes: suas declarações entram na biblioteca que as declara com `part`.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_packages::{Combinator, GraphError, SourceGraph};
use dartforge_syntax::{
    Class, Expr, ExprKind, Function, Program, Statement, StatementKind, TokenKind, Type, TypeShape,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// Símbolo declarado por uma biblioteca antes da análise de corpos.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Symbol {
    intrinsic: bool,
    owner: usize,
    class_id: Option<u32>,
}
/// Nome alcançável somente por um prefixo de import, com a origem já resolvida.
///
/// Os nomes de uma biblioteca ficam num `Vec` ordenado por `(prefixo, nome)` e
/// são consultados por busca binária. Um mapa por prefixo por arquivo alocaria
/// no caminho quente da resolução de namespaces sem ganho mensurável: os
/// arquivos reais têm poucos prefixos, e o vetor de um programa sem prefixo
/// nenhum não aloca.
#[derive(Clone, Copy)]
struct Prefixed<'a> {
    /// Prefixo declarado na diretiva, sem o ponto.
    prefix: &'a str,
    /// Nome exportado pela biblioteca importada.
    name: &'a str,
    /// Origem já resolvida, inclusive por reexport transitivo.
    symbol: Symbol,
    /// Intervalo da diretiva que trouxe o nome, para o diagnóstico de ambiguidade.
    span: Span,
}

/// Nó que um acesso `p.nome` assume depois de resolvido.
#[derive(Clone, Copy)]
enum Shape<'a> {
    /// Função de topo importada, com o nome já qualificado pela biblioteca.
    Call(&'a str),
    /// Valor de topo importado, com o nome já qualificado pela biblioteca.
    Value(&'a str),
    /// Construtor sem nome de uma classe importada.
    Construct(u32),
    /// Construtor nomeado ou método estático de uma classe importada.
    Named(u32, &'a str),
    /// Valor de enum ou campo estático de uma classe importada.
    Enum(u32, &'a str),
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
        dartforge_codegen::validate_javascript(module)?;
        Ok(dartforge_codegen::emit(module))
    })
}

/// Tempo por fase, em nanossegundos, e trabalho efetivamente realizado.
///
/// Os números respondem "quanto trabalho foi feito", não apenas "quanto tempo
/// passou": sem os contadores, um ganho por reuso de cache seria indistinguível
/// de uma máquina mais rápida. Nenhuma fase é estimada por diferença; cada uma
/// é cronometrada no próprio trecho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LinkStats {
    /// Tokenização de todas as unidades, incluindo o filtro de diretivas.
    pub lex_ns: u128,
    /// Índice de declarações de topo de cada unidade.
    pub outline_ns: u128,
    /// Resolução de namespaces, prefixos, exports e privacidade por biblioteca.
    pub namespace_ns: u128,
    /// Combinação das unidades em um programa único, com remapeamento de spans.
    pub merge_ns: u128,
    /// Análise sintática completa de cada unidade com o ambiente nominal.
    pub parse_ns: u128,
    /// Expansão de macros incorporadas antes da semântica.
    pub macros_ns: u128,
    /// Expansão de mixins e validação semântica do programa combinado.
    pub analyze_ns: u128,
    /// Passes opcionais: constantes, fusão e tree shaking.
    pub optimize_ns: u128,
    /// Lowering para HIR e emissão do texto final.
    pub emit_ns: u128,
    /// Unidades que passaram pelo front-end nesta chamada.
    pub units: usize,
    /// Bytes de fonte lidos pelo front-end nesta chamada.
    pub source_bytes: usize,
    /// Tokens conservados após remover as diretivas.
    pub tokens: usize,
    /// Classes do programa combinado, incluindo aplicações de mixin sintéticas.
    pub classes: usize,
    /// Funções de topo do programa combinado.
    pub functions: usize,
    /// Bytes do texto produzido pelo backend.
    pub output_bytes: usize,
}
impl LinkStats {
    /// Soma das fases cronometradas; não inclui descoberta nem leitura do grafo.
    pub fn front_end_ns(&self) -> u128 {
        self.lex_ns
            + self.outline_ns
            + self.namespace_ns
            + self.merge_ns
            + self.parse_ns
            + self.macros_ns
            + self.analyze_ns
            + self.optimize_ns
            + self.emit_ns
    }
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
    compile_graph_with_macro_session(
        graph,
        optimize_constants,
        merge_identical,
        false,
        &mut dartforge_macros::MacroSession::with_limits(0, 0),
        emit,
    )
}

/// Resolve bibliotecas reutilizando somente planos de macro próprios e limitados.
/// # Erros
/// Preserva validação e localização dos diagnósticos mesmo quando o plano é reutilizado.
pub fn compile_graph_with_macro_session(
    graph: &SourceGraph,
    optimize_constants: bool,
    merge_identical: bool,
    tree_shaking: bool,
    macros: &mut dartforge_macros::MacroSession,
    emit: impl FnOnce(&dartforge_hir::Module<'_>) -> Result<String, Diagnostic>,
) -> Result<String, GraphError> {
    compile_graph_instrumented(
        graph,
        optimize_constants,
        merge_identical,
        tree_shaking,
        macros,
        &mut LinkStats::default(),
        emit,
    )
}

/// Compila registrando tempo por fase e trabalho realizado em `stats`.
///
/// A instrumentação é um contador por fase e não altera o resultado. Em erro,
/// `stats` conserva as fases já concluídas, o que permite localizar onde o
/// custo apareceu antes da falha.
///
/// # Erros
/// Idênticos aos de [`compile_graph_with_macro_session`].
pub fn compile_graph_instrumented(
    graph: &SourceGraph,
    optimize_constants: bool,
    merge_identical: bool,
    tree_shaking: bool,
    macros: &mut dartforge_macros::MacroSession,
    stats: &mut LinkStats,
    emit: impl FnOnce(&dartforge_hir::Module<'_>) -> Result<String, Diagnostic>,
) -> Result<String, GraphError> {
    if graph.entry >= graph.units.len() {
        return Err(GraphError {
            path: std::path::PathBuf::new(),
            span: None,
            message: "entrada do grafo inválida".into(),
        });
    }
    // Cada parte compartilha namespace, imports e privacidade da biblioteca declarante.
    let mut library: Vec<usize> = (0..graph.units.len()).collect();
    for (owner, unit) in graph.units.iter().enumerate() {
        for part in &unit.parts {
            if part.target >= graph.units.len()
                || part.target == owner
                || unit.part_of.is_some()
                || library[part.target] != part.target
                || graph.units[part.target].part_of != Some(owner)
            {
                return Err(source_error(
                    graph,
                    owner,
                    Diagnostic::new("destino de part inválido no grafo", part.span),
                ));
            }
            library[part.target] = owner;
        }
    }
    for (unit_id, unit) in graph.units.iter().enumerate() {
        if unit.part_of.is_some()
            && (library[unit_id] == unit_id || !unit.imports.is_empty() || !unit.exports.is_empty())
        {
            return Err(source_error(
                graph,
                unit_id,
                Diagnostic::new(
                    "parte exige uma biblioteca declarante e não pode ter import ou export",
                    Span { start: 0, end: 0 },
                ),
            ));
        }
    }
    if library[graph.entry] != graph.entry {
        return Err(source_error(
            graph,
            graph.entry,
            Diagnostic::new(
                "entrada não pode ser parte de outra biblioteca",
                Span { start: 0, end: 0 },
            ),
        ));
    }
    stats.units = graph.units.len();
    stats.source_bytes = graph.units.iter().map(|unit| unit.source.len()).sum();
    // Tokenização: trabalho independente por unidade, sem estado compartilhado.
    let lex_start = std::time::Instant::now();
    let tokens = map_units(graph, |unit| {
        let all = dartforge_lexer::lex(&unit.source)?;
        let prefix = unit
            .imports
            .iter()
            .chain(&unit.exports)
            .map(|import| import.span.end)
            .chain(unit.parts.iter().map(|part| part.span.end))
            .chain(std::iter::once(unit.directives_end))
            .max()
            .unwrap_or(0);
        Ok(all
            .into_iter()
            .filter(|token| token.span.start >= prefix)
            .collect::<Vec<_>>())
    });
    let tokens = first_error(graph, tokens)?;
    stats.lex_ns += lex_start.elapsed().as_nanos();
    stats.tokens = tokens.iter().map(Vec::len).sum();
    // Índice de declarações de topo: também independente por unidade.
    let outline_start = std::time::Instant::now();
    let declarations = map_indexed(tokens.len(), |unit_id| {
        dartforge_parser::index_unit(&tokens[unit_id])
    });
    let declarations = first_error(graph, declarations)?;
    stats.outline_ns += outline_start.elapsed().as_nanos();
    let namespace_start = std::time::Instant::now();
    // Índices de biblioteca; as unidades de partes permanecem com namespace vazio.
    let mut own: Vec<HashMap<&str, Symbol>> = vec![HashMap::new(); graph.units.len()];
    let mut next_class = 0u32;
    let mut class_origins = HashMap::new();
    for (unit_id, declaration) in declarations.iter().enumerate() {
        let owner = library[unit_id];
        for (names, is_class) in [
            (&declaration.classes, true),
            (&declaration.functions, false),
        ] {
            for item in names {
                if matches!(item.name, "print" | "int" | "String" | "bool") {
                    return Err(source_error(
                        graph,
                        unit_id,
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
                            unit_id,
                            Diagnostic::new("excesso de classes", item.span),
                        )
                    })?;
                    class_origins.insert(id, (owner, item.name));
                    Some(id)
                } else {
                    None
                };
                if own[owner]
                    .insert(
                        item.name,
                        Symbol {
                            owner,
                            class_id,
                            intrinsic: false,
                        },
                    )
                    .is_some()
                {
                    return Err(source_error(
                        graph,
                        unit_id,
                        Diagnostic::new("símbolo top-level duplicado", item.span),
                    ));
                }
            }
        }
    }
    for (unit_id, symbols) in own.iter_mut().enumerate() {
        if graph.units.iter().any(|unit| {
            unit.imports
                .iter()
                .chain(&unit.exports)
                .any(|edge| edge.uri == "dart:async" && edge.target == unit_id)
        }) {
            for name in ["Timer", "scheduleMicrotask"] {
                symbols.insert(
                    name,
                    Symbol {
                        owner: unit_id,
                        class_id: None,
                        intrinsic: true,
                    },
                );
            }
        }
    }
    let exported = exported_namespaces(graph, &own)?;
    let mut visible = own.clone();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        for import in &unit.imports {
            // Import com prefixo não entra no namespace sem qualificação: seus
            // nomes só existem em `prefixed`, alcançáveis por `p.nome`. Por isso
            // não há ambiguidade entre um nome com prefixo e outro sem.
            if import.prefix.is_some() {
                continue;
            }
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
    // Namespaces alcançáveis somente por prefixo. Um programa sem prefixo não
    // paga nada aqui: os dois vetores externos continuam vazios e sem alocação.
    let mut prefixed: Vec<Vec<Prefixed>> = Vec::new();
    let mut prefixes: Vec<Vec<&str>> = Vec::new();
    if graph
        .units
        .iter()
        .flat_map(|unit| &unit.imports)
        .any(|import| import.prefix.is_some())
    {
        prefixed = vec![Vec::new(); graph.units.len()];
        prefixes = vec![Vec::new(); graph.units.len()];
        for (unit_id, unit) in graph.units.iter().enumerate() {
            for import in &unit.imports {
                let Some(prefix) = import.prefix.as_deref() else {
                    continue;
                };
                // Um prefixo não pode competir com um nome já alcançável sem
                // qualificação: em Dart o mesmo texto não é os dois ao mesmo tempo.
                if visible[unit_id].contains_key(prefix) {
                    return Err(source_error(
                        graph,
                        unit_id,
                        Diagnostic::new(
                            format!(
                                "prefixo de import {prefix} colide com um nome visível nesta biblioteca"
                            ),
                            import.span,
                        ),
                    ));
                }
                prefixes[unit_id].push(prefix);
                let imported = exported.get(import.target).ok_or_else(|| {
                    source_error(
                        graph,
                        unit_id,
                        Diagnostic::new("destino de import inválido", import.span),
                    )
                })?;
                // `allows_name` já recusa `_nome`: a privacidade por biblioteca
                // não atravessa a fronteira, nem através de um prefixo.
                for (&name, &symbol) in imported {
                    if !allows_name(name, &import.combinators) {
                        continue;
                    }
                    prefixed[unit_id].push(Prefixed {
                        prefix,
                        name,
                        symbol,
                        span: import.span,
                    });
                }
            }
        }
        for (unit_id, entries) in prefixed.iter_mut().enumerate() {
            entries.sort_unstable_by(|left, right| {
                (left.prefix, left.name, left.symbol).cmp(&(right.prefix, right.name, right.symbol))
            });
            // Dois imports com o mesmo prefixo compõem um namespace único; o
            // mesmo nome vindo de declarações diferentes é ambíguo, como em Dart.
            if let Some(pair) = entries.windows(2).find(|pair| {
                pair[0].prefix == pair[1].prefix
                    && pair[0].name == pair[1].name
                    && pair[0].symbol.owner != pair[1].symbol.owner
            }) {
                return Err(source_error(
                    graph,
                    unit_id,
                    Diagnostic::new(
                        format!(
                            "import ambíguo: {}.{}; use show/hide para desambiguar",
                            pair[1].prefix, pair[1].name
                        ),
                        pair[1].span,
                    ),
                ));
            }
            entries.dedup_by(|right, left| left.prefix == right.prefix && left.name == right.name);
            prefixes[unit_id].sort_unstable();
            prefixes[unit_id].dedup();
        }
    }
    // A arena permanece imóvel até emissão; referências da AST nunca escapam desta função.
    let mut names = HashMap::new();
    for (unit_id, unit_tokens) in tokens.iter().enumerate() {
        let owner = library[unit_id];
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
    let mut reports = Vec::new();
    stats.namespace_ns += namespace_start.elapsed().as_nanos();
    // Ambientes nominais resolvidos em sequência; o parsing em si é independente.
    let parse_start = std::time::Instant::now();
    let environments: Vec<BTreeMap<&str, u32>> = (0..graph.units.len())
        .map(|unit_id| {
            visible[library[unit_id]]
                .iter()
                .filter_map(|(&name, symbol)| symbol.class_id.map(|id| (name, id)))
                .collect()
        })
        .collect();
    // Classes alcançáveis por prefixo, na ordem `(prefixo, nome)` herdada de
    // `prefixed`: o parser as consulta por busca binária. Uma parte herda as do
    // arquivo que a declara, porque herda também seus imports.
    let prefixed_classes: Vec<Vec<(&str, &str, u32)>> = if prefixed.is_empty() {
        Vec::new()
    } else {
        (0..graph.units.len())
            .map(|unit_id| {
                prefixed[library[unit_id]]
                    .iter()
                    .filter_map(|entry| {
                        entry
                            .symbol
                            .class_id
                            .map(|id| (entry.prefix, entry.name, id))
                    })
                    .collect()
            })
            .collect()
    };
    let parsed = map_indexed(graph.units.len(), |unit_id| {
        dartforge_parser::parse_unit_with_prefixed_classes(
            &tokens[unit_id],
            graph.units[unit_id].source.len(),
            environments[unit_id].clone(),
            prefixed_classes.get(unit_id).map_or(&[][..], Vec::as_slice),
        )
    });
    let mut programs = first_error(graph, parsed)?;
    stats.parse_ns += parse_start.elapsed().as_nanos();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        let owner = library[unit_id];
        let _ = unit;
        let program = &programs[unit_id];
        for annotation in program
            .functions
            .iter()
            .flat_map(|f| &f.annotations)
            .chain(program.classes.iter().flat_map(|c| &c.annotations))
            .chain(
                program
                    .classes
                    .iter()
                    .flat_map(|c| c.methods.iter().chain(&c.abstract_methods))
                    .flat_map(|f| &f.annotations),
            )
        {
            let name = metadata_name(annotation);
            if visible[owner].contains_key(name) {
                return Err(source_error(
                    graph,
                    unit_id,
                    Diagnostic::new(
                        "anotação sombreada exige resolução de constantes ainda não suportada",
                        annotation.span,
                    ),
                ));
            }
        }
        for function in &program.functions {
            if let Some(binding) = &function.native_binding {
                let imported = graph.units[owner]
                    .imports
                    .iter()
                    .any(|edge| edge.uri == "dart:ffi" && edge.prefix.as_deref() == binding.prefix);
                let shadowed = binding.prefix.map_or_else(
                    || {
                        ["Native", "Int32", "Int64", "Void"]
                            .iter()
                            .any(|name| visible[owner].contains_key(name))
                    },
                    |prefix| visible[owner].contains_key(prefix),
                );
                if !imported || shadowed {
                    return Err(source_error(
                        graph,
                        unit_id,
                        Diagnostic::new(
                            "@Native exige import dart:ffi correspondente, sem sombreamento",
                            binding.span,
                        ),
                    ));
                }
            }
        }
    }
    let macros_start = std::time::Instant::now();
    for (unit_id, program) in programs.iter_mut().enumerate() {
        reports.push(
            macros
                .expand(program, graph.units[unit_id].source.len())
                .map_err(|error| source_error(graph, unit_id, error))?,
        );
    }
    let mut classes = HashMap::new();
    for (unit_id, program) in programs.iter_mut().enumerate() {
        let owner = library[unit_id];
        for class in &mut program.classes {
            class.library_id = owner;
            classes.insert(
                class.id,
                ClassNames {
                    supports_implicit_members: true,
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
    for (owner, program) in programs.iter().enumerate() {
        for class in &program.classes {
            for annotation in class
                .methods
                .iter()
                .chain(&class.abstract_methods)
                .flat_map(|f| &f.annotations)
            {
                let name = metadata_name(annotation);
                let mut pending = vec![class.id];
                let mut seen = HashSet::new();
                while let Some(id) = pending.pop() {
                    if !seen.insert(id) {
                        continue;
                    }
                    if let Some(info) = classes.get(&id) {
                        if info.members.contains(name) {
                            return Err(source_error(
                                graph,
                                owner,
                                Diagnostic::new(
                                    "anotação sombreada por membro exige resolução de constantes ainda não suportada",
                                    annotation.span,
                                ),
                            ));
                        }
                        pending.extend(info.superclass);
                        pending.extend(&info.mixins);
                    }
                }
            }
        }
    }
    let entry_unit = (0..graph.units.len())
        .filter(|&unit_id| library[unit_id] == graph.entry)
        .find(|&unit_id| {
            programs[unit_id]
                .functions
                .iter()
                .any(|function| function.name == "main")
        })
        .ok_or_else(|| {
            source_error(
                graph,
                graph.entry,
                Diagnostic::new("entrada exige void main()", Span { start: 0, end: 0 }),
            )
        })?;
    let entry_function = programs[entry_unit]
        .functions
        .iter()
        .find(|function| function.name == "main")
        .expect("a unidade selecionada declara main");
    let future_void = matches!(entry_function.return_type, Type::Applied(id)
        if programs[entry_unit].types.get(id as usize) == Some(&TypeShape::Future(Type::Void)));
    if (entry_function.return_type != Type::Void && !(entry_function.is_async && future_void))
        || !entry_function.parameters.is_empty()
        || !entry_function.type_parameters.is_empty()
        || entry_function.native_binding.is_some()
    {
        return Err(source_error(
            graph,
            entry_unit,
            Diagnostic::new(
                "entrada exige void main() sem parâmetros",
                entry_function.span,
            ),
        ));
    }
    let entry_span = entry_function.span;
    let main_is_async = entry_function.is_async;
    let mut offsets = Vec::new();
    let mut offset = 0usize;
    for (unit, report) in graph.units.iter().zip(&reports) {
        offsets.push(offset);
        offset = report
            .extent
            .max(unit.source.len())
            .checked_add(1)
            .and_then(|extent| offset.checked_add(extent))
            .ok_or_else(|| {
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
            report: &reports[unit_id],
            types: &program.types,
            type_offset,
            unit: unit_id,
            library: library[unit_id],
            graph,
            visible: &visible,
            prefixed: prefixed
                .get(library[unit_id])
                .map_or(&[][..], Vec::as_slice),
            prefixes: prefixes
                .get(library[unit_id])
                .map_or(&[][..], Vec::as_slice),
            names: &names,
            classes: &classes,
            class_origins: &class_origins,
            scopes: vec![],
            current_class: None,
            factory_class: None,
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
                TypeShape::Map { key, value } => TypeShape::Map {
                    key: remap_type(*key, type_offset),
                    value: remap_type(*value, type_offset),
                },
                TypeShape::List(t) => TypeShape::List(remap_type(*t, type_offset)),
                TypeShape::Set(t) => TypeShape::Set(remap_type(*t, type_offset)),
                TypeShape::Future(t) => TypeShape::Future(remap_type(*t, type_offset)),
                TypeShape::Nullable(t) => TypeShape::Nullable(remap_type(*t, type_offset)),
                TypeShape::Record { positional, named } => TypeShape::Record {
                    positional: positional
                        .iter()
                        .map(|t| remap_type(*t, type_offset))
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, t)| (name.clone(), remap_type(*t, type_offset)))
                        .collect(),
                },
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
        main_is_arrow: false,
        main_is_async,
        types: linked_types,
        extensions: vec![],
        classes: vec![],
        functions: vec![],
        statements: vec![Statement {
            kind: entry_statement(
                Expr {
                    kind: ExprKind::Call {
                        name: entry_name,
                        arguments: vec![],
                    },
                    span: Span {
                        start: entry_span.start + offsets[entry_unit],
                        end: entry_span.end + offsets[entry_unit],
                    },
                },
                main_is_async,
            ),
            span: Span {
                start: entry_span.start + offsets[entry_unit],
                end: entry_span.end + offsets[entry_unit],
            },
        }],
    };
    stats.macros_ns += macros_start.elapsed().as_nanos();
    let merge_start = std::time::Instant::now();
    for program in programs {
        linked.classes.extend(program.classes);
        linked.functions.extend(program.functions);
    }
    stats.classes = linked.classes.len();
    stats.functions = linked.functions.len();
    stats.merge_ns += merge_start.elapsed().as_nanos();
    let analyze_start = std::time::Instant::now();
    let resolution = dartforge_hir::expand_mixins(&mut linked)
        .and_then(|()| dartforge_semantic::analyze_with_async_library(&linked, true))
        .map_err(|error| {
            let unit_id = offsets
                .iter()
                .rposition(|&start| start <= error.span.start)
                .unwrap_or(graph.entry);
            source_error(
                graph,
                unit_id,
                reports[unit_id].remap(Diagnostic::new(
                    error.message,
                    Span {
                        start: error.span.start.saturating_sub(offsets[unit_id]),
                        end: error.span.end.saturating_sub(offsets[unit_id]),
                    },
                )),
            )
        })?;
    stats.analyze_ns += analyze_start.elapsed().as_nanos();
    let optimize_start = std::time::Instant::now();
    if optimize_constants {
        dartforge_optimizer::fold_constants(&mut linked);
    }
    if merge_identical {
        dartforge_optimizer::merge_identical_functions(&mut linked, &resolution);
    }
    if tree_shaking {
        dartforge_optimizer::tree_shake(&mut linked, &resolution);
    }
    stats.optimize_ns += optimize_start.elapsed().as_nanos();
    let emit_start = std::time::Instant::now();
    let emitted = emit(&dartforge_hir::lower_resolved(linked, resolution)).map_err(|error| {
        let unit_id = offsets
            .iter()
            .rposition(|&start| start <= error.span.start)
            .unwrap_or(graph.entry);
        source_error(
            graph,
            unit_id,
            reports[unit_id].remap(Diagnostic::new(
                error.message,
                Span {
                    start: error.span.start.saturating_sub(offsets[unit_id]),
                    end: error.span.end.saturating_sub(offsets[unit_id]),
                },
            )),
        )
    })?;
    stats.emit_ns += emit_start.elapsed().as_nanos();
    stats.output_bytes = emitted.len();
    Ok(emitted)
}

/// Abaixo deste número de unidades o custo de coordenação supera o ganho.
const PARALLEL_UNIT_THRESHOLD: usize = 4;

/// Aplica `work` a cada unidade, em paralelo quando há unidades suficientes.
///
/// A ordem dos resultados é sempre a ordem das unidades: o paralelismo não pode
/// mudar identidades, IDs de classe nem qual diagnóstico é relatado primeiro.
fn map_units<'a, T: Send + 'a>(
    graph: &'a SourceGraph,
    work: impl Fn(&'a dartforge_packages::SourceUnit) -> Result<T, Diagnostic> + Send + Sync,
) -> Vec<Result<T, Diagnostic>> {
    if graph.units.len() < PARALLEL_UNIT_THRESHOLD {
        return graph.units.iter().map(work).collect();
    }
    use rayon::prelude::*;
    graph.units.par_iter().map(work).collect()
}

/// Igual a [`map_units`], mas indexado, para trabalho que consulta vetores já prontos.
fn map_indexed<T: Send>(
    count: usize,
    work: impl Fn(usize) -> Result<T, Diagnostic> + Send + Sync,
) -> Vec<Result<T, Diagnostic>> {
    if count < PARALLEL_UNIT_THRESHOLD {
        return (0..count).map(work).collect();
    }
    use rayon::prelude::*;
    (0..count).into_par_iter().map(work).collect()
}

/// Converte resultados por unidade no primeiro erro **em ordem de unidade**.
///
/// Coletar diretamente em `Result` devolveria um erro qualquer entre os que
/// falharam, tornando o diagnóstico dependente do escalonamento das threads.
fn first_error<T>(
    graph: &SourceGraph,
    results: Vec<Result<T, Diagnostic>>,
) -> Result<Vec<T>, GraphError> {
    let mut values = Vec::with_capacity(results.len());
    for (unit_id, result) in results.into_iter().enumerate() {
        match result {
            Ok(value) => values.push(value),
            Err(error) => return Err(source_error(graph, unit_id, error)),
        }
    }
    Ok(values)
}

/// Devolve o Future da entrada assíncrona sem inserir um segundo nó com o mesmo span.
fn entry_statement(expression: Expr<'_>, is_async: bool) -> StatementKind<'_> {
    if is_async {
        StatementKind::Return(Some(expression))
    } else {
        StatementKind::Expression(expression)
    }
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

/// Recupera o nome core reconhecido antes de qualquer renomeação de símbolos.
fn metadata_name(annotation: &dartforge_syntax::Annotation) -> &'static str {
    match &annotation.kind {
        dartforge_syntax::AnnotationKind::JsonCodable => "JsonCodable",
        dartforge_syntax::AnnotationKind::DataClass => "DataClass",
        dartforge_syntax::AnnotationKind::Override => "override",
        dartforge_syntax::AnnotationKind::Deprecated { message: None } => "deprecated",
        dartforge_syntax::AnnotationKind::Deprecated { message: Some(_) } => "Deprecated",
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
    report: &'g dartforge_macros::ExpansionReport,
    types: &'g [TypeShape],
    type_offset: u32,
    /// Arquivo analisado, usado em diagnósticos e no deslocamento de spans.
    unit: usize,
    /// Biblioteca dona dos símbolos, igual à unidade fora de uma parte.
    library: usize,
    graph: &'g SourceGraph,
    visible: &'g [HashMap<&'a str, Symbol>],
    /// Namespace alcançável por prefixo nesta biblioteca, ordenado por `(prefixo, nome)`.
    prefixed: &'g [Prefixed<'a>],
    /// Prefixos declarados pela biblioteca, ordenados, para recusar `p` sem ponto.
    prefixes: &'g [&'a str],
    names: &'a HashMap<(usize, &'a str), String>,
    classes: &'g HashMap<u32, ClassNames<'a>>,
    class_origins: &'g HashMap<u32, (usize, &'a str)>,
    scopes: Vec<HashSet<&'a str>>,
    current_class: Option<u32>,
    factory_class: Option<u32>,
    offset: usize,
}
impl<'a> Resolver<'a, '_> {
    /// Cria diagnóstico no arquivo corrente antes da mudança dos spans.
    fn error(&self, span: Span, message: impl Into<String>) -> GraphError {
        source_error(
            self.graph,
            self.unit,
            self.report.remap(Diagnostic::new(message, span)),
        )
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
        let mut pending: Vec<u32> = self
            .current_class
            .or(self.factory_class)
            .into_iter()
            .collect();
        let mut visited = HashSet::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            let Some(class) = self.classes.get(&id) else {
                continue;
            };
            if class.members.contains(name)
                && (!name.starts_with('_') || class.owner == self.library)
            {
                return true;
            }
            pending.extend(class.superclass);
            pending.extend(class.interfaces.iter().copied());
            pending.extend(class.mixins.iter().copied());
        }
        false
    }
    /// Busca binária no namespace alcançado por prefixo da biblioteca corrente.
    fn prefixed_symbol(&self, prefix: &str, name: &str) -> Option<Symbol> {
        self.prefixed
            .binary_search_by(|entry| (entry.prefix, entry.name).cmp(&(prefix, name)))
            .ok()
            .map(|index| self.prefixed[index].symbol)
    }
    /// Resolve `p.nome` ou recusa nomeando o prefixo e o nome exatos.
    ///
    /// Um `_nome` nunca chega ao vetor, então a mesma recusa cobre a
    /// privacidade por biblioteca: um prefixo não abre a fronteira.
    fn prefixed_lookup(&self, prefix: &str, name: &str, span: Span) -> Result<Symbol, GraphError> {
        self.prefixed_symbol(prefix, name).ok_or_else(|| {
            self.error(
                span,
                format!(
                    "nome não exportado pela biblioteca importada com prefixo {prefix}: {name}"
                ),
            )
        })
    }
    /// Informa se o nome é um prefixo de import declarado por esta biblioteca.
    fn is_prefix(&self, name: &str) -> bool {
        self.prefixes.binary_search(&name).is_ok()
    }
    /// Reconhece o identificador de um prefixo, sem confundi-lo com um local.
    fn prefix_of(&self, expression: &Expr<'a>) -> Option<&'a str> {
        match expression.kind {
            ExprKind::Identifier(name) if !self.local(name) && self.is_prefix(name) => Some(name),
            _ => None,
        }
    }
    /// Devolve o nome global de um símbolo importado, preservando intrínsecos.
    fn global_name(&self, symbol: Symbol, name: &'a str) -> &'a str {
        if symbol.intrinsic {
            name
        } else {
            self.names[&(symbol.owner, name)].as_str()
        }
    }
    /// Reescreve `p.nome` no nó que o nome importado exige.
    ///
    /// O parser não conhece prefixos em posição de expressão: `p.f()` chega como
    /// chamada de método sobre o identificador `p`, e `p.C.nomeada(...)` como
    /// chamada sobre um acesso a membro. Reescrever aqui concentra a diferença
    /// em um lugar e deixa renomeação, privacidade e construção no mesmo caminho
    /// dos nomes sem prefixo.
    ///
    /// Devolve `true` quando o nó já está resolvido por completo. Construtor
    /// nomeado e valor de enum voltam como `false` de propósito: os braços que
    /// já existem em [`Resolver::expression`] cuidam deles sem duplicação.
    fn prefixed_expression(&mut self, expression: &mut Expr<'a>) -> Result<bool, GraphError> {
        if self.prefixed.is_empty() {
            return Ok(false);
        }
        let span = expression.span;
        // A forma é decidida antes de mexer no nó: a consulta precisa de `&self`
        // e a substituição, de `&mut`.
        let shape = match &expression.kind {
            ExprKind::MethodCall { receiver, name, .. } => {
                if let Some(prefix) = self.prefix_of(receiver) {
                    let symbol = self.prefixed_lookup(prefix, name, span)?;
                    match symbol.class_id {
                        Some(class_id) => Shape::Construct(class_id),
                        None => Shape::Call(self.global_name(symbol, *name)),
                    }
                } else if let ExprKind::Member {
                    receiver: owner,
                    name: class,
                } = &receiver.kind
                {
                    let Some(prefix) = self.prefix_of(owner) else {
                        return Ok(false);
                    };
                    Shape::Named(self.prefixed_class(prefix, class, span)?, *name)
                } else {
                    return Ok(false);
                }
            }
            ExprKind::Member { receiver, name } => {
                if let Some(prefix) = self.prefix_of(receiver) {
                    let symbol = self.prefixed_lookup(prefix, name, span)?;
                    if symbol.class_id.is_some() {
                        return Err(self
                            .error(span, format!("{prefix}.{name} é uma classe e não um valor")));
                    }
                    Shape::Value(self.global_name(symbol, *name))
                } else if let ExprKind::Member {
                    receiver: owner,
                    name: class,
                } = &receiver.kind
                {
                    let Some(prefix) = self.prefix_of(owner) else {
                        return Ok(false);
                    };
                    Shape::Enum(self.prefixed_class(prefix, class, span)?, *name)
                } else {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        };
        let mut arguments = match &mut expression.kind {
            ExprKind::MethodCall { arguments, .. } => std::mem::take(arguments),
            _ => Vec::new(),
        };
        // Construtor nomeado e valor de enum voltam aos braços que já existem,
        // que aplicam privacidade, renomeação de membro e verificação de tipo.
        match shape {
            Shape::Named(class_id, name) => {
                expression.kind = ExprKind::NamedConstruct {
                    class_id,
                    name,
                    arguments,
                };
                return Ok(false);
            }
            Shape::Enum(class_id, name) => {
                expression.kind = ExprKind::EnumValue { class_id, name };
                return Ok(false);
            }
            _ => {}
        }
        for argument in &mut arguments {
            self.expression(argument)?;
        }
        expression.kind = match shape {
            Shape::Call(name) => ExprKind::Call { name, arguments },
            Shape::Construct(class_id) => ExprKind::Construct {
                class_id,
                arguments,
            },
            Shape::Value(name) => ExprKind::Identifier(name),
            Shape::Named(..) | Shape::Enum(..) => unreachable!("devolvidos acima"),
        };
        self.span(&mut expression.span);
        Ok(true)
    }
    /// Exige que `p.Nome` seja uma classe antes de construir ou ler um estático.
    fn prefixed_class(
        &self,
        prefix: &'a str,
        class: &'a str,
        span: Span,
    ) -> Result<u32, GraphError> {
        self.prefixed_lookup(prefix, class, span)?
            .class_id
            .ok_or_else(|| self.error(span, format!("{prefix}.{class} não é uma classe")))
    }
    /// Retorna um nome privado pertencente à biblioteca corrente.
    fn member_name(&self, name: &'a str) -> &'a str {
        if name.starts_with('_') {
            self.names[&(self.library, name)].as_str()
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
                TypeShape::Map { key, value } => {
                    self.ty(*key, span)?;
                    self.ty(*value, span)?;
                }
                TypeShape::Record { positional, named } => {
                    for t in positional.iter().chain(named.iter().map(|(_, t)| t)) {
                        self.ty(*t, span)?;
                    }
                }
                TypeShape::List(t)
                | TypeShape::Set(t)
                | TypeShape::Iterable(t)
                | TypeShape::Nullable(t)
                | TypeShape::Future(t) => self.ty(*t, span)?,
                TypeShape::Function { result, parameters } => {
                    self.ty(*result, span)?;
                    for t in parameters {
                        self.ty(*t, span)?;
                    }
                }
            }
        }
        if ty == Type::Timer
            && !self.visible[self.library]
                .get("Timer")
                .is_some_and(|symbol| symbol.intrinsic)
        {
            return Err(self.error(span, "Timer requires a visible dart:async import"));
        }
        let name = match ty {
            Type::Timer => Some("Timer"),
            Type::Duration => Some("Duration"),
            Type::Class(id) | Type::NullableClass(id) => {
                self.class_origins.get(&id).map(|(_, name)| *name)
            }
            Type::Int | Type::NullableInt => Some("int"),
            Type::String | Type::NullableString => Some("String"),
            Type::Bool | Type::NullableBool => Some("bool"),
            Type::Object | Type::NullableObject => Some("Object"),
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
        for annotation in &mut class.annotations {
            self.span(&mut annotation.span);
        }
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
            if let Some(initializer) = &mut field.initializer {
                self.expression(initializer)?;
            }
            field.name = self.member_name(field.name);
            self.span(&mut field.span);
        }
        for interface in &class.interfaces {
            self.ty(Type::Class(*interface), class.span)?;
        }
        if let Some(constructor) = &mut class.constructor {
            for parameter in &mut constructor.parameters {
                self.ty(parameter.ty, parameter.span)?;
                parameter.ty = remap_type(parameter.ty, self.type_offset);
                if let Some(field) = &mut parameter.field {
                    *field = self.member_name(field);
                }
                // O valor padrão é código da biblioteca de origem: sem percorrê-lo
                // os spans ficariam no espaço da unidade e poderiam colidir com o
                // span já remapeado de outra unidade na tabela de constantes.
                if let Some(default) = &mut parameter.default {
                    self.expression(default)?;
                }
                self.span(&mut parameter.span);
            }
            self.scopes.push(
                constructor
                    .parameters
                    .iter()
                    .filter(|p| p.field.is_none())
                    .map(|p| p.name)
                    .collect(),
            );
            self.block(&mut constructor.body)?;
            self.scopes.pop();
            self.span(&mut constructor.span);
        }
        for method in class.methods.iter_mut().chain(&mut class.abstract_methods) {
            if method.name == class.name {
                return Err(self.error(method.span, "método não pode ter o nome da classe"));
            }
            self.function(method, false)?;
        }
        self.current_class = None;
        self.factory_class = Some(class.id);
        for factory in &mut class.factories {
            self.function(factory, false)?;
        }
        self.factory_class = None;
        class.name = self.names[&(self.library, class.name)].as_str();
        self.span(&mut class.span);
        self.current_class = None;
        Ok(())
    }
    /// Resolve assinatura fora do escopo de parâmetros e corpo em escopo aninhado.
    fn function(&mut self, function: &mut Function<'a>, top_level: bool) -> Result<(), GraphError> {
        for parameter in &mut function.type_parameters {
            self.ty(parameter.bound, parameter.span)?;
            parameter.bound = remap_type(parameter.bound, self.type_offset);
            self.span(&mut parameter.span);
        }
        if let Some(binding) = &mut function.native_binding {
            self.span(&mut binding.span);
        }
        for annotation in &mut function.annotations {
            self.span(&mut annotation.span);
        }
        self.ty(function.return_type, function.span)?;
        function.return_type = remap_type(function.return_type, self.type_offset);
        for parameter in &mut function.parameters {
            self.ty(parameter.ty, parameter.span)?;
            parameter.ty = remap_type(parameter.ty, self.type_offset);
            // Ver a nota em `class`: o padrão também precisa de spans remapeados.
            if let Some(default) = &mut parameter.default {
                self.expression(default)?;
            }
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
            self.names[&(self.library, function.name)].as_str()
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
                .flat_map(|statement| match &statement.kind {
                    StatementKind::Variable { name, .. } => vec![*name],
                    StatementKind::RecordDestructure {
                        positional, named, ..
                    } => positional
                        .iter()
                        .map(|(n, _)| *n)
                        .chain(named.iter().map(|(_, n, _)| *n))
                        .filter(|n| *n != "_")
                        .collect(),
                    _ => vec![],
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
            StatementKind::RecordDestructure {
                positional,
                named,
                initializer,
                ..
            } => {
                self.expression(initializer)?;
                for (_, span) in positional {
                    self.span(span);
                }
                for (_, _, span) in named {
                    self.span(span);
                }
            }
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
            StatementKind::Labeled { body, .. } => self.statement(body)?,
            StatementKind::ForIn {
                name,
                iterable,
                body,
                ..
            } => {
                self.expression(iterable)?;
                let mut scope = HashSet::new();
                scope.insert(*name);
                self.scopes.push(scope);
                self.block(body)?;
                self.scopes.pop();
            }
            StatementKind::Assert { condition, message } => {
                self.expression(condition)?;
                if let Some(message) = message {
                    self.expression(message)?;
                }
            }
            StatementKind::Try {
                body,
                catches,
                finally_body,
            } => {
                self.block(body)?;
                for clause in catches {
                    let mut scope = HashSet::new();
                    scope.extend(clause.exception);
                    scope.extend(clause.stack_trace);
                    self.scopes.push(scope);
                    self.block(&mut clause.body)?;
                    self.scopes.pop();
                }
                if let Some(body) = finally_body {
                    self.block(body)?;
                }
            }
            StatementKind::Break
            | StatementKind::Continue
            | StatementKind::BreakLabel(_)
            | StatementKind::ContinueLabel(_)
            | StatementKind::Rethrow => {}
        }
        self.span(&mut statement.span);
        Ok(())
    }
    /// Resolve chamadas e construtores sem reescrever texto nem capturar globais indevidos.
    fn expression(&mut self, expression: &mut Expr<'a>) -> Result<(), GraphError> {
        if self.prefixed_expression(expression)? {
            return Ok(());
        }
        // Um prefixo não é um identificador comum: só `p.nome` alcança o
        // namespace importado, e um nome de topo com o mesmo texto já foi
        // recusado na construção do namespace.
        if let ExprKind::Identifier(name)
        | ExprKind::Call { name, .. }
        | ExprKind::GenericCall { name, .. } = &expression.kind
            && !self.local(name)
            && !self.implicit_member(name)
            && self.is_prefix(name)
        {
            return Err(self.error(
                expression.span,
                format!("prefixo de import {name} exige um nome: use {name}.nome"),
            ));
        }
        if let ExprKind::Identifier(name @ ("Timer" | "scheduleMicrotask")) = &expression.kind
            && !self.local(name)
            && !self.implicit_member(name)
            && !self.visible[self.library].contains_key(name)
        {
            return Err(self.error(
                expression.span,
                "async intrinsic requires a visible dart:async import",
            ));
        }
        if self.factory_class.is_some() {
            let name = match &expression.kind {
                ExprKind::Identifier(name)
                | ExprKind::Call { name, .. }
                | ExprKind::GenericCall { name, .. } => Some(*name),
                _ => None,
            };
            if name.is_some_and(|name| !self.local(name) && self.implicit_member(name)) {
                return Err(self.error(
                    expression.span,
                    "instance members cannot be accessed from a factory",
                ));
            }
        }
        if let ExprKind::GenericCall { type_arguments, .. } = &mut expression.kind {
            for t in type_arguments {
                self.ty(*t, expression.span)?;
                *t = remap_type(*t, self.type_offset);
            }
        }
        match &mut expression.kind {
            ExprKind::Await(value) => self.expression(value)?,
            ExprKind::FutureValue { value, value_type } => {
                if let Some(ty) = value_type {
                    self.ty(*ty, expression.span)?;
                    *ty = remap_type(*ty, self.type_offset);
                }
                if let Some(value) = value {
                    self.expression(value)?;
                }
            }
            ExprKind::FutureDelayed {
                duration,
                computation,
                value_type,
            } => {
                if let Some(ty) = value_type {
                    self.ty(*ty, expression.span)?;
                    *ty = remap_type(*ty, self.type_offset);
                }
                self.expression(duration)?;
                if let Some(value) = computation {
                    self.expression(value)?;
                }
            }
            ExprKind::Duration { parts } => {
                for (_, value) in parts {
                    self.expression(value)?;
                }
            }
            ExprKind::Cascade {
                receiver, sections, ..
            } => {
                self.expression(receiver)?;
                for section in sections {
                    self.statement(section)?;
                }
            }
            ExprKind::CascadeReceiver => {}
            ExprKind::Record { fields } => {
                for (_, field) in fields {
                    self.expression(field)?;
                }
            }
            ExprKind::TypeTest { operand, ty, .. } | ExprKind::Cast { operand, ty } => {
                self.ty(*ty, expression.span)?;
                *ty = remap_type(*ty, self.type_offset);
                self.expression(operand)?;
            }
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
                    let symbol = self.visible[self.library].get(name).ok_or_else(|| {
                        self.error(
                            expression.span,
                            format!("função não visível nesta biblioteca: {name}"),
                        )
                    })?;
                    if symbol.class_id.is_some() {
                        return Err(self.error(expression.span, "classe usada como função"));
                    }
                    if !symbol.intrinsic {
                        *name = self.names[&(symbol.owner, *name)].as_str();
                    }
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
            ExprKind::Map {
                key_type,
                value_type,
                entries,
            } => {
                for ty in [key_type, value_type].into_iter().flatten() {
                    self.ty(*ty, expression.span)?;
                    *ty = remap_type(*ty, self.type_offset);
                }
                for (key, value) in entries {
                    self.expression(key)?;
                    // `None` marca elemento de controle: só a chave é real.
                    if let Some(value) = value {
                        self.expression(value)?;
                    }
                }
            }
            ExprKind::Set {
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
            ExprKind::Spread { operand, .. } => self.expression(operand)?,
            ExprKind::MapEntry { key, value } => {
                self.expression(key)?;
                self.expression(value)?;
            }
            ExprKind::CollectionIf {
                condition,
                then_element,
                else_element,
            } => {
                self.expression(condition)?;
                self.expression(then_element)?;
                if let Some(element) = else_element {
                    self.expression(element)?;
                }
            }
            ExprKind::CollectionFor { header, element } => {
                self.statement(header)?;
                self.expression(element)?;
            }
            ExprKind::NullShort {
                receiver, chain, ..
            } => {
                self.expression(receiver)?;
                self.expression(chain)?;
            }
            ExprKind::NamedConstruct {
                class_id,
                name,
                arguments,
            } => {
                self.ty(Type::Class(*class_id), expression.span)?;
                let (owner, _) = self
                    .class_origins
                    .get(class_id)
                    .ok_or_else(|| self.error(expression.span, "unknown factory class"))?;
                if name.starts_with('_') && *owner != self.library {
                    return Err(self.error(
                        expression.span,
                        "private factory belongs to another library",
                    ));
                }
                *name = self.member_name(name);
                for argument in arguments {
                    self.expression(argument)?;
                }
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
                if let Some(symbol) = self.visible[self.library].get(name)
                    && symbol.class_id.is_none()
                    && !symbol.intrinsic
                {
                    *name = self.names[&(symbol.owner, *name)].as_str();
                }
            }
            ExprKind::Construct {
                class_id,
                arguments,
            } => {
                for argument in arguments {
                    self.expression(argument)?;
                }
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
                if name.starts_with('_') && *owner != self.library {
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
            // Braço mínimo de recursão para a variante de incremento: o alvo é
            // um identificador e precisa passar pelo mesmo caminho de qualquer
            // outro — sem isto, `x++` num programa com mais de uma biblioteca
            // perderia a renomeação de membro privado e o span remapeado.
            ExprKind::Increment { target, .. } => self.expression(target)?,
            ExprKind::Binary { left, right, .. } => {
                self.expression(left)?;
                self.expression(right)?;
            }
            // Os trechos literais não referenciam nada; as expressões, sim, e
            // sem esta recursão um nome importado dentro de '${...}' ficaria
            // sem qualificação de biblioteca e sem span remapeado.
            ExprKind::Interpolation(parts) => {
                for part in parts {
                    if let dartforge_syntax::StringPart::Expression(value) = part {
                        self.expression(value)?;
                    }
                }
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
    /// Futuras, callbacks e entrada async atravessam unidades com spans e tipos próprios.
    #[test]
    fn async_graph_traverses_futures_and_propagates_entry() {
        let mut graph = graph(&[
            (
                "Future<void> main() async{await value();Timer(Duration.zero,()=>print(2));scheduleMicrotask(()=>print(3));}",
                &[1, 2],
            ),
            (
                "Future<int> value() async=>await Future<int>.delayed(Duration(milliseconds:1),() async=>4);",
                &[],
            ),
            ("", &[]),
        ]);
        graph.units[0].imports[1].uri = "dart:async".into();
        compile_graph_with(&graph, false, |module| {
            assert!(module.main_is_async);
            assert!(matches!(
                module.statements[0].kind,
                StatementKind::Return(Some(_))
            ));
            assert!(
                module
                    .functions
                    .iter()
                    .filter(|function| function.is_async)
                    .count()
                    >= 2
            );
            Ok(String::new())
        })
        .unwrap();
    }
    /// O namespace async respeita combinadores, reexports, sombras locais e ambiguidades.
    #[test]
    fn async_intrinsics_require_visible_library_symbols() {
        for source in [
            "void main(){Timer(Duration.zero,()=>print(1));}",
            "void main(){var f=scheduleMicrotask;}",
            "void main(){Timer? timer=null;}",
        ] {
            assert!(compile_graph(&graph(&[(source, &[])]), false).is_err());
        }
        let mut reexport = graph(&[
            ("void main(){scheduleMicrotask(()=>print(1));}", &[1]),
            ("", &[]),
            ("", &[]),
        ]);
        let mut exported = edge(2, vec![Combinator::Show(vec!["scheduleMicrotask".into()])]);
        exported.uri = "dart:async".into();
        reexport.units[1].exports.push(exported);
        assert!(compile_graph(&reexport, false).is_ok());
        reexport.units[0].source = "void main(){Timer(Duration.zero,()=>print(1));}".into();
        assert!(compile_graph(&reexport, false).is_err());
        let mut ambiguity = graph(&[
            ("void main(){}", &[1, 2]),
            ("", &[]),
            ("void scheduleMicrotask(){}", &[]),
        ]);
        ambiguity.units[0].imports[0].uri = "dart:async".into();
        assert!(
            compile_graph(&ambiguity, false)
                .unwrap_err()
                .message
                .contains("ambíguo")
        );
        let own = graph(&[(
            "void scheduleMicrotask(){}void main(){scheduleMicrotask();}",
            &[],
        )]);
        assert!(compile_graph(&own, false).is_ok());
    }
    /// Expansão por biblioteca preserva chaves JSON privadas e remapeia os tipos Map.
    #[test]
    fn imported_json_macro_and_factory_types_are_linked() {
        let graph = graph(&[
            (
                "void main(){var user=User.fromJson({'_name':'Ana'});print(user.toJson()['_name'] as String);}",
                &[1],
            ),
            ("@JsonCodable() class User{final String _name;}", &[]),
        ]);
        for optimize in [false, true] {
            let js = compile_graph(&graph, optimize).unwrap();
            assert!(js.contains("_name"));
        }
        let error = compile_graph_with(&graph, false, |module| {
            Err(Diagnostic::new(
                "generated backend failure",
                module.classes[0].factories[0].span,
            ))
        })
        .unwrap_err();
        assert_eq!(error.path, graph.units[1].path);
        assert_eq!(
            error.span,
            Some(Span {
                start: 0,
                end: "@JsonCodable()".len()
            })
        );
        assert!(error.message.contains("JsonCodable"));
    }
    /// Fábricas privadas não atravessam bibliotecas e seus corpos não recebem this implícito.
    #[test]
    fn named_factory_privacy_and_static_scope() {
        let graph = graph(&[
            ("void main(){C._make();}", &[1]),
            ("class C{factory C._make()=>C();}", &[]),
        ]);
        assert!(
            compile_graph(&graph, false)
                .unwrap_err()
                .message
                .contains("private factory")
        );
        let graph = self::graph(&[
            ("void main(){print(C.make().value);}", &[1]),
            (
                "int value()=>3;class C{int value=1;C(int v){value=v;}factory C.make()=>C(value());}",
                &[],
            ),
        ]);
        assert!(
            compile_graph(&graph, false)
                .unwrap_err()
                .message
                .contains("instance members")
        );
    }
    /// O mapa produzido mantém a chave original mesmo com renomeação do campo privado.
    #[test]
    #[ignore = "requer Node.js no PATH"]
    fn imported_json_private_key_roundtrip_executes() {
        let graph = graph(&[
            (
                "void main(){var user=User.fromJson({'_name':'Ana'});print(user.toJson()['_name'] as String);}",
                &[1],
            ),
            ("@JsonCodable() class User{final String _name;}", &[]),
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
            assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), "Ana");
        }
    }
    /// Cascades importados remapeiam chamadas e campos privados sem tocar no receptor sintético.
    #[test]
    fn linked_cascade_sections_resolve_private_members_and_global_calls() {
        let graph = graph(&[
            ("void main(){print(make().read());}", &[1]),
            (
                "class Box{int _value=0;int read()=>_value;} int _effect()=>2; Box make()=>Box().._value=_effect()+3;",
                &[],
            ),
        ]);
        for optimize in [false, true] {
            let output = compile_graph(&graph, optimize).unwrap();
            assert!(output.contains("$dartforgeCascade"));
        }
    }
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
    /// Partes compartilham o namespace da biblioteca e exigem um grafo coerente.
    #[test]
    fn graph_parts_share_the_library_namespace() {
        let mut sources = graph(&[
            ("void main(){print(ajuda()+_segredo());}", &[]),
            ("int ajuda()=>1; int _segredo()=>2;", &[]),
        ]);
        // Sem part a segunda unidade é outra biblioteca e nada dela fica visível.
        assert!(
            compile_graph(&sources, false)
                .unwrap_err()
                .message
                .contains("não visível")
        );
        sources.units[0].parts.push(dartforge_packages::Part {
            uri: "library_1.dart".into(),
            target: 1,
            span: Span { start: 0, end: 0 },
        });
        sources.units[1].part_of = Some(0);
        for optimize in [false, true] {
            let js = compile_graph(&sources, optimize).unwrap();
            assert!(js.contains("$lib0$ajuda"));
            assert!(js.contains("$lib0$_segredo"));
            assert!(!js.contains("$lib1$"));
        }
        // Reivindicação divergente, import na parte e entrada como parte são erros.
        sources.units[1].part_of = Some(1);
        assert!(compile_graph(&sources, false).is_err());
        sources.units[1].part_of = Some(0);
        sources.units[1].imports.push(edge(0, vec![]));
        assert!(compile_graph(&sources, false).is_err());
        sources.units[1].imports.clear();
        sources.entry = 1;
        assert!(
            compile_graph(&sources, false)
                .unwrap_err()
                .message
                .contains("entrada não pode ser parte")
        );
    }
    /// Monta um grafo em memória com imports representados pelas arestas declaradas.
    fn graph(sources: &[(&str, &[usize])]) -> SourceGraph {
        SourceGraph {
            environment: dartforge_packages::CompilationEnvironment::javascript(),
            entry: 0,
            config_origin: None,
            units: sources
                .iter()
                .enumerate()
                .map(|(id, (source, targets))| dartforge_packages::SourceUnit {
                    path: PathBuf::from(format!("library_{id}.dart")),
                    source: (*source).into(),
                    exports: vec![],
                    parts: vec![],
                    part_of: None,
                    directives_end: 0,
                    imports: targets
                        .iter()
                        .map(|&target| dartforge_packages::Import {
                            prefix: None,
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
            prefix: None,
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
        let source = "class A {int f(){return 1;} int g(){return f();}} void main(){}";
        let js = compile_graph(
            &graph(&[(source, &[1]), ("int f(){return 9;}", &[])]),
            false,
        )
        .unwrap();
        assert!(js.contains("return this.$df_f();"));
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
            let js = compile_graph(&graph, optimize).unwrap();
            assert!(js.contains("return this.$df_f();"));
            assert!(!js.contains("return $df_$lib0$f();"));
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
