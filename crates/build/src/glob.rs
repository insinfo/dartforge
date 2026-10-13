//! The semantics of `package:glob` 2.1.3, which decides `sources`,
//! `generate_for` and `findAssets` in `build_runner`.
//!
//! Parser: `glob-2.1.3/lib/src/parser.dart`, line by line. Matching: the regex
//! of `ast.dart` (`^…$`): `*` → `[^/]*`, `**` → `[^]*` (everything, including
//! `/`), `?` → `[^/]`, ranges never match `/`, `{a,b}` → alternation. It is a
//! backtracking matcher over the tree, without a regex engine.
//!
//! **Case**: Dart's `Glob` uses the platform's path context and, on Windows,
//! is case-insensitive (`glob.dart`, `factory Glob`). The oracle runs on
//! Windows; the same applies here through `cfg!(windows)`.

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Literal(Vec<char>),
    Star,
    DoubleStar,
    Any,
    Range {
        ranges: Vec<(char, char)>,
        negated: bool,
    },
    Options(Vec<Vec<Node>>),
}

#[derive(Debug, Clone)]
pub struct Glob {
    pub pattern: String,
    nodes: Vec<Node>,
    case_sensitive: bool,
}

impl PartialEq for Glob {
    fn eq(&self, o: &Self) -> bool {
        self.pattern == o.pattern
    }
}

struct Reader<'a> {
    c: Vec<char>,
    i: usize,
    pattern: &'a str,
}

