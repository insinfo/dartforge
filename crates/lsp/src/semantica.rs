//! Consultas semânticas transitórias. A árvore, o programa e a tabela de tipos
//! pertencem a uma única requisição e caem antes da próxima versão do texto.

use crate::{Analisador, AnalisadorSintatico, DocumentStore, Hover, Referencias};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{gerado::Construtor, load::load_lenient_gerados, model::{Program, UnitId}, sdk::SdkLayout};
use dartforge_intern::Interner;
use url::Url;

/// Analisador com a resolução e os tipos da inferência comum: definição,
/// hover e referências pela identidade da declaração (`crate::projeto`),
/// completar, renomear e ações. Na falta do SDK, mantém as respostas
/// sintáticas.
pub struct AnalisadorSemantico {
    sintatico: AnalisadorSintatico,
    sdk: Option<SdkLayout>,
    /// Nomes públicos de topo do SDK (importar biblioteca), montado na
    /// primeira vez que um nome indefinido pede. Tamanho fixo pelo SDK.
    indice_sdk: Option<crate::acoes::IndiceSdk>,
}

impl AnalisadorSemantico {
    pub fn novo(sdk: Option<SdkLayout>) -> Self {
        Self { sintatico: AnalisadorSintatico::new(), sdk, indice_sdk: None }
    }

    /// Descobre o SDK pelos mesmos caminhos usados pelo compilador.
    pub fn descobrir() -> Self {
        let sdk = SdkLayout::discover().and_then(|p| SdkLayout::load(&p, "dartdevc").ok());
        Self::novo(sdk)
    }

