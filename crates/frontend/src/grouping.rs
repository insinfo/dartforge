//! The grouping stack of fasta's scanner (`abstract_scanner.dart`,
//! `appendBeginGroup`, `appendEndGroup`, `discardBeginGroupUntil`,
//! `insertSyntheticClosers`, `unmatchedBeginGroup`, `appendEofToken`),
//! rebuilt over our lexer's tokens: a missing closer becomes a zero-length
//! synthetic token (the `SyntheticToken`) with the scanner's `EXPECTED_TOKEN`
//! error (the `UnmatchedToken`, `errors.dart:71`), and an extra closer stays
//! as a plain token.
//!
//! During recovery, fasta compares two options (inserting the closers or
//! ignoring the closer read) by rescanning at most 100 `bigSwitch` calls
//! ahead and counting the recoveries (`recoveryOptionTokenizer`); here the
//! count walks the same tokens, with each one's call weight (the whitespace
//! and comments between them are calls too).
//!
//! Known difference: our lexer closes the `${…}` interpolation at the `}`
//! that ends it and cannot reopen the string; when fasta's recovery would
//! discard an interpolation group, here the closer read is ignored.

use crate::token::{Interp, Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// The `kind` of a `BeginToken`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Parens,
    Brackets,
    Braces,
    Less,
    Interpolation,
}

impl Group {
    /// `closeBraceInfoFor`.
    fn closer(self) -> (Op, &'static str) {
        match self {
            Group::Parens => (Op::RParen, ")"),
            Group::Brackets => (Op::RBracket, "]"),
            Group::Braces | Group::Interpolation => (Op::RBrace, "}"),
            Group::Less => (Op::Gt, ">"),
        }
    }
}

/// What one `bigSwitch` call does to the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupEffect {
    Nothing,
    Opens(Group),
    /// `appendEndGroup` with the `openKind` (for `}`, the interpolation too).
    Closes(Group),
    /// `appendGt`, `appendGtGt`, `appendGtGtGt`.
    Greater(u8),
    /// `discardOpenLt`: `;`, the tokens starting with `=`, `this`.
    DiscardLt,
}

/// One of our tokens seen as `bigSwitch` calls.
#[derive(Debug, Clone, Copy)]
struct Passo {
    /// Whitespace and comment calls before the token.
    trivia: u32,
    /// Calls of the token itself (0 when it continues another one).
    calls: u32,
    first: GroupEffect,
    /// The effect of the second call (the continuation of the string after an
    /// interpolation's `}`, which may open another one).
    second: GroupEffect,
}

/// `bigSwitch` calls in the gap between two tokens: each whitespace
/// character (followed by any further spaces) and each comment.
fn trivia_calls(texto: &str) -> u32 {
    let b = texto.as_bytes();
    let mut i = 0;
    let mut n = 0;
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\n' | b'\r' => {
                n += 1;
                i += 1;
                while i < b.len() && b[i] == b' ' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'/') => {
                n += 1;
                while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                n += 1;
                let mut level = 0usize;
                while i < b.len() {
                    if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                        level += 1;
                        i += 2;
                    } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                        level -= 1;
                        i += 2;
                        if level == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => i += 1,
        }
    }
    n
}

