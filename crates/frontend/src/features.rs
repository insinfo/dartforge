//! Language version and features enabled per library.
//!
//! Contract in `docs/VERSOES-LINGUAGEM.md`. Each Dart library has a
//! **language version** (`accepted/2.8/language-versioning`): the library's
//! own `// @dart = x.y` marker, otherwise the package's `languageVersion` in
//! `package_config.json`, otherwise the tool's current version. The version
//! decides which language features apply there — like the CFE's
//! `LibraryFeatures` (`front_end/lib/src/api_prototype/experimental_flags_generated.dart`).
//!
//! Cost: the version is resolved **once** per library, in the loader, before
//! parsing; the parser and later phases only query a bit set
//! ([`LibraryFeatures::tem`]). No phase redoes the computation per node.

pub use dartforge_diagnostics::AnalyzerReference;
use dartforge_diagnostics::Span;

/// Language version `major.minor` (there is no patch version for the
/// language).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct LanguageVersion {
    pub major: u16,
    pub minor: u16,
}

impl LanguageVersion {
    pub const fn new(major: u16, minor: u16) -> Self {
        LanguageVersion { major, minor }
    }

    /// The lowest version Dart 3 accepts (`kernel_target.dart`,
    /// `leastSupportedVersion`): below it there is no null safety.
    pub const MINIMA: LanguageVersion = LanguageVersion::new(2, 12);

    /// DartForge's **floor** and the platform version: the `dart:*` libraries
    /// are those of SDK 3.6.2 (`docs/VERSOES-LINGUAGEM.md`, D1) and are always
    /// analyzed at this version.
    pub const PISO: LanguageVersion = LanguageVersion::new(3, 6);

    /// The tool's current version (D2): that of a file without a marker and
    /// outside any package, and the ceiling of the accepted markers.
    pub const ATUAL: LanguageVersion = LanguageVersion::new(3, 13);

    /// Reads `x.y` (the format of `package_config.json` and of
    /// `--language-version`). Surrounding spaces are tolerated; anything else is
    /// `None`.
    pub fn parse(texto: &str) -> Option<LanguageVersion> {
        let (a, b) = texto.trim().split_once('.')?;
        if a.is_empty()
            || b.is_empty()
            || !a.bytes().all(|c| c.is_ascii_digit())
            || !b.bytes().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        Some(LanguageVersion::new(a.parse().ok()?, b.parse().ok()?))
    }
}

/// The current version ([`LanguageVersion::ATUAL`]).
impl Default for LanguageVersion {
    fn default() -> Self {
        LanguageVersion::ATUAL
    }
}

impl std::fmt::Display for LanguageVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A language feature whose presence depends on the version (or on an
/// experiment).
///
/// Only the features after the 3.6 floor and the experiments DartForge
/// tracks go here. The names are those of `tools/experimental_features.yaml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Feature {
    /// 3.7 — local/parameter/type parameter `_` does not bind a name.
    WildcardVariables,
    /// 3.7 — inference that combines constraints with the declared bounds.
    InferenceUsingBounds,
    /// 3.8 — `?e` and `?k: ?v` in collection literals.
    NullAwareElements,
    /// 3.9 — promotion, reachability and definite assignment assume null
    /// safety.
    SoundFlowAnalysis,
    /// 3.9 — a getter and setter of incompatible types is no longer an error.
    GetterSetterError,
    /// 3.10 — `.id`, `.new`, `const .id(…)` through the context type.
    DotShorthands,
    /// 3.12 — named `{this._x}`, called as `x:`.
    PrivateNamedParameters,
    /// 3.13 — primary constructors, `new`/`factory` without the class name,
    /// `;` body; `var`/`final` only on a declaring parameter.
    PrimaryConstructors,
    /// Experiment (no `enabledIn`): hand-written augmentations.
    Augmentations,
    /// Experiment: *parts* with their own imports.
    EnhancedParts,
    /// Experiment: macros.
    Macros,
    /// 2.13 — `typedef X = T;` with `T` not a function type.
    NonfunctionTypeAliases,
    /// 2.14 — type arguments in annotations; a generic function type as a type
    /// argument.
    GenericMetadata,
    /// 3.0 — `base`, `interface`, `final` and `mixin` on classes (and
    /// `base mixin`).
    ClassModifiers,
    /// 3.0 — `sealed class`.
    SealedClass,
}

