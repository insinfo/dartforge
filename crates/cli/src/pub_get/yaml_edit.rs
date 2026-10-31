//! Text edits of `pubspec.yaml` that keep its layout, as `package:yaml_edit`
//! makes them for Pub: a new key goes in alphabetical position when the
//! map is sorted and last otherwise, a removed key takes its lines with it,
//! and an emptied section disappears.
//!
//! Only what Pub edits is supported: second-level entries of a block or
//! flow top-level section.

/// A value to write.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Scalar(String),
    Map(Vec<(String, Value)>),
}

/// `yaml_edit`'s scalar style: plain when YAML reads it back as the same
/// string, double-quoted otherwise.
pub(crate) fn scalar(text: &str) -> String {
    let first = text.chars().next();
    let dangerous = text.is_empty()
        || first.is_some_and(|c| "-?:,[]{}#&*!|>'\"%@`".contains(c) || c.is_whitespace())
        || text.ends_with(char::is_whitespace)
        || text.contains(": ")
        || text.contains(" #")
        || text.ends_with(':')
        || text.chars().any(|c| c.is_control())
        || matches!(
            text.to_ascii_lowercase().as_str(),
            "null"
                | "~"
                | "true"
                | "false"
                | "yes"
                | "no"
                | "on"
                | "off"
                | ".nan"
                | ".inf"
                | "-.inf"
        )
        || text.parse::<f64>().is_ok()
        || text.parse::<i64>().is_ok()
        || (text.starts_with("0x") && i64::from_str_radix(&text[2..], 16).is_ok())
        || (text.starts_with("0o") && i64::from_str_radix(&text[2..], 8).is_ok());
    if dangerous {
        serde_json::to_string(text).expect("string serialization")
    } else {
        text.to_owned()
    }
}

