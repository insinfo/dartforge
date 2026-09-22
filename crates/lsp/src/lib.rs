//! Adaptador de diagnósticos para o futuro servidor LSP.
//!
//! Esta crate ainda não implementa transporte, JSON-RPC ou sincronização de documentos.
use dartforge_diagnostics::Diagnostic;
use std::collections::HashMap;

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

/// Texto vigente de um documento aberto no editor.
#[derive(Debug, Clone)]
struct OpenDocument {
    /// Versão LSP mais recente recebida; cresce a cada `didChange`.
    version: i32,
    /// Texto integral vigente. É o único estado retido por documento.
    text: String,
}

/// Dono explícito dos documentos abertos: quem descarta a versão N−1.
///
/// `CompilerSession` retém exatamente um snapshot porque há uma compilação por
/// vez; um LSP tem N arquivos abertos × M versões, e o desenho precisa dizer
/// quem descarta a versão anterior. Aqui o dono é este mapa: cada documento tem
/// **uma** entrada, e a chegada da versão N substitui a N−1 no lugar — o `drop`
/// da `String` antiga é determinístico, não elegibilidade futura para um
/// coletor. Fechar o documento remove a entrada, e nada do documento sobrevive.
///
/// Sincronização integral por enquanto (`didChange` com texto completo): o
/// incremental por intervalos exige conversão UTF-16 correta e é o passo
/// seguinte, depois deste desenho de propriedade. Reenviar o arquivo inteiro a
/// cada tecla reintroduz custo, mas reter M versões reintroduz retenção — entre
/// os dois males, o custo transitório é o que não trava a máquina.
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
        self.documents.insert(uri, OpenDocument { version, text });
    }

    /// Aplica a versão N e descarta a N−1 no lugar; falso quando inexistente.
    ///
    /// Versão menor ou igual à vigente é ignorada e devolve falso: fora de
    /// ordem não pode ressuscitar texto velho.
    pub fn update(&mut self, uri: &str, version: i32, text: String) -> bool {
        match self.documents.get_mut(uri) {
            Some(current) if version > current.version => {
                current.version = version;
                current.text = text;
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

    /// Documentos atualmente retidos; cada um custa exatamente um texto.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Verdadeiro quando nenhum documento está aberto.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Diagnostica o texto vigente com a rota multi-erro do compilador.
    ///
    /// Lista vazia quando o documento está fechado ou pertence ao subconjunto;
    /// nunca retém o resultado, que é transitório do chamador.
    pub fn diagnose_open(&self, uri: &str) -> Vec<Diagnostic> {
        self.get(uri).map_or_else(Vec::new, diagnose)
    }
}
