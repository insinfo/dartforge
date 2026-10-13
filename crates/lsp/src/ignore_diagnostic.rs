//! The three Dart 3.6.2 "ignore" fixes (`COR:ignore_diagnostic.dart`;
//! docs/LSP-ESPECIFICACAO.md §13.7.1 item 6): `Ignore 'x' for this line`,
//! `Ignore 'x' for the whole file` and ``Ignore 'x' in `analysis_options.yaml` ``,
//! the last one with `YamlEditor.update` from `yaml_edit` 2.2.3
//! (`editor.dart`, `map_mutations.dart`, `strings.dart`, `utils.dart`)
//! ported along the paths the fix uses.
//! Written without compiling or running (2026-10-05).

use crate::actions::CodeAction;
use crate::Edit;
use dartforge_analysis::non_dart::yaml::{self, Node, Value};
use dartforge_diagnostics::{Diagnostic, Span};
use std::path::{Path, PathBuf};
use url::Url;

/// `LineInfo.fromContent`: the line starts (`\n`, `\r\n` and `\r`).
fn line_starts(t: &str) -> Vec<usize> {
    let b = t.as_bytes();
    let mut v = vec![0];
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\r' if b.get(i + 1) == Some(&b'\n') => {
                v.push(i + 2);
                i += 2;
                continue;
            }
            b'\r' | b'\n' => v.push(i + 1),
            _ => {}
        }
        i += 1;
    }
    v
}

/// `IgnoreInfo.ignoreMatcher` (`//+[ ]*ignore:`) at the start of the text.
fn starts_with_ignore(line: &str) -> bool {
    let slashes = line.len() - line.trim_start_matches('/').len();
    if slashes < 2 {
        return false;
    }
    line[slashes..].trim_start_matches(' ').starts_with("ignore:")
}

/// The options that apply to the file and the path of the
/// `analysis_options.yaml` (`analysisOptions.file`), when it exists.
pub(crate) fn file_options(path: &Path) -> (dartforge_parity::filtros::Opcoes, Option<PathBuf>) {
    let root = crate::project::project_root(path);
    let file = dartforge_parity::filtros::Opcoes::de_subpasta(path, &root).unwrap_or_else(|| root.join("analysis_options.yaml"));
    let exists = file.is_file();
    (dartforge_parity::filtros::Opcoes::ler_arquivo(&file), exists.then_some(file))
}

