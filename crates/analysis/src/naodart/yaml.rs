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
pub enum Valor {
    /// Empty, `~` or `null`.
    Nulo,
    /// A string (a quoted, block, or plain scalar that is not a number,
    /// boolean or null).
    Texto(String),
    /// A number or boolean, with the text as written.
    Outro(String),
    Lista(Vec<No>),
    /// The pairs in file order.
    Mapa(Vec<(No, No)>),
}

/// A node with its range in the source.
#[derive(Debug, Clone, PartialEq)]
pub struct No {
    pub span: Span,
    pub valor: Valor,
}

impl No {
    /// The text of a string scalar.
    pub fn texto(&self) -> Option<&str> {
        match &self.valor {
            Valor::Texto(t) => Some(t),
            _ => None,
        }
    }

    pub fn mapa(&self) -> Option<&[(No, No)]> {
        match &self.valor {
            Valor::Mapa(m) => Some(m),
            _ => None,
        }
    }

    pub fn lista(&self) -> Option<&[No]> {
        match &self.valor {
            Valor::Lista(l) => Some(l),
            _ => None,
        }
    }

    /// A scalar (`YamlScalar`): everything that is neither a list nor a map.
    pub fn escalar(&self) -> bool {
        !matches!(self.valor, Valor::Lista(_) | Valor::Mapa(_))
    }

    pub fn nulo(&self) -> bool {
        matches!(self.valor, Valor::Nulo)
    }

    /// The pair with string key `key` of a map (`nodes[key]`): the key and the
    /// value. The last occurrence wins, as in `package:yaml`'s map.
    pub fn par(&self, key: &str) -> Option<&(No, No)> {
        self.mapa()?.iter().rev().find(|(k, _)| k.texto() == Some(key))
    }

    pub fn campo(&self, key: &str) -> Option<&No> {
        self.par(key).map(|(_, v)| v)
    }
}

enum OpenNode {
    Lista(usize, Vec<No>),
    Mapa(usize, Vec<(No, No)>, Option<No>),
}

struct Builder<'a> {
    fonte: &'a str,
    /// The byte offset of each character (the marker counts characters).
    bytes: Vec<usize>,
    stack: Vec<OpenNode>,
    root: Option<No>,
}

