//! Varredura de JavaScript emitido por compilador.
//!
//! Não é um parser de JavaScript: é um leitor de **declarações de topo**. O
//! `dart_sdk.js` do DDC e os nossos módulos são saída de compilador, com uma
//! gramática pequena e regular — o que precisamos é saber onde cada declaração
//! de topo começa e termina, e isso um leitor de caracteres que respeite
//! strings, comentários e a profundidade de `{}`/`()`/`[]` resolve sem
//! construir árvore nenhuma.
//!
//! A propriedade que o teste exige é **conservação**: a concatenação das fatias
//! devolvidas tem de ser, byte a byte, o texto de entrada. Uma varredura que
//! perde um byte corromperia o programa em silêncio, que é exatamente o modo
//! de falha que não se quer num minificador.

/// Percorre `src` pulando a string que começa em `i` (que aponta para a aspa).
/// Devolve o índice logo depois da aspa de fechamento.
fn pular_string(b: &[u8], mut i: usize) -> usize {
    let aspa = b[i];
    i += 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            c if c == aspa => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Como [`pular_string`], para comentários. `i` aponta para a primeira `/`.
fn pular_comentario(b: &[u8], mut i: usize) -> Option<usize> {
    if i + 1 >= b.len() || b[i] != b'/' {
        return None;
    }
    match b[i + 1] {
        b'/' => {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            Some(i)
        }
        b'*' => {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            Some((i + 2).min(b.len()))
        }
        _ => None,
    }
}

/// Fatias `[início, fim)` das declarações de topo de `src`.
///
/// Uma declaração termina no primeiro `;` ou `}` que esteja em profundidade
/// zero e seja seguido (só por espaços) de quebra de linha — é a forma que o
/// DDC e o nosso emissor produzem. A quebra de linha fica na fatia, para que a
/// concatenação reconstrua o original.
pub fn declaracoes(src: &str) -> Vec<(usize, usize)> {
    let b = src.as_bytes();
    let mut fatias = Vec::new();
    let (mut i, mut inicio) = (0usize, 0usize);
    let (mut chaves, mut parens, mut colchetes) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = pular_string(b, i);
            continue;
        }
        if c == b'/' {
            if let Some(j) = pular_comentario(b, i) {
                i = j;
                continue;
            }
        }
        match c {
            b'{' => chaves += 1,
            b'}' => chaves -= 1,
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => colchetes += 1,
            b']' => colchetes -= 1,
            _ => {}
        }
        i += 1;
        if chaves == 0 && parens == 0 && colchetes == 0 && (c == b';' || c == b'}') {
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\r') {
                j += 1;
            }
            if j < b.len() && b[j] == b'\n' {
                fatias.push((inicio, j + 1));
                inicio = j + 1;
                i = j + 1;
            }
        }
    }
    if inicio < b.len() {
        fatias.push((inicio, b.len()));
    }
    fatias
}

