//! Regular expression validation of the Dart 3.6.2 VM's `RegExp(...)`
//! (`runtime/vm/regexp_parser.cc`): what `valid_regexps` catches as a
//! `FormatException` when the analyzer (which runs on the VM) builds the
//! `RegExp` with the literal's text.
//!
//! The flags are the constructor's defaults (`multiLine: false`,
//! `caseSensitive: true`, `dotAll: false`) and the argument's `unicode`. The
//! port follows the parser line by line: the reader (`ReadNext`, `Advance`,
//! `Reset` and the position, which joins surrogate pairs only with
//! `unicode`), the disjunction with the group stack, character classes,
//! escapes, group names (`ID_Start`/`ID_Continue` of Unicode 15.1, the one of
//! ICU 74.2), numbered (with `ScanForCaptures`) and named references, and
//! `\p{...}` with the tables of `regexp_unicode_g`.
//!
//! From the `RegExpBuilder` we keep what decides whether a quantifier is
//! valid (`AddQuantifierToAtom`): the pending empty, the pending surrogate and
//! the kind of the last term (text, assertion, lookahead, lookbehind or
//! other); a non-capturing group with a single term is that term. Known
//! deviation: the maximum matched length (`max_match`) is zero only for
//! assertions and lookarounds (a capture that only matches empty counts as an
//! other term), which only changes the result with a literal lone surrogate
//! before a quantifier in a `unicode` pattern; and the case where the VM
//! reaches `UNREACHABLE` (nothing to quantify after a pending surrogate) is
//! not reported.
//! Written without compiling or running (2026-10-05).

use super::regexp_unicode_g as tables;

const END: u32 = 1 << 21;
const MAX_CAPTURES: i64 = 1 << 16;
const INFINITY: i64 = i32::MAX as i64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Initial,
    Capture,
    Grouping,
    PositiveLookaround,
    NegativeLookaround,
}

/// The kind of a builder term.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Term {
    Text,
    Assertion,
    Lookahead,
    Lookbehind,
    Other,
}

impl Term {
    fn lookaround(self) -> bool {
        matches!(self, Term::Lookahead | Term::Lookbehind)
    }
    /// `max_match() == 0`.
    fn only_empty(self) -> bool {
        matches!(self, Term::Assertion | Term::Lookahead | Term::Lookbehind)
    }
}

/// The `RegExpBuilder`, reduced to what decides the quantifiers.
#[derive(Default)]
struct Builder {
    pending_empty: bool,
    pending_surrogate: bool,
    terms: Vec<Term>,
    alternatives: usize,
}

impl Builder {
    fn flush_surrogate(&mut self) {
        if self.pending_surrogate {
            self.pending_surrogate = false;
            self.add_term(Term::Other);
        }
    }
    fn add_term(&mut self, t: Term) {
        self.flush_surrogate();
        self.pending_empty = false;
        self.terms.push(t);
    }
    fn add_char(&mut self) {
        self.add_term(Term::Text);
    }
    fn add_atom(&mut self, t: Option<Term>) {
        match t {
            None => self.pending_empty = true,
            Some(t) => self.add_term(t),
        }
    }
    fn add_empty(&mut self) {
        self.pending_empty = true;
    }
    fn add_lead(&mut self) {
        self.flush_surrogate();
        self.pending_surrogate = true;
    }
    fn add_trail(&mut self) {
        if self.pending_surrogate {
            self.pending_surrogate = false;
            self.add_term(Term::Text);
        } else {
            self.pending_surrogate = true;
            self.flush_surrogate();
        }
    }
    fn add_unicode(&mut self, c: u32, unicode: bool) {
        if c > 0xFFFF {
            self.add_lead();
            self.add_trail();
        } else if unicode && lead(c) {
            self.add_lead();
        } else if unicode && trail(c) {
            self.add_trail();
        } else {
            self.add_char();
        }
    }
    fn add_escaped_unicode(&mut self, c: u32, unicode: bool) {
        self.flush_surrogate();
        self.add_unicode(c, unicode);
        self.flush_surrogate();
    }
    fn new_alternative(&mut self) {
        self.flush_surrogate();
        self.pending_empty = false;
        self.terms.clear();
        self.alternatives += 1;
    }
    /// `ToRegExp`: `None` for `RegExpEmpty`; the single term of a single
    /// alternative is the term itself.
    fn finish_regexp(&mut self) -> Option<Term> {
        self.flush_surrogate();
        if self.alternatives == 0 {
            return match self.terms[..] {
                [] => None,
                [t] => Some(t),
                _ => Some(Term::Other),
            };
        }
        Some(Term::Other)
    }
    /// `AddQuantifierToAtom`: `Err` on "invalid quantifier.".
    fn quantify(&mut self, min: i64, unicode: bool) -> Result<(), ()> {
        if self.pending_empty {
            self.pending_empty = false;
            return Ok(());
        }
        let Some(&t) = self.terms.last() else {
            // The VM's `UNREACHABLE()`: not reported.
            return Ok(());
        };
        if t.lookaround() && (unicode || t == Term::Lookbehind) {
            return Err(());
        }
        self.terms.pop();
        if t.only_empty() {
            if min != 0 {
                self.terms.push(t);
            }
            return Ok(());
        }
        self.terms.push(Term::Other);
        Ok(())
    }
}

