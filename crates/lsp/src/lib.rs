//! Servidor LSP do DartForge: documentos, sincronização incremental e despacho.
//!
//! O desenho copia o do rust-analyzer (`references/rust-analyzer`), não o
//! código: um [`Servidor`] dono de tudo (documentos, fila, cancelamentos),
//! passado por `&mut` ao despacho; documentos em memória versionados como em
//! `mem_docs.rs`; fila com cancelamento como em `op_queue.rs`; conversão
//! UTF-16 como em `lsp/utils.rs` (aqui em [`utf16`]).
//!
//! Dois analisadores convivem, por regra de coexistência com os outros
//! agentes: [`diagnose`] usa o compilador do subconjunto antigo e serve só
//! aos testes legados; o servidor diagnostica pelo parser novo
//! (`dartforge_frontend::parser::parse`) através do [`trait Analisador`],
//! cuja implementação sintática de hoje ([`AnalisadorSintatico`]) será
//! trocada pela semântica (`crates/types`) sem tocar no transporte.

pub mod servidor;
pub mod transporte;
pub mod utf16;

use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;
use utf16::TabelaLinhas;

pub use servidor::Servidor;

/// Analisa o texto e retorna **todos** os diagnósticos que o pipeline encontra.
///
/// O parser recupera de erros em fronteiras de declaração, então um arquivo com
/// três declarações quebradas rende três diagnósticos, ordenados por span
/// (início, depois fim). Um editor que mostrasse um erro por arquivo obrigaria a
/// recompilar a cada correção para descobrir o próximo, o que é inútil.
///
/// As fases posteriores à sintaxe — macros, mixins, semântica, emissão — ainda
/// param no primeiro erro, e por isso a lista volta a ter no máximo um elemento
/// assim que a sintaxe do arquivo está correta.
///
/// A lista vazia indica que o programa pertence ao subconjunto aceito pelo compilador.
/// Isso não equivale a uma análise completa de toda a linguagem Dart.
///
/// Rota legada: usa o compilador do subconjunto antigo e **não** serve para
/// arquivos reais (recusaria quase todo arquivo válido com erros que não são
/// erros). O servidor usa [`AnalisadorSintatico`]; esta função fica para os
/// testes antigos.
///
/// # Exemplos
///
/// ```
/// assert!(dartforge_lsp::diagnose("void main() {}").is_empty());
/// assert_eq!(dartforge_lsp::diagnose("void main() { print(desconhecido); }").len(), 1);
/// let tres = "void a() { int ; } void b() { int ; } void main() { int ; }";
/// assert_eq!(dartforge_lsp::diagnose(tres).len(), 3);
/// ```
pub fn diagnose(source: &str) -> Vec<Diagnostic> {
    dartforge_compiler::compile_diagnostics(source)
}

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

    /// Verdadeiro quando nenhum documento está aberto.
    pub fn is_empty(&self) -> bool {
        self.documentos.is_empty()
    }

    /// Diagnostica o texto vigente com a rota multi-erro do compilador.
    ///
    /// Lista vazia quando o documento está fechado ou pertence ao subconjunto;
    /// nunca retém o resultado, que é transitório do chamador.
    pub fn diagnose_open(&self, uri: &str) -> Vec<Diagnostic> {
        self.get(uri).map_or_else(Vec::new, diagnose)
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
}

/// Análise sintática: o parser novo, sem resolução (nomes e tipos chegam depois).
///
/// Cada chamada interna num [`dartforge_intern::Interner`] novo e o descarta
/// com a árvore: só os diagnósticos (mensagem + span) atravessam a chamada.
/// Aceita 100% do SDK, do corpus pub e do `new_sali`; arquivos válidos não
/// geram diagnósticos, ao contrário da rota legada [`diagnose`].
///
/// ```
/// use dartforge_lsp::{Analisador, AnalisadorSintatico};
/// let mut a = AnalisadorSintatico::new();
/// assert!(a.diagnosticar("file:///a.dart", "void main() {}").is_empty());
/// assert_eq!(a.diagnosticar("file:///b.dart", "void a() { int x = ; }").len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct AnalisadorSintatico;

impl AnalisadorSintatico {
    /// Cria o analisador sintático (sem estado: cada chamada é independente).
    pub fn new() -> Self {
        Self
    }
}

impl Analisador for AnalisadorSintatico {
    /// Analisa com o parser completo de Dart 3.6, com recuperação por declaração.
    fn diagnosticar(&mut self, _uri: &str, texto: &str) -> Vec<Diagnostic> {
        let mut nomes = dartforge_intern::Interner::new();
        dartforge_frontend::parser::parse(texto, &mut nomes).diagnostics
    }
}
