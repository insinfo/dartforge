//! A semântica do `package:glob` 2.1.3, que decide `sources`, `generate_for`
//! e `findAssets` no `build_runner`.
//!
//! Parser: `glob-2.1.3/lib/src/parser.dart`, linha por linha. Casamento: a
//! regex de `ast.dart` (`^…$`): `*` → `[^/]*`, `**` → `[^]*` (tudo, inclusive
//! `/`), `?` → `[^/]`, faixas nunca casam `/`, `{a,b}` → alternativa. É um
//! casador por retrocesso sobre a árvore, sem motor de regex.
//!
//! **Maiúsculas**: o `Glob` do Dart usa o contexto de caminho da plataforma
//! e, no Windows, é insensível a maiúsculas (`glob.dart`, `factory Glob`). O
//! oráculo roda no Windows; aqui vale o mesmo por `cfg!(windows)`.

#[derive(Debug, Clone, PartialEq)]
enum No {
    Literal(Vec<char>),
    Estrela,
    DuplaEstrela,
    Qualquer,
    Faixa { faixas: Vec<(char, char)>, negada: bool },
    Opcoes(Vec<Vec<No>>),
}

#[derive(Debug, Clone)]
pub struct Glob {
    pub padrao: String,
    nos: Vec<No>,
    sensivel: bool,
}

impl PartialEq for Glob {
    fn eq(&self, o: &Self) -> bool {
        self.padrao == o.padrao
    }
}

struct Leitor<'a> {
    c: Vec<char>,
    i: usize,
    padrao: &'a str,
}

