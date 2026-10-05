//! Servidor LSP do DartForge: documentos, sincronização incremental e despacho.
//!
//! O desenho copia o do rust-analyzer (`references/rust-analyzer`), não o
//! código: um [`Servidor`] dono de tudo (documentos, fila, cancelamentos),
//! passado por `&mut` ao despacho; documentos em memória versionados como em
//! `mem_docs.rs`; fila com cancelamento como em `op_queue.rs`; conversão
//! UTF-16 como em `lsp/utils.rs` (aqui em [`utf16`]).
//!
//! O servidor diagnostica em dois tempos: na hora, pelo parser novo e pelos
//! verificadores locais de `crates/analise` através do [`trait Analisador`];
//! em segundo plano, quando o analisador tem SDK
//! ([`Analisador::sdk_para_diagnosticos`]), pela mesma análise tipada do
//! `dartforge analyze` (`tipado`), publicada só se a versão ainda é a
//! vigente (`docs/LSP.md`, "Diagnósticos tipados").

pub mod servidor;
mod acoes;
mod arvore_analyzer;
mod assinatura;
mod assistencias;
mod assistencias2;
mod assistencias3;
mod casador;
mod chamadas;
mod dicas;
mod escrever_tipo;
mod especies_g;
mod estrutura;
mod fonte_corrigir;
mod fonte_imports;
mod fonte_ordenar;
mod hierarquia;
mod simbolos_workspace;
mod ignorar;
mod aproximado;
mod completar;
mod conhecidas;
mod consulta;
mod contorno;
mod correcoes;
mod correcoes_dart;
mod criar;
mod dartdoc;
mod destaques;
mod descricao;
mod indice;
mod inserir;
mod item_completar;
mod navegacao;
mod projeto;
mod realce;
mod refatoracoes;
mod refatoracoes_embutir;
mod refatoracoes_exec;
mod refatoracoes_metodo;
mod refatoracoes_mover;
mod registro;
mod relevancia;
mod relevancia_tabelas;
mod renomear;
mod rotulos;
mod semantica;
mod sessao;
mod simbolos;
mod tipado;
pub mod transporte;
pub mod utf16;

use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;
use utf16::TabelaLinhas;

pub use servidor::Servidor;
pub use semantica::AnalisadorSemantico;
pub use completar::{Chamada, Completar, ImportAutomatico, ItemCompletar};
pub use renomear::{Edicao, RenomearArquivo, Renomeacao};

pub use acoes::AcaoDeCodigo;
pub use refatoracoes::{ComandoDeRefatoracao, PedidoDeRefatoracao, Refatoracao, ResultadoDeRefatoracao};
pub use assinatura::Assinatura;
pub use chamadas::{EspecieDeChamada, ItemDeChamada};
pub use dicas::Dica;
pub use estrutura::Dobra;
pub use sessao::EstatisticasSessao;


/// Posição LSP: linha e coluna em **unidades UTF-16** (ambas a partir de 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posicao {
    /// Linha a partir de 0.
    pub linha: u32,
    /// Coluna em unidades UTF-16 a partir de 0.
    pub coluna: u32,
}

/// Uma entrada de `contentChanges` de `textDocument/didChange`.
///
/// `intervalo` ausente significa substituição integral (sincronização `full`,
/// que o servidor aceita mas nunca pede: a capacidade anunciada é incremental).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MudancaConteudo {
    /// Intervalo em posições UTF-16 no texto vigente, ou `None` para tudo.
    pub intervalo: Option<(Posicao, Posicao)>,
    /// Texto de substituição.
    pub texto: String,
}

/// Resposta de `textDocument/hover`: o intervalo (bytes) da referência sob
/// o cursor, a descrição do elemento (`int soma(int a, int b)`), o tipo
/// estático quando é variável ou getter, e o comentário de documentação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hover {
    /// Intervalo do nome sob o cursor, em bytes do texto vigente.
    pub intervalo: dartforge_diagnostics::Span,
    /// Descrição do elemento, como o servidor do Dart a escreve.
    pub descricao: String,
    /// Tipo estático (variáveis e getters), já formatado.
    pub tipo: Option<String>,
    /// Documentação (`///` ou `/** */`), já sem os marcadores.
    pub documentacao: Option<String>,
    /// A biblioteca que declara o elemento (não local), como o Dart a
    /// mostra: `package:x/y.dart`, `dart:core`, ou o caminho relativo à raiz
    /// do projeto para um arquivo fora de `lib/`.
    pub biblioteca: Option<String>,
}