impl Feature {
    /// All of them, in declaration order (the index is the bit in
    /// [`LibraryFeatures`]).
    pub const TODOS: [Feature; 15] = [
        Feature::WildcardVariables,
        Feature::InferenceUsingBounds,
        Feature::NullAwareElements,
        Feature::SoundFlowAnalysis,
        Feature::GetterSetterError,
        Feature::DotShorthands,
        Feature::PrivateNamedParameters,
        Feature::PrimaryConstructors,
        Feature::Augmentations,
        Feature::EnhancedParts,
        Feature::Macros,
        Feature::NonfunctionTypeAliases,
        Feature::GenericMetadata,
        Feature::ClassModifiers,
        Feature::SealedClass,
    ];

    /// Experiment name in `tools/experimental_features.yaml` (also the value of
    /// `--enable-experiment`).
    pub const fn nome(self) -> &'static str {
        match self {
            Feature::WildcardVariables => "wildcard-variables",
            Feature::InferenceUsingBounds => "inference-using-bounds",
            Feature::NullAwareElements => "null-aware-elements",
            Feature::SoundFlowAnalysis => "sound-flow-analysis",
            Feature::GetterSetterError => "getter-setter-error",
            Feature::DotShorthands => "dot-shorthands",
            Feature::PrivateNamedParameters => "private-named-parameters",
            Feature::PrimaryConstructors => "primary-constructors",
            Feature::Augmentations => "augmentations",
            Feature::EnhancedParts => "enhanced-parts",
            Feature::Macros => "macros",
            Feature::NonfunctionTypeAliases => "nonfunction-type-aliases",
            Feature::GenericMetadata => "generic-metadata",
            Feature::ClassModifiers => "class-modifiers",
            Feature::SealedClass => "sealed-class",
        }
    }

    /// `enabledIn` from `experimental_features.yaml`: the language version from
    /// which the feature applies with no flag at all. `None` = experiment.
    pub const fn enabled_in(self) -> Option<LanguageVersion> {
        match self {
            Feature::WildcardVariables | Feature::InferenceUsingBounds => {
                Some(LanguageVersion::new(3, 7))
            }
            Feature::NullAwareElements => Some(LanguageVersion::new(3, 8)),
            Feature::SoundFlowAnalysis | Feature::GetterSetterError => {
                Some(LanguageVersion::new(3, 9))
            }
            Feature::DotShorthands => Some(LanguageVersion::new(3, 10)),
            Feature::PrivateNamedParameters => Some(LanguageVersion::new(3, 12)),
            Feature::PrimaryConstructors => Some(LanguageVersion::new(3, 13)),
            Feature::Augmentations | Feature::EnhancedParts | Feature::Macros => None,
            Feature::NonfunctionTypeAliases => Some(LanguageVersion::new(2, 13)),
            Feature::GenericMetadata => Some(LanguageVersion::new(2, 14)),
            Feature::ClassModifiers | Feature::SealedClass => Some(LanguageVersion::new(3, 0)),
        }
    }

    /// The features the 3.6.2 analyzer does not know: a file that uses the
    /// syntax of one of them, in a library where it is disabled, has 3.13.4 as
    /// its reference (docs/ANALYZER-ESPECIFICACAO.md, T2 §1.1).
    pub const DESCONHECIDOS_NO_3_6: [Feature; 3] =
        [Feature::DotShorthands, Feature::PrivateNamedParameters, Feature::PrimaryConstructors];

    /// The version the 3.6.2 analyzer writes in the `experiment_not_enabled`
    /// fix, when it is not that of [`Feature::enabled_in`]:
    /// `releaseVersion ?? 3.6.0`, and in 3.6.2's `experiments.g.dart` these
    /// features had no release version yet (T2 §1.2).
    pub const fn version_reported_in_3_6(self) -> Option<LanguageVersion> {
        match self {
            Feature::NullAwareElements | Feature::WildcardVariables | Feature::InferenceUsingBounds => {
                Some(LanguageVersion::PISO)
            }
            _ => None,
        }
    }

    /// The feature by experiment name.
    pub fn do_nome(nome: &str) -> Option<Feature> {
        Feature::TODOS.into_iter().find(|f| f.nome() == nome)
    }

    const fn bit(self) -> u32 {
        1 << (self as u8)
    }
}

