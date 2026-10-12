//! A pilha de grouping do scanner do fasta (`abstract_scanner.dart`,
//! `appendBeginGroup`, `appendEndGroup`, `discardBeginGroupUntil`,
//! `insertSyntheticClosers`, `unmatchedBeginGroup`, `appendEofToken`),
//! refeita sobre os tokens do nosso lexer: o fecho que falta vira um token
//! sintético de comprimento zero (o `SyntheticToken`) com o erro
//! `EXPECTED_TOKEN` do scanner (o `UnmatchedToken`, `errors.dart:71`), e o
//! fecho que sobra fica como token comum.
//!
//! Na recuperação, o fasta compara duas opções (inserir os fechos ou ignorar
//! o fecho lido) reescaneando no máximo 100 chamadas do `bigSwitch` à
//! frente e contando as recuperações (`recoveryOptionTokenizer`); aqui a
//! contagem anda pelos mesmos tokens, com o peso de chamadas de cada um
//! (os espaços em branco e os comentários entre eles também são chamadas).
//!
//! Diferença conhecida: o nosso lexer fecha a interpolação `${…}` na `}`
//! que a termina e não sabe reabrir a string; quando a recuperação do fasta
//! descartaria um grupo de interpolação, aqui o fecho lido é ignorado.

use crate::token::{Interp, Keyword, Kind, Op, Token};
use dartforge_diagnostics::{Diagnostic, Span, codigos};

/// O `kind` de um `BeginToken`.
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

/// O que uma chamada do `bigSwitch` faz com a pilha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupEffect {
    Nothing,
    Opens(Group),
    /// `appendEndGroup` com o `openKind` (para `}`, também a interpolação).
    Closes(Group),
    /// `appendGt`, `appendGtGt`, `appendGtGtGt`.
    Greater(u8),
    /// `discardOpenLt`: `;`, os tokens que começam com `=`, `this`.
    DiscardLt,
}

/// Um token nosso visto como chamadas do `bigSwitch`.
#[derive(Debug, Clone, Copy)]
struct Passo {
    /// Chamadas de espaço em branco e comentário antes do token.
    trivia: u32,
    /// Chamadas do próprio token (0 quando é a continuação de outro).
    calls: u32,
    first: GroupEffect,
    /// O efeito da segunda chamada (a continuação da string depois da `}`
    /// de uma interpolação, que pode abrir outra).
    second: GroupEffect,
}

