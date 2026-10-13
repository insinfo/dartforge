//! Compilation the way `sass_builder` 2.2.1 does it (`lib/sass_builder.dart`
//! and `lib/src/build_importer.dart`): the input is a `build_runner` asset
//! (`package:pkg/x.scss` for what is in `lib/`, `asset:pkg/web/y.scss` for
//! the rest), and the stylesheets it loads are resolved by the
//! `BuildImporter`, that is, by the `AssetId.resolve` of the `build` package
//! against the packages of `package_config.json`.
//!
//! Options that `sass_builder` passes to dart-sass
//! (`compileStringToResultAsync`): `syntax: Syntax.forPath(input)`,
//! `importers: [BuildImporter]`, `style` (`expanded` by default,
//! `compressed`), `url: inputId.uri` and `sourceMap`. No `loadPaths`,
//! `quietDeps`, `charset` (default: on) or custom functions; the base
//! importer of the input is `FilesystemImporter.cwd`, which does not resolve
//! `package:`/`asset:` URLs — in practice only the `BuildImporter` resolves.
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::importer::{Importer, ImporterResult};
use crate::{InputSyntax, Options, OutputStyle};

/// A `build_runner` asset: package and path inside it (`lib/a.scss`,
/// `web/b.scss`), always with `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Asset {
    pub package: String,
    pub path: String,
}

impl Asset {
    /// `AssetId.uri`: `package:` inside `lib/`, `asset:` outside.
    pub fn uri(&self) -> String {
        match self.path.strip_prefix("lib/") {
            Some(rest) => format!("package:{}/{rest}", self.package),
            None => format!("asset:{}/{}", self.package, self.path),
        }
    }

    /// `AssetId.resolve(uri, from: from)`: a relative URL resolves against
    /// the URI of `from` (`Uri.resolveUri`); only `package:` and `asset:` are
    /// accepted.
    pub fn resolve(uri: &str, from: &Asset) -> Result<Asset, String> {
        let resolved = match scheme(uri) {
            Some(_) => uri.to_owned(),
            None => resolve_relative(&from.uri(), uri),
        };
        let error =
            || format!("Cannot resolve {uri}; only \"package\" and \"asset\" schemes supported");
        let (sch, rest) = resolved.split_once(':').ok_or_else(error)?;
        // `pathSegments` of Dart's `Uri`: the path without query/fragment,
        // split on `/` (a leading `/` does not produce an empty segment).
        let rest = rest.split(['?', '#']).next().unwrap_or("");
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        let mut segs = rest.split('/');
        let package = segs.next().unwrap_or("").to_owned();
        let others: Vec<&str> = segs.collect();
        match sch {
            "package" => Ok(Asset {
                package,
                path: join_segments("lib", &others),
            }),
            "asset" => Ok(Asset {
                package,
                path: join_segments("", &others),
            }),
            _ => Err(error()),
        }
    }
}

