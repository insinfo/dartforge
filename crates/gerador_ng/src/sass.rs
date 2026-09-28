//! O Sass do `sass_builder` 2.2.1 (dart-sass 1.102.0), pelo compilador do
//! crate `dartforge-sass`.
//!
//! `styleUrls: ['x.css']` aponta para um arquivo que não existe no disco: o
//! `sass_builder` o gera de `x.scss`. Sem isto, nenhum componente com folha
//! de estilo pode ser gerado sem o `build_runner`.
//!
//! O compilador é o `grass_compiler` 0.13.4 trazido para o repositório
//! (`crates/sass`) e corrigido para sair **byte a byte** igual ao dart-sass
//! 1.102.0 — qualquer SCSS, sem subconjunto. As folhas que `@use`,
//! `@forward` e `@import` carregam são resolvidas como o `BuildImporter` do
//! `sass_builder` as resolve: relativas ao arquivo e `package:` pelo
//! `.dart_tool/package_config.json` do projeto (ver
//! [`dartforge_sass::sass_builder`]).
use crate::visao::Motivo;
use dartforge_sass::OutputStyle;
use dartforge_sass::sass_builder::{self, Ativo, Disco, Pacotes};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Como a compilação lê os arquivos (o `BuildStep` do `build_runner`): o
/// motor de build passa o dele, que vê as saídas de builders anteriores.
pub use dartforge_sass::sass_builder::Leitor;

/// O estilo de saída do `sass_builder` (`outputStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estilo {
    Expandido,
    Comprimido,
}

impl Estilo {
    fn do_dart_sass(self) -> OutputStyle {
        match self {
            Estilo::Expandido => OutputStyle::Expanded,
            Estilo::Comprimido => OutputStyle::Compressed,
        }
    }
}

/// Compila Sass para CSS, sem resolver módulos (`@use`/`@import` falham).
///
/// # Erros
///
/// [`Motivo::Estilos`] quando o dart-sass recusaria a folha.
///
/// ```
/// use dartforge_gerador_ng::sass::compilar;
/// assert_eq!(compilar(".a { .b { top: 0 } }").unwrap(), ".a .b{top:0}\n");
/// ```
pub fn compilar(fonte: &str) -> Result<String, Motivo> {
    compilar_em(fonte, None)
}

/// Compila Sass resolvendo `@use`, `@forward` e `@import` a partir de `dir`,
/// para o caminho do shim do ngdart (a folha de um componente cujo `.css`
/// não existe e que o motor não trouxe do `sass_builder`).
///
/// O estilo é o `compressed`: o do new_sali, contra o qual este caminho foi
/// conferido quando o Sass daqui era um subconjunto. O shim tira os
/// comentários e espaços; o que importa são os valores como o dart-sass os
/// escreve nesse estilo.
///
/// # Erros
///
/// [`Motivo::Estilos`] quando o dart-sass recusaria a folha.
///
/// ```
/// use dartforge_gerador_ng::sass::compilar_em;
/// assert_eq!(compilar_em("$c: #fff;\na { color: $c; }", None).unwrap(), "a{color:#fff}\n");
/// ```
pub fn compilar_em(fonte: &str, dir: Option<&Path>) -> Result<String, Motivo> {
    compilar_com(fonte, dir, Estilo::Comprimido).map(|(css, _)| css)
}

/// Compila como o `sass_builder` 2.2.1 (o dart-sass do lock): o CSS **byte a
/// byte** do oficial no estilo pedido, terminado em `\n` — sem o comentário
/// `/*# sourceMappingURL=… */` do fim, que depende do nome do arquivo e de
/// haver mapa; quem escreve o `.css` o acrescenta —, e os arquivos que a
/// compilação leu (os módulos de `@use`/`@import`, que também são entradas).
///
/// A folha é tratada como um arquivo de `dir` (o pacote é o do
/// `pubspec.yaml` mais próximo; sem um, `dir` é a raiz de um pacote
/// anônimo). Quem sabe o arquivo de entrada deve usar [`compilar_arquivo`],
/// que também gera o mapa.
///
/// # Erros
///
/// [`Motivo::Estilos`] quando o dart-sass recusaria a folha.
///
/// ```
/// use dartforge_gerador_ng::sass::{compilar_com, Estilo};
/// let (css, lidos) = compilar_com(".a { color: red; }", None, Estilo::Expandido).unwrap();
/// assert_eq!(css, ".a {\n  color: red;\n}\n");
/// assert!(lidos.is_empty());
/// ```
pub fn compilar_com(
    fonte: &str,
    dir: Option<&Path>,
    estilo: Estilo,
) -> Result<(String, Vec<PathBuf>), Motivo> {
    // Sem diretório, um pacote anônimo sem raiz: nada se carrega.
    let (entrada, pacotes) = match dir {
        Some(d) => identificar(&d.join("__entrada__.scss")),
        None => (
            Ativo {
                pacote: "anonimo".into(),
                caminho: "__entrada__.scss".into(),
            },
            Pacotes::new(),
        ),
    };
    let c = sass_builder::compilar(
        fonte,
        &entrada,
        &pacotes,
        &Disco,
        estilo.do_dart_sass(),
        false,
    )
    .map_err(|_| Motivo::Estilos)?;
    Ok((format!("{}\n", c.css), c.lidos))
}

