//! Servidor LSP do DartForge: documentos, sincronização incremental e despacho.
//!
//! O desenho copia o do rust-analyzer (`references/rust-analyzer`), não o
//! código: um [`Server`] dono de tudo (documentos, fila, cancelamentos),
//! passado por `&mut` ao despacho; documentos em memória versionados como em
//! `mem_docs.rs`; fila com cancelamento como em `op_queue.rs`; conversão
//! UTF-16 como em `lsp/utils.rs` (aqui em [`utf16`]).
//!
//! O servidor diagnostica em dois tempos: na hora, pelo parser novo e pelos
//! verificadores locais de `crates/analysis` através do [`trait Analyzer`];
//! em segundo plano, quando o analisador tem SDK
//! ([`Analyzer::sdk_for_diagnostics`]), pela mesma análise tipada do
//! `dartforge analyze` (`typed`), publicada só se a versão ainda é a
//! vigente (`docs/LSP.md`, "Diagnósticos tipados").

pub mod formatting;
pub mod server;
mod actions;
pub(crate) use dartforge_analysis::arvore_analyzer;
mod signature;
mod assist_conditionals;
mod assist_destructure;
mod assist_enum;
mod assist_surround;
mod assist_expressions;
mod assist_flutter;
mod assist_flutter_widget;
mod assist_functions;
mod assist_imports;
mod assist_loops;
mod assist_super;
mod assist_switch;
mod assist_switch_expression;
mod assist_members;
mod assist_strings;
mod assist_type;
mod assists;
mod assists2;
mod assists3;
mod fuzzy_matcher;
mod call_hierarchy;
mod inlay_hints;
mod write_type;
mod especies_g;
mod structure;
mod fix_all;
mod organize_imports;
mod sort_members;
mod hierarchy;
mod workspace_symbols;
mod referenced_names;
mod ignore_diagnostic;
mod completion;
mod known_libraries;
mod query;
mod outline;
mod fixes;
mod dart_fixes;
mod create;
mod dartdoc;
mod document_highlight;
mod description;
mod index;
mod links;
mod colors;
mod augmentations;
mod move_file;
mod insert;
mod completion_item;
mod navigation;
mod project;
mod semantic_tokens;
mod refactorings;
mod refactorings_inline;
mod refactorings_exec;
mod refactorings_method;
mod refactorings_move;
mod registration;
mod relevance;
mod relevance_tables;
mod rename;
mod closing_labels;
mod semantic;
mod session;
mod symbols;
mod snippets;
mod typed;
pub mod transport;
pub mod utf16;

use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;
use utf16::LineTable;

pub use server::Server;
pub use semantic::SemanticAnalyzer;
pub use completion::{Call, Completion, AutoImport, CompletionItem};
pub use rename::{Edit, RenameFile, Rename};

pub use actions::{CodeAction, LinkedGroup, Linked};
pub use refactorings::{RefactoringCommand, RefactoringRequest, Refactoring, RefactoringResult};
pub use signature::Signature;
pub use call_hierarchy::{CallKind, CallItem};
pub use inlay_hints::Hint;
pub use structure::Fold;
pub use session::SessionStats;


/// Posição LSP: linha e coluna em **unidades UTF-16** (ambas a partir de 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Linha a partir de 0.
    pub line: u32,
    /// Coluna em unidades UTF-16 a partir de 0.
    pub column: u32,
}

/// Uma entrada de `contentChanges` de `textDocument/didChange`.
///
/// `range` ausente significa substituição integral (sincronização `full`,
/// que o servidor aceita mas nunca pede: a capacidade anunciada é incremental).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentChange {
    /// Intervalo em posições UTF-16 no texto vigente, ou `None` para tudo.
    pub range: Option<(Position, Position)>,
    /// Texto de substituição.
    pub text: String,
}

/// Resposta de `textDocument/hover`: o intervalo (bytes) da referência sob
/// o cursor, a descrição do elemento (`int sum(int a, int b)`), o tipo
/// estático quando é variável ou getter, e o comentário de documentação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Intervalo do nome sob o cursor, em bytes do texto vigente.
    pub range: dartforge_diagnostics::Span,
    /// Descrição do elemento, como o servidor do Dart a escreve.
    pub description: String,
    /// Tipo estático (variáveis e getters), já formatado.
    pub ty: Option<String>,
    /// Documentação (`///` ou `/** */`), já sem os marcadores.
    pub documentation: Option<String>,
    /// A biblioteca que declara o elemento (não local), como o Dart a
    /// mostra: `package:x/y.dart`, `dart:core`, ou o caminho relativo à raiz
    /// do projeto para um arquivo fora de `lib/`.
    pub library: Option<String>,
}

