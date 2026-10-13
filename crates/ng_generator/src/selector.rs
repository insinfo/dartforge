//! Directive CSS selectors and matching them against template elements.
//!
//! Port of `selector.dart` and `attribute_matcher.dart` from `ngcompiler`. The
//! official compiler decides which directives an element gets by matching the
//! selector of each one (`@Directive(selector: 'form:not([ngNoForm])')`)
//! against a description of the element (`createElementCssSelector`, in
//! `template_parser.dart`): the tag name, each attribute with its value, each
//! property binding and each event as an attribute, and the classes of
//! `class="..."`.
//!
//! The generator needs the same answer for two reasons: to know that an element
//! belongs to a directive it does not emit yet (and refuse — emitting the plain
//! element compiles and does something else) and, further on, to instantiate
//! the directives it knows how to emit, in the right order.

/// A simple selector: `tag.class[attr=value]:not(...)`. A comma-separated list
/// becomes several of these.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selector {
    pub element: Option<String>,
    pub classes: Vec<String>,
    pub attributes: Vec<Attribute>,
    pub not: Vec<Selector>,
}

/// `[name]`, `[name=value]`, `[name~=value]`…
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub matching: Match,
}

/// The shapes of `attribute_matcher.dart`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match {
    /// `[name]`: it only has to exist.
    Exists,
    /// `[name=value]`, with the value already lowercase.
    Equal(Option<String>),
    /// `[name~=value]`: one of the space-separated items.
    List(String),
    /// `[name|=value]`: the value or `value-…`.
    Hyphen(String),
    /// `[name^=value]`.
    Prefix(String),
    /// `[name$=value]`.
    Suffix(String),
    /// `[name*=value]`.
    Contains(String),
}

impl Match {
    fn accepts(&self, value: Option<&str>) -> bool {
        match self {
            Match::Exists => true,
            Match::Equal(v) => v.as_deref() == value,
            // The others get the value with `!` in the official code: an
            // attribute without a value does not get here in practice; without
            // a value, it does not match.
            Match::List(v) => value.is_some_and(|x| x.split_whitespace().any(|i| i == v)),
            Match::Hyphen(v) => value.is_some_and(|x| x == v || x.starts_with(&format!("{v}-"))),
            Match::Prefix(v) => value.is_some_and(|x| x.starts_with(v.as_str())),
            Match::Suffix(v) => value.is_some_and(|x| x.ends_with(v.as_str())),
            Match::Contains(v) => value.is_some_and(|x| x.contains(v.as_str())),
        }
    }
}

