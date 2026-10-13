//! The `pubspec.yaml` lint rules (docs/ANALYZER-ESPECIFICACAO-INFRA.md §8),
//! written directly from the 3.6.2 emitters (`linter/lib/src/rules/pub/`),
//! over the analyzer's `Pubspec` model (`analyzer/lib/src/lint/pub.dart`):
//! `package_names`, `secure_pubspec_urls` and `sort_pub_dependencies`; and
//! what `depend_on_referenced_packages` reads from the package's
//! `pubspec.yaml`.
//!
//! `Pubspec.parseYaml` only reads a document that is a map; the keys are the
//! scalar ones, and the last occurrence wins. A node's text is the `toString`
//! of the scalar's value (numbers and booleans as written; null has none).
//! `IgnoreInfo.forYaml` (`# ignore:` and `# ignore_for_file:`) is in
//! [`YamlIgnores`], which also applies to the `PubspecValidator`.
//! Written without compiling or running (2026-10-05).

use super::codes_g as c;
use super::rules::LintReport;
use super::LintCode;
use crate::non_dart::yaml::{self, Node, Value};
use dartforge_diagnostics::Span;

/// `_PSNode.text`.
fn text(node: &Node) -> Option<String> {
    match &node.value {
        Value::Text(t) | Value::Other(t) => Some(t.clone()),
        _ => None,
    }
}

/// The text of a scalar key (`key.toString()`).
fn key_text(node: &Node) -> Option<String> {
    if node.is_scalar() { text(node).or_else(|| node.is_null().then(|| "null".to_string())) } else { None }
}

/// `_findEntry(map, key)`: the scalar value of the last equal key.
fn find_entry<'a>(map: &'a Node, name: &str) -> Option<&'a Node> {
    let mut found = None;
    for (k, v) in map.map()? {
        if key_text(k).as_deref() == Some(name) {
            found = Some(v).filter(|v| v.is_scalar());
        }
    }
    found
}

/// A dependency (`_PSDependency`).
pub struct Dependency<'a> {
    pub name: &'a Node,
    /// The `git` `url` and the `hosted` one.
    pub urls: Vec<&'a Node>,
}

/// The `pubspec.yaml` in the linter's model.
#[derive(Default)]
pub struct Pubspec<'a> {
    pub name: Option<&'a Node>,
    pub documentation: Option<&'a Node>,
    pub homepage: Option<&'a Node>,
    pub issue_tracker: Option<&'a Node>,
    pub repository: Option<&'a Node>,
    pub dependencies: Option<Vec<Dependency<'a>>>,
    pub dev_dependencies: Option<Vec<Dependency<'a>>>,
    pub dependency_overrides: Option<Vec<Dependency<'a>>>,
}

/// `_processDependencies`.
fn dependencies_of(v: &Node) -> Option<Vec<Dependency<'_>>> {
    let pairs = v.map()?;
    let mut list = Vec::new();
    for (k, value) in pairs {
        if !k.is_scalar() {
            continue;
        }
        let mut git: Option<&Node> = None;
        let mut hosted: Option<&Node> = None;
        if let Some(details) = value.map() {
            for (dk, dv) in details {
                match key_text(dk).as_deref() {
                    Some("hosted") => {
                        hosted = if dv.is_scalar() { Some(dv) } else if dv.map().is_some() { find_entry(dv, "url") } else { None };
                    }
                    Some("git") => {
                        git = if dv.is_scalar() { Some(dv) } else if dv.map().is_some() { find_entry(dv, "url") } else { None };
                    }
                    _ => {}
                }
            }
        }
        list.push(Dependency { name: k, urls: git.into_iter().chain(hosted).collect() });
    }
    Some(list)
}

/// `Pubspec.parseYaml`.
pub fn read(root: &Node) -> Pubspec<'_> {
    let mut p = Pubspec::default();
    let Some(pairs) = root.map() else { return p };
    for (k, v) in pairs {
        if !k.is_scalar() {
            continue;
        }
        let scalar = || Some(v).filter(|v| v.is_scalar());
        match key_text(k).as_deref() {
            Some("homepage") => p.homepage = scalar(),
            Some("repository") => p.repository = scalar(),
            Some("issue_tracker") => p.issue_tracker = scalar(),
            Some("name") => p.name = scalar(),
            Some("documentation") => p.documentation = scalar(),
            Some("dependencies") => p.dependencies = dependencies_of(v),
            Some("dev_dependencies") => p.dev_dependencies = dependencies_of(v),
            Some("dependency_overrides") => p.dependency_overrides = dependencies_of(v),
            _ => {}
        }
    }
    p
}

