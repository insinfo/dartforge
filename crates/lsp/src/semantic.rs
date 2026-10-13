//! O analisador semântico do LSP. O programa, as árvores e a tabela de tipos
//! de uma consulta ficam, no máximo, na sessão limitada de [`crate::session`]
//! (uma entrada, descartada a cada mudança de texto); o completar, que
//! analisa um texto com sentinela, é sempre transitório.

use crate::{Analyzer, SyntacticAnalyzer, DocumentStore, Hover, References};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{gerado::Construtor, load::load_lenient_gerados, model::{Program, UnitId}, sdk::SdkLayout};
use dartforge_intern::Interner;
use url::Url;

/// Analisador com a resolução e os tipos da inferência comum: definição,
/// hover e referências pela identidade da declaração (`crate::project`),
/// completar, renomear e ações. Na falta do SDK, mantém as respostas
/// sintáticas.
pub struct SemanticAnalyzer {
    syntactic: SyntacticAnalyzer,
    sdk: Option<SdkLayout>,
    /// Nomes públicos de topo do SDK (importar biblioteca), montado na
    /// primeira vez que um nome indefinido pede. Tamanho fixo pelo SDK.
    sdk_index: Option<crate::index::SdkIndex>,
    /// Nomes públicos de topo do projeto do último documento consultado,
    /// atualizado por arquivo (importação automática e ação de importar).
    project_index: crate::index::ProjectIndex,
    /// O último programa carregado, reaproveitado enquanto nada mudou.
    session: crate::session::Session,
    /// A carga ampla das buscas não locais (`references`, `implementation`:
    /// o projeto, o SDK inteiro e os pacotes), retida à parte da sessão
    /// principal para não ser trocada a cada `hover`; os corpos das
    /// bibliotecas de fora do projeto são inferidos sob demanda e ficam
    /// (docs/LSP-ESPECIFICACAO.md §11.9 B).
    wide_session: crate::session::Session,
    /// Os resumos das bibliotecas conhecidas (namespace de exportação) do
    /// `ImportLibrary`.
    known: crate::known_libraries::LibraryIndex,
    /// O `maxCompletionItems` vigente.
    completion_max: usize,
    not_imported_in_completion: bool,
    /// O caminho incremental (docs/LSP-ESPECIFICACAO.md §16.10 I3 e I4):
    /// `didChange` só de corpos troca a unidade retida no lugar, e o
    /// completar infere só o corpo do sentinela. Desligado por
    /// `DARTFORGE_LSP_INCREMENTAL=0` ou [`SemanticAnalyzer::without_incremental`]
    /// (a comparação com o caminho completo).
    incremental: bool,
}

impl SemanticAnalyzer {
    /// Analisador com o SDK dado; sem SDK, só as respostas sintáticas.
    ///
    /// ```
    /// let a = dartforge_lsp::SemanticAnalyzer::new(None);
    /// assert_eq!(a.session_stats().loaded, 0);
    /// ```
    pub fn new(sdk: Option<SdkLayout>) -> Self {
        Self { syntactic: SyntacticAnalyzer::new(), sdk, sdk_index: None, project_index: crate::index::ProjectIndex::default(), session: crate::session::Session::new(), wide_session: crate::session::Session::wide(), known: Default::default(), completion_max: crate::completion::MAX_PATTERN, not_imported_in_completion: true, incremental: std::env::var("DARTFORGE_LSP_INCREMENTAL").map(|v| v.trim() != "0").unwrap_or(true) }
    }

    /// Troca o orçamento da sessão semântica (MiB de fonte retida; `0`
    /// desliga a retenção). O padrão vem de `DARTFORGE_LSP_SESSAO_MIB`.
    ///
    /// ```
    /// use dartforge_lsp::SemanticAnalyzer;
    /// let a = SemanticAnalyzer::new(None).with_session_budget(0);
    /// assert_eq!(a.session_stats().retained_source, 0);
    /// ```
    pub fn with_session_budget(mut self, mib: usize) -> Self {
        self.session = crate::session::Session::with_budget(mib);
        self
    }

