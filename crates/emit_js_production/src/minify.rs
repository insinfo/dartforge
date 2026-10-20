//! Compaction of the production file (step 7 of the plan, the safe part).
//!
//! Removes comments and the whitespace that does not separate tokens,
//! without touching any name: nothing a program observes changes. Renaming
//! locals and properties (`docs/JS-PRODUCAO.md` §3) is left out — see the end
//! of §3 there.
//!
//! The rules, each with the reason that makes it safe:
//!
//! * **line breaks stay**: a run of whitespace containing a line break (or a
//!   block comment containing one) becomes a single line break. Automatic
//!   semicolon insertion (`return`↵`x`, `a`↵`++b`) depends on the line break
//!   and not on the rest of the whitespace, so the meaning is the same;
//! * **whitespace between tokens only where it separates**: between two word
//!   characters (`var x`, `return 1`), between `+ +` and `- -` (`a + +b` is
//!   not `a++b`), between `/` and `/` or `*` (it does not open a comment),
//!   between `<` and `!` and between `-` and `>` (`<!--` and `-->` are HTML
//!   comments in scripts), and between a number and `.` (`1 .x` is not
//!   `1.x`);
//! * **literals untouched**: strings, *template literals* (with the `${…}`
//!   expressions inside) and regular expressions are copied byte for byte. A
//!   slash is a regex at the start of an expression (after punctuation that
//!   does not close an expression and after the keywords that expect one,
//!   `return`/`typeof`/…) and a division after a word, number, literal, `)`,
//!   `]` and `++`/`--`; a "regex" that reaches the end of the line without
//!   closing goes back to being a division.
//!
//! The output is deterministic (one pass, no table) and has the same tokens
//! in the same order as the input — that is what the test checks, besides
//! the whole corpus run with the compacted file.

/// Kind of the last significant token, to decide regex vs. division and the
/// whitespace between tokens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Last {
    Nothing,
    /// Word: identifier, keyword or number. `regex_after` says whether it is
    /// a keyword after which an expression comes; `number`, whether it is a
    /// numeric literal.
    Word {
        regex_after: bool,
        number: bool,
    },
    /// String, template or regex.
    Literal,
    /// Punctuation: the character, and whether it forms `++`/`--` with the
    /// previous one.
    Punct {
        c: u8,
        doubled: bool,
    },
}

/// Pending whitespace between the last token and the next one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Space {
    Zero,
    Blank,
    Newline,
}

fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 || c == b'\\'
}

const KEYWORDS_BEFORE_EXPRESSION: &[&[u8]] = &[
    b"return",
    b"typeof",
    b"instanceof",
    b"in",
    b"of",
    b"new",
    b"delete",
    b"void",
    b"throw",
    b"case",
    b"do",
    b"else",
    b"yield",
    b"await",
];

/// Compacts `src` (JS) according to the rules of the module.
pub fn compact(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut last = Last::Nothing;
    let mut pending = Space::Zero;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Whitespace and line breaks.
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' || c == 0x0b || c == 0x0c {
            let q = if c == b'\n' || c == b'\r' {
                Space::Newline
            } else {
                Space::Blank
            };
            pending = pending.max(q);
            i += 1;
            continue;
        }
        // Comments.
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            pending = pending.max(Space::Blank);
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = src[i + 2..]
                .find("*/")
                .map(|k| i + 2 + k + 2)
                .unwrap_or(b.len());
            let with_newline = b[i..end].iter().any(|&x| x == b'\n' || x == b'\r');
            pending = pending.max(if with_newline {
                Space::Newline
            } else {
                Space::Blank
            });
            i = end;
            continue;
        }
        // The next token: [i, end) and its kind.
        let (end, kind) = if c == b'"' || c == b'\'' {
            (string_end(b, i), Last::Literal)
        } else if c == b'`' {
            (template_end(b, i), Last::Literal)
        } else if c == b'/' && regex_allowed(last) {
            match regex_end(b, i) {
                Some(f) => (f, Last::Literal),
                None => (i + 1, Last::Punct { c, doubled: false }),
            }
        } else if is_word(c) {
            let number =
                c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit));
            let f = if number {
                number_end(b, i)
            } else {
                word_end(b, i)
            };
            let regex_after = !number && KEYWORDS_BEFORE_EXPRESSION.contains(&&b[i..f]);
            (
                f,
                Last::Word {
                    regex_after,
                    number,
                },
            )
        } else if c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            (
                number_end(b, i),
                Last::Word {
                    regex_after: false,
                    number: true,
                },
            )
        } else {
            let doubled = (c == b'+' || c == b'-')
                && matches!(last, Last::Punct { c: d, doubled: false } if d == c)
                && pending == Space::Zero;
            (i + 1, Last::Punct { c, doubled })
        };
        match pending {
            Space::Newline if !out.is_empty() => out.push(b'\n'),
            Space::Blank if needs_space(last, out.last().copied(), b[i]) => out.push(b' '),
            _ => {}
        }
        out.extend_from_slice(&b[i..end]);
        pending = Space::Zero;
        last = kind;
        i = end;
    }
    if !out.is_empty() && src.ends_with('\n') {
        out.push(b'\n');
    }
    // Only input bytes, cut at ASCII token boundaries: valid UTF-8.
    String::from_utf8(out).expect("compaction only cuts at ASCII boundaries")
}

fn regex_allowed(u: Last) -> bool {
    match u {
        Last::Nothing => true,
        Last::Word { regex_after, .. } => regex_after,
        Last::Literal => false,
        Last::Punct { c, doubled } => !doubled && !matches!(c, b')' | b']'),
    }
}

