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
use crate::view::Reason;
use dartforge_sass::OutputStyle;
use dartforge_sass::sass_builder::{self, Asset, Disk, Packages};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How the compilation reads files (the `build_runner` `BuildStep`): the build
/// engine passes its own, which sees the outputs of earlier builders.
pub use dartforge_sass::sass_builder::AssetReader;

/// The imitated dart-sass version (that of `sass` in `pubspec.lock`).
pub use dartforge_sass::DartSassVersion;

/// The output style of `sass_builder` (`outputStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Expanded,
    Compressed,
}

/// Output settings for an asset compilation.
///
/// ```
/// use dartforge_ng_generator::sass::{AssetOptions, Style, DartSassVersion};
/// let options = AssetOptions {
///     style: Style::Compressed,
///     source_map: true,
///     version: DartSassVersion::V1_102_0,
/// };
/// assert!(options.source_map);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct AssetOptions {
    /// CSS output style.
    pub style: Style,
    /// Whether to produce a source map and its CSS reference.
    pub source_map: bool,
    /// Exact Dart Sass compatibility profile.
    pub version: DartSassVersion,
}

impl Style {
    fn output_style(self) -> OutputStyle {
        match self {
            Style::Expanded => OutputStyle::Expanded,
            Style::Compressed => OutputStyle::Compressed,
        }
    }
}