/// Resposta de `textDocument/references`: a declaração (quando se sabe onde
/// está) e os usos, cada um com a URI do arquivo e o span em bytes do texto
/// que o servidor enxerga (aberto, ou o do disco para arquivo fechado).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct References {
    /// Onde o elemento é declarado; pode estar fora do projeto (SDK).
    pub declaration: Option<(String, dartforge_diagnostics::Span)>,
    /// Usos, em ordem de (URI, offset).
    pub uses: Vec<(String, dartforge_diagnostics::Span)>,
}

impl References {
    /// A forma sintática (declaração primeiro, depois os usos, tudo no
    /// mesmo documento) como [`References`].
    fn of_list(uri: &str, spans: Vec<dartforge_diagnostics::Span>) -> Self {
        let mut spans = spans.into_iter().map(|s| (uri.to_string(), s));
        let declaration = spans.next();
        Self { declaration, uses: spans.collect() }
    }
}

/// Texto vigente de um documento aberto no editor.
#[derive(Debug, Clone)]
struct OpenDocument {
    /// Versão LSP mais recente recebida; cresce a cada `didChange`.
    version: i32,
    /// Texto integral vigente. É o único estado retido por documento.
    text: String,
    /// Inícios de linha do texto vigente, para converter UTF-16 sem revarrer.
    lines: LineTable,
}

impl OpenDocument {
    /// Abre o documento já com a tabela de linhas pronta.
    fn new(version: i32, text: String) -> Self {
        let lines = LineTable::build(&text);
        Self {
            version,
            text,
            lines,
        }
    }

    /// Aplica as mudanças em ordem e recalcula a tabela do ponto editado em
    /// diante, uma vez por mudança. Devolve a primeira linha tocada, para
    /// quem quiser medir o custo da atualização.
    fn apply(&mut self, changes: &[ContentChange]) {
        for change in changes {
            match change.range {
                None => {
                    self.text = change.text.clone();
                    self.lines = LineTable::build(&self.text);
                }
                Some((start, end)) => {
                    let de =
                        self.lines
                            .position_offset(&self.text, start.line, start.column);
                    let until = self
                        .lines
                        .position_offset(&self.text, end.line, end.column);
                    let (de, until) = if de <= until { (de, until) } else { (until, de) };
                    self.text.replace_range(de..until, &change.text);
                    let line = self.lines.line_of(&self.text, de);
                    self.lines.recompute_from(&self.text, line);
                }
            }
        }
    }
}

/// Dono explícito dos documentos abertos: quem descarta a versão N−1.
///
/// Cada documento tem **uma** entrada (texto + tabela + versão), e a chegada
/// da versão N substitui a N−1 no lugar — o `drop` da `String` antiga é
/// determinístico, não elegibilidade futura para um coletor. Fechar o
/// documento remove a entrada, e nada do documento sobrevive.
///
/// ```
/// let mut docs = dartforge_lsp::DocumentStore::new();
/// docs.open("file:///a.dart".into(), 1, "void main() {}".into());
/// docs.update("file:///a.dart", 2, "void main() { print(1); }".into());
/// assert_eq!(docs.version("file:///a.dart"), Some(2));
/// assert!(docs.close("file:///a.dart"));
/// assert!(docs.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct DocumentStore {
    documents: HashMap<String, OpenDocument>,
}

impl DocumentStore {
    /// Cria um servidor vazio, sem documentos retidos.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um documento aberto; reabrir substitui, sem reter o anterior.
    pub fn open(&mut self, uri: String, version: i32, text: String) {
        self.documents
            .insert(uri, OpenDocument::new(version, text));
    }

    /// Aplica a versão N e descarta a N−1 no lugar; falso quando inexistente.
    ///
    /// Versão menor ou igual à vigente é ignorada e devolve falso: fora de
    /// ordem não pode ressuscitar texto velho.
    pub fn update(&mut self, uri: &str, version: i32, text: String) -> bool {
        match self.documents.get_mut(uri) {
            Some(current) if version > current.version => {
                *current = OpenDocument::new(version, text);
                true
            }
            _ => false,
        }
    }

