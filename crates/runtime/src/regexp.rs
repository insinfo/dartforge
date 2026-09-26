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
// O `_RegExp` da sobreposição (`sdk_nativo/core/regexp_patch.dart`) compila
// o padrão uma vez (`DartForge_regexp_compilar`, um id numa tabela do
// processo) e pede cada casamento (`DartForge_regexp_executar`); as
// posições do último casamento são lidas por `DartForge_regexp_captura`.

/// Uma classe de caracteres (`[...]`, `\d`, `.`).
#[derive(Clone, Debug)]
enum ItemRe {
    Faixa(u32, u32),
    Digito(bool),
    Palavra(bool),
    Espaco(bool),
    Propriedade(PropRe, bool),
}

/// As propriedades Unicode de `\p{...}` que o motor conhece.
#[derive(Clone, Copy, Debug)]
enum PropRe {
    Letra,
    Maiuscula,
    Minuscula,
    Numero,
    Digito,
    Alfabetica,
    EspacoBranco,
    Pontuacao,
    Qualquer,
    Ascii,
}

#[derive(Clone, Debug)]
enum NoRe {
    Vazio,
    Char(u32),
    Qualquer,
    Classe(Vec<ItemRe>, bool),
    Inicio,
    Fim,
    Fronteira(bool),
    Grupo(Option<usize>, Box<NoRe>),
    Seq(Vec<NoRe>),
    Alt(Vec<NoRe>),
    Repete {
        no: Box<NoRe>,
        min: u32,
        max: u32,
        guloso: bool,
        /// As capturas dentro do átomo, `[de, ate)`: zeradas a cada iteração.
        grupos: (usize, usize),
    },
    Retro(usize),
    RetroNome(String),
    Olha {
        no: Box<NoRe>,
        positivo: bool,
        atras: bool,
    },
}

/// Um padrão compilado.
struct ProgramaRe {
    raiz: NoRe,
    grupos: usize,
    nomes: Vec<(String, usize)>,
    multilinha: bool,
    sensivel: bool,
    unicode: bool,
    ponto_tudo: bool,
}

// ---------------------------------------------------------------------------
// Análise do padrão.

struct AnalisadorRe<'a> {
    s: &'a [u32],
    i: usize,
    unicode: bool,
    grupos: usize,
    nomes: Vec<(String, usize)>,
    tem_nomes: bool,
}

type ResRe<T> = Result<T, String>;

