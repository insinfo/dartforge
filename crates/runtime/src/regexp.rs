// Runtime nativo: o motor de expressões regulares do `RegExp` (o irregexp da
// VM, `runtime/vm/regexp*.cc`, faz este papel lá). Semântica do ECMAScript
// (ECMA-262 §22.2), que é a do `RegExp` do Dart, com o Anexo B no modo não
// unicode, como o irregexp: `]` e `{` soltos são literais, escape
// desconhecido é o próprio caractere, `\0nn` octal.
//
// O motor é de retrocesso com continuações, sobre as unidades UTF-16 da
// string (os índices que o `dart:core` vê); no modo `unicode` um par de
// surrogates é lido como um ponto de código. A repetição de um átomo de um
// caractere sem grupos (`.*`, `\d+`, `[a-z]{2,}`) é iterativa, sem recursão
// por caractere.
//
// O `_RegExp` da sobreposição (`native_sdk/core/regexp_patch.dart`) compila
// o padrão uma vez (`DartForge_regexp_compilar`) num `_ProgramaDeRegExp` (cid
// 16, `BRUTO`+`ATTACHMENT`: o `Box<ReProgram>` solto pelo coletor quando o
// `_RegExp` morre, docs/NATIVO-ESPACO-UNIFICADO.md §4.3) e pede cada
// casamento (`DartForge_regexp_executar`); as posições do último casamento
// são lidas por `DartForge_regexp_captura`.

/// Uma classe de caracteres (`[...]`, `\d`, `.`).
#[derive(Clone, Debug)]
enum ReItem {
    Range(u32, u32),
    Digit(bool),
    Word(bool),
    Space(bool),
    Property(ReProp, bool),
}

/// As propriedades Unicode de `\p{...}` que o motor conhece.
#[derive(Clone, Copy, Debug)]
enum ReProp {
    Letter,
    Upper,
    Lower,
    Number,
    Digit,
    Alphabetic,
    Whitespace,
    Punctuation,
    Any,
    Ascii,
}

#[derive(Clone, Debug)]
enum ReNode {
    Void,
    Char(u32),
    Any,
    Class(Vec<ReItem>, bool),
    Start,
    End,
    Boundary(bool),
    Group(Option<usize>, Box<ReNode>),
    Seq(Vec<ReNode>),
    Alt(Vec<ReNode>),
    Repeat {
        no: Box<ReNode>,
        min: u32,
        max: u32,
        greedy: bool,
        /// As capturas dentro do átomo, `[de, until)`: zeradas a cada iteração.
        groups: (usize, usize),
    },
    Back(usize),
    NamedBack(String),
    Peek {
        no: Box<ReNode>,
        positive: bool,
        behind: bool,
    },
}

/// Um padrão compilado.
struct ReProgram {
    root: ReNode,
    groups: usize,
    names: Vec<(String, usize)>,
    multiline: bool,
    sensitive: bool,
    unicode: bool,
    dot_all: bool,
    /// O padrão de origem: a cópia entre isolados recompila no destino (§2.12).
    source: Vec<u16>,
}

// ---------------------------------------------------------------------------
// Análise do padrão.

struct ReParser<'a> {
    s: &'a [u32],
    i: usize,
    unicode: bool,
    groups: usize,
    names: Vec<(String, usize)>,
    has_names: bool,
}

type ReResult<T> = Result<T, String>;

