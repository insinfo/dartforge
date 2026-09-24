//! Consultas semânticas transitórias. A árvore, o programa e a tabela de tipos
//! pertencem a uma única requisição e caem antes da próxima versão do texto.

use crate::{Analisador, AnalisadorSintatico, DocumentStore, navegacao};
use dartforge_diagnostics::{Diagnostic, Span};
use dartforge_elements::{gerado::Construtor, load::load_lenient_gerados, model::{Element, FunctionKind, FunctionRef, Program, UnitId, VariableRef}, sdk::SdkLayout};
use dartforge_frontend::ast::{ParameterKind, TypeKind};
use dartforge_intern::Interner;
use dartforge_types::{CoreTypes, TypeTable, resolve_outline};
use url::Url;

/// Analisador sintático com resolução de variáveis e funções de topo importadas.
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

    fn carregar(&self, uri: &str, texto: &str, documentos: Option<&DocumentStore>) -> Option<(Program, Interner, UnitId)> {
        let sdk = self.sdk.as_ref()?;
        let caminho = Url::parse(uri).ok()?.to_file_path().ok()?;
        if caminho.extension().is_none_or(|e| e != "dart") { return None; }
        let mut gerador = Construtor::nova();
        if let Some(documentos) = documentos {
            for aberto in documentos.uris() {
                let Some(fonte) = documentos.get(aberto) else { continue };
                let Some(arquivo) = Url::parse(aberto).ok().and_then(|u| u.to_file_path().ok()) else { continue };
                if arquivo.extension().is_some_and(|e| e == "dart") {
                    gerador.por(arquivo, fonte.to_owned(), "lsp", vec![]);
                }
            }
        }
        // A chamada direta da trait também usa o texto recebido, mesmo sem
        // DocumentStore. O arquivo da requisição prevalece sobre a coleção.
        gerador.por(caminho.clone(), texto.to_owned(), "lsp", vec![]);
        let geracao = gerador.concluir(1).ok()?;
        let mut nomes = Interner::new();
        let (programa, _) = load_lenient_gerados(&caminho, sdk, None, &mut nomes, None, None, Some(geracao));
        let chave = dartforge_elements::gerado::chave(&caminho);
        let unidade = programa.units.iter().position(|u| u.path.as_deref().map(dartforge_elements::gerado::chave).as_ref() == Some(&chave))?;
        Some((programa, nomes, UnitId(unidade as u32)))
    }

    fn elemento_importado(programa: &Program, unidade: UnitId, offset: usize) -> Option<(Span, Element)> {
        let u = programa.unit(unidade);
        if !u.ast.patterns.is_empty() { return None; }
        let referencia = navegacao::referencia_expr(&u.ast, offset)?;
        if navegacao::sombreado(&u.ast, referencia.sym) { return None; }
        let binding = programa.lookup(u.library, referencia.sym)?;
        if binding.ambiguous { return None; }
        let elemento = binding.getter?;
        let biblioteca = match elemento {
            Element::Variable(id) => programa.variable(id).library,
            Element::Function(id) if matches!(programa.function(id).kind, FunctionKind::Function | FunctionKind::Getter) => programa.function(id).library,
            _ => return None,
        };
        if biblioteca == u.library { return None; }
        Some((referencia.span, elemento))
    }

    fn destino_variavel(programa: &Program, variavel: &dartforge_elements::model::VariableElement) -> Option<(String, Span)> {
        let VariableRef::TopLevel { unit, decl, index } = variavel.node else { return None };
        let u = programa.unit(unit);
        let dartforge_frontend::ast::DeclKind::Variables(lista) = &u.ast.decl(decl).kind else { return None };
        let nome = lista.variables.get(index)?.name.span;
        let uri = Url::from_file_path(u.path.as_ref()?).ok()?.to_string();
        Some((uri, nome))
    }

    fn destino_funcao(programa: &Program, funcao: &dartforge_elements::model::FunctionElement) -> Option<(String, Span)> {
        let FunctionRef::Function { unit, function } = funcao.node else { return None };
        let u = programa.unit(unit);
        let span = u.ast.function(function).name?.span;
        let uri = Url::from_file_path(u.path.as_ref()?).ok()?.to_string();
        Some((uri, span))
    }

    fn definir(&mut self, uri: &str, texto: &str, offset: usize, documentos: Option<&DocumentStore>) -> Option<(String, Option<Span>)> {
        if let Some(alvo) = self.sintatico.definicao(uri, texto, offset) { return Some(alvo) }
        let (programa, _, unidade) = self.carregar(uri, texto, documentos)?;
        let (_, elemento) = Self::elemento_importado(&programa, unidade, offset)?;
        let (uri, span) = match elemento {
            Element::Variable(id) => Self::destino_variavel(&programa, programa.variable(id))?,
            Element::Function(id) => Self::destino_funcao(&programa, programa.function(id))?,
            _ => return None,
        };
        Some((uri, Some(span)))
    }

    fn passar_hover(&mut self, uri: &str, texto: &str, offset: usize, documentos: Option<&DocumentStore>) -> Option<(Span, String, Option<String>)> {
        if let Some(descricao) = self.sintatico.hover(uri, texto, offset) { return Some(descricao) }
        let (programa, nomes, unidade) = self.carregar(uri, texto, documentos)?;
        let (referencia, elemento) = Self::elemento_importado(&programa, unidade, offset)?;
        let mut tabela = TypeTable::new();
        let core = CoreTypes::init(&mut tabela, &programa, &nomes);
        let (outline, _) = resolve_outline(&programa, &nomes, &mut tabela, &core);
        match elemento {
            Element::Variable(id) => {
                let tipo = outline.variables[id.0 as usize].declared_type?;
                let texto_tipo = tabela.format(tipo, &nomes, &programa);
                if texto_tipo == "dynamic" { return None; }
                let nome = nomes.resolve(programa.variable(id).name);
                Some((referencia, format!("{texto_tipo} {nome}"), Some(texto_tipo)))
            }
            Element::Function(id) => {
                let FunctionRef::Function { unit, function } = programa.function(id).node else { return None };
                let ast = &programa.unit(unit).ast;
                let declaracao = ast.function(function);
                if !declaracao.type_params.is_empty() { return None; }
                let retorno_escrito = declaracao.return_type?;
                if !matches!(&ast.ty(retorno_escrito).kind, TypeKind::Void)
                    && navegacao::tipo_primitivo(ast, &nomes, retorno_escrito).is_none()
                { return None; }
                let dados = &outline.functions[id.0 as usize];
                let retorno = tabela.format(dados.return_type, &nomes, &programa);
                let nome = nomes.resolve(programa.function(id).name);
                if programa.function(id).kind == FunctionKind::Getter {
                    if declaracao.parameters.is_some() { return None; }
                    return Some((referencia, format!("{retorno} get {nome}"), Some(retorno)));
                }
                let parametros_escritos = declaracao.parameters.as_ref()?;
                if parametros_escritos.len() > 2 { return None; }
                for p in parametros_escritos.iter() {
                    if p.kind != ParameterKind::Required || p.covariant || p.final_ || p.var_
                        || p.const_ || p.this_ || p.super_ || p.default_value.is_some()
                        || !p.function_type_params.is_empty() || p.function_parameters.is_some()
                        || navegacao::tipo_primitivo(ast, &nomes, p.ty?).is_none()
                    { return None; }
                }
                if dados.parameters.len() != parametros_escritos.len() { return None; }
                let mut params = Vec::new();
                for p in dados.parameters.iter() {
                    if p.kind != ParameterKind::Required { return None; }
                    let nome = nomes.resolve(p.name?);
                    params.push(format!("{} {nome}", tabela.format(p.ty, &nomes, &programa)));
                }
                Some((referencia, format!("{retorno} {nome}({})", params.join(", ")), None))
            }
            _ => None,
        }
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
        self.definir(uri, texto, offset, None)
    }

    fn definicao_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<(String, Option<Span>)> {
        self.definir(uri, texto, offset, Some(documentos))
    }

    fn hover(&mut self, uri: &str, texto: &str, offset: usize) -> Option<(Span, String, Option<String>)> {
        self.passar_hover(uri, texto, offset, None)
    }

    fn hover_no_workspace(&mut self, uri: &str, texto: &str, offset: usize, documentos: &DocumentStore) -> Option<(Span, String, Option<String>)> {
        self.passar_hover(uri, texto, offset, Some(documentos))
    }

    fn documento_fechado(&mut self, uri: &str) {
        self.sintatico.documento_fechado(uri);
    }
}