/// O resultado de [`compilar_arquivo`].
#[derive(Debug, Clone)]
pub struct Folha {
    /// O `.css` exatamente como o `sass_builder` o grava (com o comentário
    /// `sourceMappingURL` quando há mapa).
    pub css: String,
    /// O `.css.map`, quando pedido.
    pub mapa: Option<String>,
    /// Os arquivos lidos (a entrada não entra) e os procurados sem existir:
    /// dependências da compilação.
    pub lidos: Vec<PathBuf>,
    pub sondados: Vec<PathBuf>,
}

/// Compila o arquivo `scss` como o `sass_builder` (entrada, pacote e
/// `package_config.json` achados a partir do caminho) e, se `mapa`, gera o
/// `.css.map` dele.
///
/// # Erros
///
/// [`Motivo::Estilos`] quando o arquivo não se lê ou o dart-sass recusaria
/// a folha.
pub fn compilar_arquivo(scss: &Path, estilo: Estilo, mapa: bool) -> Result<Folha, Motivo> {
    let fonte = std::fs::read_to_string(scss).map_err(|_| Motivo::Estilos)?;
    let (entrada, pacotes) = identificar(scss);
    let c = sass_builder::compilar(
        &fonte,
        &entrada,
        &pacotes,
        &Disco,
        estilo.do_dart_sass(),
        mapa,
    )
    .map_err(|_| Motivo::Estilos)?;
    Ok(Folha {
        css: c.arquivo_css(&entrada),
        mapa: c.mapa,
        lidos: c.lidos,
        sondados: c.sondados,
    })
}

/// Compila o ativo `caminho` (relativo à raiz do pacote, com `/`) do pacote
/// `pacote`, de texto `fonte`, como o `SassBuilder` do `sass_builder` 2.2.1:
/// `raizes` são as raízes dos pacotes do `package_config.json` e `leitor`,
/// o `BuildStep`. Com `mapa`, gera o `.css.map` e põe o comentário
/// `sourceMappingURL` no `.css`.
///
/// # Erros
///
/// A mensagem do dart-sass quando a folha é recusada.
///
/// ```
/// use dartforge_gerador_ng::sass::{compilar_ativo, Estilo, Leitor};
/// use std::collections::BTreeMap;
/// use std::path::Path;
/// struct Nada;
/// impl Leitor for Nada {
///     fn existe(&self, _: &Path) -> bool { false }
///     fn ler(&self, _: &Path) -> std::io::Result<String> { Err(std::io::ErrorKind::NotFound.into()) }
/// }
/// let f = compilar_ativo("a { b: c }", "app", "web/x.scss", &BTreeMap::new(), &Nada, Estilo::Comprimido, true).unwrap();
/// assert_eq!(f.css, "a{b:c}\n\n/*# sourceMappingURL=x.css.map */\n");
/// assert_eq!(f.mapa.unwrap(), r#"{"version":3,"sourceRoot":"","sources":["x.scss"],"names":[],"mappings":"AAAA"}"#);
/// ```
pub fn compilar_ativo(
    fonte: &str,
    pacote: &str,
    caminho: &str,
    raizes: &BTreeMap<String, PathBuf>,
    leitor: &dyn Leitor,
    estilo: Estilo,
    mapa: bool,
) -> Result<Folha, String> {
    let mut pacotes = Pacotes::new();
    for (nome, raiz) in raizes {
        pacotes.inserir(nome.clone(), raiz.clone());
    }
    let entrada = Ativo {
        pacote: pacote.to_owned(),
        caminho: caminho.to_owned(),
    };
    let c = sass_builder::compilar(
        fonte,
        &entrada,
        &pacotes,
        leitor,
        estilo.do_dart_sass(),
        mapa,
    )
    .map_err(|e| e.to_string())?;
    Ok(Folha {
        css: c.arquivo_css(&entrada),
        mapa: c.mapa,
        lidos: c.lidos,
        sondados: c.sondados,
    })
}

/// O ativo de `arquivo` e os pacotes: o pacote é o do `pubspec.yaml` mais
/// próximo (sem um, o diretório do arquivo é a raiz de um pacote anônimo);
/// os outros vêm do `.dart_tool/package_config.json` do pacote ou de um
/// diretório acima (espaço de trabalho).
fn identificar(arquivo: &Path) -> (Ativo, Pacotes) {
    let dir = arquivo.parent().unwrap_or(Path::new(""));
    let raiz = dir.ancestors().find(|d| d.join("pubspec.yaml").is_file());
    let (raiz, nome) = match raiz {
        Some(r) => (
            r.to_path_buf(),
            nome_do_pubspec(&r.join("pubspec.yaml")).unwrap_or_default(),
        ),
        None => (dir.to_path_buf(), "anonimo".to_owned()),
    };
    let mut pacotes = raiz
        .ancestors()
        .map(|d| d.join(".dart_tool/package_config.json"))
        .find(|c| c.is_file())
        .and_then(|c| Pacotes::de_package_config(&c).ok())
        .unwrap_or_default();
    pacotes.inserir(nome.clone(), raiz.clone());
    let caminho = arquivo
        .strip_prefix(&raiz)
        .unwrap_or(arquivo)
        .to_string_lossy()
        .replace('\\', "/");
    (
        Ativo {
            pacote: nome,
            caminho,
        },
        pacotes,
    )
}

/// O `name:` de um `pubspec.yaml`.
fn nome_do_pubspec(pubspec: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(pubspec).ok()?;
    texto.lines().find_map(|l| {
        let resto = l.strip_prefix("name:")?;
        let nome = resto.split('#').next()?.trim().trim_matches(['"', '\'']);
        (!nome.is_empty()).then(|| nome.to_owned())
    })
}
