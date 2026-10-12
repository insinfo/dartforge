//! Builder triggers (`build_runner` ≥ 2.7.0, `build_config` ≥ 1.2.0): port of
//! `build_plan/build_triggers.dart` and of `_allowedByTriggers` in
//! `build/build.dart` of `build_runner` 2.16.1.
//!
//! A step whose builder has the option `run_only_if_triggered: true` (in the
//! phase's already merged options) only runs if some trigger of the builder
//! fires on the primary input; with no trigger at all, it never runs. The
//! triggers come from the `triggers` section of the `build.yaml` of **all**
//! the packages in the build, accumulated by builder name (without normalizing
//! the key: `annotation` is not `package:annotation`).
//!
//! * `import <library>`: the primary unit has an `import` whose URI is
//!   literally `package:<library>` (an `export`, a relative import or the
//!   parts do not count).
//! * `annotation <Name>`: some **top-level** declaration of the primary unit
//!   or of an included part (`part '...'`) has metadata whose written name is
//!   `Name` — whole (`@Name`, `@p.Name` with a prefix, `@Name.c`) or without
//!   the first segment (`@p.Name` matches `Name`; `@p.Name.c` matches
//!   `Name.c`).
//!
//! The official implementation parses with
//! `parseString(throwIfDiagnostics: false)` and walks the `directives` and
//! `declarations` of the `CompilationUnit`. Here the same information comes
//! from the tokens of DartForge's lexer, at the top level (outside
//! parentheses, brackets and braces): metadata followed by a directive belongs
//! to the directive, followed by anything else it belongs to the declaration.
//! A source the lexer rejects (an unterminated string or comment) counts as
//! triggered: the builder runs and reports the error, instead of the step
//! silently disappearing.
use crate::config::BuildConfig;
use crate::valor::Valor;
use dartforge_frontend::token::{Kind, Op};
use std::collections::BTreeMap;

/// A trigger.
///
/// The `Debug` form of the variants goes into the engine's fingerprint
/// (`motor.rs` hashes `Gatilhos::por_builder`), so their names are kept.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trigger {
    /// `import <library>`: compared with `package:<library>`.
    Import(String),
    /// `annotation <Name>`.
    Anotacao(String),
}

impl Trigger {
    /// `BuildTrigger._tryParse`: the trigger and, if any, the warning.
    fn parse(text: &str) -> (Option<Trigger>, Option<String>) {
        if let Some(i) = text.strip_prefix("import ") {
            let warning = (!valid_import(i)).then(|| format!("Invalid import trigger: `{i}`"));
            (Some(Trigger::Import(i.to_string())), warning)
        } else if let Some(a) = text.strip_prefix("annotation ") {
            let warning =
                (!valid_annotation(a)).then(|| format!("Invalid annotation trigger: `{a}`"));
            (Some(Trigger::Anotacao(a.to_string())), warning)
        } else {
            (None, Some(format!("Invalid trigger: `{text}`")))
        }
    }

    /// Does it need the parts (`checksParts`)?
    fn reads_parts(&self) -> bool {
        matches!(self, Trigger::Anotacao(_))
    }

    /// `triggersOn`.
    fn fires(&self, units: &[Unit]) -> bool {
        match self {
            Trigger::Import(i) => {
                let target = format!("package:{i}");
                units.iter().any(|u| u.imports.contains(&target))
            }
            Trigger::Anotacao(a) => units.iter().any(|u| {
                u.annotations.iter().any(|name| {
                    name == a || name.split_once('.').is_some_and(|(_, rest)| rest == a)
                })
            }),
        }
    }
}

/// `^[a-z][a-z0-9_/.]*$`.
fn valid_import(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|p| p.is_ascii_lowercase())
        && c.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || "_/.".contains(x))
}

/// `^[a-zA-Z_][a-zA-Z0-9]*$` (the `_` only in the first position, as in the
/// official implementation).
fn valid_annotation(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|p| p.is_ascii_alphabetic() || p == '_')
        && c.all(|x| x.is_ascii_alphanumeric())
}

/// `^[a-z][a-z0-9_]*:[a-z][a-z0-9_]` searched with `contains`: it is enough
/// for the start to match.
fn valid_builder_name(s: &str) -> bool {
    let Some((package, name)) = s.split_once(':') else {
        return false;
    };
    let mut p = package.chars();
    let mut n = name.chars();
    p.next().is_some_and(|x| x.is_ascii_lowercase())
        && p.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '_')
        && n.next().is_some_and(|x| x.is_ascii_lowercase())
}

