//! The Sass of `sass_builder` 2.2.1 (dart-sass 1.102.0), through the compiler
//! of the `dartforge-sass` crate.
//!
//! `styleUrls: ['x.css']` points to a file that does not exist on disk:
//! `sass_builder` generates it from `x.scss`. Without this, no component with a
//! stylesheet could be generated without `build_runner`.
//!
//! The compiler is `grass_compiler` 0.13.4 brought into the repository
//! (`crates/sass`) and fixed to produce output **byte for byte** equal to
//! dart-sass 1.102.0 — any SCSS, no subset. The stylesheets loaded by `@use`,
//! `@forward` and `@import` are resolved as the `sass_builder` `BuildImporter`
//! resolves them: relative to the file, and `package:` through the project's
//! `.dart_tool/package_config.json` (see [`dartforge_sass::sass_builder`]).
use crate::view::Motivo;
use dartforge_sass::OutputStyle;
use dartforge_sass::sass_builder::{self, Ativo, Disco, Pacotes};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How the compilation reads files (the `build_runner` `BuildStep`): the build
/// engine passes its own, which sees the outputs of earlier builders.
pub use dartforge_sass::sass_builder::Leitor;

/// The imitated dart-sass version (that of `sass` in `pubspec.lock`).
pub use dartforge_sass::VersaoDartSass;

/// The output style of `sass_builder` (`outputStyle`).
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

/// Compiles Sass to CSS, without resolving modules (`@use`/`@import` fail).
///
/// # Errors
///
/// [`Motivo::Estilos`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::compilar;
/// assert_eq!(compilar(".a { .b { top: 0 } }").unwrap(), ".a .b{top:0}\n");
/// ```
pub fn compilar(fonte: &str) -> Result<String, Motivo> {
    compilar_em(fonte, None)
}

/// Compiles Sass resolving `@use`, `@forward` and `@import` from `dir`, for the
/// ngdart shim path (the stylesheet of a component whose `.css` does not exist
/// and that the engine did not bring from `sass_builder`).
///
/// The style is `compressed`: the one of new_sali, against which this path was
/// checked when the Sass here was a subset. The shim strips comments and
/// whitespace; what matters are the values as dart-sass writes them in that
/// style.
///
/// # Errors
///
/// [`Motivo::Estilos`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::compilar_em;
/// assert_eq!(compilar_em("$c: #fff;\na { color: $c; }", None).unwrap(), "a{color:#fff}\n");
/// ```
pub fn compilar_em(fonte: &str, dir: Option<&Path>) -> Result<String, Motivo> {
    compilar_com(fonte, dir, Estilo::Comprimido).map(|(css, _)| css)
}

/// Compiles like `sass_builder` 2.2.1 (the dart-sass of the lock): the official
/// CSS **byte for byte** in the requested style, ending in `\n` — without the
/// trailing `/*# sourceMappingURL=… */` comment, which depends on the file name
/// and on there being a map; whoever writes the `.css` appends it —, and the
/// files the compilation read (the `@use`/`@import` modules, which are inputs
/// too).
///
/// The stylesheet is treated as a file in `dir` (the package is that of the
/// nearest `pubspec.yaml`; without one, `dir` is the root of an anonymous
/// package). Whoever knows the input file should use [`compilar_arquivo`],
/// which also generates the map.
///
/// # Errors
///
/// [`Motivo::Estilos`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::{compilar_com, Estilo};
/// let (css, lidos) = compilar_com(".a { color: red; }", None, Estilo::Expandido).unwrap();
/// assert_eq!(css, ".a {\n  color: red;\n}\n");
/// assert!(lidos.is_empty());
/// ```
pub fn compilar_com(
    fonte: &str,
    dir: Option<&Path>,
    estilo: Estilo,
) -> Result<(String, Vec<PathBuf>), Motivo> {
    // Without a directory, an anonymous package without a root: nothing is
    // loaded.
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
        VersaoDartSass::V1_102_0,
    )
    .map_err(|_| Motivo::Estilos)?;
    Ok((format!("{}\n", c.css), c.lidos))
}

/// O resultado de [`compilar_arquivo`].
#[derive(Debug, Clone)]
pub struct Folha {
    /// The `.css` exactly as `sass_builder` writes it (with the
    /// `sourceMappingURL` comment when there is a map).
    pub css: String,
    /// O `.css.map`, quando pedido.
    pub mapa: Option<String>,
    /// The files read (the input is not included) and those looked up without
    /// existing: dependencies of the compilation.
    pub lidos: Vec<PathBuf>,
    pub sondados: Vec<PathBuf>,
}

/// Compiles the `scss` file like `sass_builder` (input, package and
/// `package_config.json` found from the path) and, if `mapa`, generates its
/// `.css.map`.
///
/// # Errors
///
/// [`Motivo::Estilos`] when the file cannot be read or dart-sass would refuse
/// the stylesheet.
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
        VersaoDartSass::V1_102_0,
    )
    .map_err(|_| Motivo::Estilos)?;
    Ok(Folha {
        css: c.arquivo_css(&entrada),
        mapa: c.mapa,
        lidos: c.lidos,
        sondados: c.sondados,
    })
}

/// Compiles the asset `caminho` (relative to the package root, with `/`) of
/// package `pacote`, with text `fonte`, like the `SassBuilder` of
/// `sass_builder` 2.2.1: `raizes` are the package roots of
/// `package_config.json` and `leitor` is the `BuildStep`. With `mapa`,
/// generates the `.css.map` and puts the `sourceMappingURL` comment in the
/// `.css`. `versao` is the imitated dart-sass (the `sass` of `pubspec.lock`).
///
/// # Errors
///
/// The dart-sass message when the stylesheet is refused.
///
/// ```
/// use dartforge_ng_generator::sass::{compilar_ativo, Estilo, Leitor, VersaoDartSass};
/// use std::collections::BTreeMap;
/// use std::path::Path;
/// struct Nothing;
/// impl Leitor for Nothing {
///     fn existe(&self, _: &Path) -> bool { false }
///     fn ler(&self, _: &Path) -> std::io::Result<String> { Err(std::io::ErrorKind::NotFound.into()) }
/// }
/// let f = compilar_ativo("a { b: c }", "app", "web/x.scss", &BTreeMap::new(), &Nothing, Estilo::Comprimido, true, VersaoDartSass::V1_102_0).unwrap();
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
    versao: VersaoDartSass,
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
        versao,
    )
    .map_err(|e| e.to_string())?;
    Ok(Folha {
        css: c.arquivo_css(&entrada),
        mapa: c.mapa,
        lidos: c.lidos,
        sondados: c.sondados,
    })
}

/// The asset of `arquivo` and the packages: the package is that of the nearest
/// `pubspec.yaml` (without one, the file's directory is the root of an
/// anonymous package); the others come from the
/// `.dart_tool/package_config.json` of the package or of a directory above it
/// (workspace).
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

/// The `name:` of a `pubspec.yaml`.
fn nome_do_pubspec(pubspec: &Path) -> Option<String> {
    let texto = std::fs::read_to_string(pubspec).ok()?;
    texto.lines().find_map(|l| {
        let resto = l.strip_prefix("name:")?;
        let nome = resto.split('#').next()?.trim().trim_matches(['"', '\'']);
        (!nome.is_empty()).then(|| nome.to_owned())
    })
}