/// Resposta de `textDocument/references`: a declaração (quando se sabe onde
/// está) e os usos, cada um com a URI do arquivo e o span em bytes do texto
/// que o servidor enxerga (aberto, ou o do disco para arquivo fechado).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Referencias {
    /// Onde o elemento é declarado; pode estar fora do projeto (SDK).
    pub declaracao: Option<(String, dartforge_diagnostics::Span)>,
    /// Usos, em ordem de (URI, offset).
    pub usos: Vec<(String, dartforge_diagnostics::Span)>,
}

impl Referencias {
    /// A forma sintática (declaração primeiro, depois os usos, tudo no
    /// mesmo documento) como [`Referencias`].
    fn de_lista(uri: &str, spans: Vec<dartforge_diagnostics::Span>) -> Self {
        let mut spans = spans.into_iter().map(|s| (uri.to_string(), s));
        let declaracao = spans.next();
        Self { declaracao, usos: spans.collect() }
    }
}

/// Texto vigente de um documento aberto no editor.
#[derive(Debug, Clone)]
struct DocumentoAberto {
    /// Versão LSP mais recente recebida; cresce a cada `didChange`.
    versao: i32,
    /// Texto integral vigente. É o único estado retido por documento.
    texto: String,
    /// Inícios de linha do texto vigente, para converter UTF-16 sem revarrer.
    linhas: TabelaLinhas,
}

impl DocumentoAberto {
    /// Abre o documento já com a tabela de linhas pronta.
    fn novo(versao: i32, texto: String) -> Self {
        let linhas = TabelaLinhas::construir(&texto);
        Self {
            versao,
            texto,
            linhas,
        }
    }