    /// Carrega o programa de `uri` com `texto` e os demais documentos abertos
    /// nos textos vigentes (os outros arquivos vêm do disco).
    pub(crate) fn carregar(&self, uri: &str, texto: &str, documentos: Option<&DocumentStore>) -> Option<(Program, Interner, UnitId)> {
        let sdk = self.sdk.as_ref()?;
        let caminho = Url::parse(uri).ok()?.to_file_path().ok()?;
        if caminho.extension().is_none_or(|e| e != "dart") { return None; }
        let mut gerador = documentos.map_or_else(Construtor::nova, Self::abertos);
        // A chamada direta da trait também usa o texto recebido, mesmo sem
        // DocumentStore. O arquivo da requisição prevalece sobre a coleção.
        gerador.por(caminho.clone(), texto.to_owned(), "lsp", vec![]);
        // Uma parte não é biblioteca: a carga entra pela dona, que a inclui.
        // Se a dona não a declara (`part` ausente), a parte entra sozinha.
        let dona = biblioteca_dona(&caminho, texto, documentos);
        let geracao = gerador.concluir(1).ok()?;
        let chave = dartforge_elements::gerado::chave(&caminho);
        for entrada in dona.iter().chain([&caminho]) {
            let mut nomes = Interner::new();
            let (programa, _) = load_lenient_gerados(entrada, sdk, None, &mut nomes, None, None, Some(geracao.clone()));
            let unidade = programa.units.iter().position(|u| u.path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&chave));
            if let Some(unidade) = unidade {
                return Some((programa, nomes, UnitId(unidade as u32)));
            }
        }
        None
    }

    /// Índice dos nomes públicos do SDK, montado na primeira chamada.
    pub(crate) fn indice_sdk(&mut self) -> &crate::acoes::IndiceSdk {
        let sdk = self.sdk.as_ref();
        self.indice_sdk.get_or_insert_with(|| sdk.map(crate::acoes::indexar_sdk).unwrap_or_default())
    }

    /// SDK carregado, quando há.
    pub(crate) fn sdk(&self) -> Option<&SdkLayout> {
        self.sdk.as_ref()
    }

    /// Geração em memória com os textos vigentes dos documentos `.dart`
    /// abertos: na carga, eles valem mais que o disco.
    pub(crate) fn abertos(documentos: &DocumentStore) -> Construtor {
        let mut gerador = Construtor::nova();
        for aberto in documentos.uris() {
            let Some(fonte) = documentos.get(aberto) else { continue };
            let Some(arquivo) = Url::parse(aberto).ok().and_then(|u| u.to_file_path().ok()) else { continue };
            if arquivo.extension().is_some_and(|e| e == "dart") {
                gerador.por(arquivo, fonte.to_owned(), "lsp", vec![]);
            }
        }
        gerador
    }

    /// Um [`DocumentStore`] só com `uri`, para as chamadas da trait que
    /// recebem o texto sem a coleção de abertos.
    fn so_este(uri: &str, texto: &str) -> DocumentStore {
        let mut docs = DocumentStore::new();
        docs.open(uri.to_string(), 0, texto.to_string());
        docs
    }

    /// Definição pela identidade semântica (`crate::projeto`): o literal de
    /// diretiva continua sintático (o arquivo apontado); nomes vêm da
    /// resolução da inferência comum. Sem SDK, ou se a biblioteca não
    /// carrega, valem as respostas sintáticas conservadoras.
    fn definir(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<(String, Option<Span>)> {
        if let Some((destino, None)) = self.sintatico.definicao(uri, texto, offset) {
            return Some((destino, None));
        }
        if let Some(projeto) = crate::projeto::carregar_biblioteca(self, documentos, uri)
            && let Some(unidade) = projeto.unidade_do_uri(uri)
            && let Ok(Some(d)) = projeto.identificar(unidade, offset)
            && let Some((u, span)) = projeto.declaracao(&d)
        {
            return Some((projeto.uri_da_unidade(u)?, Some(span)));
        }
        self.sintatico.definicao_no_workspace(uri, texto, offset, documentos)
    }

    fn passar_hover(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<Hover> {
        if let Some(projeto) = crate::projeto::carregar_biblioteca(self, documentos, uri)
            && let Some(unidade) = projeto.unidade_do_uri(uri)
            && let Ok(Some(d)) = projeto.identificar(unidade, offset)
            && let Some(h) = projeto.hover(unidade, &d)
        {
            return Some(h);
        }
        self.sintatico.hover_no_workspace(uri, texto, offset, documentos)
    }

    /// Referências pela identidade da declaração, em todas as bibliotecas
    /// do projeto (abertas ou só no disco). Declarações de dependências (SDK,
    /// pacotes) entram como declaração, mas os usos só são procurados no
    /// projeto.
    fn referencias_semanticas(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Referencias> {
        let projeto = crate::projeto::carregar_projeto(self, documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let d = projeto.identificar(unidade, offset).ok()??;
        let declaracao = projeto.declaracao(&d);
        let declaracoes = projeto.declaracoes(&d.alvo);
        let mut usos: Vec<(String, Span)> = Vec::new();
        for (u, de, ate) in projeto.ocorrencias(&d.alvo, false).ok()? {
            if declaracoes.contains(&(u, de, ate)) {
                continue;
            }
            usos.push((projeto.uri_da_unidade(u)?, Span { start: de, end: ate }));
        }
        usos.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
        let declaracao = declaracao.and_then(|(u, s)| Some((projeto.uri_da_unidade(u)?, s)));
        Some(Referencias { declaracao, usos })
    }
}

/// A biblioteca dona de `caminho` quando o texto é uma parte (`part of`):
/// pela URI escrita (relativa ou `package:`), ou, na forma antiga
/// `part of nome;`, pelo arquivo do projeto que declara `part` para ela.
pub(crate) fn biblioteca_dona(caminho: &std::path::Path, texto: &str, documentos: Option<&DocumentStore>) -> Option<std::path::PathBuf> {
    use dartforge_elements::gerado::chave;
    use dartforge_frontend::ast::DirectiveKind;
    if !texto.contains("part") {
        return None;
    }
    let mut nomes = Interner::new();
    let analisado = dartforge_frontend::parser::parse(texto, &mut nomes);
    let uri = analisado.unit.directives.iter().find_map(|d| match &d.kind {
        DirectiveKind::PartOf { uri, .. } => Some(uri.as_ref().and_then(dartforge_elements::load::string_lit_value)),
        _ => None,
    })?;
    if let Some(uri) = uri {
        if uri.starts_with("package:") {
            let config = dartforge_elements::config::PackageConfig::discover(caminho)?;
            return dartforge_elements::config::PackageConfig::load(&config).ok()?.resolve_package_uri(&uri).ok();
        }
        return Some(chave(&caminho.parent()?.join(uri)));
    }
    let alvo = chave(caminho);
    let raiz = crate::projeto::raiz_do_projeto(caminho);
    crate::projeto::arquivos_do_projeto(&raiz).into_iter().find(|candidato| {
        let aberto = documentos
            .and_then(|d| Url::from_file_path(candidato).ok().and_then(|u| d.get(u.as_str()).map(str::to_string)));
        let Some(fonte) = aberto.or_else(|| std::fs::read_to_string(candidato).ok()) else { return false };
        if !fonte.contains("part") {
            return false;
        }
        let mut nomes = Interner::new();
        let analisado = dartforge_frontend::parser::parse(&fonte, &mut nomes);
        analisado.unit.directives.iter().any(|d| match &d.kind {
            DirectiveKind::Part { uri } => dartforge_elements::load::string_lit_value(uri)
                .and_then(|u| candidato.parent().map(|dir| chave(&dir.join(u))))
                .is_some_and(|p| p == alvo),
            _ => false,
        })
    })
}

impl Analisador for AnalisadorSemantico {
    fn diagnosticar(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic> {
        self.sintatico.diagnosticar(uri, texto)
    }

    fn simbolos(&mut self, uri: &str, texto: &str) -> Vec<serde_json::Value> {
        self.sintatico.simbolos(uri, texto)
    }

    fn definicao(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(String, Option<Span>)> {
        let so_este = Self::so_este(uri, texto);
        self.definir(uri, texto, offset, &so_este)
    }

    fn definicao_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<(String, Option<Span>)> {
        self.definir(uri, texto, offset, documentos)
    }

    fn referencias(&mut self, uri: &str, texto: &str, offset: usize) -> Option<Vec<Span>> {
        self.sintatico.referencias(uri, texto, offset)
    }

    fn referencias_em(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Referencias> {
        if self.sdk.is_some() && documentos.get(uri).is_some() {
            return self.referencias_semanticas(documentos, uri, offset);
        }
        self.sintatico.referencias_em(documentos, uri, offset)
    }

    fn hover(&mut self, uri: &str, texto: &str, offset: usize) -> Option<Hover> {
        let so_este = Self::so_este(uri, texto);
        self.passar_hover(uri, texto, offset, &so_este)
    }

    fn hover_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<Hover> {
        self.passar_hover(uri, texto, offset, documentos)
    }

    fn documento_fechado(&mut self, uri: &str) {
        self.sintatico.documento_fechado(uri);
    }

    fn sdk_para_diagnosticos(&self) -> Option<std::path::PathBuf> {
        self.sdk.as_ref().map(|s| s.root.clone())
    }

    fn preparar_renomeacao(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
        let Some(projeto) = crate::projeto::carregar_projeto(self, documentos, uri) else { return Ok(None) };
        crate::renomear::preparar(&projeto, uri, offset)
    }

    fn renomear(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, novo: &str) -> Result<crate::Renomeacao, String> {
        let projeto = crate::projeto::carregar_projeto(self, documentos, uri)
            .ok_or_else(|| "O projeto do arquivo não pôde ser carregado (SDK ausente ou URI que não é de arquivo).".to_string())?;
        crate::renomear::renomear(&projeto, uri, offset, novo)
    }

    fn acoes(&mut self, documentos: &DocumentStore, uri: &str, inicio: usize, fim: usize) -> Vec<crate::AcaoDeCodigo> {
        let Some(texto) = documentos.get(uri) else { return Vec::new() };
        let diagnosticos = self.diagnosticar(uri, texto);
        let mut saida = crate::acoes::corrigir_sintaxe(uri, &diagnosticos, inicio, fim);
        saida.extend(crate::acoes::importar(self, documentos, uri, inicio, fim));
        saida
    }

    fn completar(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<crate::Completar> {
        let texto = documentos.get(uri)?;
        let features = self.sintatico.features(uri, texto);
        crate::completar::completar(self, documentos, uri, texto, offset, features)
    }
}