fn steps(source: &str, tokens: &[Token]) -> Vec<Passo> {
    let mut output = Vec::with_capacity(tokens.len());
    // The interpolation of the last piece of each open string.
    let mut excerpts: Vec<Interp> = Vec::new();
    let mut previous_end = 0usize;
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        let trivia = trivia_calls(source.get(previous_end..t.span.start).unwrap_or(""));
        previous_end = t.span.end;
        let texto = &source[t.span.start..t.span.end];
        let mut p = Passo { trivia, calls: 1, first: GroupEffect::Nothing, second: GroupEffect::Nothing };
        match t.kind {
            Kind::StrBegin(_, interp) => {
                excerpts.push(interp);
                if interp == Interp::Brace {
                    p.first = GroupEffect::Opens(Group::Interpolation);
                }
            }
            Kind::StrMid(..) | Kind::StrEnd(_) => {
                let previous = excerpts.pop();
                let new_ = match t.kind {
                    Kind::StrMid(_, interp) => Some(interp),
                    _ => None,
                };
                let abre = if new_ == Some(Interp::Brace) { GroupEffect::Opens(Group::Interpolation) } else { GroupEffect::Nothing };
                if previous == Some(Interp::Brace) {
                    // The `}` and the continuation of the string: two calls.
                    p.first = GroupEffect::Closes(Group::Interpolation);
                    p.calls = 2;
                    p.second = abre;
                } else {
                    // After `$name`, the same `tokenizeString` call.
                    p.calls = 0;
                    p.first = abre;
                }
                if let Some(interp) = new_ {
                    excerpts.push(interp);
                }
            }
            Kind::Ident
                if i > 0
                    && matches!(tokens[i - 1].kind, Kind::StrBegin(_, Interp::Ident) | Kind::StrMid(_, Interp::Ident))
                    && tokens[i - 1].span.end == t.span.start =>
            {
                p.calls = 0;
            }
            Kind::Keyword(Keyword::This) => p.first = GroupEffect::DiscardLt,
            Kind::Op(op) => match op {
                Op::LParen => p.first = GroupEffect::Opens(Group::Parens),
                Op::LBrace => p.first = GroupEffect::Opens(Group::Braces),
                Op::Lt => p.first = GroupEffect::Opens(Group::Less),
                Op::RParen => p.first = GroupEffect::Closes(Group::Parens),
                Op::RBracket => p.first = GroupEffect::Closes(Group::Brackets),
                Op::RBrace => p.first = GroupEffect::Closes(Group::Braces),
                Op::Semicolon => p.first = GroupEffect::DiscardLt,
                Op::LBracket => {
                    // `[]` and `[]=` are the index token, without a group.
                    if t.glued && tokens.get(i + 1).is_some_and(|n| n.kind == Kind::Op(Op::RBracket)) {
                        output.push(p);
                        let mut j = i + 1;
                        let glued_equals = tokens[j].glued && tokens.get(j + 1).is_some_and(|n| n.kind == Kind::Op(Op::Assign));
                        let until = if glued_equals { j + 1 } else { j };
                        while j <= until {
                            output.push(Passo { trivia: 0, calls: 0, first: GroupEffect::Nothing, second: GroupEffect::Nothing });
                            previous_end = tokens[j].span.end;
                            j += 1;
                        }
                        i = until + 1;
                        continue;
                    }
                    p.first = GroupEffect::Opens(Group::Brackets);
                }
                Op::Gt => {
                    // `>`, `>>`, `>>>` and, glued to a `=`, `>=`, `>>=`, `>>>=` (no effect on
                    // the stack).
                    let mut n = 1usize;
                    while n < 3 && tokens[i + n - 1].glued && tokens.get(i + n).is_some_and(|x| x.kind == Kind::Op(Op::Gt)) {
                        n += 1;
                    }
                    let last = i + n - 1;
                    let following = tokens.get(last + 1).filter(|_| tokens[last].glued);
                    let equals_ = following.filter(|x| source[x.span.start..x.span.end].starts_with('='));
                    p.first = if equals_.is_some() { GroupEffect::Nothing } else { GroupEffect::Greater(n as u8) };
                    output.push(p);
                    for k in i + 1..=last {
                        output.push(Passo { trivia: 0, calls: 0, first: GroupEffect::Nothing, second: GroupEffect::Nothing });
                        previous_end = tokens[k].span.end;
                    }
                    i = last + 1;
                    if let Some(x) = equals_ {
                        // The rest of the token after the `=` the `>` took.
                        let rest = &source[x.span.start + 1..x.span.end];
                        let effect = if rest.starts_with('=') {
                            GroupEffect::DiscardLt
                        } else if rest.starts_with('>') {
                            GroupEffect::Greater(1)
                        } else {
                            GroupEffect::Nothing
                        };
                        output.push(Passo { trivia: 0, calls: u32::from(!rest.is_empty()), first: effect, second: GroupEffect::Nothing });
                        previous_end = x.span.end;
                        i += 1;
                    }
                    continue;
                }
                _ if texto.starts_with('=') => p.first = GroupEffect::DiscardLt,
                _ => {}
            },
            _ => {}
        }
        output.push(p);
        i += 1;
    }
    output
}

/// The result of a `discardBeginGroupUntil`.
enum GroupSearch {
    /// The top group is the expected one (after discarding the `<`).
    Direct,
    /// Found further down: the stack height with it on top.
    Below(usize),
    /// There is no opener of that kind: the stack goes back to what it was.
    NoneFound,
}

fn discard_less(stack: &mut Vec<(Group, usize)>) {
    while stack.last().is_some_and(|g| g.0 == Group::Less) {
        stack.pop();
    }
}

fn matches_here(closer: Group, opener: Group) -> bool {
    // An interpolation's `}` is a plain `}` to the scanner (`openKind` `{`),
    // which accepts the `{` and the `${`.
    match closer {
        Group::Braces | Group::Interpolation => matches!(opener, Group::Braces | Group::Interpolation),
        _ => closer == opener,
    }
}