/// The features enabled in a library: the version and a bit set.
///
/// `Copy` and 8 bytes: the parser and later phases store the value, not a
/// reference, and a query is a bitwise `&`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LibraryFeatures {
    versao: LanguageVersion,
    bits: u32,
}

impl LibraryFeatures {
    /// The features of a library at version `versao`, plus the requested
    /// `experimentos`.
    ///
    /// The caller decides whether the experiments apply to the library: by the
    /// rule of the versioning proposal (`:159`), only to those that do **not**
    /// have a marker and whose default version is the current one. See
    /// [`LibraryFeatures::for_library`].
    pub fn new(versao: LanguageVersion, experimentos: &[Feature]) -> Self {
        let mut bits = 0;
        for f in Feature::TODOS {
            if f.enabled_in().is_some_and(|v| versao >= v) {
                bits |= f.bit();
            }
        }
        for f in experimentos {
            bits |= f.bit();
        }
        LibraryFeatures { versao, bits }
    }

    /// The whole rule for a library: `marker` is the version of the
    /// `// @dart = x.y`, if any; `default_version` the package's (or the current one,
    /// outside a package); `current_` the tool's; `experimentos`, those
    /// requested with `--enable-experiment`, which only apply without a marker
    /// and with the default equal to the current version.
    pub fn for_library(
        marker: Option<LanguageVersion>,
        default_version: LanguageVersion,
        current_: LanguageVersion,
        experimentos: &[Feature],
    ) -> Self {
        match marker {
            Some(v) => LibraryFeatures::new(v, &[]),
            None if default_version == current_ => LibraryFeatures::new(default_version, experimentos),
            None => LibraryFeatures::new(default_version, &[]),
        }
    }

    /// The tool's current version, without experiments.
    pub fn atual() -> Self {
        LibraryFeatures::new(LanguageVersion::ATUAL, &[])
    }

    /// O piso 3.6: bibliotecas `dart:*` e o corpus 3.6.
    pub fn piso() -> Self {
        LibraryFeatures::new(LanguageVersion::PISO, &[])
    }

    #[inline]
    pub fn tem(self, f: Feature) -> bool {
        self.bits & f.bit() != 0
    }

    #[inline]
    pub fn versao(self) -> LanguageVersion {
        self.versao
    }

    /// Message of the disabled-feature diagnostic.
    pub fn disabled_message(self, f: Feature) -> String {
        match f.enabled_in() {
            Some(v) => format!(
                "the '{}' feature requires language version {v} or later (this library is at {})",
                f.nome(),
                self.versao
            ),
            None => format!(
                "the '{}' feature is experimental: it requires --enable-experiment={} (this library is at {})",
                f.nome(),
                f.nome(),
                self.versao
            ),
        }
    }
}

impl Default for LibraryFeatures {
    fn default() -> Self {
        LibraryFeatures::atual()
    }
}

