//! A geração de **um** arquivo, para o motor de build (`crates/build`,
//! `docs/BUILD-PEDIDOS-GERADOR-NG.md`): o motor tem uma ação por `.dart` e
//! reexecuta só a do componente editado. Além do texto, a geração devolve o
//! que perguntou a outras bibliotecas ([`ConsultaNg`]) — é por essas
//! consultas que o motor sabe quem acordar quando um `@Input` de um filho
//! muda ou um seletor aparece.
//!
//! A saída é a mesma de [`crate::gerar_em`]: as duas passam por
//! `gerar_interno`.
use crate::resolucao::{Designado, Resolucao};
use crate::visao::Recusa;
use crate::{Achados, Indice, Pacote};
use dartforge_intern::Interner;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// O que a geração de um arquivo produz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaidaArquivo {
    /// Texto do `<nome>.template.dart`.
    pub template: String,
    /// Arquivos lidos: o `.dart`, o `.html` do `templateUrl`, cada folha do
    /// `styleUrls` (o `.css`, ou o `.scss` de onde ele sai).
    pub entradas: Vec<PathBuf>,
    /// Saídas extras (`<nome>.css.shim.dart`), por caminho natural.
    pub extras: Vec<(PathBuf, String)>,
    /// O que a geração perguntou a outras bibliotecas, sem repetição, na
    /// ordem em que perguntou.
    pub consultas: Vec<ConsultaNg>,
}

/// Uma pergunta que a geração fez fora do próprio arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConsultaNg {
    /// Um componente ou diretiva de `directives:` resolvido: o seletor e
    /// quem o declara.
    Filho {
        seletor: String,
        biblioteca: String,
        classe: String,
    },
    /// Um tipo resolvido pelo banco semântico (injeção, tipo de membro para
    /// o `interpolate*`, item de `directives:`/`pipes:`): a biblioteca que o
    /// declara e o nome.
    Tipo { biblioteca: String, nome: String },
    /// Uma tag do template que não é HTML e que nenhum componente de
    /// `directives:` declara: se um aparecer, a geração muda.
    SeletorAusente(String),
}

/// Lê e analisa um `.dart`: os achados que a geração e o índice usam.
pub fn analisar_arquivo(fonte: &Path, texto: &str, nomes: &mut Interner) -> Achados {
    let _ = fonte;
    let tokens = dartforge_frontend::lexer::lex(texto);
    let analisada = dartforge_frontend::parser::parse_lexed(texto, tokens, nomes);
    crate::achar(&analisada.ast, &analisada.unit, texto, nomes)
}

/// Gera o `.template.dart` de `fonte` — o mesmo texto que
/// [`crate::gerar_em`] escreveria para ele —, ou diz por que não. A recusa
/// traz o [`crate::visao::Motivo`] (`recusa.motivo`) e a sub-forma.
pub fn gerar_arquivo(
    pacote: &Pacote,
    fonte: &Path,
    achados: &Achados,
    resolvedor: Option<&dyn Resolucao>,
    nomes: &mut Interner,
    indice: &Indice,
) -> Result<SaidaArquivo, Recusa> {
    let nome = fonte
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let gravador = resolvedor.map(Gravador::novo);
    let gerado = crate::gerar_interno(
        pacote,
        fonte,
        &nome,
        achados,
        gravador.as_ref().map(|g| g as &dyn Resolucao),
        nomes,
        indice,
    );
    let mut consultas: Vec<ConsultaNg> = gravador
        .map(|g| g.consultas.into_inner())
        .unwrap_or_default();
    consultas.extend(filhos_consultados(fonte, achados, resolvedor, indice));
    let (template, entradas, extras) = gerado?;
    let mut unicas = Vec::new();
    for c in consultas {
        if !unicas.contains(&c) {
            unicas.push(c);
        }
    }
    Ok(SaidaArquivo {
        template,
        entradas,
        extras,
        consultas: unicas,
    })
}

/// Os filhos que cada componente do arquivo resolveu, e as tags que não
/// acharam dono.
fn filhos_consultados(
    fonte: &Path,
    achados: &Achados,
    resolvedor: Option<&dyn Resolucao>,
    indice: &Indice,
) -> Vec<ConsultaNg> {
    let mut saida = Vec::new();
    for comp in &achados.componentes {
        let (usadas, _) = indice.diretivas_de(comp, fonte, resolvedor);
        for u in &usadas {
            let seletor = match (&u.filho, &u.diretiva) {
                (Some(f), _) => f.seletor.clone(),
                (None, Some(d)) => d.seletor.clone(),
                (None, None) => indice
                    .diretivas
                    .get(&(u.uri.clone(), u.classe.clone()))
                    .map(|d| d.seletor.clone())
                    .unwrap_or_default(),
            };
            saida.push(ConsultaNg::Filho {
                seletor,
                biblioteca: u.uri.clone(),
                classe: u.classe.clone(),
            });
        }
        let filhos = crate::filhos_por_tag(&usadas);
        let template = match (&comp.template, &comp.template_url) {
            (Some(t), _) => t.clone(),
            (None, Some(u)) => fonte
                .parent()
                .and_then(|d| std::fs::read_to_string(d.join(u)).ok())
                .unwrap_or_default(),
            (None, None) => String::new(),
        };
        let mut tags = Vec::new();
        tags_de(&crate::html::analisar(&template), &mut tags);
        for t in tags {
            if !crate::dom::tag_html(&t) && !filhos.contains_key(&t) && t != "ng-container" {
                saida.push(ConsultaNg::SeletorAusente(t));
            }
        }
    }
    saida
}

fn tags_de(nos: &[crate::html::No], saida: &mut Vec<String>) {
    for n in nos {
        if let crate::html::No::Elemento(e) = n {
            if !saida.contains(&e.nome) {
                saida.push(e.nome.clone());
            }
            tags_de(&e.filhos, saida);
        }
    }
}

/// Um [`Resolucao`] que responde pelo de dentro e anota cada tipo achado.
struct Gravador<'a> {
    dentro: &'a dyn Resolucao,
    consultas: RefCell<Vec<ConsultaNg>>,
}

impl<'a> Gravador<'a> {
    fn novo(dentro: &'a dyn Resolucao) -> Self {
        Gravador {
            dentro,
            consultas: RefCell::new(Vec::new()),
        }
    }

    fn anotar(&self, biblioteca: &str, nome: &str) {
        let simples = nome.trim_end_matches('?');
        let simples = simples.rsplit('.').next().unwrap_or(simples);
        let c = ConsultaNg::Tipo {
            biblioteca: biblioteca.to_string(),
            nome: simples.to_string(),
        };
        let mut v = self.consultas.borrow_mut();
        if !v.contains(&c) {
            v.push(c);
        }
    }
}

impl Resolucao for Gravador<'_> {
    fn uri_do_tipo(&self, arquivo: &Path, nome: &str) -> Option<String> {
        let r = self.dentro.uri_do_tipo(arquivo, nome);
        if let Some(uri) = &r {
            self.anotar(uri, nome);
        }
        r
    }

    fn tipo_do_membro(&self, arquivo: &Path, tipo: &str, membro: &str) -> Option<(String, PathBuf)> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.tipo_do_membro(arquivo, tipo, membro)
    }

    fn designado(&self, arquivo: &Path, nome: &str) -> Option<Designado> {
        let r = self.dentro.designado(arquivo, nome);
        if let Some(Designado::Classe { uri }) = &r {
            self.anotar(uri, nome);
        }
        r
    }
}