    /// Sem o caminho incremental: cada edição derruba a sessão e o completar
    /// carrega e infere a biblioteca inteira.
    ///
    /// ```
    /// let a = dartforge_lsp::SemanticAnalyzer::new(None).without_incremental();
    /// assert_eq!(a.session_stats().body_swaps, 0);
    /// ```
    pub fn without_incremental(mut self) -> Self {
        self.incremental = false;
        self
    }

    /// Contadores da sessão semântica (reaproveitamentos, cargas,
    /// invalidações, fonte retida).
    pub fn session_stats(&self) -> crate::session::SessionStats {
        self.session.stats
    }

    /// A biblioteca de `uri` (definição, hover, ações), da sessão ou carregada.
    pub(crate) fn library(&mut self, documents: &DocumentStore, uri: &str) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let file = crate::project::uri_file(uri)?;
        self.session.get(crate::session::Scope::Library(file), documents, || {
            crate::project::load_library(sdk, documents, uri)
        })
    }

    /// O projeto inteiro de `uri` (referências, renomear), da sessão ou carregado.
    pub(crate) fn project(&mut self, documents: &DocumentStore, uri: &str) -> Option<crate::session::Use<'_>> {
        let sdk = self.sdk.as_ref()?;
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        self.session.get(crate::session::Scope::Project(root), documents, || {
            crate::project::load_project(sdk, documents, uri)
        })
    }

    /// A carga ampla de `uri` com os corpos das bibliotecas que citam ou
    /// declaram `name` inferidos (os candidatos de `_addResults`, §11.2), da
    /// sessão ampla ou carregada.
    pub(crate) fn wide(&mut self, documents: &DocumentStore, uri: &str, name: &str) -> Option<crate::session::Use<'_>> {
        let SemanticAnalyzer { wide_session, sdk: Some(sdk), .. } = self else { return None };
        let root = crate::project::project_root(&crate::project::uri_file(uri)?);
        let mut usage = wide_session.get(crate::session::Scope::Project(root), documents, || {
            crate::project::load_wide_project(sdk, documents, uri)
        })?;
        usage.widen_to(name);
        Some(usage)
    }

    /// Descobre o SDK pelos mesmos caminhos usados pelo compilador.
    pub fn discover() -> Self {
        // O SDK como o analyzer o enxerga (M1): sem patches, com as
        // bibliotecas internas do dart2js.
        let sdk = SdkLayout::discover().and_then(|p| SdkLayout::load_como_analyzer(&p).ok());
        Self::new(sdk)
    }

    /// Índice dos nomes públicos do SDK, montado na primeira chamada.
    pub(crate) fn sdk_index(&mut self) -> &crate::index::SdkIndex {
        let sdk = self.sdk.as_ref();
        self.sdk_index.get_or_insert_with(|| sdk.map(crate::index::index_sdk).unwrap_or_default())
    }

    /// Geração em memória com os textos vigentes dos documentos `.dart`
    /// abertos: na carga, eles valem mais que o disco.
    pub(crate) fn open(documents: &DocumentStore) -> Construtor {
        let mut generator = Construtor::nova();
        for open in documents.uris() {
            let Some(source) = documents.get(open) else { continue };
            // O caminho canônico (a caixa do disco num sistema de arquivos
            // que não distingue caixa), o mesmo de `uri_file`.
            let Some(file) = crate::project::uri_file(open) else { continue };
            if file.extension().is_some_and(|e| e == "dart") {
                generator.por(file, source.to_owned(), "lsp", vec![]);
            }
        }
        generator
    }

    /// Um [`DocumentStore`] só com `uri`, para as chamadas da trait que
    /// recebem o texto sem a coleção de abertos.
    fn only_this(uri: &str, text: &str) -> DocumentStore {
        let mut docs = DocumentStore::new();
        docs.open(uri.to_string(), 0, text.to_string());
        docs
    }

    /// Definição pela identidade semântica (`crate::project`): o literal de
    /// diretiva continua sintático (o arquivo apontado); nomes vêm da
    /// resolução da inferência comum. Sem SDK, ou se a biblioteca não
    /// carrega, valem as respostas sintáticas conservadoras.
    fn define(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<Span>)> {
        if let Some((target, None)) = self.syntactic.definition(uri, text, offset) {
            return Some((target, None));
        }
        if let Some(project) = self.library(documents, uri)
            && let Some(unit) = project.uri_unit(uri)
            && let Ok(Some(d)) = project.identify(unit, offset)
            && let Some((u, span)) = project.declaration(&d)
        {
            return Some((project.unit_uri(u)?, Some(span)));
        }
        self.syntactic.definition_in_workspace(uri, text, offset, documents)
    }

    /// Hover pela identidade semântica; sem ela, o sintático conservador.
    fn pass_hover(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
        if let Some(project) = self.library(documents, uri)
            && let Some(unit) = project.uri_unit(uri)
            && let Ok(Some(d)) = project.identify(unit, offset)
            && let Some(h) = project.hover(unit, &d)
        {
            return Some(h);
        }
        self.syntactic.hover_in_workspace(uri, text, offset, documents)
    }

    /// Referências pela identidade da declaração, em todas as bibliotecas
    /// do projeto (abertas ou só no disco). Declarações de dependências (SDK,
    /// pacotes) entram como declaração, mas os usos só são procurados no
    /// projeto.
    fn semantic_references(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        // Busca não local: a carga ampla, com o SDK e os pacotes que citam o
        // nome (docs/LSP-ESPECIFICACAO.md §11.9 B); a local fica no projeto.
        let name = {
            let project = self.project(documents, uri)?;
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        };
        if let Some(name) = name
            && let Some(wide) = self.wide(documents, uri, &name)
        {
            return references_in(&wide, uri, offset);
        }
        let project = self.project(documents, uri)?;
        references_in(&project, uri, offset)
    }
}