/// The `discardBeginGroupUntil` loop, without deciding the recovery. Leaves
/// the stack as fasta leaves it: without the `<` on top in the direct case.
fn search(stack: &mut Vec<(Group, usize)>, closer: Group) -> GroupSearch {
    let original = stack.clone();
    let mut first = true;
    loop {
        discard_less(stack);
        let Some(&(top_level, _)) = stack.last() else { break };
        if matches_here(closer, top_level) {
            if first {
                return GroupSearch::Direct;
            }
            return GroupSearch::Below(stack.len());
        }
        first = false;
        stack.pop();
        if stack.is_empty() {
            break;
        }
    }
    *stack = original;
    GroupSearch::NoneFound
}

fn apply_greater(stack: &mut Vec<(Group, usize)>, n: u8) {
    for _ in 0..n {
        if stack.last().is_some_and(|g| g.0 == Group::Less) {
            stack.pop();
        } else {
            break;
        }
    }
}

/// The `recoveryOptionTokenizer` of an option, from step `since`: the
/// recoveries counted in at most 101 `bigSwitch` calls.
fn simular(steps: &[Passo], since: usize, stack: &mut Vec<(Group, usize)>, mut recoveries: usize) -> usize {
    let mut iterations = 0u32;
    // The last step is the `Eof`, which the loop does not read.
    for p in steps.iter().take(steps.len().saturating_sub(1)).skip(since) {
        iterations += p.trivia;
        if iterations > 100 {
            return recoveries;
        }
        if p.calls == 0 {
            simulated_effect(p.first, stack, &mut recoveries);
            continue;
        }
        iterations += 1;
        simulated_effect(p.first, stack, &mut recoveries);
        if iterations > 100 {
            return recoveries;
        }
        if p.calls == 2 {
            iterations += 1;
            simulated_effect(p.second, stack, &mut recoveries);
            if iterations > 100 {
                return recoveries;
            }
        }
    }
    recoveries
}

fn simulated_effect(effect: GroupEffect, stack: &mut Vec<(Group, usize)>, recoveries: &mut usize) {
    match effect {
        GroupEffect::Nothing => {}
        GroupEffect::Opens(g) => {
            if !matches!(g, Group::Less | Group::Parens) {
                discard_less(stack);
            }
            stack.push((g, 0));
        }
        GroupEffect::DiscardLt => discard_less(stack),
        GroupEffect::Greater(n) => apply_greater(stack, n),
        GroupEffect::Closes(g) => {
            let height = stack.len();
            match search(stack, g) {
                GroupSearch::Direct => {
                    stack.pop();
                }
                GroupSearch::Below(h) => {
                    *recoveries += 1;
                    // `insertSyntheticClosers`: one `unmatchedBeginGroup` per entry above the
                    // one found, the `<` included.
                    *recoveries += height - h;
                    stack.pop();
                }
                GroupSearch::NoneFound => *recoveries += 1,
            }
        }
    }
}

/// A synthetic closer: the `endGroup` of an opener the scanner did not
/// match.
#[derive(Debug, Clone, Copy)]
pub struct SyntheticCloser {
    /// The start of the opener.
    pub abre: usize,
    /// Where the closer is (the `SyntheticToken`'s `charOffset`).
    pub em: usize,
    /// The index of its error among the returned errors.
    pub erro: usize,
}