/// `p.url.join(base, p.url.joinAll(segs))` with the normalization that the
/// `AssetId` constructor does (`p.url.normalize`).
fn join_segments(base: &str, segs: &[&str]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for s in std::iter::once(base).chain(segs.iter().copied()) {
        match s {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|u| *u != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// The scheme of a URI (`[a-zA-Z][a-zA-Z0-9+.-]*:`), if any.
fn scheme(uri: &str) -> Option<&str> {
    let (e, _) = uri.split_once(':')?;
    let mut cs = e.chars();
    let first = cs.next()?;
    (first.is_ascii_alphabetic() && cs.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c)))
        .then_some(e)
}

/// Dart's `Uri.resolveUri` for a reference without a scheme against a base
/// without authority (`package:`/`asset:`): RFC 3986 §5.2, with Dart's
/// removal of dot segments (`_removeDotSegments`, which for a path without
/// a leading `/` keeps the leftover `..` — `_normalizeRelativePath`).
fn resolve_relative(base: &str, rel: &str) -> String {
    let (sch, base_path) = base.split_once(':').unwrap_or(("", base));
    let base_path = base_path.split(['?', '#']).next().unwrap_or("");
    let (rel_path, suffix) = match rel.find(['?', '#']) {
        Some(i) => (&rel[..i], &rel[i..]),
        None => (rel, ""),
    };
    if rel_path.is_empty() {
        return format!("{sch}:{base_path}{suffix}");
    }
    let joined = if rel_path.starts_with('/') {
        rel_path.to_owned()
    } else {
        match base_path.rfind('/') {
            Some(i) => format!("{}{rel_path}", &base_path[..=i]),
            None => rel_path.to_owned(),
        }
    };
    let absolute = joined.starts_with('/');
    let mut output: Vec<&str> = Vec::new();
    let segs: Vec<&str> = joined.split('/').collect();
    let n = segs.len();
    let mut trailing_slash = false;
    for (i, s) in segs.iter().enumerate() {
        let last = i + 1 == n;
        match *s {
            "." => trailing_slash = last,
            ".." => {
                if output.last().is_some_and(|u| *u != ".." && !u.is_empty()) {
                    output.pop();
                } else if !absolute {
                    output.push("..");
                }
                trailing_slash = last;
            }
            "" if i == 0 && absolute => {}
            s => {
                output.push(s);
                trailing_slash = false;
            }
        }
    }
    let mut c = output.join("/");
    if absolute {
        c.insert(0, '/');
    }
    if trailing_slash {
        c.push('/');
    }
    format!("{sch}:{c}{suffix}")
}

/// The package roots, by name, as in `package_config.json`.
#[derive(Debug, Clone, Default)]
pub struct Packages {
    roots: HashMap<String, PathBuf>,
}

impl Packages {
    /// Empty packages; add to it with [`Packages::insert`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds (or replaces) the root of a package.
    pub fn insert(&mut self, name: impl Into<String>, root: impl Into<PathBuf>) {
        self.roots.insert(name.into(), root.into());
    }

    /// The root of `name`, if known.
    pub fn root(&self, name: &str) -> Option<&Path> {
        self.roots.get(name).map(PathBuf::as_path)
    }

    /// Reads a `package_config.json` (version 2): `rootUri` absolute
    /// (`file:///…`) or relative to the file's directory.
    ///
    /// # Errors
    ///
    /// The file cannot be read or does not have the expected shape.
    pub fn from_package_config(path: &Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let dir = path.parent().unwrap_or(Path::new(""));
        let mut p = Packages::new();
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "invalid package_config.json");
        // Minimal JSON reading: the objects of `packages` with `name` and
        // `rootUri` (the crate does not depend on a JSON reader).
        let list = text.find("\"packages\"").ok_or_else(invalid)?;
        let mut rest = &text[list..];
        while let Some(i) = rest.find('{') {
            let end = rest[i..].find('}').ok_or_else(invalid)? + i;
            let obj = &rest[i..end];
            if let (Some(name), Some(root)) = (json_field(obj, "name"), json_field(obj, "rootUri"))
            {
                let root = match root.strip_prefix("file://") {
                    Some(abs) => PathBuf::from(percent_decode(abs)),
                    None => dir.join(percent_decode(&root)),
                };
                p.insert(name, normalize(&root));
            }
            rest = &rest[end + 1..];
        }
        Ok(p)
    }
}

fn json_field(obj: &str, name: &str) -> Option<String> {
    let i = obj.find(&format!("\"{name}\""))? + name.len() + 2;
    let after = obj[i..]
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let mut s = String::new();
    let mut cs = after.chars();
    while let Some(c) = cs.next() {
        match c {
            '"' => return Some(s),
            '\\' => match cs.next()? {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                'u' => {
                    let h: String = cs.by_ref().take(4).collect();
                    s.push(char::from_u32(u32::from_str_radix(&h, 16).ok()?)?);
                }
                o => s.push(o),
            },
            c => s.push(c),
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

/// How the importer reads assets: `BuildStep.canRead`/`readAsString`.
/// The default is the disk; the build engine passes its own (which sees the
/// outputs of earlier builders and records the dependencies).
pub trait AssetReader {
    /// Does the file exist and can it be read?
    fn exists(&self, path: &Path) -> bool;
    /// The text of the file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or is not UTF-8.
    fn read(&self, path: &Path) -> io::Result<String>;
}

/// The disk, and nothing else.
#[derive(Debug, Clone, Copy, Default)]
pub struct Disk;

impl AssetReader for Disk {
    fn exists(&self, path: &Path) -> bool {
        path.is_file()
    }
    fn read(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }
}

/// The `BuildImporter` of `sass_builder` over a [`AssetReader`], keeping what
/// was read and what was probed without existing (the action's
/// dependencies).
struct BuildImporter<'a> {
    input: &'a Asset,
    packages: &'a Packages,
    reader: &'a dyn AssetReader,
    read: RefCell<Vec<PathBuf>>,
    probed: RefCell<Vec<PathBuf>>,
}

impl std::fmt::Debug for BuildImporter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildImporter")
            .field("input", self.input)
            .finish()
    }
}