/// Looks for the `// @dart = x.y` marker in the source header: before the
/// first token, after a `#!` and other comments.
///
/// Follows the CFE *scanner*
/// (`_fe_analyzer_shared/lib/src/scanner/abstract_scanner.dart`,
/// `tokenizeLanguageVersionOrSingleLineComment`): only `//` (not `///`),
/// spaces (not tabs) around `@dart`, `=` and the version, and nothing after
/// the version but spaces up to the end of the line. A marker inside a block
/// comment does not count, and only the first one applies. A pure scan over
/// bytes, without allocating: it costs the size of the comment header.
pub fn marcador_versao(fonte: &str) -> Option<(LanguageVersion, Span)> {
    let b = fonte.as_bytes();
    let mut i = 0;
    // BOM UTF-8.
    if b.starts_with(&[0xEF, 0xBB, 0xBF]) {
        i = 3;
    }
    // `#!` only on the first line.
    if b[i..].starts_with(b"#!") {
        while i < b.len() && b[i] != b'\n' {
            i += 1;
        }
    }
    loop {
        while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n' | 0x0C) {
            i += 1;
        }
        if b[i..].starts_with(b"//") {
            let start_offset = i;
            let mut end_offset = i;
            while end_offset < b.len() && b[end_offset] != b'\n' && b[end_offset] != b'\r' {
                end_offset += 1;
            }
            if let Some(v) = comment_version(&b[start_offset..end_offset]) {
                return Some((
                    v,
                    Span {
                        start: start_offset,
                        end: end_offset,
                    },
                ));
            }
            i = end_offset;
        } else if b[i..].starts_with(b"/*") {
            // Block comment, nested as in the lexer.
            let mut prof = 0usize;
            loop {
                if i >= b.len() {
                    return None;
                }
                if b[i..].starts_with(b"/*") {
                    prof += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    prof -= 1;
                    i += 2;
                    if prof == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else {
            return None;
        }
    }
}

/// `// @dart = x.y` (without the `\n`), with the CFE grammar; otherwise
/// `None`.
fn comment_version(line: &[u8]) -> Option<LanguageVersion> {
    let mut r = line.strip_prefix(b"//")?;
    if r.first() == Some(&b'/') {
        return None; // `///` is documentation
    }
    let spaces = |r: &mut &[u8]| {
        while r.first() == Some(&b' ') {
            *r = &r[1..];
        }
    };
    spaces(&mut r);
    r = r.strip_prefix(b"@dart")?;
    spaces(&mut r);
    r = r.strip_prefix(b"=")?;
    spaces(&mut r);
    let digits_ = |r: &mut &[u8]| -> Option<u16> {
        let n = r.iter().take_while(|c| c.is_ascii_digit()).count();
        if n == 0 {
            return None;
        }
        let v = std::str::from_utf8(&r[..n]).ok()?.parse().ok();
        *r = &r[n..];
        v
    };
    let major = digits_(&mut r)?;
    r = r.strip_prefix(b".")?;
    let minor = digits_(&mut r)?;
    spaces(&mut r);
    r.is_empty().then_some(LanguageVersion::new(major, minor))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn v(a: u16, b: u16) -> LanguageVersion {
        LanguageVersion::new(a, b)
    }

    #[test]
    fn simple_marker_and_with_spaces() {
        assert_eq!(
            marcador_versao("// @dart=3.6\nvoid main() {}").map(|m| m.0),
            Some(v(3, 6))
        );
        assert_eq!(
            marcador_versao("//@dart = 2.12   \nx").map(|m| m.0),
            Some(v(2, 12))
        );
        assert_eq!(
            marcador_versao("// @dart = 3.13").map(|m| m.0),
            Some(v(3, 13))
        );
        let (_, span) = marcador_versao("\n\n// @dart=3.7\r\nmain(){}").unwrap();
        assert_eq!(span, Span { start: 2, end: 14 });
    }

    #[test]
    fn marker_after_script_and_comments() {
        let f = "#!/usr/bin/env dart\n// license\n/* block\n// @dart=2.1 does not count */\n// @dart=3.8\nimport 'x.dart';";
        assert_eq!(marcador_versao(f).map(|m| m.0), Some(v(3, 8)));
        assert_eq!(
            marcador_versao("\u{FEFF}// @dart=3.7\n").map(|m| m.0),
            Some(v(3, 7))
        );
    }

    #[test]
    fn only_the_first_counts_and_only_before_the_first_token() {
        assert_eq!(
            marcador_versao("// @dart=3.6\n// @dart=3.13\n").map(|m| m.0),
            Some(v(3, 6))
        );
        assert_eq!(marcador_versao("library a;\n// @dart=3.6\n"), None);
    }

    #[test]
    fn forms_that_are_not_markers() {
        for f in [
            "/// @dart=3.6\n",      // documentation
            "// @dart=3.6 extra\n", // garbage after the version
            "//\t@dart=3.6\n",      // tab
            "// @dart=3\n",         // no minor
            "// @dart=.6\n",        // no major
            "// @ dart=3.6\n",      // space inside `@dart`
            "/* // @dart=3.6 */ x", // inside a block
            "// @dartx=3.6\n",
        ] {
            assert_eq!(marcador_versao(f), None, "{f:?}");
        }
        // A plain comment before does not prevent the following marker.
        assert_eq!(
            marcador_versao("// @dart=3.6 extra\n// @dart=3.7\n").map(|m| m.0),
            Some(v(3, 7))
        );
    }

    #[test]
    fn features_by_version() {
        let f36 = LibraryFeatures::new(v(3, 6), &[]);
        // Those after the floor are disabled in 3.6; the earlier ones (2.13, 2.14,
        // 3.0) are enabled.
        let old_ones = [Feature::NonfunctionTypeAliases, Feature::GenericMetadata, Feature::ClassModifiers, Feature::SealedClass];
        assert!(Feature::TODOS.iter().filter(|f| !old_ones.contains(f)).all(|f| !f36.tem(*f)));
        assert!(old_ones.iter().all(|f| f36.tem(*f)));
        let f212 = LibraryFeatures::new(v(2, 12), &[]);
        assert!(old_ones.iter().all(|f| !f212.tem(*f)));
        let f214 = LibraryFeatures::new(v(2, 14), &[]);
        assert!(f214.tem(Feature::GenericMetadata) && f214.tem(Feature::NonfunctionTypeAliases) && !f214.tem(Feature::ClassModifiers));
        let f310 = LibraryFeatures::new(v(3, 10), &[]);
        assert!(
            f310.tem(Feature::WildcardVariables)
                && f310.tem(Feature::NullAwareElements)
                && f310.tem(Feature::DotShorthands)
        );
        assert!(
            !f310.tem(Feature::PrivateNamedParameters) && !f310.tem(Feature::PrimaryConstructors)
        );
        let atual = LibraryFeatures::atual();
        assert!(atual.tem(Feature::PrimaryConstructors) && !atual.tem(Feature::Macros));
        assert!(LibraryFeatures::new(v(3, 6), &[Feature::Macros]).tem(Feature::Macros));
        assert_eq!(LanguageVersion::parse(" 3.13 "), Some(v(3, 13)));
        assert_eq!(LanguageVersion::parse("3"), None);
        assert_eq!(LanguageVersion::parse("3.x"), None);
        assert_eq!(
            Feature::do_nome("dot-shorthands"),
            Some(Feature::DotShorthands)
        );
    }

    #[test]
    fn experiments_only_without_marker_and_in_current_version() {
        let atual = LanguageVersion::ATUAL;
        let exp = [Feature::Macros];
        assert!(LibraryFeatures::for_library(None, atual, atual, &exp).tem(Feature::Macros));
        assert!(
            !LibraryFeatures::for_library(Some(atual), atual, atual, &exp).tem(Feature::Macros)
        );
        assert!(!LibraryFeatures::for_library(None, v(3, 6), atual, &exp).tem(Feature::Macros));
    }

    /// The transcribed table matches the SDK's `experimental_features.yaml`,
    /// when the reference clone exists (`references/` is outside git).
    #[test]
    fn table_matches_the_sdk_yaml() {
        let yaml = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../references/dart-sdk/tools/experimental_features.yaml");
        let Ok(texto) = std::fs::read_to_string(&yaml) else {
            eprintln!("{} missing; check skipped", yaml.display());
            return;
        };
        for f in Feature::TODOS {
            let cab = format!("\n  {}:\n", f.nome());
            let start_offset = texto
                .find(&cab)
                .unwrap_or_else(|| panic!("{} fora do yaml", f.nome()))
                + cab.len();
            let block: String = texto[start_offset..]
                .lines()
                .take_while(|l| l.starts_with("    ") || l.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            let enabled = block
                .lines()
                .find_map(|l| l.trim().strip_prefix("enabledIn:"))
                .map(|s| {
                    let s = s.trim().trim_matches('\'');
                    let mut p = s.split('.');
                    LanguageVersion::new(
                        p.next().unwrap().parse().unwrap(),
                        p.next().unwrap().parse().unwrap(),
                    )
                });
            assert_eq!(enabled, f.enabled_in(), "{}", f.nome());
        }
    }
}