/// As referências do que está em `offset` de `uri`, no projeto dado.
fn references_in(project: &crate::project::Project, uri: &str, offset: usize) -> Option<References> {
    {
        let unit = project.uri_unit(uri)?;
        let d = project.identify(unit, offset).ok()??;
        if let crate::project::Target::Prefix { library, name } = d.target {
            let (declaration, uses) = project.prefix_references(unit, offset, library, name)?;
            let mut uses: Vec<(String, Span)> = uses.into_iter().filter_map(|(u, s)| Some((project.unit_uri(u)?, s))).collect();
            uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
            let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
            return Some(References { declaration, uses });
        }
        let declaration = project.declaration_to_references(&d);
        let declarations = project.declarations(&d.target);
        // Busca não local: só os arquivos candidatos de `_addResults`
        // (docs/LSP-ESPECIFICACAO.md §11.2), inclusive os buracos do Dart
        // (redirecionamento de fábrica, operadores fora da biblioteca).
        let candidates = project.search_name(&d.target).map(|name| {
            let mut units: Vec<UnitId> = declarations.iter().map(|(u, _, _)| *u).collect();
            if let Some((u, _)) = project.declaration(&d) {
                units.push(u);
            }
            units.dedup();
            project.candidate_files(&name, &units)
        });
        let mut uses: Vec<(String, Span)> = Vec::new();
        // Só os candidatos aceitam usos: a varredura fica neles.
        for (u, de, until) in project.occurrences_in(&d.target, false, candidates.as_ref()).ok()? {
            if declarations.contains(&(u, de, until)) {
                continue;
            }
            if candidates.as_ref().is_some_and(|c| !c.contains(&u)) {
                continue;
            }
            uses.push((project.unit_uri(u)?, Span { start: de, end: until }));
        }
        uses.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaration = declaration.and_then(|(u, s)| Some((project.unit_uri(u)?, s)));
        Some(References { declaration, uses })
    }
}

/// Carrega o programa de `uri` com `text` e os demais documentos abertos
/// nos textos vigentes (os outros arquivos vêm do disco). Uma parte entra
/// pela biblioteca dona.
pub(crate) fn load(sdk: &SdkLayout, uri: &str, text: &str, documents: Option<&DocumentStore>) -> Option<(Program, Interner, UnitId)> {
    let path = crate::project::uri_file(uri)?;
    if path.extension().is_none_or(|e| e != "dart") { return None; }
    let mut generator = documents.map_or_else(Construtor::nova, SemanticAnalyzer::open);
    // A chamada direta da trait também usa o texto recebido, mesmo sem
    // DocumentStore. O arquivo da requisição prevalece sobre a coleção.
    generator.por(path.clone(), text.to_owned(), "lsp", vec![]);
    // Uma parte não é biblioteca: a carga entra pela dona, que a inclui.
    // Se a dona não a declara (`part` ausente), a parte entra sozinha.
    let owning = owning_library(&path, text, documents);
    let generation = generator.concluir(1).ok()?;
    let chave = dartforge_elements::gerado::chave(&path);
    for entry in owning.iter().chain([&path]) {
        let mut names = Interner::new();
        let (program, _) = load_lenient_gerados(entry, sdk, None, &mut names, None, None, Some(generation.clone()));
        let unit = program.units.iter().position(|u| u.path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&chave));
        if let Some(unit) = unit {
            return Some((program, names, UnitId(unit as u32)));
        }
    }
    None
}

