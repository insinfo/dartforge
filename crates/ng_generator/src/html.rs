//! Parser of ngdart HTML templates.
//!
//! Equivalent to `package:ngast` (the parser `ngcompiler` uses), in the part
//! the generator needs: elements, text, interpolation, comments, the binding
//! forms (`[x]`, `(x)`, `[(x)]`, `#ref`, `*ngIf`) and `<ng-content>`.
//!
//! The whitespace reduction (`preserveWhitespace: false`, which is the default)
//! follows the rules of ngast's `MinimizeWhitespaceVisitor`, because they
//! decide which text nodes exist — and therefore the generated code. They were
//! read from the package itself, not guessed.

/// Elements that HTML closes by itself.
pub(crate) const EMPTY: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Elements that are normally `display: inline` — ngast's list, which decides
/// where whitespace is significant.
const INLINE: &[&str] = &[
    "a", "abbr", "acronym", "b", "bdo", "big", "br", "button", "cite", "code", "dfn", "em", "i",
    "img", "input", "kbd", "label", "map", "object", "q", "samp", "script", "select", "small",
    "span", "strong", "sub", "sup", "textarea", "time", "tt", "var",
];

/// `&ngsp;` becomes this character in the ngast scanner and turns back into a
/// space at the end, escaping the reduction.
pub(crate) const NGSP: char = '\u{E500}';
const NBSP: char = '\u{00A0}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Str(String),
    /// `{{ expression }}`, with the byte range of the whole form (`{{` to `}}`)
    /// in the template file — it is what goes in the `/* REF:url:start:end */`
    /// comment the official compiler writes.
    Interpolation {
        expr: String,
        start: usize,
        end: usize,
    },
    Comment(String),
    /// `<ng-content select="...">`.
    Content {
        selector: Option<String>,
    },
}

/// A binding written on the element.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Binding {
    /// `hidden` in `[hidden]`, `click` in `(click)`.
    pub name: String,
    /// Dart expression between quotes, as written.
    pub value: String,
    /// Byte range of the whole attribute in the template (from the name to the
    /// end of the value), which is what goes in the `/* REF:url:start:end */`
    /// comment.
    pub start: usize,
    pub end: usize,
    /// Written without `=` (`<input required>`, and the `dir` that desugaring
    /// `*dir="key: x"` generates): `EmptyExpr`, which for a `bool` input is
    /// `true`. `x=""` is not this.
    pub without_value: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    /// `class="x"` — a literal value, which may contain interpolation.
    pub attributes: Vec<Binding>,
    /// `[x]="e"`.
    pub properties: Vec<Binding>,
    /// `(x)="e"`.
    pub events: Vec<Binding>,
    /// `[(x)]="e"`.
    pub bananas: Vec<Binding>,
    /// `#ref` or `#ref="ngForm"`.
    pub references: Vec<Binding>,
    /// `*ngIf="e"` — a forma abreviada do `<template>`.
    pub star: Option<Binding>,
    /// `@i18n="description"`, `@i18n:title="…"`: the annotations (ngast's
    /// `AnnotationAst`), with the name without the `@`.
    pub annotations: Vec<Binding>,
    pub children: Vec<Node>,
    /// The `[dirX]="e"` bindings of a hand-written `<template dir let-x
    /// [dirX]="e">`, after it is rewritten as `*dir` (see
    /// `template_as_container` in `view.rs`): the `REF` of each input is the
    /// range of the written binding, not that of the whole `star`. Empty for
    /// the rest.
    pub template_bindings: Vec<Binding>,
    /// The already decomposed microsyntax of that rewritten `<template>`: the
    /// bindings and the `let-`s written separately, without going back to the
    /// text (which `isMicroExpression` would not always recognize, case j75).
    pub template_micro: Option<crate::micro::Micro>,
    /// Position of the `<` that opens the element in the template (in bytes):
    /// it identifies the node, as the start of the `star` identifies the `*`
    /// (the anchor of a written `<template>` in view queries).
    pub start: usize,
}