impl<'a> ReParser<'a> {
    fn end(&self) -> bool {
        self.i >= self.s.len()
    }
    fn peek(&self) -> Option<u32> {
        self.s.get(self.i).copied()
    }
    fn peek_at(&self, k: usize) -> Option<u32> {
        self.s.get(self.i + k).copied()
    }
    fn e(&self, c: char) -> bool {
        self.peek() == Some(c as u32)
    }
    fn eat(&mut self, c: char) -> bool {
        if self.e(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn disjunction(&mut self) -> ReResult<ReNode> {
        let mut alts = vec![self.alternative()?];
        while self.eat('|') {
            alts.push(self.alternative()?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { ReNode::Alt(alts) })
    }

    fn alternative(&mut self) -> ReResult<ReNode> {
        let mut terms = Vec::new();
        while !self.end() && !self.e('|') && !self.e(')') {
            terms.push(self.term()?);
        }
        Ok(match terms.len() {
            0 => ReNode::Void,
            1 => terms.pop().unwrap(),
            _ => ReNode::Seq(terms),
        })
    }

    fn term(&mut self) -> ReResult<ReNode> {
        let groups_before = self.groups;
        let c = self.peek().unwrap();
        let atom = match char::from_u32(c).unwrap_or('\0') {
            '^' => {
                self.i += 1;
                return Ok(ReNode::Start);
            }
            '$' => {
                self.i += 1;
                return Ok(ReNode::End);
            }
            '\\' if self.peek_at(1) == Some('b' as u32) => {
                self.i += 2;
                return Ok(ReNode::Boundary(true));
            }
            '\\' if self.peek_at(1) == Some('B' as u32) => {
                self.i += 2;
                return Ok(ReNode::Boundary(false));
            }
            '(' if self.peek_at(1) == Some('?' as u32) => {
                let t = self.peek_at(2);
                let behind = t == Some('<' as u32)
                    && matches!(self.peek_at(3), Some(x) if x == '=' as u32 || x == '!' as u32);
                if t == Some('=' as u32) || t == Some('!' as u32) || behind {
                    let positive = if behind { self.peek_at(3) } else { t } == Some('=' as u32);
                    self.i += if behind { 4 } else { 3 };
                    let no = self.disjunction()?;
                    if !self.eat(')') {
                        return Err("Unterminated group".into());
                    }
                    let peeks = ReNode::Peek { no: Box::new(no), positive, behind };
                    // Anexo B: lookahead pode ser quantificado no modo não
                    // unicode; o lookbehind não.
                    if behind || self.unicode {
                        return Ok(peeks);
                    }
                    peeks
                } else if t == Some(':' as u32) {
                    self.i += 3;
                    let no = self.disjunction()?;
                    if !self.eat(')') {
                        return Err("Unterminated group".into());
                    }
                    ReNode::Group(None, Box::new(no))
                } else if t == Some('<' as u32) {
                    self.i += 3;
                    let name = self.group_name()?;
                    if self.names.iter().any(|(n, _)| *n == name) {
                        return Err("Duplicate capture group name".into());
                    }
                    self.groups += 1;
                    let idx = self.groups;
                    self.names.push((name, idx));
                    let no = self.disjunction()?;
                    if !self.eat(')') {
                        return Err("Unterminated group".into());
                    }
                    ReNode::Group(Some(idx), Box::new(no))
                } else {
                    return Err("Invalid group".into());
                }
            }
            '(' => {
                self.i += 1;
                self.groups += 1;
                let idx = self.groups;
                let no = self.disjunction()?;
                if !self.eat(')') {
                    return Err("Unterminated group".into());
                }
                ReNode::Group(Some(idx), Box::new(no))
            }
            '.' => {
                self.i += 1;
                ReNode::Any
            }
            '[' => {
                self.i += 1;
                self.class()?
            }
            '\\' => {
                self.i += 1;
                self.atom_escape()?
            }
            '*' | '+' | '?' => return Err("Nothing to repeat".into()),
            '{' => {
                if self.unicode {
                    return Err("Nothing to repeat".into());
                }
                // Anexo B: `{` que não começa um quantificador é literal.
                if self.brace_quantifier_at(self.i).is_some() {
                    return Err("Nothing to repeat".into());
                }
                self.i += 1;
                ReNode::Char('{' as u32)
            }
            ')' => return Err("Unmatched ')'".into()),
            ']' | '}' if self.unicode => return Err("Lone quantifier brackets".into()),
            _ => {
                let cp = self.read_pattern_char();
                ReNode::Char(cp)
            }
        };
        self.quantifier(atom, groups_before)
    }

    /// Um caractere do padrão; no modo unicode, um par de surrogates.
    fn read_pattern_char(&mut self) -> u32 {
        let c = self.s[self.i];
        self.i += 1;
        if self.unicode
            && (0xD800..0xDC00).contains(&c)
            && let Some(b) = self.peek()
            && (0xDC00..0xE000).contains(&b)
        {
            self.i += 1;
            return 0x10000 + ((c - 0xD800) << 10) + (b - 0xDC00);
        }
        c
    }

    fn group_name(&mut self) -> ReResult<String> {
        let mut name = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("Invalid capture group name".into());
            };
            self.i += 1;
            if c == '>' as u32 {
                break;
            }
            let ch = char::from_u32(c).ok_or("Invalid capture group name")?;
            let ok = if name.is_empty() {
                ch == '$' || ch == '_' || ch.is_alphabetic()
            } else {
                ch == '$' || ch == '_' || ch.is_alphanumeric() || ch == '\u{200c}' || ch == '\u{200d}'
            };
            if !ok {
                return Err("Invalid capture group name".into());
            }
            name.push(ch);
        }
        if name.is_empty() {
            return Err("Invalid capture group name".into());
        }
        Ok(name)
    }

    /// `{n}`, `{n,}` ou `{n,m}` a partir de `i`: (min, max, índice depois).
    fn brace_quantifier_at(&self, mut i: usize) -> Option<(u32, u32, usize)> {
        if self.s.get(i) != Some(&('{' as u32)) {
            return None;
        }
        i += 1;
        let num = |i: &mut usize| -> Option<u32> {
            let begin = *i;
            let mut v: u64 = 0;
            while let Some(&c) = self.s.get(*i) {
                if !(('0' as u32)..=('9' as u32)).contains(&c) {
                    break;
                }
                v = (v * 10 + u64::from(c - '0' as u32)).min(u64::from(u32::MAX));
                *i += 1;
            }
            if *i == begin { None } else { Some(v as u32) }
        };
        let min = num(&mut i)?;
        let max = if self.s.get(i) == Some(&(',' as u32)) {
            i += 1;
            if self.s.get(i) == Some(&('}' as u32)) { u32::MAX } else { num(&mut i)? }
        } else {
            min
        };
        if self.s.get(i) != Some(&('}' as u32)) {
            return None;
        }
        Some((min, max, i + 1))
    }

    fn quantifier(&mut self, atom: ReNode, groups_before: usize) -> ReResult<ReNode> {
        let (min, max) = match self.peek().and_then(char::from_u32) {
            Some('*') => {
                self.i += 1;
                (0, u32::MAX)
            }
            Some('+') => {
                self.i += 1;
                (1, u32::MAX)
            }
            Some('?') => {
                self.i += 1;
                (0, 1)
            }
            Some('{') => match self.brace_quantifier_at(self.i) {
                Some((min, max, end)) => {
                    self.i = end;
                    if min > max {
                        return Err("numbers out of order in {} quantifier.".into());
                    }
                    (min, max)
                }
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        if matches!(atom, ReNode::Start | ReNode::End | ReNode::Boundary(_)) {
            return Err("Nothing to repeat".into());
        }
        let greedy = !self.eat('?');
        Ok(ReNode::Repeat {
            no: Box::new(atom),
            min,
            max,
            greedy,
            groups: (groups_before + 1, self.groups + 1),
        })
    }

    /// O escape depois de `\` fora de classe.
    fn atom_escape(&mut self) -> ReResult<ReNode> {
        let Some(c) = self.peek() else {
            return Err("\\ at end of pattern".into());
        };
        let ch = char::from_u32(c).unwrap_or('\0');
        if ('1'..='9').contains(&ch) {
            // Referência para trás: o número inteiro, se houver tantos
            // grupos no padrão (conferido no fim); senão, Anexo B (octal ou
            // o próprio dígito).
            let begin = self.i;
            let mut n: usize = 0;
            while let Some(d) = self.peek().filter(|d| (('0' as u32)..=('9' as u32)).contains(d)) {
                n = n.saturating_mul(10).saturating_add((d - '0' as u32) as usize);
                self.i += 1;
            }
            if n <= self.total_groups() {
                return Ok(ReNode::Back(n));
            }
            if self.unicode {
                return Err("Invalid escape".into());
            }
            self.i = begin;
            return Ok(ReNode::Char(self.octal_or_digit()));
        }
        if ch == 'k' && (self.unicode || self.has_names) {
            self.i += 1;
            if !self.eat('<') {
                return Err("Invalid named reference".into());
            }
            let name = self.group_name()?;
            return Ok(ReNode::NamedBack(name));
        }
        if let Some(item) = self.common_class_escape()? {
            return Ok(ReNode::Class(vec![item], false));
        }
        let cp = self.char_escape(false)?;
        Ok(ReNode::Char(cp))
    }

    /// O total de grupos de captura do padrão inteiro (para decidir se `\n`
    /// é referência).
    fn total_groups(&self) -> usize {
        let mut n = 0;
        let mut i = 0;
        let mut in_class = false;
        while i < self.s.len() {
            let c = self.s[i];
            if c == '\\' as u32 {
                i += 2;
                continue;
            }
            if in_class {
                if c == ']' as u32 {
                    in_class = false;
                }
            } else if c == '[' as u32 {
                in_class = true;
            } else if c == '(' as u32 {
                let next_ix = self.s.get(i + 1).copied();
                let after = self.s.get(i + 2).copied();
                let third = self.s.get(i + 3).copied();
                if next_ix != Some('?' as u32)
                    || (after == Some('<' as u32)
                        && third != Some('=' as u32)
                        && third != Some('!' as u32))
                {
                    n += 1;
                }
            }
            i += 1;
        }
        n
    }

    /// Anexo B: `\0`–`\377` octal, ou `\8`/`\9` como o dígito.
    fn octal_or_digit(&mut self) -> u32 {
        let c = self.s[self.i];
        if c >= '8' as u32 {
            self.i += 1;
            return c;
        }
        let mut v = 0u32;
        let mut n = 0;
        while n < 3 {
            let Some(d) = self.peek().filter(|d| (('0' as u32)..=('7' as u32)).contains(d)) else { break };
            let new = v * 8 + (d - '0' as u32);
            if new > 0o377 {
                break;
            }
            v = new;
            self.i += 1;
            n += 1;
        }
        v
    }

    /// `\d \D \w \W \s \S \p{..} \P{..}`, ou `None` (não consome).
    fn common_class_escape(&mut self) -> ReResult<Option<ReItem>> {
        let Some(c) = self.peek().and_then(char::from_u32) else { return Ok(None) };
        let item = match c {
            'd' => ReItem::Digit(true),
            'D' => ReItem::Digit(false),
            'w' => ReItem::Word(true),
            'W' => ReItem::Word(false),
            's' => ReItem::Space(true),
            'S' => ReItem::Space(false),
            'p' | 'P' if self.unicode => {
                self.i += 1;
                if !self.eat('{') {
                    return Err("Invalid property name".into());
                }
                let mut name = String::new();
                while let Some(x) = self.peek() {
                    self.i += 1;
                    if x == '}' as u32 {
                        break;
                    }
                    name.push(char::from_u32(x).unwrap_or('?'));
                }
                let prop = match name.trim_start_matches("General_Category=").trim_start_matches("gc=") {
                    "L" | "Letter" => ReProp::Letter,
                    "Lu" | "Uppercase_Letter" => ReProp::Upper,
                    "Ll" | "Lowercase_Letter" => ReProp::Lower,
                    "N" | "Number" => ReProp::Number,
                    "Nd" | "Decimal_Number" | "digit" => ReProp::Digit,
                    "Alphabetic" | "Alpha" => ReProp::Alphabetic,
                    "White_Space" | "space" => ReProp::Whitespace,
                    "P" | "Punctuation" | "punct" => ReProp::Punctuation,
                    "Any" => ReProp::Any,
                    "ASCII" => ReProp::Ascii,
                    "Uppercase" | "Upper" => ReProp::Upper,
                    "Lowercase" | "Lower" => ReProp::Lower,
                    _ => return Err("Invalid property name".into()),
                };
                return Ok(Some(ReItem::Property(prop, c == 'p')));
            }
            _ => return Ok(None),
        };
        self.i += 1;
        Ok(Some(item))
    }

    /// O caractere de um escape (`\n`, `\x41`, `\u{1F600}`, `\cA`, identidade).
    fn char_escape(&mut self, in_class: bool) -> ReResult<u32> {
        let c = self.s[self.i];
        self.i += 1;
        let ch = char::from_u32(c).unwrap_or('\0');
        Ok(match ch {
            'n' => 0x0A,
            'r' => 0x0D,
            't' => 0x09,
            'v' => 0x0B,
            'f' => 0x0C,
            'b' if in_class => 0x08,
            '-' if in_class => '-' as u32,
            '0' if !self.peek().is_some_and(|d| (('0' as u32)..=('9' as u32)).contains(&d)) => 0,
            '0'..='7' if !self.unicode => {
                self.i -= 1;
                self.octal_or_digit()
            }
            'c' => match self.peek() {
                Some(l) if (('a' as u32)..=('z' as u32)).contains(&l) || (('A' as u32)..=('Z' as u32)).contains(&l) => {
                    self.i += 1;
                    l % 32
                }
                Some(l) if in_class && !self.unicode && ((('0' as u32)..=('9' as u32)).contains(&l) || l == '_' as u32) => {
                    self.i += 1;
                    l % 32
                }
                _ if self.unicode => return Err("Invalid unicode escape".into()),
                _ => {
                    // Anexo B: `\c` sem letra é a barra e o `c`.
                    self.i -= 1;
                    '\\' as u32
                }
            },
            'x' => match (self.peek().and_then(hex_re), self.peek_at(1).and_then(hex_re)) {
                (Some(a), Some(b)) => {
                    self.i += 2;
                    a * 16 + b
                }
                _ if self.unicode => return Err("Invalid escape".into()),
                _ => 'x' as u32,
            },
            'u' => {
                if self.unicode && self.e('{') {
                    self.i += 1;
                    let mut v: u32 = 0;
                    let mut n = 0;
                    while let Some(h) = self.peek().and_then(hex_re) {
                        v = v.saturating_mul(16).saturating_add(h);
                        self.i += 1;
                        n += 1;
                    }
                    if n == 0 || !self.eat('}') || v > 0x10FFFF {
                        return Err("Invalid Unicode escape".into());
                    }
                    v
                } else {
                    let hs: Vec<Option<u32>> = (0..4).map(|k| self.peek_at(k).and_then(hex_re)).collect();
                    if hs.iter().all(Option::is_some) {
                        self.i += 4;
                        let v = hs.iter().fold(0, |a, h| a * 16 + h.unwrap());
                        // No modo unicode, `😀` é um ponto de código.
                        if self.unicode
                            && (0xD800..0xDC00).contains(&v)
                            && self.peek() == Some('\\' as u32)
                            && self.peek_at(1) == Some('u' as u32)
                        {
                            let ls: Vec<Option<u32>> = (2..6).map(|k| self.peek_at(k).and_then(hex_re)).collect();
                            if ls.iter().all(Option::is_some) {
                                let b = ls.iter().fold(0, |a, h| a * 16 + h.unwrap());
                                if (0xDC00..0xE000).contains(&b) {
                                    self.i += 6;
                                    return Ok(0x10000 + ((v - 0xD800) << 10) + (b - 0xDC00));
                                }
                            }
                        }
                        v
                    } else if self.unicode {
                        return Err("Invalid Unicode escape".into());
                    } else {
                        'u' as u32
                    }
                }
            }
            _ => {
                if self.unicode && !"^$\\.*+?()[]{}|/".contains(ch) {
                    return Err("Invalid escape".into());
                }
                self.i -= 1;
                self.read_pattern_char()
            }
        })
    }

    /// `[...]` (o `[` já consumido).
    fn class(&mut self) -> ReResult<ReNode> {
        let negated = self.eat('^');
        let mut items = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err("Unterminated character class".into());
            };
            if c == ']' as u32 {
                self.i += 1;
                break;
            }
            let a = self.class_atom()?;
            if self.e('-') && self.peek_at(1).is_some_and(|x| x != ']' as u32) {
                self.i += 1;
                let b = self.class_atom()?;
                match (a, b) {
                    (Ok(x), Ok(y)) => {
                        if x > y {
                            return Err("Range out of order in character class".into());
                        }
                        items.push(ReItem::Range(x, y));
                    }
                    (a, b) => {
                        if self.unicode {
                            return Err("Invalid character class".into());
                        }
                        // Anexo B: `[\d-z]` é `\d`, `-` e `z`.
                        for x in [a, Ok('-' as u32), b] {
                            match x {
                                Ok(c) => items.push(ReItem::Range(c, c)),
                                Err(item) => items.push(item),
                            }
                        }
                    }
                }
            } else {
                match a {
                    Ok(c) => items.push(ReItem::Range(c, c)),
                    Err(item) => items.push(item),
                }
            }
        }
        Ok(ReNode::Class(items, negated))
    }

    /// Um átomo de classe: um caractere (`Ok`) ou uma classe (`Err`).
    fn class_atom(&mut self) -> ReResult<Result<u32, ReItem>> {
        if self.eat('\\') {
            if self.end() {
                return Err("\\ at end of pattern".into());
            }
            if let Some(item) = self.common_class_escape()? {
                return Ok(Err(item));
            }
            return Ok(Ok(self.char_escape(true)?));
        }
        Ok(Ok(self.read_pattern_char()))
    }
}

fn hex_re(c: u32) -> Option<u32> {
    char::from_u32(c)?.to_digit(16)
}

/// Resolve `\k<name>` e confere as referências.
fn resolve_re_names(no: &mut ReNode, names: &[(String, usize)]) -> ReResult<()> {
    match no {
        ReNode::NamedBack(n) => {
            let Some((_, i)) = names.iter().find(|(x, _)| x == n) else {
                return Err("Invalid named capture referenced".into());
            };
            *no = ReNode::Back(*i);
        }
        ReNode::Group(_, f) | ReNode::Repeat { no: f, .. } | ReNode::Peek { no: f, .. } => resolve_re_names(f, names)?,
        ReNode::Seq(v) | ReNode::Alt(v) => {
            for f in v {
                resolve_re_names(f, names)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn compile_re(pattern: &[u16], multiline: bool, sensitive: bool, unicode: bool, dot_all: bool) -> ReResult<ReProgram> {
    let s: Vec<u32> = pattern.iter().map(|&u| u32::from(u)).collect();
    let has_names = {
        let mut i = 0;
        let mut found = false;
        while i + 2 < s.len() {
            if s[i] == '\\' as u32 {
                i += 2;
                continue;
            }
            if s[i] == '(' as u32
                && s[i + 1] == '?' as u32
                && s[i + 2] == '<' as u32
                && s.get(i + 3).is_some_and(|&c| c != '=' as u32 && c != '!' as u32)
            {
                found = true;
                break;
            }
            i += 1;
        }
        found
    };
    let mut a = ReParser { s: &s, i: 0, unicode, groups: 0, names: Vec::new(), has_names };
    let mut root = a.disjunction()?;
    if !a.end() {
        return Err(if a.e(')') { "Unmatched ')'".into() } else { "Unexpected character".into() });
    }
    let names = std::mem::take(&mut a.names);
    resolve_re_names(&mut root, &names)?;
    Ok(ReProgram { root, groups: a.groups, names, multiline, sensitive, unicode, dot_all, source: pattern.to_vec() })
}

// ---------------------------------------------------------------------------
// Casamento.

fn is_line_terminator(c: u32) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

fn is_re_word(c: u32) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

fn is_re_space(c: u32) -> bool {
    matches!(
        c,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

/// `Canonicalize` (ECMA-262 §22.2.2.7.3): no modo não unicode, a maiúscula
/// simples, sem levar um não ASCII para ASCII; no unicode, a minúscula
/// simples (aproximação do *simple case folding*).
fn re_canonical(c: u32, unicode: bool) -> u32 {
    let Some(ch) = char::from_u32(c) else { return c };
    if unicode {
        let mut l = ch.to_lowercase();
        return match (l.next(), l.next()) {
            (Some(x), None) => x as u32,
            _ => c,
        };
    }
    let mut u = ch.to_uppercase();
    match (u.next(), u.next()) {
        (Some(x), None) => {
            if c >= 128 && (x as u32) < 128 { c } else { x as u32 }
        }
        _ => c,
    }
}

impl ReItem {
    fn includes(&self, c: u32) -> bool {
        match *self {
            ReItem::Range(a, b) => a <= c && c <= b,
            ReItem::Digit(p) => (0x30..=0x39).contains(&c) == p,
            ReItem::Word(p) => is_re_word(c) == p,
            ReItem::Space(p) => is_re_space(c) == p,
            ReItem::Property(prop, p) => {
                let ch = char::from_u32(c);
                let v = match prop {
                    ReProp::Letter | ReProp::Alphabetic => ch.is_some_and(char::is_alphabetic),
                    ReProp::Upper => ch.is_some_and(char::is_uppercase),
                    ReProp::Lower => ch.is_some_and(char::is_lowercase),
                    ReProp::Number => ch.is_some_and(char::is_numeric),
                    ReProp::Digit => ch.is_some_and(|x| x.is_numeric() && x.to_digit(10).is_some() || x.is_ascii_digit()),
                    ReProp::Whitespace => ch.is_some_and(char::is_whitespace),
                    ReProp::Punctuation => ch.is_some_and(|x| x.is_ascii_punctuation() || (!x.is_alphanumeric() && !x.is_whitespace() && !x.is_control() && (x as u32) > 127)),
                    ReProp::Any => true,
                    ReProp::Ascii => c < 128,
                };
                v == p
            }
        }
    }
}

struct ReMachine<'a> {
    s: &'a [u16],
    p: &'a ReProgram,
    caps: Vec<i64>,
}

type Cont<'k, 'a> = &'k mut dyn FnMut(&mut ReMachine<'a>, usize) -> bool;

impl<'a> ReMachine<'a> {
    /// O caractere que começa em `pos` (para frente) ou termina em `pos`
    /// (para trás), e quantas unidades ocupa.
    fn do_read(&self, pos: usize, behind: bool) -> Option<(u32, usize)> {
        if behind {
            if pos == 0 {
                return None;
            }
            let b = u32::from(self.s[pos - 1]);
            if self.p.unicode && (0xDC00..0xE000).contains(&b) && pos >= 2 {
                let a = u32::from(self.s[pos - 2]);
                if (0xD800..0xDC00).contains(&a) {
                    return Some((0x10000 + ((a - 0xD800) << 10) + (b - 0xDC00), 2));
                }
            }
            Some((b, 1))
        } else {
            let a = u32::from(*self.s.get(pos)?);
            if self.p.unicode && (0xD800..0xDC00).contains(&a) {
                if let Some(&b) = self.s.get(pos + 1) {
                    let b = u32::from(b);
                    if (0xDC00..0xE000).contains(&b) {
                        return Some((0x10000 + ((a - 0xD800) << 10) + (b - 0xDC00), 2));
                    }
                }
            }
            Some((a, 1))
        }
    }

    fn walk(pos: usize, n: usize, behind: bool) -> usize {
        if behind { pos - n } else { pos + n }
    }

    /// Se o átomo de um caractere `no` aceita `c`.
    fn accepts(&self, no: &ReNode, c: u32) -> bool {
        match no {
            ReNode::Char(x) => {
                if self.p.sensitive {
                    *x == c
                } else {
                    re_canonical(*x, self.p.unicode) == re_canonical(c, self.p.unicode)
                }
            }
            ReNode::Any => self.p.dot_all || !is_line_terminator(c),
            ReNode::Class(items, negated) => {
                let mut inside = items.iter().any(|i| i.includes(c));
                if !inside && !self.p.sensitive {
                    let cc = re_canonical(c, self.p.unicode);
                    inside = items.iter().any(|i| match *i {
                        ReItem::Range(a, b) => {
                            (a..=b.min(a + 0x3000)).any(|x| re_canonical(x, self.p.unicode) == cc)
                        }
                        _ => false,
                    });
                }
                inside != *negated
            }
            _ => false,
        }
    }

    fn is_single_char(no: &ReNode) -> bool {
        matches!(no, ReNode::Char(_) | ReNode::Any | ReNode::Class(..))
    }

    fn in_word(&self, pos: isize) -> bool {
        pos >= 0 && (pos as usize) < self.s.len() && is_re_word(u32::from(self.s[pos as usize]))
    }

    fn match_at(&mut self, no: &ReNode, pos: usize, behind: bool, k: Cont<'_, 'a>) -> bool {
        match no {
            ReNode::Void => k(self, pos),
            ReNode::Char(_) | ReNode::Any | ReNode::Class(..) => match self.do_read(pos, behind) {
                Some((c, n)) if self.accepts(no, c) => k(self, Self::walk(pos, n, behind)),
                _ => false,
            },
            ReNode::Start => {
                let ok = pos == 0 || (self.p.multiline && is_line_terminator(u32::from(self.s[pos - 1])));
                ok && k(self, pos)
            }
            ReNode::End => {
                let ok = pos == self.s.len() || (self.p.multiline && is_line_terminator(u32::from(self.s[pos])));
                ok && k(self, pos)
            }
            ReNode::Boundary(yes) => {
                let a = self.in_word(pos as isize - 1);
                let b = self.in_word(pos as isize);
                ((a != b) == *yes) && k(self, pos)
            }
            ReNode::Group(None, f) => self.match_at(f, pos, behind, k),
            ReNode::Group(Some(i), f) => {
                let i = *i;
                self.match_at(f, pos, behind, &mut |me: &mut ReMachine<'a>, end: usize| {
                    let (a0, a1) = (me.caps[2 * i], me.caps[2 * i + 1]);
                    let (begin, fi) = if behind { (end, pos) } else { (pos, end) };
                    me.caps[2 * i] = begin as i64;
                    me.caps[2 * i + 1] = fi as i64;
                    if k(me, end) {
                        return true;
                    }
                    me.caps[2 * i] = a0;
                    me.caps[2 * i + 1] = a1;
                    false
                })
            }
            ReNode::Seq(v) => self.match_seq(v, pos, behind, k),
            ReNode::Alt(v) => {
                for f in v {
                    if self.match_at(f, pos, behind, k) {
                        return true;
                    }
                }
                false
            }
            ReNode::Repeat { no, min, max, greedy, groups } => {
                if Self::is_single_char(no) && groups.0 == groups.1 {
                    return self.simple_repeat(no, *min, *max, *greedy, pos, behind, k);
                }
                self.repeat(no, *min, *max, *greedy, *groups, 0, pos, behind, usize::MAX, k)
            }
            ReNode::Back(i) => {
                let (a, b) = (self.caps[2 * i], self.caps[2 * i + 1]);
                if a < 0 || b < 0 {
                    return k(self, pos);
                }
                let (a, b) = (a as usize, b as usize);
                let n = b - a;
                let (begin, end) = if behind {
                    if pos < n {
                        return false;
                    }
                    (pos - n, pos)
                } else {
                    if pos + n > self.s.len() {
                        return false;
                    }
                    (pos, pos + n)
                };
                let equal = (0..n).all(|j| {
                    let x = u32::from(self.s[a + j]);
                    let y = u32::from(self.s[begin + j]);
                    x == y || (!self.p.sensitive && re_canonical(x, self.p.unicode) == re_canonical(y, self.p.unicode))
                });
                equal && k(self, if behind { begin } else { end })
            }
            ReNode::NamedBack(_) => false,
            ReNode::Peek { no, positive, behind: backwards } => {
                let saved = self.caps.clone();
                let r = self.match_at(no, pos, *backwards, &mut |_: &mut ReMachine<'a>, _| true);
                if *positive {
                    if r && k(self, pos) {
                        return true;
                    }
                    self.caps = saved;
                    false
                } else {
                    self.caps = saved;
                    !r && k(self, pos)
                }
            }
        }
    }

    fn match_seq(&mut self, v: &[ReNode], pos: usize, behind: bool, k: Cont<'_, 'a>) -> bool {
        if v.is_empty() {
            return k(self, pos);
        }
        // Para trás (lookbehind), a sequência é casada da direita para a
        // esquerda.
        let (first, rest): (&ReNode, &[ReNode]) = if behind {
            (&v[v.len() - 1], &v[..v.len() - 1])
        } else {
            (&v[0], &v[1..])
        };
        self.match_at(first, pos, behind, &mut |me: &mut ReMachine<'a>, p| me.match_seq(rest, p, behind, k))
    }

    /// Repetição de um átomo de um caractere sem grupos: conta quantos
    /// casam e tenta a continuação de cada comprimento, sem recursão.
    #[allow(clippy::too_many_arguments)]
    fn simple_repeat(&mut self, no: &ReNode, min: u32, max: u32, greedy: bool, pos: usize, behind: bool, k: Cont<'_, 'a>) -> bool {
        let mut positions = vec![pos];
        let mut p = pos;
        while (positions.len() as u64 - 1) < u64::from(max) {
            match self.do_read(p, behind) {
                Some((c, n)) if self.accepts(no, c) => {
                    p = Self::walk(p, n, behind);
                    positions.push(p);
                }
                _ => break,
            }
        }
        let n = positions.len() - 1;
        if (n as u64) < u64::from(min) {
            return false;
        }
        if greedy {
            for j in (min as usize..=n).rev() {
                if k(self, positions[j]) {
                    return true;
                }
            }
        } else {
            for &q in &positions[min as usize..=n] {
                if k(self, q) {
                    return true;
                }
            }
        }
        false
    }

    /// `RepeatMatcher` (ECMA-262 §22.2.2.3.1).
    #[allow(clippy::too_many_arguments)]
    fn repeat(
        &mut self,
        no: &ReNode,
        min: u32,
        max: u32,
        greedy: bool,
        groups: (usize, usize),
        done_count: u32,
        pos: usize,
        behind: bool,
        previous_pos: usize,
        k: Cont<'_, 'a>,
    ) -> bool {
        if max == 0 {
            return k(self, pos);
        }
        // Uma iteração vazia depois do mínimo não conta (a guarda de vazio).
        if done_count >= min && pos == previous_pos {
            return false;
        }
        if min > 0 {
            return self.iteration(no, min, max, greedy, groups, done_count, pos, behind, k);
        }
        if greedy {
            if self.iteration(no, min, max, greedy, groups, done_count, pos, behind, k) {
                return true;
            }
            k(self, pos)
        } else {
            if k(self, pos) {
                return true;
            }
            self.iteration(no, min, max, greedy, groups, done_count, pos, behind, k)
        }
    }

    /// Uma iteração do átomo de uma repetição: zera as capturas dele, casa,
    /// e segue na repetição com um a menos no mínimo e no máximo.
    #[allow(clippy::too_many_arguments)]
    fn iteration(
        &mut self,
        no: &ReNode,
        min: u32,
        max: u32,
        greedy: bool,
        groups: (usize, usize),
        done_count: u32,
        pos: usize,
        behind: bool,
        k: Cont<'_, 'a>,
    ) -> bool {
        let saved: Vec<i64> = self.caps[2 * groups.0..2 * groups.1].to_vec();
        for x in &mut self.caps[2 * groups.0..2 * groups.1] {
            *x = -1;
        }
        let new_max = if max == u32::MAX { max } else { max - 1 };
        let new_min = min.saturating_sub(1);
        let done_count1 = done_count.saturating_add(1);
        let r = self.match_at(no, pos, behind, &mut |me: &mut ReMachine<'a>, p| {
            if min == 0 && p == pos {
                return false;
            }
            me.repeat(no, new_min, new_max, greedy, groups, done_count1, p, behind, pos, k)
        });
        if !r {
            self.caps[2 * groups.0..2 * groups.1].copy_from_slice(&saved);
        }
        r
    }
}

/// Casa `p` em `s` a partir de `start` (só lá, se `sticky`). Devolve as
/// posições `[início, end]` de cada grupo (−1 quando não participou).
fn run_re(p: &ReProgram, s: &[u16], start: usize, sticky: bool) -> Option<Vec<i64>> {
    let mut m = ReMachine { s, p, caps: vec![-1; 2 * (p.groups + 1)] };
    let mut i = start;
    while i <= s.len() {
        for x in m.caps.iter_mut() {
            *x = -1;
        }
        let begin = i;
        let found = m.match_at(&p.root, i, false, &mut |me: &mut ReMachine<'_>, end| {
            me.caps[0] = begin as i64;
            me.caps[1] = end as i64;
            true
        });
        if found {
            return Some(m.caps);
        }
        if sticky {
            break;
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Natives do `_RegExp` da sobreposição.

thread_local! {
    /// As posições do último casamento e a mensagem da última compilação que
    /// falhou (lidas logo em seguida pelo Dart, na mesma thread).
    static ULTIMO_RE: std::cell::RefCell<(Vec<i64>, String)> = const { std::cell::RefCell::new((Vec::new(), String::new())) };
}

/// Solta o programa de um `_ProgramaDeRegExp` morto (chamado pelo coletor).
///
/// # Safety
/// `p` saiu de `Box::into_raw` de um `Box<ReProgram>` em [`new_re_program`]
/// e é solto uma vez só.
unsafe fn release_re_program(p: *mut u8) {
    // SAFETY: contrato acima.
    drop(unsafe { Box::from_raw(p.cast::<ReProgram>()) });
}

/// Um `_ProgramaDeRegExp` com o programa `p`.
fn new_re_program(p: ReProgram) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(
            crate::layout::cid::REGEXP_PROGRAM,
            1,
            crate::layout::flags::BRUTO | crate::layout::flags::ATTACHMENT,
        );
        let bytes = std::mem::size_of::<ReProgram>() + 16 * p.source.len();
        let ptr = Box::into_raw(Box::new(p)).cast::<u8>();
        // SAFETY: `release_re_program` é a única liberação de `ptr`.
        unsafe { heap.attach(h, release_re_program, ptr, bytes) };
        h
    })
}

/// O programa do `_ProgramaDeRegExp` `h`, emprestado enquanto `h` vive.
///
/// # Panics
/// Se `h` não é um programa (bug do compilador).
fn re_program(heap: &Heap, h: i64) -> &ReProgram {
    assert!(
        crate::layout::is_object(h) && heap.class(h) == crate::layout::cid::REGEXP_PROGRAM,
        "bug do compilador: programa de RegExp esperado
handle {h}"
    );
    // SAFETY: o anexo de um programa vivo é um `ReProgram`, solto só quando o
    // bloco morre (e o empréstimo do heap impede a coleta).
    unsafe { &*heap.attachment(h).cast::<ReProgram>() }
}

/// O padrão e as opções `(multiline, sensível, unicode, dot_all)` do
/// programa `h`: a cópia entre isolados recompila no destino (§2.12).
fn re_program_pattern(heap: &Heap, h: i64) -> (Vec<u16>, [bool; 4]) {
    let p = re_program(heap, h);
    (p.source.clone(), [p.multiline, p.sensitive, p.unicode, p.dot_all])
}

/// Recompila no isolado corrente um programa copiado de outro (§2.12); o
/// padrão já compilou uma vez, então não falha.
fn re_program_from_pattern(source: &[u16], options: [bool; 4]) -> i64 {
    let p = compile_re(source, options[0], options[1], options[2], options[3]).expect("padrão de RegExp já compilado");
    new_re_program(p)
}

/// Casa o programa `h` nas unidades `s` a partir de `start` (só lá, se
/// `sticky`): as posições `[início, end]` de cada grupo.
fn regexp_match_at(h: i64, s: &[u16], start: usize, sticky: bool) -> Option<Vec<i64>> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        run_re(re_program(&heap, h), s, start, sticky)
    })
}

/// `DartForge_regexp_compilar`: compila o padrão e devolve o
/// `_ProgramaDeRegExp`, ou null com a mensagem em `DartForge_regexp_erro`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_compilar(
    pattern: i64,
    multiline: u8,
    sensitive: u8,
    unicode: u8,
    dot_all: u8,
) -> i64 {
    let r = with_string(pattern, |t| {
        let u: Vec<u16> = t.unidades().collect();
        compile_re(&u, multiline != 0, sensitive != 0, unicode != 0, dot_all != 0)
    });
    match r {
        Ok(p) => new_re_program(p),
        Err(e) => {
            ULTIMO_RE.with(|u| u.borrow_mut().1 = e);
            0
        }
    }
}

