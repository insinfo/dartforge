//! The filters the analyzer applies last (plan §2.2, item 4): `analyzer:
//! exclude:` and `analyzer: errors:` of `analysis_options.yaml`, and the
//! `// ignore:` and `// ignore_for_file:` comments.
//!
//! References (analyzer 6.11.0): `IgnoreInfo` (`src/ignore_comments/
//! ignore_info.dart`: a comment on a line of its own applies to the next
//! line; at the end of a line of code, to that line), `ErrorCode.isIgnorable`
//! (an `ERROR`-severity error is not ignored by a comment) and the options'
//! `ErrorProcessor` (`ignore` removes; `info`/`warning`/`error` change the
//! severity).
//!
//! `analysis_options.yaml` is read with a real YAML reader, with the
//! `include:`s merged (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.2.4;
//! rewritten on 2026-10-04 without compiling or running).

use dartforge_diagnostics::{Diagnostic, Severity};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yaml_rust2::{Yaml, YamlLoader};

/// What `analysis_options.yaml` configures for the diagnostics
/// (docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.4 to §4.6 and III.2.4): read with
/// a real YAML reader, with the `include:`s merged.
#[derive(Debug, Default, Clone)]
pub struct AnalysisOptions {
    /// The options file's folder: the `exclude` globs are relative to it.
    /// `None` for options made from a loose text.
    pub folder: Option<PathBuf>,
    /// `analyzer: exclude:` globs, relative to the options' directory.
    pub exclude: Vec<String>,
    /// `analyzer: errors:` — code → `None` (ignore) or the new severity.
    pub errors: BTreeMap<String, Option<Severity>>,
    /// `analyzer: cannot-ignore:` — codes (lower case) and severities
    /// (`error`, `warning`, `info`) that an `// ignore:` does not silence.
    pub unignorable: BTreeSet<String>,
    /// `analyzer: language: strict-casts`.
    pub strict_casts: bool,
    /// `analyzer: language: strict-inference`.
    pub strict_inference: bool,
    /// `analyzer: language: strict-raw-types`.
    pub strict_raw_types: bool,
    /// `analyzer: enable-experiment:`.
    pub experiments: Vec<String>,
    /// `linter: rules:` — rule → enabled (the list form enables each item).
    pub rules: BTreeMap<String, bool>,
    /// `analyzer: optional-checks: chrome-os-manifest-checks`: enables the
    /// `AndroidManifest.xml` validator.
    pub chrome_os_manifest_checks: bool,
}

/// The `String` items of a YAML list.
fn strings(node: &Yaml) -> Vec<&str> {
    node.as_vec().map(|l| l.iter().filter_map(Yaml::as_str).collect()).unwrap_or_default()
}

/// The list holds only strings (`Merger`: a candidate to become a map of booleans).
fn is_string_list(node: &Yaml) -> bool {
    node.as_vec().is_some_and(|l| l.iter().all(|x| x.as_str().is_some()))
}

/// The map holds only boolean values.
fn is_bool_map(node: &Yaml) -> bool {
    node.as_hash().is_some_and(|m| m.iter().all(|(_, v)| v.as_bool().is_some()))
}

/// The list of strings as a map `{item: true}`.
fn promote(node: Yaml) -> Yaml {
    match node {
        Yaml::Array(l) => Yaml::Hash(l.into_iter().map(|x| (x, Yaml::Boolean(true))).collect()),
        other => other,
    }
}

/// `Merger.merge` (`analyzer/lib/src/util/yaml.dart:23-100`): `base` is the
/// included file, `top` the including one (which wins). Maps merge key by
/// key; lists concatenate without repeats; a list of strings against a map of
/// booleans becomes a map; a scalar from the top wins, unless null.
fn merge(base: Yaml, top: Yaml) -> Yaml {
    let (base, top) = if is_string_list(&base) && is_bool_map(&top) {
        (promote(base), top)
    } else if is_bool_map(&base) && is_string_list(&top) {
        (base, promote(top))
    } else {
        (base, top)
    };
    match (base, top) {
        (Yaml::Hash(mut b), Yaml::Hash(c)) => {
            for (k, v) in c {
                let new = match b.remove(&k) {
                    Some(old) => merge(old, v),
                    None => v,
                };
                b.insert(k, new);
            }
            Yaml::Hash(b)
        }
        (Yaml::Array(mut b), Yaml::Array(c)) => {
            for x in c {
                if !b.contains(&x) {
                    b.push(x);
                }
            }
            Yaml::Array(b)
        }
        (b, Yaml::Null | Yaml::BadValue) => b,
        (_, c) => c,
    }
}