/// Compiles Sass to CSS, without resolving modules (`@use`/`@import` fail).
///
/// # Errors
///
/// [`Reason::Styles`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::compile;
/// assert_eq!(compile(".a { .b { top: 0 } }").unwrap(), ".a .b{top:0}\n");
/// ```
pub fn compile(source: &str) -> Result<String, Reason> {
    compile_in(source, None)
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
/// [`Reason::Styles`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::compile_in;
/// assert_eq!(compile_in("$c: #fff;\na { color: $c; }", None).unwrap(), "a{color:#fff}\n");
/// ```
pub fn compile_in(source: &str, dir: Option<&Path>) -> Result<String, Reason> {
    compile_with(source, dir, Style::Compressed).map(|(css, _)| css)
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
/// package). Whoever knows the input file should use [`compile_file`],
/// which also generates the map.
///
/// # Errors
///
/// [`Reason::Styles`] when dart-sass would refuse the stylesheet.
///
/// ```
/// use dartforge_ng_generator::sass::{compile_with, Style};
/// let (css, read_files) = compile_with(".a { color: red; }", None, Style::Expanded).unwrap();
/// assert_eq!(css, ".a {\n  color: red;\n}\n");
/// assert!(read_files.is_empty());
/// ```
pub fn compile_with(
    source: &str,
    dir: Option<&Path>,
    style: Style,
) -> Result<(String, Vec<PathBuf>), Reason> {
    // Without a directory, an anonymous package without a root: nothing is
    // loaded.
    let (input, packages) = match dir {
        Some(d) => identify(&d.join("__entrada__.scss")),
        None => (
            Asset {
                package: "anonimo".into(),
                path: "__entrada__.scss".into(),
            },
            Packages::new(),
        ),
    };
    let c = sass_builder::compile(
        source,
        &input,
        &packages,
        &Disk,
        style.output_style(),
        false,
        DartSassVersion::V1_102_0,
    )
    .map_err(|_| Reason::Styles)?;
    Ok((format!("{}\n", c.css), c.read_files))
}

/// The result of [`compile_file`].
#[derive(Debug, Clone)]
pub struct Sheet {
    /// The `.css` exactly as `sass_builder` writes it (with the
    /// `sourceMappingURL` comment when there is a map).
    pub css: String,
    /// The `.css.map`, when requested.
    pub map: Option<String>,
    /// The files read (the input is not included) and those looked up without
    /// existing: dependencies of the compilation.
    pub read_files: Vec<PathBuf>,
    pub probed_files: Vec<PathBuf>,
}

/// Compiles the `scss` file like `sass_builder` (input, package and
/// `package_config.json` found from the path) and, if `map`, generates its
/// `.css.map`.
///
/// # Errors
///
/// [`Reason::Styles`] when the file cannot be read or dart-sass would refuse
/// the stylesheet.
pub fn compile_file(scss: &Path, style: Style, map: bool) -> Result<Sheet, Reason> {
    let source = std::fs::read_to_string(scss).map_err(|_| Reason::Styles)?;
    let (input, packages) = identify(scss);
    let c = sass_builder::compile(
        &source,
        &input,
        &packages,
        &Disk,
        style.output_style(),
        map,
        DartSassVersion::V1_102_0,
    )
    .map_err(|_| Reason::Styles)?;
    Ok(Sheet {
        css: c.css_file(&input),
        map: c.map,
        read_files: c.read_files,
        probed_files: c.probed_files,
    })
}

/// Compiles the asset `path` (relative to the package root, with `/`) of
/// package `package`, with text `source`, like the `SassBuilder` of
/// `sass_builder` 2.2.1: `roots` are the package roots of
/// `package_config.json` and `reader` is the `BuildStep`. With `map`,
/// generates the `.css.map` and puts the `sourceMappingURL` comment in the
/// `.css`. `version` is the imitated dart-sass (the `sass` of `pubspec.lock`).
///
/// # Errors
///
/// The dart-sass message when the stylesheet is refused.
///
/// ```
/// use dartforge_ng_generator::sass::{compile_asset, AssetOptions, Style, AssetReader, DartSassVersion};
/// use std::collections::BTreeMap;
/// use std::path::Path;
/// struct Nothing;
/// impl AssetReader for Nothing {
///     fn exists(&self, _: &Path) -> bool { false }
///     fn read(&self, _: &Path) -> std::io::Result<String> { Err(std::io::ErrorKind::NotFound.into()) }
/// }
/// let f = compile_asset("a { b: c }", "app", "web/x.scss", &BTreeMap::new(), &Nothing, AssetOptions { style: Style::Compressed, source_map: true, version: DartSassVersion::V1_102_0 }).unwrap();
/// assert_eq!(f.css, "a{b:c}\n\n/*# sourceMappingURL=x.css.map */\n");
/// assert_eq!(f.map.unwrap(), r#"{"version":3,"sourceRoot":"","sources":["x.scss"],"names":[],"mappings":"AAAA"}"#);
/// ```
pub fn compile_asset(
    source: &str,
    package: &str,
    path: &str,
    roots: &BTreeMap<String, PathBuf>,
    reader: &dyn AssetReader,
    options: AssetOptions,
) -> Result<Sheet, String> {
    let AssetOptions { style, source_map: map, version } = options;
    let mut packages = Packages::new();
    for (name, root) in roots {
        packages.insert(name.clone(), root.clone());
    }
    let input = Asset {
        package: package.to_owned(),
        path: path.to_owned(),
    };
    let c = sass_builder::compile(
        source,
        &input,
        &packages,
        reader,
        style.output_style(),
        map,
        version,
    )
    .map_err(|e| e.to_string())?;
    Ok(Sheet {
        css: c.css_file(&input),
        map: c.map,
        read_files: c.read_files,
        probed_files: c.probed_files,
    })
}

/// The asset of `file` and the packages: the package is that of the nearest
/// `pubspec.yaml` (without one, the file's directory is the root of an
/// anonymous package); the others come from the
/// `.dart_tool/package_config.json` of the package or of a directory above it
/// (workspace).
fn identify(file: &Path) -> (Asset, Packages) {
    let dir = file.parent().unwrap_or(Path::new(""));
    let root = dir.ancestors().find(|d| d.join("pubspec.yaml").is_file());
    let (root, name) = match root {
        Some(r) => (
            r.to_path_buf(),
            pubspec_name(&r.join("pubspec.yaml")).unwrap_or_default(),
        ),
        None => (dir.to_path_buf(), "anonimo".to_owned()),
    };
    let mut packages = root
        .ancestors()
        .map(|d| d.join(".dart_tool/package_config.json"))
        .find(|c| c.is_file())
        .and_then(|c| Packages::from_package_config(&c).ok())
        .unwrap_or_default();
    packages.insert(name.clone(), root.clone());
    let path = file
        .strip_prefix(&root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/");
    (
        Asset {
            package: name,
            path,
        },
        packages,
    )
}

/// The `name:` of a `pubspec.yaml`.
fn pubspec_name(pubspec: &Path) -> Option<String> {
    let text = std::fs::read_to_string(pubspec).ok()?;
    text.lines().find_map(|l| {
        let rest = l.strip_prefix("name:")?;
        let name = rest.split('#').next()?.trim().trim_matches(['"', '\'']);
        (!name.is_empty()).then(|| name.to_owned())
    })
}