/// `^[_$a-z]+(\d[_a-z\d]*)?$` case-insensitively.
fn is_identifier(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b'_' || b[i] == b'$' || b[i].is_ascii_alphabetic()) {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    if i == b.len() {
        return true;
    }
    if !b[i].is_ascii_digit() {
        return false;
    }
    b[i + 1..].iter().all(|c| *c == b'_' || c.is_ascii_alphanumeric())
}

/// The reserved words of `Keyword` (`isReservedWord`).
const RESERVED: [&str; 33] = [
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else", "enum", "extends", "false", "final", "finally",
    "for", "if", "in", "is", "new", "null", "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

/// `isValidPackageName`: `^_*[a-z](?:_?[a-z\d])*$`, an identifier and not
/// reserved.
fn is_valid_package_name(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i] == b'_' {
        i += 1;
    }
    if i >= b.len() || !b[i].is_ascii_lowercase() {
        return false;
    }
    i += 1;
    while i < b.len() {
        if b[i] == b'_' {
            i += 1;
            if i >= b.len() {
                return false;
            }
        }
        if !(b[i].is_ascii_lowercase() || b[i].is_ascii_digit()) {
            return false;
        }
        i += 1;
    }
    is_identifier(s) && !RESERVED.contains(&s)
}

/// The scheme of `Uri.tryParse(text)`, lowercased.
fn scheme(t: &str) -> Option<String> {
    let end = t.find(|c: char| matches!(c, ':' | '/' | '?' | '#'))?;
    if t.as_bytes()[end] != b':' || end == 0 {
        return None;
    }
    let s = &t[..end];
    let b = s.as_bytes();
    if !b[0].is_ascii_alphabetic() || !b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')) {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

/// The `pubspec.yaml` lints for the text `source`.
pub fn run(source: &str, enabled: &dyn Fn(&str) -> bool) -> Vec<LintReport> {
    let mut out: Vec<LintReport> = Vec::new();
    let Ok(Some(root)) = yaml::parse(source) else { return out };
    let p = read(&root);
    let mut report = |code: &'static LintCode, span: Span, args: &[&str]| {
        out.push(LintReport { code, span, args: args.iter().map(|x| x.to_string()).collect() });
    };
    let lists = [&p.dependencies, &p.dev_dependencies, &p.dependency_overrides];

    // `package_names`.
    if enabled("package_names")
        && let Some(n) = p.name
        && let Some(t) = text(n)
        && !is_valid_package_name(&t)
    {
        report(&c::PACKAGE_NAMES, n.span, &[t.as_str()]);
    }

    // `secure_pubspec_urls`: in the order of `accept` (documentation,
    // homepage, issue_tracker, repository; then the dependencies).
    if enabled("secure_pubspec_urls") {
        let mut check = |node: &Node| {
            if let Some(t) = text(node)
                && let Some(s) = scheme(&t)
                && (s == "http" || s == "git")
            {
                report(&c::SECURE_PUBSPEC_URLS, node.span, &[s.as_str()]);
            }
        };
        for node in [p.documentation, p.homepage, p.issue_tracker, p.repository].into_iter().flatten() {
            check(node);
        }
        for list in lists.iter().copied().flatten() {
            for d in list {
                for u in &d.urls {
                    check(u);
                }
            }
        }
    }

    // `sort_pub_dependencies`: the first out-of-order one in each list.
    if enabled("sort_pub_dependencies") {
        for list in lists.iter().copied().flatten() {
            let mut by_position: Vec<&Dependency<'_>> = list.iter().collect();
            by_position.sort_by_key(|d| d.name.span.start);
            let mut previous: Vec<u16> = Vec::new();
            for d in by_position {
                let Some(t) = text(d.name) else { continue };
                let units: Vec<u16> = t.encode_utf16().collect();
                if units < previous {
                    report(&c::SORT_PUB_DEPENDENCIES, d.name.span, &[]);
                    break;
                }
                previous = units;
            }
        }
    }
    out
}

