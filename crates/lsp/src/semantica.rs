//! Consultas semânticas transitórias. A árvore, o programa e a tabela de tipos
//! pertencem a uma única requisição e caem antes da próxima versão do texto.

use crate::{Analisador, AnalisadorSintatico, navegacao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{gerado::Construtor, load::load_lenient_gerados, model::{Element, Program, UnitId, VariableId, VariableRef}, sdk::SdkLayout};
use dartforge_intern::Interner;
use dartforge_types::{CoreTypes, TypeTable, resolve_outline};
use url::Url;

/// Analisador sintático com resolução de variáveis de topo importadas.
/// O SDK é configuração pequena; nenhum `Program`, AST ou `TypeTable` fica
/// retido entre chamadas. Na falta do SDK, mantém as respostas sintáticas.
pub struct AnalisadorSemantico {
    sintatico: AnalisadorSintatico,
    sdk: Option<SdkLayout>,
}

impl AnalisadorSemantico {
    pub fn novo(sdk: Option<SdkLayout>) -> Self {
        Self { sintatico: AnalisadorSintatico::new(), sdk }
    }

    /// Descobre o SDK pelos mesmos caminhos usados pelo compilador.
    pub fn descobrir() -> Self {
        let sdk = SdkLayout::discover().and_then(|p| SdkLayout::load(&p, "dartdevc").ok());
        Self::novo(sdk)
    }

    fn carregar(&self, uri: &str, texto: &str) -> Option<(Program, Interner, UnitId)> {
        let sdk = self.sdk.as_ref()?;
        let caminho = Url::parse(uri).ok()?.to_file_path().ok()?;
        if caminho.extension().is_none_or(|e| e != "dart") { return None; }
        let mut gerador = Construtor::nova();
        gerador.por(caminho.clone(), texto.to_owned(), "lsp", vec![]);
        let geracao = gerador.concluir(1).ok()?;
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient_gerados(&caminho, sdk, None, &mut nomes, None, None, Some(geracao));
        let chave = dartforge_elements::gerado::chave(&caminho);
        let unidade = programa.units.iter().position(|u| u.path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&chave))?;
        Some((programa, nomes, UnitId(unidade as u32)))
    }

    fn variavel_importada(programa: &Program, unidade: UnitId, offset: usize) -> Option<(Span, VariableId)> {
        let u = programa.unit(unidade);
        if !u.ast.patterns.is_empty() { return None; }
        let referencia = navegacao::referencia_expr(&u.ast, offset)?;
        if navegacao::sombreado(&u.ast, referencia.sym) { return None; }
        let binding = programa.lookup(u.library, referencia.sym)?;
        if binding.ambiguous { return None; }
        let Element::Variable(id) = binding.getter? else { return None };
        if programa.variable(id).library == u.library { return None; }
        Some((referencia.span, id))
    }

    fn destino_variavel(programa: &Program, variavel: &dartforge_elements::model::VariableElement) -> Option<(String, Span)> {
        let VariableRef::TopLevel { unit, decl, index } = variavel.node else { return None };
        let u = programa.unit(unit);
        let dartforge_frontend::ast::DeclKind::Variables(lista) = &u.ast.decl(decl).kind else { return None };
        let nome = lista.variables.get(index)?.name.span;
        let uri = Url::from_file_path(u.path.as_ref()?).ok()?.to_string();
        Some((uri, nome))
    }
}

impl Analisador for AnalisadorSemantico {
    fn diagnosticar(&mut self, uri: &str, texto: &str) -> Vec<Diagnostic> {
        self.sintatico.diagnosticar(uri, texto)
    }

    fn simbolos(&mut self, uri: &str, texto: &str) -> Vec<serde_json::Value> {
        self.sintatico.simbolos(uri, texto)
    }

    fn definicao(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(String, Option<Span>)> {
        if let Some(alvo) = self.sintatico.definicao(uri, texto, offset) { return Some(alvo) }
        let (programa, _, unidade) = self.carregar(uri, texto)?;
        let (_, id) = Self::variavel_importada(&programa, unidade, offset)?;
        let (uri, span) = Self::destino_variavel(&programa, programa.variable(id))?;
        Some((uri, Some(span)))
    }

    fn hover(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(Span, String, Option<String>)> {
        if let Some(descricao) = self.sintatico.hover(uri, texto, offset) { return Some(descricao) }
        let (programa, nomes, unidade) = self.carregar(uri, texto)?;
        let (referencia, id) = Self::variavel_importada(&programa, unidade, offset)?;
        let mut tabela = TypeTable::new();
        let core = CoreTypes::init(&mut tabela, &programa, &nomes);
        let (outline, _) = resolve_outline(&programa, &nomes, &mut tabela, &core);
        let tipo = outline.variables[id.0 as usize].declared_type?;
        let texto_tipo = tabela.format(tipo, &nomes, &programa);
        if texto_tipo == "dynamic" { return None; }
        let nome = nomes.resolve(programa.variable(id).name);
        Some((referencia, format!("{texto_tipo} {nome}"), Some(texto_tipo)))
    }

    fn documento_fechado(&mut self, uri: &str) {
        self.sintatico.documento_fechado(uri);
    }
}