impl<'a> AnalisadorRe<'a> {
    fn fim(&self) -> bool {
        self.i >= self.s.len()
    }
    fn ver(&self) -> Option<u32> {
        self.s.get(self.i).copied()
    }
    fn ver_em(&self, k: usize) -> Option<u32> {
        self.s.get(self.i + k).copied()
    }
    fn e(&self, c: char) -> bool {
        self.ver() == Some(c as u32)
    }
    fn comer(&mut self, c: char) -> bool {
        if self.e(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn disjuncao(&mut self) -> ResRe<NoRe> {
        let mut alts = vec![self.alternativa()?];
        while self.comer('|') {
            alts.push(self.alternativa()?);
        }
        Ok(if alts.len() == 1 { alts.pop().unwrap() } else { NoRe::Alt(alts) })
    }

    fn alternativa(&mut self) -> ResRe<NoRe> {
        let mut termos = Vec::new();
        while !self.fim() && !self.e('|') && !self.e(')') {
            termos.push(self.termo()?);
        }
        Ok(match termos.len() {
            0 => NoRe::Vazio,
            1 => termos.pop().unwrap(),
            _ => NoRe::Seq(termos),
        })
    }

    fn termo(&mut self) -> ResRe<NoRe> {
        let grupos_antes = self.grupos;
        let c = self.ver().unwrap();
        let atomo = match char::from_u32(c).unwrap_or('\0') {
            '^' => {
                self.i += 1;
                return Ok(NoRe::Inicio);
            }
            '$' => {
                self.i += 1;
                return Ok(NoRe::Fim);
            }
            '\\' if self.ver_em(1) == Some('b' as u32) => {
                self.i += 2;
                return Ok(NoRe::Fronteira(true));
            }
            '\\' if self.ver_em(1) == Some('B' as u32) => {
                self.i += 2;
                return Ok(NoRe::Fronteira(false));
            }
            '(' if self.ver_em(1) == Some('?' as u32) => {
                let t = self.ver_em(2);
                let atras = t == Some('<' as u32)
                    && matches!(self.ver_em(3), Some(x) if x == '=' as u32 || x == '!' as u32);
                if t == Some('=' as u32) || t == Some('!' as u32) || atras {
                    let positivo = if atras { self.ver_em(3) } else { t } == Some('=' as u32);
                    self.i += if atras { 4 } else { 3 };
                    let no = self.disjuncao()?;
                    if !self.comer(')') {
                        return Err("Unterminated group".into());
                    }
                    let olha = NoRe::Olha { no: Box::new(no), positivo, atras };
                    // Anexo B: lookahead pode ser quantificado no modo não
                    // unicode; o lookbehind não.
                    if atras || self.unicode {
                        return Ok(olha);
                    }
                    olha
                } else if t == Some(':' as u32) {
                    self.i += 3;
                    let no = self.disjuncao()?;
                    if !self.comer(')') {
                        return Err("Unterminated group".into());
                    }
                    NoRe::Grupo(None, Box::new(no))
                } else if t == Some('<' as u32) {
                    self.i += 3;
                    let nome = self.nome_de_grupo()?;
                    if self.nomes.iter().any(|(n, _)| *n == nome) {
                        return Err("Duplicate capture group name".into());
                    }
                    self.grupos += 1;
                    let idx = self.grupos;
                    self.nomes.push((nome, idx));
                    let no = self.disjuncao()?;
                    if !self.comer(')') {
                        return Err("Unterminated group".into());
                    }
                    NoRe::Grupo(Some(idx), Box::new(no))
                } else {
                    return Err("Invalid group".into());
                }
            }
            '(' => {
                self.i += 1;
                self.grupos += 1;
                let idx = self.grupos;
                let no = self.disjuncao()?;
                if !self.comer(')') {
                    return Err("Unterminated group".into());
                }
                NoRe::Grupo(Some(idx), Box::new(no))
            }
            '.' => {
                self.i += 1;
                NoRe::Qualquer
            }
            '[' => {
                self.i += 1;
                self.classe()?
            }
            '\\' => {
                self.i += 1;
                self.escape_de_atomo()?
            }
            '*' | '+' | '?' => return Err("Nothing to repeat".into()),
            '{' => {
                if self.unicode {
                    return Err("Nothing to repeat".into());
                }
                // Anexo B: `{` que não começa um quantificador é literal.
                if self.quantificador_chaves_em(self.i).is_some() {
                    return Err("Nothing to repeat".into());
                }
                self.i += 1;
                NoRe::Char('{' as u32)
            }
            ')' => return Err("Unmatched ')'".into()),
            ']' | '}' if self.unicode => return Err("Lone quantifier brackets".into()),
            _ => {
                let cp = self.ler_caractere_do_padrao();
                NoRe::Char(cp)
            }
        };
        self.quantificador(atomo, grupos_antes)
    }

    /// Um caractere do padrão; no modo unicode, um par de surrogates.
    fn ler_caractere_do_padrao(&mut self) -> u32 {
        let c = self.s[self.i];
        self.i += 1;
        if self.unicode
            && (0xD800..0xDC00).contains(&c)
            && let Some(b) = self.ver()
            && (0xDC00..0xE000).contains(&b)
        {
            self.i += 1;
            return 0x10000 + ((c - 0xD800) << 10) + (b - 0xDC00);
        }
        c
    }

    fn nome_de_grupo(&mut self) -> ResRe<String> {
        let mut nome = String::new();
        loop {
            let Some(c) = self.ver() else {
                return Err("Invalid capture group name".into());
            };
            self.i += 1;
            if c == '>' as u32 {
                break;
            }
            let ch = char::from_u32(c).ok_or("Invalid capture group name")?;
            let ok = if nome.is_empty() {
                ch == '$' || ch == '_' || ch.is_alphabetic()
            } else {
                ch == '$' || ch == '_' || ch.is_alphanumeric() || ch == '\u{200c}' || ch == '\u{200d}'
            };
            if !ok {
                return Err("Invalid capture group name".into());
            }
            nome.push(ch);
        }
        if nome.is_empty() {
            return Err("Invalid capture group name".into());
        }
        Ok(nome)
    }

    /// `{n}`, `{n,}` ou `{n,m}` a partir de `i`: (min, max, índice depois).
    fn quantificador_chaves_em(&self, mut i: usize) -> Option<(u32, u32, usize)> {
        if self.s.get(i) != Some(&('{' as u32)) {
            return None;
        }
        i += 1;
        let num = |i: &mut usize| -> Option<u32> {
            let ini = *i;
            let mut v: u64 = 0;
            while let Some(&c) = self.s.get(*i) {
                if !(('0' as u32)..=('9' as u32)).contains(&c) {
                    break;
                }
                v = (v * 10 + u64::from(c - '0' as u32)).min(u64::from(u32::MAX));
                *i += 1;
            }
            if *i == ini { None } else { Some(v as u32) }
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

    fn quantificador(&mut self, atomo: NoRe, grupos_antes: usize) -> ResRe<NoRe> {
        let (min, max) = match self.ver().and_then(char::from_u32) {
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
            Some('{') => match self.quantificador_chaves_em(self.i) {
                Some((min, max, fim)) => {
                    self.i = fim;
                    if min > max {
                        return Err("numbers out of order in {} quantifier.".into());
                    }
                    (min, max)
                }
                None => return Ok(atomo),
            },
            _ => return Ok(atomo),
        };
        if matches!(atomo, NoRe::Inicio | NoRe::Fim | NoRe::Fronteira(_)) {
            return Err("Nothing to repeat".into());
        }
        let guloso = !self.comer('?');
        Ok(NoRe::Repete {
            no: Box::new(atomo),
            min,
            max,
            guloso,
            grupos: (grupos_antes + 1, self.grupos + 1),
        })
    }

    /// O escape depois de `\` fora de classe.
    fn escape_de_atomo(&mut self) -> ResRe<NoRe> {
        let Some(c) = self.ver() else {
            return Err("\\ at end of pattern".into());
        };
        let ch = char::from_u32(c).unwrap_or('\0');
        if ('1'..='9').contains(&ch) {
            // Referência para trás: o número inteiro, se houver tantos
            // grupos no padrão (conferido no fim); senão, Anexo B (octal ou
            // o próprio dígito).
            let ini = self.i;
            let mut n: usize = 0;
            while let Some(d) = self.ver().filter(|d| (('0' as u32)..=('9' as u32)).contains(d)) {
                n = n.saturating_mul(10).saturating_add((d - '0' as u32) as usize);
                self.i += 1;
            }
            if n <= self.total_de_grupos() {
                return Ok(NoRe::Retro(n));
            }
            if self.unicode {
                return Err("Invalid escape".into());
            }
            self.i = ini;
            return Ok(NoRe::Char(self.octal_ou_digito()));
        }
        if ch == 'k' && (self.unicode || self.tem_nomes) {
            self.i += 1;
            if !self.comer('<') {
                return Err("Invalid named reference".into());
            }
            let nome = self.nome_de_grupo()?;
            return Ok(NoRe::RetroNome(nome));
        }
        if let Some(item) = self.escape_de_classe_comum()? {
            return Ok(NoRe::Classe(vec![item], false));
        }
        let cp = self.escape_de_caractere(false)?;
        Ok(NoRe::Char(cp))
    }

    /// O total de grupos de captura do padrão inteiro (para decidir se `\n`
    /// é referência).
    fn total_de_grupos(&self) -> usize {
        let mut n = 0;
        let mut i = 0;
        let mut em_classe = false;
        while i < self.s.len() {
            let c = self.s[i];
            if c == '\\' as u32 {
                i += 2;
                continue;
            }
            if em_classe {
                if c == ']' as u32 {
                    em_classe = false;
                }
            } else if c == '[' as u32 {
                em_classe = true;
            } else if c == '(' as u32 {
                let prox = self.s.get(i + 1).copied();
                let depois = self.s.get(i + 2).copied();
                let terceiro = self.s.get(i + 3).copied();
                if prox != Some('?' as u32)
                    || (depois == Some('<' as u32)
                        && terceiro != Some('=' as u32)
                        && terceiro != Some('!' as u32))
                {
                    n += 1;
                }
            }
            i += 1;
        }
        n
    }

    /// Anexo B: `\0`–`\377` octal, ou `\8`/`\9` como o dígito.
    fn octal_ou_digito(&mut self) -> u32 {
        let c = self.s[self.i];
        if c >= '8' as u32 {
            self.i += 1;
            return c;
        }
        let mut v = 0u32;
        let mut n = 0;
        while n < 3 {
            let Some(d) = self.ver().filter(|d| (('0' as u32)..=('7' as u32)).contains(d)) else { break };
            let novo = v * 8 + (d - '0' as u32);
            if novo > 0o377 {
                break;
            }
            v = novo;
            self.i += 1;
            n += 1;
        }
        v
    }

    /// `\d \D \w \W \s \S \p{..} \P{..}`, ou `None` (não consome).
    fn escape_de_classe_comum(&mut self) -> ResRe<Option<ItemRe>> {
        let Some(c) = self.ver().and_then(char::from_u32) else { return Ok(None) };
        let item = match c {
            'd' => ItemRe::Digito(true),
            'D' => ItemRe::Digito(false),
            'w' => ItemRe::Palavra(true),
            'W' => ItemRe::Palavra(false),
            's' => ItemRe::Espaco(true),
            'S' => ItemRe::Espaco(false),
            'p' | 'P' if self.unicode => {
                self.i += 1;
                if !self.comer('{') {
                    return Err("Invalid property name".into());
                }
                let mut nome = String::new();
                while let Some(x) = self.ver() {
                    self.i += 1;
                    if x == '}' as u32 {
                        break;
                    }
                    nome.push(char::from_u32(x).unwrap_or('?'));
                }
                let prop = match nome.trim_start_matches("General_Category=").trim_start_matches("gc=") {
                    "L" | "Letter" => PropRe::Letra,
                    "Lu" | "Uppercase_Letter" => PropRe::Maiuscula,
                    "Ll" | "Lowercase_Letter" => PropRe::Minuscula,
                    "N" | "Number" => PropRe::Numero,
                    "Nd" | "Decimal_Number" | "digit" => PropRe::Digito,
                    "Alphabetic" | "Alpha" => PropRe::Alfabetica,
                    "White_Space" | "space" => PropRe::EspacoBranco,
                    "P" | "Punctuation" | "punct" => PropRe::Pontuacao,
                    "Any" => PropRe::Qualquer,
                    "ASCII" => PropRe::Ascii,
                    "Uppercase" | "Upper" => PropRe::Maiuscula,
                    "Lowercase" | "Lower" => PropRe::Minuscula,
                    _ => return Err("Invalid property name".into()),
                };
                return Ok(Some(ItemRe::Propriedade(prop, c == 'p')));
            }
            _ => return Ok(None),
        };
        self.i += 1;
        Ok(Some(item))
    }

    /// O caractere de um escape (`\n`, `\x41`, `\u{1F600}`, `\cA`, identidade).
    fn escape_de_caractere(&mut self, em_classe: bool) -> ResRe<u32> {
        let c = self.s[self.i];
        self.i += 1;
        let ch = char::from_u32(c).unwrap_or('\0');
        Ok(match ch {
            'n' => 0x0A,
            'r' => 0x0D,
            't' => 0x09,
            'v' => 0x0B,
            'f' => 0x0C,
            'b' if em_classe => 0x08,
            '-' if em_classe => '-' as u32,
            '0' if !self.ver().is_some_and(|d| (('0' as u32)..=('9' as u32)).contains(&d)) => 0,
            '0'..='7' if !self.unicode => {
                self.i -= 1;
                self.octal_ou_digito()
            }
            'c' => match self.ver() {
                Some(l) if (('a' as u32)..=('z' as u32)).contains(&l) || (('A' as u32)..=('Z' as u32)).contains(&l) => {
                    self.i += 1;
                    l % 32
                }
                Some(l) if em_classe && !self.unicode && ((('0' as u32)..=('9' as u32)).contains(&l) || l == '_' as u32) => {
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
            'x' => match (self.ver().and_then(hex_re), self.ver_em(1).and_then(hex_re)) {
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
                    while let Some(h) = self.ver().and_then(hex_re) {
                        v = v.saturating_mul(16).saturating_add(h);
                        self.i += 1;
                        n += 1;
                    }
                    if n == 0 || !self.comer('}') || v > 0x10FFFF {
                        return Err("Invalid Unicode escape".into());
                    }
                    v
                } else {
                    let hs: Vec<Option<u32>> = (0..4).map(|k| self.ver_em(k).and_then(hex_re)).collect();
                    if hs.iter().all(Option::is_some) {
                        self.i += 4;
                        let v = hs.iter().fold(0, |a, h| a * 16 + h.unwrap());
                        // No modo unicode, `😀` é um ponto de código.
                        if self.unicode
                            && (0xD800..0xDC00).contains(&v)
                            && self.ver() == Some('\\' as u32)
                            && self.ver_em(1) == Some('u' as u32)
                        {
                            let ls: Vec<Option<u32>> = (2..6).map(|k| self.ver_em(k).and_then(hex_re)).collect();
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
                self.ler_caractere_do_padrao()
            }
        })
    }

    /// `[...]` (o `[` já consumido).
    fn classe(&mut self) -> ResRe<NoRe> {
        let negada = self.comer('^');
        let mut itens = Vec::new();
        loop {
            let Some(c) = self.ver() else {
                return Err("Unterminated character class".into());
            };
            if c == ']' as u32 {
                self.i += 1;
                break;
            }
            let a = self.atomo_de_classe()?;
            if self.e('-') && self.ver_em(1).is_some_and(|x| x != ']' as u32) {
                self.i += 1;
                let b = self.atomo_de_classe()?;
                match (a, b) {
                    (Ok(x), Ok(y)) => {
                        if x > y {
                            return Err("Range out of order in character class".into());
                        }
                        itens.push(ItemRe::Faixa(x, y));
                    }
                    (a, b) => {
                        if self.unicode {
                            return Err("Invalid character class".into());
                        }
                        // Anexo B: `[\d-z]` é `\d`, `-` e `z`.
                        for x in [a, Ok('-' as u32), b] {
                            match x {
                                Ok(c) => itens.push(ItemRe::Faixa(c, c)),
                                Err(item) => itens.push(item),
                            }
                        }
                    }
                }
            } else {
                match a {
                    Ok(c) => itens.push(ItemRe::Faixa(c, c)),
                    Err(item) => itens.push(item),
                }
            }
        }
        Ok(NoRe::Classe(itens, negada))
    }

    /// Um átomo de classe: um caractere (`Ok`) ou uma classe (`Err`).
    fn atomo_de_classe(&mut self) -> ResRe<Result<u32, ItemRe>> {
        if self.comer('\\') {
            if self.fim() {
                return Err("\\ at end of pattern".into());
            }
            if let Some(item) = self.escape_de_classe_comum()? {
                return Ok(Err(item));
            }
            return Ok(Ok(self.escape_de_caractere(true)?));
        }
        Ok(Ok(self.ler_caractere_do_padrao()))
    }
}

fn hex_re(c: u32) -> Option<u32> {
    char::from_u32(c)?.to_digit(16)
}

/// Resolve `\k<nome>` e confere as referências.
fn resolver_nomes_re(no: &mut NoRe, nomes: &[(String, usize)]) -> ResRe<()> {
    match no {
        NoRe::RetroNome(n) => {
            let Some((_, i)) = nomes.iter().find(|(x, _)| x == n) else {
                return Err("Invalid named capture referenced".into());
            };
            *no = NoRe::Retro(*i);
        }
        NoRe::Grupo(_, f) | NoRe::Repete { no: f, .. } | NoRe::Olha { no: f, .. } => resolver_nomes_re(f, nomes)?,
        NoRe::Seq(v) | NoRe::Alt(v) => {
            for f in v {
                resolver_nomes_re(f, nomes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn compilar_re(padrao: &[u16], multilinha: bool, sensivel: bool, unicode: bool, ponto_tudo: bool) -> ResRe<ProgramaRe> {
    let s: Vec<u32> = padrao.iter().map(|&u| u32::from(u)).collect();
    let tem_nomes = {
        let mut i = 0;
        let mut achou = false;
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
                achou = true;
                break;
            }
            i += 1;
        }
        achou
    };
    let mut a = AnalisadorRe { s: &s, i: 0, unicode, grupos: 0, nomes: Vec::new(), tem_nomes };
    let mut raiz = a.disjuncao()?;
    if !a.fim() {
        return Err(if a.e(')') { "Unmatched ')'".into() } else { "Unexpected character".into() });
    }
    let nomes = std::mem::take(&mut a.nomes);
    resolver_nomes_re(&mut raiz, &nomes)?;
    Ok(ProgramaRe { raiz, grupos: a.grupos, nomes, multilinha, sensivel, unicode, ponto_tudo })
}

// ---------------------------------------------------------------------------
// Casamento.

fn e_terminador_de_linha(c: u32) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

fn e_palavra_re(c: u32) -> bool {
    matches!(c, 0x30..=0x39 | 0x41..=0x5A | 0x61..=0x7A | 0x5F)
}

fn e_espaco_re(c: u32) -> bool {
    matches!(
        c,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

/// `Canonicalize` (ECMA-262 §22.2.2.7.3): no modo não unicode, a maiúscula
/// simples, sem levar um não ASCII para ASCII; no unicode, a minúscula
/// simples (aproximação do *simple case folding*).
fn canonico_re(c: u32, unicode: bool) -> u32 {
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

impl ItemRe {
    fn contem(&self, c: u32) -> bool {
        match *self {
            ItemRe::Faixa(a, b) => a <= c && c <= b,
            ItemRe::Digito(p) => (0x30..=0x39).contains(&c) == p,
            ItemRe::Palavra(p) => e_palavra_re(c) == p,
            ItemRe::Espaco(p) => e_espaco_re(c) == p,
            ItemRe::Propriedade(prop, p) => {
                let ch = char::from_u32(c);
                let v = match prop {
                    PropRe::Letra | PropRe::Alfabetica => ch.is_some_and(char::is_alphabetic),
                    PropRe::Maiuscula => ch.is_some_and(char::is_uppercase),
                    PropRe::Minuscula => ch.is_some_and(char::is_lowercase),
                    PropRe::Numero => ch.is_some_and(char::is_numeric),
                    PropRe::Digito => ch.is_some_and(|x| x.is_numeric() && x.to_digit(10).is_some() || x.is_ascii_digit()),
                    PropRe::EspacoBranco => ch.is_some_and(char::is_whitespace),
                    PropRe::Pontuacao => ch.is_some_and(|x| x.is_ascii_punctuation() || (!x.is_alphanumeric() && !x.is_whitespace() && !x.is_control() && (x as u32) > 127)),
                    PropRe::Qualquer => true,
                    PropRe::Ascii => c < 128,
                };
                v == p
            }
        }
    }
}

struct MaquinaRe<'a> {
    s: &'a [u16],
    p: &'a ProgramaRe,
    caps: Vec<i64>,
}

type Cont<'k, 'a> = &'k mut dyn FnMut(&mut MaquinaRe<'a>, usize) -> bool;

impl<'a> MaquinaRe<'a> {
    /// O caractere que começa em `pos` (para frente) ou termina em `pos`
    /// (para trás), e quantas unidades ocupa.
    fn ler(&self, pos: usize, atras: bool) -> Option<(u32, usize)> {
        if atras {
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

    fn andar(pos: usize, n: usize, atras: bool) -> usize {
        if atras { pos - n } else { pos + n }
    }

    /// Se o átomo de um caractere `no` aceita `c`.
    fn aceita(&self, no: &NoRe, c: u32) -> bool {
        match no {
            NoRe::Char(x) => {
                if self.p.sensivel {
                    *x == c
                } else {
                    canonico_re(*x, self.p.unicode) == canonico_re(c, self.p.unicode)
                }
            }
            NoRe::Qualquer => self.p.ponto_tudo || !e_terminador_de_linha(c),
            NoRe::Classe(itens, negada) => {
                let mut dentro = itens.iter().any(|i| i.contem(c));
                if !dentro && !self.p.sensivel {
                    let cc = canonico_re(c, self.p.unicode);
                    dentro = itens.iter().any(|i| match *i {
                        ItemRe::Faixa(a, b) => {
                            (a..=b.min(a + 0x3000)).any(|x| canonico_re(x, self.p.unicode) == cc)
                        }
                        _ => false,
                    });
                }
                dentro != *negada
            }
            _ => false,
        }
    }

    fn e_um_caractere(no: &NoRe) -> bool {
        matches!(no, NoRe::Char(_) | NoRe::Qualquer | NoRe::Classe(..))
    }

    fn em_palavra(&self, pos: isize) -> bool {
        pos >= 0 && (pos as usize) < self.s.len() && e_palavra_re(u32::from(self.s[pos as usize]))
    }

    fn casar(&mut self, no: &NoRe, pos: usize, atras: bool, k: Cont<'_, 'a>) -> bool {
        match no {
            NoRe::Vazio => k(self, pos),
            NoRe::Char(_) | NoRe::Qualquer | NoRe::Classe(..) => match self.ler(pos, atras) {
                Some((c, n)) if self.aceita(no, c) => k(self, Self::andar(pos, n, atras)),
                _ => false,
            },
            NoRe::Inicio => {
                let ok = pos == 0 || (self.p.multilinha && e_terminador_de_linha(u32::from(self.s[pos - 1])));
                ok && k(self, pos)
            }
            NoRe::Fim => {
                let ok = pos == self.s.len() || (self.p.multilinha && e_terminador_de_linha(u32::from(self.s[pos])));
                ok && k(self, pos)
            }
            NoRe::Fronteira(sim) => {
                let a = self.em_palavra(pos as isize - 1);
                let b = self.em_palavra(pos as isize);
                ((a != b) == *sim) && k(self, pos)
            }
            NoRe::Grupo(None, f) => self.casar(f, pos, atras, k),
            NoRe::Grupo(Some(i), f) => {
                let i = *i;
                self.casar(f, pos, atras, &mut |me: &mut MaquinaRe<'a>, fim: usize| {
                    let (a0, a1) = (me.caps[2 * i], me.caps[2 * i + 1]);
                    let (ini, fi) = if atras { (fim, pos) } else { (pos, fim) };
                    me.caps[2 * i] = ini as i64;
                    me.caps[2 * i + 1] = fi as i64;
                    if k(me, fim) {
                        return true;
                    }
                    me.caps[2 * i] = a0;
                    me.caps[2 * i + 1] = a1;
                    false
                })
            }
            NoRe::Seq(v) => self.casar_seq(v, pos, atras, k),
            NoRe::Alt(v) => {
                for f in v {
                    if self.casar(f, pos, atras, k) {
                        return true;
                    }
                }
                false
            }
            NoRe::Repete { no, min, max, guloso, grupos } => {
                if Self::e_um_caractere(no) && grupos.0 == grupos.1 {
                    return self.repetir_simples(no, *min, *max, *guloso, pos, atras, k);
                }
                self.repetir(no, *min, *max, *guloso, *grupos, 0, pos, atras, usize::MAX, k)
            }
            NoRe::Retro(i) => {
                let (a, b) = (self.caps[2 * i], self.caps[2 * i + 1]);
                if a < 0 || b < 0 {
                    return k(self, pos);
                }
                let (a, b) = (a as usize, b as usize);
                let n = b - a;
                let (ini, fim) = if atras {
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
                let iguais = (0..n).all(|j| {
                    let x = u32::from(self.s[a + j]);
                    let y = u32::from(self.s[ini + j]);
                    x == y || (!self.p.sensivel && canonico_re(x, self.p.unicode) == canonico_re(y, self.p.unicode))
                });
                iguais && k(self, if atras { ini } else { fim })
            }
            NoRe::RetroNome(_) => false,
            NoRe::Olha { no, positivo, atras: para_tras } => {
                let salvo = self.caps.clone();
                let r = self.casar(no, pos, *para_tras, &mut |_: &mut MaquinaRe<'a>, _| true);
                if *positivo {
                    if r && k(self, pos) {
                        return true;
                    }
                    self.caps = salvo;
                    false
                } else {
                    self.caps = salvo;
                    !r && k(self, pos)
                }
            }
        }
    }

    fn casar_seq(&mut self, v: &[NoRe], pos: usize, atras: bool, k: Cont<'_, 'a>) -> bool {
        if v.is_empty() {
            return k(self, pos);
        }
        // Para trás (lookbehind), a sequência é casada da direita para a
        // esquerda.
        let (primeiro, resto): (&NoRe, &[NoRe]) = if atras {
            (&v[v.len() - 1], &v[..v.len() - 1])
        } else {
            (&v[0], &v[1..])
        };
        self.casar(primeiro, pos, atras, &mut |me: &mut MaquinaRe<'a>, p| me.casar_seq(resto, p, atras, k))
    }

    /// Repetição de um átomo de um caractere sem grupos: conta quantos
    /// casam e tenta a continuação de cada comprimento, sem recursão.
    #[allow(clippy::too_many_arguments)]
    fn repetir_simples(&mut self, no: &NoRe, min: u32, max: u32, guloso: bool, pos: usize, atras: bool, k: Cont<'_, 'a>) -> bool {
        let mut posicoes = vec![pos];
        let mut p = pos;
        while (posicoes.len() as u64 - 1) < u64::from(max) {
            match self.ler(p, atras) {
                Some((c, n)) if self.aceita(no, c) => {
                    p = Self::andar(p, n, atras);
                    posicoes.push(p);
                }
                _ => break,
            }
        }
        let n = posicoes.len() - 1;
        if (n as u64) < u64::from(min) {
            return false;
        }
        if guloso {
            for j in (min as usize..=n).rev() {
                if k(self, posicoes[j]) {
                    return true;
                }
            }
        } else {
            for &q in &posicoes[min as usize..=n] {
                if k(self, q) {
                    return true;
                }
            }
        }
        false
    }

    /// `RepeatMatcher` (ECMA-262 §22.2.2.3.1).
    #[allow(clippy::too_many_arguments)]
    fn repetir(
        &mut self,
        no: &NoRe,
        min: u32,
        max: u32,
        guloso: bool,
        grupos: (usize, usize),
        feitas: u32,
        pos: usize,
        atras: bool,
        pos_anterior: usize,
        k: Cont<'_, 'a>,
    ) -> bool {
        if max == 0 {
            return k(self, pos);
        }
        // Uma iteração vazia depois do mínimo não conta (a guarda de vazio).
        if feitas >= min && pos == pos_anterior {
            return false;
        }
        if min > 0 {
            return self.iteracao(no, min, max, guloso, grupos, feitas, pos, atras, k);
        }
        if guloso {
            if self.iteracao(no, min, max, guloso, grupos, feitas, pos, atras, k) {
                return true;
            }
            k(self, pos)
        } else {
            if k(self, pos) {
                return true;
            }
            self.iteracao(no, min, max, guloso, grupos, feitas, pos, atras, k)
        }
    }

    /// Uma iteração do átomo de uma repetição: zera as capturas dele, casa,
    /// e segue na repetição com um a menos no mínimo e no máximo.
    #[allow(clippy::too_many_arguments)]
    fn iteracao(
        &mut self,
        no: &NoRe,
        min: u32,
        max: u32,
        guloso: bool,
        grupos: (usize, usize),
        feitas: u32,
        pos: usize,
        atras: bool,
        k: Cont<'_, 'a>,
    ) -> bool {
        let salvo: Vec<i64> = self.caps[2 * grupos.0..2 * grupos.1].to_vec();
        for x in &mut self.caps[2 * grupos.0..2 * grupos.1] {
            *x = -1;
        }
        let novo_max = if max == u32::MAX { max } else { max - 1 };
        let novo_min = min.saturating_sub(1);
        let feitas1 = feitas.saturating_add(1);
        let r = self.casar(no, pos, atras, &mut |me: &mut MaquinaRe<'a>, p| {
            if min == 0 && p == pos {
                return false;
            }
            me.repetir(no, novo_min, novo_max, guloso, grupos, feitas1, p, atras, pos, k)
        });
        if !r {
            self.caps[2 * grupos.0..2 * grupos.1].copy_from_slice(&salvo);
        }
        r
    }
}

/// Casa `p` em `s` a partir de `inicio` (só lá, se `pegajoso`). Devolve as
/// posições `[início, fim]` de cada grupo (−1 quando não participou).
fn executar_re(p: &ProgramaRe, s: &[u16], inicio: usize, pegajoso: bool) -> Option<Vec<i64>> {
    let mut m = MaquinaRe { s, p, caps: vec![-1; 2 * (p.grupos + 1)] };
    let mut i = inicio;
    while i <= s.len() {
        for x in m.caps.iter_mut() {
            *x = -1;
        }
        let ini = i;
        let achou = m.casar(&p.raiz, i, false, &mut |me: &mut MaquinaRe<'_>, fim| {
            me.caps[0] = ini as i64;
            me.caps[1] = fim as i64;
            true
        });
        if achou {
            return Some(m.caps);
        }
        if pegajoso {
            break;
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Natives do `_RegExp` da sobreposição.

thread_local! {
    static PROGRAMAS_RE: std::cell::RefCell<Vec<ProgramaRe>> = const { std::cell::RefCell::new(Vec::new()) };
    static ULTIMO_RE: std::cell::RefCell<(Vec<i64>, String)> = const { std::cell::RefCell::new((Vec::new(), String::new())) };
}

fn unidades_re(handle: i64) -> Vec<u16> {
    match texto_de(handle) {
        Texto::Um(b) => b.into_iter().map(u16::from).collect(),
        Texto::Dois(v) => v,
    }
}

/// `DartForge_regexp_compilar`: compila o padrão e devolve o id, ou −1 com
/// a mensagem em `DartForge_regexp_erro`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_compilar(
    padrao: i64,
    multilinha: u8,
    sensivel: u8,
    unicode: u8,
    ponto_tudo: u8,
) -> i64 {
    let u = unidades_re(padrao);
    match compilar_re(&u, multilinha != 0, sensivel != 0, unicode != 0, ponto_tudo != 0) {
        Ok(p) => PROGRAMAS_RE.with(|v| {
            let mut v = v.borrow_mut();
            v.push(p);
            (v.len() - 1) as i64
        }),
        Err(e) => {
            ULTIMO_RE.with(|u| u.borrow_mut().1 = e);
            -1
        }
    }
}

/// `DartForge_regexp_erro`: a mensagem da última compilação que falhou.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_erro() -> i64 {
    let m = ULTIMO_RE.with(|u| u.borrow().1.clone());
    alocar_str(&m)
}

/// `DartForge_regexp_grupos`: quantos grupos de captura.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_grupos(id: i64) -> i64 {
    PROGRAMAS_RE.with(|v| v.borrow().get(id as usize).map_or(0, |p| p.grupos as i64))
}

/// `DartForge_regexp_n_nomes`: quantos grupos nomeados.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_n_nomes(id: i64) -> i64 {
    PROGRAMAS_RE.with(|v| v.borrow().get(id as usize).map_or(0, |p| p.nomes.len() as i64))
}

/// `DartForge_regexp_nome`: o nome do `i`-ésimo grupo nomeado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_nome(id: i64, i: i64) -> i64 {
    let n = PROGRAMAS_RE.with(|v| v.borrow().get(id as usize).and_then(|p| p.nomes.get(i as usize)).map(|x| x.0.clone()));
    alocar_str(&n.unwrap_or_default())
}

/// `DartForge_regexp_indice_do_nome`: o índice do `i`-ésimo grupo nomeado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_indice_do_nome(id: i64, i: i64) -> i64 {
    PROGRAMAS_RE.with(|v| v.borrow().get(id as usize).and_then(|p| p.nomes.get(i as usize)).map_or(-1, |x| x.1 as i64))
}

/// `DartForge_regexp_executar`: casa a partir de `inicio`; as posições ficam
/// para `DartForge_regexp_captura`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_regexp_executar(id: i64, alvo: i64, inicio: i64, pegajoso: u8) -> u8 {
    let s = unidades_re(alvo);
    if inicio < 0 || inicio as usize > s.len() {
        return 0;
    }
    let r = PROGRAMAS_RE.with(|v| {
        let v = v.borrow();
        let p = v.get(id as usize)?;
        executar_re(p, &s, inicio as usize, pegajoso != 0)
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
