//! Generation of **one** file, for the build engine (`crates/build`,
//! `docs/BUILD-PEDIDOS-GERADOR-NG.md`): the engine has one action per `.dart`
//! and reruns only that of the edited component. Besides the text, the
//! generation returns what it asked other libraries ([`ConsultaNg`]) — it is
//! through these queries that the engine knows whom to wake up when a child's
//! `@Input` changes or a selector appears.
//!
//! The output is the same as [`crate::gerar_em`]: both go through
//! `gerar_interno`.
use crate::resolucao::{Designado, Resolucao};
use crate::view::Recusa;
use crate::{Achados, Indice, Pacote};
use dartforge_intern::Interner;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// What generating one file produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaidaArquivo {
    /// Texto do `<nome>.template.dart`.
    pub template: String,
    /// Files read: the `.dart`, the `.html` of `templateUrl`, each `styleUrls`
    /// stylesheet (the `.css`, or the `.scss` it comes from).
    pub entradas: Vec<PathBuf>,
    /// Extra outputs (`<name>.css.shim.dart`), by natural path.
    pub extras: Vec<(PathBuf, String)>,
    /// What the generation asked other libraries, without repetition, in the
    /// order it asked.
    pub consultas: Vec<ConsultaNg>,
}

/// A question the generation asked outside its own file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConsultaNg {
    /// A component or directive of `directives:` resolved: the selector and who
    /// declares it.
    Filho {
        seletor: String,
        biblioteca: String,
        classe: String,
    },
    /// A type resolved by the semantic database (injection, member type for
    /// `interpolate*`, item of `directives:`/`pipes:`): the library that
    /// declares it and the name.
    Tipo { biblioteca: String, nome: String },
    /// A template tag that is not HTML and that no component of `directives:`
    /// declares: if one appears, the generation changes.
    SeletorAusente(String),
}

/// Reads and analyzes a `.dart`: the findings the generation and the index use.
pub fn analisar_arquivo(fonte: &Path, texto: &str, nomes: &mut Interner) -> Achados {
    let _ = fonte;
    let tokens = dartforge_frontend::lexer::lex(texto);
    let analisada = dartforge_frontend::parser::parse_lexed(texto, tokens, nomes);
    crate::achar(&analisada.ast, &analisada.unit, texto, nomes)
}

/// Generates the `.template.dart` of `fonte` — the same text
/// [`crate::gerar_em`] would write for it —, or says why not. The refusal
/// carries the [`crate::view::Motivo`] (`recusa.motivo`) and the sub-shape.
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

/// The children each component of the file resolved, and the tags that found no
/// owner.
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

/// A [`Resolucao`] that answers for the inner one and records each type found.
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

    fn tipo_do_membro(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
    ) -> Option<(String, PathBuf)> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.tipo_do_membro(arquivo, tipo, membro)
    }

    fn metodo(&self, arquivo: &Path, tipo: &str, nome: &str) -> Option<crate::resolucao::Metodo> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.metodo(arquivo, tipo, nome)
    }

    fn limites_de_tipo(&self, arquivo: &Path, tipo: &str) -> Option<Vec<bool>> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.limites_de_tipo(arquivo, tipo)
    }

    fn tem_setter(&self, arquivo: &Path, tipo: &str, membro: &str) -> bool {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.tem_setter(arquivo, tipo, membro)
    }

    fn tipo_do_membro_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
        livres: &[String],
    ) -> Option<(String, PathBuf)> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro
            .tipo_do_membro_livre(arquivo, tipo, membro, livres)
    }

    fn metodo_livre(
        &self,
        arquivo: &Path,
        tipo: &str,
        nome: &str,
        livres: &[String],
    ) -> Option<crate::resolucao::Metodo> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.metodo_livre(arquivo, tipo, nome, livres)
    }

    fn membro_final(&self, arquivo: &Path, tipo: &str, membro: &str) -> Option<bool> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.membro_final(arquivo, tipo, membro)
    }

    fn designado(&self, arquivo: &Path, nome: &str) -> Option<Designado> {
        let r = self.dentro.designado(arquivo, nome);
        if let Some(Designado::Classe { uri }) = &r {
            self.anotar(uri, nome);
        }
        r
    }

    // The type missing from the scope depends on the libraries the file
    // imports, which the import closure of the invalidation (`NgEstagioA`)
    // already covers; there is no declaring library to record.
    fn tipo_inexistente(&self, arquivo: &Path, tipo: &str) -> bool {
        self.dentro.tipo_inexistente(arquivo, tipo)
    }

    fn exportado(
        &self,
        arquivo: &Path,
        nome: &str,
    ) -> Option<(String, crate::resolucao::Exportado)> {
        let r = self.dentro.exportado(arquivo, nome);
        if let Some((uri, _)) = &r {
            self.anotar(uri, nome);
        }
        r
    }

    fn membro_estatico(
        &self,
        arquivo: &Path,
        tipo: &str,
        membro: &str,
    ) -> Option<crate::resolucao::Estatico> {
        if let Some(uri) = self.dentro.uri_do_tipo(arquivo, tipo) {
            self.anotar(&uri, tipo);
        }
        self.dentro.membro_estatico(arquivo, tipo, membro)
    }

    fn tipo_da_saida(&self, uri: &str, classe: &str, membro: &str) -> Option<(String, PathBuf)> {
        self.anotar(uri, classe);
        self.dentro.tipo_da_saida(uri, classe, membro)
    }
}