    /// Aplica as mudanças em ordem e recalcula a tabela do ponto editado em
    /// diante, uma vez por mudança. Devolve a primeira linha tocada, para
    /// quem quiser medir o custo da atualização.
    fn aplicar(&mut self, mudancas: &[MudancaConteudo]) {
        for mudanca in mudancas {
            match mudanca.intervalo {
                None => {
                    self.texto = mudanca.texto.clone();
                    self.linhas = TabelaLinhas::construir(&self.texto);
                }
                Some((inicio, fim)) => {
                    let de =
                        self.linhas
                            .offset_de_posicao(&self.texto, inicio.linha, inicio.coluna);
                    let ate = self
                        .linhas
                        .offset_de_posicao(&self.texto, fim.linha, fim.coluna);
                    let (de, ate) = if de <= ate { (de, ate) } else { (ate, de) };
                    self.texto.replace_range(de..ate, &mudanca.texto);
                    let linha = self.linhas.linha_de(&self.texto, de);
                    self.linhas.recalc_a_partir(&self.texto, linha);
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
    documentos: HashMap<String, DocumentoAberto>,
}

impl DocumentStore {
    /// Cria um servidor vazio, sem documentos retidos.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um documento aberto; reabrir substitui, sem reter o anterior.
    pub fn open(&mut self, uri: String, version: i32, text: String) {
        self.documentos
            .insert(uri, DocumentoAberto::novo(version, text));
    }

    /// Aplica a versão N e descarta a N−1 no lugar; falso quando inexistente.
    ///
    /// Versão menor ou igual à vigente é ignorada e devolve falso: fora de
    /// ordem não pode ressuscitar texto velho.
    pub fn update(&mut self, uri: &str, version: i32, text: String) -> bool {
        match self.documentos.get_mut(uri) {
            Some(atual) if version > atual.versao => {
                *atual = DocumentoAberto::novo(version, text);
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
    /// use dartforge_lsp::{DocumentStore, MudancaConteudo, Posicao};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void main() {}".into());
    /// let troca = MudancaConteudo {
    ///     intervalo: Some((Posicao { linha: 0, coluna: 13 }, Posicao { linha: 0, coluna: 13 })),
    ///     texto: " print(1);".into(),
    /// };
    /// assert!(docs.apply("file:///a.dart", 2, &[troca]));
    /// assert_eq!(docs.get("file:///a.dart"), Some("void main() { print(1);}"));
    /// ```
    pub fn apply(&mut self, uri: &str, version: i32, mudancas: &[MudancaConteudo]) -> bool {
        match self.documentos.get_mut(uri) {
            Some(atual) if version > atual.versao => {
                atual.aplicar(mudancas);
                atual.versao = version;
                true
            }
            _ => false,
        }
    }

    /// Fecha o documento e descarta seu texto; falso quando inexistente.
    pub fn close(&mut self, uri: &str) -> bool {
        self.documentos.remove(uri).is_some()
    }

    /// Texto vigente de um documento aberto, se existir.
    pub fn get(&self, uri: &str) -> Option<&str> {
        self.documentos.get(uri).map(|aberto| aberto.texto.as_str())
    }

    /// Versão vigente de um documento aberto, se existir.
    pub fn version(&self, uri: &str) -> Option<i32> {
        self.documentos.get(uri).map(|aberto| aberto.versao)
    }

    /// Tabela de linhas do documento, para converter spans sem revarrer.
    pub(crate) fn linhas(&self, uri: &str) -> Option<&TabelaLinhas> {
        self.documentos.get(uri).map(|aberto| &aberto.linhas)
    }

    /// Documentos atualmente retidos; cada um custa exatamente um texto.
    pub fn len(&self) -> usize {
        self.documentos.len()
    }

    /// URIs dos documentos abertos, sem reter cópia dos textos.
    pub(crate) fn uris(&self) -> impl Iterator<Item = &str> {
        self.documentos.keys().map(String::as_str)
    }

    /// Verdadeiro quando nenhum documento está aberto.
    pub fn is_empty(&self) -> bool {
        self.documentos.is_empty()
    }

    /// Diagnostica o texto vigente pelo parser novo.
    ///
    /// Lista vazia quando o documento está fechado; nunca retém o resultado,
    /// que é transitório do chamador.
    pub fn diagnose_open(&self, uri: &str) -> Vec<Diagnostic> {
        match self.get(uri) {
            Some(texto) => AnalisadorSintatico::new().diagnosticar(uri, texto),
            None => Vec::new(),
        }
    }
}

/// Análise que produz os diagnósticos publicados após cada mudança.
///
/// O [`Servidor`] chama este trait, nunca o parser diretamente. A
/// assinatura leva `uri` e `texto` (não o documento) para que a
/// implementação possa consultar outros arquivos. Os diagnósticos tipados
/// não passam por [`Analisador::diagnosticar`] (que é síncrono): o servidor
/// os pede ao trabalhador em segundo plano com o SDK de
/// [`Analisador::sdk_para_diagnosticos`].
pub trait Analisador {
    /// Diagnostica o texto vigente e devolve spans em bytes UTF-8.
    ///
    /// O resultado é transitório do chamador: nada é retido entre chamadas,
    /// para que N edições não retenham N análises (o modo de falha do LSP do
    /// Dart, medido no PLANO.md).
    fn diagnosticar(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic>;

    /// Símbolos sintáticos do documento, sem guardar a árvore entre edições.
    fn simbolos(&mut self, _uri: &str, _texto: &str) -> Vec<serde_json::Value> {
        Vec::new()
    }

    /// Definição conservadora da posição no texto: URI e seleção no destino.
    fn definicao(&mut self, _uri: &str, _texto: &str, _offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        None
    }

    /// Variante com os buffers abertos, para resolver imports ainda não
    /// salvos sem duplicar documentos no analisador residente.
    fn definicao_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, _documentos: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        self.definicao(uri, texto, offset)
    }

    /// Descrição, tipo e documentação do que está sob o cursor.
    fn hover(&mut self, _uri: &str, _texto: &str, _offset: usize) -> Option<Hover> {
        None
    }

    /// Variante com os buffers abertos (textos ainda não salvos).
    fn hover_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, _documentos: &DocumentStore) -> Option<Hover> {
        self.hover(uri, texto, offset)
    }

    /// Itens de completar na posição `offset` (bytes) do documento aberto
    /// `uri`. `None` quando a análise não sabe responder (sem SDK, por
    /// exemplo); lista vazia quando não há o que oferecer.
    ///
    /// ```
    /// use dartforge_lsp::{Analisador, AnalisadorSemantico, AnalisadorSintatico, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f(int x) { x. }".into());
    /// // Sem tipos não há o que completar; o semântico sem SDK também não sabe.
    /// assert!(AnalisadorSintatico::new().completar(&docs, "file:///a.dart", 18).is_none());
    /// assert!(AnalisadorSemantico::novo(None).completar(&docs, "file:///a.dart", 18).is_none());
    /// ```
    fn completar(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize) -> Option<Completar> {
        None
    }

    /// O `maxCompletionItems` da configuração do cliente (o `maxSuggestions`
    /// do coletor e do truncamento).
    fn definir_maximo_de_completar(&mut self, _maximo: usize) {}

    /// `prepareRename`: intervalo (bytes) e texto do nome renomeável sob o
    /// cursor; `Ok(None)` quando não há nome ali.
    ///
    /// # Erros
    ///
    /// Mensagem para o usuário quando o elemento existe mas não pode ser
    /// renomeado (SDK, pacote externo, forma ainda não suportada).
    fn preparar_renomeacao(
        &mut self,
        _documentos: &DocumentStore,
        _uri: &str,
        _offset: usize,
    ) -> Result<Option<(dartforge_diagnostics::Span, String)>, String> {
        Ok(None)
    }

    /// `rename`: as edições em todos os arquivos do projeto.
    ///
    /// ```
    /// use dartforge_lsp::{Analisador, AnalisadorSintatico, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "var x = 1;".into());
    /// let mut a = AnalisadorSintatico::new();
    /// assert_eq!(a.preparar_renomeacao(&docs, "file:///a.dart", 4), Ok(None));
    /// assert!(a.renomear(&docs, "file:///a.dart", 4, "y").is_err());
    /// ```
    ///
    /// # Erros
    ///
    /// Nome inválido, elemento não renomeável ou conflito, com a mensagem
    /// para o usuário.
    fn renomear(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize, _novo: &str) -> Result<Renomeacao, String> {
        Err("Renomear exige a análise semântica (SDK do Dart).".into())
    }

    /// `codeAction`: correções e assistências para o intervalo `inicio..fim`
    /// (bytes) do documento aberto. `publicados` são os diagnósticos tipados
    /// já publicados para a versão vigente (o servidor não passa os de uma
    /// versão velha). O padrão oferece as correções dos diagnósticos
    /// sintáticos (inserir `;`).
    ///
    /// ```
    /// use dartforge_lsp::{Analisador, AnalisadorSintatico, DocumentStore};
    /// let mut docs = DocumentStore::new();
    /// docs.open("file:///a.dart".into(), 1, "void f() { var x = 1 }".into());
    /// let acoes = AnalisadorSintatico::new().acoes(&docs, "file:///a.dart", 19, 19, &[]);
    /// assert_eq!(acoes[0].titulo, "Insert ';'");
    /// assert_eq!(acoes[0].edicoes[0].texto, ";");
    /// ```
    fn acoes(&mut self, documentos: &DocumentStore, uri: &str, inicio: usize, fim: usize, _publicados: &[Diagnostic]) -> Vec<AcaoDeCodigo> {
        let Some(texto) = documentos.get(uri) else { return Vec::new() };
        let diagnosticos = self.diagnosticar(uri, texto);
        let mut saida = acoes::corrigir_sintaxe(uri, texto, &diagnosticos, inicio, fim);
        saida.push(acoes::organizar_imports(uri, texto));
        saida
    }

    /// As refatorações listadas em `offset..offset + comprimento`
    /// (docs/LSP-ESPECIFICACAO.md §13.11.1); `criar_arquivos` é o
    /// `supportsFileCreation` do cliente.
    fn refatoracoes(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize, _comprimento: usize, _criar_arquivos: bool) -> Vec<Refatoracao> {
        Vec::new()
    }

    /// `refactor.perform`/`refactor.validate` de uma refatoração legada
    /// (§13.11.2).
    fn executar_refatoracao(&mut self, _documentos: &DocumentStore, _uri: &str, _pedido: &PedidoDeRefatoracao) -> ResultadoDeRefatoracao {
        ResultadoDeRefatoracao::NaoAnalisado
    }

    /// `dart.refactor.move_top_level_to_file` para o arquivo `destino`
    /// (§13.11.9 e).
    fn mover_para_arquivo(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize, _comprimento: usize, _destino: &str) -> ResultadoDeRefatoracao {
        ResultadoDeRefatoracao::NaoAnalisado
    }

    /// Descarta estado associado ao documento quando ele sai do editor.
    fn documento_fechado(&mut self, _uri: &str) {}

    /// O texto de `uri` mudou (`didOpen` ou `didChange` aceito): o que o
    /// analisador retém de consultas anteriores deixa de valer e cai aqui.
    fn documento_alterado(&mut self, _uri: &str) {}

    /// O `lib/` do SDK com que o servidor roda, em segundo plano, a análise
    /// tipada do `dartforge analyze` sobre os documentos abertos
    /// (`docs/LSP.md`, "Diagnósticos tipados"). `None` (o padrão): só os
    /// diagnósticos de [`Analisador::diagnosticar`].
    fn sdk_para_diagnosticos(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Referências conservadoras no próprio documento: declaração primeiro,
    /// depois os usos, todos como spans em bytes UTF-8.
    ///
    /// Só responde nos mesmos casos seguros de [`Analisador::definicao`]
    /// (tipo, variável, função ou getter de topo únicos, sem imports nem
    /// sombras): `None` significa "não sei", nunca "não há". O resultado é
    /// transitório do chamador, como nos demais métodos.
    fn referencias(&mut self, _uri: &str, _texto: &str, _offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        None
    }

    /// Referências no workspace: a declaração e os usos, cada um com a URI
    /// do arquivo. O padrão delega ao próprio documento. Nada é retido entre
    /// pedidos além do que a política de sessão do analisador permite.
    fn referencias_em(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Referencias> {
        let texto = documentos.get(uri)?;
        let spans = self.referencias(uri, texto, offset)?;
        Some(Referencias::de_lista(uri, spans))
    }

    /// Ajuda de assinatura na lista de argumentos que contém `offset`
    /// (`textDocument/signatureHelp`). `automatica`: o editor pediu ao
    /// digitar `(`; só responde se esse `(` abre a lista. O padrão: nenhuma.
    fn assinatura(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize, _automatica: bool) -> Option<Assinatura> {
        None
    }

    /// As ocorrências, no próprio documento, do que `offset` denota
    /// (`textDocument/documentHighlight`). O padrão usa as referências
    /// conservadoras do documento.
    fn destaques(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let texto = documentos.get(uri)?.to_string();
        self.referencias(uri, &texto, offset)
    }

    /// Implementações (`textDocument/implementation`): subtipos de uma
    /// classe ou as sobrescritas de um membro, com a URI de cada uma.
    fn implementacoes(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize) -> Vec<(String, dartforge_diagnostics::Span)> {
        Vec::new()
    }

    /// A declaração do tipo estático do que `offset` denota
    /// (`textDocument/typeDefinition`).
    fn definicao_de_tipo(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize) -> Option<(String, dartforge_diagnostics::Span)> {
        None
    }

    /// Regiões de dobra (`textDocument/foldingRange`); `so_linhas`: o
    /// cliente anunciou `lineFoldingOnly`.
    fn dobras(&mut self, _uri: &str, _texto: &str, _so_linhas: bool) -> Vec<Dobra> {
        Vec::new()
    }

    /// Faixas de seleção em `offset`, da mais interna à mais externa
    /// (`textDocument/selectionRange`).
    fn selecoes(&mut self, _uri: &str, _texto: &str, _offset: usize) -> Vec<dartforge_diagnostics::Span> {
        Vec::new()
    }

    /// Os rótulos de fechamento do documento
    /// (`dart/textDocument/publishClosingLabels`): o intervalo (bytes) do nó
    /// e o texto. O padrão não tem nenhum.
    fn rotulos_de_fechamento(&mut self, _documentos: &DocumentStore, _uri: &str) -> Vec<(dartforge_diagnostics::Span, String)> {
        Vec::new()
    }

    /// Dicas embutidas do documento inteiro (`textDocument/inlayHint`).
    fn dicas(&mut self, _documentos: &DocumentStore, _uri: &str) -> Vec<Dica> {
        Vec::new()
    }

    /// Tokens semânticos do documento já codificados para o protocolo
    /// (grupos de 5 números); `multilinha`: o cliente aceita tokens de
    /// várias linhas; `faixa` (bytes): só os que a tocam.
    fn tokens_semanticos(&mut self, _documentos: &DocumentStore, _uri: &str, _multilinha: bool, _faixa: Option<(usize, usize)>) -> Option<Vec<u32>> {
        None
    }

    /// O executável em `offset` como item da hierarquia de chamadas
    /// (`textDocument/prepareCallHierarchy`).
    fn preparar_chamadas(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize) -> Option<ItemDeChamada> {
        None
    }

    /// Chamadas recebidas (`true`) ou feitas pelo item do cliente: o nome
    /// em `offset` de `uri`, o nome exibido (o `_isMatchingElement`) e se a
    /// espécie é construtor (o construtor sem nome implícito da classe); os
    /// intervalos de cada uma (no arquivo de quem chama, nas recebidas).
    fn chamadas(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize, _nome: &str, _construtor: bool, _recebidas: bool) -> Vec<(ItemDeChamada, Vec<dartforge_diagnostics::Span>)> {
        Vec::new()
    }

    /// O item da hierarquia de tipos em `offset`
    /// (`textDocument/prepareTypeHierarchy`).
    fn preparar_hierarquia(&mut self, _documentos: &DocumentStore, _uri: &str, _offset: usize) -> Option<ItemDeTipo> {
        None
    }

    /// Supertipos (`true`) ou subtipos diretos da classe cujo
    /// `ElementLocation` é `referencia` (o `data.ref` do item), no projeto de
    /// `uri`; `ancora` é o `data.anchor` (a referência e o caminho). `None`
    /// quando a referência não localiza uma classe (resposta `null`).
    fn hierarquia(&mut self, _documentos: &DocumentStore, _uri: &str, _referencia: &str, _ancora: Option<(&str, &[usize])>, _supertipos: bool) -> Option<Vec<ItemDeTipo>> {
        None
    }
}

/// Um item da hierarquia de tipos: o nome exibido (`Base<int>`), o arquivo,
/// a declaração inteira (com a documentação) e o nome, o `ElementLocation`
/// da classe e a âncora (referência e caminho) dos supertipos com
/// argumentos de tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDeTipo {
    pub nome: String,
    pub uri: String,
    pub intervalo: dartforge_diagnostics::Span,
    pub selecao: dartforge_diagnostics::Span,
    pub referencia: String,
    pub ancora: Option<(String, Vec<usize>)>,
}

/// Análise sintática: o parser novo, sem resolução (nomes e tipos chegam depois).
///
/// Cada chamada interna num [`dartforge_intern::Interner`] novo e o descarta
/// com a árvore: só os diagnósticos (mensagem + span) atravessam a chamada.
/// Aceita 100% do SDK, do corpus pub e do `new_sali`; arquivos válidos não
/// geram diagnósticos.
///
/// ```
/// use dartforge_lsp::{Analisador, AnalisadorSintatico};
/// let mut a = AnalisadorSintatico::new();
/// assert!(a.diagnosticar("file:///a.dart", "void main() {}").is_empty());
/// assert_eq!(a.diagnosticar("file:///b.dart", "void a() { int x = ; }").len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct AnalisadorSintatico {
    /// `package_config.json` já lidos, pelo caminho (um por projeto aberto):
    /// a versão de linguagem padrão de cada arquivo vem dele.
    configs: std::collections::HashMap<std::path::PathBuf, dartforge_elements::config::PackageConfig>,
    /// Versão padrão (a do pacote) de cada documento já visto.
    padroes: std::collections::HashMap<String, dartforge_frontend::LanguageVersion>,
}

impl AnalisadorSintatico {
    /// Cria o analisador sintático.
    pub fn new() -> Self {
        Self::default()
    }

    /// Os recursos de linguagem do arquivo `uri` (`docs/VERSOES-LINGUAGEM.md`
    /// §2): o marcador `// @dart = x.y`, senão o `languageVersion` do pacote
    /// no `package_config.json` que o contém, senão a versão corrente. Sem
    /// isso, um projeto 3.6 veria erro em `final` de parâmetro (proibido na
    /// 3.13) e um 3.13 veria erro em construtor primário.
    pub(crate) fn features(&mut self, uri: &str, texto: &str) -> dartforge_frontend::LibraryFeatures {
        use dartforge_frontend::{LanguageVersion, LibraryFeatures};
        if let Some((v, _)) = dartforge_frontend::features::marcador_versao(texto) {
            return LibraryFeatures::new(v, &[]);
        }
        if let Some(v) = self.padroes.get(uri) {
            return LibraryFeatures::new(*v, &[]);
        }
        // Uma vez por documento: o caminho canônico (é o que o
        // `package_config` guarda) e o pacote que o contém.
        let caminho = url::Url::parse(uri)
            .ok()
            .filter(|u| u.scheme() == "file")
            .and_then(|u| u.to_file_path().ok())
            .map(|p| dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&p).unwrap_or(p)));
        let padrao = caminho
            .as_deref()
            .and_then(|p| {
                let cfg = dartforge_elements::config::PackageConfig::discover(p)?;
                if !self.configs.contains_key(&cfg) {
                    let lido = dartforge_elements::config::PackageConfig::load(&cfg).ok()?;
                    self.configs.insert(cfg.clone(), lido);
                }
                self.configs.get(&cfg)?.pacote_da_biblioteca(uri, Some(p))?.language_version
            })
            .unwrap_or(LanguageVersion::ATUAL);
        self.padroes.insert(uri.to_string(), padrao);
        LibraryFeatures::new(padrao, &[])
    }

    /// Só os diagnósticos do parser (sempre publicados), na versão de
    /// linguagem do arquivo.
    pub(crate) fn sintaxe(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, texto);
        let mut nomes = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
        // Nome e texto do analyzer que é a referência do arquivo (T2).
        let referencia = parsed.referencia;
        parsed.diagnostics.into_iter().filter_map(|d| d.na_referencia(referencia)).collect()
    }
}

impl Analisador for AnalisadorSintatico {
    /// Analisa com o parser completo, na versão de linguagem do arquivo, com
    /// recuperação por declaração.
    ///
    /// Depois da sintaxe, os verificadores de `crates/analise` que não
    /// dependem de tipos (nomes duplicados, locais não usados) sobre o próprio
    /// arquivo; destes, só sai o que a regra de publicação deixa
    /// (`dartforge_analise::publicacao`): código verificado contra o oráculo,
    /// e não suprimido por `// ignore:` (o filtro do `dartforge analyze`).
    fn diagnosticar(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, texto);
        let mut nomes = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
        let mut saida = parsed.diagnostics;
        // `experiment_not_enabled` não desmonta a árvore (o recurso é lido inteiro).
        let erros_sintaticos: Vec<dartforge_diagnostics::Span> = saida
            .iter()
            .filter(|d| !d.code.is_some_and(|c| c.info().nome == "experiment_not_enabled"))
            .map(|d| d.span)
            .collect();
        let unidade = dartforge_analise::Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte: texto };
        let curinga = features.tem(dartforge_frontend::features::Feature::WildcardVariables);
        let referencia = parsed.referencia;
        let juntar = referencia == dartforge_diagnostics::Referencia::V3_6;
        let semanticos = dartforge_analise::duplicatas::duplicatas(&[unidade], &nomes, curinga, juntar)
            .into_iter()
            .map(|(_, d)| d)
            .chain(dartforge_analise::enums::sem_constantes(&[unidade]).into_iter().map(|(_, d)| d))
            .chain(dartforge_analise::inicializacao::finais_nao_inicializados(&[unidade], &nomes).into_iter().map(|(_, d)| d))
            .chain(dartforge_analise::locais::nao_usados(unidade, &nomes, curinga, &erros_sintaticos))
            .chain(dartforge_analise::externos::inicializadores(unidade))
            .chain(dartforge_analise::operadores::aridade(unidade, &nomes));
        // `// ignore:` como no `dartforge analyze` (o mesmo filtro).
        let ignorados = dartforge_paridade::filtros::Ignorados::de_texto(texto);
        let linhas = dartforge_paridade::json::Linhas::new(texto);
        saida.extend(
            semanticos
                .filter(|d| dartforge_analise::publicacao::publicado(d, false))
                .filter(|d| !ignorados.ignora(d, linhas.ponto(d.span.start).line)),
        );
        // Por último, o nome e o texto do analyzer de referência (T2): os
        // filtros acima conhecem os códigos pelo nome do 3.6.
        saida.into_iter().filter_map(|d| d.na_referencia(referencia)).collect()
    }

    fn simbolos(&mut self, uri: &str, texto: &str) -> Vec<serde_json::Value> {
        let features = self.features(uri, texto);
        simbolos::do_documento(texto, features)
    }

    fn definicao(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        let features = self.features(uri, texto);
        match navegacao::destino(uri, texto, features, offset)? {
            navegacao::Alvo::Arquivo(destino) => Some((destino, None)),
            navegacao::Alvo::NomeLocal(tipo) => Some((uri.to_string(), Some(tipo.declaracao))),
        }
    }

    fn hover(&mut self, uri: &str, texto: &str, offset: usize) -> Option<Hover> {
        let features = self.features(uri, texto);
        match navegacao::destino(uri, texto, features, offset)? {
            navegacao::Alvo::Arquivo(_) => None,
            navegacao::Alvo::NomeLocal(tipo) => Some(Hover {
                intervalo: tipo.referencia,
                descricao: tipo.descricao?,
                tipo: tipo.tipo_estatico,
                documentacao: None,
                biblioteca: None,
            }),
        }
    }

    fn documento_fechado(&mut self, uri: &str) {
        self.padroes.remove(uri);
        // A configuração pode ter mudado enquanto o arquivo estava fechado.
        // A próxima análise a carrega novamente; nada do projeto fechado fica
        // retido indefinidamente na sessão do servidor.
        self.configs.clear();
    }

    fn referencias(&mut self, uri: &str, texto: &str, offset: usize) -> Option<Vec<dartforge_diagnostics::Span>> {
        let features = self.features(uri, texto);
        navegacao::referencias(uri, texto, features, offset)
    }

    fn definicao_no_workspace(&mut self, uri: &str, _texto: &str, offset: usize, documentos: &DocumentStore) -> Option<(String, Option<dartforge_diagnostics::Span>)> {
        navegacao::definicao_em(documentos, uri, offset, self)
    }

    fn hover_no_workspace(&mut self, uri: &str, _texto: &str, offset: usize, documentos: &DocumentStore) -> Option<Hover> {
        let (intervalo, descricao, tipo) = navegacao::hover_em(documentos, uri, offset, self)?;
        Some(Hover { intervalo, descricao, tipo, documentacao: None, biblioteca: None })
    }

    fn dobras(&mut self, uri: &str, texto: &str, so_linhas: bool) -> Vec<Dobra> {
        let features = self.features(uri, texto);
        estrutura::dobras(texto, features, so_linhas)
    }

    fn selecoes(&mut self, uri: &str, texto: &str, offset: usize) -> Vec<dartforge_diagnostics::Span> {
        let features = self.features(uri, texto);
        estrutura::selecoes(texto, features, offset)
    }

    fn referencias_em(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Referencias> {
        let mut achados = navegacao::referencias_em(documentos, uri, offset, self)?.into_iter();
        let declaracao = achados.next();
        Some(Referencias { declaracao, usos: achados.collect() })
    }
}