struct State {
    group: Group,
    lookbehind: bool,
    capture_index: i64,
    name: Option<Vec<u16>>,
    builder: Builder,
}

fn lead(c: u32) -> bool {
    (0xD800..=0xDBFF).contains(&c)
}

fn trail(c: u32) -> bool {
    (0xDC00..=0xDFFF).contains(&c)
}

fn ch(c: char) -> u32 {
    c as u32
}

fn is_digit(c: u32) -> bool {
    (ch('0')..=ch('9')).contains(&c)
}

fn is_alnum(c: u32) -> bool {
    is_digit(c) || (ch('a')..=ch('z')).contains(&c) || (ch('A')..=ch('Z')).contains(&c)
}

/// `HexValue`.
fn hex(c: u32) -> i64 {
    let c = c.wrapping_sub(ch('0'));
    if c <= 9 {
        return c as i64;
    }
    let c = (c | 0x20).wrapping_sub(ch('a') - ch('0'));
    if c <= 5 {
        return c as i64 + 10;
    }
    -1
}

fn in_ranges(ranges: &[(u32, u32)], c: u32) -> bool {
    ranges
        .binary_search_by(|&(a, b)| {
            if b < c {
                std::cmp::Ordering::Less
            } else if a > c {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

fn is_ascii_ident_char(c: u32) -> bool {
    is_alnum(c) || c == ch('_') || c == ch('$')
}

/// `IsIdentifierStart`.
fn is_ident_start(c: u32) -> bool {
    if c > 127 {
        return in_ranges(&tables::ID_START, c);
    }
    is_ascii_ident_char(c) && !is_digit(c)
}

/// `IsIdentifierPart`.
fn is_ident_part(c: u32) -> bool {
    if c > 127 {
        return in_ranges(&tables::ID_CONTINUE, c) || c == 0x200C || c == 0x200D;
    }
    is_ascii_ident_char(c)
}

/// `IsSyntaxCharacterOrSlash`.
fn is_syntax_or_slash(c: u32) -> bool {
    matches!(char::from_u32(c), Some('^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/'))
}

/// `IsUnicodePropertyValueCharacter(char)`: the code arrives truncated to a
/// byte (`char`), and a byte above 127 is negative.
fn is_property_char(c: u32) -> bool {
    let b = c & 0xFF;
    is_alnum(b) || b == ch('_')
}

struct Parser<'a> {
    input: &'a [u16],
    unicode: bool,
    current: u32,
    next_pos: usize,
    has_more: bool,
    captures_started: i64,
    capture_count: i64,
    scanned: bool,
    has_named: bool,
    named_captures: Vec<Vec<u16>>,
    named_references: Vec<Vec<u16>>,
}

type R<T> = Result<T, ()>;

impl Parser<'_> {
    /// `ReadNext`.
    fn read_next(&self) -> (u32, usize) {
        let mut pos = self.next_pos;
        let c0 = self.input[pos] as u32;
        let mut c = c0;
        pos += 1;
        if self.unicode && pos < self.input.len() && lead(c0) {
            let c1 = self.input[pos] as u32;
            if trail(c1) {
                c = 0x10000 + ((c0 - 0xD800) << 10) + (c1 - 0xDC00);
                pos += 1;
            }
        }
        (c, pos)
    }
    fn has_next(&self) -> bool {
        self.next_pos < self.input.len()
    }
    fn next(&self) -> u32 {
        if self.has_next() { self.read_next().0 } else { END }
    }
    fn advance(&mut self) {
        if self.has_next() {
            let (c, pos) = self.read_next();
            self.current = c;
            self.next_pos = pos;
        } else {
            self.current = END;
            self.next_pos = self.input.len() + 1;
            self.has_more = false;
        }
    }
    fn advance_by(&mut self, n: usize) {
        self.next_pos += n - 1;
        self.advance();
    }
    fn reset(&mut self, pos: usize) {
        self.next_pos = pos;
        self.has_more = pos < self.input.len();
        self.advance();
    }
    fn position(&self) -> usize {
        self.next_pos - 1
    }

    /// `ParseDisjunction`.
    fn disjunction(&mut self) -> R<()> {
        let mut stack: Vec<State> =
            vec![State { group: Group::Initial, lookbehind: false, capture_index: 0, name: None, builder: Builder::default() }];
        loop {
            let c = self.current;
            let unicode = self.unicode;
            // `true`: the atom accepts a quantifier (the `switch`'s `break`).
            let quantifiable = if c == END {
                if stack.len() > 1 {
                    return Err(()); // "Unterminated group"
                }
                return Ok(());
            } else if c == ch(')') {
                if stack.len() == 1 {
                    return Err(()); // "Unmatched ')'"
                }
                self.advance();
                let mut state = stack.pop().expect("open group");
                let body = state.builder.finish_regexp();
                let term = match state.group {
                    Group::Capture => {
                        if let Some(name) = state.name.take() {
                            // `CreateNamedCaptureAtIndex`.
                            if self.named_captures.contains(&name) {
                                return Err(()); // "Duplicate capture group name"
                            }
                            self.named_captures.push(name);
                        }
                        Some(Term::Other)
                    }
                    Group::Grouping => body,
                    _ => Some(if state.lookbehind { Term::Lookbehind } else { Term::Lookahead }),
                };
                stack.last_mut().expect("initial state").builder.add_atom(term);
                true
            } else if c == ch('|') {
                self.advance();
                stack.last_mut().expect("state").builder.new_alternative();
                false
            } else if c == ch('*') || c == ch('+') || c == ch('?') {
                return Err(()); // "Nothing to repeat"
            } else if c == ch('^') || c == ch('$') {
                self.advance();
                stack.last_mut().expect("state").builder.add_term(Term::Assertion);
                false
            } else if c == ch('.') {
                self.advance();
                stack.last_mut().expect("state").builder.add_term(Term::Text);
                true
            } else if c == ch('(') {
                let new_state = self.open_paren(stack.last().expect("state"))?;
                stack.push(new_state);
                false
            } else if c == ch('[') {
                self.char_class()?;
                stack.last_mut().expect("state").builder.add_term(Term::Text);
                true
            } else if c == ch('\\') {
                self.escape(&mut stack)?
            } else {
                if c == ch('{') && self.interval().is_some() {
                    return Err(()); // "Nothing to repeat"
                }
                if (c == ch('{') || c == ch('}') || c == ch(']')) && unicode {
                    return Err(()); // "Lone quantifier brackets"
                }
                let current = self.current;
                stack.last_mut().expect("state").builder.add_unicode(current, unicode);
                self.advance();
                true
            };
            if !quantifiable {
                continue;
            }
            let c = self.current;
            let min;
            if c == ch('*') || c == ch('?') {
                min = 0;
                self.advance();
            } else if c == ch('+') {
                min = 1;
                self.advance();
            } else if c == ch('{') {
                match self.interval() {
                    Some((a, b)) => {
                        if b < a {
                            return Err(()); // "numbers out of order in {} quantifier."
                        }
                        min = a;
                    }
                    None => continue,
                }
            } else {
                continue;
            }
            if self.current == ch('?') {
                self.advance();
            }
            stack.last_mut().expect("state").builder.quantify(min, unicode)?; // "invalid quantifier."
        }
    }

    /// The `\` outside a class; returns whether the atom accepts a quantifier.
    fn escape(&mut self, stack: &mut [State]) -> R<bool> {
        let unicode = self.unicode;
        let n = self.next();
        let top = stack.len() - 1;
        if n == END {
            return Err(()); // "\\ at end of pattern"
        }
        let Some(nc) = char::from_u32(n) else {
            return self.identity_escape(stack);
        };
        match nc {
            'b' | 'B' => {
                self.advance_by(2);
                stack[top].builder.add_term(Term::Assertion);
                Ok(false)
            }
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => {
                self.advance_by(2);
                stack[top].builder.add_term(Term::Text);
                Ok(true)
            }
            'p' | 'P' => {
                self.advance_by(2);
                if unicode {
                    if !self.property() {
                        return Err(()); // "Invalid property name"
                    }
                    stack[top].builder.add_term(Term::Text);
                } else {
                    stack[top].builder.add_char();
                }
                Ok(true)
            }
            '1'..='9' => {
                if let Some(index) = self.back_reference_index() {
                    if inside_capture(stack, index) {
                        stack[top].builder.add_empty();
                    } else {
                        stack[top].builder.add_atom(Some(Term::Other));
                    }
                    return Ok(true);
                }
                if unicode {
                    return Err(()); // kUnicodeIdentity
                }
                if nc == '8' || nc == '9' {
                    stack[top].builder.add_char();
                    self.advance_by(2);
                    return Ok(true);
                }
                self.escape_zero(stack)
            }
            '0' => self.escape_zero(stack),
            'f' | 'n' | 'r' | 't' | 'v' => {
                self.advance_by(2);
                stack[top].builder.add_char();
                Ok(true)
            }
            'c' => {
                self.advance();
                let control = self.next();
                let letter = control & !0x20;
                if letter < ch('A') || ch('Z') < letter {
                    if unicode {
                        return Err(()); // kUnicodeIdentity
                    }
                    // The backslash becomes a character; the `c` is read again.
                    stack[top].builder.add_char();
                } else {
                    self.advance_by(2);
                    stack[top].builder.add_char();
                }
                Ok(true)
            }
            'x' => {
                self.advance_by(2);
                if self.escape_hex(2).is_some() || !unicode {
                    stack[top].builder.add_char();
                    Ok(true)
                } else {
                    Err(()) // kUnicodeIdentity
                }
            }
            'u' => {
                self.advance_by(2);
                if let Some(v) = self.escape_unicode() {
                    stack[top].builder.add_escaped_unicode(v, unicode);
                    Ok(true)
                } else if !unicode {
                    stack[top].builder.add_char();
                    Ok(true)
                } else {
                    Err(()) // kUnicodeIdentity
                }
            }
            'k' if unicode || self.has_named_captures() => {
                self.advance_by(2);
                self.named_back_reference(stack)?;
                Ok(true)
            }
            _ => self.identity_escape(stack),
        }
    }

    /// The `default:` of `\`: the identity escape.
    fn identity_escape(&mut self, stack: &mut [State]) -> R<bool> {
        self.advance();
        if !self.unicode || is_syntax_or_slash(self.current) {
            let top = stack.len() - 1;
            stack[top].builder.add_char();
            self.advance();
            Ok(true)
        } else {
            Err(()) // kUnicodeIdentity
        }
    }

    /// `case '0':` (and the fall-through of the digits that are not references).
    fn escape_zero(&mut self, stack: &mut [State]) -> R<bool> {
        self.advance();
        if self.unicode && is_digit(self.next()) {
            return Err(()); // "Invalid decimal escape"
        }
        self.octal();
        let top = stack.len() - 1;
        stack[top].builder.add_char();
        Ok(true)
    }

    /// `ParseOpenParenthesis`.
    fn open_paren(&mut self, state: &State) -> R<State> {
        let mut lookbehind = state.lookbehind;
        let mut named = false;
        let mut group = Group::Capture;
        self.advance();
        if self.current == ch('?') {
            let n = self.next();
            if n == ch(':') {
                self.advance_by(2);
                group = Group::Grouping;
            } else if n == ch('=') || n == ch('!') {
                self.advance_by(2);
                lookbehind = false;
                group = if n == ch('=') { Group::PositiveLookaround } else { Group::NegativeLookaround };
            } else if n == ch('<') {
                self.advance();
                let n2 = self.next();
                if n2 == ch('=') || n2 == ch('!') {
                    self.advance_by(2);
                    lookbehind = true;
                    group = if n2 == ch('=') { Group::PositiveLookaround } else { Group::NegativeLookaround };
                } else {
                    named = true;
                    self.has_named = true;
                    self.advance();
                }
            } else {
                return Err(()); // "Invalid group"
            }
        }
        let mut name = None;
        if group == Group::Capture {
            if self.captures_started >= MAX_CAPTURES {
                return Err(()); // "Too many captures"
            }
            self.captures_started += 1;
            if named {
                name = Some(self.group_name()?);
            }
        }
        Ok(State { group, lookbehind, capture_index: self.captures_started, name, builder: Builder::default() })
    }

    /// `ScanForCaptures`.
    fn scan_for_captures(&mut self) {
        let saved = self.position();
        let mut n = self.captures_started;
        loop {
            let c = self.current;
            if c == END {
                break;
            }
            self.advance();
            if c == ch('\\') {
                self.advance();
            } else if c == ch('[') {
                loop {
                    let c2 = self.current;
                    if c2 == END {
                        break;
                    }
                    self.advance();
                    if c2 == ch('\\') {
                        self.advance();
                    } else if c2 == ch(']') {
                        break;
                    }
                }
            } else if c == ch('(') {
                if self.current == ch('?') {
                    self.advance();
                    if self.current != ch('<') {
                        continue;
                    }
                    self.advance();
                    if self.current == ch('=') || self.current == ch('!') {
                        continue;
                    }
                    self.has_named = true;
                }
                n += 1;
            }
        }
        self.capture_count = n;
        self.scanned = true;
        self.reset(saved);
    }

    /// `ParseBackReferenceIndex`.
    fn back_reference_index(&mut self) -> Option<i64> {
        let start = self.position();
        let mut value = (self.next() - ch('0')) as i64;
        self.advance_by(2);
        while is_digit(self.current) {
            value = 10 * value + (self.current - ch('0')) as i64;
            if value > MAX_CAPTURES {
                self.reset(start);
                return None;
            }
            self.advance();
        }
        if value > self.captures_started {
            if !self.scanned {
                self.scan_for_captures();
            }
            if value > self.capture_count {
                self.reset(start);
                return None;
            }
        }
        Some(value)
    }

    /// `HasNamedCaptures`.
    fn has_named_captures(&mut self) -> bool {
        if self.has_named || self.scanned {
            return self.has_named;
        }
        self.scan_for_captures();
        self.has_named
    }

    /// `ParseCaptureGroupName`.
    fn group_name(&mut self) -> R<Vec<u16>> {
        let mut name: Vec<u16> = Vec::new();
        let push = |name: &mut Vec<u16>, c: u32| {
            if c <= 0xFFFF {
                name.push(c as u16);
            } else {
                let v = c - 0x10000;
                name.push((0xD800 + (v >> 10)) as u16);
                name.push((0xDC00 + (v & 0x3FF)) as u16);
            }
        };
        let mut at_start = true;
        loop {
            let mut c = self.current;
            self.advance();
            if c == ch('\\') && self.current == ch('u') {
                self.advance();
                match self.escape_unicode() {
                    Some(v) => c = v,
                    None => return Err(()), // "Invalid Unicode escape sequence"
                }
            }
            // The slash would pass `ID_Start` and `ID_Continue`.
            if c == ch('\\') {
                return Err(()); // "Invalid capture group name"
            }
            if at_start {
                if !is_ident_start(c) {
                    return Err(());
                }
                push(&mut name, c);
                at_start = false;
            } else if c == ch('>') {
                break;
            } else if is_ident_part(c) {
                push(&mut name, c);
            } else {
                return Err(());
            }
        }
        Ok(name)
    }

    /// `ParseNamedBackReference`.
    fn named_back_reference(&mut self, stack: &mut [State]) -> R<()> {
        if self.current != ch('<') {
            return Err(()); // "Invalid named reference"
        }
        self.advance();
        let name = self.group_name()?;
        let top = stack.len() - 1;
        if stack.iter().any(|s| s.name.as_ref() == Some(&name)) {
            stack[top].builder.add_empty();
        } else {
            stack[top].builder.add_atom(Some(Term::Other));
            self.named_references.push(name);
        }
        Ok(())
    }

    /// The digits of a `{}` bound, saturated at `kInfinity`.
    fn interval_number(&mut self) -> i64 {
        let mut n: i64 = 0;
        while is_digit(self.current) {
            let d = (self.current - ch('0')) as i64;
            if n > (INFINITY - d) / 10 {
                loop {
                    self.advance();
                    if !is_digit(self.current) {
                        break;
                    }
                }
                return INFINITY;
            }
            n = 10 * n + d;
            self.advance();
        }
        n
    }

    /// `ParseIntervalQuantifier`: `(min, max)`, or nothing with the position
    /// restored.
    fn interval(&mut self) -> Option<(i64, i64)> {
        let start = self.position();
        self.advance();
        if !is_digit(self.current) {
            self.reset(start);
            return None;
        }
        let min = self.interval_number();
        let max;
        if self.current == ch('}') {
            max = min;
            self.advance();
        } else if self.current == ch(',') {
            self.advance();
            if self.current == ch('}') {
                max = INFINITY;
                self.advance();
            } else {
                max = self.interval_number();
                if self.current != ch('}') {
                    self.reset(start);
                    return None;
                }
                self.advance();
            }
        } else {
            self.reset(start);
            return None;
        }
        Some((min, max))
    }

    /// `ParseOctalLiteral`.
    fn octal(&mut self) -> u32 {
        let octal = |c: u32| (ch('0')..=ch('7')).contains(&c);
        let mut value = self.current.wrapping_sub(ch('0'));
        self.advance();
        if octal(self.current) {
            value = value.wrapping_mul(8).wrapping_add(self.current - ch('0'));
            self.advance();
            if value < 32 && octal(self.current) {
                value = value * 8 + self.current - ch('0');
                self.advance();
            }
        }
        value
    }

    /// `ParseHexEscape`.
    fn escape_hex(&mut self, len: usize) -> Option<u32> {
        let start = self.position();
        let mut value: u32 = 0;
        for _ in 0..len {
            let d = hex(self.current);
            if d < 0 {
                self.reset(start);
                return None;
            }
            value = value.wrapping_mul(16).wrapping_add(d as u32);
            self.advance();
        }
        Some(value)
    }

    /// `ParseUnlimitedLengthHexNumber`.
    fn hex_number(&mut self, limit: u32) -> Option<u32> {
        let mut x: u32 = 0;
        let mut d = hex(self.current);
        if d < 0 {
            return None;
        }
        while d >= 0 {
            x = x * 16 + d as u32;
            if x > limit {
                return None;
            }
            self.advance();
            d = hex(self.current);
        }
        Some(x)
    }

    /// `ParseUnicodeEscape` (the `\u` already read).
    fn escape_unicode(&mut self) -> Option<u32> {
        if self.current == ch('{') && self.unicode {
            let start = self.position();
            self.advance();
            if let Some(v) = self.hex_number(0x10FFFF)
                && self.current == ch('}')
            {
                self.advance();
                return Some(v);
            }
            self.reset(start);
            return None;
        }
        let v = self.escape_hex(4)?;
        if self.unicode && lead(v) && self.current == ch('\\') {
            let start = self.position();
            if self.next() == ch('u') {
                self.advance_by(2);
                if let Some(t) = self.escape_hex(4)
                    && trail(t)
                {
                    return Some(0x10000 + ((v - 0xD800) << 10) + (t - 0xDC00));
                }
            }
            self.reset(start);
        }
        Some(v)
    }

    /// `ParsePropertyClassName` + `AddPropertyClassRange` (the `\p`/`\P`
    /// already read): `false` for a name that is not valid. Names are exact
    /// (ICU finds them by loose matching, and the VM checks the exact alias).
    fn property(&mut self) -> bool {
        let mut name1 = String::new();
        let mut name2: Option<String> = None;
        if self.current != ch('{') {
            return false;
        }
        self.advance();
        while self.current != ch('}') && self.current != ch('=') {
            if !is_property_char(self.current) || !self.has_next() {
                return false;
            }
            name1.push((self.current & 0xFF) as u8 as char);
            self.advance();
        }
        if self.current == ch('=') {
            let mut v = String::new();
            self.advance();
            while self.current != ch('}') {
                if !is_property_char(self.current) || !self.has_next() {
                    return false;
                }
                v.push((self.current & 0xFF) as u8 as char);
                self.advance();
            }
            name2 = Some(v);
        }
        self.advance();
        let in_list = |list: &[&str], n: &str| list.contains(&n);
        match &name2 {
            // The general category, `Any`/`ASCII`/`Assigned` and the binary ones.
            None => {
                in_list(&tables::GENERAL_CATEGORIES, &name1)
                    || matches!(name1.as_str(), "Any" | "ASCII" | "Assigned")
                    || in_list(&tables::BINARY_PROPERTIES, &name1)
            }
            Some(value) => match name1.as_str() {
                "gc" | "General_Category" => in_list(&tables::GENERAL_CATEGORIES, value),
                "sc" | "Script" => in_list(&tables::SCRIPTS, value),
                "scx" | "Script_Extensions" => in_list(&tables::SCRIPT_EXTENSIONS, value),
                _ => false,
            },
        }
    }

    /// `ParseClassCharacterEscape` (at the `\`).
    fn class_char_escape(&mut self) -> R<u32> {
        self.advance();
        let c = self.current;
        let simple_control = match char::from_u32(c) {
            Some('b') => Some(0x08),
            Some('f') => Some(0x0C),
            Some('n') => Some(0x0A),
            Some('r') => Some(0x0D),
            Some('t') => Some(0x09),
            Some('v') => Some(0x0B),
            _ => None,
        };
        if let Some(x) = simple_control {
            self.advance();
            return Ok(x);
        }
        if c == ch('c') {
            let control = self.next();
            let letter = control & !0x20;
            if (ch('A')..=ch('Z')).contains(&letter) {
                self.advance_by(2);
                return Ok(control & 0x1F);
            }
            if self.unicode {
                return Err(()); // "Invalid class escape"
            }
            if is_digit(control) || control == ch('_') {
                self.advance_by(2);
                return Ok(control & 0x1F);
            }
            return Ok(ch('\\'));
        }
        if c == ch('0') && self.unicode && !is_digit(self.next()) {
            self.advance();
            return Ok(0);
        }
        if (ch('0')..=ch('7')).contains(&c) {
            if self.unicode {
                return Err(()); // "Invalid class escape"
            }
            return Ok(self.octal());
        }
        if c == ch('x') {
            self.advance();
            if let Some(v) = self.escape_hex(2) {
                return Ok(v);
            }
            if self.unicode {
                return Err(()); // "Invalid escape"
            }
            return Ok(ch('x'));
        }
        if c == ch('u') {
            self.advance();
            if let Some(v) = self.escape_unicode() {
                return Ok(v);
            }
            if self.unicode {
                return Err(()); // kUnicodeIdentity
            }
            return Ok(ch('u'));
        }
        if !self.unicode || is_syntax_or_slash(c) || c == ch('-') {
            self.advance();
            return Ok(c);
        }
        Err(()) // kUnicodeIdentity
    }

    /// `ParseClassEscape`: `(is a class, character)`.
    fn class_escape(&mut self) -> R<(bool, u32)> {
        let first = self.current;
        if first == ch('\\') {
            let n = self.next();
            if [ch('w'), ch('W'), ch('d'), ch('D'), ch('s'), ch('S')].contains(&n) {
                self.advance_by(2);
                return Ok((true, 0));
            }
            if (n == ch('p') || n == ch('P')) && self.unicode {
                self.advance_by(2);
                if !self.property() {
                    return Err(()); // "Invalid property name in character class"
                }
                return Ok((true, 0));
            }
            if n == END {
                return Err(()); // "\\ at end of pattern"
            }
            return Ok((false, self.class_char_escape()?));
        }
        self.advance();
        Ok((false, first))
    }

    /// `ParseCharacterClass`.
    fn char_class(&mut self) -> R<()> {
        self.advance();
        if self.current == ch('^') {
            self.advance();
        }
        while self.has_more && self.current != ch(']') {
            let (is_class1, c1) = self.class_escape()?;
            if self.current == ch('-') {
                self.advance();
                if self.current == END || self.current == ch(']') {
                    break;
                }
                let (is_class2, c2) = self.class_escape()?;
                if is_class1 || is_class2 {
                    if self.unicode {
                        return Err(()); // "Invalid character class"
                    }
                    continue;
                }
                if c1 > c2 {
                    return Err(()); // "Range out of order in character class"
                }
            }
        }
        if !self.has_more {
            return Err(()); // "Unterminated character class"
        }
        self.advance();
        Ok(())
    }
}

/// `IsInsideCaptureGroup(index)`.
fn inside_capture(stack: &[State], index: i64) -> bool {
    for s in stack.iter().rev() {
        if s.group != Group::Capture {
            continue;
        }
        if index == s.capture_index {
            return true;
        }
        if index > s.capture_index {
            return false;
        }
    }
    false
}

/// The VM's `RegExp(source, unicode: unicode)` does not throw `FormatException`.
pub fn is_valid(source: &[u16], unicode: bool) -> bool {
    let mut a = Parser {
        input: source,
        unicode,
        current: END,
        next_pos: 0,
        has_more: true,
        captures_started: 0,
        capture_count: 0,
        scanned: false,
        has_named: false,
        named_captures: Vec::new(),
        named_references: Vec::new(),
    };
    a.advance();
    if a.disjunction().is_err() {
        return false;
    }
    // `PatchNamedBackReferences`.
    a.named_references.iter().all(|r| a.named_captures.contains(r))
}

#[cfg(test)]
mod tests {
    use super::is_valid;

    fn v(s: &str, u: bool) -> bool {
        is_valid(&s.encode_utf16().collect::<Vec<u16>>(), u)
    }

    #[test]
    fn vm_errors_and_valid_patterns() {
        assert!(v("a+b*", false));
        assert!(!v("(", false));
        assert!(!v(")", false));
        assert!(!v("*a", false));
        assert!(!v("a{2,1}", false));
        assert!(v("a{,1}", false));
        assert!(!v("a{,1}", true));
        assert!(!v("[b-a]", false));
        assert!(!v("[a", false));
        assert!(!v("\\", false));
        assert!(!v("(?<=a)*", false));
        assert!(v("(?=a)*", false));
        assert!(!v("(?=a)*", true));
        assert!(!v("(?:(?<=a))+", false));
        assert!(v("\\1(a)", false));
        assert!(v("\\2(a)", false));
        assert!(!v("\\2(a)", true));
        assert!(!v("(?<n>a)(?<n>b)", false));
        assert!(!v("\\k<m>(?<n>a)", false));
        assert!(v("\\k<m>", false));
        assert!(v("\\p{Lu}", true));
        assert!(v("\\p{Script=Latin}", true));
        assert!(!v("\\p{Script=Hrkt}", true));
        assert!(!v("\\p{lu}", true));
        assert!(!v("\\-", true));
        assert!(v("[\\-]", true));
        assert!(!v("[\\d-a]", true));
        assert!(v("[\\d-a]", false));
        assert!(!v("(?a)", false));
        assert!(v("(?<ação>x)\\k<ação>", false));
    }
}
