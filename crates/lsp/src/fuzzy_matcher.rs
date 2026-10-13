//! O `FuzzyMatcher` do analyzer (`AN:src/utilities/fuzzy_matcher.dart`;
//! docs/LSP-ESPECIFICACAO.md §14.7.2): o score de um candidato contra o
//! padrão digitado, em unidades UTF-16 como o Dart, com os três estilos
//! (`TEXT` no completar, `SYMBOL` no `workspace/symbol`, `FILENAME`).
//! Escrito sem compilar nem executar (2026-10-05).

/// `MatchStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Style {
    Text,
    Symbol,
    File,
}

/// `CharRole`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    None,
    Separator,
    Tail,
    UppercaseTail,
    Head,
}

/// `_CharType` (`NONE`, `PUNCT`, `LOWER`, `UPPER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Type {
    None = 0,
    Punctuation = 1,
    Lowercase = 2,
    Uppercase = 3,
}

const MAX_ENTRY: usize = 127;
const MAX_PATTERN: usize = 63;
const MIN_SCORE: i32 = -10000;

/// A tabela `TYPES` (os códigos 0..127).
const TYPES: &[u8; 128] = b"0000000000000000000000000000000010000000000000112222222222100000\
0333333333333333333333333330000002222222222222222222222222200000";

fn ty(c: u16) -> Type {
    if c >= 128 {
        return Type::Lowercase;
    }
    match TYPES[c as usize] {
        b'1' => Type::Punctuation,
        b'2' => Type::Lowercase,
        b'3' => Type::Uppercase,
        _ => Type::None,
    }
}

/// `toLowerCase` em UTF-16, mantendo o comprimento (o que o casamento por
/// índice exige; um caractere cuja minúscula muda o comprimento fica como
/// está).
fn lowercase(s: &[u16]) -> Vec<u16> {
    let text = String::from_utf16_lossy(s);
    let low: Vec<u16> = text.to_lowercase().encode_utf16().collect();
    if low.len() == s.len() {
        return low;
    }
    s.iter().map(|&c| if (b'A' as u16..=b'Z' as u16).contains(&c) { c + 32 } else { c }).collect()
}

/// Um casador para um padrão.
pub(crate) struct Matcher {
    pattern: Vec<u16>,
    pattern_min: Vec<u16>,
    short: Vec<u16>,
    style: Style,
    sensitive: bool,
    offset: usize,
    default_roles: Vec<Role>,
    roles: Vec<Role>,
    table: Vec<Vec<i32>>,
    scale: f64,
    last_length: usize,
}

impl Matcher {
    /// `FuzzyMatcher(pattern, matchStyle)`.
    pub(crate) fn new(pattern: &str, style: Style) -> Matcher {
        let mut p: Vec<u16> = pattern.encode_utf16().collect();
        p.truncate(MAX_PATTERN);
        let pattern_min = lowercase(&p);
        let sensitive = p != pattern_min;
        let short = pattern_min[..pattern_min.len().min(3)].to_vec();
        let offset = p.len() + 1;
        let table = vec![vec![0i32; 2 * offset]; MAX_ENTRY + 1];
        let mut c = Matcher {
            pattern: p,
            pattern_min,
            short,
            style,
            sensitive,
            offset,
            default_roles: Vec::new(),
            roles: vec![Role::None; MAX_ENTRY],
            table,
            scale: 0.0,
            last_length: 0,
        };
        let mut roles = vec![Role::None; c.pattern.len()];
        let pattern = c.pattern.clone();
        c.map(&pattern, &mut roles);
        c.default_roles = roles;
        let max = if style == Style::Text { 6.0 } else { 4.0 };
        c.scale = if c.pattern.is_empty() { 0.0 } else { 1.0 / (max * c.pattern.len() as f64) };
        c
    }