/// A biblioteca dona de `path` quando o texto é uma parte (`part of`):
/// pela URI escrita (relativa ou `package:`), ou, na forma antiga
/// `part of name;`, pelo arquivo do projeto que declara `part` para ela.
pub(crate) fn owning_library(path: &std::path::Path, text: &str, documents: Option<&DocumentStore>) -> Option<std::path::PathBuf> {
    use dartforge_elements::gerado::chave;
    use dartforge_frontend::ast::DirectiveKind;
    if !text.contains("part") {
        return None;
    }
    let mut names = Interner::new();
    let analyzed = dartforge_frontend::parser::parse(text, &mut names);
    let uri = analyzed.unit.directives.iter().find_map(|d| match &d.kind {
        DirectiveKind::PartOf { uri, .. } => Some(uri.as_ref().and_then(dartforge_elements::load::string_lit_value)),
        _ => None,
    })?;
    if let Some(uri) = uri {
        if uri.starts_with("package:") {
            let config = dartforge_elements::config::PackageConfig::discover(path)?;
            return dartforge_elements::config::PackageConfig::load(&config).ok()?.resolve_package_uri(&uri).ok();
        }
        return Some(chave(&path.parent()?.join(uri)));
    }
    let target = chave(path);
    let root = crate::project::project_root(path);
    crate::project::project_files(&root).into_iter().find(|candidate| {
        let open = documents
            .and_then(|d| Url::from_file_path(candidate).ok().and_then(|u| d.get(u.as_str()).map(str::to_string)));
        let Some(source) = open.or_else(|| std::fs::read_to_string(candidate).ok()) else { return false };
        if !source.contains("part") {
            return false;
        }
        let mut names = Interner::new();
        let analyzed = dartforge_frontend::parser::parse(&source, &mut names);
        analyzed.unit.directives.iter().any(|d| match &d.kind {
            DirectiveKind::Part { uri } => dartforge_elements::load::string_lit_value(uri)
                .and_then(|u| candidate.parent().map(|dir| chave(&dir.join(u))))
                .is_some_and(|p| p == target),
            _ => false,
        })
    })
}