/// Chamadas do `bigSwitch` no trecho entre dois tokens: cada caractere de
/// espaço em branco (seguido dos espaços que vierem) e cada comentário.
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
    // A interpolação do último trecho de cada string aberta.
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
                let anterior = excerpts.pop();
                let nova = match t.kind {
                    Kind::StrMid(_, interp) => Some(interp),
                    _ => None,
                };
                let abre = if nova == Some(Interp::Brace) { GroupEffect::Opens(Group::Interpolation) } else { GroupEffect::Nothing };
                if anterior == Some(Interp::Brace) {
                    // A `}` e a continuação da string: duas chamadas.
                    p.first = GroupEffect::Closes(Group::Interpolation);
                    p.calls = 2;
                    p.second = abre;
                } else {
                    // Depois de `$nome`, a mesma chamada do `tokenizeString`.
                    p.calls = 0;
                    p.first = abre;
                }
                if let Some(interp) = nova {
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
                    // `[]` e `[]=` são o token de índice, sem grupo.
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
                    // `>`, `>>`, `>>>` e, colados a um `=`, `>=`, `>>=`,
                    // `>>>=` (sem efeito na pilha).
                    let mut n = 1usize;
                    while n < 3 && tokens[i + n - 1].glued && tokens.get(i + n).is_some_and(|x| x.kind == Kind::Op(Op::Gt)) {
                        n += 1;
                    }
                    let ultimo = i + n - 1;
                    let following = tokens.get(ultimo + 1).filter(|_| tokens[ultimo].glued);
                    let equals_ = following.filter(|x| source[x.span.start..x.span.end].starts_with('='));
                    p.first = if equals_.is_some() { GroupEffect::Nothing } else { GroupEffect::Greater(n as u8) };
                    output.push(p);
                    for k in i + 1..=ultimo {
                        output.push(Passo { trivia: 0, calls: 0, first: GroupEffect::Nothing, second: GroupEffect::Nothing });
                        previous_end = tokens[k].span.end;
                    }
                    i = ultimo + 1;
                    if let Some(x) = equals_ {
                        // O resto do token depois do `=` que o `>` levou.
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

/// O resultado de um `discardBeginGroupUntil`.
enum GroupSearch {
    /// O grupo do topo é o esperado (depois de descartar os `<`).
    Direct,
    /// Achado mais abaixo: a altura da pilha com ele no topo.
    Below(usize),
    /// Não há abridor da espécie: a pilha volta ao que era.
    NoneFound,
}

fn discard_less(stack: &mut Vec<(Group, usize)>) {
    while stack.last().is_some_and(|g| g.0 == Group::Less) {
        stack.pop();
    }
}

fn matches_here(closer: Group, opener: Group) -> bool {
    // A `}` de uma interpolação é uma `}` comum para o scanner (`openKind`
    // `{`), que aceita o `{` e o `${`.
    match closer {
        Group::Braces | Group::Interpolation => matches!(opener, Group::Braces | Group::Interpolation),
        _ => closer == opener,
    }
}

/// O laço do `discardBeginGroupUntil`, sem decidir a recuperação. Deixa a
/// pilha como o fasta a deixa: sem os `<` do topo no caso direto.
fn search(stack: &mut Vec<(Group, usize)>, closer: Group) -> GroupSearch {
    let original = stack.clone();
    let mut first = true;
    loop {
        discard_less(stack);
        let Some(&(topo, _)) = stack.last() else { break };
        if matches_here(closer, topo) {
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

/// O `recoveryOptionTokenizer` de uma opção, a partir do passo `since`:
/// as recuperações contadas em no máximo 101 chamadas do `bigSwitch`.
fn simular(steps: &[Passo], since: usize, stack: &mut Vec<(Group, usize)>, mut recoveries: usize) -> usize {
    let mut iterations = 0u32;
    // O último passo é o `Eof`, que o laço não lê.
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
                    // `insertSyntheticClosers`: um `unmatchedBeginGroup` por
                    // entrada acima da achada, os `<` inclusive.
                    *recoveries += height - h;
                    stack.pop();
                }
                GroupSearch::NoneFound => *recoveries += 1,
            }
        }
    }
}

/// Um fecho sintético: o `endGroup` de um abridor que o scanner não casou.
#[derive(Debug, Clone, Copy)]
pub struct SyntheticCloser {
    /// O início do abridor.
    pub abre: usize,
    /// Onde o fecho está (o `charOffset` do `SyntheticToken`).
    pub em: usize,
    /// O índice do erro dele entre os erros devolvidos.
    pub erro: usize,
}

/// Os tokens com os fechos sintéticos inseridos, os erros do scanner e os
/// fechos (para o `moveSynthetic` do parser).
pub fn group_tokens(source: &str, tokens: Vec<Token>) -> (Vec<Token>, Vec<Diagnostic>, Vec<SyntheticCloser>) {
    let steps = steps(source, &tokens);
    debug_assert_eq!(steps.len(), tokens.len());
    let mut stack: Vec<(Group, usize)> = Vec::new();
    // (antes do token de índice, token sintético)
    let mut insertions: Vec<(usize, Token)> = Vec::new();
    let mut errors = Vec::new();
    let mut closers: Vec<SyntheticCloser> = Vec::new();
    let synthetic = |op: Op, em: usize| Token { kind: Kind::Op(op), span: Span { start: em, end: em }, glued: false };
    let unmatched = |g: Group, opener: usize, em: usize, antes: usize, insertions: &mut Vec<(usize, Token)>, errors: &mut Vec<Diagnostic>, closers: &mut Vec<SyntheticCloser>| {
        let (op, texto) = g.closer();
        insertions.push((antes, synthetic(op, em)));
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
                                // Opção 2: o fecho fica como token comum.
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
        while let Some((_, s)) = next.next_if(|(antes, _)| *antes == i) {
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
        // O `insertSyntheticClosers` olha o abridor achado, não o `<`.
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