    /// `fuzzyMap`.
    fn map(&self, s: &[u16], roles: &mut [Role]) {
        let mut previous = Type::None;
        for (i, &c) in s.iter().enumerate() {
            let t = ty(c);
            let mut role = Role::None;
            match t {
                Type::Lowercase => role = if previous <= Type::Punctuation { Role::Head } else { Role::Tail },
                Type::Uppercase => {
                    role = Role::Head;
                    let next_lowercase = i + 1 < s.len() && (b'a' as u16..=b'z' as u16).contains(&s[i + 1]);
                    if previous == Type::Uppercase && !next_lowercase {
                        role = Role::UppercaseTail;
                    }
                }
                Type::Punctuation => {
                    let separates = (self.style == Style::File && c == b'/' as u16)
                        || (self.style == Style::Symbol && (c == b'.' as u16 || c == b':' as u16 || c == b' ' as u16));
                    if separates {
                        role = Role::Separator;
                    }
                }
                Type::None => {}
            }
            roles[i] = role;
            previous = t;
        }
        let mut i = s.len();
        while i > 0 && roles[i - 1] == Role::Separator {
            roles[i - 1] = Role::None;
            i -= 1;
        }
    }

    fn score_in(&self, i: usize, j: usize, k: usize) -> i32 {
        self.table[i][j + k * self.offset] >> 1
    }

    fn best_k(&self, i: usize, j: usize) -> usize {
        if self.score_in(i, j, 0) < self.score_in(i, j, 1) { 1 } else { 0 }
    }

    fn previous_k(&self, i: usize, j: usize, k: usize) -> usize {
        (self.table[i][j + k * self.offset] & 1) as usize
    }

    /// `computeScore`.
    fn compute(&mut self, cand: &[u16], cand_min: &[u16]) -> i32 {
        let m = self.pattern.len();
        let o = self.offset;
        for j in 0..=m {
            self.table[0][j] = if j == 0 { 0 } else { MIN_SCORE << 1 };
            self.table[0][o + j] = MIN_SCORE << 1;
        }
        let mut segments = 1;
        let mut last_start = 0usize;
        for i in 0..cand.len() {
            if self.roles[i] == Role::Separator {
                segments += 1;
                last_start = i + 1;
            }
        }
        for i in 1..=cand.len() {
            let head = self.roles[i - 1] == Role::Head;
            if self.roles[i - 1] == Role::Separator && segments > 1 {
                segments -= 1;
            }
            let seg_score = if segments > 1 { 0 } else { 1 };
            let mut worth = 0;
            if segments == 1 && head && self.style != Style::Text {
                worth += 1;
            }
            if i - 1 == last_start {
                worth += 3;
            }
            for j in 0..=m {
                self.table[i][o + j] = MIN_SCORE << 1;
                if segments > 1 && j == m {
                    self.table[i][j] = MIN_SCORE << 1;
                    continue;
                }
                let k = self.best_k(i - 1, j);
                let mut skip = self.score_in(i - 1, j, k);
                if j != m {
                    skip -= worth;
                }
                self.table[i][j] = (skip << 1) + k as i32;
                if j == 0 || cand_min[i - 1] != self.pattern_min[j - 1] {
                    continue;
                }
                let mut dot = seg_score;
                if self.roles[i - 1] == Role::Tail && self.default_roles[j - 1] == Role::Head {
                    if j > 1 {
                        continue;
                    }
                    let end = cand_min.len().min(i - 1 + self.short.len());
                    if self.short[..] != cand_min[i - 1..end] {
                        continue;
                    }
                    dot -= 4;
                }
                if cand[i - 1] == self.pattern[j - 1] || (head && (!self.sensitive || self.default_roles[j - 1] == Role::Head)) {
                    dot += 1;
                }
                for k in 0..2 {
                    let mut s = self.score_in(i - 1, j - 1, k) + dot;
                    let previous_matched = k == 1;
                    let consecutive = previous_matched || i - 1 == 0 || i - 1 == last_start;
                    if consecutive || (self.style == Style::Text && j - 1 == 0) {
                        s += if self.style == Style::Text { 4 } else { 2 };
                    }
                    if !previous_matched && matches!(self.roles[i - 1], Role::Tail | Role::UppercaseTail) {
                        s -= 3;
                    }
                    if s > (self.table[i][o + j] >> 1) {
                        self.table[i][o + j] = (s << 1) + k as i32;
                    }
                }
            }
        }
        let n = cand.len();
        self.score_in(n, m, self.best_k(n, m))
    }