/// Dart's `\w` (without unicode) plus the hyphen: `[-\w]`.
fn from_name(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Reads a `[-\w]+` name starting at `i`; returns the end.
fn name_up_to(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && from_name(b[j]) {
        j += 1;
    }
    j
}

/// Tries the attribute alternative:
/// `\[([-\w]+)(?:([~|^$*]?=)(['"]?)([^\]'"]*)\6)?\]`.
/// Returns (name, operator, value, end).
fn attribute_at(s: &str, i: usize) -> Option<(String, Option<String>, Option<String>, usize)> {
    let b = s.as_bytes();
    if b.get(i) != Some(&b'[') {
        return None;
    }
    let begin = i + 1;
    let name_end_at = name_up_to(b, begin);
    if name_end_at == begin {
        return None;
    }
    let name = s[begin..name_end_at].to_string();
    // The optional group: operator, quotes, value and the same quote.
    let optional = (|| {
        let mut j = name_end_at;
        let mut op = String::new();
        if matches!(b.get(j), Some(b'~' | b'|' | b'^' | b'$' | b'*')) {
            op.push(b[j] as char);
            j += 1;
        }
        if b.get(j) != Some(&b'=') {
            return None;
        }
        op.push('=');
        j += 1;
        let quote = match b.get(j) {
            Some(&q @ (b'\'' | b'"')) => {
                j += 1;
                Some(q)
            }
            _ => None,
        };
        let vini = j;
        while j < b.len() && !matches!(b[j], b']' | b'\'' | b'"') {
            j += 1;
        }
        let value = s[vini..j].to_string();
        if let Some(q) = quote {
            if b.get(j) != Some(&q) {
                return None;
            }
            j += 1;
        }
        Some((op, value, j))
    })();
    match optional {
        Some((op, value, j)) if b.get(j) == Some(&b']') => {
            Some((name, Some(op), Some(value), j + 1))
        }
        // Without the optional group, the `]` must come right after the name.
        _ if b.get(name_end_at) == Some(&b']') => Some((name, None, None, name_end_at + 1)),
        _ => None,
    }
}

/// The selector that receives what is read: the outer one or, inside `:not(`,
/// the last negated one.
fn target(current: &mut Selector, in_not: bool) -> &mut Selector {
    if in_not {
        current.not.last_mut().expect("`:not(` opened a selector")
    } else {
        current
    }
}

/// Whitespace, comma, whitespace: `\s*,\s*`.
fn comma_at(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if b.get(j) != Some(&b',') {
        return None;
    }
    j += 1;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    Some(j)
}

impl Selector {
    /// `CssSelector.parse`: the alternatives of the official regular
    /// expression, in the same order, and whatever matches none of them is
    /// skipped.
    pub fn parse(text: &str) -> Vec<Selector> {
        fn close(res: &mut Vec<Selector>, mut s: Selector) {
            if !s.not.is_empty()
                && s.element.is_none()
                && s.classes.is_empty()
                && s.attributes.is_empty()
            {
                s.element = Some("*".into());
            }
            res.push(s);
        }
        let b = text.as_bytes();
        let mut results = Vec::new();
        let mut current = Selector::default();
        // Inside `:not(`, the selector that receives is the last one of
        // `current.not`.
        let mut in_not = false;
        let mut i = 0;
        while i < b.len() {
            // 1. `:not(`
            if b[i..].starts_with(b":not(") {
                in_not = true;
                current.not.push(Selector::default());
                i += 5;
                continue;
            }
            // 2. tag name
            if from_name(b[i]) {
                let f = name_up_to(b, i);
                target(&mut current, in_not).element = Some(text[i..f].to_string());
                i = f;
                continue;
            }
            // 3. `.class`
            if b[i] == b'.' && i + 1 < b.len() && from_name(b[i + 1]) {
                let f = name_up_to(b, i + 1);
                target(&mut current, in_not)
                    .classes
                    .push(text[i + 1..f].to_ascii_lowercase());
                i = f;
                continue;
            }
            // 4. `[attribute]`
            if let Some((name, op, value, f)) = attribute_at(text, i) {
                let value = value.map(|v| v.to_lowercase());
                let matching = match op.as_deref() {
                    None => Some(Match::Exists),
                    Some("=") => Some(Match::Equal(value)),
                    Some(o) => {
                        let v = value.unwrap_or_default();
                        // With an empty value these match nothing, and the
                        // official code does not even register them.
                        if v.is_empty() {
                            None
                        } else {
                            match o {
                                "~=" => Some(Match::List(v)),
                                "|=" => Some(Match::Hyphen(v)),
                                "^=" => Some(Match::Prefix(v)),
                                "$=" => Some(Match::Suffix(v)),
                                _ => Some(Match::Contains(v)),
                            }
                        }
                    }
                };
                if let Some(matching) = matching {
                    target(&mut current, in_not)
                        .attributes
                        .push(Attribute { name, matching });
                }
                i = f;
                continue;
            }
            // 5. `)`
            if b[i] == b')' {
                in_not = false;
                i += 1;
                continue;
            }
            // 6. `,`
            if let Some(f) = comma_at(b, i) {
                close(&mut results, std::mem::take(&mut current));
                in_not = false;
                i = f;
                continue;
            }
            i += 1;
        }
        close(&mut results, current);
        results
    }

    /// Is the selector just a tag name (`li-modal`)? It is the shape the child
    /// component emitter knows how to match by name.
    pub fn tag_only(&self) -> Option<&str> {
        match self {
            Selector {
                element: Some(e),
                classes,
                attributes,
                not,
            } if classes.is_empty() && attributes.is_empty() && not.is_empty() && e != "*" => {
                Some(e)
            }
            _ => None,
        }
    }

    /// `SelectorMatcher.match` for a single selector, against the description
    /// of an element. The rules the official matching tree produces: `*` alone
    /// matches any element; `*` with a class or attribute matches nothing (the
    /// partial is looked up by tag name); each class and each attribute must be
    /// present; no `:not(...)` may match.
    pub fn matches(&self, el: &Element) -> bool {
        let simple = self.classes.is_empty() && self.attributes.is_empty();
        match self.element.as_deref() {
            Some("*") if simple => {}
            Some("*") => return false,
            Some(e) if e != el.name => return false,
            _ => {}
        }
        if self.element.is_none() && simple {
            // Empty selector: does not go into any map.
            return false;
        }
        if !self.classes.iter().all(|c| el.classes.contains(c)) {
            return false;
        }
        let attributes_ok = self.attributes.iter().all(|a| {
            el.attributes
                .iter()
                .any(|(n, v)| *n == a.name && a.matching.accepts(v.as_deref()))
        });
        if !attributes_ok {
            return false;
        }
        !self.not.iter().any(|n| n.matches_as_not(el))
    }

    /// A `:not(...)` selector is matched by a separate `SelectorMatcher`,
    /// without the `*` rule that `addResult` applies.
    fn matches_as_not(&self, el: &Element) -> bool {
        let simple = self.classes.is_empty() && self.attributes.is_empty();
        if simple && self.element.is_none() {
            return false;
        }
        Selector {
            element: self.element.clone(),
            classes: self.classes.clone(),
            attributes: self.attributes.clone(),
            not: Vec::new(),
        }
        .matches(el)
    }
}

/// The description of an element for matching (`createElementCssSelector`):
/// name, attributes with value (lowercase) and classes.
#[derive(Debug, Clone, Default)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, Option<String>)>,
    pub classes: Vec<String>,
}