/// Whether the whitespace between the previous token (which ends in `a`) and
/// the next one (which starts with `b`) must stay.
fn needs_space(last: Last, a: Option<u8>, b: u8) -> bool {
    let Some(a) = a else { return false };
    if is_word(a) && is_word(b) {
        return true;
    }
    if matches!(last, Last::Word { number: true, .. }) && b == b'.' {
        return true;
    }
    matches!(
        (a, b),
        (b'+', b'+') | (b'-', b'-') | (b'/', b'/') | (b'/', b'*') | (b'<', b'!') | (b'-', b'>')
    )
}

fn word_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && is_word(b[i]) {
        // `\uXXXX` in an identifier: the whole escape belongs to the word.
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    i.min(b.len())
}

fn number_end(b: &[u8], mut i: usize) -> usize {
    let hex = b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(b'x' | b'X'));
    while i < b.len() {
        let c = b[i];
        // The sign only belongs to the number right after the exponent
        // (`1e-5`).
        let exponent_sign =
            (c == b'+' || c == b'-') && !hex && i > 0 && matches!(b[i - 1], b'e' | b'E');
        if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || exponent_sign {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn string_end(b: &[u8], i: usize) -> usize {
    let quote = b[i];
    let mut j = i + 1;
    while j < b.len() && b[j] != quote {
        if b[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(b.len())
}

/// End of a *template literal* that starts at `i`, with the `${…}`
/// expressions (which may contain strings, templates, comments and braces).
fn template_end(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'`' => return j + 1,
            b'$' if b.get(j + 1) == Some(&b'{') => j = template_expression_end(b, j + 2),
            _ => j += 1,
        }
    }
    b.len()
}

/// End (after the `}`) of the template expression that starts at `i`.
fn template_expression_end(b: &[u8], mut i: usize) -> usize {
    let mut depth = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = string_end(b, i),
            b'`' => i = template_end(b, i),
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                if depth == 0 {
                    return i + 1;
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// End of a regex that starts at `i` (with its flags), or `None` if it does
/// not close before the end of the line (so the slash was a division).
fn regex_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    let mut class = false;
    // `//` was already read as a comment; `/` followed by `*` too.
    loop {
        let c = *b.get(j)?;
        match c {
            b'\n' | b'\r' => return None,
            b'\\' => j += 1,
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => break,
            _ => {}
        }
        j += 1;
    }
    j += 1;
    while j < b.len() && (b[j].is_ascii_alphabetic()) {
        j += 1;
    }
    Some(j)
}

/// Marker that `own_sdk::assemble` puts right after the SDK module: what
/// comes before it (the *bootstrap* and the SDK) is the runtime, whose
/// *strings* are all potential names; what comes after is the program.
/// Compression deletes it (an expression without effect).
pub const SDK_END_MARKER: &str = "dartforge:sdk-end";

/// Runtime functions (`dart.X`) that receive *strings* that are **not**
/// property names: symbol descriptions, messages, constant values, *record*
/// shapes. The other `dart.X` calls (`dsend`, `dload`, `bind`,
/// `defineExtensionMethods`, the signatures…) treat their *strings* as names.
const DART_WITHOUT_NAMES: &[&str] = &[
    "privateName",
    "const",
    "constList",
    "constMap",
    "constSet",
    "nullCheck",
    "throw",
    "fn",
    "equals",
    "toString",
    "str",
    "strSafe",
    "hashCode",
    "identical",
    "lateFields",
    "recordLiteral",
    "registerExtension",
    "jsInteropNullCheck",
    "checkNativeNonNull",
];

/// JavaScript methods whose *string* is a property name
/// (`Object.defineProperty(o, "x", …)`, `o.hasOwnProperty("x")`).
const JS_WITH_NAMES: &[&str] = &[
    "defineProperty",
    "getOwnPropertyDescriptor",
    "hasOwnProperty",
    "propertyIsEnumerable",
    "get",
    "has",
    "set",
    "deleteProperty",
];

/// Identifier-shaped literal *strings* that reach run time as a **property
/// name** and therefore cannot be renamed (`docs/JS-PRODUCAO-TAMANHO.md`
/// §9.1):
///
/// * all of the runtime's (the *bootstrap* and the SDK, before
///   [`SDK_END_MARKER`]): `dart:_runtime` and the `JS()` *templates* index
///   objects by *strings* in many ways;
/// * in the program, only those in name position: inside the arguments of a
///   `dart.X` call (outside [`DART_WITHOUT_NAMES`]) or `js_util.X`, on the
///   left of `in`, and in the arguments of [`JS_WITH_NAMES`].
///
/// A program data *string* (a JSON key, a column name, a CSS class, the
/// message of a `LateError`) does not reserve the name: in the DDC contract
/// it never becomes a Dart member access without going through one of those
/// places. `DARTFORGE_JSPROD_STRINGS=all` goes back to the old rule (every
/// *string*).
struct IdentifierStrings {
    names: std::collections::HashSet<String>,
    /// Before compression, literal keys (`static ["new"]()`, `x["foo"]`,
    /// `{"a": 1}`) do not reserve: compression replaces them with the
    /// unquoted form, which the renaming treats like the others. Whatever is
    /// still quoted after it reserves (the second visit).
    before: bool,
    /// Start of the SDK end marker in the source (`u32::MAX` without a
    /// marker: everything is runtime, the old rule).
    sdk_end: u32,
    /// For each open call, whether the *strings* inside it are names.
    stack: Vec<bool>,
}

impl<'a> oxc_ast_visit::Visit<'a> for IdentifierStrings {
    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        if self.is_name(it.span.start) {
            self.record(it.value.as_str());
        }
    }
    fn visit_template_literal(&mut self, it: &oxc_ast::ast::TemplateLiteral<'a>) {
        if self.is_name(it.span.start) {
            for q in &it.quasis {
                if let Some(c) = &q.value.cooked {
                    self.record(c.as_str());
                }
            }
        }
        oxc_ast_visit::walk::walk_template_literal(self, it);
    }
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        // `dart.bind(o, "m")` does not reserve: the *string* is rewritten
        // along with the property (`rewrite_binds`).
        let context = call_takes_names(&it.callee) && !is_bind(&it.callee);
        // The context applies to the arguments and to what is inside them
        // (`() => ["a"]` of static signatures, `[…]` of
        // `defineExtensionMethods`), until the next call, `new` or function
        // body, which open their own.
        self.stack.push(false);
        self.visit_expression(&it.callee);
        self.stack.pop();
        self.stack.push(context);
        for a in &it.arguments {
            self.visit_argument(a);
        }
        self.stack.pop();
    }
    fn visit_new_expression(&mut self, it: &oxc_ast::ast::NewExpression<'a>) {
        self.stack.push(false);
        oxc_ast_visit::walk::walk_new_expression(self, it);
        self.stack.pop();
    }
    fn visit_property_key(&mut self, it: &oxc_ast::ast::PropertyKey<'a>) {
        if self.before && matches!(it, oxc_ast::ast::PropertyKey::StringLiteral(_)) {
            return;
        }
        oxc_ast_visit::walk::walk_property_key(self, it);
    }
    fn visit_computed_member_expression(
        &mut self,
        it: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) {
        if self.before && matches!(it.expression, oxc_ast::ast::Expression::StringLiteral(_)) {
            self.visit_expression(&it.object);
            return;
        }
        oxc_ast_visit::walk::walk_computed_member_expression(self, it);
    }
    fn visit_variable_declarator(&mut self, it: &oxc_ast::ast::VariableDeclarator<'a>) {
        // The deduplication's `var t$S… = "…"` stay at the top of the file,
        // before the marker, but only carry data *strings*: those in name
        // position are not deduplicated (`StringOccurrences`).
        if it
            .id
            .get_binding_identifier()
            .is_some_and(|b| b.name.starts_with("t$S"))
        {
            return;
        }
        oxc_ast_visit::walk::walk_variable_declarator(self, it);
    }
    fn visit_function_body(&mut self, it: &oxc_ast::ast::FunctionBody<'a>) {
        // A function body with statements (the `defineLazy` *getters*, the
        // methods): what it does is not an argument of the call. An
        // expression arrow (`() => ["a"]`) has no statements and inherits.
        let expression = it.statements.len() == 1
            && matches!(
                it.statements[0],
                oxc_ast::ast::Statement::ExpressionStatement(_)
            )
            && it.directives.is_empty();
        let inherited = self.stack.last().copied().unwrap_or(false);
        self.stack.push(expression && inherited);
        oxc_ast_visit::walk::walk_function_body(self, it);
        self.stack.pop();
    }
    fn visit_binary_expression(&mut self, it: &oxc_ast::ast::BinaryExpression<'a>) {
        if it.operator == oxc_syntax::operator::BinaryOperator::In
            && let oxc_ast::ast::Expression::StringLiteral(s) = &it.left
        {
            self.record(s.value.as_str());
        }
        oxc_ast_visit::walk::walk_binary_expression(self, it);
    }
}

impl IdentifierStrings {
    fn new(sdk_end: u32) -> Self {
        IdentifierStrings {
            names: std::collections::HashSet::new(),
            before: true,
            sdk_end,
            stack: Vec::new(),
        }
    }
    fn is_name(&self, start: u32) -> bool {
        start < self.sdk_end || self.stack.last().copied().unwrap_or(false)
    }
    fn record(&mut self, v: &str) {
        let mut cs = v.chars();
        let ok = cs
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
            && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
        if ok && !self.names.contains(v) {
            self.names.insert(v.to_string());
        }
    }
}

/// Start, in the source, of the [`SDK_END_MARKER`] *string* (`u32::MAX`
/// without it).
pub(crate) fn marker_start(program: &oxc_ast::ast::Program<'_>) -> u32 {
    use oxc_ast::ast::{Expression, Statement};
    fn find(stmts: &[Statement<'_>]) -> Option<u32> {
        for s in stmts {
            if let Statement::ExpressionStatement(e) = s {
                match &e.expression {
                    Expression::StringLiteral(l) if l.value == SDK_END_MARKER => {
                        return Some(l.span.start);
                    }
                    // The `(function () { … })();` wrapper of the whole file.
                    Expression::CallExpression(c) => {
                        if let Expression::ParenthesizedExpression(p) = &c.callee
                            && let Expression::FunctionExpression(f) = &p.expression
                            && let Some(b) = &f.body
                            && let Some(x) = find(&b.statements)
                        {
                            return Some(x);
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }
    if std::env::var("DARTFORGE_JSPROD_STRINGS").is_ok_and(|v| v == "all") {
        return u32::MAX;
    }
    find(&program.body).unwrap_or(u32::MAX)
}

/// Literal *strings* in expression position (not object keys nor
/// directives), with the source span of each occurrence.
struct StringOccurrences {
    map: std::collections::HashMap<String, Vec<(u32, u32)>>,
    /// Start of the SDK end marker ([`SDK_END_MARKER`]).
    sdk_end: u32,
    /// As in [`IdentifierStrings`]: whether the *strings* of the open call
    /// are names.
    stack: Vec<bool>,
}

impl StringOccurrences {
    fn record(&mut self, it: &oxc_ast::ast::StringLiteral<'_>) {
        self.map
            .entry(it.value.to_string())
            .or_default()
            .push((it.span.start, it.span.end));
    }
}

impl<'a> oxc_ast_visit::Visit<'a> for StringOccurrences {
    fn visit_string_literal(&mut self, it: &oxc_ast::ast::StringLiteral<'a>) {
        // Identifier-shaped, it stays literal where it is (or may be) a
        // property name: in the runtime, in name position (the renaming's
        // reservation reads it there) and as the key of `x["_n$1c"]` or of
        // `in` (the visits below), which compression replaces with
        // `x._n$1c`.
        if is_identifier(it.value.as_str())
            && (it.span.start < self.sdk_end || self.stack.last().copied().unwrap_or(false))
        {
            return;
        }
        self.record(it);
    }
    fn visit_computed_member_expression(
        &mut self,
        it: &oxc_ast::ast::ComputedMemberExpression<'a>,
    ) {
        self.visit_expression(&it.object);
        match &it.expression {
            oxc_ast::ast::Expression::StringLiteral(l) if is_identifier(l.value.as_str()) => {}
            e => self.visit_expression(e),
        }
    }
    fn visit_binary_expression(&mut self, it: &oxc_ast::ast::BinaryExpression<'a>) {
        if it.operator == oxc_syntax::operator::BinaryOperator::In
            && let oxc_ast::ast::Expression::StringLiteral(l) = &it.left
            && is_identifier(l.value.as_str())
        {
            self.visit_expression(&it.right);
            return;
        }
        oxc_ast_visit::walk::walk_binary_expression(self, it);
    }
    fn visit_call_expression(&mut self, it: &oxc_ast::ast::CallExpression<'a>) {
        self.stack.push(false);
        self.visit_expression(&it.callee);
        self.stack.pop();
        self.stack.push(call_takes_names(&it.callee));
        for a in &it.arguments {
            self.visit_argument(a);
        }
        self.stack.pop();
    }
    fn visit_new_expression(&mut self, it: &oxc_ast::ast::NewExpression<'a>) {
        self.stack.push(false);
        oxc_ast_visit::walk::walk_new_expression(self, it);
        self.stack.pop();
    }
    fn visit_function_body(&mut self, it: &oxc_ast::ast::FunctionBody<'a>) {
        let expression = it.statements.len() == 1
            && matches!(
                it.statements[0],
                oxc_ast::ast::Statement::ExpressionStatement(_)
            )
            && it.directives.is_empty();
        let inherited = self.stack.last().copied().unwrap_or(false);
        self.stack.push(expression && inherited);
        oxc_ast_visit::walk::walk_function_body(self, it);
        self.stack.pop();
    }
    fn visit_property_key(&mut self, it: &oxc_ast::ast::PropertyKey<'a>) {
        if matches!(it, oxc_ast::ast::PropertyKey::StringLiteral(_)) {
            return;
        }
        oxc_ast_visit::walk::walk_property_key(self, it);
    }
    fn visit_directive(&mut self, _it: &oxc_ast::ast::Directive<'a>) {}
}

fn is_identifier(v: &str) -> bool {
    let mut cs = v.chars();
    cs.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Whether the *strings* in the arguments of a call to `callee` are property
/// names (see [`IdentifierStrings`]).
/// `dart.bind`/`dart.bindCall` (or their flattened forms): the *tearoff*,
/// which reads the method and the signature by the property name.
fn is_bind(callee: &oxc_ast::ast::Expression<'_>) -> bool {
    use oxc_ast::ast::Expression;
    let name = match callee {
        Expression::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(o) if o.name == "dart") => {
            Some(m.property.name.as_str())
        }
        Expression::Identifier(i) => crate::flatten::member(i.name.as_str(), "dart"),
        _ => None,
    };
    matches!(name, Some("bind" | "bindCall"))
}

/// After property renaming, the name *string* in the program's `dart.bind(o,
/// "m")` becomes the property's new name: `bind` reads `o[name]` and the
/// signature `sig[name]`, and both were renamed
/// (`docs/JS-PRODUCAO-TAMANHO.md` §9.7). *Tearoff* equality compares the
/// receiver and the method, not the name.
fn rewrite_binds<'a>(
    program: &mut oxc_ast::ast::Program<'a>,
    map: &std::collections::HashMap<String, String>,
    allocator: &'a oxc_allocator::Allocator,
) {
    use oxc_ast_visit::VisitMut;
    struct R<'m, 'a> {
        map: &'m std::collections::HashMap<String, String>,
        allocator: &'a oxc_allocator::Allocator,
    }
    impl<'a> VisitMut<'a> for R<'_, 'a> {
        fn visit_call_expression(&mut self, it: &mut oxc_ast::ast::CallExpression<'a>) {
            if is_bind(&it.callee) {
                for a in it.arguments.iter_mut().skip(1) {
                    if let oxc_ast::ast::Argument::StringLiteral(l) = a
                        && let Some(new) = self.map.get(l.value.as_str())
                    {
                        l.value = self.allocator.alloc_str(new).into();
                        l.raw = None;
                    }
                }
            }
            oxc_ast_visit::walk_mut::walk_call_expression(self, it);
        }
    }
    R { map, allocator }.visit_program(program);
}

fn call_takes_names(callee: &oxc_ast::ast::Expression<'_>) -> bool {
    use oxc_ast::ast::Expression;
    match callee {
        Expression::StaticMemberExpression(m) => {
            let prop = m.property.name.as_str();
            match &m.object {
                Expression::Identifier(o) if o.name == "dart" => {
                    !DART_WITHOUT_NAMES.contains(&prop)
                }
                Expression::Identifier(o) if o.name == "js_util" => true,
                _ => JS_WITH_NAMES.contains(&prop),
            }
        }
        // The named-parameter prologue of `shapes.rs`: `t$O(o, "k", d)` reads
        // `o[k]`.
        Expression::Identifier(i) if i.name == "t$O" => true,
        // The same ones, already flattened (`dart$$dsend`, `flatten.rs`).
        Expression::Identifier(i) => {
            if let Some(x) = crate::flatten::member(i.name.as_str(), "dart") {
                !DART_WITHOUT_NAMES.contains(&x)
            } else {
                crate::flatten::member(i.name.as_str(), "js_util").is_some()
            }
        }
        _ => false,
    }
}

/// *String* deduplication (`docs/JS-PRODUCAO-TAMANHO.md` §3.4): every
/// repeated *string* that pays off becomes a `var t$S…` at the top of the
/// file's IIFE, and each occurrence in expression position cites the
/// variable instead (the rti recipes `t$R("core|int")`, the `dsend` names,
/// the messages). The value stays literal in the declaration: the name
/// reservation of the property renaming sees it.
fn dedup_strings(source: &str) -> Result<String, String> {
    use oxc_ast_visit::Visit;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let mut oc = StringOccurrences {
        map: std::collections::HashMap::new(),
        sdk_end: marker_start(&parsed.program),
        stack: Vec::new(),
    };
    oc.visit_program(&parsed.program);
    // It pays off when the `n` copies cost more than the declaration and the
    // `n` references (≈ 3 bytes each after minification).
    let mut chosen: Vec<(String, Vec<(u32, u32)>)> = oc
        .map
        .into_iter()
        .filter(|(_, v)| {
            let n = v.len();
            let size = (v[0].1 - v[0].0) as usize;
            n >= 2 && size * n > size + 8 + 3 * n
        })
        .collect();
    chosen.sort_by(|a, b| a.0.cmp(&b.0));
    let head = "(function () {\n";
    if chosen.is_empty() || !source.starts_with(head) {
        return Ok(source.to_string());
    }
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut decl = String::new();
    for (i, (_, v)) in chosen.iter().enumerate() {
        let name = format!("t$S{i}");
        let (a, b) = v[0];
        decl.push_str(if decl.is_empty() { "var " } else { ",\n" });
        decl.push_str(&format!("{name} = {}", &source[a as usize..b as usize]));
        for &(a, b) in v {
            edits.push((a as usize, b as usize, name.clone()));
        }
    }
    decl.push_str(";\n");
    edits.sort_by_key(|t| t.0);
    let mut out = String::with_capacity(source.len());
    out.push_str(head);
    out.push_str(&decl);
    let mut pos = head.len();
    for (a, b, name) in edits {
        if a < pos {
            continue;
        }
        out.push_str(&source[pos..a]);
        out.push_str(&name);
        pos = b;
    }
    out.push_str(&source[pos..]);
    Ok(out)
}

/// Property names that every JavaScript object (or function, *array*,
/// *string*) already has: no renamed member may land on them.
const JS_PROTOTYPE_NAMES: &[&str] = &[
    "constructor",
    "prototype",
    "__proto__",
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "toString",
    "toLocaleString",
    "valueOf",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "name",
    "length",
    "call",
    "apply",
    "bind",
    "arguments",
    "caller",
    "at",
    "of",
    "is",
    "big",
    "sub",
    "sup",
    "map",
    "pop",
    "raw",
    "now",
    "get",
    "set",
    "has",
    "add",
    "keys",
    "fill",
    "find",
    "flat",
    "join",
    "push",
    "some",
    "sort",
    "trim",
    "link",
    "bold",
    "anchor",
    "blink",
    "fixed",
    "small",
    "strike",
    "fontsize",
    "fontcolor",
    "italics",
    "then",
    "catch",
    "finally",
    "next",
    "done",
    "value",
    "return",
    "throw",
];

/// Counts the program's properties and gives each name in `renamed` a short
/// name: the most used gets the smallest (`base54`, `oxc`'s order). No
/// target coincides with a name that **stays** in the file (a property not
/// renamed, a quoted key, `extra_taken`); a renamed name may become another's
/// target, because all of its occurrences change. Deterministic: ties broken
/// by name order.
///
/// Two renamed names share a target only when [`crate::properties::Names`]
/// says no object can carry both (they never meet in a cohabitation group,
/// and neither is solitary): the field slots of `dart2js`, which make the
/// views of different libraries read alike (`docs/JS-PRODUCAO-TAMANHO.md`
/// §13.1). Without groups every name gets its own target.
fn assign_names(
    program: &oxc_ast::ast::Program<'_>,
    renamed: &std::collections::HashSet<String>,
    extra_taken: &[&str],
    names: &crate::properties::Names,
) -> Vec<(String, String)> {
    use oxc_ast_visit::Visit;
    struct Count(std::collections::HashMap<String, u32>);
    impl Count {
        fn one(&mut self, n: &str) {
            *self.0.entry(n.to_string()).or_default() += 1;
        }
    }
    impl<'a> oxc_ast_visit::Visit<'a> for Count {
        fn visit_static_member_expression(
            &mut self,
            it: &oxc_ast::ast::StaticMemberExpression<'a>,
        ) {
            self.one(it.property.name.as_str());
            oxc_ast_visit::walk::walk_static_member_expression(self, it);
        }
        fn visit_computed_member_expression(
            &mut self,
            it: &oxc_ast::ast::ComputedMemberExpression<'a>,
        ) {
            if let oxc_ast::ast::Expression::StringLiteral(l) = &it.expression {
                self.one(l.value.as_str());
            }
            oxc_ast_visit::walk::walk_computed_member_expression(self, it);
        }
        fn visit_property_key(&mut self, it: &oxc_ast::ast::PropertyKey<'a>) {
            match it {
                oxc_ast::ast::PropertyKey::StaticIdentifier(i) => self.one(i.name.as_str()),
                oxc_ast::ast::PropertyKey::StringLiteral(l) => self.one(l.value.as_str()),
                _ => {}
            }
            oxc_ast_visit::walk::walk_property_key(self, it);
        }
        fn visit_assignment_target_property_identifier(
            &mut self,
            it: &oxc_ast::ast::AssignmentTargetPropertyIdentifier<'a>,
        ) {
            self.one(it.binding.name.as_str());
            oxc_ast_visit::walk::walk_assignment_target_property_identifier(self, it);
        }
        fn visit_binary_expression(&mut self, it: &oxc_ast::ast::BinaryExpression<'a>) {
            if it.operator == oxc_syntax::operator::BinaryOperator::In
                && let oxc_ast::ast::Expression::StringLiteral(l) = &it.left
            {
                self.one(l.value.as_str());
            }
            oxc_ast_visit::walk::walk_binary_expression(self, it);
        }
    }
    let mut c = Count(std::collections::HashMap::new());
    c.visit_program(program);
    let mut taken: std::collections::HashSet<&str> =
        c.0.keys()
            .map(String::as_str)
            .filter(|n| !renamed.contains(*n))
            .collect();
    taken.extend(extra_taken.iter().copied());
    let mut order: Vec<(&String, u32)> = renamed
        .iter()
        .map(|n| (n, c.0.get(n).copied().unwrap_or(0)))
        .collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    // The groups of each shareable name (renamed, not solitary).
    let mut groups_of: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (g, members) in names.groups.iter().enumerate() {
        for m in members {
            if renamed.contains(m) && !names.solitary.contains(m) {
                groups_of.entry(m.as_str()).or_default().push(g);
            }
        }
    }
    // The valid targets in `base54` order, generated on demand.
    let mut targets: Vec<String> = Vec::new();
    let mut k = 0u32;
    let mut target = |j: usize, targets: &mut Vec<String>| -> String {
        while targets.len() <= j {
            let n = oxc_mangler::base54(k);
            k += 1;
            let n = n.as_str();
            if !taken.contains(n) && !matches!(n, "__proto__" | "constructor" | "prototype") {
                targets.push(n.to_string());
            }
        }
        targets[j].clone()
    };
    // Bit sets over target indices: used by anyone, by a solitary name, and
    // inside each group.
    fn get(bits: &[u64], j: usize) -> bool {
        bits.get(j / 64).is_some_and(|w| w >> (j % 64) & 1 == 1)
    }
    fn put(bits: &mut Vec<u64>, j: usize) {
        if bits.len() <= j / 64 {
            bits.resize(j / 64 + 1, 0);
        }
        bits[j / 64] |= 1 << (j % 64);
    }
    let mut any: Vec<u64> = Vec::new();
    let mut solo: Vec<u64> = Vec::new();
    let mut used: Vec<Vec<u64>> = vec![Vec::new(); names.groups.len()];
    let mut out = Vec::with_capacity(order.len());
    for (from, _) in order {
        let j = match groups_of.get(from.as_str()) {
            None => {
                let mut j = 0;
                while get(&any, j) {
                    j += 1;
                }
                put(&mut solo, j);
                j
            }
            Some(gs) => {
                let mut forbidden = solo.clone();
                for &g in gs {
                    let u = &used[g];
                    if forbidden.len() < u.len() {
                        forbidden.resize(u.len(), 0);
                    }
                    for (w, x) in forbidden.iter_mut().zip(u.iter()) {
                        *w |= x;
                    }
                }
                let mut j = 0;
                while get(&forbidden, j) {
                    j += 1;
                }
                for &g in gs {
                    put(&mut used[g], j);
                }
                j
            }
        };
        put(&mut any, j);
        out.push((from.clone(), target(j, &mut targets)));
    }
    out
}

/// Like [`minify_names`], also renaming the properties of `names.renamable`
/// that are neither reserved nor appear as a *string*
/// (`docs/JS-PRODUCAO-TAMANHO.md` §3.1), with `oxc_minifier`'s
/// `PropertyMangler`.
///
/// # Errors
///
/// The text is not valid JS for the parser (which would be an emitter
/// defect), or `oxc` rejects the property-renaming options: the first
/// message.
pub fn minify_with_properties(
    js: &str,
    names: &crate::properties::Names,
) -> Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_ast_visit::Visit;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_minifier::{ManglePropertiesOptions, Minifier, MinifierOptions};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    let source = format!("(function () {{\n{js}\n}})();\n");
    // `DARTFORGE_JSPROD_DEDUP=0` turns off string deduplication.
    // A repeated `this` becomes a local (`alias_this.rs`);
    // `DARTFORGE_JSPROD_THIS=0` turns it off.
    let source = if std::env::var("DARTFORGE_JSPROD_THIS").map_or(true, |v| v != "0") {
        crate::alias_this::apply(&source)
    } else {
        source
    };
    let source = if std::env::var("DARTFORGE_JSPROD_DEDUP").map_or(true, |v| v != "0") {
        dedup_strings(&source)?
    } else {
        source
    };
    // Library members as variables (`flatten.rs`); `DARTFORGE_JSPROD_FLATTEN=0`
    // turns it off.
    let source = if std::env::var("DARTFORGE_JSPROD_FLATTEN").map_or(true, |v| v != "0") {
        crate::flatten::apply(&source)?.0
    } else {
        source
    };
    // Trivial functions inlined at their calls (`inline.rs`);
    // `DARTFORGE_JSPROD_INLINE=0` turns it off.
    let source = if std::env::var("DARTFORGE_JSPROD_INLINE").map_or(true, |v| v != "0") {
        crate::inline::apply(&source)?
    } else {
        source
    };
    // Compact forms (`shapes.rs`); `DARTFORGE_JSPROD_SHAPES=0` turns it off.
    let source = if std::env::var("DARTFORGE_JSPROD_SHAPES").map_or(true, |v| v != "0") {
        crate::shapes::apply(&source)?
    } else {
        source
    };
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, &source, SourceType::cjs()).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let mut program = parsed.program;
    // The identifier-shaped *strings* are read **before** compression: it
    // joins lists of *strings* (`["a","b"]` becomes `"a.b".split(".")` in
    // `defineExtensionAccessors`), and the name disappears from the text
    // while still being read at run time. And again afterwards, for whatever
    // it creates. `DARTFORGE_JSPROD_COMPRESS=0` turns compression off.
    let mut strings = IdentifierStrings::new(marker_start(&program));
    strings.visit_program(&program);
    if std::env::var("DARTFORGE_JSPROD_COMPRESS").map_or(true, |v| v != "0") {
        let mut c = oxc_minifier::CompressOptions::smallest();
        // `Object.create(dart.library)` of a namespace nobody cites: no effect.
        c.treeshake.manual_pure_functions = [
            "dart.privateName",
            "dart.fnType",
            "dart.gFnType",
            "dart$$privateName",
            "dart$$fnType",
            "dart$$gFnType",
            "t$R",
            "Object.create",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let _ = Minifier::new(MinifierOptions {
            mangle: None,
            mangle_properties: None,
            compress: Some(c),
        })
        .minify(&allocator, &mut program);
    }
    strings.before = false;
    strings.visit_program(&program);
    let mut free: Vec<&str> = names
        .renamable
        .iter()
        .map(String::as_str)
        .filter(|n| !names.reserved.contains(*n) && !strings.names.contains(*n) && n.len() > 1)
        .filter(|n| {
            n.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
                && !n.as_bytes()[0].is_ascii_digit()
        })
        .collect();
    free.sort_unstable();
    // `DARTFORGE_JSPROD_PROPS_WHY=a,b`: why each name stays or goes
    // (diagnostics).
    if let Ok(targets) = std::env::var("DARTFORGE_JSPROD_PROPS_WHY") {
        for a in targets.split(',') {
            eprintln!(
                "[jsprod] property {a}: renamable={} reserved={} name-string={} free={}",
                names.renamable.contains(a),
                names.reserved.contains(a),
                strings.names.contains(a),
                free.contains(&a)
            );
        }
    }
    // `oxc`'s `include` is a regex; the list of free names does not fit in
    // one. So: a first collection with everything eligible gives the
    // candidates `oxc` sees; those that are not free go into the reserved
    // set; the second collection renames only the free ones.
    if !free.is_empty() {
        let free: std::collections::HashSet<&str> = free.iter().copied().collect();
        let mut base = ManglePropertiesOptions::from_pattern(".")?;
        for r in names.reserved.iter().chain(strings.names.iter()) {
            base.reserved.insert(r.as_str().into());
        }
        let mut probe = oxc_minifier::PropertyMangler::new_in(base.clone(), &allocator);
        probe.collect(&program);
        let candidates: Vec<String> = probe
            .assign()
            .keys()
            .map(|k| k.as_str().to_string())
            .collect();
        let probe_candidates = candidates.clone();
        let mut o = base;
        for c in candidates {
            // A property with a private name of the program (`_x$1c`,
            // `$C$x$1c`): generated by the emitter, only cited by the text
            // itself.
            let private = c
                .rsplit_once('$')
                .is_some_and(|(b, t)| !b.is_empty() && names.tags.contains(&format!("${t}")));
            if !free.contains(c.as_str())
                && !(private && !names.reserved.contains(&c) && !strings.names.contains(&c))
            {
                o.reserved.insert(c.as_str().into());
            }
        }
        // The short names on our own (`assign_names`): `oxc` avoids every
        // *string* of the file as a target, and the single letters (`"a"`,
        // `"p"` of tags and CSS) went unused.
        if std::env::var("DARTFORGE_JSPROD_OWN_NAMES").map_or(true, |v| v != "0") {
            let renamed: std::collections::HashSet<String> = probe_candidates
                .iter()
                .filter(|c| !o.reserved.contains(c.as_str()))
                .cloned()
                .collect();
            let mut extra_taken: Vec<&str> = JS_PROTOTYPE_NAMES.to_vec();
            extra_taken.extend(names.reserved.iter().map(String::as_str));
            extra_taken.extend(strings.names.iter().map(String::as_str));
            for (from, to) in assign_names(&program, &renamed, &extra_taken, names) {
                o.cache
                    .insert(from.into(), Some(to.into()))
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut pm = oxc_minifier::PropertyMangler::new_in(o, &allocator);
        pm.collect(&program);
        let map: std::collections::HashMap<String, String> = pm
            .assign()
            .iter()
            .map(|(a, b)| (a.as_str().to_string(), b.as_str().to_string()))
            .collect();
        pm.rewrite(&mut program);
        rewrite_binds(&mut program, &map, &allocator);
    }
    let options = MinifierOptions {
        mangle: Some(oxc_minifier::MangleOptions {
            top_level: Some(true),
            ..oxc_minifier::MangleOptions::default()
        }),
        mangle_properties: None,
        compress: None,
    };
    let ret = Minifier::new(options).minify(&allocator, &mut program);
    let output = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(ret.scoping)
        .with_private_member_mappings(ret.class_private_mappings)
        .build(&program);
    Ok(output.code)
}

/// Minification of identifiers by scope and of whitespace
/// (`docs/JS-PRODUCAO-SDK-PROPRIO.md` §7.2), with `oxc`: parser → semantics
/// → *mangler* → minified printer.
///
/// The file is wrapped in a function, so that the top-level names (`dart`,
/// `core`, `L$pacote__lib`, `t$R`) are also locals and get shortened —
/// nothing outside the file cites them. Free globals (`self`, `window`,
/// `console`, the DOM constructors) have no declaration in the file and so
/// are never renamed; a scope with a direct `eval` stays intact (a *mangler*
/// rule). **Properties do not change**: the Dart member name is observable in
/// the DDC contract (`dsend(o, "foo")`, `NoSuchMethodError`, interop).
///
/// # Errors
///
/// The text is not valid JS for the parser (which would be an emitter
/// defect): the parser's first message.
pub fn minify_names(js: &str) -> Result<String, String> {
    use oxc_allocator::Allocator;
    use oxc_codegen::{Codegen, CodegenOptions};
    use oxc_mangler::{MangleOptions, Mangler};
    use oxc_parser::Parser;
    use oxc_span::SourceType;
    let source = format!("(function () {{\n{js}\n}})();\n");
    let allocator = Allocator::default();
    let kind = SourceType::cjs();
    let parsed = Parser::new(&allocator, &source, kind).parse();
    if let Some(e) = parsed.diagnostics.first() {
        return Err(format!("oxc could not parse the production file: {e}"));
    }
    let mangle = Mangler::new()
        .with_options(MangleOptions {
            top_level: Some(true),
            ..MangleOptions::default()
        })
        .build(&parsed.program);
    let output = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(Some(mangle.scoping))
        .with_private_member_mappings(Some(mangle.class_private_mappings))
        .build(&parsed.program);
    Ok(output.code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_short_names_only_outside_common_groups() {
        let js = "a.foo; a.foo; a.foo; b.bar; b.bar; c.baz; d.stat; e.unknown;";
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, js, oxc_span::SourceType::cjs()).parse();
        let renamed: std::collections::HashSet<String> = ["foo", "bar", "baz", "stat", "unknown"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // `foo` and `bar` never meet; `baz` lives with `foo`.
        let names = crate::properties::Names {
            groups: vec![vec!["foo".into(), "baz".into()], vec!["bar".into()]],
            solitary: ["stat".to_string()].into_iter().collect(),
            ..Default::default()
        };
        let map: std::collections::HashMap<String, String> =
            assign_names(&parsed.program, &renamed, &[], &names)
                .into_iter()
                .collect();
        assert_eq!(map["foo"], map["bar"]);
        assert_ne!(map["foo"], map["baz"]);
        for solo in ["stat", "unknown"] {
            for other in [
                "foo",
                "bar",
                "baz",
                if solo == "stat" { "unknown" } else { "stat" },
            ] {
                assert_ne!(map[solo], map[other], "{solo} shares with {other}");
            }
        }
        // Without groups, every name gets its own target.
        let map: std::collections::HashMap<String, String> = assign_names(
            &parsed.program,
            &renamed,
            &[],
            &crate::properties::Names::default(),
        )
        .into_iter()
        .collect();
        let targets: std::collections::HashSet<&String> = map.values().collect();
        assert_eq!(targets.len(), renamed.len());
    }

    #[test]
    fn removes_comments_and_indentation() {
        let js = "// header\nfunction f(a, b) {\n  /* sum */\n  return a + b;\n}\n";
        assert_eq!(compact(js), "function f(a,b){\nreturn a+b;\n}\n");
    }

    #[test]
    fn keeps_line_breaks_that_decide_semicolons() {
        // `return`↵`x` returns `undefined`; `a`↵`++b` increments `b`.
        assert_eq!(compact("return\n  x"), "return\nx");
        assert_eq!(compact("a\n++b"), "a\n++b");
        // A block comment with a line break is a line break.
        assert_eq!(compact("return /*\n*/ x"), "return\nx");
    }

    #[test]
    fn whitespace_that_separates_tokens_stays() {
        assert_eq!(compact("a + +b"), "a+ +b");
        assert_eq!(compact("a - -b"), "a- -b");
        assert_eq!(compact("a++ + b"), "a++ +b");
        assert_eq!(compact("var x = typeof y"), "var x=typeof y");
        assert_eq!(compact("1 .toString()"), "1 .toString()");
        assert_eq!(compact("x < !y"), "x< !y");
        assert_eq!(compact("x-- > y"), "x-- >y");
        assert_eq!(compact("a / /re/.source.length"), "a/ /re/.source.length");
    }

    #[test]
    fn literals_are_copied_as_they_are() {
        assert_eq!(compact("s = 'a  // b'  ;"), "s='a  // b';");
        assert_eq!(compact("s = \"x \\\" /* y */\""), "s=\"x \\\" /* y */\"");
        assert_eq!(
            compact("t = `a  ${ f( '}' ) }  b`"),
            "t=`a  ${ f( '}' ) }  b`"
        );
        assert_eq!(
            compact("r = /a  b[/ ]c/g.test(x)"),
            "r=/a  b[/ ]c/g.test(x)"
        );
        assert_eq!(compact("return /  x/.exec(s)"), "return/  x/.exec(s)");
    }

    #[test]
    fn slash_after_expression_is_division() {
        assert_eq!(compact("x = a / b / c"), "x=a/b/c");
        assert_eq!(compact("x = (a) / 2 / 3"), "x=(a)/2/3");
        assert_eq!(compact("x = a++ / 2 / 3"), "x=a++/2/3");
        assert_eq!(compact("x = 1e-5 / 2"), "x=1e-5/2");
    }

    #[test]
    fn is_deterministic_and_idempotent() {
        let js = "var a = {b: 1, // c\n  d: [1, 2]};\nfunction g() { return a.b  +  a.d[0]; }\n";
        let one = compact(js);
        assert_eq!(one, compact(js));
        assert_eq!(compact(&one), one);
    }
}