/// `BuildTriggers`: the triggers accumulated from all packages.
#[derive(Debug, Clone, Default)]
pub struct Gatilhos {
    /// By builder name (as written in `build.yaml`), without repetition, in
    /// the order in which they appeared.
    pub por_builder: BTreeMap<String, Vec<Trigger>>,
    /// Parsing warnings, by package (`warningsByPackage`).
    pub warnings: Vec<(String, Vec<String>)>,
}

impl Gatilhos {
    /// `BuildTriggers.fromConfigs`, in the given package order.
    ///
    /// ```
    /// use dartforge_build::config::BuildConfig;
    /// use dartforge_build::gatilhos::{Gatilhos, Trigger};
    /// use dartforge_build::perfil::Perfil;
    /// let profile = Perfil::das_versoes(Some("1.3.3"), Some("2.16.1"));
    /// let t = "triggers:\n  p:b:\n    - annotation A\n    - import p/a.dart\n    - other\n";
    /// let c = BuildConfig::de_texto_no_perfil("p", &[], t, "build.yaml", profile).unwrap();
    /// let g = Gatilhos::das_configs([&c]);
    /// assert_eq!(g.por_builder["p:b"], [Trigger::Anotacao("A".into()), Trigger::Import("p/a.dart".into())]);
    /// assert_eq!(g.warnings[0].1, ["Invalid trigger: `other`"]);
    /// ```
    pub fn das_configs<'a>(configs: impl IntoIterator<Item = &'a BuildConfig>) -> Gatilhos {
        let mut g = Gatilhos::default();
        for c in configs {
            let mut warnings = Vec::new();
            for (builder, value) in &c.gatilhos {
                if !valid_builder_name(builder) {
                    warnings.push(format!("Invalid builder name: `{builder}`"));
                }
                let Valor::Lista(items) = value else {
                    warnings.push(format!(
                        "Invalid `triggers`, should be a list of triggers: {}",
                        value.texto_canonico()
                    ));
                    continue;
                };
                let mut new = Vec::new();
                for item in items {
                    // An item that is not a string is neither a trigger nor a
                    // warning.
                    let Valor::Texto(t) = item else { continue };
                    let (trigger, warning) = Trigger::parse(t);
                    new.extend(trigger);
                    warnings.extend(warning);
                }
                if !new.is_empty() {
                    let v = g.por_builder.entry(builder.clone()).or_default();
                    for n in new {
                        if !v.contains(&n) {
                            v.push(n);
                        }
                    }
                }
            }
            if !warnings.is_empty() {
                g.warnings.push((c.pacote.clone(), warnings));
            }
        }
        g
    }

    /// The triggers of the builder `key` (`package:name`). `short`:
    /// `build_runner` between 2.7.0 and 2.10.0 looked up `p:p` as `p`.
    pub fn do_builder(&self, key: &str, short: bool) -> Option<&[Trigger]> {
        let name = match key.split_once(':') {
            Some((p, n)) if short && p == n => p,
            _ => key,
        };
        self.por_builder.get(name).map(Vec::as_slice)
    }

    /// `renderWarnings`.
    pub fn texto_dos_avisos(&self) -> Option<String> {
        if self.warnings.is_empty() {
            return None;
        }
        let mut s = String::new();
        for (package, warnings) in &self.warnings {
            s.push_str(&format!("build.yaml of package:{package}:\n"));
            for a in warnings {
                s.push_str(&format!("  {a}\n"));
            }
        }
        s.push_str("See https://pub.dev/packages/build_config#triggers for valid usage.");
        Some(s)
    }
}

/// What triggers look at in a compilation unit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unit {
    /// `stringValue` of each `import` (`None` if it has interpolation).
    pub imports: Vec<String>,
    /// URIs of the `part` directives (not `part of`).
    pub parts: Vec<String>,
    /// Written name of the metadata of each top-level declaration: `Name`,
    /// `p.Name`, `Name.c`, `p.Name.c`.
    pub annotations: Vec<String>,
}