/// The three fixes of each `HintCode`/`WarningCode` diagnostic on lines
/// `start..end`, one set per diagnostic (deduplication by title is the
/// sorter's job).
pub(crate) fn ignore(uri: &str, text: &str, diagnostics: &[Diagnostic], start_: usize, end: usize) -> Vec<CodeAction> {
    let path = crate::project::uri_file(uri);
    let (options, options_file) = match path.as_deref() {
        Some(c) => file_options(c),
        None => (dartforge_parity::filtros::Opcoes::default(), None),
    };
    let tx = crate::refactorings_exec::Text::new(text);
    let eol = tx.eol();
    let lines = line_starts(text);
    let line_of = |o: usize| lines.partition_point(|&s| s <= o).saturating_sub(1);
    let mut output = Vec::new();
    for d in diagnostics {
        if !(d.span.start <= end && start_ <= d.span.end) {
            continue;
        }
        let Some(c) = d.code else { continue };
        let info = c.info();
        // Only `LintCode`, `HintCode` and `WarningCode`.
        if !(info.unique.starts_with("HintCode.") || info.unique.starts_with("WarningCode.") || info.unique.starts_with("LintCode.")) {
            continue;
        }
        // `_isCodeUnignorable`.
        if !options.ignoravel(d) || matches!(options.errors.get(info.name), Some(None)) {
            continue;
        }
        let code = info.name;
        let insert_in = |o: usize, before: bool, after: bool, prefix: &str| -> (Span, String) {
            let indent = tx.line_prefix(o);
            let mut s = String::new();
            if before {
                s.push_str(eol);
            }
            s.push_str(indent);
            s.push_str(&format!("// {prefix}: {code}"));
            s.push_str(eol);
            if after {
                s.push_str(eol);
            }
            (Span { start: o, end: o }, s)
        };
        // `IgnoreDiagnosticOnLine`.
        let n = line_of(d.span.start.min(text.len()));
        let edit = if n == 0 {
            insert_in(0, false, false, "ignore")
        } else {
            let previous = lines[n - 1];
            let start = lines[n];
            if starts_with_ignore(text[previous..start].trim()) {
                let o = start - eol.len();
                (Span { start: o, end: o }, format!(", {code}"))
            } else {
                insert_in(start, false, false, "ignore")
            }
        };
        output.push(action(uri, format!("Ignore '{code}' for this line"), "quickfix.ignore.line", vec![edit], d));
        // `IgnoreDiagnosticInFile`.
        let total = lines.len();
        let edit = if total == 1 {
            insert_in(0, false, true, "ignore_for_file")
        } else {
            let mut blank = None;
            let mut start = 0;
            let mut existing = None;
            for k in 0..total - 1 {
                start = lines[k];
                let next = lines[k + 1];
                let l = text[start..next].trim();
                if l.starts_with("// ignore_for_file:") {
                    existing = Some(next - eol.len());
                    break;
                }
                if l.is_empty() {
                    blank = Some(start);
                    continue;
                }
                if l.starts_with("#!") || l.starts_with("//") {
                    continue;
                }
                break;
            }
            match (existing, blank) {
                (Some(o), _) => (Span { start: o, end: o }, format!(", {code}")),
                (None, Some(o)) => insert_in(o, true, false, "ignore_for_file"),
                (None, None) => insert_in(start, false, true, "ignore_for_file"),
            }
        };
        output.push(action(uri, format!("Ignore '{code}' for the whole file"), "quickfix.ignore.file", vec![edit], d));
        // `IgnoreDiagnosticInAnalysisOptionsFile`.
        if let Some(file) = &options_file
            && let Ok(content) = std::fs::read_to_string(file)
            && let Some((offset, new_text)) = ignore_in_yaml(&content, code)
            && let Ok(uri_yaml) = Url::from_file_path(file)
        {
            output.push(CodeAction {
                title: format!("Ignore '{code}' in `analysis_options.yaml`"),
                kind: "quickfix.ignore.analysis".into(),
                edits: vec![Edit { uri: uri_yaml.to_string(), span: Span { start: offset, end: offset }, text: new_text }],
                diagnostic: Some(d.clone()),
                create_file: None, linked: Default::default(),
            });
        }
    }
    output
}

fn action(uri: &str, title: String, kind: &str, edits: Vec<(Span, String)>, d: &Diagnostic) -> CodeAction {
    CodeAction {
        title,
        kind: kind.into(),
        edits: edits.into_iter().map(|(span, text)| Edit { uri: uri.to_string(), span, text }).collect(),
        diagnostic: Some(d.clone()),
        create_file: None, linked: Default::default(),
    }
}

// -- yaml_edit ------------------------------------------------------------------

/// A new value: a simple scalar or a map (`wrapAsYamlNode` of a `Map`).
enum New {
    Scalar(String),
    Map(Vec<(String, New)>),
}

/// `getLineEnding`: `\r\n` if more than half of the line breaks use it.
fn line_end(t: &str) -> &'static str {
    let b = t.as_bytes();
    let (mut unix, mut windows) = (0, 0);
    for (i, &c) in b.iter().enumerate() {
        if c == b'\n' {
            if i != 0 && b[i - 1] == b'\r' {
                windows += 1;
            } else {
                unix += 1;
            }
        }
    }
    if windows > unix { "\r\n" } else { "\n" }
}

/// `String.lastIndexOf(c, start)` (inclusive).
fn last_index(t: &str, c: u8, start: usize) -> Option<usize> {
    let b = t.as_bytes();
    let end = start.min(b.len().saturating_sub(1));
    if b.is_empty() {
        return None;
    }
    (0..=end).rev().find(|&i| b[i] == c)
}

fn in_flow(t: &str, node: &Node) -> bool {
    t[node.span.start.min(t.len())..].starts_with(['{', '['])
}