    /// `isPoorMatch`.
    fn bad(&self) -> bool {
        if self.pattern.len() < 2 {
            return false;
        }
        let mut i = self.last_length;
        let mut j = self.pattern.len();
        let mut k = self.best_k(i, j);
        let mut counter = 0;
        let mut length = 0;
        while i > 0 {
            let takes = k == 1;
            k = self.previous_k(i, j, k);
            if takes {
                length += 1;
                if k == 0 && length < 3 && self.roles[i - 1] == Role::Tail {
                    counter += 1;
                    if counter > 1 {
                        return true;
                    }
                }
                j -= 1;
            } else {
                length = 0;
            }
            i -= 1;
        }
        false
    }

    /// `match`.
    fn matches(&mut self, cand: &[u16], cand_min: &[u16]) -> bool {
        let mut i = 0;
        let mut j = 0;
        while i < cand_min.len() && j < self.pattern_min.len() {
            if cand_min[i] == self.pattern_min[j] {
                j += 1;
            }
            i += 1;
        }
        if j != self.pattern_min.len() {
            return false;
        }
        let mut roles = std::mem::take(&mut self.roles);
        self.map(cand, &mut roles);
        self.roles = roles;
        if self.style != Style::Text {
            let mut sep = cand_min.len() as isize - 1;
            while sep >= i as isize && self.roles[sep as usize] != Role::Separator {
                sep -= 1;
            }
            if sep >= i as isize {
                let last = *self.pattern_min.last().unwrap_or(&0);
                return cand_min[sep as usize..].contains(&last);
            }
        }
        true
    }

    /// `score(candidate)`: −1 quando não casa; 1 no padrão vazio.
    pub(crate) fn score(&mut self, candidate: &str) -> f64 {
        let mut cand: Vec<u16> = candidate.encode_utf16().collect();
        if cand.len() > MAX_ENTRY {
            if self.style == Style::File {
                cand = cand[cand.len() - MAX_ENTRY..].to_vec();
            } else {
                cand.truncate(MAX_ENTRY);
            }
        }
        if self.pattern.is_empty() {
            return 1.0;
        }
        self.last_length = cand.len();
        let cand_min = lowercase(&cand);
        if self.matches(&cand, &cand_min) {
            let mut s = self.compute(&cand, &cand_min);
            if s > MIN_SCORE / 2 && !self.bad() {
                if self.pattern.len() == cand.len() {
                    return 1.0;
                }
                if s < 0 {
                    s = 0;
                }
                let n = s as f64 * self.scale;
                return if n > 1.0 { 1.0 } else { n };
            }
        }
        -1.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Matcher, Style};

    fn s(p: &str, c: &str) -> f64 {
        Matcher::new(p, Style::Text).score(c)
    }

    #[test]
    fn spec_examples() {
        assert_eq!(s("St", "StateError"), 1.0);
        assert_eq!(s("St", "StackTrace"), 1.0);
        assert_eq!(s("st", "StateError"), 1.0);
        assert!((s("st", "toString") - 0.75).abs() < 1e-9);
        assert_eq!(s("pr", "print"), 1.0);
        assert_eq!(s("print", "print"), 1.0);
        assert_eq!(s("x", "max"), 0.0);
        assert_eq!(s("", "qualquer"), 1.0);
        assert_eq!(s("zz", "print"), -1.0);
    }
}