impl Reader<'_> {
    fn at_end(&self) -> bool {
        self.i >= self.c.len()
    }
    fn peek(&self, ch: char) -> bool {
        self.c.get(self.i) == Some(&ch)
    }
    fn take(&mut self, ch: char) -> bool {
        if self.peek(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn error(&self, msg: &str) -> String {
        format!(
            "invalid glob '{}': {msg} (position {})",
            self.pattern, self.i
        )
    }
    fn read(&mut self) -> Result<char, String> {
        let ch = *self
            .c
            .get(self.i)
            .ok_or_else(|| self.error("unexpected end"))?;
        self.i += 1;
        Ok(ch)
    }

    fn sequence(&mut self, in_options: bool) -> Result<Vec<Node>, String> {
        if self.at_end() {
            return Err(self.error("expected a glob."));
        }
        let mut nodes = Vec::new();
        while !self.at_end() {
            if in_options && (self.peek(',') || self.peek('}')) {
                break;
            }
            nodes.push(self.node(in_options)?);
        }
        Ok(nodes)
    }

    fn node(&mut self, in_options: bool) -> Result<Node, String> {
        if self.take('*') {
            return Ok(if self.take('*') {
                Node::DoubleStar
            } else {
                Node::Star
            });
        }
        if self.take('?') {
            return Ok(Node::Any);
        }
        if self.take('[') {
            return self.range();
        }
        if self.take('{') {
            return self.options();
        }
        self.literal(in_options)
    }

    fn range(&mut self) -> Result<Node, String> {
        if self.peek(']') {
            return Err(self.error("unexpected \"]\"."));
        }
        let negated = self.take('!') || self.take('^');
        let mut ranges = Vec::new();
        let read_char = |l: &mut Self| -> Result<char, String> {
            let ch = l.read()?;
            if negated || ch != '/' {
                Ok(ch)
            } else {
                Err(l.error("\"/\" may not be used in a range."))
            }
        };
        while !self.take(']') {
            if self.at_end() {
                return Err(self.error("expected \"]\"."));
            }
            self.take('\\');
            let ch = read_char(self)?;
            if self.take('-') {
                if self.peek(']') {
                    ranges.push((ch, ch));
                    ranges.push(('-', '-'));
                    continue;
                }
                self.take('\\');
                let end = read_char(self)?;
                if end < ch {
                    return Err(self.error("Range out of order."));
                }
                ranges.push((ch, end));
            } else {
                ranges.push((ch, ch));
            }
        }
        Ok(Node::Range { ranges, negated })
    }

    fn options(&mut self) -> Result<Node, String> {
        if self.peek('}') {
            return Err(self.error("unexpected \"}\"."));
        }
        let mut ops = Vec::new();
        loop {
            ops.push(self.sequence(true)?);
            if !self.take(',') {
                break;
            }
        }
        if ops.len() == 1 {
            return Err(self.error("expected \",\"."));
        }
        if !self.take('}') {
            return Err(self.error("expected \"}\"."));
        }
        Ok(Node::Options(ops))
    }

    fn literal(&mut self, in_options: bool) -> Result<Node, String> {
        let stops = |ch: char| {
            matches!(ch, '*' | '{' | '[' | '?' | '\\' | '}' | ']' | '(' | ')')
                || (in_options && ch == ',')
        };
        let mut t = Vec::new();
        loop {
            while let Some(&ch) = self.c.get(self.i) {
                if stops(ch) {
                    break;
                }
                t.push(ch);
                self.i += 1;
            }
            if self.take('\\') {
                t.push(self.read()?);
                continue;
            }
            break;
        }
        for ch in [']', '(', ')'] {
            if self.peek(ch) {
                return Err(self.error(&format!("unexpected \"{ch}\"")));
            }
        }
        if !in_options && self.peek('}') {
            return Err(self.error("unexpected \"}\""));
        }
        Ok(Node::Literal(t))
    }
}

fn equal(a: char, b: char, case_sensitive: bool) -> bool {
    a == b || (!case_sensitive && a.to_lowercase().eq(b.to_lowercase()))
}

fn in_range(c: char, ranges: &[(char, char)], case_sensitive: bool) -> bool {
    let inside = |x: char| ranges.iter().any(|&(a, b)| a <= x && x <= b);
    if inside(c) {
        return true;
    }
    if case_sensitive {
        return false;
    }
    c.to_lowercase().any(inside) || c.to_uppercase().any(inside)
}

impl Glob {
    pub fn new(pattern: &str) -> Result<Glob, String> {
        Self::with_case_sensitivity(pattern, !cfg!(windows))
    }

    pub fn with_case_sensitivity(pattern: &str, case_sensitive: bool) -> Result<Glob, String> {
        let mut l = Reader {
            c: pattern.chars().collect(),
            i: 0,
            pattern,
        };
        let nodes = l.sequence(false)?;
        Ok(Glob {
            pattern: pattern.to_string(),
            nodes,
            case_sensitive,
        })
    }

    /// Whether the whole path (POSIX, relative to the package) matches the
    /// pattern.
    pub fn matches(&self, path: &str) -> bool {
        let t: Vec<char> = path.chars().collect();
        let mut k = |end: usize| end == t.len();
        match_nodes(&self.nodes, 0, &t, 0, self.case_sensitive, &mut k)
    }

    /// Could some path starting with `dir/` match? Conservative (it may say
    /// yes too often): used to prune directory scanning.
    pub fn may_descend(&self, dir: &str) -> bool {
        let t: Vec<char> = format!("{dir}/").chars().collect();
        match_prefix(&self.nodes, 0, &t, 0, self.case_sensitive)
    }
}

/// Matches `nodes[i..]` against `t[j..]`, calling `k(pos)` at every possible
/// end.
fn match_nodes(
    nodes: &[Node],
    i: usize,
    t: &[char],
    j: usize,
    s: bool,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let Some(node) = nodes.get(i) else { return k(j) };
    match node {
        Node::Literal(l) => {
            if t.len() - j < l.len() || !l.iter().zip(&t[j..]).all(|(a, b)| equal(*a, *b, s)) {
                return false;
            }
            match_nodes(nodes, i + 1, t, j + l.len(), s, k)
        }
        Node::Any => j < t.len() && t[j] != '/' && match_nodes(nodes, i + 1, t, j + 1, s, k),
        Node::Range { ranges, negated } => {
            j < t.len()
                && t[j] != '/'
                && (in_range(t[j], ranges, s) != *negated)
                && match_nodes(nodes, i + 1, t, j + 1, s, k)
        }
        Node::Star => {
            // Greedy, with backtracking: the longest span without `/` first.
            let mut end = j;
            while end < t.len() && t[end] != '/' {
                end += 1;
            }
            (j..=end).rev().any(|e| match_nodes(nodes, i + 1, t, e, s, k))
        }
        Node::DoubleStar => (j..=t.len()).rev().any(|e| match_nodes(nodes, i + 1, t, e, s, k)),
        Node::Options(ops) => ops.iter().any(|op| {
            let mut cont = |e: usize| match_nodes(nodes, i + 1, t, e, s, k);
            match_nodes(op, 0, t, j, s, &mut cont)
        }),
    }
}

/// Like `match_nodes`, but accepts when the text ends before the pattern (the
/// rest of the pattern could still match what comes after `t`).
fn match_prefix(nodes: &[Node], i: usize, t: &[char], j: usize, s: bool) -> bool {
    if j == t.len() {
        return true;
    }
    let Some(node) = nodes.get(i) else { return false };
    match node {
        Node::Literal(l) => {
            let n = l.len().min(t.len() - j);
            if !l[..n]
                .iter()
                .zip(&t[j..j + n])
                .all(|(a, b)| equal(*a, *b, s))
            {
                return false;
            }
            if n < l.len() {
                return true;
            }
            match_prefix(nodes, i + 1, t, j + n, s)
        }
        Node::Any => t[j] != '/' && match_prefix(nodes, i + 1, t, j + 1, s),
        Node::Range { ranges, negated } => {
            t[j] != '/'
                && (in_range(t[j], ranges, s) != *negated)
                && match_prefix(nodes, i + 1, t, j + 1, s)
        }
        Node::Star => {
            let mut end = j;
            while end < t.len() && t[end] != '/' {
                end += 1;
            }
            end == t.len() || (j..=end).any(|e| match_prefix(nodes, i + 1, t, e, s))
        }
        Node::DoubleStar => true,
        Node::Options(ops) => ops.iter().any(|op| {
            let mut rest: Vec<Node> = op.clone();
            rest.extend_from_slice(&nodes[i + 1..]);
            match_prefix(&rest, 0, t, j, s)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(p: &str) -> Glob {
        Glob::with_case_sensitivity(p, true).unwrap()
    }

    // Cases from `glob-2.1.3/test/match_test.dart`, the relative part.
    #[test]
    fn literals_and_stars() {
        assert!(g("foo").matches("foo"));
        assert!(!g("foo").matches("foo/bar"));
        assert!(g("foo*").matches("foobar"));
        assert!(g("foo*").matches("foo"));
        assert!(!g("foo*").matches("foo/bar"));
        assert!(g("*").matches("foo"));
        assert!(!g("*").matches("foo/bar"));
        assert!(g("**").matches("foo/bar/baz"));
        assert!(g("foo/**").matches("foo/bar/baz"));
        assert!(!g("foo/**").matches("foo"));
        assert!(g("**/*.dart").matches("a/b/c.dart"));
        // `**/` requires the slash: the regex is `^[^]*/[^/]*\.dart$`.
        assert!(!g("**/*.dart").matches("c.dart"));
        assert!(g("lib/**.dart").matches("lib/a/b.dart"));
        assert!(g("lib/**.dart").matches("lib/b.dart"));
        assert!(g("README*").matches("README.md"));
        assert!(!g("README*").matches("doc/README.md"));
        assert!(g("$package$").matches("$package$"));
        assert!(g("lib/$lib$").matches("lib/$lib$"));
    }

    #[test]
    fn any_ranges_options() {
        assert!(g("fo?").matches("foo"));
        assert!(!g("fo?").matches("fo/"));
        assert!(g("[a-c]x").matches("bx"));
        assert!(!g("[a-c]x").matches("dx"));
        assert!(g("[!a-c]x").matches("dx"));
        assert!(!g("[!a-c]x").matches("/x"));
        assert!(g("{foo,bar}.dart").matches("bar.dart"));
        assert!(g("a{,/**}").matches("a"));
        assert!(g("a{,/**}").matches("a/b/c"));
        assert!(g("x[-]").matches("x-"));
        assert!(g("x[a-]").matches("x-"));
        assert!(g(r"a\*b").matches("a*b"));
        assert!(!g(r"a\*b").matches("axb"));
    }

    #[test]
    fn syntax_errors() {
        assert!(Glob::new("").is_err());
        assert!(Glob::new("{a}").is_err());
        assert!(Glob::new("a]").is_err());
        assert!(Glob::new("a(").is_err());
        assert!(Glob::new("[]").is_err());
        assert!(Glob::new("[b-a]").is_err());
        assert!(Glob::new("a}").is_err());
        assert!(Glob::new("[/]").is_err());
    }

    #[test]
    fn case_on_windows() {
        let i = Glob::with_case_sensitivity("lib/**.DART", false).unwrap();
        assert!(i.matches("lib/a.dart"));
        assert!(!g("lib/**.DART").matches("lib/a.dart"));
    }

    #[test]
    fn directory_pruning() {
        assert!(g("lib/**").may_descend("lib"));
        assert!(g("lib/**").may_descend("lib/src"));
        assert!(!g("lib/**").may_descend("web"));
        assert!(!g("README*").may_descend("doc"));
        assert!(g("**/*.dart").may_descend("x"));
        assert!(g("{lib,web}/*.dart").may_descend("web"));
        assert!(!g("{lib,web}/*.dart").may_descend("web/a"));
        assert!(!g("$package$").may_descend(".dart_tool"));
    }
}