/// `DartForge_regexp_erro`: a mensagem da última compilação que falhou.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_erro() -> i64 {
    let m = ULTIMO_RE.with(|u| u.borrow().1.clone());
    allocate_str(&m)
}

/// `DartForge_regexp_grupos`: quantos grupos de captura.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_grupos(program: i64) -> i64 {
    HEAP.with(|heap| re_program(&heap.borrow(), program).groups as i64)
}

/// `DartForge_regexp_n_nomes`: quantos grupos nomeados.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_n_nomes(program: i64) -> i64 {
    HEAP.with(|heap| re_program(&heap.borrow(), program).names.len() as i64)
}

/// `DartForge_regexp_nome`: o nome do `i`-ésimo grupo nomeado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_nome(program: i64, i: i64) -> i64 {
    let n = HEAP.with(|heap| re_program(&heap.borrow(), program).names.get(i as usize).map(|x| x.0.clone()));
    allocate_str(&n.unwrap_or_default())
}

/// `DartForge_regexp_indice_do_nome`: o índice do `i`-ésimo grupo nomeado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_indice_do_nome(program: i64, i: i64) -> i64 {
    HEAP.with(|heap| re_program(&heap.borrow(), program).names.get(i as usize).map_or(-1, |x| x.1 as i64))
}