impl Element {
    /// The microsyntax of the element's `*`: that of the rewritten
    /// `<template>`, or that of the `star` text.
    pub fn star_micro(&self) -> Option<crate::micro::Micro> {
        let star = self.star.as_ref()?;
        Some(match &self.template_micro {
            Some(m) => m.clone(),
            None => crate::micro::parse(&star.name, &star.value),
        })
    }

    /// Has a property binding: `[x]` or an attribute with `{{ }}` — what makes
    /// the element become a field of the view.
    pub fn binds_property(&self) -> bool {
        !self.properties.is_empty() || self.attributes.iter().any(|a| a.value.contains("{{"))
    }

    fn inline(&self) -> bool {
        INLINE.contains(&self.name.to_ascii_lowercase().as_str())
    }
}

/// Does the template project content (`<ng-content>`)? The component's user
/// needs to know: with projection the child view is created by
/// `createAndProject`, without it by `create`.
pub fn has_projection(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Content { .. } => true,
        Node::Element(e) => has_projection(&e.children),
        _ => false,
    })
}

/// The `<ng-content>`s of the template, in document order, with the `select` of
/// each one — the component's `ngContentSelectors`.
pub fn projections(nodes: &[Node]) -> Vec<Option<String>> {
    let mut output = Vec::new();
    fn walk(nodes: &[Node], output: &mut Vec<Option<String>>) {
        for n in nodes {
            match n {
                Node::Content { selector } => output.push(selector.clone()),
                Node::Element(e) => walk(&e.children, output),
                _ => {}
            }
        }
    }
    walk(nodes, &mut output);
    output
}

/// Parses a template. Shape errors do not stop it: the parser recovers and goes
/// on, like ngast's, so that a broken template does not bring down the whole
/// generation.
pub fn parse(source: &str) -> Vec<Node> {
    parse_with_error(source).0
}

/// Like [`parse`], with the first shape error that ngast rejects: with it,
/// the official compiler fails to compile the template.
pub fn parse_with_error(source: &str) -> (Vec<Node>, Option<String>) {
    parse_in_mode(source, false)
}

/// `preserve`: the official `_PreserveWhitespaceVisitor`
/// (`ast_template_parser.dart:1610-1657`) in place of the
/// `MinimizeWhitespaceVisitor` — no text disappears or is trimmed; only
/// `&ngsp;` becomes a space, in all text.
pub fn parse_in_mode(source: &str, preserve: bool) -> (Vec<Node>, Option<String>) {
    let mut p = Parser {
        b: source.as_bytes(),
        i: 0,
        source,
        error: None,
    };
    let raw_nodes = p.nodes(None);
    let mut nodes = if preserve {
        replace_ngsp(raw_nodes)
    } else {
        minimize_whitespace(raw_nodes)
    };
    if !source.is_ascii() {
        in_utf16(&mut nodes, source);
    }
    (nodes, p.error)
}

/// `&ngsp;` becomes a space in all text of the tree.
fn replace_ngsp(nodes: Vec<Node>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|n| match n {
            Node::Str(t) => Node::Str(t.replace(NGSP, " ")),
            Node::Element(mut e) => {
                e.children = replace_ngsp(std::mem::take(&mut e.children));
                Node::Element(e)
            }
            other => other,
        })
        .collect()
}

/// Adds `k` to the positions of all bindings and interpolations: the template
/// written in the annotation, whose `REF` positions are those of the `.dart`
/// (the official compiler adds the `templateOffset`).
pub fn shift_positions(nodes: &mut [Node], k: usize) {
    for node in nodes {
        match node {
            Node::Interpolation { start, end, .. } => {
                *start += k;
                *end += k;
            }
            Node::Element(e) => {
                for l in e
                    .attributes
                    .iter_mut()
                    .chain(e.properties.iter_mut())
                    .chain(e.events.iter_mut())
                    .chain(e.bananas.iter_mut())
                    .chain(e.references.iter_mut())
                    .chain(e.star.iter_mut())
                {
                    l.start += k;
                    l.end += k;
                }
                shift_positions(&mut e.children, k);
            }
            _ => {}
        }
    }
}