/// `getContentSensitiveEnd`.
fn sensitive_end(t: &str, node: &Node) -> usize {
    match &node.value {
        Value::List(l) if !in_flow(t, node) => l.last().map_or(node.span.end, |x| sensitive_end(t, x)),
        Value::Map(m) if !in_flow(t, node) => m.last().map_or(node.span.end, |(_, v)| sensitive_end(t, v)),
        _ => node.span.end,
    }
}

/// `getMapIndentation` (`None`: the empty block map, `UnsupportedError`).
fn map_indent(t: &str, map: &Node) -> Option<usize> {
    if in_flow(t, map) {
        return Some(0);
    }
    let (last, _) = map.map()?.last()?;
    let o = last.span.start;
    let nl = last_index(t, b'\n', o);
    let q = last_index(t, b'?', o);
    Some(match (q, nl) {
        (None, None) => o,
        (None, Some(n)) => o - n - 1,
        (Some(q), None) => q,
        (Some(q), Some(n)) if q > n => q - n - 1,
        (Some(_), Some(n)) => o - n - 1,
    })
}

/// `getListIndentation`.
fn list_indent(t: &str, list: &Node) -> Option<usize> {
    if in_flow(t, list) {
        return Some(0);
    }
    let last = list.list()?.last()?;
    let hyphen = last_index(t, b'-', last.span.start.checked_sub(1)?)?;
    if hyphen == 0 {
        return Some(0);
    }
    let nl = hyphen.checked_sub(1).and_then(|h| last_index(t, b'\n', h));
    Some(match nl {
        Some(n) => hyphen - n - 1,
        None => hyphen,
    })
}

/// `getIndentation(editor)`: the indentation of the last non-null block
/// collection at the second level; 2 when there is none.
fn editor_indent(t: &str, root: &Node) -> usize {
    let mut indent = 2;
    let children: Vec<&Node> = match &root.value {
        Value::Map(m) if !in_flow(t, root) => m.iter().map(|(_, v)| v).collect(),
        Value::List(l) if !in_flow(t, root) => l.iter().collect(),
        _ => Vec::new(),
    };
    for f in children {
        let r = match &f.value {
            Value::List(_) => list_indent(t, f).unwrap_or(0),
            Value::Map(_) => map_indent(t, f).unwrap_or(0),
            _ => 0,
        };
        if r != 0 {
            indent = r;
        }
    }
    indent
}

/// The `toString` of a scalar key.
fn key_text(k: &Node) -> String {
    match &k.value {
        Value::Text(s) | Value::Other(s) => s.clone(),
        Value::Null => "null".to_string(),
        _ => String::new(),
    }
}

/// `getMapInsertionIndex`.
fn insertion_index(map: &[(Node, Node)], key: &str) -> usize {
    let keys: Vec<String> = map.iter().map(|(k, _)| key_text(k)).collect();
    for i in 1..keys.len() {
        if keys[i] < keys[i - 1] {
            return map.len();
        }
    }
    keys.iter().position(|k| k.as_str() > key).unwrap_or(map.len())
}

/// `yamlEncodeFlow`.
fn in_flow_text(v: &New) -> String {
    match v {
        New::Scalar(s) => s.clone(),
        New::Map(m) => format!("{{{}}}", m.iter().map(|(k, x)| format!("{k}: {}", in_flow_text(x))).collect::<Vec<_>>().join(", ")),
    }
}

/// `yamlEncodeBlock`.
fn in_block(v: &New, indent: usize, eol: &str) -> String {
    match v {
        New::Scalar(s) => s.clone(),
        New::Map(m) if m.is_empty() => format!("{}{{}}", " ".repeat(indent)),
        New::Map(m) => m
            .iter()
            .map(|(k, x)| {
                let key = format!("{}{k}", " ".repeat(indent));
                let value = in_block(x, indent + 2, eol);
                match x {
                    New::Map(n) if !n.is_empty() => format!("{key}:{eol}{value}"),
                    _ => format!("{key}: {value}"),
                }
            })
            .collect::<Vec<_>>()
            .join(eol),
    }
}

fn non_empty_collection(v: &New) -> bool {
    matches!(v, New::Map(m) if !m.is_empty())
}

