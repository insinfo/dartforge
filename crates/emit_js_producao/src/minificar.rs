//! Compactação do arquivo de produção (etapa 7 do plano, parte segura).
//!
//! Tira comentários e o espaço que não separa tokens, sem tocar em nome
//! nenhum: nada que um programa observe muda. A renomeação de locais e
//! propriedades (`docs/JS-PRODUCAO.md` §3) fica de fora — ver o fim do §3 lá.
//!
//! As regras, cada uma pela razão que a torna segura:
//!
//! * **quebra de linha fica**: uma sequência de espaço com quebra (ou um
//!   comentário de bloco com quebra) vira uma quebra só. A inserção
//!   automática de ponto e vírgula (`return`↵`x`, `a`↵`++b`) depende da
//!   quebra e não do resto do espaço, então o sentido é o mesmo;
//! * **espaço entre tokens só onde separa**: entre dois caracteres de
//!   palavra (`var x`, `return 1`), entre `+ +` e `- -` (`a + +b` não é
//!   `a++b`), entre `/` e `/` ou `*` (não abre comentário), entre `<` e `!` e
//!   entre `-` e `>` (`<!--` e `-->` são comentários HTML em script), e entre
//!   número e `.` (`1 .x` não é `1.x`);
//! * **literais intactos**: cadeias, *template literals* (com as expressões
//!   `${…}` dentro) e expressões regulares são copiados byte a byte. A barra
//!   é regex no começo de expressão (depois de pontuação que não fecha
//!   expressão e das palavras-chave que esperam uma, `return`/`typeof`/…) e
//!   divisão depois de palavra, número, literal, `)`, `]` e `++`/`--`; uma
//!   "regex" que chega ao fim da linha sem fechar volta a ser divisão.
//!
//! A saída é determinística (uma passada, sem tabela) e tem os mesmos tokens
//! na mesma ordem que a entrada — é o que o teste confere, além do corpus
//! inteiro executado com o arquivo compactado.

/// Espécie do último token significativo, para decidir regex × divisão e
/// o espaço entre tokens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ultimo {
    Nada,
    /// Palavra: identificador, palavra-chave ou número. `regex_depois` diz
    /// se é uma palavra-chave depois da qual vem expressão; `numero`, se é
    /// um literal numérico.
    Palavra { regex_depois: bool, numero: bool },
    /// Cadeia, template ou regex.
    Literal,
    /// Pontuação: o caractere, e se ele forma `++`/`--` com o anterior.
    Pontuacao { c: u8, dobrado: bool },
}

/// Espaço pendente entre o último token e o próximo.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Espaco {
    Nenhum,
    Simples,
    Quebra,
}

fn e_palavra(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80 || c == b'\\'
}

const PALAVRAS_ANTES_DE_EXPRESSAO: &[&[u8]] = &[
    b"return", b"typeof", b"instanceof", b"in", b"of", b"new", b"delete", b"void", b"throw", b"case", b"do", b"else", b"yield", b"await",
];