/// The tokens with the synthetic closers inserted, the scanner errors and
/// the closers (for the parser's `moveSynthetic`).
pub fn group_tokens(source: &str, tokens: Vec<Token>) -> (Vec<Token>, Vec<Diagnostic>, Vec<SyntheticCloser>) {
    let steps = steps(source, &tokens);
    debug_assert_eq!(steps.len(), tokens.len());
    let mut stack: Vec<(Group, usize)> = Vec::new();
    // (before the index token, synthetic token)
    let mut insertions: Vec<(usize, Token)> = Vec::new();
    let mut errors = Vec::new();
    let mut closers: Vec<SyntheticCloser> = Vec::new();
    let synthetic = |op: Op, em: usize| Token { kind: Kind::Op(op), span: Span { start: em, end: em }, glued: false };
    let unmatched = |g: Group, opener: usize, em: usize, before: usize, insertions: &mut Vec<(usize, Token)>, errors: &mut Vec<Diagnostic>, closers: &mut Vec<SyntheticCloser>| {
        let (op, texto) = g.closer();
        insertions.push((before, synthetic(op, em)));
        closers.push(SyntheticCloser { abre: tokens[opener].span.start, em, erro: errors.len() });
        errors.push(Diagnostic::com_codigo(codigos::scanner::EXPECTED_TOKEN, Span { start: em, end: em + 1 }, [texto]));
    };
    for (i, (t, p)) in tokens.iter().zip(&steps).enumerate() {
        if t.kind == Kind::Eof {
            // `appendEofToken`.
            discard_less(&mut stack);
            while let Some((g, opener)) = stack.pop() {
                unmatched(g, opener, t.span.start, i, &mut insertions, &mut errors, &mut closers);
            }
            break;
        }
        for (k, effect) in [p.first, p.second].into_iter().enumerate() {
            if k == 1 && p.calls < 2 {
                break;
            }
            match effect {
                GroupEffect::Nothing => {}
                GroupEffect::Opens(g) => {
                    if !matches!(g, Group::Less | Group::Parens) {
                        discard_less(&mut stack);
                    }
                    stack.push((g, i));
                }
                GroupEffect::DiscardLt => discard_less(&mut stack),
                GroupEffect::Greater(n) => apply_greater(&mut stack, n),
                GroupEffect::Closes(g) => {
                    let original = stack.clone();
                    match search(&mut stack, g) {
                        GroupSearch::Direct => {
                            stack.pop();
                        }
                        GroupSearch::NoneFound => {}
                        GroupSearch::Below(h) => {
                            let interpolation_in_middle = original[h..].iter().any(|e| e.0 == Group::Interpolation);
                            let option1 = {
                                let mut s = stack.clone();
                                s.pop();
                                let r = simular(&steps, i + 1, &mut s, original.len() - h);
                                r + s.len()
                            };
                            let option2 = {
                                let mut s = original.clone();
                                let r = simular(&steps, i + 1, &mut s, 0);
                                r + s.len() + 1
                            };
                            let interpolation_closer = g == Group::Interpolation;
                            if !interpolation_closer && (option2 < option1 || interpolation_in_middle) {
                                // Option 2: the closer stays as a plain token.
                                stack = original;
                            } else {
                                for &(e, opener) in original[h..].iter().rev() {
                                    unmatched(e, opener, t.span.start, i, &mut insertions, &mut errors, &mut closers);
                                }
                                stack.pop();
                            }
                        }
                    }
                }
            }
        }
    }
    if insertions.is_empty() {
        return (tokens, errors, closers);
    }
    let mut output = Vec::with_capacity(tokens.len() + insertions.len());
    let mut next = insertions.into_iter().peekable();
    for (i, t) in tokens.into_iter().enumerate() {
        while let Some((_, s)) = next.next_if(|(before, _)| *before == i) {
            output.push(s);
        }
        output.push(t);
    }
    (output, errors, closers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closers(src: &str) -> (Vec<String>, Vec<(usize, String)>) {
        let tokens = crate::lexer::lex(src).unwrap();
        let (t, e, _) = group_tokens(src, tokens);
        let synthetics = t
            .iter()
            .filter(|t| t.span.start == t.span.end && t.kind != Kind::Eof)
            .map(|t| format!("{:?}@{}", t.kind, t.span.start))
            .collect();
        let errors = e.iter().map(|d| (d.span.start, d.message.clone())).collect();
        (synthetics, errors)
    }

    #[test]
    fn open_paren_before_brace() {
        let src = "void m() {\n  f(1, \n}\n";
        let (s, e) = closers(src);
        let key = src.rfind('}').unwrap();
        assert_eq!(s, vec![format!("Op(RParen)@{key}")]);
        assert_eq!(e, vec![(key, "Expected to find ')'.".to_string())]);
    }

    #[test]
    fn open_less_also_gets_closer() {
        // `insertSyntheticClosers` looks at the opener found, not at the `<`.
        let src = "void m() {\n  f(a < b, 1\n}\n";
        let (s, _) = closers(src);
        let key = src.rfind('}').unwrap();
        assert_eq!(s, vec![format!("Op(Gt)@{key}"), format!("Op(RParen)@{key}")]);
    }

    #[test]
    fn bracket_with_parens_inside() {
        let src = "void n() {\n  [1, (2]\n}\n";
        let (s, _) = closers(src);
        let bracket = src.find(']').unwrap();
        assert_eq!(s, vec![format!("Op(RParen)@{bracket}")]);
    }

    #[test]
    fn end_of_file_closes_everything() {
        let src = "void m() {\n  f(1";
        let (s, e) = closers(src);
        assert_eq!(s, vec![format!("Op(RParen)@{}", src.len()), format!("Op(RBrace)@{}", src.len())]);
        assert_eq!(e.len(), 2);
    }

    #[test]
    fn index_and_generics_do_not_open_group() {
        let (s, e) = closers("operator []=(i, v) {}\nList<List<int>> x = [];\nvar y = a < b;\n");
        assert!(s.is_empty() && e.is_empty(), "{s:?} {e:?}");
    }
}