impl Leitor<'_> {
    fn fim(&self) -> bool {
        self.i >= self.c.len()
    }
    fn olha(&self, ch: char) -> bool {
        self.c.get(self.i) == Some(&ch)
    }
    fn toma(&mut self, ch: char) -> bool {
        if self.olha(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn erro(&self, msg: &str) -> String {
        format!("glob inválido '{}': {msg} (posição {})", self.padrao, self.i)
    }
    fn le(&mut self) -> Result<char, String> {
        let ch = *self.c.get(self.i).ok_or_else(|| self.erro("fim inesperado"))?;
        self.i += 1;
        Ok(ch)
    }

    fn sequencia(&mut self, em_opcoes: bool) -> Result<Vec<No>, String> {
        if self.fim() {
            return Err(self.erro("expected a glob."));
        }
        let mut nos = Vec::new();
        while !self.fim() {
            if em_opcoes && (self.olha(',') || self.olha('}')) {
                break;
            }
            nos.push(self.no(em_opcoes)?);
        }
        Ok(nos)
    }

    fn no(&mut self, em_opcoes: bool) -> Result<No, String> {
        if self.toma('*') {
            return Ok(if self.toma('*') { No::DuplaEstrela } else { No::Estrela });
        }
        if self.toma('?') {
            return Ok(No::Qualquer);
        }
        if self.toma('[') {
            return self.faixa();
        }
        if self.toma('{') {
            return self.opcoes();
        }
        self.literal(em_opcoes)
    }

    fn faixa(&mut self) -> Result<No, String> {
        if self.olha(']') {
            return Err(self.erro("unexpected \"]\"."));
        }
        let negada = self.toma('!') || self.toma('^');
        let mut faixas = Vec::new();
        let ler_char = |l: &mut Self| -> Result<char, String> {
            let ch = l.le()?;
            if negada || ch != '/' {
                Ok(ch)
            } else {
                Err(l.erro("\"/\" may not be used in a range."))
            }
        };
        while !self.toma(']') {
            if self.fim() {
                return Err(self.erro("expected \"]\"."));
            }
            self.toma('\\');
            let ch = ler_char(self)?;
            if self.toma('-') {
                if self.olha(']') {
                    faixas.push((ch, ch));
                    faixas.push(('-', '-'));
                    continue;
                }
                self.toma('\\');
                let fim = ler_char(self)?;
                if fim < ch {
                    return Err(self.erro("Range out of order."));
                }
                faixas.push((ch, fim));
            } else {
                faixas.push((ch, ch));
            }
        }
        Ok(No::Faixa { faixas, negada })
    }

    fn opcoes(&mut self) -> Result<No, String> {
        if self.olha('}') {
            return Err(self.erro("unexpected \"}\"."));
        }
        let mut ops = Vec::new();
        loop {
            ops.push(self.sequencia(true)?);
            if !self.toma(',') {
                break;
            }
        }
        if ops.len() == 1 {
            return Err(self.erro("expected \",\"."));
        }
        if !self.toma('}') {
            return Err(self.erro("expected \"}\"."));
        }
        Ok(No::Opcoes(ops))
    }

    fn literal(&mut self, em_opcoes: bool) -> Result<No, String> {
        let para = |ch: char| {
            matches!(ch, '*' | '{' | '[' | '?' | '\\' | '}' | ']' | '(' | ')') || (em_opcoes && ch == ',')
        };
        let mut t = Vec::new();
        loop {
            while let Some(&ch) = self.c.get(self.i) {
                if para(ch) {
                    break;
                }
                t.push(ch);
                self.i += 1;
            }
            if self.toma('\\') {
                t.push(self.le()?);
                continue;
            }
            break;
        }
        for ch in [']', '(', ')'] {
            if self.olha(ch) {
                return Err(self.erro(&format!("unexpected \"{ch}\"")));
            }
        }
        if !em_opcoes && self.olha('}') {
            return Err(self.erro("unexpected \"}\""));
        }
        Ok(No::Literal(t))
    }
}

fn igual(a: char, b: char, sensivel: bool) -> bool {
    a == b || (!sensivel && a.to_lowercase().eq(b.to_lowercase()))
}

fn na_faixa(c: char, faixas: &[(char, char)], sensivel: bool) -> bool {
    let dentro = |x: char| faixas.iter().any(|&(a, b)| a <= x && x <= b);
    if dentro(c) {
        return true;
    }
    if sensivel {
        return false;
    }
    c.to_lowercase().any(dentro) || c.to_uppercase().any(dentro)
}

impl Glob {
    pub fn novo(padrao: &str) -> Result<Glob, String> {
        Self::com_sensibilidade(padrao, !cfg!(windows))
    }

    pub fn com_sensibilidade(padrao: &str, sensivel: bool) -> Result<Glob, String> {
        let mut l = Leitor { c: padrao.chars().collect(), i: 0, padrao };
        let nos = l.sequencia(false)?;
        Ok(Glob { padrao: padrao.to_string(), nos, sensivel })
    }

    /// O caminho inteiro (POSIX, relativo ao pacote) casa com o padrão.
    pub fn casa(&self, caminho: &str) -> bool {
        let t: Vec<char> = caminho.chars().collect();
        let mut k = |fim: usize| fim == t.len();
        casar(&self.nos, 0, &t, 0, self.sensivel, &mut k)
    }

    /// Algum caminho que começa com `dir/` pode casar? Conservador (pode dizer
    /// sim a mais): serve para podar a varredura de diretórios.
    pub fn pode_descer(&self, dir: &str) -> bool {
        let t: Vec<char> = format!("{dir}/").chars().collect();
        casar_prefixo(&self.nos, 0, &t, 0, self.sensivel)
    }
}

/// Casa `nos[i..]` contra `t[j..]`, chamando `k(pos)` a cada fim possível.
fn casar(
    nos: &[No],
    i: usize,
    t: &[char],
    j: usize,
    s: bool,
    k: &mut dyn FnMut(usize) -> bool,
) -> bool {
    let Some(no) = nos.get(i) else { return k(j) };
    match no {
        No::Literal(l) => {
            if t.len() - j < l.len() || !l.iter().zip(&t[j..]).all(|(a, b)| igual(*a, *b, s)) {
                return false;
            }
            casar(nos, i + 1, t, j + l.len(), s, k)
        }
        No::Qualquer => j < t.len() && t[j] != '/' && casar(nos, i + 1, t, j + 1, s, k),
        No::Faixa { faixas, negada } => {
            j < t.len() && t[j] != '/' && (na_faixa(t[j], faixas, s) != *negada) && casar(nos, i + 1, t, j + 1, s, k)
        }
        No::Estrela => {
            // Guloso, com retrocesso: o maior trecho sem `/` primeiro.
            let mut fim = j;
            while fim < t.len() && t[fim] != '/' {
                fim += 1;
            }
            (j..=fim).rev().any(|e| casar(nos, i + 1, t, e, s, k))
        }
        No::DuplaEstrela => (j..=t.len()).rev().any(|e| casar(nos, i + 1, t, e, s, k)),
        No::Opcoes(ops) => ops.iter().any(|op| {
            let mut cont = |e: usize| casar(nos, i + 1, t, e, s, k);
            casar(op, 0, t, j, s, &mut cont)
        }),
    }
}

/// Como `casar`, mas aceita quando o texto acaba antes do padrão (o resto do
/// padrão ainda poderia casar o que vem depois de `t`).
fn casar_prefixo(nos: &[No], i: usize, t: &[char], j: usize, s: bool) -> bool {
    if j == t.len() {
        return true;
    }
    let Some(no) = nos.get(i) else { return false };
    match no {
        No::Literal(l) => {
            let n = l.len().min(t.len() - j);
            if !l[..n].iter().zip(&t[j..j + n]).all(|(a, b)| igual(*a, *b, s)) {
                return false;
            }
            if n < l.len() {
                return true;
            }
            casar_prefixo(nos, i + 1, t, j + n, s)
        }
        No::Qualquer => t[j] != '/' && casar_prefixo(nos, i + 1, t, j + 1, s),
        No::Faixa { faixas, negada } => {
            t[j] != '/' && (na_faixa(t[j], faixas, s) != *negada) && casar_prefixo(nos, i + 1, t, j + 1, s)
        }
        No::Estrela => {
            let mut fim = j;
            while fim < t.len() && t[fim] != '/' {
                fim += 1;
            }
            fim == t.len() || (j..=fim).any(|e| casar_prefixo(nos, i + 1, t, e, s))
        }
        No::DuplaEstrela => true,
        No::Opcoes(ops) => ops.iter().any(|op| {
            let mut resto: Vec<No> = op.clone();
            resto.extend_from_slice(&nos[i + 1..]);
            casar_prefixo(&resto, 0, t, j, s)
        }),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn g(p: &str) -> Glob {
        Glob::com_sensibilidade(p, true).unwrap()
    }

    // Casos do `glob-2.1.3/test/match_test.dart`, na parte relativa.
    #[test]
    fn literais_e_estrelas() {
        assert!(g("foo").casa("foo"));
        assert!(!g("foo").casa("foo/bar"));
        assert!(g("foo*").casa("foobar"));
        assert!(g("foo*").casa("foo"));
        assert!(!g("foo*").casa("foo/bar"));
        assert!(g("*").casa("foo"));
        assert!(!g("*").casa("foo/bar"));
        assert!(g("**").casa("foo/bar/baz"));
        assert!(g("foo/**").casa("foo/bar/baz"));
        assert!(!g("foo/**").casa("foo"));
        assert!(g("**/*.dart").casa("a/b/c.dart"));
        // `**/` exige a barra: a regex é `^[^]*/[^/]*\.dart$`.
        assert!(!g("**/*.dart").casa("c.dart"));
        assert!(g("lib/**.dart").casa("lib/a/b.dart"));
        assert!(g("lib/**.dart").casa("lib/b.dart"));
        assert!(g("README*").casa("README.md"));
        assert!(!g("README*").casa("doc/README.md"));
        assert!(g("$package$").casa("$package$"));
        assert!(g("lib/$lib$").casa("lib/$lib$"));
    }

    #[test]
    fn qualquer_faixas_opcoes() {
        assert!(g("fo?").casa("foo"));
        assert!(!g("fo?").casa("fo/"));
        assert!(g("[a-c]x").casa("bx"));
        assert!(!g("[a-c]x").casa("dx"));
        assert!(g("[!a-c]x").casa("dx"));
        assert!(!g("[!a-c]x").casa("/x"));
        assert!(g("{foo,bar}.dart").casa("bar.dart"));
        assert!(g("a{,/**}").casa("a"));
        assert!(g("a{,/**}").casa("a/b/c"));
        assert!(g("x[-]").casa("x-"));
        assert!(g("x[a-]").casa("x-"));
        assert!(g(r"a\*b").casa("a*b"));
        assert!(!g(r"a\*b").casa("axb"));
    }

    #[test]
    fn erros_de_sintaxe() {
        assert!(Glob::novo("").is_err());
        assert!(Glob::novo("{a}").is_err());
        assert!(Glob::novo("a]").is_err());
        assert!(Glob::novo("a(").is_err());
        assert!(Glob::novo("[]").is_err());
        assert!(Glob::novo("[b-a]").is_err());
        assert!(Glob::novo("a}").is_err());
        assert!(Glob::novo("[/]").is_err());
    }

    #[test]
    fn maiusculas_no_windows() {
        let i = Glob::com_sensibilidade("lib/**.DART", false).unwrap();
        assert!(i.casa("lib/a.dart"));
        assert!(!g("lib/**.DART").casa("lib/a.dart"));
    }

    #[test]
    fn poda_de_diretorios() {
        assert!(g("lib/**").pode_descer("lib"));
        assert!(g("lib/**").pode_descer("lib/src"));
        assert!(!g("lib/**").pode_descer("web"));
        assert!(!g("README*").pode_descer("doc"));
        assert!(g("**/*.dart").pode_descer("x"));
        assert!(g("{lib,web}/*.dart").pode_descer("web"));
        assert!(!g("{lib,web}/*.dart").pode_descer("web/a"));
        assert!(!g("$package$").pode_descer(".dart_tool"));
    }
}