/// Compacta `src` (JS) conforme as regras do módulo.
pub fn compactar(src: &str) -> String {
    let b = src.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut ultimo = Ultimo::Nada;
    let mut pendente = Espaco::Nenhum;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        // Espaço e quebras.
        if c == b' ' || c == b'\t' || c == b'\n' || c == b'\r' || c == 0x0b || c == 0x0c {
            let q = if c == b'\n' || c == b'\r' { Espaco::Quebra } else { Espaco::Simples };
            pendente = pendente.max(q);
            i += 1;
            continue;
        }
        // Comentários.
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' && b[i] != b'\r' {
                i += 1;
            }
            pendente = pendente.max(Espaco::Simples);
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let fim = src[i + 2..].find("*/").map(|k| i + 2 + k + 2).unwrap_or(b.len());
            let com_quebra = b[i..fim].iter().any(|&x| x == b'\n' || x == b'\r');
            pendente = pendente.max(if com_quebra { Espaco::Quebra } else { Espaco::Simples });
            i = fim;
            continue;
        }
        // O próximo token: [i, fim) e a espécie dele.
        let (fim, especie) = if c == b'"' || c == b'\'' {
            (fim_de_cadeia(b, i), Ultimo::Literal)
        } else if c == b'`' {
            (fim_de_template(b, i), Ultimo::Literal)
        } else if c == b'/' && regex_permitida(ultimo) {
            match fim_de_regex(b, i) {
                Some(f) => (f, Ultimo::Literal),
                None => (i + 1, Ultimo::Pontuacao { c, dobrado: false }),
            }
        } else if e_palavra(c) {
            let numero = c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit));
            let f = if numero { fim_de_numero(b, i) } else { fim_de_palavra(b, i) };
            let regex_depois = !numero && PALAVRAS_ANTES_DE_EXPRESSAO.contains(&&b[i..f]);
            (f, Ultimo::Palavra { regex_depois, numero })
        } else if c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            (fim_de_numero(b, i), Ultimo::Palavra { regex_depois: false, numero: true })
        } else {
            let dobrado = (c == b'+' || c == b'-') && matches!(ultimo, Ultimo::Pontuacao { c: d, dobrado: false } if d == c) && pendente == Espaco::Nenhum;
            (i + 1, Ultimo::Pontuacao { c, dobrado })
        };
        match pendente {
            Espaco::Quebra if !out.is_empty() => out.push(b'\n'),
            Espaco::Simples if precisa_de_espaco(ultimo, out.last().copied(), b[i]) => out.push(b' '),
            _ => {}
        }
        out.extend_from_slice(&b[i..fim]);
        pendente = Espaco::Nenhum;
        ultimo = especie;
        i = fim;
    }
    if !out.is_empty() && src.ends_with('\n') {
        out.push(b'\n');
    }
    // Só bytes da entrada, cortados em fronteiras de token ASCII: UTF-8 válido.
    String::from_utf8(out).expect("a compactação corta só em fronteiras ASCII")
}

fn regex_permitida(u: Ultimo) -> bool {
    match u {
        Ultimo::Nada => true,
        Ultimo::Palavra { regex_depois, .. } => regex_depois,
        Ultimo::Literal => false,
        Ultimo::Pontuacao { c, dobrado } => !dobrado && !matches!(c, b')' | b']'),
    }
}

/// Se o espaço entre o token anterior (que termina em `a`) e o próximo (que
/// começa em `b`) precisa ficar.
fn precisa_de_espaco(ultimo: Ultimo, a: Option<u8>, b: u8) -> bool {
    let Some(a) = a else { return false };
    if e_palavra(a) && e_palavra(b) {
        return true;
    }
    if matches!(ultimo, Ultimo::Palavra { numero: true, .. }) && b == b'.' {
        return true;
    }
    matches!((a, b), (b'+', b'+') | (b'-', b'-') | (b'/', b'/') | (b'/', b'*') | (b'<', b'!') | (b'-', b'>'))
}

