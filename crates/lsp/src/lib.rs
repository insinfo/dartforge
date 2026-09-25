//! Servidor LSP do DartForge: documentos, sincronização incremental e despacho.
//!
//! O desenho copia o do rust-analyzer (`references/rust-analyzer`), não o
//! código: um [`Servidor`] dono de tudo (documentos, fila, cancelamentos),
//! passado por `&mut` ao despacho; documentos em memória versionados como em
//! `mem_docs.rs`; fila com cancelamento como em `op_queue.rs`; conversão
//! UTF-16 como em `lsp/utils.rs` (aqui em [`utf16`]).
//!
//! O servidor diagnostica pelo parser novo
//! (`dartforge_frontend::parser::parse`) através do [`trait Analisador`],
//! cuja implementação sintática de hoje ([`AnalisadorSintatico`]) será
//! trocada pela semântica (`crates/types`) sem tocar no transporte.

pub mod servidor;
mod navegacao;
mod semantica;
mod simbolos;
pub mod transporte;
pub mod utf16;

use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;
use utf16::TabelaLinhas;

pub use servidor::Servidor;
pub use semantica::AnalisadorSemantico;


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
/// A costura pela qual a semântica (`crates/types`) entrará sem tocar no
/// transporte: o [`Servidor`] chama este trait, nunca o parser diretamente.
/// A assinatura leva `uri` e `texto` (não o documento) para que a
/// implementação futura possa consultar outros arquivos.
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

    /// Descrição sintática segura e intervalo da referência sob o cursor.
    fn hover(&mut self, _uri: &str, _texto: &str, _offset: usize) -> Option<(dartforge_diagnostics::Span, String, Option<String>)> {
        None
    }

    fn hover_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, _documentos: &DocumentStore) -> Option<(dartforge_diagnostics::Span, String, Option<String>)> {
        self.hover(uri, texto, offset)
    }

    /// Descarta estado associado ao documento quando ele sai do editor.
    fn documento_fechado(&mut self, _uri: &str) {}
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
    fn features(&mut self, uri: &str, texto: &str) -> dartforge_frontend::LibraryFeatures {
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
}

impl Analisador for AnalisadorSintatico {
    /// Analisa com o parser completo, na versão de linguagem do arquivo, com
    /// recuperação por declaração.
    ///
    /// Depois da sintaxe, os verificadores de `crates/analise` que não
    /// dependem de tipos (nomes duplicados, locais não usados) sobre o próprio
    /// arquivo; destes, só sai o que a regra de publicação deixa
    /// (`dartforge_analise::publicacao`): código verificado contra o oráculo.
    fn diagnosticar(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic> {
        let features = self.features(uri, texto);
        let mut nomes = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse_com(texto, &mut nomes, features);
        let mut saida = parsed.diagnostics;
        let unidade = dartforge_analise::Unidade { ast: &parsed.ast, unit: &parsed.unit, fonte: texto };
        let curinga = features.tem(dartforge_frontend::features::Feature::WildcardVariables);
        let semanticos = dartforge_analise::duplicatas::duplicatas(&[unidade], &nomes, curinga)
            .into_iter()
            .map(|(_, d)| d)
            .chain(dartforge_analise::enums::sem_constantes(&[unidade]).into_iter().map(|(_, d)| d))
            .chain(dartforge_analise::inicializacao::finais_nao_inicializados(&[unidade], &nomes).into_iter().map(|(_, d)| d))
            .chain(dartforge_analise::locais::nao_usados(unidade, &nomes, curinga))
            .chain(dartforge_analise::externos::inicializadores(unidade))
            .chain(dartforge_analise::operadores::aridade(unidade, &nomes));
        saida.extend(semanticos.filter(|d| dartforge_analise::publicacao::publicado(d, false)));
        saida
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

    fn hover(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(dartforge_diagnostics::Span, String, Option<String>)> {
        let features = self.features(uri, texto);
        match navegacao::destino(uri, texto, features, offset)? {
            navegacao::Alvo::NomeLocal(tipo) => Some((tipo.referencia, tipo.descricao?, tipo.tipo_estatico)),
            navegacao::Alvo::Arquivo(_) => None,
        }
    }

    fn documento_fechado(&mut self, uri: &str) {
        self.padroes.remove(uri);
        // A configuração pode ter mudado enquanto o arquivo estava fechado.
        // A próxima análise a carrega novamente; nada do projeto fechado fica
        // retido indefinidamente na sessão do servidor.
        self.configs.clear();
    }
}