    /// Aplica mudanças incrementais (intervalos UTF-16) como a versão N.
    ///
    /// Mesma regra de versão de [`DocumentStore::update`]: documento
    /// inexistente ou versão fora de ordem devolve falso e não toca em nada.
    ///
    /// ```
    /// use dartforge_lsp::{DocumentStore, ContentChange, Position};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void main() {}".into());
    /// let swap = ContentChange {
    ///     range: Some((Position { line: 0, column: 13 }, Position { line: 0, column: 13 })),
    ///     text: " print(1);".into(),
    /// };
    /// assert!(docs.apply("file:///a.dart", 2, &[swap]));
    /// assert_eq!(docs.get("file:///a.dart"), Some("void main() { print(1);}"));
    /// ```
    pub fn apply(&mut self, uri: &str, version: i32, changes: &[ContentChange]) -> bool {
        match self.documents.get_mut(uri) {
            Some(current) if version > current.version => {
                current.apply(changes);
                current.version = version;
                true
            }
            _ => false,
        }
    }

    /// Fecha o documento e descarta seu texto; falso quando inexistente.
    pub fn close(&mut self, uri: &str) -> bool {
        self.documents.remove(uri).is_some()
    }

    /// Texto vigente de um documento aberto, se existir.
    pub fn get(&self, uri: &str) -> Option<&str> {
        self.documents.get(uri).map(|open| open.text.as_str())
    }

    /// Versão vigente de um documento aberto, se existir.
    pub fn version(&self, uri: &str) -> Option<i32> {
        self.documents.get(uri).map(|open| open.version)
    }

    /// Tabela de linhas do documento, para converter spans sem revarrer.
    pub(crate) fn lines(&self, uri: &str) -> Option<&LineTable> {
        self.documents.get(uri).map(|open| &open.lines)
    }

    /// Documentos atualmente retidos; cada um custa exatamente um texto.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// URIs dos documentos abertos, sem reter cópia dos textos.
    pub(crate) fn uris(&self) -> impl Iterator<Item = &str> {
        self.documents.keys().map(String::as_str)
    }

    /// Verdadeiro quando nenhum documento está aberto.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Diagnostica o texto vigente pelo parser novo.
    ///
    /// Lista vazia quando o documento está fechado; nunca retém o resultado,
    /// que é transitório do chamador.
    pub fn diagnose_open(&self, uri: &str) -> Vec<Diagnostic> {
        match self.get(uri) {
            Some(text) => SyntacticAnalyzer::new().diagnose(uri, text),
            None => Vec::new(),
        }
    }
}

/// Análise que produz os diagnósticos publicados após cada mudança.
///
/// O [`Server`] chama este trait, nunca o parser diretamente. A
/// assinatura leva `uri` e `text` (não o documento) para que a
/// implementação possa consultar outros arquivos. Os diagnósticos tipados
/// não passam por [`Analyzer::diagnose`] (que é síncrono): o servidor
/// os pede ao trabalhador em segundo plano com o SDK de
/// [`Analyzer::sdk_for_diagnostics`].
pub trait Analyzer {
    /// Diagnostica o texto vigente e devolve spans em bytes UTF-8.
    ///
    /// O resultado é transitório do chamador: nada é retido entre chamadas,
    /// para que N edições não retenham N análises (o modo de falha do LSP do
    /// Dart, medido no PLANO.md).
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic>;

    /// Símbolos sintáticos do documento, sem guardar a árvore entre edições.
    fn symbols(&mut self, _uri: &str, _text: &str) -> Vec<serde_json::Value> {
        Vec::new()
    }

    /// Definição conservadora da posição no texto: URI e seleção no destino.
    fn definition(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        None
    }

    /// Variante com os buffers abertos, para resolver imports ainda não
    /// salvos sem duplicar documentos no analisador residente.
    fn definition_in_workspace(&mut self, uri: &str, text: &str, offset: usize, _documents: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        self.definition(uri, text, offset)
    }