impl Analyzer for SemanticAnalyzer {
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        self.syntactic.diagnose(uri, text)
    }

    fn symbols(&mut self, uri: &str, text: &str) -> Vec<serde_json::Value> {
        self.syntactic.symbols(uri, text)
    }

    fn definition(&mut self, uri: &str, text: &str, offset: usize) -> Option<(String, Option<Span>)> {
        let only_this = Self::only_this(uri, text);
        self.define(uri, text, offset, &only_this)
    }

    fn definition_in_workspace(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<Span>)> {
        self.define(uri, text, offset, documents)
    }

    fn references(&mut self, uri: &str, text: &str, offset: usize) -> Option<Vec<Span>> {
        self.syntactic.references(uri, text, offset)
    }

    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        if self.sdk.is_some() && documents.get(uri).is_some() {
            return self.semantic_references(documents, uri, offset);
        }
        self.syntactic.references_in(documents, uri, offset)
    }

    fn hover(&mut self, uri: &str, text: &str, offset: usize) -> Option<Hover> {
        let only_this = Self::only_this(uri, text);
        self.pass_hover(uri, text, offset, &only_this)
    }

    fn hover_in_workspace(&mut self, uri: &str, text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
        self.pass_hover(uri, text, offset, documents)
    }

    fn closed_document(&mut self, uri: &str) {
        self.syntactic.closed_document(uri);
        self.session.invalidate();
        self.wide_session.invalidate();
    }

    fn changed_document(&mut self, _uri: &str) {
        self.session.invalidate();
        self.wide_session.invalidate();
    }

    fn open_document(&mut self, documents: &DocumentStore, uri: &str) {
        if self.incremental {
            self.session.open(documents, uri);
            self.wide_session.open(documents, uri);
        } else {
            self.changed_document(uri);
        }
    }

    fn document_closed_at(&mut self, documents: &DocumentStore, uri: &str) {
        self.syntactic.closed_document(uri);
        if self.incremental {
            self.session.close(documents, uri);
            self.wide_session.close(documents, uri);
        } else {
            self.session.invalidate();
            self.wide_session.invalidate();
        }
    }

    fn edited_document(&mut self, documents: &DocumentStore, uri: &str) {
        if self.incremental {
            self.session.update(documents, uri);
            self.wide_session.update(documents, uri);
        } else {
            self.session.invalidate();
            self.wide_session.invalidate();
        }
    }

    fn sdk_for_diagnostics(&self) -> Option<std::path::PathBuf> {
        self.sdk.as_ref().map(|s| s.root.clone())
    }

    fn prepare_rename(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
        let Some(project) = self.project(documents, uri) else { return Ok(None) };
        crate::rename::prepare(&project, uri, offset)
    }

    fn rename(&mut self, documents: &DocumentStore, uri: &str, offset: usize, new: &str) -> Result<crate::Rename, String> {
        let project = self.project(documents, uri)
            .ok_or_else(|| "O projeto do arquivo não pôde ser carregado (SDK ausente ou URI que não é de arquivo).".to_string())?;
        crate::rename::rename(&project, uri, offset, new)
    }

    fn actions(&mut self, documents: &DocumentStore, uri: &str, start: usize, end: usize, published_: &[Diagnostic]) -> Vec<crate::CodeAction> {
        let Some(text) = documents.get(uri) else { return Vec::new() };
        // Os imediatos da versão vigente e os tipados já publicados para ela.
        let mut diagnostics = self.diagnose(uri, text);
        for d in published_ {
            if !diagnostics.iter().any(|x| x.code == d.code && x.span == d.span) {
                diagnostics.push(d.clone());
            }
        }
        // As correções valem para os diagnósticos das linhas pedidas (o
        // servidor do Dart filtra por linha); as assistências, para o
        // cursor.
        let (line_start_, line_end) = {
            let a = text[..start.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
            let b = text[end.min(text.len())..].find('\n').map_or(text.len(), |i| end + i);
            (a, b)
        };
        let mut output = crate::actions::fix_syntax(uri, text, &diagnostics, line_start_, line_end);
        // "Fix all in file" das sintáticas.
        let all = crate::actions::fix_in_whole_file(&output, &diagnostics, text, |o, a, b| {
            crate::actions::fix_syntax(uri, text, std::slice::from_ref(o), a, b)
        });
        output.extend(all);
        output.push(crate::actions::organize_imports(uri, text));
        // As de ignorar vêm por último, como a prioridade do Dart.
        let ignore = crate::ignore_diagnostic::ignore(uri, text, &diagnostics, line_start_, line_end);
        if self.sdk.is_none() {
            output.extend(ignore);
            return output;
        }
        self.sdk_index();
        let Some(file) = crate::project::uri_file(uri) else {
            output.extend(ignore);
            return output;
        };
        let SemanticAnalyzer { session, sdk: Some(sdk), sdk_index: Some(_index), known, .. } = self else {
            output.extend(ignore);
            return output;
        };
        if let Some(mut project) = session.get(crate::session::Scope::Library(file), documents, || {
            crate::project::load_library(sdk, documents, uri)
        }) {
            let published = crate::actions::fix_published(&project, uri, &diagnostics, line_start_, line_end);
            let all = crate::actions::fix_in_whole_file(&published, &diagnostics, text, |o, a, b| {
                crate::actions::fix_published(&project, uri, std::slice::from_ref(o), a, b)
            });
            output.extend(published);
            output.extend(all);
            // Importar antes de criar: a prioridade das correções do Dart.
            output.extend(crate::actions::import(&project, Some(&*sdk), known, documents, uri, line_start_, line_end));
            let fixed = crate::fixes::fix(&mut project, uri, &diagnostics, line_start_, line_end);
            let all = crate::actions::fix_in_whole_file(&fixed, &diagnostics, text, |o, a, b| {
                crate::fixes::fix(&mut project, uri, std::slice::from_ref(o), a, b)
            });
            output.extend(fixed);
            output.extend(all);
            output.extend(crate::fixes::required_arguments(&project, uri, line_start_, line_end));
            if let Some(unit) = project.uri_unit(uri) {
                let created = project.create_undefined(uri, unit, line_start_, line_end);
                output.extend(created);
            }
            if let Some(unit) = project.uri_unit(uri) {
                {
                    let cx = crate::refactorings::Context::new(&project, unit);
                    output.extend(cx.import_add_show(uri, start, end));
                    output.extend(cx.convert_to_package_import(uri, start, end));
                    output.extend(cx.convert_to_relative_import(uri, start, end));
                    output.extend(cx.convert_part_of_to_uri(uri, start, end));
                    output.extend(cx.add_return_type(uri, start, end));
                    output.extend(cx.add_type_annotation(uri, start, end));
                    output.extend(cx.convert_class_to_mixin(uri, start, end));
                    output.extend(cx.convert_class_to_enum(uri, start, end));
                    output.extend(cx.add_diagnostic_reference(uri, start, end));
                    output.extend(cx.flutter_assists(uri, start, end));
                    output.extend(cx.flutter_to_stateful(uri, start, end));
                    output.extend(cx.flutter_to_stateless(uri, start, end));
                    output.extend(cx.convert_to_async_body(uri, start, end));
                    // `ConvertIntoBlockBody.missingBody` como assistência: o
                    // nó da seleção (`NodeLocator`).
                    if let Some((span, text)) = cx.convert_to_block_body(dartforge_diagnostics::Span { start: start, end: end }) {
                        output.push(crate::fixes::block_body(crate::actions::CodeAction {
                            title: "Convert to block body".into(),
                            kind: "refactor.convert.bodyToBlock".into(),
                            edits: vec![crate::Edit { uri: uri.to_string(), span, text }],
                            diagnostic: None,
                            create_file: None, linked: Default::default(),
                        }));
                    }
                    output.extend(cx.convert_to_final_field(uri, start, end));
                    output.extend(cx.convert_to_for_index(uri, start, end));
                    output.extend(cx.convert_to_getter(uri, start, end));
                    output.extend(cx.convert_to_normal_parameter(uri, start, end));
                    output.extend(cx.encapsulate_field(uri, start, end));
                    output.extend(cx.convert_to_field_parameter(uri, start, end));
                    output.extend(cx.shadow_field(uri, start, end));
                    output.extend(cx.convert_to_super_parameters(uri, start, end));
                    output.extend(cx.remove_type_annotation(uri, start, end));
                    output.extend(cx.conditional_to_if_else(uri, start, end));
                    output.extend(cx.convert_text_quotes(uri, start, end));
                    output.extend(cx.convert_to_is_not_empty(uri, start, end));
                    output.extend(cx.convert_to_int_literal(uri, start, end));
                    output.extend(cx.swap_to_var(uri, start, end));
                    output.extend(cx.convert_to_function_syntax(uri, start, end));
                    output.extend(cx.wrap_commands(uri, start, end));
                    output.extend(cx.destructure_local(uri, start, end));
                    output.extend(cx.convert_to_if_case(uri, start, end));
                    output.extend(cx.switch_to_if_case_chain(uri, start, end));
                    output.extend(cx.if_to_switch(uri, start, end));
                    output.extend(cx.switch_expression_to_statement(uri, start, end));
                    output.extend(cx.switch_to_expression(uri, start, end));
                    output.extend(cx.split_and_condition(uri, start, end));
                    output.extend(cx.convert_to_null_aware(uri, start, end));
                    output.extend(cx.convert_to_multiline(uri, start, end));
                    output.extend(cx.convert_add_all_to_spread(uri, start, end));
                    output.extend(cx.inline_invocation_add(uri, start, end));
                    output.extend(cx.conditional_to_if_element(uri, start, end));
                    output.extend(cx.block_documentation(uri, start, end));
                    output.extend(cx.line_documentation(uri, start, end));
                    output.extend(cx.convert_to_map_literal(uri, start, end));
                    output.extend(cx.convert_to_set_literal(uri, start, end));
                    output.extend(cx.map_from_iterable_to_for(uri, start, end));
                }
                output.extend(project.rewrite_assists(uri, unit, start, end));
                output.extend(project.syntactic_assists(uri, unit, start));
                output.extend(project.condition_assists(uri, unit, start));
                output.extend(project.join_assists(uri, unit, start));
            }
        }
        output.extend(ignore);
        output
    }

    fn refactorings(&mut self, documents: &DocumentStore, uri: &str, offset: usize, length: usize, create_files: bool) -> Vec<crate::Refactoring> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.refactorings(unit, offset, length, create_files)
    }

    fn run_refactoring(&mut self, documents: &DocumentStore, uri: &str, request: &crate::RefactoringRequest) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else { return crate::RefactoringResult::NotAnalyzed };
        crate::refactorings_exec::run(&project, uri, request)
    }

    fn move_to_file(&mut self, documents: &DocumentStore, uri: &str, offset: usize, length: usize, target: &str) -> crate::RefactoringResult {
        let Some(project) = self.project(documents, uri) else { return crate::RefactoringResult::NotAnalyzed };
        crate::refactorings_exec::move_(&project, uri, offset, length, target)
    }

    fn folds(&mut self, uri: &str, text: &str, lines_only: bool) -> Vec<crate::Fold> {
        self.syntactic.folds(uri, text, lines_only)
    }

    fn selections(&mut self, uri: &str, text: &str, offset: usize) -> Vec<Span> {
        self.syntactic.selections(uri, text, offset)
    }

    fn signature(&mut self, documents: &DocumentStore, uri: &str, offset: usize, auto: bool) -> Option<crate::Signature> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.signature(unit, offset, auto)
    }

    fn document_highlights(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<Span>> {
        if self.sdk.is_none() {
            let text = documents.get(uri)?.to_string();
            return self.syntactic.references(uri, &text, offset);
        }
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.document_highlights(unit, offset)
    }

    fn implementations(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Vec<(String, Span)> {
        // `searchAllSubtypes` procura em todo arquivo conhecido, inclusive o
        // SDK inteiro (`JSString` de `dart:_interceptors` implementa
        // `String`): a carga ampla, como a das referências.
        let name = self.project(documents, uri).and_then(|project| {
            let unit = project.uri_unit(uri)?;
            let d = project.identify(unit, offset).ok()??;
            project.search_name(&d.target)
        });
        if let Some(name) = name
            && let Some(wide) = self.wide(documents, uri, &name)
            && let Some(unit) = wide.uri_unit(uri)
        {
            return wide.implementations(unit, offset).into_iter().filter_map(|(u, s)| Some((wide.unit_uri(u)?, s))).collect();
        }
        let Some(project) = self.project(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project
            .implementations(unit, offset)
            .into_iter()
            .filter_map(|(u, s)| Some((project.unit_uri(u)?, s)))
            .collect()
    }

    fn colors(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, [u8; 4])> {
        // As bibliotecas das constantes de cor (o `Colors` do Flutter) precisam
        // dos corpos: sem elas inferidas, a carga própria com elas.
        let extras = {
            let Some(project) = self.library(documents, uri) else { return Vec::new() };
            let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
            project.color_libraries(unit)
        };
        if extras.is_empty() {
            let Some(mut project) = self.library(documents, uri) else { return Vec::new() };
            let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
            return project.colors(unit);
        }
        let Some(sdk) = self.sdk.as_ref() else { return Vec::new() };
        let Some(project) = crate::project::load_library(sdk, documents, uri) else { return Vec::new() };
        let crate::project::Project { query, root, mut libraries, referenced_names, reverse_index, comments, occurrences_by_unit } = project;
        let mut list: Vec<dartforge_elements::model::LibraryId> = libraries.iter().copied().chain(extras.iter().copied()).collect();
        list.sort();
        list.dedup();
        libraries.extend(extras);
        let mut project = crate::project::Project { query: query.reinfer(&list), root, libraries, referenced_names, reverse_index, comments, occurrences_by_unit };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.colors(unit)
    }

    fn color_presentation(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<bool> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        project.has_flutter_color().then(|| project.requires_const(unit, offset))
    }

    fn augmentation_lenses(&mut self, documents: &DocumentStore, uri: &str, augmented: bool, augmentations: bool) -> Vec<(Span, &'static str, String, Span)> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project
            .augmentation_lenses(unit, augmented, augmentations)
            .into_iter()
            .filter_map(|(s, t, u, target)| Some((s, t, project.unit_uri(u)?, target)))
            .collect()
    }

    fn augmentation_neighbor(&mut self, documents: &DocumentStore, uri: &str, offset: usize, next: bool) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.augmentation_neighbor(unit, offset, next)?;
        Some((project.unit_uri(u)?, s))
    }

    fn upper(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<(String, Span)> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.upper(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn type_definition(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<(String, Span)> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (u, s) = project.type_definition(unit, offset)?;
        Some((project.unit_uri(u)?, s))
    }

    fn closing_labels(&mut self, documents: &DocumentStore, uri: &str) -> Vec<(Span, String)> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.closing_labels(unit)
    }

    fn hints(&mut self, documents: &DocumentStore, uri: &str) -> Vec<crate::Hint> {
        let Some(project) = self.library(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        project.hints(unit)
    }

    fn semantic_tokens(&mut self, documents: &DocumentStore, uri: &str, multiline: bool, range: Option<(usize, usize)>) -> Option<Vec<u32>> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let highlights = project.highlights(unit);
        let text = documents.get(uri)?;
        let table = documents.lines(uri)?;
        Some(crate::semantic_tokens::encode(text, table, &highlights, multiline, range))
    }

    fn prepare_calls(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::CallItem> {
        let project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let e = project.call_target(unit, offset)?;
        project.call_item(e)
    }

    fn calls(&mut self, documents: &DocumentStore, uri: &str, offset: usize, name: &str, constructor: bool, received: bool) -> Vec<(crate::CallItem, Vec<Span>)> {
        let Some(project) = self.project(documents, uri) else { return Vec::new() };
        let Some(unit) = project.uri_unit(uri) else { return Vec::new() };
        let Some((e, _)) = project.item_target(unit, offset, name, constructor) else { return Vec::new() };
        if received {
            // Só executáveis recebem chamadas.
            if !matches!(e, crate::call_hierarchy::CallElem::Function(_) | crate::call_hierarchy::CallElem::Local(..)) {
                return Vec::new();
            }
            project.incoming_calls(e)
        } else {
            project.outgoing_calls(unit, offset)
        }
    }

    fn prepare_hierarchy(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::TypeItem> {
        let mut project = self.library(documents, uri)?;
        let unit = project.uri_unit(uri)?;
        let (c, ty) = project.hierarchy_target(unit, offset)?;
        project.type_item(c, ty)
    }

    fn hierarchy(&mut self, documents: &DocumentStore, uri: &str, reference: &str, anchor: Option<(&str, &[usize])>, supertypes: bool) -> Option<Vec<crate::TypeItem>> {
        let mut project = self.project(documents, uri)?;
        if supertypes { project.reference_supertypes(reference, anchor) } else { project.reference_subtypes(reference) }
    }

    fn set_completion_max(&mut self, max: usize) {
        self.completion_max = max;
    }

    fn set_not_imported_in_completion(&mut self, yes: bool) {
        self.not_imported_in_completion = yes;
    }

    fn completion(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<crate::Completion> {
        let text = documents.get(uri)?;
        let features = self.syntactic.features(uri, text);
        self.sdk.as_ref()?;
        self.sdk_index();
        let self_max = self.completion_max;
        let not_imported = self.not_imported_in_completion;
        let SemanticAnalyzer { sdk: Some(sdk), sdk_index: Some(index), project_index, known, session, incremental, .. } = self else {
            return None;
        };
        let sdk: &SdkLayout = sdk;
        let indices = crate::completion::Indices { sdk: index, project: project_index, known };
        // O estado retido da biblioteca (o do texto do documento), para o
        // caminho incremental (docs/LSP-ESPECIFICACAO.md §16.7); carregado e
        // retido aqui se faltar, para que as teclas seguintes o achem.
        let file = crate::project::uri_file(uri)?;
        let mut retained = if *incremental {
            session.get(crate::session::Scope::Library(file), documents, || crate::project::load_library(sdk, documents, uri))
        } else {
            None
        };
        crate::completion::completion(sdk, indices, documents, uri, text, offset, features, self_max, not_imported, retained.as_deref_mut())
    }
}