/// What `depend_on_referenced_packages` reads: the package name, the
/// dependencies and the dev dependencies.
pub fn available_dependencies(source: &str, with_dev: bool) -> Option<Vec<String>> {
    let root = yaml::parse(source).ok()??;
    let p = read(&root);
    let mut v = vec![text(p.name?)?];
    for d in p.dependencies.iter().flatten() {
        v.extend(text(d.name));
    }
    if with_dev {
        for d in p.dev_dependencies.iter().flatten() {
            v.extend(text(d.name));
        }
    }
    Some(v)
}

/// `IgnoreInfo.forYaml`: the names silenced per line (`# ignore:`) and in the
/// whole file (`# ignore_for_file:`), lowercased.
pub struct YamlIgnores {
    whole_file: Vec<String>,
    per_line: std::collections::HashMap<usize, Vec<String>>,
    line_starts: Vec<usize>,
}

/// `_trimmedCommaSeparatedMatcher` (`[^\s,]([^,]*[^\s,])?`).
fn split_names(t: &str) -> Vec<String> {
    t.split(',').map(str::trim).filter(|x| !x.is_empty()).map(str::to_lowercase).collect()
}

impl YamlIgnores {
    pub fn parse(source: &str) -> YamlIgnores {
        let mut line_starts = vec![0usize];
        let b = source.as_bytes();
        let mut i = 0;
        while i < b.len() {
            match b[i] {
                b'\r' => {
                    if b.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    line_starts.push(i + 1);
                }
                b'\n' => line_starts.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        let mut r = YamlIgnores { whole_file: Vec::new(), per_line: Default::default(), line_starts };
        // The lines as the `RegExp` `.` sees them (without `\n`, `\r`, U+2028
        // and U+2029).
        let lines: Vec<(usize, &str)> = {
            let mut v = Vec::new();
            let mut start = 0;
            for (k, ch) in source.char_indices() {
                if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                    v.push((start, &source[start..k]));
                    start = k + ch.len_utf8();
                }
            }
            v.push((start, &source[start..]));
            v
        };
        for &(start, line) in &lines {
            // `#[ ]*ignore_for_file:(?<ignored>.*)`: each occurrence.
            let mut from = 0;
            while let Some(k) = line[from..].find('#') {
                let pos = from + k;
                let rest = line[pos + 1..].trim_start_matches(' ');
                if let Some(names) = rest.strip_prefix("ignore_for_file:") {
                    r.whole_file.extend(split_names(names));
                    break;
                }
                from = pos + 1;
            }
            // `^(?<before>.*)#+[ ]*ignore:(?<ignored>.*)` (multiline): the
            // greedy `.*` takes the last occurrence of `#+ *ignore:`, and all
            // of its `#` but one.
            let mut last: Option<(usize, usize)> = None;
            for (k, _) in line.match_indices('#') {
                let rest = line[k + 1..].trim_start_matches(' ');
                if let Some(names) = rest.strip_prefix("ignore:") {
                    last = Some((k, line.len() - names.len()));
                }
            }
            if let Some((hash, after)) = last {
                // `before.trim().isEmpty`: the comment applies to the next
                // line.
                let line_no = line_of(&r.line_starts, start);
                let target = if line[..hash].trim().is_empty() { line_no + 1 } else { line_no };
                r.per_line.entry(target).or_default().extend(split_names(&line[after..]));
            }
        }
        r
    }

    /// Code `name` (unique name `unique`) at `pos` is silenced.
    pub fn ignores(&self, name: &str, unique: &str, pos: usize) -> bool {
        let matches = |x: &String| *x == name.to_lowercase() || *x == unique.to_lowercase();
        if self.whole_file.iter().any(matches) {
            return true;
        }
        self.per_line.get(&line_of(&self.line_starts, pos)).is_some_and(|v| v.iter().any(matches))
    }
}

fn line_of(line_starts: &[usize], pos: usize) -> usize {
    line_starts.partition_point(|&x| x <= pos)
}
