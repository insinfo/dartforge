//! Liga bibliotecas Dart do grafo por símbolos e ASTs, sem concatenar fontes.
//! Cada arquivo constitui uma biblioteca, com imports diretos e privacidade por unidade.
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_packages::{GraphError, SourceGraph};
use dartforge_syntax::{
    Class, Expr, ExprKind, Function, Program, Statement, StatementKind, TokenKind, Type,
};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Símbolo declarado por uma biblioteca antes da análise de corpos.
#[derive(Clone, Copy)]
struct Symbol {
    owner: usize,
    class_id: Option<u32>,
}
/// Metadados imutáveis usados para verificar resolução antes da renomeação.
struct ClassNames<'a> {
    owner: usize,
    superclass: Option<u32>,
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
    let mut visible = own.clone();
    for (unit_id, unit) in graph.units.iter().enumerate() {
        for import in &unit.imports {
            let imported = own.get(import.target).ok_or_else(|| {
                source_error(
                    graph,
                    unit_id,
                    Diagnostic::new("destino de import inválido", import.span),
                )
            })?;
            // A ordem lexical evita que a semente do HashMap escolha o primeiro erro.
            let mut imported_names: Vec<_> = imported.iter().collect();
            imported_names.sort_unstable_by_key(|(name, _)| **name);
            for (&name, &symbol) in imported_names {
                if name.starts_with('_') || own[unit_id].contains_key(name) {
                    continue;
                }
                if let Some(previous) = visible[unit_id].get(name)
                    && previous.owner != symbol.owner
                {
                    return Err(source_error(
                        graph,
                        unit_id,
                        Diagnostic::new(
                            format!(
                                "import ambíguo: {name}; prefixos e combinadores ainda não suportados"
                            ),
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
    for (owner, program) in programs.iter().enumerate() {
        for class in &program.classes {
            classes.insert(
                class.id,
                ClassNames {
                    owner,
                    superclass: class.superclass,
                    members: class
                        .fields
                        .iter()
                        .map(|field| field.name)
                        .chain(class.methods.iter().map(|method| method.name))
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
    for (unit_id, program) in programs.iter_mut().enumerate() {
        let mut resolver = Resolver {
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
    }
    let entry_name = names[&(graph.entry, "main")].as_str();
    let mut linked = Program {
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
    dartforge_semantic::validate(&linked).map_err(|error| {
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
    Ok(dartforge_codegen::emit(&dartforge_hir::lower(linked)))
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
struct Resolver<'a, 'g> {
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
        let mut cursor = self.current_class;
        let mut visited = HashSet::new();
        while let Some(id) = cursor {
            if !visited.insert(id) {
                break;
            }
            let Some(class) = self.classes.get(&id) else {
                break;
            };
            if class.members.contains(name) && (!name.starts_with('_') || class.owner == self.unit)
            {
                return true;
            }
            cursor = class.superclass;
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
        for field in &mut class.fields {
            if field.name == class.name {
                return Err(self.error(field.span, "membro não pode ter o nome da classe"));
            }
            self.ty(field.ty, field.span)?;
            self.expression(&mut field.initializer)?;
            field.name = self.member_name(field.name);
            self.span(&mut field.span);
        }
        for method in &mut class.methods {
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
        for parameter in &mut function.parameters {
            self.ty(parameter.ty, parameter.span)?;
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
    fn statement(&mut self, statement: &mut Statement<'a>) -> Result<(), GraphError> {
        match &mut statement.kind {
            StatementKind::Variable {
                annotation,
                initializer,
                ..
            } => {
                if let Some(ty) = annotation {
                    self.ty(*ty, statement.span)?;
                }
                self.expression(initializer)?;
            }
            StatementKind::Assign { name, value } => {
                if !self.local(name) {
                    return Err(
                        self.error(statement.span, "atribuição exige variável local declarada")
                    );
                }
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
        match &mut expression.kind {
            ExprKind::Call { name, arguments } => {
                if self.local(name) {
                    return Err(
                        self.error(expression.span, "chamada de variável local não suportada")
                    );
                }
                if self.implicit_member(name) {
                    return Err(self.error(
                        expression.span,
                        "chamada de membro exige receptor this explícito",
                    ));
                }
                if *name != "print" {
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
                    imports: targets
                        .iter()
                        .map(|&target| dartforge_packages::Import {
                            uri: format!("library_{target}.dart"),
                            target,
                            span: Span { start: 0, end: 0 },
                        })
                        .collect(),
                })
                .collect(),
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
                "import ambíguo: alpha; prefixos e combinadores ainda não suportados"
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