fn fim_de_palavra(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && e_palavra(b[i]) {
        // `\uXXXX` em identificador: o escape inteiro é da palavra.
        if b[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    i.min(b.len())
}

fn fim_de_numero(b: &[u8], mut i: usize) -> usize {
    let hex = b.get(i) == Some(&b'0') && matches!(b.get(i + 1), Some(b'x' | b'X'));
    while i < b.len() {
        let c = b[i];
        // O sinal só é do número logo depois do expoente (`1e-5`).
        let sinal_de_expoente = (c == b'+' || c == b'-') && !hex && i > 0 && matches!(b[i - 1], b'e' | b'E');
        if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || sinal_de_expoente {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn fim_de_cadeia(b: &[u8], i: usize) -> usize {
    let aspa = b[i];
    let mut j = i + 1;
    while j < b.len() && b[j] != aspa {
        if b[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    (j + 1).min(b.len())
}

/// Fim de um *template literal* que começa em `i`, com as expressões
/// `${…}` (que podem ter cadeias, templates, comentários e chaves).
fn fim_de_template(b: &[u8], i: usize) -> usize {
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'`' => return j + 1,
            b'$' if b.get(j + 1) == Some(&b'{') => j = fim_de_expressao_de_template(b, j + 2),
            _ => j += 1,
        }
    }
    b.len()
}

/// Fim (depois da `}`) da expressão de template que começa em `i`.
fn fim_de_expressao_de_template(b: &[u8], mut i: usize) -> usize {
    let mut prof = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => i = fim_de_cadeia(b, i),
            b'`' => i = fim_de_template(b, i),
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
                prof += 1;
                i += 1;
            }
            b'}' => {
                if prof == 0 {
                    return i + 1;
                }
                prof -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    b.len()
}

/// Fim de uma regex que começa em `i` (com as flags), ou `None` se não
/// fecha antes do fim da linha (então a barra era divisão).
fn fim_de_regex(b: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 1;
    let mut classe = false;
    // `//` já foi lido como comentário; `/` seguido de `*` também.
    loop {
        let c = *b.get(j)?;
        match c {
            b'\n' | b'\r' => return None,
            b'\\' => j += 1,
            b'[' => classe = true,
            b']' => classe = false,
            b'/' if !classe => break,
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

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tira_comentarios_e_indentacao() {
        let js = "// cabeçalho\nfunction f(a, b) {\n  /* soma */\n  return a + b;\n}\n";
        assert_eq!(compactar(js), "function f(a,b){\nreturn a+b;\n}\n");
    }

    #[test]
    fn preserva_quebras_que_decidem_ponto_e_virgula() {
        // `return`↵`x` devolve `undefined`; `a`↵`++b` incrementa `b`.
        assert_eq!(compactar("return\n  x"), "return\nx");
        assert_eq!(compactar("a\n++b"), "a\n++b");
        // Comentário de bloco com quebra é quebra.
        assert_eq!(compactar("return /*\n*/ x"), "return\nx");
    }

    #[test]
    fn espaco_que_separa_tokens_fica() {
        assert_eq!(compactar("a + +b"), "a+ +b");
        assert_eq!(compactar("a - -b"), "a- -b");
        assert_eq!(compactar("a++ + b"), "a++ +b");
        assert_eq!(compactar("var x = typeof y"), "var x=typeof y");
        assert_eq!(compactar("1 .toString()"), "1 .toString()");
        assert_eq!(compactar("x < !y"), "x< !y");
        assert_eq!(compactar("x-- > y"), "x-- >y");
        assert_eq!(compactar("a / /re/.source.length"), "a/ /re/.source.length");
    }

    #[test]
    fn literais_sao_copiados_como_estao() {
        assert_eq!(compactar("s = 'a  // b'  ;"), "s='a  // b';");
        assert_eq!(compactar("s = \"x \\\" /* y */\""), "s=\"x \\\" /* y */\"");
        assert_eq!(compactar("t = `a  ${ f( '}' ) }  b`"), "t=`a  ${ f( '}' ) }  b`");
        assert_eq!(compactar("r = /a  b[/ ]c/g.test(x)"), "r=/a  b[/ ]c/g.test(x)");
        assert_eq!(compactar("return /  x/.exec(s)"), "return/  x/.exec(s)");
    }

    #[test]
    fn barra_depois_de_expressao_e_divisao() {
        assert_eq!(compactar("x = a / b / c"), "x=a/b/c");
        assert_eq!(compactar("x = (a) / 2 / 3"), "x=(a)/2/3");
        assert_eq!(compactar("x = a++ / 2 / 3"), "x=a++/2/3");
        assert_eq!(compactar("x = 1e-5 / 2"), "x=1e-5/2");
    }

    #[test]
    fn e_deterministica_e_idempotente() {
        let js = "var a = {b: 1, // c\n  d: [1, 2]};\nfunction g() { return a.b  +  a.d[0]; }\n";
        let um = compactar(js);
        assert_eq!(um, compactar(js));
        assert_eq!(compactar(&um), um);
    }
}