impl BuildImporter<'_> {
    fn file(&self, a: &Asset) -> Option<PathBuf> {
        self.packages.root(&a.package).map(|r| r.join(&a.path))
    }

    /// `buildStep.canRead(id)`.
    fn can_read(&self, a: &Asset) -> bool {
        let Some(f) = self.file(a) else {
            return false;
        };
        let yes = self.reader.exists(&f);
        let mut list = if yes {
            self.read.borrow_mut()
        } else {
            self.probed.borrow_mut()
        };
        if !list.contains(&f) {
            list.push(f);
        }
        yes
    }

    /// `_resolveImport`, resolving relative URLs against `from`.
    fn resolve(&self, import: &str, from: &Asset) -> Result<Option<Asset>, String> {
        let ext = extension(import);
        if ext == ".sass" || ext == ".scss" {
            return exactly_one(self.try_import(import, from)?);
        }
        if let Some(a) = exactly_one(self.try_with_extensions(import, from)?)? {
            return Ok(Some(a));
        }
        // `_tryImportAsDirectory`: `p.url.join(import, 'index')`.
        let index = if import.is_empty() || import.ends_with('/') {
            format!("{import}index")
        } else {
            format!("{import}/index")
        };
        exactly_one(self.try_with_extensions(&index, from)?)
    }

    fn try_with_extensions(&self, import: &str, from: &Asset) -> Result<Vec<Asset>, String> {
        let mut v = self.try_import(&format!("{import}.sass"), from)?;
        v.extend(self.try_import(&format!("{import}.scss"), from)?);
        Ok(v)
    }

    /// `_tryImport`: the partial `_name` and the file itself, in that order.
    fn try_import(&self, import: &str, from: &Asset) -> Result<Vec<Asset>, String> {
        let mut v = Vec::new();
        let (dir, base) = dirname_basename(import);
        let partial = if dir == "." {
            format!("_{base}")
        } else if dir.ends_with('/') {
            format!("{dir}_{base}")
        } else {
            format!("{dir}/_{base}")
        };
        let partial = Asset::resolve(&partial, from)?;
        if self.can_read(&partial) {
            v.push(partial);
        }
        let itself = Asset::resolve(import, from)?;
        if self.can_read(&itself) {
            v.push(itself);
        }
        Ok(v)
    }
}

/// `p.extension` (URL style): from the last `.` of the last segment, unless
/// it is its first character.
fn extension(s: &str) -> &str {
    let base = s.rsplit('/').next().unwrap_or(s);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[i..],
        _ => "",
    }
}

/// `p.dirname`/`p.basename` of the `path` package (POSIX style, as
/// `sass_builder` calls them on the Dart VM on Linux), without trailing
/// slashes.
fn dirname_basename(s: &str) -> (String, String) {
    let t = s.trim_end_matches('/');
    if t.is_empty() {
        return (
            if s.starts_with('/') {
                "/".into()
            } else {
                ".".into()
            },
            if s.starts_with('/') {
                "/".into()
            } else {
                String::new()
            },
        );
    }
    match t.rfind('/') {
        Some(0) => ("/".into(), t[1..].into()),
        Some(i) => {
            let d = t[..i].trim_end_matches('/');
            (
                if d.is_empty() { "/".into() } else { d.into() },
                t[i + 1..].into(),
            )
        }
        None => (".".into(), t.into()),
    }
}

fn exactly_one(v: Vec<Asset>) -> Result<Option<Asset>, String> {
    match v.len() {
        0 => Ok(None),
        1 => Ok(v.into_iter().next()),
        _ => Err("It is not clear which file to import.".into()),
    }
}

impl Importer for BuildImporter<'_> {
    fn canonicalize(
        &self,
        url: &str,
        base: &str,
        _for_import: bool,
    ) -> Result<Option<String>, String> {
        // `ImportCache.canonicalize`: a relative URL goes first to the
        // importer of the stylesheet that contains it, resolved against that
        // stylesheet's canonical URL; then to the list of importers with the
        // URL as it came — and the `BuildImporter` resolves it against the
        // **input** (`AssetId.resolve(…, from: inputId)`).
        if scheme(url).is_none() {
            if let Ok(from) = Asset::resolve(base, self.input) {
                if from != *self.input {
                    let resolved = resolve_relative(base, url);
                    if let Some(a) = self.resolve(&resolved, &from)? {
                        return Ok(Some(a.uri()));
                    }
                }
            }
        }
        Ok(self.resolve(url, self.input)?.map(|a| a.uri()))
    }

    fn load(&self, canonical: &str) -> Result<ImporterResult, String> {
        let a = Asset::resolve(canonical, self.input)?;
        let f = self
            .file(&a)
            .ok_or_else(|| format!("unknown package: {}", a.package))?;
        let contents = self.reader.read(&f).map_err(|e| format!("{e}"))?;
        let mut l = self.read.borrow_mut();
        if !l.contains(&f) {
            l.push(f);
        }
        Ok(ImporterResult {
            contents,
            syntax: InputSyntax::for_path(Path::new(&a.path)),
        })
    }
}