/// The YAML map of an options text: a non-map or a YAML error → an empty
/// map (`getOptionsFromSource`, `analysis_options_provider.dart:81-83`).
fn map_from_text(text: &str) -> Yaml {
    let empty = || Yaml::Hash(Default::default());
    match YamlLoader::load_from_str(text) {
        Ok(docs) => match docs.into_iter().next() {
            Some(m @ Yaml::Hash(_)) => m,
            _ => empty(),
        },
        Err(_) => empty(),
    }
}

/// `getOptionsFromSource`: the file's map with the `include:` (a scalar,
/// relative to the file or `package:`) merged underneath, recursively.
/// `visited` breaks cycles (the original does not detect them while loading).
fn map_from_file(file: &Path, packages: &dyn Fn(&str) -> Option<PathBuf>, visited: &mut Vec<PathBuf>) -> Yaml {
    let Ok(text) = std::fs::read_to_string(file) else { return Yaml::Hash(Default::default()) };
    visited.push(file.to_path_buf());
    let options = map_from_text(&text);
    let included = options["include"].as_str().and_then(|uri| {
        if uri.starts_with("package:") {
            packages(uri)
        } else {
            file.parent().map(|p| p.join(uri))
        }
    });
    match included {
        Some(parent) if parent.is_file() && !visited.contains(&parent) => merge(map_from_file(&parent, packages, visited), options),
        _ => options,
    }
}

impl AnalysisOptions {
    /// Reads `<root>/analysis_options.yaml`, if it exists, with the
    /// `include:`s (the `package:` ones through the `package_config.json`
    /// above the root).
    pub fn read(root: &Path) -> AnalysisOptions {
        AnalysisOptions::read_file(&root.join("analysis_options.yaml"))
    }

    /// Reads an options file with the `include:`s merged.
    pub fn read_file(file: &Path) -> AnalysisOptions {
        if !file.is_file() {
            return AnalysisOptions::default();
        }
        let config = dartforge_elements::config::PackageConfig::discover(file)
            .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
        let packages = |uri: &str| config.as_ref().and_then(|c| c.resolve_package_uri(uri).ok());
        let map = map_from_file(file, &packages, &mut Vec::new());
        let mut o = AnalysisOptions::from_map(&map);
        o.folder = file.parent().map(Path::to_path_buf);
        o
    }

    /// The options of a loose text (without following `include:`).
    pub fn from_text(text: &str) -> AnalysisOptions {
        AnalysisOptions::from_map(&map_from_text(text))
    }

    /// `applyOptions` (`apply_options.dart:179-237`) over the already merged map.
    fn from_map(map: &Yaml) -> AnalysisOptions {
        let mut o = AnalysisOptions::default();
        let analyzer = &map["analyzer"];
        if let Some(errors) = analyzer["errors"].as_hash() {
            for (k, v) in errors.iter() {
                let (Some(k), Some(v)) = (k.as_str(), v.as_str()) else { continue };
                let sev = match v.to_lowercase().as_str() {
                    "ignore" => None,
                    "info" => Some(Severity::Info),
                    "warning" => Some(Severity::Warning),
                    "error" => Some(Severity::Error),
                    _ => continue,
                };
                o.errors.insert(k.to_lowercase(), sev);
            }
        }
        for g in strings(&analyzer["exclude"]) {
            o.exclude.push(g.to_string());
            // `x/**` also excludes the folder `x` itself.
            if let Some(folder) = g.strip_suffix("/**") {
                o.exclude.push(folder.to_string());
            }
        }
        for n in strings(&analyzer["cannot-ignore"]) {
            o.unignorable.insert(n.to_lowercase());
        }
        let language = &analyzer["language"];
        o.strict_casts = language["strict-casts"].as_bool().unwrap_or(false);
        o.strict_inference = language["strict-inference"].as_bool().unwrap_or(false);
        o.strict_raw_types = language["strict-raw-types"].as_bool().unwrap_or(false);
        o.experiments = strings(&analyzer["enable-experiment"]).into_iter().map(str::to_string).collect();
        let checks = &analyzer["optional-checks"];
        o.chrome_os_manifest_checks = checks.as_str() == Some("chrome-os-manifest-checks")
            || checks["chrome-os-manifest-checks"].as_bool().unwrap_or(false);
        let rules = &map["linter"]["rules"];
        for r in strings(rules) {
            o.rules.insert(r.to_string(), true);
        }
        if let Some(m) = rules.as_hash() {
            for (k, v) in m.iter() {
                if let (Some(k), Some(v)) = (k.as_str(), v.as_bool()) {
                    o.rules.insert(k.to_string(), v);
                }
            }
        }
        o
    }