/// `updateInMap(editor, map, key, value)`: the edit (offset, text).
fn update_in_map(t: &str, root: &Node, map: &Node, key: &str, value: &New) -> Option<(usize, String)> {
    let pairs = map.map()?;
    let existing = pairs.iter().rev().find(|(k, _)| key_text(k) == key);
    let flow = in_flow(t, map);
    let eol = line_end(t);
    match (existing, flow) {
        (None, true) => {
            // `_addToFlowMap`.
            let pair = format!("{key}: {}", in_flow_text(value));
            if pairs.is_empty() {
                return Some((map.span.end - 1, pair));
            }
            let i = insertion_index(pairs, key);
            if i == pairs.len() {
                return Some((map.span.end - 1, format!(", {pair}")));
            }
            Some((pairs[i].0.span.start, format!("{pair}, ")))
        }
        (None, false) => {
            // `_addToBlockMap`.
            let indent_map = map_indent(t, map)?;
            let new_indent = indent_map + editor_indent(t, root);
            let mut formatted = " ".repeat(indent_map);
            let mut offset = map.span.end;
            let i = insertion_index(pairs, key);
            if !pairs.is_empty() {
                if i == pairs.len() {
                    let end = sensitive_end(t, &pairs[pairs.len() - 1].1);
                    match t[end.min(t.len())..].find('\n') {
                        Some(k) => offset = end + k + 1,
                        None => formatted = format!("{eol}{formatted}"),
                    }
                } else {
                    offset = last_index(t, b'\n', pairs[i].0.span.start).map_or(0, |n| n + 1);
                }
            }
            let value_text = in_block(value, new_indent, eol);
            if non_empty_collection(value) {
                formatted.push_str(&format!("{key}:{eol}{value_text}{eol}"));
            } else {
                formatted.push_str(&format!("{key}: {value_text}{eol}"));
            }
            Some((offset, formatted))
        }
        (Some((_, v)), true) => {
            // `_replaceInFlowMap`.
            Some((v.span.start, in_flow_text(value)))
        }
        (Some((k, _)), false) => {
            // `_replaceInBlockMap`: from the end of the key + 1 (the `:`).
            let indent_map = map_indent(t, map)?;
            let new_indent = indent_map + editor_indent(t, root);
            let mut text = in_block(value, new_indent, eol);
            if non_empty_collection(value) {
                text = format!("{eol}{text}");
            }
            if !text.starts_with(eol) {
                text = format!(" {text}");
            }
            Some((k.span.end + 1, text))
        }
    }
}

/// The edit of `IgnoreDiagnosticInAnalysisOptionsFile` (the offset and the
/// `replacement`, emitted as an insertion); `None` for invalid YAML.
pub(crate) fn ignore_in_yaml(content: &str, code: &str) -> Option<(usize, String)> {
    let root = yaml::parse(content).ok()?;
    let errors = || New::Map(vec![(code.to_string(), New::Scalar("ignore".to_string()))]);
    let Some(r) = root.as_ref().filter(|r| r.map().is_some()) else {
        // `path = []`: the whole document (the insertion at its start).
        let eol = line_end(content);
        let start = root.as_ref().map_or(content.len(), |r| r.span.start);
        let value = New::Map(vec![("analyzer".to_string(), New::Map(vec![("errors".to_string(), errors())]))]);
        return Some((start, in_block(&value, 0, eol)));
    };
    let analyzer = r.field("analyzer").filter(|a| a.map().is_some() && a.par("errors").is_some());
    match analyzer {
        None => update_in_map(content, r, r, "analyzer", &New::Map(vec![("errors".to_string(), errors())])),
        Some(a) => {
            let e = a.field("errors")?;
            e.map()?;
            update_in_map(content, r, e, code, &New::Scalar("ignore".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ignore_in_yaml;

    #[test]
    fn ignore_yaml() {
        assert_eq!(ignore_in_yaml("", "x"), Some((0, "analyzer:\n  errors:\n    x: ignore".to_string())));
        assert_eq!(
            ignore_in_yaml("analyzer:\n  errors:\n    a: ignore\n    z: ignore\n", "m"),
            Some((34, "    m: ignore\n".to_string()))
        );
        assert_eq!(
            ignore_in_yaml("include: package:lints/recommended.yaml\n", "x"),
            Some((0, "analyzer:\n  errors:\n    x: ignore\n".to_string()))
        );
    }
}