fn render_flow(value: &Value) -> String {
    match value {
        Value::Scalar(text) => scalar(text),
        Value::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{}: {}", scalar(k), render_flow(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The lines of `key: value` at `indent` in block style.
fn render_entry(key: &str, value: &Value, indent: usize, step: usize) -> Vec<String> {
    let pad = " ".repeat(indent);
    match value {
        Value::Scalar(text) => vec![format!("{pad}{}: {}", scalar(key), scalar(text))],
        Value::Map(entries) if entries.is_empty() => vec![format!("{pad}{}: {{}}", scalar(key))],
        Value::Map(entries) => {
            let mut lines = vec![format!("{pad}{}:", scalar(key))];
            for (k, v) in entries {
                lines.extend(render_entry(k, v, indent + step, step));
            }
            lines
        }
    }
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank_or_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.is_empty() || trimmed.starts_with('#')
}

/// The key of a mapping line (`key:` or `key: value`), unquoted.
fn line_key(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') || trimmed.starts_with('-') {
        return None;
    }
    let (key, rest) = if let Some(quoted) = trimmed.strip_prefix('"') {
        let end = quoted.find('"')?;
        (quoted[..end].to_owned(), &quoted[end + 1..])
    } else if let Some(quoted) = trimmed.strip_prefix('\'') {
        let end = quoted.find('\'')?;
        (quoted[..end].to_owned(), &quoted[end + 1..])
    } else {
        let colon = trimmed.find(':')?;
        (trimmed[..colon].trim_end().to_owned(), &trimmed[colon..])
    };
    let rest = rest.trim_start();
    let after = rest.strip_prefix(':')?;
    if !(after.is_empty() || after.starts_with(' ') || after.starts_with('\t')) {
        return None;
    }
    Some(key)
}

/// The text after `key:` on a mapping line, without a trailing comment.
fn inline_value(line: &str) -> (usize, usize) {
    let colon = line.find(':').expect("a mapping line");
    let start = colon + 1 + (line[colon + 1..].len() - line[colon + 1..].trim_start().len());
    let mut end = line.len();
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    for index in start..bytes.len() {
        match (quote, bytes[index]) {
            (None, b'"') | (None, b'\'') => quote = Some(bytes[index]),
            (Some(q), b) if b == q => quote = None,
            (None, b'#') if index > 0 && bytes[index - 1] == b' ' => {
                end = index;
                break;
            }
            _ => {}
        }
    }
    let value_end = start + line[start..end].trim_end().len();
    (start, value_end.max(start))
}

struct Section {
    /// Line of the top-level key.
    key_line: usize,
    /// One past the last line of the section's content (before trailing
    /// blank lines and comments).
    end: usize,
    /// Entries: name, first line, one past last line.
    entries: Vec<(String, usize, usize)>,
    indent: Option<usize>,
}

pub(crate) struct Editor {
    lines: Vec<String>,
    trailing_newline: bool,
}

impl Editor {
    pub fn new(text: &str) -> Self {
        let trailing_newline = text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        Self {
            lines: if text.is_empty() {
                Vec::new()
            } else {
                body.split('\n').map(str::to_owned).collect()
            },
            trailing_newline,
        }
    }

    pub fn text(&self) -> String {
        let mut text = self.lines.join("\n");
        if self.trailing_newline || !self.lines.is_empty() {
            text.push('\n');
        }
        text
    }

    fn section(&self, name: &str) -> Option<Section> {
        let key_line = self
            .lines
            .iter()
            .position(|l| indentation(l) == 0 && line_key(l).as_deref() == Some(name))?;
        let mut end = key_line + 1;
        let mut last_content = key_line + 1;
        while end < self.lines.len() {
            let line = &self.lines[end];
            if !line.trim().is_empty() && indentation(line) == 0 {
                break;
            }
            if !is_blank_or_comment(line) {
                last_content = end + 1;
            }
            end += 1;
        }
        let mut entries: Vec<(String, usize, usize)> = Vec::new();
        let mut indent = None;
        for index in key_line + 1..last_content {
            let line = &self.lines[index];
            if is_blank_or_comment(line) {
                continue;
            }
            let current = indentation(line);
            let child = *indent.get_or_insert(current);
            if current == child
                && let Some(key) = line_key(line)
            {
                entries.push((key, index, index + 1));
            } else if let Some(last) = entries.last_mut() {
                last.2 = index + 1;
            }
        }
        Some(Section {
            key_line,
            end: last_content,
            entries,
            indent,
        })
    }

    /// The flow map text of a section written `key: {...}`, if it is one.
    fn flow(&self, line: usize) -> Option<(usize, usize)> {
        let (start, end) = inline_value(&self.lines[line]);
        let value = &self.lines[line][start..end];
        (value.starts_with('{') && value.ends_with('}')).then_some((start, end))
    }

    /// Sets `section.name` to `value`, creating the section if needed.
    pub fn set(&mut self, section: &str, name: &str, value: &Value) {
        let Some(found) = self.section(section) else {
            // A new top-level key goes last (pubspec keys are not sorted).
            while self.lines.last().is_some_and(|l| l.trim().is_empty()) && !self.trailing_newline {
                self.lines.pop();
            }
            self.lines.push(format!("{section}:"));
            self.lines.extend(render_entry(name, value, 2, 2));
            self.trailing_newline = true;
            return;
        };
        if let Some((start, end)) = self.flow(found.key_line) {
            let text = self.lines[found.key_line][start + 1..end - 1]
                .trim()
                .to_owned();
            let mut entries: Vec<(String, String)> = Vec::new();
            for part in split_flow(&text) {
                if let Some((k, v)) = part.split_once(':') {
                    entries.push((k.trim().to_owned(), v.trim().to_owned()));
                }
            }
            match entries.iter_mut().find(|(k, _)| k == name) {
                Some(entry) => entry.1 = render_flow(value),
                None => entries.push((name.to_owned(), render_flow(value))),
            }
            let rendered = format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            self.lines[found.key_line].replace_range(start..end, &rendered);
            return;
        }
        let indent = found.indent.unwrap_or(2);
        if let Some((_, first, last)) = found.entries.iter().find(|(k, _, _)| k == name).cloned() {
            if let Value::Scalar(text) = value
                && last == first + 1
                && line_key(&self.lines[first]).is_some()
            {
                let (start, end) = inline_value(&self.lines[first]);
                if start < end {
                    self.lines[first].replace_range(start..end, &scalar(text));
                    return;
                }
            }
            let rendered = render_entry(name, value, indent, 2);
            self.lines.splice(first..last, rendered);
            return;
        }
        if found.entries.is_empty() {
            // `section:` with a null value becomes a block map.
            let (start, end) = inline_value(&self.lines[found.key_line]);
            if start < end {
                self.lines[found.key_line].truncate(start);
                let trimmed = self.lines[found.key_line].trim_end().to_owned();
                self.lines[found.key_line] = trimmed;
            }
            let rendered = render_entry(name, value, indent, 2);
            let at = found.key_line + 1;
            self.lines.splice(at..at, rendered);
            return;
        }
        // A single key does not make a map sorted.
        let sorted = found.entries.len() > 1 && found.entries.windows(2).all(|w| w[0].0 <= w[1].0);
        let at = if sorted {
            found
                .entries
                .iter()
                .find(|(k, _, _)| k.as_str() > name)
                .map(|(_, first, _)| *first)
                .unwrap_or(found.entries.last().expect("not empty").2)
        } else {
            found.entries.last().expect("not empty").2
        };
        let rendered = render_entry(name, value, indent, 2);
        self.lines.splice(at..at, rendered);
        let _ = found.end;
    }

    /// Whether `section` has an entry `name`.
    pub fn contains(&self, section: &str, name: &str) -> bool {
        let Some(found) = self.section(section) else {
            return false;
        };
        if let Some((start, end)) = self.flow(found.key_line) {
            let text = &self.lines[found.key_line][start + 1..end - 1];
            return split_flow(text)
                .iter()
                .any(|part| part.split_once(':').is_some_and(|(k, _)| k.trim() == name));
        }
        found.entries.iter().any(|(k, _, _)| k == name)
    }

    /// Removes `section.name`; an emptied section is removed too.
    pub fn remove(&mut self, section: &str, name: &str) {
        let Some(found) = self.section(section) else {
            return;
        };
        if let Some((start, end)) = self.flow(found.key_line) {
            let text = self.lines[found.key_line][start + 1..end - 1].to_owned();
            let parts: Vec<String> = split_flow(&text)
                .into_iter()
                .filter(|part| part.split_once(':').is_none_or(|(k, _)| k.trim() != name))
                .map(|p| p.trim().to_owned())
                .collect();
            if parts.is_empty() {
                self.lines.remove(found.key_line);
            } else {
                let rendered = format!("{{{}}}", parts.join(", "));
                self.lines[found.key_line].replace_range(start..end, &rendered);
            }
            return;
        }
        let Some((_, first, last)) = found.entries.iter().find(|(k, _, _)| k == name).cloned()
        else {
            return;
        };
        if found.entries.len() == 1 {
            self.lines.drain(found.key_line..last.max(found.end));
        } else {
            self.lines.drain(first..last);
        }
    }
}

/// Splits the inside of a flow map at top-level commas.
fn split_flow(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                current.push(c);
            }
            (Some(q), _) if c == q => {
                quote = None;
                current.push(c);
            }
            (None, '{' | '[') => {
                depth += 1;
                current.push(c);
            }
            (None, '}' | ']') => {
                depth -= 1;
                current.push(c);
            }
            (None, ',') if depth == 0 => {
                if !current.trim().is_empty() {
                    parts.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(text: &str) -> Value {
        Value::Scalar(text.into())
    }

    #[test]
    fn edits_follow_yaml_edit() {
        let mut editor = Editor::new(
            "name: app\nenvironment:\n  sdk: ^3.5.0\n\ndependencies:\n  args: ^2.0.0\n  path: ^1.8.0 # comment\n\ndev_dependencies:\n  lints: ^5.0.0\n",
        );
        editor.set("dependencies", "meta", &s("^1.19.0"));
        editor.set("dependencies", "collection", &s(">=1.17.0 <2.0.0"));
        editor.set("dev_dependencies", "test_api", &s("^0.7.7"));
        editor.set("dependencies", "path", &s("^1.9.0"));
        assert_eq!(
            editor.text(),
            "name: app\nenvironment:\n  sdk: ^3.5.0\n\ndependencies:\n  args: ^2.0.0\n  collection: \">=1.17.0 <2.0.0\"\n  meta: ^1.19.0\n  path: ^1.9.0 # comment\n\ndev_dependencies:\n  lints: ^5.0.0\n  test_api: ^0.7.7\n"
        );
        let mut unsorted =
            Editor::new("name: app\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n");
        unsorted.set(
            "dependencies",
            "pdfbox_dart",
            &Value::Map(vec![(
                "git".into(),
                s("https://github.com/insinfo/pdfbox_dart"),
            )]),
        );
        assert_eq!(
            unsorted.text(),
            "name: app\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n  pdfbox_dart:\n    git: https://github.com/insinfo/pdfbox_dart\n"
        );
        let mut none = Editor::new("name: app\nenvironment:\n  sdk: ^3.5.0\n");
        none.set("dependencies", "path", &s("^1.9.1"));
        assert_eq!(
            none.text(),
            "name: app\nenvironment:\n  sdk: ^3.5.0\ndependencies:\n  path: ^1.9.1\n"
        );
        let mut empty = Editor::new("name: app\ndependencies:\n");
        empty.set("dependencies", "path", &s("^1.9.1"));
        assert_eq!(empty.text(), "name: app\ndependencies:\n  path: ^1.9.1\n");
        let mut flow = Editor::new("name: app\ndependencies: {}\n");
        flow.set("dependencies", "path", &s("^1.9.1"));
        assert_eq!(flow.text(), "name: app\ndependencies: {path: ^1.9.1}\n");
        let mut removal = Editor::new(
            "name: app\ndependencies:\n  path: ^1.8.0\n  args: ^2.0.0\n\ndev_dependencies:\n  lints: ^5.0.0\n",
        );
        removal.remove("dependencies", "args");
        removal.remove("dev_dependencies", "lints");
        assert_eq!(
            removal.text(),
            "name: app\ndependencies:\n  path: ^1.8.0\n\n"
        );
        assert_eq!(scalar("1.8.0"), "1.8.0");
        assert_eq!(scalar("1.0"), "\"1.0\"");
        assert_eq!(scalar("any"), "any");
    }
}