/// The parser walks in bytes, but the official `REF:url:start:end` counts in
/// UTF-16 code units: `ngast` opens the template with `SourceFile.fromString`,
/// which uses `text.codeUnits`. A `©` before the binding is 2 bytes and 1 unit.
fn in_utf16(nodes: &mut [Node], source: &str) {
    let conv = |b: usize| source.get(..b).map_or(b, |s| s.encode_utf16().count());
    for node in nodes {
        match node {
            Node::Interpolation { start, end, .. } => {
                *start = conv(*start);
                *end = conv(*end);
            }
            Node::Element(e) => {
                for l in e
                    .attributes
                    .iter_mut()
                    .chain(e.properties.iter_mut())
                    .chain(e.events.iter_mut())
                    .chain(e.bananas.iter_mut())
                    .chain(e.references.iter_mut())
                    .chain(e.star.iter_mut())
                {
                    l.start = conv(l.start);
                    l.end = conv(l.end);
                }
                in_utf16(&mut e.children, source);
            }
            _ => {}
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
    source: &'a str,
    /// A shape error ngast does not recover from (the official compiler fails
    /// and generates nothing).
    error: Option<String>,
}

impl<'a> Parser<'a> {
    fn end(&self) -> bool {
        self.i >= self.b.len()
    }

    fn look(&self, k: usize) -> u8 {
        *self.b.get(self.i + k).unwrap_or(&0)
    }

    /// Comparison in bytes: the cursor walks byte by byte and may stop in the
    /// middle of a multibyte character, where slicing the string would panic.
    fn starts_with_word(&self, s: &str) -> bool {
        self.b[self.i.min(self.b.len())..].starts_with(s.as_bytes())
    }

    /// Reads nodes up to the end or up to the closing of `parent`.
    fn nodes(&mut self, parent: Option<&str>) -> Vec<Node> {
        let mut output = Vec::new();
        while !self.end() {
            if self.starts_with_word("</") {
                let closes = self.read_closing();
                match parent {
                    // Fechamento do nosso elemento: devolve ao chamador.
                    Some(p) if closes.eq_ignore_ascii_case(p) => return output,
                    // Closing of another element: HTML allows it, we ignore it.
                    _ => continue,
                }
            }
            if self.starts_with_word("<!--") {
                output.push(self.read_comment());
                continue;
            }
            if self.look(0) == b'<' && (self.look(1).is_ascii_alphabetic() || self.look(1) == b'!')
            {
                if self.look(1) == b'!' {
                    // `<!DOCTYPE …>` e afins: fora do template do Angular.
                    self.skip_until(b'>');
                    continue;
                }
                if let Some(node) = self.read_element() {
                    output.push(node);
                }
                continue;
            }
            self.read_text(&mut output);
        }
        output
    }

    fn skip_until(&mut self, end: u8) {
        while !self.end() && self.look(0) != end {
            self.i += 1;
        }
        if !self.end() {
            self.i += 1;
        }
    }

    fn read_comment(&mut self) -> Node {
        self.i += 4;
        let start = self.i;
        while !self.end() && !self.starts_with_word("-->") {
            self.i += 1;
        }
        let text = self.source[start..self.i.min(self.source.len())].to_string();
        if !self.end() {
            self.i += 3;
        }
        Node::Comment(text)
    }

    fn read_closing(&mut self) -> String {
        self.i += 2;
        let start = self.i;
        while !self.end() && self.look(0) != b'>' {
            self.i += 1;
        }
        let name = self.source[start..self.i.min(self.source.len())]
            .trim()
            .to_string();
        if !self.end() {
            self.i += 1;
        }
        name
    }

    /// Text and interpolations up to the next `<`.
    fn read_text(&mut self, output: &mut Vec<Node>) {
        let start = self.i;
        // `_allTextMatches` (`simple_tokenizer.dart:30`): the text goes up to
        // the next `<`, but inside `{{ }}` the state is interpolation, where
        // `<` is just the operator (`a < b`). Outside it, a `<` that opens
        // neither a tag nor a comment is an `openTagStart` without a name: an
        // error in ngast, and the official compiler generates nothing; here it
        // is consumed (without hanging) and recorded.
        while !self.end() {
            if self.starts_with_word("{{") {
                match self.source[self.i + 2..].find("}}") {
                    Some(f) => self.i += 2 + f + 2,
                    None => self.i = self.b.len(),
                }
                continue;
            }
            if self.look(0) == b'<' {
                let opens =
                    self.look(1).is_ascii_alphabetic() || matches!(self.look(1), b'/' | b'!');
                if opens {
                    break;
                }
                self.error
                    .get_or_insert_with(|| "`<` that does not open a tag in the text".into());
            }
            self.i += 1;
        }
        let raw_text = &self.source[start..self.i];
        let mut rest = raw_text;
        while let Some(pos) = rest.find("{{") {
            if pos > 0 {
                output.push(Node::Str(decode(&rest[..pos])));
            }
            rest = &rest[pos + 2..];
            match rest.find("}}") {
                Some(f) => {
                    let begin = rest.as_ptr() as usize - self.source.as_ptr() as usize - 2;
                    // The value of a text interpolation goes through
                    // `_unEscapeText` like the text (R1.5: `{{ a &amp;&amp; b
                    // }}` is `a && b`).
                    output.push(Node::Interpolation {
                        expr: decode(&rest[..f]).trim().to_string(),
                        start: begin,
                        end: begin + 2 + f + 2,
                    });
                    rest = &rest[f + 2..];
                }
                None => {
                    // An unclosed `{{`: becomes text, like any other character.
                    output.push(Node::Str(decode(rest)));
                    return;
                }
            }
        }
        if !rest.is_empty() {
            output.push(Node::Str(decode(rest)));
        }
    }

    fn read_element(&mut self) -> Option<Node> {
        let opening = self.i;
        self.i += 1;
        let start = self.i;
        while !self.end()
            && !self.look(0).is_ascii_whitespace()
            && !matches!(self.look(0), b'>' | b'/')
        {
            self.i += 1;
        }
        let name = self.source[start..self.i].to_string();
        let mut el = Element {
            name,
            start: opening,
            ..Default::default()
        };
        let mut content_selector = None;
        let mut alone = false;
        loop {
            self.skip_whitespace();
            if self.end() {
                break;
            }
            if self.look(0) == b'>' {
                self.i += 1;
                break;
            }
            if self.look(0) == b'/' && self.look(1) == b'>' {
                self.i += 2;
                alone = true;
                break;
            }
            let start = self.i;
            let Some((name, value, without_value)) = self.read_attribute() else {
                break;
            };
            classify(
                &mut el,
                &mut content_selector,
                name,
                value,
                (start, self.i),
                without_value,
            );
        }
        let empty = EMPTY.contains(&el.name.to_ascii_lowercase().as_str());
        if !alone && !empty {
            let name = el.name.clone();
            el.children = self.nodes(Some(&name));
        }
        if el.name.eq_ignore_ascii_case("ng-content") {
            return Some(Node::Content {
                selector: content_selector,
            });
        }
        Some(Node::Element(el))
    }

    fn skip_whitespace(&mut self) {
        while !self.end() && self.look(0).is_ascii_whitespace() {
            self.i += 1;
        }
    }

    /// The name, the value and whether it was written without `=`.
    fn read_attribute(&mut self) -> Option<(String, String, bool)> {
        let start = self.i;
        while !self.end()
            && !self.look(0).is_ascii_whitespace()
            && !matches!(self.look(0), b'=' | b'>' | b'/')
        {
            self.i += 1;
        }
        if self.i == start {
            // A character that does not start an attribute name: advance so as
            // not to hang.
            self.i += 1;
            return None;
        }
        let name = self.source[start..self.i].to_string();
        let name_end = self.i;
        self.skip_whitespace();
        if self.end() || self.look(0) != b'=' {
            // Without a value, the attribute range (that of the `REF`) is just
            // the name: the whitespace up to the next one is left out.
            self.i = name_end;
            return Some((name, String::new(), true));
        }
        self.i += 1;
        self.skip_whitespace();
        let quotes = self.look(0);
        if quotes == b'"' || quotes == b'\'' {
            self.i += 1;
            let begin = self.i;
            while !self.end() && self.look(0) != quotes {
                self.i += 1;
            }
            let value = self.source[begin..self.i].to_string();
            if !self.end() {
                self.i += 1;
            }
            Some((name, value, false))
        } else {
            let begin = self.i;
            while !self.end()
                && !self.look(0).is_ascii_whitespace()
                && !matches!(self.look(0), b'>' | b'/')
            {
                self.i += 1;
            }
            Some((name, self.source[begin..self.i].to_string(), false))
        }
    }
}

/// Puts the attribute read into the right list of the element.
fn classify(
    el: &mut Element,
    content: &mut Option<String>,
    name: String,
    value: String,
    (start, end): (usize, usize),
    without_value: bool,
) {
    let l = Binding {
        name: String::new(),
        value: value.clone(),
        start,
        end,
        without_value,
    };
    if let Some(internal) = name.strip_prefix("[(").and_then(|n| n.strip_suffix(")]")) {
        el.bananas.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
        el.properties.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix('(').and_then(|n| n.strip_suffix(')')) {
        el.events.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix('#') {
        el.references.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix('*') {
        el.star = Some(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix('@') {
        el.annotations.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix("on-").filter(|n| !n.is_empty()) {
        // The long forms that ngast's `RecursiveAstParser` recognizes: `on-x`
        // is `(x)` and `bind-x` is `[x]` (there is no `bindon-` or `ref-`).
        el.events.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else if let Some(internal) = name.strip_prefix("bind-").filter(|n| !n.is_empty()) {
        el.properties.push(Binding {
            name: internal.to_string(),
            ..l
        });
    } else {
        if el.name.eq_ignore_ascii_case("ng-content") && name == "select" {
            *content = Some(value.clone());
        }
        el.attributes.push(Binding {
            name,
            value,
            start,
            end,
            without_value,
        });
    }
}

/// The `_unEscapeText` of the ngast tokenizer (`simple_tokenizer.dart`),
/// applied only to text (attribute values stay raw): each match of
/// `&#([0-9]{2,4});|&#x([0-9A-Fa-f]{2,4});|&([a-zA-Z]+);`, from left to right,
/// becomes the character of the code (`String.fromCharCode`) or that of the
/// table [`crate::entities::ENTITIES`] — and a name that is not in it, the
/// name itself. A code in the surrogate range (`&#xD800;`) would be a lone
/// surrogate in the Dart string, which the file written in UTF-8 carries as
/// U+FFFD.
fn decode(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    let b = t.as_bytes();
    let mut output = String::with_capacity(t.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        if b[i] != b'&' {
            i += 1;
            continue;
        }
        if let Some((end, value)) = entity_at(t, i) {
            output.push_str(&t[copied..i]);
            output.push_str(&value);
            i = end;
            copied = end;
        } else {
            i += 1;
        }
    }
    output.push_str(&t[copied..]);
    output
}

/// The character reference that starts at `i` (at the `&`): where it ends
/// (after the `;`) and the text that replaces it.
fn entity_at(t: &str, i: usize) -> Option<(usize, String)> {
    let b = t.as_bytes();
    let code =
        |n: u32| -> String { char::from_u32(n).map_or('\u{FFFD}'.to_string(), |c| c.to_string()) };
    // `&#` + 2 to 4 digits + `;`
    if b.get(i + 1) == Some(&b'#') {
        let start = i + 2;
        let n = b[start..]
            .iter()
            .take(4)
            .take_while(|c| c.is_ascii_digit())
            .count();
        if n >= 2 && b.get(start + n) == Some(&b';') {
            let v: u32 = t[start..start + n].parse().ok()?;
            return Some((start + n + 1, code(v)));
        }
        // `&#x` + 2 a 4 hexadecimais + `;`
        if b.get(i + 2) == Some(&b'x') {
            let start = i + 3;
            let n = b[start..]
                .iter()
                .take(4)
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            if n >= 2 && b.get(start + n) == Some(&b';') {
                let v = u32::from_str_radix(&t[start..start + n], 16).ok()?;
                return Some((start + n + 1, code(v)));
            }
        }
        return None;
    }
    // `&` + letras + `;`
    let start = i + 1;
    let n = b[start..]
        .iter()
        .take_while(|c| c.is_ascii_alphabetic())
        .count();
    if n >= 1 && b.get(start + n) == Some(&b';') {
        let name = &t[start..start + n];
        let value = crate::entities::ENTITIES
            .binary_search_by(|(k, _)| k.cmp(&name))
            .map_or(name, |k| crate::entities::ENTITIES[k].1);
        return Some((start + n + 1, value.to_string()));
    }
    None
}

/// ngast's `MinimizeWhitespaceVisitor`: drops a whitespace-only text node
/// between nodes that are not inline, and collapses the rest into a single
/// space.
/// ngast's `MinimizeWhitespaceVisitor` (`preserveWhitespace: false`), top-down
/// like it: each list of children is reduced looking at the **raw** neighbors
/// (their children not yet reduced) and only then is each child visited.
/// `<pre>` and whatever has `@preserveWhitespace` stay as they are, with
/// everything inside (`_bailOutToPreserveWhitespace`).
fn minimize_whitespace(nodes: Vec<Node>) -> Vec<Node> {
    reduce_whitespace(nodes)
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) => {
                let preserves =
                    e.name == "pre" || e.annotations.iter().any(|a| a.name == "preserveWhitespace");
                if !preserves && !e.children.is_empty() {
                    e.children = minimize_whitespace(std::mem::take(&mut e.children));
                }
                Node::Element(e)
            }
            other => other,
        })
        .collect()
}

fn reduce_whitespace(nodes: Vec<Node>) -> Vec<Node> {
    let mut output: Vec<Node> = Vec::with_capacity(nodes.len());
    // `anterior` is the *already processed* node — if the previous text
    // disappeared, the left neighbor is what remained before it. `seguinte` is
    // the original node at index i+1, not yet processed. That is how `ngast`
    // does it.
    let mut previous_is_collapsible = true;
    for (i, node) in nodes.iter().enumerate() {
        let Node::Str(text) = node else {
            previous_is_collapsible = collapses_next_to(Some(node), true);
            output.push(node.clone());
            continue;
        };
        let collapses_left = previous_is_collapsible;
        let collapses_right = collapses_next_to(nodes.get(i + 1), false);
        if collapses_left && collapses_right && text.trim().is_empty() && !text.contains(NBSP) {
            // Whitespace-only text between two block nodes: dropped.
            previous_is_collapsible = true;
            continue;
        }
        match collapse(text, collapses_left, collapses_right) {
            Some(t) => {
                // A text node is a `StandaloneTemplateAst`: it holds the
                // neighbor's whitespace, like any other node that is not a
                // block element.
                previous_is_collapsible = false;
                output.push(Node::Str(t));
            }
            // Text removed: for the next neighbor it is as if there were
            // nothing to the left (`prevNode = null` in ngast).
            None => previous_is_collapsible = true,
        }
    }
    output
}

/// Collapses the whitespace of a text: every run of two or more becomes one
/// space (`RegExp(r'\s\s+')` — a single character, `\n` included, stays as is),
/// and the ends are trimmed according to the neighbors.
fn collapse(text: &str, trim_left: bool, trim_right: bool) -> Option<String> {
    let mut v = String::with_capacity(text.len());
    let mut block = String::new();
    let close = |block: &mut String, v: &mut String| {
        if block.chars().count() == 1 {
            v.push_str(block);
        } else if !block.is_empty() {
            v.push(' ');
        }
        block.clear();
    };
    for c in text.chars() {
        if c.is_whitespace() && c != NBSP {
            block.push(c);
            continue;
        }
        close(&mut block, &mut v);
        v.push(c);
    }
    close(&mut block, &mut v);
    // `&ngsp;` only becomes a space after trimming the ends (the `visitText` of
    // `MinimizeWhitespaceVisitor` runs over the already collapsed text): a `\n
    // &ngsp;` after a block element becomes `' '`. `&nbsp;` is protected before
    // trimming (`preserveNbsp`, `\uE501`): Rust's `trim` would take it along.
    let blank = |c: char| c.is_whitespace() && c != NBSP;
    let mut v = v;
    if trim_left {
        v = v.trim_start_matches(blank).to_string();
    }
    if trim_right {
        v = v.trim_end_matches(blank).to_string();
    }
    let v = v.replace(NGSP, " ");
    if v.is_empty() { None } else { Some(v) }
}

/// The rule of `_shouldCollapseAdjacentTo`: text, interpolation and comments do
/// not hold whitespace; an element holds it if it is inline; `<template>`,
/// `<ng-container>` and the element of a `*` (the `EmbeddedTemplateAst` that
/// wraps it) decide by the child at the end touching the text
/// (`_shouldCollapseWrapperNode`; `last`: the neighbor is on the left).
fn collapses_next_to(node: Option<&Node>, last: bool) -> bool {
    match node {
        None => true,
        Some(Node::Element(e)) if e.star.is_some() => {
            let mut inside = e.clone();
            inside.star = None;
            collapses_wrapped(&[Node::Element(inside)], last)
        }
        Some(Node::Element(e)) if e.name == "template" || e.name == "ng-container" => {
            collapses_wrapped(&e.children, last)
        }
        Some(Node::Element(e)) => !e.inline(),
        Some(Node::Interpolation { .. }) => false,
        Some(Node::Content { .. }) => false,
        // Text and comments are template nodes like any other
        // (`StandaloneTemplateAst`): the whitespace next to them is
        // significant.
        Some(Node::Str(_)) => false,
        Some(Node::Comment(_)) => false,
    }
}

/// `_shouldCollapseWrapperNode`: without children, inline content may come
/// (does not collapse); a whitespace-only child at the end gives way to its
/// neighbor, except when it is the only one. The child is judged with
/// `lastNode` false, as in ngast.
fn collapses_wrapped(children: &[Node], last: bool) -> bool {
    let (Some(first), Some(end)) = (children.first(), children.last()) else {
        return false;
    };
    let mut edge = if last { end } else { first };
    if let Node::Str(t) = edge
        && t.trim().is_empty()
    {
        if children.len() == 1 {
            return false;
        }
        edge = if last {
            &children[children.len() - 2]
        } else {
            &children[1]
        };
    }
    collapses_next_to(Some(edge), false)
}

#[cfg(test)]
mod testes {

    /// A stray `<` in the text does not hang the parser and is a shape error
    /// (ngast reads it as `openTagStart`); inside `{{ }}` it is the operator.
    #[test]
    fn less_than_in_text_and_in_interpolation() {
        let (nodes, error) = parse_with_error("<div>a < b</div><i><3</i>");
        assert!(error.is_some());
        assert_eq!(nodes.len(), 2);
        let (nodes, error) = parse_with_error("<p>{{ x < 3 }}</p>");
        assert!(error.is_none());
        let Node::Element(p) = &nodes[0] else {
            panic!()
        };
        assert!(matches!(&p.children[..], [Node::Interpolation { expr, .. }] if expr == "x < 3"));
    }

    use super::*;

    #[test]
    fn empty_template_and_comment() {
        assert!(parse("").is_empty());
        assert_eq!(
            parse("<!--{{message}}-->"),
            vec![Node::Comment("{{message}}".into())]
        );
    }

    /// `bind-x`/`on-x` are a binding and an event, `@x` is an annotation: none
    /// of the three is a DOM attribute (corpus cases i36 and i20).
    #[test]
    fn long_forms_and_annotations() {
        let nodes = parse(r#"<p bind-title="t" on-click="f()" @i18n="d">x</p>"#);
        let [Node::Element(e)] = nodes.as_slice() else {
            panic!("not a single element: {nodes:?}");
        };
        assert!(e.attributes.is_empty());
        assert_eq!(e.properties[0].name, "title");
        assert_eq!((e.properties[0].start, e.properties[0].end), (3, 17));
        assert_eq!(e.events[0].name, "click");
        assert_eq!(e.annotations[0].name, "i18n");
        assert_eq!(e.annotations[0].value, "d");
    }

    #[test]
    fn ng_content() {
        assert_eq!(
            parse("<ng-content></ng-content>"),
            vec![Node::Content { selector: None }]
        );
        assert_eq!(
            parse("<ng-content select='.x'></ng-content>"),
            vec![Node::Content {
                selector: Some(".x".into())
            }]
        );
    }

    #[test]
    fn element_with_children_and_attributes() {
        let n = parse("<div class=\"a\"><span>oi</span></div>");
        let Node::Element(div) = &n[0] else {
            panic!("{n:?}")
        };
        assert_eq!(div.name, "div");
        assert_eq!(div.attributes[0].name, "class");
        assert_eq!(div.attributes[0].value, "a");
        let Node::Element(span) = &div.children[0] else {
            panic!("{:?}", div.children)
        };
        assert_eq!(span.children, vec![Node::Str("oi".into())]);
    }

    #[test]
    fn binding_shapes() {
        let n = parse("<x [p]=\"a\" (e)=\"b()\" [(v)]=\"c\" #r *ngIf=\"d\"></x>");
        let Node::Element(x) = &n[0] else { panic!() };
        assert_eq!(x.properties[0].name, "p");
        assert_eq!(x.events[0].name, "e");
        assert_eq!(x.bananas[0].name, "v");
        assert_eq!(x.references[0].name, "r");
        assert_eq!(x.star.as_ref().unwrap().name, "ngIf");
        assert_eq!(x.star.as_ref().unwrap().value, "d");
    }

    #[test]
    fn interpolation_splits_the_text() {
        assert_eq!(
            parse("a{{ b }}c"),
            vec![
                Node::Str("a".into()),
                Node::Interpolation {
                    expr: "b".into(),
                    start: 1,
                    end: 8
                },
                Node::Str("c".into())
            ]
        );
    }

    /// The new_sali footer case: the `©` before the interpolation counts one
    /// unit, not two bytes.
    #[test]
    fn position_in_utf16_units() {
        assert_eq!(
            parse("<span>© {{ano}}</span>"),
            vec![Node::Element(Element {
                name: "span".into(),
                children: vec![
                    Node::Str("© ".into()),
                    Node::Interpolation {
                        expr: "ano".into(),
                        start: 8,
                        end: 15
                    },
                ],
                ..Default::default()
            })]
        );
    }

    #[test]
    fn empty_element_does_not_swallow_the_rest() {
        let n = parse("<br><span>x</span>");
        assert_eq!(n.len(), 2);
        let Node::Element(br) = &n[0] else {
            panic!("{n:?}")
        };
        assert!(br.children.is_empty());
    }

    /// Real template of `NoDataComponent`: between `<img>` (an inline element)
    /// and the comment one space remains, and all the rest of the whitespace
    /// disappears. The official compiler emits exactly one `appendText(_el_0, '
    /// ')` there.
    #[test]
    fn no_data_space_equal_to_official() {
        let n = parse(
            "<div class=\"c\">\n      <img src=\"a.svg\">  \n     <!-- x -->\n    <div>{{m}}</div>\n</div>",
        );
        let Node::Element(div) = &n[0] else {
            panic!("{n:?}")
        };
        let texts: Vec<&String> = div
            .children
            .iter()
            .filter_map(|f| match f {
                Node::Str(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec![&" ".to_string()], "{:?}", div.children);
    }

    /// ngast rule: `<div>\n  <span>x</span>\n</div>` collapses to
    /// `<div><span>x</span></div>`.
    #[test]
    fn space_between_blocks_disappears() {
        let n = parse("<div>\n  <p>x</p>\n</div>");
        let Node::Element(div) = &n[0] else { panic!() };
        assert_eq!(div.children.len(), 1, "{:?}", div.children);
    }
}