/// Reads the top-level directives and metadata of a Dart source. `None` when
/// the lexer rejects the source.
///
/// ```
/// use dartforge_build::gatilhos::parse;
/// let u = parse("import 'package:a/b.dart' as b;\npart 'x.g.dart';\n@b.Generate()\nclass A { @Other() void f() {} }\n").unwrap();
/// assert_eq!(u.imports, ["package:a/b.dart"]);
/// assert_eq!(u.parts, ["x.g.dart"]);
/// assert_eq!(u.annotations, ["b.Generate"]);
/// ```
pub fn parse(source: &str) -> Option<Unit> {
    let tokens = dartforge_frontend::lexer::lex(source).ok()?;
    // A source with a scanner error (the unterminated string; the lexer goes
    // on, like fasta's) is left unanalyzed: the caller fires as a precaution.
    if tokens.iter().any(|t| matches!(t.kind, Kind::Str(f) | Kind::StrEnd(f) if f.aberta)) {
        return None;
    }
    let text = |i: usize| tokens[i].text(source);
    let is_ident = |i: usize, t: &str| tokens[i].kind == Kind::Ident && text(i) == t;
    let is_string = |i: usize| matches!(tokens[i].kind, Kind::Str(_) | Kind::StrBegin(..));
    let mut u = Unit::default();
    let mut pending: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let k = tokens[i].kind;
        match k {
            Kind::Eof => break,
            Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => {
                depth += 1;
                i += 1;
                continue;
            }
            Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => {
                depth = depth.saturating_sub(1);
                i += 1;
                continue;
            }
            _ if depth > 0 => {
                i += 1;
                continue;
            }
            _ => {}
        }
        if k == Kind::Op(Op::At) {
            // `@a`, `@a.b`, `@a.b.c`, with type arguments and optional
            // arguments (the parentheses are skipped by the loop).
            let mut name = Vec::new();
            let mut j = i + 1;
            if tokens[j].kind == Kind::Ident {
                name.push(text(j));
                j += 1;
                while tokens[j].kind == Kind::Op(Op::Dot) && tokens[j + 1].kind == Kind::Ident {
                    name.push(text(j + 1));
                    j += 2;
                }
            }
            if tokens[j].kind == Kind::Op(Op::Lt) {
                let mut angles = 0usize;
                while tokens[j].kind != Kind::Eof {
                    match tokens[j].kind {
                        Kind::Op(Op::Lt) => angles += 1,
                        Kind::Op(Op::Gt) => angles = angles.saturating_sub(1),
                        _ => {}
                    }
                    j += 1;
                    if angles == 0 {
                        break;
                    }
                }
            }
            if !name.is_empty() {
                pending.push(name.join("."));
            }
            i = j;
            continue;
        }
        // Directives: the metadata before them belongs to them.
        let directive = (is_ident(i, "import") || is_ident(i, "export")) && is_string(i + 1)
            || is_ident(i, "part") && (is_string(i + 1) || is_ident(i + 1, "of"))
            || is_ident(i, "library")
                && (tokens[i + 1].kind == Kind::Ident
                    || tokens[i + 1].kind == Kind::Op(Op::Semicolon));
        if directive {
            pending.clear();
            let value = string_value(&tokens, source, i + 1);
            if is_ident(i, "import") {
                u.imports.extend(value);
            } else if is_ident(i, "part") && !is_ident(i + 1, "of") {
                u.parts.extend(value);
            }
            // Up to the top-level `;`.
            let mut j = i + 1;
            let mut p = 0usize;
            while tokens[j].kind != Kind::Eof {
                match tokens[j].kind {
                    Kind::Op(Op::LParen | Op::LBracket | Op::LBrace) => p += 1,
                    Kind::Op(Op::RParen | Op::RBracket | Op::RBrace) => p = p.saturating_sub(1),
                    Kind::Op(Op::Semicolon) if p == 0 => break,
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        // Any other top-level token starts (or continues) a declaration.
        u.annotations.append(&mut pending);
        i += 1;
    }
    Some(u)
}

/// `StringLiteral.stringValue` starting at token `i`: adjacent strings
/// concatenated; `None` if any of them has interpolation.
fn string_value(
    tokens: &[dartforge_frontend::token::Token],
    source: &str,
    mut i: usize,
) -> Option<String> {
    let mut s = String::new();
    let mut any = false;
    loop {
        match tokens[i].kind {
            Kind::Str(f) => {
                let t = tokens[i].text(source);
                let t = if f.raw { &t[1..] } else { t };
                let quotes = if f.triple { 3 } else { 1 };
                let content = &t[quotes..t.len() - quotes];
                let v = dartforge_frontend::lexer::decode_string(content, f.raw, f.triple).ok()?;
                s.push_str(&v.to_string_lossy());
                any = true;
                i += 1;
            }
            Kind::StrBegin(..) => return None,
            _ => break,
        }
    }
    any.then_some(s)
}

/// `_allowedByTriggers` after the option: does any trigger fire? `primary`:
/// the input's source; `part(uri)`: the text of a part readable by the phase
/// (the call records the query), or `None`.
pub fn disparado(
    triggers: &[Trigger],
    primary: &str,
    mut part: impl FnMut(&str) -> Option<String>,
) -> bool {
    let Some(unit) = parse(primary) else {
        return true;
    };
    let mut all: Option<Vec<Unit>> = None;
    for g in triggers {
        if g.reads_parts() {
            let units = all.get_or_insert_with(|| {
                let mut v = vec![unit.clone()];
                for uri in &unit.parts {
                    if let Some(t) = part(uri) {
                        // A part the lexer rejects: the official implementation
                        // still parses it with errors; here it is left without
                        // declarations.
                        v.extend(parse(&t));
                    }
                }
                v
            });
            if g.fires(units) {
                return true;
            }
        } else if g.fires(std::slice::from_ref(&unit)) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotations(f: &str) -> Vec<String> {
        parse(f).unwrap().annotations
    }

    #[test]
    fn top_level_metadata() {
        assert_eq!(annotations("@A() class X {}"), ["A"]);
        assert_eq!(annotations("@p.A() class X {}"), ["p.A"]);
        assert_eq!(annotations("@p.A.c() class X {}"), ["p.A.c"]);
        assert_eq!(annotations("@A<int>() class X {}"), ["A"]);
        assert_eq!(annotations("@A\n@B(1, [2]) void f() {}"), ["A", "B"]);
        // Member, parameter and enum value are not top-level.
        assert!(annotations("class X { @A() void f(@B() int x) {} }").is_empty());
        assert!(annotations("enum E { @A() a }").is_empty());
        // Directive metadata is not declaration metadata.
        assert!(annotations("@A() library x;\n@B() import 'a.dart';").is_empty());
        // Comments and strings do not count.
        assert!(annotations("// @A()\nconst s = '@A()';").is_empty());
    }

    #[test]
    fn directives() {
        let u = parse(
            "library;\nimport 'package:a/b.dart';\nimport \"package:\" 'a/c.dart';\nimport 'x$y.dart';\nexport 'package:a/d.dart';\nimport 'package:a/e.dart' if (dart.library.io) 'package:a/f.dart';\npart 'p.dart';\n",
        )
        .unwrap();
        assert_eq!(
            u.imports,
            ["package:a/b.dart", "package:a/c.dart", "package:a/e.dart"]
        );
        assert_eq!(u.parts, ["p.dart"]);
        assert!(parse("part of 'x.dart';").unwrap().parts.is_empty());
    }

    #[test]
    fn firing() {
        let a = [Trigger::Anotacao("Generate".into())];
        assert!(disparado(&a, "@Generate() class X {}", |_| None));
        assert!(disparado(&a, "@an.Generate() class X {}", |_| None));
        assert!(!disparado(&a, "@an.Generate.named() class X {}", |_| None));
        assert!(!disparado(&a, "@Generate.named() class X {}", |_| None));
        assert!(disparado(
            &[Trigger::Anotacao("Generate.named".into())],
            "@Generate.named() class X {}",
            |_| None
        ));
        // Through the parts.
        let mut read = Vec::new();
        assert!(disparado(&a, "part 'y.dart';\nclass X {}", |u| {
            read.push(u.to_string());
            Some("part of 'x.dart';\n@Generate() class Y {}".into())
        }));
        assert_eq!(read, ["y.dart"]);
        // The import does not look at parts.
        let i = [Trigger::Import("a/b.dart".into())];
        assert!(!disparado(&i, "part 'y.dart';", |_| Some(
            "import 'package:a/b.dart';".into()
        )));
        assert!(disparado(&i, "import 'package:a/b.dart';", |_| None));
        assert!(!disparado(&i, "import 'b.dart';", |_| None));
        // Without triggers, it does not fire; a source the lexer rejects fires.
        assert!(!disparado(&[], "@Generate() class X {}", |_| None));
        assert!(disparado(&a, "const s = 'unterminated", |_| None));
    }

    #[test]
    fn name_validation() {
        assert!(valid_builder_name("p:b"));
        assert!(valid_builder_name("p_1:b"));
        assert!(!valid_builder_name("b"));
        assert!(!valid_builder_name("P:b"));
        assert!(valid_import("a/b_c.dart"));
        assert!(!valid_import("package:a/b.dart"));
        assert!(valid_annotation("_Generate"));
        assert!(!valid_annotation("Gene_rate"));
    }
}
