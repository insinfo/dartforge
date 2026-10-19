//! YAML with positions, for the non-Dart file validators
//! (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.2.4 item 1 and step 7): the
//! document tree with the range (source bytes) of each node, built over the
//! marked events of `yaml-rust2`.
//!
//! The event parser only gives each node's **start**. The end is computed
//! here: for a scalar, from its text in the source (matching quotes, a block
//! by indentation); for a flow list or map, by the `]` or `}`; in block style,
//! by the end of the last child (like `package:yaml`).
//! Written without compiling or running (2026-10-05).

use dartforge_diagnostics::Span;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

/// A node's value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Empty, `~` or `null`.
    Null,
    /// A string (a quoted, block, or plain scalar that is not a number,
    /// boolean or null).
    Text(String),
    /// A number or boolean, with the text as written.
    Other(String),
    List(Vec<Node>),
    /// The pairs in file order.
    Map(Vec<(Node, Node)>),
}

/// A node with its range in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub span: Span,
    pub value: Value,
}

impl Node {
    /// The text of a string scalar.
    pub fn text(&self) -> Option<&str> {
        match &self.value {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    pub fn map(&self) -> Option<&[(Node, Node)]> {
        match &self.value {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&[Node]> {
        match &self.value {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// A scalar (`YamlScalar`): everything that is neither a list nor a map.
    pub fn is_scalar(&self) -> bool {
        !matches!(self.value, Value::List(_) | Value::Map(_))
    }

    pub fn is_null(&self) -> bool {
        matches!(self.value, Value::Null)
    }

    /// The pair with string key `key` of a map (`nodes[key]`): the key and the
    /// value. The last occurrence wins, as in `package:yaml`'s map.
    pub fn par(&self, key: &str) -> Option<&(Node, Node)> {
        self.map()?.iter().rev().find(|(k, _)| k.text() == Some(key))
    }

    pub fn field(&self, key: &str) -> Option<&Node> {
        self.par(key).map(|(_, v)| v)
    }
}

enum OpenNode {
    List(usize, Vec<Node>),
    Map(usize, Vec<(Node, Node)>, Option<Node>),
}

struct Builder<'a> {
    source: &'a str,
    /// The byte offset of each character (the marker counts characters).
    bytes: Vec<usize>,
    stack: Vec<OpenNode>,
    root: Option<Node>,
}

impl Builder<'_> {
    fn offset(&self, m: &Marker) -> usize {
        self.bytes.get(m.index()).copied().unwrap_or(self.source.len())
    }

    fn deliver(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(OpenNode::List(_, items)) => items.push(node),
            Some(OpenNode::Map(_, pairs, key)) => match key.take() {
                Some(k) => pairs.push((k, node)),
                None => *key = Some(node),
            },
            None => {
                if self.root.is_none() {
                    self.root = Some(node);
                }
            }
        }
    }

    /// The end of a quoted scalar starting at `start`.
    fn quotes_end(&self, start: usize) -> usize {
        let b = self.source.as_bytes();
        let Some(&quote) = b.get(start) else { return start };
        let mut i = start + 1;
        while i < b.len() {
            if quote == b'"' && b[i] == b'\\' {
                i += 2;
            } else if b[i] == quote {
                // `''` inside single quotes is an escaped quote.
                if quote == b'\'' && b.get(i + 1) == Some(&b'\'') {
                    i += 2;
                } else {
                    return i + 1;
                }
            } else {
                i += 1;
            }
        }
        b.len()
    }

    /// The end of a block scalar (`|` or `>`): the last content line with the
    /// block's indentation.
    fn block_end(&self, start: usize) -> usize {
        let rest = &self.source[start..];
        let Some(first_break) = rest.find('\n') else { return self.source.len() };
        let mut end = start + first_break;
        let mut pos = end + 1;
        let mut indent: Option<usize> = None;
        for line in self.source[pos..].split_inclusive('\n') {
            let contents = line.trim_end_matches(['\n', '\r']);
            if !contents.trim().is_empty() {
                let r = contents.len() - contents.trim_start().len();
                match indent {
                    None if r == 0 => break,
                    None => indent = Some(r),
                    Some(base) if r < base => break,
                    _ => {}
                }
                end = pos + contents.len();
            }
            pos += line.len();
        }
        end
    }

    fn is_scalar(&self, value: String, style: TScalarStyle, start: usize) -> Node {
        match style {
            TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => {
                Node { span: Span { start, end: self.quotes_end(start) }, value: Value::Text(value) }
            }
            TScalarStyle::Literal | TScalarStyle::Folded => {
                Node { span: Span { start, end: self.block_end(start) }, value: Value::Text(value) }
            }
            TScalarStyle::Plain => {
                let rest = &self.source[start.min(self.source.len())..];
                // The empty node arrives as `~` without the `~` being in the source.
                if value == "~" && !rest.starts_with('~') {
                    return Node { span: Span { start, end: start }, value: Value::Null };
                }
                let length = if rest.starts_with(value.as_str()) {
                    value.len()
                } else {
                    // A multi-line plain scalar: up to the end of the first line,
                    // without the comment.
                    let line = rest.split('\n').next().unwrap_or("");
                    line.find(" #").unwrap_or(line.len()).min(line.trim_end().len())
                };
                let span = Span { start, end: start + length };
                let lowercase = value.to_ascii_lowercase();
                let value = if value.is_empty() || value == "~" || lowercase == "null" {
                    Value::Null
                } else if lowercase == "true" || lowercase == "false" || value.parse::<f64>().is_ok() || value.starts_with("0x") {
                    Value::Other(value)
                } else {
                    Value::Text(value)
                };
                Node { span, value }
            }
        }
    }

    fn close(&mut self, mark: usize) {
        let Some(open) = self.stack.pop() else { return };
        let (start, last_end, value) = match open {
            OpenNode::List(start, items) => (start, items.last().map(|n| n.span.end), Value::List(items)),
            OpenNode::Map(start, pairs, _) => (start, pairs.last().map(|(_, v)| v.span.end), Value::Map(pairs)),
        };
        let in_flow = self.source[start.min(self.source.len())..].starts_with(['[', '{']);
        let end = if in_flow { (mark + 1).min(self.source.len()) } else { last_end.unwrap_or(start).max(start) };
        self.deliver(Node { span: Span { start, end }, value });
    }
}

impl MarkedEventReceiver for Builder<'_> {
    fn on_event(&mut self, ev: Event, mark: Marker) {
        let offset = self.offset(&mark);
        match ev {
            Event::Scalar(value, style, ..) => {
                let node = self.is_scalar(value, style, offset);
                self.deliver(node);
            }
            Event::SequenceStart(..) => self.stack.push(OpenNode::List(offset, Vec::new())),
            Event::MappingStart(..) => self.stack.push(OpenNode::Map(offset, Vec::new(), None)),
            Event::SequenceEnd | Event::MappingEnd => self.close(offset),
            // A reference to an anchor: without the resolved value, a null.
            Event::Alias(_) => self.deliver(Node { span: Span { start: offset, end: offset }, value: Value::Null }),
            _ => {}
        }
    }
}

/// A YAML syntax error: the message and the offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlError {
    pub message: String,
    pub offset: usize,
}

/// The tree of the first document of `source`; `None` for a file without a
/// document (empty or only comments).
pub fn parse(source: &str) -> Result<Option<Node>, YamlError> {
    let mut bytes: Vec<usize> = source.char_indices().map(|(i, _)| i).collect();
    bytes.push(source.len());
    let mut builder = Builder { source, bytes, stack: Vec::new(), root: None };
    let mut parser = Parser::new_from_str(source);
    match parser.load(&mut builder, false) {
        Ok(()) => Ok(builder.root),
        Err(e) => {
            let offset = builder.bytes.get(e.marker().index()).copied().unwrap_or(source.len());
            Err(YamlError { message: e.info().to_string(), offset })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fragment<'a>(source: &'a str, n: &Node) -> &'a str {
        &source[n.span.start..n.span.end]
    }

    #[test]
    fn key_and_value_positions() {
        let source = "name: app\ndependencies:\n  a: ^1.0.0\n  b:\n    path: '../b' # perto\nflutter:\n";
        let root = parse(source).unwrap().unwrap();
        let (key, value) = root.par("name").unwrap();
        assert_eq!((fragment(source, key), fragment(source, value)), ("name", "app"));
        let b = root.field("dependencies").unwrap().field("b").unwrap();
        let (key_of_path, path) = b.par("path").unwrap();
        assert_eq!(fragment(source, key_of_path), "path");
        assert_eq!(fragment(source, path), "'../b'");
        assert_eq!(path.text(), Some("../b"));
        // The empty value is null, with length zero.
        let flutter = root.field("flutter").unwrap();
        assert!(flutter.is_null() && flutter.span.start == flutter.span.end);
    }

    #[test]
    fn flow_and_types() {
        let source = "a: [1, x, true]\nb: {k: v}\n";
        let root = parse(source).unwrap().unwrap();
        let a = root.field("a").unwrap();
        assert_eq!(fragment(source, a), "[1, x, true]");
        let items = a.list().unwrap();
        assert!(matches!(items[0].value, Value::Other(_)) && items[1].text() == Some("x") && matches!(items[2].value, Value::Other(_)));
        assert_eq!(fragment(source, root.field("b").unwrap()), "{k: v}");
    }

    #[test]
    fn syntax_error() {
        assert!(parse("a: [\n").is_err());
        assert_eq!(parse("# only a comment\n").unwrap(), None);
    }
}