    /// Descrição, tipo e documentação do que está sob o cursor.
    fn hover(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<Hover> {
        None
    }

    /// Variante com os buffers abertos (textos ainda não salvos).
    fn hover_in_workspace(&mut self, uri: &str, text: &str, offset: usize, _documents: &DocumentStore) -> Option<Hover> {
        self.hover(uri, text, offset)
    }

    /// Itens de completar na posição `offset` (bytes) do documento aberto
    /// `uri`. `None` quando a análise não sabe responder (sem SDK, por
    /// exemplo); lista vazia quando não há o que oferecer.
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SemanticAnalyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f(int x) { x. }".into());
    /// // Sem tipos não há o que completar; o semântico sem SDK também não sabe.
    /// assert!(SyntacticAnalyzer::new().completion(&docs, "file:///a.dart", 18).is_none());
    /// assert!(SemanticAnalyzer::new(None).completion(&docs, "file:///a.dart", 18).is_none());
    /// ```
    fn completion(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<Completion> {
        None
    }

    /// O `maxCompletionItems` da configuração do cliente (o `maxSuggestions`
    /// do coletor e do truncamento).
    fn set_completion_max(&mut self, _max: usize) {}

    /// Se o completar sugere declarações de bibliotecas não importadas (o
    /// `suggestFromUnimportedLibraries && capabilities.applyEdit` do
    /// `handler_completion.dart:367`).
    fn set_not_imported_in_completion(&mut self, _yes: bool) {}

    /// `prepareRename`: intervalo (bytes) e texto do nome renomeável sob o
    /// cursor; `Ok(None)` quando não há nome ali.
    ///
    /// # Erros
    ///
    /// Mensagem para o usuário quando o elemento existe mas não pode ser
    /// renomeado (SDK, pacote externo, forma ainda não suportada).
    fn prepare_rename(
        &mut self,
        _documents: &DocumentStore,
        _uri: &str,
        _offset: usize,
    ) -> Result<Option<(dartforge_diagnostics::Span, String)>, String> {
        Ok(None)
    }

    /// `rename`: as edições em todos os arquivos do projeto.
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "var x = 1;".into());
    /// let mut a = SyntacticAnalyzer::new();
    /// assert_eq!(a.prepare_rename(&docs, "file:///a.dart", 4), Ok(None));
    /// assert!(a.rename(&docs, "file:///a.dart", 4, "y").is_err());
    /// ```
    ///
    /// # Erros
    ///
    /// Nome inválido, elemento não renomeável ou conflito, com a mensagem
    /// para o usuário.
    fn rename(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _new: &str) -> Result<Rename, String> {
        Err("Renomear exige a análise semântica (SDK do Dart).".into())
    }

    /// `codeAction`: correções e assistências para o intervalo `start..end`
    /// (bytes) do documento aberto. `published` são os diagnósticos tipados
    /// já publicados para a versão vigente (o servidor não passa os de uma
    /// versão velha). O padrão oferece as correções dos diagnósticos
    /// sintáticos (inserir `;`).
    ///
    /// ```
    /// use dartforge_lsp::{Analyzer, SyntacticAnalyzer, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f() { var x = 1 }".into());
    /// let actions = SyntacticAnalyzer::new().actions(&docs, "file:///a.dart", 19, 19, &[]);
    /// assert_eq!(actions[0].title, "Insert ';'");
    /// assert_eq!(actions[0].edits[0].text, ";");
    /// ```
    fn actions(&mut self, documents: &DocumentStore, uri: &str, start: usize, end: usize, _published: &[Diagnostic]) -> Vec<CodeAction> {
        let Some(text) = documents.get(uri) else { return Vec::new() };
        let diagnostics = self.diagnose(uri, text);
        let mut output = actions::fix_syntax(uri, text, &diagnostics, start, end);
        output.push(actions::organize_imports(uri, text));
        output
    }

    /// As refatorações listadas em `offset..offset + length`
    /// (docs/LSP-ESPECIFICACAO.md §13.11.1); `create_files` é o
    /// `supportsFileCreation` do cliente.
    fn refactorings(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _length: usize, _create_files: bool) -> Vec<Refactoring> {
        Vec::new()
    }

    /// `refactor.perform`/`refactor.validate` de uma refatoração legada
    /// (§13.11.2).
    fn run_refactoring(&mut self, _documents: &DocumentStore, _uri: &str, _request: &RefactoringRequest) -> RefactoringResult {
        RefactoringResult::NotAnalyzed
    }

    /// `dart.refactor.move_top_level_to_file` para o arquivo `target`
    /// (§13.11.9 e).
    fn move_to_file(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _length: usize, _target: &str) -> RefactoringResult {
        RefactoringResult::NotAnalyzed
    }

    /// Descarta estado associado ao documento quando ele sai do editor.
    fn closed_document(&mut self, _uri: &str) {}

    /// O texto de `uri` mudou (`didOpen` ou `didChange` aceito): o que o
    /// analisador retém de consultas anteriores deixa de valer e cai aqui.
    fn changed_document(&mut self, _uri: &str) {}

    /// `didOpen`: `uri` passou a estar aberto em `documents`. O padrão é
    /// [`Analyzer::changed_document`]; quem retém estado pode mantê-lo
    /// quando o texto aberto é o que já leu do disco.
    fn open_document(&mut self, _documents: &DocumentStore, uri: &str) {
        self.changed_document(uri);
    }

    /// `didClose`: `uri` já saiu de `documents`. O padrão é
    /// [`Analyzer::closed_document`]; quem retém estado pode mantê-lo
    /// quando o texto do disco é o que já tinha.
    fn document_closed_at(&mut self, _documents: &DocumentStore, uri: &str) {
        self.closed_document(uri);
    }

    /// `didChange` aceito: o texto de `uri` em `documents` é o novo. O
    /// padrão é [`Analyzer::changed_document`]; quem retém
    /// estado pode trocá-lo no lugar quando só corpos mudaram.
    fn edited_document(&mut self, _documents: &DocumentStore, uri: &str) {
        self.changed_document(uri);
    }

    /// O `lib/` do SDK com que o servidor roda, em segundo plano, a análise
    /// tipada do `dartforge analyze` sobre os documentos abertos
    /// (`docs/LSP.md`, "Diagnósticos tipados"). `None` (o padrão): só os
    /// diagnósticos de [`Analyzer::diagnose`].
    fn sdk_for_diagnostics(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Referências conservadoras no próprio documento: declaração primeiro,
    /// depois os usos, todos como spans em bytes UTF-8.
    ///
    /// Só responde nos mesmos casos seguros de [`Analyzer::definition`]
    /// (tipo, variável, função ou getter de topo únicos, sem imports nem
    /// sombras): `None` significa "não sei", nunca "não há". O resultado é
    /// transitório do chamador, como nos demais métodos.
    fn references(&mut self, _uri: &str, _text: &str, _offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        None
    }

    /// Referências no workspace: a declaração e os usos, cada um com a URI
    /// do arquivo. O padrão delega ao próprio documento. Nada é retido entre
    /// pedidos além do que a política de sessão do analisador permite.
    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        let text = documents.get(uri)?;
        let spans = self.references(uri, text, offset)?;
        Some(References::of_list(uri, spans))
    }

    /// Ajuda de assinatura na lista de argumentos que contém `offset`
    /// (`textDocument/signatureHelp`). `auto`: o editor pediu ao
    /// digitar `(`; só responde se esse `(` abre a lista. O padrão: nenhuma.
    fn signature(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _auto: bool) -> Option<Signature> {
        None
    }

    /// As ocorrências, no próprio documento, do que `offset` denota
    /// (`textDocument/documentHighlight`). O padrão usa as referências
    /// conservadoras do documento.
    fn document_highlights(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let text = documents.get(uri)?.to_string();
        self.references(uri, &text, offset)
    }

    /// Implementações (`textDocument/implementation`): subtipos de uma
    /// classe ou as sobrescritas de um membro, com a URI de cada uma.
    fn implementations(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Vec<(String, dartforge_diagnostics::Span)> {
        Vec::new()
    }

    /// As cores de `uri` (`textDocument/documentColor`): o intervalo e o
    /// ARGB de 0 a 255.
    fn colors(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<(dartforge_diagnostics::Span, [u8; 4])> {
        Vec::new()
    }

    /// `textDocument/colorPresentation`: o `Color` do Flutter existe e a
    /// expressão em `offset` exige `const` (`None`: sem o `Color`).
    fn color_presentation(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<bool> {
        None
    }

    /// As lentes de augmentation de `uri` (`AugmentationCodeLensProvider`):
    /// o nome, o título e o alvo (URI e span).
    fn augmentation_lenses(&mut self, _documents: &DocumentStore, _uri: &str, _augmented: bool, _augmentations: bool) -> Vec<(dartforge_diagnostics::Span, &'static str, String, dartforge_diagnostics::Span)> {
        Vec::new()
    }

    /// `dart/textDocument/augmented` (`next` falso) e
    /// `dart/textDocument/augmentation`: o vizinho na cadeia de augmentation.
    fn augmentation_neighbor(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _next: bool) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// `dart/textDocument/super`: o construtor, a classe ou o membro que o
    /// que `offset` denota sobrescreve ou estende.
    fn upper(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// A declaração do tipo estático do que `offset` denota
    /// (`textDocument/typeDefinition`).
    fn type_definition(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// Regiões de dobra (`textDocument/foldingRange`); `lines_only`: o
    /// cliente anunciou `lineFoldingOnly`.
    fn folds(&mut self, _uri: &str, _text: &str, _only_lines: bool) -> Vec<Fold> {
        Vec::new()
    }

    /// Faixas de seleção em `offset`, da mais interna à mais externa
    /// (`textDocument/selectionRange`).
    fn selections(&mut self, _uri: &str, _text: &str, _offset: usize) -> Vec<dartforge_diagnostics::Span> {
        Vec::new()
    }

    /// Os rótulos de fechamento do documento
    /// (`dart/textDocument/publishClosingLabels`): o intervalo (bytes) do nó
    /// e o texto. O padrão não tem nenhum.
    fn closing_labels(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<(dartforge_diagnostics::Span, String)> {
        Vec::new()
    }

    /// Dicas embutidas do documento inteiro (`textDocument/inlayHint`).
    fn hints(&mut self, _documents: &DocumentStore, _uri: &str) -> Vec<Hint> {
        Vec::new()
    }

    /// Tokens semânticos do documento já codificados para o protocolo
    /// (grupos de 5 números); `multiline`: o cliente aceita tokens de
    /// várias linhas; `range` (bytes): só os que a tocam.
    fn semantic_tokens(&mut self, _documents: &DocumentStore, _uri: &str, _multiline: bool, _range: Option<(usize, usize)>) -> Option<Vec<u32>> {
        None
    }

    /// O executável em `offset` como item da hierarquia de chamadas
    /// (`textDocument/prepareCallHierarchy`).
    fn prepare_calls(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<CallItem> {
        None
    }

    /// Chamadas recebidas (`true`) ou feitas pelo item do cliente: o nome
    /// em `offset` de `uri`, o nome exibido (o `_isMatchingElement`) e se a
    /// espécie é construtor (o construtor sem nome implícito da classe); os
    /// intervalos de cada uma (no arquivo de quem chama, nas recebidas).
    fn calls(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize, _name: &str, _constructor: bool, _received: bool) -> Vec<(CallItem, Vec<dartforge_diagnostics::Span>)> {
        Vec::new()
    }

    /// O item da hierarquia de tipos em `offset`
    /// (`textDocument/prepareTypeHierarchy`).
    fn prepare_hierarchy(&mut self, _documents: &DocumentStore, _uri: &str, _offset: usize) -> Option<TypeItem> {
        None
    }

    /// Supertipos (`true`) ou subtipos diretos da classe cujo
    /// `ElementLocation` é `reference` (o `data.ref` do item), no projeto de
    /// `uri`; `anchor` é o `data.anchor` (a referência e o caminho). `None`
    /// quando a referência não localiza uma classe (resposta `null`).
    fn hierarchy(&mut self, _documents: &DocumentStore, _uri: &str, _reference: &str, _anchor: Option<(&str, &[usize])>, _supertypes: bool) -> Option<Vec<TypeItem>> {
        None
    }
}

/// Um item da hierarquia de tipos: o nome exibido (`Base<int>`), o arquivo,
/// a declaração inteira (com a documentação) e o nome, o `ElementLocation`
/// da classe e a âncora (referência e caminho) dos supertipos com
/// argumentos de tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeItem {
    pub name: String,
    pub uri: String,
    pub range: dartforge_diagnostics::Span,
    pub selection: dartforge_diagnostics::Span,
    pub reference: String,
    pub anchor: Option<(String, Vec<usize>)>,
}

/// Análise sintática: o parser novo, sem resolução (nomes e tipos chegam depois).
///
/// Cada chamada interna num [`dartforge_intern::Interner`] novo e o descarta
/// com a árvore: só os diagnósticos (mensagem + span) atravessam a chamada.
/// Aceita 100% do SDK, do corpus pub e do `new_sali`; arquivos válidos não
/// geram diagnósticos.
///
/// ```
/// use dartforge_lsp::{Analyzer, SyntacticAnalyzer};
/// let mut a = SyntacticAnalyzer::new();
/// assert!(a.diagnose("file:///a.dart", "void main() {}").is_empty());
/// // O identificador que falta e, como no `dart analyze`, o local não usado.
/// assert_eq!(a.diagnose("file:///b.dart", "void a() { int x = ; }").len(), 2);
/// ```
#[derive(Debug, Default)]
pub struct SyntacticAnalyzer {
    /// `package_config.json` já lidos, pelo caminho (um por projeto aberto):
    /// a versão de linguagem padrão de cada arquivo vem dele.
    configs: std::collections::HashMap<std::path::PathBuf, dartforge_elements::config::PackageConfig>,
    /// Versão padrão (a do pacote) de cada documento já visto.
    patterns: std::collections::HashMap<String, dartforge_frontend::LanguageVersion>,
}

impl SyntacticAnalyzer {
    /// Cria o analisador sintático.
    pub fn new() -> Self {
        Self::default()
    }

    /// Os recursos de linguagem do arquivo `uri` (`docs/VERSOES-LINGUAGEM.md`
    /// §2): o marcador `// @dart = x.y`, senão o `languageVersion` do pacote
    /// no `package_config.json` que o contém, senão a versão corrente. Sem
    /// isso, um projeto 3.6 veria erro em `final` de parâmetro (proibido na
    /// 3.13) e um 3.13 veria erro em construtor primário.
    pub(crate) fn features(&mut self, uri: &str, text: &str) -> dartforge_frontend::LibraryFeatures {
        use dartforge_frontend::{LanguageVersion, LibraryFeatures};
        if let Some((v, _)) = dartforge_frontend::features::marcador_versao(text) {
            return LibraryFeatures::new(v, &[]);
        }
        if let Some(v) = self.patterns.get(uri) {
            return LibraryFeatures::new(*v, &[]);
        }
        // Uma vez por documento: o caminho canônico (é o que o
        // `package_config` guarda) e o pacote que o contém.
        let path = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
            .map(|p| dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&p).unwrap_or(p)));
        let pattern = path
            .as_deref()
            .and_then(|p| {
                let cfg = dartforge_elements::config::PackageConfig::discover(p)?;
                if !self.configs.contains_key(&cfg) {
                    let read = dartforge_elements::config::PackageConfig::load(&cfg).ok()?;
                    self.configs.insert(cfg.clone(), read);
                }
                self.configs.get(&cfg)?.pacote_da_biblioteca(uri, Some(p))?.language_version
            })
            .unwrap_or(LanguageVersion::ATUAL);
        self.patterns.insert(uri.to_string(), pattern);
        LibraryFeatures::new(pattern, &[])
    }

    /// Só os diagnósticos do parser (sempre publicados), na versão de
    /// linguagem do arquivo.
    pub(crate) fn syntax(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, text);
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
        // Nome e texto do analyzer que é a referência do arquivo (T2).
        let reference = parsed.referencia;
        parsed.diagnostics.into_iter().filter_map(|d| d.na_referencia(reference)).collect()
    }
}

impl Analyzer for SyntacticAnalyzer {
    /// Analisa com o parser completo, na versão de linguagem do arquivo, com
    /// recuperação por declaração.
    ///
    /// Depois da sintaxe, os verificadores de `crates/analysis` que não
    /// dependem de tipos (nomes duplicados, locais não usados) sobre o próprio
    /// arquivo; destes, só sai o que a regra de publicação deixa
    /// (`dartforge_analysis::publication`): código verificado contra o oráculo,
    /// e não suprimido por `// ignore:` (o filtro do `dartforge analyze`).
    fn diagnose(&mut self, uri: &str, text: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, text);
        let mut names = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(text, &mut names, features);
        let mut output = parsed.diagnostics;
        let unit = dartforge_analysis::Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte: text };
        // Os locais não usados pela regra dos trechos pulados (T5), como no
        // `dartforge analyze`: sem porta por erro de sintaxe.
        let skipped = dartforge_analysis::Pulados { fonte: text, trechos: &parsed.pulados };
        let wildcard = features.tem(dartforge_frontend::features::Feature::WildcardVariables);
        let reference = parsed.referencia;
        let join = reference == dartforge_diagnostics::Referencia::V3_6;
        let semantic = dartforge_analysis::duplicatas::duplicatas(&[unit], &names, wildcard, join)
            .into_iter()
            .map(|(_, d)| d)
            .chain(dartforge_analysis::enums::sem_constantes(&[unit]).into_iter().map(|(_, d)| d))
            .chain(dartforge_analysis::inicializacao::finais_nao_inicializados(&[unit], &names).into_iter().map(|(_, d)| d))
            .chain(dartforge_analysis::locais::nao_usados_com_pulados(unit, &names, wildcard, skipped))
            .chain(dartforge_analysis::externos::inicializadores(unit))
            .chain(dartforge_analysis::operadores::aridade(unit, &names));
        // `// ignore:` como no `dartforge analyze` (o mesmo filtro).
        let ignored = dartforge_parity::filtros::Ignorados::de_texto(text);
        let lines = dartforge_parity::json::Linhas::new(text);
        output.extend(
            semantic
                .filter(|d| dartforge_analysis::publicacao::publicado(d, false))
                .filter(|d| !ignored.ignora(d, lines.ponto(d.span.start).line)),
        );
        // Por último, o nome e o texto do analyzer de referência (T2): os
        // filtros acima conhecem os códigos pelo nome do 3.6.
        output.into_iter().filter_map(|d| d.na_referencia(reference)).collect()
    }

    fn symbols(&mut self, uri: &str, text: &str) -> Vec<serde_json::Value> {
        let features = self.features(uri, text);
        symbols::of_document(text, features)
    }

    fn definition(&mut self, uri: &str, text: &str, offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        let features = self.features(uri, text);
        match navigation::target(uri, text, features, offset)? {
            navigation::Target::File(target) => Some((target, None)),
            navigation::Target::LocalName(ty) => Some((uri.to_string(), Some(ty.declaration))),
        }
    }

    fn hover(&mut self, uri: &str, text: &str, offset: usize) -> Option<Hover> {
        let features = self.features(uri, text);
        match navigation::target(uri, text, features, offset)? {
            navigation::Target::File(_) => None,
            navigation::Target::LocalName(ty) => Some(Hover {
                range: ty.reference,
                description: ty.description?,
                ty: ty.static_type,
                documentation: None,
                library: None,
            }),
        }
    }

    fn closed_document(&mut self, uri: &str) {
        self.patterns.remove(uri);
        // A configuração pode ter mudado enquanto o arquivo estava fechado.
        // A próxima análise a carrega novamente; nada do projeto fechado fica
        // retido indefinidamente na sessão do servidor.
        self.configs.clear();
    }

    fn references(&mut self, uri: &str, text: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let features = self.features(uri, text);
        navigation::references(uri, text, features, offset)
    }

    fn definition_in_workspace(&mut self, uri: &str, _text: &str, offset: usize, documents: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        navigation::definition_in(documents, uri, offset, self)
    }

    fn hover_in_workspace(&mut self, uri: &str, _text: &str, offset: usize, documents: &DocumentStore) -> Option<Hover> {
        let (range, description, ty) = navigation::hover_in(documents, uri, offset, self)?;
        Some(Hover { range, description, ty, documentation: None, library: None })
    }

    fn folds(&mut self, uri: &str, text: &str, lines_only: bool) -> Vec<Fold> {
        let features = self.features(uri, text);
        structure::folds(text, features, lines_only)
    }

    fn selections(&mut self, uri: &str, text: &str, offset: usize) -> Vec<dartforge_diagnostics::Span> {
        let features = self.features(uri, text);
        structure::selections(text, features, offset)
    }

    fn references_in(&mut self, documents: &DocumentStore, uri: &str, offset: usize) -> Option<References> {
        let mut found = navigation::references_in(documents, uri, offset, self)?.into_iter();
        let declaration = found.next();
        Some(References { declaration, uses: found.collect() })
    }
}