/// `DartForge_regexp_executar`: casa a partir de `start`; as posições ficam
/// para `DartForge_regexp_captura`. As unidades de uma `_TwoByteString` são
/// lidas direto do bloco; as de uma `_OneByteString` são alargadas antes (o
/// motor anda em `u16`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_executar(program: i64, target: i64, start: i64, sticky: u8) -> u8 {
    let r = HEAP.with(|heap| {
        let heap = heap.borrow();
        let p = re_program(&heap, program);
        let t = heap.text(target).expect("bug do compilador: string esperada");
        if start < 0 || start as usize > t.len() {
            return None;
        }
        match t {
            crate::textos::TextoRef::Dois(u) => run_re(p, u, start as usize, sticky != 0),
            crate::textos::TextoRef::Um(b) => {
                let u: Vec<u16> = b.iter().map(|&x| u16::from(x)).collect();
                run_re(p, &u, start as usize, sticky != 0)
            }
        }
    });
    match r {
        Some(caps) => {
            ULTIMO_RE.with(|u| u.borrow_mut().0 = caps);
            1
        }
        None => 0,
    }
}

/// `DartForge_regexp_captura`: a posição `i` do último casamento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_captura(i: i64) -> i64 {
    ULTIMO_RE.with(|u| u.borrow().0.get(i as usize).copied().unwrap_or(-1))
}
