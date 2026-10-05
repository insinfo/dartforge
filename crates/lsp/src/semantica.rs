//! O analisador semântico do LSP. O programa, as árvores e a tabela de tipos
//! de uma consulta ficam, no máximo, na sessão limitada de [`crate::sessao`]
//! (uma entrada, descartada a cada mudança de texto); o completar, que
//! analisa um texto com sentinela, é sempre transitório.

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
    indice_sdk: Option<crate::indice::IndiceSdk>,
    /// Nomes públicos de topo do projeto do último documento consultado,
    /// atualizado por arquivo (importação automática e ação de importar).
    indice_projeto: crate::indice::IndiceProjeto,
    /// O último programa carregado, reaproveitado enquanto nada mudou.
    sessao: crate::sessao::Sessao,
    /// Os resumos das bibliotecas conhecidas (namespace de exportação) do
    /// `ImportLibrary`.
    conhecidas: crate::conhecidas::IndiceDeBibliotecas,
    /// O `maxCompletionItems` vigente.
    maximo_de_completar: usize,
}

impl AnalisadorSemantico {
    /// Analisador com o SDK dado; sem SDK, só as respostas sintáticas.
    ///
    /// ```
    /// let a = dartforge_lsp::AnalisadorSemantico::novo(None);
    /// assert_eq!(a.estatisticas_da_sessao().carregadas, 0);
    /// ```
    pub fn novo(sdk: Option<SdkLayout>) -> Self {
        Self { sintatico: AnalisadorSintatico::new(), sdk, indice_sdk: None, indice_projeto: crate::indice::IndiceProjeto::default(), sessao: crate::sessao::Sessao::nova(), conhecidas: Default::default(), maximo_de_completar: crate::completar::MAXIMO_PADRAO }
    }

    /// Troca o orçamento da sessão semântica (MiB de fonte retida; `0`
    /// desliga a retenção). O padrão vem de `DARTFORGE_LSP_SESSAO_MIB`.
    ///
    /// ```
    /// use dartforge_lsp::AnalisadorSemantico;
    /// let a = AnalisadorSemantico::novo(None).com_orcamento_de_sessao(0);
    /// assert_eq!(a.estatisticas_da_sessao().fonte_retida, 0);
    /// ```
    pub fn com_orcamento_de_sessao(mut self, mib: usize) -> Self {
        self.sessao = crate::sessao::Sessao::com_orcamento(mib);
        self
    }

    /// Contadores da sessão semântica (reaproveitamentos, cargas,
    /// invalidações, fonte retida).
    pub fn estatisticas_da_sessao(&self) -> crate::sessao::EstatisticasSessao {
        self.sessao.estatisticas
    }