/// Fatias das entradas de topo de um corpo de objeto literal (o texto **entre**
/// as chaves), separadas por vírgula em profundidade zero. A vírgula fica na
/// fatia, pelo mesmo motivo de [`declaracoes`]: conservação.
pub fn entradas_de_objeto(corpo: &str) -> Vec<(usize, usize)> {
    let b = corpo.as_bytes();
    let mut fatias = Vec::new();
    let (mut i, mut inicio) = (0usize, 0usize);
    let (mut chaves, mut parens, mut colchetes) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = pular_string(b, i);
            continue;
        }
        if c == b'/' {
            if let Some(j) = pular_comentario(b, i) {
                i = j;
                continue;
            }
        }
        match c {
            b'{' => chaves += 1,
            b'}' => chaves -= 1,
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => colchetes += 1,
            b']' => colchetes -= 1,
            b',' if chaves == 0 && parens == 0 && colchetes == 0 => {
                fatias.push((inicio, i + 1));
                inicio = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if inicio < b.len() {
        fatias.push((inicio, b.len()));
    }
    fatias
}

/// Fatias dos membros de um corpo de classe (o texto **entre** as chaves).
///
/// Um membro do contrato do DDC é sempre `cabeçalho { corpo }` — método,
/// getter, setter, estático ou gerador. Termina, portanto, na chave que fecha
/// o seu corpo em profundidade zero. Um `;` solto em profundidade zero também
/// fecha (campos de classe, que o DDC não emite mas o ES permite).
pub fn entradas_de_classe(corpo: &str) -> Vec<(usize, usize)> {
    let b = corpo.as_bytes();
    let mut fatias = Vec::new();
    let (mut i, mut inicio) = (0usize, 0usize);
    let (mut chaves, mut parens, mut colchetes) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = pular_string(b, i);
            continue;
        }
        if c == b'/' {
            if let Some(j) = pular_comentario(b, i) {
                i = j;
                continue;
            }
        }
        match c {
            b'{' => chaves += 1,
            b'}' => {
                chaves -= 1;
                if chaves == 0 && parens == 0 && colchetes == 0 {
                    fatias.push((inicio, i + 1));
                    inicio = i + 1;
                }
            }
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => colchetes += 1,
            b']' => colchetes -= 1,
            b';' if chaves == 0 && parens == 0 && colchetes == 0 => {
                fatias.push((inicio, i + 1));
                inicio = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if inicio < b.len() {
        fatias.push((inicio, b.len()));
    }
    fatias
}

/// Índice da chave que fecha a que começa em `abre` (que aponta para `{`).
pub fn fecha_chave(src: &str, abre: usize) -> Option<usize> {
    fecha(src, abre, b'{', b'}')
}

/// Índice do colchete que fecha o que começa em `abre` (que aponta para `[`).
pub fn fecha_colchete(src: &str, abre: usize) -> Option<usize> {
    fecha(src, abre, b'[', b']')
}

fn fecha(src: &str, abre: usize, ab: u8, fe: u8) -> Option<usize> {
    let b = src.as_bytes();
    let mut i = abre;
    let mut d = 0i32;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = pular_string(b, i);
            continue;
        }
        if c == b'/' {
            if let Some(j) = pular_comentario(b, i) {
                i = j;
                continue;
            }
        }
        if c == ab {
            d += 1;
        } else if c == fe {
            d -= 1;
            if d == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod testes {
    use super::*;

    fn junta(src: &str) -> String {
        declaracoes(src).into_iter().map(|(a, b)| &src[a..b]).collect()
    }

    #[test]
    fn conserva_bytes() {
        let src = "var a = 1;\nfn.x = function () {\n  return \";\\n}\";\n};\n// fim }\nb.c = 2;\n";
        assert_eq!(junta(src), src);
        assert_eq!(declaracoes(src).len(), 3, "três declarações de topo");
    }

    #[test]
    fn chave_de_fechamento_ignora_string() {
        let src = "x = { a: \"}\" };";
        let abre = src.find('{').unwrap();
        assert_eq!(&src[fecha_chave(src, abre).unwrap()..], "};");
    }

    #[test]
    fn membros_de_classe_conservam_bytes() {
        let corpo = "\n  f() {\n    return \"}\";\n  }\n  get x() { return 1; }\n";
        let v = entradas_de_classe(corpo);
        // dois membros mais a quebra de linha final, que fica numa fatia própria
        assert_eq!(v.len(), 3);
        assert_eq!(v.iter().map(|&(a, b)| &corpo[a..b]).collect::<String>(), corpo);
    }

    #[test]
    fn entradas_de_objeto_no_nivel_zero() {
        let corpo = " a: 1, b: {c: 2, d: 3}, e: f(1, 2) ";
        let v = entradas_de_objeto(corpo);
        assert_eq!(v.len(), 3);
        assert_eq!(v.iter().map(|&(a, b)| &corpo[a..b]).collect::<String>(), corpo);
    }
}