impl Builder<'_> {
    fn offset(&self, m: &Marker) -> usize {
        self.bytes.get(m.index()).copied().unwrap_or(self.fonte.len())
    }

    fn deliver(&mut self, node: No) {
        match self.stack.last_mut() {
            Some(OpenNode::Lista(_, items)) => items.push(node),
            Some(OpenNode::Mapa(_, pairs, key)) => match key.take() {
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
        let b = self.fonte.as_bytes();
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
        let rest = &self.fonte[start..];
        let Some(first_break) = rest.find('\n') else { return self.fonte.len() };
        let mut end = start + first_break;
        let mut pos = end + 1;
        let mut indent: Option<usize> = None;
        for line in self.fonte[pos..].split_inclusive('\n') {
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

    fn escalar(&self, value: String, style: TScalarStyle, start: usize) -> No {
        match style {
            TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => {
                No { span: Span { start, end: self.quotes_end(start) }, valor: Valor::Texto(value) }
            }
            TScalarStyle::Literal | TScalarStyle::Folded => {
                No { span: Span { start, end: self.block_end(start) }, valor: Valor::Texto(value) }
            }
            TScalarStyle::Plain => {
                let rest = &self.fonte[start.min(self.fonte.len())..];
                // The empty node arrives as `~` without the `~` being in the source.
                if value == "~" && !rest.starts_with('~') {
                    return No { span: Span { start, end: start }, valor: Valor::Nulo };
                }
                let length = if rest.starts_with(value.as_str()) {
                    value.len()
                } else {
                    // A multi-line plain scalar: up to the end of the first line,
                    // without the comment.
                    let line = rest.split('\n').next().unwrap_or("");
                    line.find(" #").map_or(line.len(), |k| k).min(line.trim_end().len())
                };
                let span = Span { start, end: start + length };
                let minusculo = value.to_ascii_lowercase();
                let value = if value.is_empty() || value == "~" || minusculo == "null" {
                    Valor::Nulo
                } else if minusculo == "true" || minusculo == "false" || value.parse::<f64>().is_ok() || value.starts_with("0x") {
                    Valor::Outro(value)
                } else {
                    Valor::Texto(value)
                };
                No { span, valor: value }
            }
        }
    }

    fn fechar(&mut self, mark: usize) {
        let Some(open) = self.stack.pop() else { return };
        let (start, last_end, value) = match open {
            OpenNode::Lista(start, items) => (start, items.last().map(|n| n.span.end), Valor::Lista(items)),
            OpenNode::Mapa(start, pairs, _) => (start, pairs.last().map(|(_, v)| v.span.end), Valor::Mapa(pairs)),
        };
        let in_flow = self.fonte[start.min(self.fonte.len())..].starts_with(['[', '{']);
        let end = if in_flow { (mark + 1).min(self.fonte.len()) } else { last_end.unwrap_or(start).max(start) };
        self.deliver(No { span: Span { start, end }, valor: value });
    }
}

impl MarkedEventReceiver for Builder<'_> {
    fn on_event(&mut self, ev: Event, mark: Marker) {
        let offset = self.offset(&mark);
        match ev {
            Event::Scalar(value, style, ..) => {
                let node = self.escalar(value, style, offset);
                self.deliver(node);
            }
            Event::SequenceStart(..) => self.stack.push(OpenNode::Lista(offset, Vec::new())),
            Event::MappingStart(..) => self.stack.push(OpenNode::Mapa(offset, Vec::new(), None)),
            Event::SequenceEnd | Event::MappingEnd => self.fechar(offset),
            // A reference to an anchor: without the resolved value, a null.
            Event::Alias(_) => self.deliver(No { span: Span { start: offset, end: offset }, valor: Valor::Nulo }),
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
pub fn ler(source: &str) -> Result<Option<No>, YamlError> {
    let mut bytes: Vec<usize> = source.char_indices().map(|(i, _)| i).collect();
    bytes.push(source.len());
    let mut builder = Builder { fonte: source, bytes, stack: Vec::new(), root: None };
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
mod testes {
    use super::*;

    fn fragment<'a>(source: &'a str, n: &No) -> &'a str {
        &source[n.span.start..n.span.end]
    }

    #[test]
    fn posicoes_de_chaves_e_valores() {
        let source = "name: app\ndependencies:\n  a: ^1.0.0\n  b:\n    path: '../b' # perto\nflutter:\n";
        let root = ler(source).unwrap().unwrap();
        let (key, value) = root.par("name").unwrap();
        assert_eq!((fragment(source, key), fragment(source, value)), ("name", "app"));
        let b = root.campo("dependencies").unwrap().campo("b").unwrap();
        let (key_of_path, path) = b.par("path").unwrap();
        assert_eq!(fragment(source, key_of_path), "path");
        assert_eq!(fragment(source, path), "'../b'");
        assert_eq!(path.texto(), Some("../b"));
        // The empty value is null, with length zero.
        let flutter = root.campo("flutter").unwrap();
        assert!(flutter.nulo() && flutter.span.start == flutter.span.end);
    }

    #[test]
    fn fluxo_e_tipos() {
        let source = "a: [1, x, true]\nb: {k: v}\n";
        let root = ler(source).unwrap().unwrap();
        let a = root.campo("a").unwrap();
        assert_eq!(fragment(source, a), "[1, x, true]");
        let items = a.lista().unwrap();
        assert!(matches!(items[0].valor, Valor::Outro(_)) && items[1].texto() == Some("x") && matches!(items[2].valor, Valor::Outro(_)));
        assert_eq!(fragment(source, root.campo("b").unwrap()), "{k: v}");
    }

    #[test]
    fn erro_de_sintaxe() {
        assert!(ler("a: [\n").is_err());
        assert_eq!(ler("# só comentário\n").unwrap(), None);
    }
}