/// The result of [`compile`].
#[derive(Debug, Clone)]
pub struct Compiled {
    /// The `compileResult.css` (without a final `\n` or source map comment).
    pub css: String,
    /// The `.css.map` as `sass_builder` writes it (`json.encode`, with the
    /// `sources` rewritten), when requested.
    pub map: Option<String>,
    /// The files the compilation read (dependencies).
    pub read_files: Vec<PathBuf>,
    /// The files the importer looked for and did not find (also
    /// dependencies: creating one of them changes the result).
    pub probed_files: Vec<PathBuf>,
}

impl Compiled {
    /// The `.css` that `sass_builder` writes: the CSS, the
    /// `sourceMappingURL` comment when there is a source map, and `\n`.
    pub fn css_file(&self, input: &Asset) -> String {
        match &self.map {
            Some(_) => {
                let name = input.path.rsplit('/').next().unwrap_or("");
                let base = match name.rfind('.') {
                    Some(i) if i > 0 => &name[..i],
                    _ => name,
                };
                format!("{}\n\n/*# sourceMappingURL={base}.css.map */\n", self.css)
            }
            None => format!("{}\n", self.css),
        }
    }
}

/// Compiles `source`, the text of the asset `input`, like `SassBuilder`
/// with dart-sass `version`.
///
/// # Errors
///
/// The Sass compilation error (the grass/dart-sass message).
pub fn compile(
    source: &str,
    input: &Asset,
    packages: &Packages,
    reader: &dyn AssetReader,
    style: OutputStyle,
    source_map: bool,
    version: crate::DartSassVersion,
) -> Result<Compiled, Box<crate::Error>> {
    let importer = BuildImporter {
        input,
        packages,
        reader,
        read: RefCell::new(Vec::new()),
        probed: RefCell::new(Vec::new()),
    };
    let options = Options::default()
        .style(style)
        .version(version)
        .quiet(true)
        .importer(&importer)
        .input_syntax(InputSyntax::for_path(Path::new(&input.path)));
    let (css, raw_map) =
        crate::compile_with_source_map(source.to_owned(), &input.uri(), &options, source_map)?;
    let map = raw_map.map(|m| m.json_sass_builder(input));
    Ok(Compiled {
        css,
        map,
        read_files: importer.read.into_inner(),
        probed_files: importer.probed.into_inner(),
    })
}

/// The rewriting of the source map `sources` that `sass_builder` does
/// (`writeSourceMaps`): what is in `lib/` becomes
/// `packages/<package>/<rest>`; what is in `web/` of the package itself,
/// relative to `web/`; the rest stays as it came.
pub fn map_source(url: &str, input: &Asset) -> String {
    let Ok(a) = Asset::resolve(url, input) else {
        return url.to_owned();
    };
    if let Some(rest) = a.path.strip_prefix("lib/") {
        return format!("packages/{}/{rest}", a.package);
    }
    if a.package == input.package {
        if let Some(rest) = a.path.strip_prefix("web/") {
            return rest.to_owned();
        }
    }
    url.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_like_asset_id() {
        let e = Asset {
            package: "app".into(),
            path: "web/css/main.scss".into(),
        };
        assert_eq!(e.uri(), "asset:app/web/css/main.scss");
        let a = Asset::resolve("../x/_y.scss", &e).unwrap();
        assert_eq!(a.path, "web/x/_y.scss");
        let b = Asset::resolve("package:ui/src/a.scss", &e).unwrap();
        assert_eq!(
            (b.package.as_str(), b.path.as_str()),
            ("ui", "lib/src/a.scss")
        );
        assert_eq!(
            map_source("package:ui/src/a.scss", &e),
            "packages/ui/src/a.scss"
        );
        assert_eq!(
            map_source("asset:app/web/css/main.scss", &e),
            "css/main.scss"
        );
        assert!(Asset::resolve("http://x/y", &e).is_err());
    }
}