    /// The options file nearest to `file`, going up, strictly below `root`
    /// (§4.5: subfolder options). `None` when the root's applies.
    pub fn from_subfolder(file: &Path, root: &Path) -> Option<PathBuf> {
        let mut folder = file.parent();
        while let Some(p) = folder {
            if p == root || !p.starts_with(root) {
                return None;
            }
            let candidate = p.join("analysis_options.yaml");
            if candidate.is_file() {
                return Some(candidate);
            }
            folder = p.parent();
        }
        None
    }

    /// Is the file (absolute path) excluded by these options' globs, which
    /// are relative to their folder?
    pub fn excludes_file(&self, file: &Path) -> bool {
        let Some(folder) = &self.folder else { return false };
        let Ok(rel) = file.strip_prefix(folder) else { return false };
        let rel = rel.to_string_lossy().replace('\\', "/");
        self.exclude.iter().any(|g| glob(g, &rel))
    }

    /// Can an `// ignore:` silence `d`? Not if the code is in
    /// `cannot-ignore`, or its severity (the one from `errors:`, if changed
    /// there; otherwise the default) is (`AnalysisOptionsImpl.unignorableNames`).
    pub fn is_ignorable(&self, d: &Diagnostic) -> bool {
        if self.unignorable.is_empty() {
            return true;
        }
        let Some(c) = d.code else { return true };
        let info = c.info();
        if self.unignorable.contains(info.name) {
            return false;
        }
        let name_of = |s: Severity| match s {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        // `applyUnignorables` (docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.3,
        // fact c): the code is added through the severity `errors:` gave it
        // and also through the default one (the second `if` runs whenever the
        // first did not `continue`): an error downgraded to `warning` stays
        // unignorable with `cannot-ignore: [error]`.
        if let Some(Some(s)) = self.errors.get(info.name)
            && self.unignorable.contains(name_of(*s))
        {
            return false;
        }
        !self.unignorable.contains(name_of(info.severity))
    }

    /// Can the lint `name` (with unique name `unique`) be silenced by
    /// `// ignore:`? Lints only enter `cannot-ignore` by name, never by
    /// severity (§4.3, fact d), and the comparison ignores case
    /// (`code.name.toUpperCase()`).
    pub fn lint_ignorable(&self, name: &str, unique: &str) -> bool {
        !(self.unignorable.contains(&name.to_lowercase()) || self.unignorable.contains(&unique.to_lowercase()))
    }

    /// Is `rel` (relative to the root, with `/`) outside the analysis? It
    /// matches some `exclude`, or some path component starts with `.`
    /// (`ContextRootImpl._isExcluded`, steps 1 and 3).
    pub fn is_excluded(&self, rel: &str) -> bool {
        rel.split('/').any(|c| c.starts_with('.') && c != "." && c != "..") || self.exclude.iter().any(|g| glob(g, rel))
    }

    /// Applies `errors:` to a diagnostic: `None` if ignored.
    pub fn process(&self, mut d: Diagnostic) -> Option<Diagnostic> {
        let Some(c) = d.code else { return Some(d) };
        match self.errors.get(c.info().name) {
            None => Some(d),
            Some(None) => None,
            Some(Some(s)) => {
                d.severity = *s;
                Some(d)
            }
        }
    }
}

/// The closer of a glob construct starting at `p[0]` (`[` or `{`): the index
/// of the closing delimiter, skipping escapes and nesting.
fn closer(p: &[u8], open: u8, close: u8) -> Option<usize> {
    let mut level = 0usize;
    let mut i = 0;
    while i < p.len() {
        match p[i] {
            b'\\' => i += 1,
            c if c == open => level += 1,
            c if c == close => {
                level -= 1;
                if level == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A `package:glob` glob: `**` matches any sequence (including `/`), `*` any
/// sequence without `/`, `?` one character, `[abc]`, `[a-z]` and
/// `[!abc]`/`[^abc]` one character of the class (never `/`), `{a,b}` one of
/// the alternatives, and `\` removes the meaning of the next character.
pub fn glob(pattern: &str, text: &str) -> bool {
    fn matches(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p.starts_with(b"**") {
            let rest = p[2..].strip_prefix(b"/").unwrap_or(&p[2..]);
            return (0..=t.len()).any(|i| matches(rest, &t[i..])) || matches(&p[2..], t);
        }
        match p[0] {
            b'*' => (0..=t.len()).take_while(|&i| i == 0 || t[i - 1] != b'/').any(|i| matches(&p[1..], &t[i..])),
            b'?' => !t.is_empty() && t[0] != b'/' && matches(&p[1..], &t[1..]),
            b'\\' if p.len() > 1 => !t.is_empty() && t[0] == p[1] && matches(&p[2..], &t[1..]),
            b'[' => {
                let Some(end) = closer(p, b'[', b']') else { return !t.is_empty() && t[0] == b'[' && matches(&p[1..], &t[1..]) };
                if t.is_empty() || t[0] == b'/' {
                    return false;
                }
                let mut class = &p[1..end];
                let negated = matches!(class.first(), Some(b'!' | b'^'));
                if negated {
                    class = &class[1..];
                }
                let mut inside = false;
                let mut i = 0;
                while i < class.len() {
                    let mut c = class[i];
                    if c == b'\\' && i + 1 < class.len() {
                        i += 1;
                        c = class[i];
                    }
                    if i + 2 < class.len() && class[i + 1] == b'-' {
                        inside |= c <= t[0] && t[0] <= class[i + 2];
                        i += 3;
                    } else {
                        inside |= c == t[0];
                        i += 1;
                    }
                }
                inside != negated && matches(&p[end + 1..], &t[1..])
            }
            b'{' => {
                let Some(end) = closer(p, b'{', b'}') else { return !t.is_empty() && t[0] == b'{' && matches(&p[1..], &t[1..]) };
                // The alternatives: separated by the outer level's commas.
                let inner = &p[1..end];
                let mut alternatives: Vec<&[u8]> = Vec::new();
                let (mut level, mut start, mut i) = (0usize, 0usize, 0usize);
                while i < inner.len() {
                    match inner[i] {
                        b'\\' => i += 1,
                        b'{' => level += 1,
                        b'}' => level = level.saturating_sub(1),
                        b',' if level == 0 => {
                            alternatives.push(&inner[start..i]);
                            start = i + 1;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                alternatives.push(&inner[start..]);
                alternatives.iter().any(|a| {
                    let mut joined = a.to_vec();
                    joined.extend_from_slice(&p[end + 1..]);
                    matches(&joined, t)
                })
            }
            c => !t.is_empty() && t[0] == c && matches(&p[1..], &t[1..]),
        }
    }
    matches(pattern.as_bytes(), text.as_bytes())
}

/// An element of the list of an `ignore` comment (`IgnoredElement`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Ignored {
    /// A diagnostic name, in lower case, and its offset in the source.
    Name { name: String, offset: usize },
    /// `type=…`, in lower case.
    Type { kind: String },
}

/// The `// ignore:` comments of a file, like the analyzer's `IgnoreInfo`
/// (`analyzer/lib/src/ignore_comments/ignore_info.dart`;
/// docs/ANALYZER-ESPECIFICACAO-INFRA.md §4.1): per line (1-based) and for the
/// whole file. The comments come from a scan of the source that knows strings
/// and block comments — a `//` inside a string is not a comment.
#[derive(Debug, Default)]
pub struct IgnoreComments {
    by_line: BTreeMap<usize, Vec<Ignored>>,
    file: Vec<Ignored>,
    /// The repeated names (`IgnoreValidator`, `duplicate_ignore`): the name
    /// and the range of the repetition.
    repeated: Vec<(String, dartforge_diagnostics::Span)>,
}

/// The line comments (`//…`) of the source: the offset of the `//` and the
/// text up to the end of the line. Skips strings (simple, triple-quoted and
/// raw, with their `${…}` interpolations) and nested block comments.
fn line_comments(source: &str) -> Vec<(usize, &str)> {
    let b = source.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    // The stack of contexts: `Code` is code (inside strings, the `u32` is the
    // brace depth of the code within an interpolation).
    #[derive(Clone, Copy)]
    enum Ctx {
        /// Code; `braces` counts the `{` opened since the interpolation that
        /// opened it (at the top of the file it never closes).
        Code { braces: u32, from_interpolation: bool },
        /// Inside a string: the delimiter, whether it is triple, whether raw.
        Text { quote: u8, triple: bool, raw: bool },
    }
    let mut stack: Vec<Ctx> = vec![Ctx::Code { braces: 0, from_interpolation: false }];
    let mut i = 0;
    while i < n {
        let Some(top) = stack.last().copied() else { break };
        match top {
            Ctx::Code { braces, from_interpolation } => {
                let ch = b[i];
                if ch == b'/' && b.get(i + 1) == Some(&b'/') {
                    let end = source[i..].find('\n').map_or(n, |k| i + k);
                    out.push((i, source[i..end].trim_end_matches('\r')));
                    i = end;
                } else if ch == b'/' && b.get(i + 1) == Some(&b'*') {
                    let mut level = 1;
                    i += 2;
                    while i < n && level > 0 {
                        if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                            level += 1;
                            i += 2;
                        } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                            level -= 1;
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                } else if ch == b'\'' || ch == b'"' {
                    let raw = i > 0 && b[i - 1] == b'r' && (i < 2 || !(b[i - 2].is_ascii_alphanumeric() || b[i - 2] == b'_' || b[i - 2] == b'$'));
                    let triple = b.get(i + 1) == Some(&ch) && b.get(i + 2) == Some(&ch);
                    stack.push(Ctx::Text { quote: ch, triple, raw });
                    i += if triple { 3 } else { 1 };
                } else if ch == b'{' {
                    if let Some(Ctx::Code { braces, .. }) = stack.last_mut() {
                        *braces += 1;
                    }
                    i += 1;
                } else if ch == b'}' {
                    if from_interpolation && braces == 0 {
                        // Closes the interpolation: back to the string.
                        stack.pop();
                    } else if let Some(Ctx::Code { braces, .. }) = stack.last_mut() {
                        *braces = braces.saturating_sub(1);
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }
            Ctx::Text { quote, triple, raw } => {
                let ch = b[i];
                if !raw && ch == b'\\' {
                    i += 2;
                } else if !raw && ch == b'$' && b.get(i + 1) == Some(&b'{') {
                    stack.push(Ctx::Code { braces: 0, from_interpolation: true });
                    i += 2;
                } else if ch == quote && (!triple || (b.get(i + 1) == Some(&quote) && b.get(i + 2) == Some(&quote))) {
                    stack.pop();
                    i += if triple { 3 } else { 1 };
                } else if !triple && ch == b'\n' {
                    // A single-line string without a closer: the error is the
                    // lexer's; the rest of the file is code again.
                    stack.pop();
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

/// `CommentTokenExtension.ignoredElements` (`ignore_info.dart:202-330`): the
/// elements of the list after the FIRST `:` of the comment. `base` is the
/// comment's offset in the source.
fn comment_elements(lexeme: &str, base: usize) -> Vec<Ignored> {
    let b = lexeme.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let Some(colon) = lexeme.find(':') else { return out };
    let mut i = colon + 1;
    let skip_blanks = |i: &mut usize| {
        while *i < n && (b[*i] == b' ' || b[*i] == b'\t') {
            *i += 1;
        }
    };
    // A word: starts with a letter, continues with letters, digits or `_`.
    let word = |i: &mut usize| -> Option<(usize, usize)> {
        let start = *i;
        if *i < n && b[*i].is_ascii_alphabetic() {
            *i += 1;
            while *i < n && (b[*i].is_ascii_alphanumeric() || b[*i] == b'_') {
                *i += 1;
            }
            Some((start, *i))
        } else {
            None
        }
    };
    loop {
        skip_blanks(&mut i);
        if i >= n {
            return out;
        }
        let Some((start, end)) = word(&mut i) else { return out };
        let text = lexeme[start..end].to_lowercase();
        if text == "type" {
            skip_blanks(&mut i);
            if i >= n || b[i] != b'=' {
                return out;
            }
            i += 1;
            skip_blanks(&mut i);
            let Some((tstart, tend)) = word(&mut i) else { return out };
            if i < n && !(b[i] == b' ' || b[i] == b'\t' || b[i] == b',') {
                return out;
            }
            out.push(Ignored::Type { kind: lexeme[tstart..tend].to_lowercase() });
        } else {
            // A name followed by an odd character (`ignore: http://x`,
            // `name.`) ends the list without including it.
            if i < n && !(b[i] == b' ' || b[i] == b'\t' || b[i] == b',') {
                return out;
            }
            out.push(Ignored::Name { name: text, offset: base + start });
        }
        skip_blanks(&mut i);
        if i >= n || b[i] != b',' {
            // The end, or free text after the last name.
            return out;
        }
        i += 1;
    }
}

impl IgnoreComments {
    pub fn from_text(text: &str) -> IgnoreComments {
        let mut ig = IgnoreComments::default();
        // The start of each line, to find the line of an offset.
        let mut starts: Vec<usize> = vec![0];
        starts.extend(text.bytes().enumerate().filter(|(_, c)| *c == b'\n').map(|(i, _)| i + 1));
        let mut file_raw: Vec<Ignored> = Vec::new();
        let mut line_raw: Vec<(usize, Vec<Ignored>)> = Vec::new();
        for (offset, lexeme) in line_comments(text) {
            // `//+[ ]*ignore:` (two or more slashes) or `//[ ]*ignore_for_file:`
            // (exactly two): only the START of the comment is tested.
            let after_slashes = lexeme.trim_start_matches('/');
            let slashes = lexeme.len() - after_slashes.len();
            let body = after_slashes.trim_start_matches(' ');
            let is_ignore = body.starts_with("ignore:");
            let is_for_file = slashes == 2 && body.starts_with("ignore_for_file:");
            if !is_ignore && !is_for_file {
                continue;
            }
            let elements = comment_elements(lexeme, offset);
            // `forDart`: the `ignore:` test comes first — an `ignore_for_file`
            // whose text contains `ignore:` further on becomes a line ignore.
            if lexeme.contains("ignore:") {
                let line0 = starts.partition_point(|&p| p <= offset) - 1;
                let before = &text[starts[line0]..offset];
                // Alone on the line: applies to the next one; after code, to
                // its own.
                let target = if before.trim().is_empty() { line0 + 2 } else { line0 + 1 };
                line_raw.push((target, elements));
            } else if lexeme.contains("ignore_for_file:") {
                file_raw.extend(elements);
            }
        }
        // `IgnoreValidator`: a name repeated within the file comment itself,
        // and then a line name that is repeated or already covered by the
        // file. The report is on the repeated name.
        let name_of = |e: &Ignored| match e {
            Ignored::Name { name, offset } => Some((name.clone(), *offset)),
            Ignored::Type { .. } => None,
        };
        let mut seen_in_file: BTreeSet<String> = BTreeSet::new();
        for e in &file_raw {
            if let Some((name, offset)) = name_of(e) {
                if !seen_in_file.insert(name.clone()) {
                    ig.repeated.push((name.clone(), dartforge_diagnostics::Span { start: offset, end: offset + name.len() }));
                }
            }
        }
        for (_, elements) in &line_raw {
            let mut seen_in_line: BTreeSet<String> = BTreeSet::new();
            for e in elements {
                if let Some((name, offset)) = name_of(e) {
                    if seen_in_file.contains(&name) || !seen_in_line.insert(name.clone()) {
                        ig.repeated.push((name.clone(), dartforge_diagnostics::Span { start: offset, end: offset + name.len() }));
                    }
                }
            }
        }
        ig.file = file_raw;
        for (line, elements) in line_raw {
            ig.by_line.entry(line).or_default().extend(elements);
        }
        ig
    }

    /// The file's `duplicate_ignore`s (`IgnoreValidator`: in 3.6.2 it is its
    /// only report).
    pub fn duplicates(&self) -> Vec<Diagnostic> {
        self.repeated
            .iter()
            .map(|(name, span)| Diagnostic::with_code(dartforge_diagnostics::codes::warning::DUPLICATE_IGNORE, *span, [name.as_str()]))
            .collect()
    }

    /// Is the diagnostic on `line` ignored by a comment?
    ///
    /// Any severity: the `LibraryAnalyzer._filterIgnoredErrors` of analyzer
    /// 6.11.0 does not consult `ErrorCode.isIgnorable` (which rejects
    /// `ERROR`); only the options' `cannot-ignore` prevents it
    /// ([`AnalysisOptions::is_ignorable`]). Checked against `dart analyze` 3.6.2 and
    /// 3.13.4: `// ignore_for_file: uri_has_not_been_generated` silences the
    /// error.
    ///
    /// A name matches the code's emitted name or the unique name without the
    /// class prefix (`IgnoredDiagnosticName.matches`); `type=` only knows
    /// `hint`, `lint` and `warning` (the latter being `STATIC_WARNING`).
    pub fn ignores(&self, d: &Diagnostic, line: usize) -> bool {
        let Some(c) = d.code else { return false };
        let info = c.info();
        // The catalog's 3.13.4 shapes carry the `_3_13` suffix only to tell
        // the entry apart; the analyzer's unique name does not have it.
        let unique = info
            .unique
            .split_once('.')
            .map_or(String::new(), |(_, r)| r.strip_suffix("_3_13").unwrap_or(r).to_lowercase());
        let code_kind = match info.error_type {
            dartforge_diagnostics::ErrorType::Hint => "hint",
            dartforge_diagnostics::ErrorType::Lint => "lint",
            dartforge_diagnostics::ErrorType::StaticWarning => "warning",
            _ => "",
        };
        let matches = |list: &Vec<Ignored>| {
            list.iter().any(|e| match e {
                Ignored::Name { name, .. } => name == info.name || *name == unique,
                Ignored::Type { kind } => !code_kind.is_empty() && kind == code_kind,
            })
        };
        matches(&self.file) || self.by_line.get(&line).is_some_and(matches)
    }

    /// Does the `ignore` comment silence the lint named `name` (the rule's)
    /// with unique name `unique` on line `line`? `type=lint` also applies.
    pub fn ignores_lint(&self, name: &str, unique: &str, line: usize) -> bool {
        let matches = |list: &Vec<Ignored>| {
            list.iter().any(|e| match e {
                Ignored::Name { name: n, .. } => n == name || n == unique,
                Ignored::Type { kind } => kind == "lint",
            })
        };
        matches(&self.file) || self.by_line.get(&line).is_some_and(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sali_options() {
        let o = AnalysisOptions::from_text(
            "analyzer:\n  exclude:\n    - build/**\n    - 'lib/**.g.dart'\n  errors:\n    uri_has_not_been_generated: ignore\n    deprecated_member_use: ignore # x\n    dead_code: info\n",
        );
        assert_eq!(o.exclude, vec!["build/**", "build", "lib/**.g.dart"]);
        assert_eq!(o.errors.get("uri_has_not_been_generated"), Some(&None));
        assert_eq!(o.errors.get("dead_code"), Some(&Some(Severity::Info)));
        assert!(o.is_excluded("build/x/y.dart"));
        assert!(o.is_excluded("lib/a/b.g.dart"));
        assert!(!o.is_excluded("lib/a/b.dart"));
    }

    /// Real YAML: flow style, quotes, comments, `language:`, lints in both
    /// forms, and what `errors:` accepts.
    #[test]
    fn real_yaml() {
        let o = AnalysisOptions::from_text(
            "analyzer: {exclude: ['gen/**', \"x # y.dart\"], language: {strict-casts: true}}
linter:
  rules:
    - a_regra
",
        );
        assert_eq!(o.exclude, vec!["gen/**", "gen", "x # y.dart"]);
        assert!(o.strict_casts && !o.strict_inference);
        assert_eq!(o.rules.get("a_regra"), Some(&true));
        let map = AnalysisOptions::from_text("linter:
  rules:
    a_regra: false
");
        assert_eq!(map.rules.get("a_regra"), Some(&false));
        // Invalid YAML leaves the default options.
        assert!(AnalysisOptions::from_text("analyzer: [
").exclude.is_empty());
        // Hidden folders and files are left out.
        assert!(AnalysisOptions::default().is_excluded(".dart_tool/x.dart"));
        assert!(AnalysisOptions::default().is_excluded("lib/.oculto.dart"));
    }

    /// `Merger`: the including file wins; lists concatenate without repeats;
    /// a list of lints against a map of booleans becomes a map.
    #[test]
    fn include_merge() {
        let base = map_from_text("analyzer:
  exclude: [a, b]
  errors:
    dead_code: info
    todo: ignore
linter:
  rules: [r1, r2]
");
        let top = map_from_text("analyzer:
  exclude: [b, c]
  errors:
    dead_code: error
linter:
  rules:
    r2: false
");
        let o = AnalysisOptions::from_map(&merge(base, top));
        assert_eq!(o.exclude, vec!["a", "b", "c"]);
        assert_eq!(o.errors.get("dead_code"), Some(&Some(Severity::Error)));
        assert_eq!(o.errors.get("todo"), Some(&None));
        assert_eq!(o.rules.get("r1"), Some(&true));
        assert_eq!(o.rules.get("r2"), Some(&false));
    }

    /// `// ignore:` and `// ignore_for_file:` apply to errors too
    /// (`v01/min/ignora`: `dart analyze` 3.6.2 and 3.13.4 give "No issues
    /// found!"), except what `cannot-ignore` lists.
    #[test]
    fn ignore_applies_to_errors_except_cannot_ignore() {
        use dartforge_diagnostics::codes::compile_time_error as c;
        let d = Diagnostic::with_code(c::URI_HAS_NOT_BEEN_GENERATED, dartforge_diagnostics::Span { start: 0, end: 1 }, ["a.template.dart"]);
        let file = IgnoreComments::from_text("// ignore_for_file: uri_has_not_been_generated\nimport 'a.template.dart';\n");
        assert!(file.ignores(&d, 2));
        let line = IgnoreComments::from_text("// ignore: uri_has_not_been_generated\nimport 'a.template.dart';\n");
        assert!(line.ignores(&d, 2));
        assert!(!line.ignores(&d, 3));
        assert!(AnalysisOptions::default().is_ignorable(&d));
        let by_name = AnalysisOptions::from_text("analyzer:\n  cannot-ignore:\n    - uri_has_not_been_generated\n");
        assert!(!by_name.is_ignorable(&d));
        let by_severity = AnalysisOptions::from_text("analyzer:\n  cannot-ignore:\n    - error\n");
        assert!(!by_severity.is_ignorable(&d));
        // The downgraded one stays unignorable through the default severity
        // and also becomes so through the new one (checked on the 3.6.2
        // binary, §4.3 c).
        let downgraded = AnalysisOptions::from_text("analyzer:\n  errors:\n    uri_has_not_been_generated: warning\n  cannot-ignore:\n    - error\n");
        assert!(!downgraded.is_ignorable(&d));
        let by_new = AnalysisOptions::from_text("analyzer:\n  errors:\n    uri_has_not_been_generated: warning\n  cannot-ignore:\n    - warning\n");
        assert!(!by_new.is_ignorable(&d));
        let other = AnalysisOptions::from_text("analyzer:\n  cannot-ignore:\n    - info\n");
        assert!(other.is_ignorable(&d));
        // Lints, only by name.
        let lint = AnalysisOptions::from_text("analyzer:\n  cannot-ignore:\n    - avoid_print\n    - info\n");
        assert!(!lint.lint_ignorable("avoid_print", "avoid_print"));
        assert!(lint.lint_ignorable("empty_catches", "empty_catches"));
    }

    /// The table checked on the 3.6.2 binary (docs/ANALYZER-ESPECIFICACAO-INFRA.md
    /// §4.1): each comment and whether it silences the `unused_local_variable`
    /// on line 2.
    #[test]
    fn oracle_table() {
        use dartforge_diagnostics::codes::warning as w;
        let d = Diagnostic::with_code(w::UNUSED_LOCAL_VARIABLE, dartforge_diagnostics::Span { start: 0, end: 1 }, ["x"]);
        let silences = |text: &str, line: usize| IgnoreComments::from_text(text).ignores(&d, line);
        assert!(silences("// ignore: type=warning
var x = 1;
", 2));
        assert!(!silences("// ignore: type=static_warning
var x = 1;
", 2));
        assert!(silences("///ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!silences("//	ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!silences("// ignore_for_file: dead_code // ignore: unused_local_variable
var x = 1;
", 2));
        assert!(!silences("var f = 1; /* ignore: unused_local_variable */
", 1));
        assert!(silences("var s = 'http://x'; // ignore: unused_local_variable
", 1));
        assert!(!silences("// ignore: unused_local_variable porque sim

var h = 1;
", 3));
        assert!(!silences("// ignore: unused_local_variable.
var x = 1;
", 2));
        assert!(silences("// ignore: UNUSED_LOCAL_VARIABLE , dead_code
var x = 1;
", 2));
    }

    #[test]
    fn repeated_names() {
        // 0         1         2         3         4
        // 01234567890123456789012345678901234567890123456789
        // // ignore: dead_code, dead_code
        let ig = IgnoreComments::from_text("// ignore: dead_code, dead_code
var x = 1;
");
        let ds = ig.duplicates();
        assert_eq!(ds.len(), 1);
        assert_eq!((ds[0].span.start, ds[0].span.end), (22, 31));
        let ig = IgnoreComments::from_text("// ignore_for_file: dead_code
// ignore: dead_code
var x = 1;
");
        assert_eq!(ig.duplicates().len(), 1);
        assert!(IgnoreComments::from_text("// ignore: a, dead_code
var x = 1;
").duplicates().is_empty());
    }

    #[test]
    fn basic_glob() {
        assert!(glob("*.dart", "a.dart"));
        // Braces, character classes and escapes (`package:glob`).
        assert!(glob("lib/{a,b}/*.dart", "lib/b/x.dart"));
        assert!(!glob("lib/{a,b}/*.dart", "lib/c/x.dart"));
        assert!(glob("lib/[a-c]?.dart", "lib/bz.dart"));
        assert!(!glob("lib/[!a-c]?.dart", "lib/bz.dart"));
        assert!(glob("a\\*b", "a*b"));
        assert!(!glob("a\\*b", "axb"));
        assert!(!glob("*.dart", "x/a.dart"));
        assert!(glob("**/*.dart", "x/y/a.dart"));
        assert!(glob("**", "x/y"));
    }
}