impl Element {
    pub fn new(name: &str) -> Self {
        Element {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// `cssSelector.addAttribute(name, '=', value)`, and the classes when the
    /// attribute is `class`.
    pub fn attribute(&mut self, name: &str, value: Option<&str>) {
        let value = value.map(|v| v.to_lowercase());
        if name.eq_ignore_ascii_case("class")
            && let Some(v) = &value
        {
            self.classes
                .extend(v.split_whitespace().map(str::to_string));
        }
        self.attributes.push((name.to_string(), value));
    }

    /// The description of a template element, as `_elementSelector` builds it:
    /// attributes, then properties (the `[(x)]` become the property `x` and the
    /// event `xChange`, like ngast's `DesugarVisitor`), then events.
    pub fn of_template(e: &crate::html::Element) -> Self {
        let mut el = Element::new(&e.name);
        for a in &e.attributes {
            el.attribute(&a.name, Some(&a.value));
        }
        for p in e.properties.iter().chain(&e.bananas) {
            el.attribute(&p.name, Some(&p.value));
        }
        for ev in &e.events {
            el.attribute(&ev.name, Some(&ev.value));
        }
        for b in &e.bananas {
            el.attribute(
                &format!("{}Change", b.name),
                Some(&format!("{} = $event", b.value)),
            );
        }
        el
    }
}

/// Does some selector of the list match?
pub fn matches_any(selectors: &[Selector], el: &Element) -> bool {
    selectors.iter().any(|s| s.matches(el))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(name: &str, attrs: &[(&str, Option<&str>)]) -> Element {
        let mut e = Element::new(name);
        for (n, v) in attrs {
            e.attribute(n, *v);
        }
        e
    }

    #[test]
    fn parses_the_official_shapes() {
        let s = Selector::parse("form:not([ngNoForm]):not([ngFormModel]),ngForm,[ngForm]");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].element.as_deref(), Some("form"));
        assert_eq!(s[0].not.len(), 2);
        assert_eq!(s[0].not[0].attributes[0].name, "ngNoForm");
        assert_eq!(s[1].element.as_deref(), Some("ngForm"));
        assert_eq!(s[2].attributes[0].name, "ngForm");
        let s = Selector::parse("input:not([type=checkbox])[ngControl]");
        assert_eq!(
            s[0].not[0].attributes[0].matching,
            Match::Equal(Some("checkbox".into()))
        );
        assert_eq!(s[0].attributes[0].name, "ngControl");
        let s = Selector::parse("[ngFor][ngForOf]");
        assert_eq!(s[0].attributes.len(), 2);
    }

    #[test]
    fn simple_form_matches_ng_form() {
        let s = Selector::parse("form:not([ngNoForm]):not([ngFormModel]),ngForm,[ngForm]");
        assert!(matches_any(&s, &el("form", &[])));
        assert!(matches_any(&s, &el("form", &[("class", Some("x"))])));
        assert!(!matches_any(&s, &el("form", &[("ngNoForm", None)])));
        assert!(!matches_any(&s, &el("div", &[])));
    }

    #[test]
    fn option_and_input_with_ng_model() {
        assert!(matches_any(&Selector::parse("option"), &el("option", &[])));
        let s = Selector::parse(
            "input:not([type=checkbox])[ngControl],textarea[ngControl],input:not([type=checkbox])[ngModel],[ngDefaultControl]",
        );
        assert!(matches_any(&s, &el("input", &[("ngModel", Some("x"))])));
        assert!(!matches_any(
            &s,
            &el(
                "input",
                &[("type", Some("checkbox")), ("ngModel", Some("x"))]
            )
        ));
        assert!(!matches_any(&s, &el("input", &[])));
    }

    #[test]
    fn class_and_star() {
        let s = Selector::parse(".btn");
        assert!(matches_any(&s, &el("a", &[("class", Some("x BTN"))])));
        assert!(!matches_any(&s, &el("a", &[])));
        assert_eq!(Selector::parse("li-modal")[0].tag_only(), Some("li-modal"));
        assert_eq!(Selector::parse("[x]")[0].tag_only(), None);
    }
}