    /// A biblioteca de `uri` (definição, hover, ações), da sessão ou carregada.
    pub(crate) fn biblioteca(&mut self, documentos: &DocumentStore, uri: &str) -> Option<crate::sessao::Uso<'_>> {
        let sdk = self.sdk.as_ref()?;
        let arquivo = crate::projeto::arquivo_da_uri(uri)?;
        self.sessao.obter(crate::sessao::Escopo::Biblioteca(arquivo), documentos, || {
            crate::projeto::carregar_biblioteca(sdk, documentos, uri)
        })
    }

    /// O projeto inteiro de `uri` (referências, renomear), da sessão ou carregado.
    pub(crate) fn projeto(&mut self, documentos: &DocumentStore, uri: &str) -> Option<crate::sessao::Uso<'_>> {
        let sdk = self.sdk.as_ref()?;
        let raiz = crate::projeto::raiz_do_projeto(&crate::projeto::arquivo_da_uri(uri)?);
        self.sessao.obter(crate::sessao::Escopo::Projeto(raiz), documentos, || {
            crate::projeto::carregar_projeto(sdk, documentos, uri)
        })
    }

    /// Descobre o SDK pelos mesmos caminhos usados pelo compilador.
    pub fn descobrir() -> Self {
        let sdk = SdkLayout::discover().and_then(|p| SdkLayout::load(&p, "dartdevc").ok());
        Self::novo(sdk)
    }

    /// Índice dos nomes públicos do SDK, montado na primeira chamada.
    pub(crate) fn indice_sdk(&mut self) -> &crate::indice::IndiceSdk {
        let sdk = self.sdk.as_ref();
        self.indice_sdk.get_or_insert_with(|| sdk.map(crate::indice::indexar_sdk).unwrap_or_default())
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
        if let Some(projeto) = self.biblioteca(documentos, uri)
            && let Some(unidade) = projeto.unidade_do_uri(uri)
            && let Ok(Some(d)) = projeto.identificar(unidade, offset)
            && let Some((u, span)) = projeto.declaracao(&d)
        {
            return Some((projeto.uri_da_unidade(u)?, Some(span)));
        }
        self.sintatico.definicao_no_workspace(uri, texto, offset, documentos)
    }

    /// Hover pela identidade semântica; sem ela, o sintático conservador.
    fn passar_hover(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<Hover> {
        if let Some(projeto) = self.biblioteca(documentos, uri)
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
        let projeto = self.projeto(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let d = projeto.identificar(unidade, offset).ok()??;
        if let crate::projeto::Alvo::Prefixo { biblioteca, nome } = d.alvo {
            let (declaracao, usos) = projeto.referencias_de_prefixo(unidade, offset, biblioteca, nome)?;
            let mut usos: Vec<(String, Span)> = usos.into_iter().filter_map(|(u, s)| Some((projeto.uri_da_unidade(u)?, s))).collect();
            usos.sort_by(|a, b| (a.0.as_str(), a.1.start).cmp(&(b.0.as_str(), b.1.start)));
            let declaracao = declaracao.and_then(|(u, s)| Some((projeto.uri_da_unidade(u)?, s)));
            return Some(Referencias { declaracao, usos });
        }
        let declaracao = projeto.declaracao_para_referencias(&d);
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

/// Carrega o programa de `uri` com `texto` e os demais documentos abertos
/// nos textos vigentes (os outros arquivos vêm do disco). Uma parte entra
/// pela biblioteca dona.
pub(crate) fn carregar(sdk: &SdkLayout, uri: &str, texto: &str, documentos: Option<&DocumentStore>) -> Option<(Program, Interner, UnitId)> {
    let caminho = Url::parse(uri).ok()?.to_file_path().ok()?;
    if caminho.extension().is_none_or(|e| e != "dart") { return None; }
    let mut gerador = documentos.map_or_else(Construtor::nova, AnalisadorSemantico::abertos);
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
        self.sessao.invalidar();
    }

    fn documento_alterado(&mut self, _uri: &str) {
        self.sessao.invalidar();
    }

    fn sdk_para_diagnosticos(&self) -> Option<std::path::PathBuf> {
        self.sdk.as_ref().map(|s| s.root.clone())
    }

    fn preparar_renomeacao(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Result<Option<(Span, String)>, String> {
        let Some(projeto) = self.projeto(documentos, uri) else { return Ok(None) };
        crate::renomear::preparar(&projeto, uri, offset)
    }

    fn renomear(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, novo: &str) -> Result<crate::Renomeacao, String> {
        let projeto = self.projeto(documentos, uri)
            .ok_or_else(|| "O projeto do arquivo não pôde ser carregado (SDK ausente ou URI que não é de arquivo).".to_string())?;
        crate::renomear::renomear(&projeto, uri, offset, novo)
    }

    fn acoes(&mut self, documentos: &DocumentStore, uri: &str, inicio: usize, fim: usize, publicados: &[Diagnostic]) -> Vec<crate::AcaoDeCodigo> {
        let Some(texto) = documentos.get(uri) else { return Vec::new() };
        // Os imediatos da versão vigente e os tipados já publicados para ela.
        let mut diagnosticos = self.diagnosticar(uri, texto);
        for d in publicados {
            if !diagnosticos.iter().any(|x| x.code == d.code && x.span == d.span) {
                diagnosticos.push(d.clone());
            }
        }
        // As correções valem para os diagnósticos das linhas pedidas (o
        // servidor do Dart filtra por linha); as assistências, para o
        // cursor.
        let (linha_ini, linha_fim) = {
            let a = texto[..inicio.min(texto.len())].rfind('\n').map_or(0, |i| i + 1);
            let b = texto[fim.min(texto.len())..].find('\n').map_or(texto.len(), |i| fim + i);
            (a, b)
        };
        let mut saida = crate::acoes::corrigir_sintaxe(uri, texto, &diagnosticos, linha_ini, linha_fim);
        // "Fix all in file" das sintáticas.
        let todas = crate::acoes::corrigir_em_todo_o_arquivo(&saida, &diagnosticos, texto, |o, a, b| {
            crate::acoes::corrigir_sintaxe(uri, texto, std::slice::from_ref(o), a, b)
        });
        saida.extend(todas);
        saida.push(crate::acoes::organizar_imports(uri, texto));
        // As de ignorar vêm por último, como a prioridade do Dart.
        let ignorar = crate::ignorar::ignorar(uri, texto, &diagnosticos, linha_ini, linha_fim);
        if self.sdk.is_none() {
            saida.extend(ignorar);
            return saida;
        }
        self.indice_sdk();
        let Some(arquivo) = crate::projeto::arquivo_da_uri(uri) else {
            saida.extend(ignorar);
            return saida;
        };
        let AnalisadorSemantico { sessao, sdk: Some(sdk), indice_sdk: Some(_indice), conhecidas, .. } = self else {
            saida.extend(ignorar);
            return saida;
        };
        if let Some(mut projeto) = sessao.obter(crate::sessao::Escopo::Biblioteca(arquivo), documentos, || {
            crate::projeto::carregar_biblioteca(sdk, documentos, uri)
        }) {
            let publicadas = crate::acoes::corrigir_publicados(&projeto, uri, &diagnosticos, linha_ini, linha_fim);
            let todas = crate::acoes::corrigir_em_todo_o_arquivo(&publicadas, &diagnosticos, texto, |o, a, b| {
                crate::acoes::corrigir_publicados(&projeto, uri, std::slice::from_ref(o), a, b)
            });
            saida.extend(publicadas);
            saida.extend(todas);
            // Importar antes de criar: a prioridade das correções do Dart.
            saida.extend(crate::acoes::importar(&projeto, Some(&*sdk), conhecidas, documentos, uri, linha_ini, linha_fim));
            let corrigidas = crate::correcoes::corrigir(&mut projeto, uri, &diagnosticos, linha_ini, linha_fim);
            let todas = crate::acoes::corrigir_em_todo_o_arquivo(&corrigidas, &diagnosticos, texto, |o, a, b| {
                crate::correcoes::corrigir(&mut projeto, uri, std::slice::from_ref(o), a, b)
            });
            saida.extend(corrigidas);
            saida.extend(todas);
            saida.extend(crate::correcoes::argumentos_requeridos(&projeto, uri, linha_ini, linha_fim));
            if let Some(unidade) = projeto.unidade_do_uri(uri) {
                let criadas = projeto.criar_indefinidos(uri, unidade, linha_ini, linha_fim);
                saida.extend(criadas);
            }
            saida.extend(crate::acoes::assistencias(&projeto, uri, inicio, fim));
            if let Some(unidade) = projeto.unidade_do_uri(uri) {
                saida.extend(projeto.assistencias_de_reescrita(uri, unidade, inicio, fim));
                saida.extend(projeto.assistencias_sintaticas(uri, unidade, inicio));
                saida.extend(projeto.assistencias_de_condicao(uri, unidade, inicio));
                saida.extend(projeto.assistencias_de_juncao(uri, unidade, inicio));
            }
        }
        saida.extend(ignorar);
        saida
    }

    fn refatoracoes(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, comprimento: usize, criar_arquivos: bool) -> Vec<crate::Refatoracao> {
        let Some(projeto) = self.biblioteca(documentos, uri) else { return Vec::new() };
        let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
        projeto.refatoracoes(unidade, offset, comprimento, criar_arquivos)
    }

    fn executar_refatoracao(&mut self, documentos: &DocumentStore, uri: &str, pedido: &crate::PedidoDeRefatoracao) -> crate::ResultadoDeRefatoracao {
        let Some(projeto) = self.projeto(documentos, uri) else { return crate::ResultadoDeRefatoracao::NaoAnalisado };
        crate::refatoracoes_exec::executar(&projeto, uri, pedido)
    }

    fn mover_para_arquivo(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, comprimento: usize, destino: &str) -> crate::ResultadoDeRefatoracao {
        let Some(projeto) = self.projeto(documentos, uri) else { return crate::ResultadoDeRefatoracao::NaoAnalisado };
        crate::refatoracoes_exec::mover(&projeto, uri, offset, comprimento, destino)
    }

    fn dobras(&mut self, uri: &str, texto: &str, so_linhas: bool) -> Vec<crate::Dobra> {
        self.sintatico.dobras(uri, texto, so_linhas)
    }

    fn selecoes(&mut self, uri: &str, texto: &str, offset: usize) -> Vec<Span> {
        self.sintatico.selecoes(uri, texto, offset)
    }

    fn assinatura(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, automatica: bool) -> Option<crate::Assinatura> {
        let mut projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        projeto.assinatura(unidade, offset, automatica)
    }

    fn destaques(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<Vec<Span>> {
        if self.sdk.is_none() {
            let texto = documentos.get(uri)?.to_string();
            return self.sintatico.referencias(uri, &texto, offset);
        }
        let projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        projeto.destaques(unidade, offset)
    }

    fn implementacoes(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Vec<(String, Span)> {
        let Some(projeto) = self.projeto(documentos, uri) else { return Vec::new() };
        let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
        projeto
            .implementacoes(unidade, offset)
            .into_iter()
            .filter_map(|(u, s)| Some((projeto.uri_da_unidade(u)?, s)))
            .collect()
    }

    fn definicao_de_tipo(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<(String, Span)> {
        let projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let (u, s) = projeto.definicao_de_tipo(unidade, offset)?;
        Some((projeto.uri_da_unidade(u)?, s))
    }

    fn rotulos_de_fechamento(&mut self, documentos: &DocumentStore, uri: &str) -> Vec<(Span, String)> {
        let Some(projeto) = self.biblioteca(documentos, uri) else { return Vec::new() };
        let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
        projeto.rotulos_de_fechamento(unidade)
    }

    fn dicas(&mut self, documentos: &DocumentStore, uri: &str) -> Vec<crate::Dica> {
        let Some(projeto) = self.biblioteca(documentos, uri) else { return Vec::new() };
        let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
        projeto.dicas(unidade)
    }

    fn tokens_semanticos(&mut self, documentos: &DocumentStore, uri: &str, multilinha: bool, faixa: Option<(usize, usize)>) -> Option<Vec<u32>> {
        let projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let realces = projeto.realces(unidade);
        let texto = documentos.get(uri)?;
        let tabela = documentos.linhas(uri)?;
        Some(crate::realce::codificar(texto, tabela, &realces, multilinha, faixa))
    }

    fn preparar_chamadas(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<crate::ItemDeChamada> {
        let projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let e = projeto.alvo_de_chamada(unidade, offset)?;
        projeto.item_de_chamada(e)
    }

    fn chamadas(&mut self, documentos: &DocumentStore, uri: &str, offset: usize, nome: &str, construtor: bool, recebidas: bool) -> Vec<(crate::ItemDeChamada, Vec<Span>)> {
        let Some(projeto) = self.projeto(documentos, uri) else { return Vec::new() };
        let Some(unidade) = projeto.unidade_do_uri(uri) else { return Vec::new() };
        let Some((e, _)) = projeto.alvo_do_item(unidade, offset, nome, construtor) else { return Vec::new() };
        if recebidas {
            // Só executáveis recebem chamadas.
            if !matches!(e, crate::chamadas::ElemDeChamada::Funcao(_) | crate::chamadas::ElemDeChamada::Local(..)) {
                return Vec::new();
            }
            projeto.chamadas_recebidas(e)
        } else {
            projeto.chamadas_feitas(unidade, offset)
        }
    }

    fn preparar_hierarquia(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<crate::ItemDeTipo> {
        let mut projeto = self.biblioteca(documentos, uri)?;
        let unidade = projeto.unidade_do_uri(uri)?;
        let (c, tipo) = projeto.alvo_da_hierarquia(unidade, offset)?;
        projeto.item_de_tipo(c, tipo)
    }

    fn hierarquia(&mut self, documentos: &DocumentStore, uri: &str, referencia: &str, ancora: Option<(&str, &[usize])>, supertipos: bool) -> Option<Vec<crate::ItemDeTipo>> {
        let mut projeto = self.projeto(documentos, uri)?;
        if supertipos { projeto.supertipos_da_referencia(referencia, ancora) } else { projeto.subtipos_da_referencia(referencia) }
    }

    fn definir_maximo_de_completar(&mut self, maximo: usize) {
        self.maximo_de_completar = maximo;
    }

    fn completar(&mut self, documentos: &DocumentStore, uri: &str, offset: usize) -> Option<crate::Completar> {
        let texto = documentos.get(uri)?;
        let features = self.sintatico.features(uri, texto);
        self.sdk.as_ref()?;
        self.indice_sdk();
        let self_maximo = self.maximo_de_completar;
        let AnalisadorSemantico { sdk: Some(sdk), indice_sdk: Some(indice), indice_projeto, .. } = self else {
            return None;
        };
        let indices = crate::completar::Indices { sdk: indice, projeto: indice_projeto };
        crate::completar::completar(sdk, indices, documentos, uri, texto, offset, features, self_maximo)
    }
}
