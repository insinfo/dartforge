//! Casamento aproximado de nomes (completar e `workspace/symbol`), no
//! espírito do `FuzzyMatcher` do servidor do Dart: o prefixo vale mais que
//! uma subsequência, e as letras que caem em início de palavra (`gT` em
//! `getType`, `g_t` em `get_type`) valem mais que as soltas.

/// Qualidade do casamento de `consulta` em `nome`; menor é melhor e `None`
/// é "não casa". Maiúsculas e minúsculas não distinguem o casamento, só
/// desempatam (a grafia exata ganha).
///
/// * 0 — prefixo com a mesma grafia;
/// * 1 — prefixo;
/// * 2 — contém;
/// * 3 — subsequência em inícios de palavra (`cD` em `carregarDados`);
/// * 4 — subsequência qualquer, com a primeira letra no começo do nome.
pub(crate) fn pontuar(consulta: &str, nome: &str) -> Option<u8> {
    if consulta.is_empty() {
        return Some(1);
    }
    if nome.starts_with(consulta) {
        return Some(0);
    }
    let c = consulta.to_lowercase();
    let n = nome.to_lowercase();
    if n.starts_with(&c) {
        return Some(1);
    }
    if n.contains(&c) {
        return Some(2);
    }
    let letras: Vec<char> = nome.chars().collect();
    let inicios: Vec<usize> = (0..letras.len())
        .filter(|&i| {
            i == 0
                || letras[i].is_uppercase() && !letras[i - 1].is_uppercase()
                || letras[i - 1] == '_' && letras[i] != '_'
                || letras[i].is_ascii_digit() && !letras[i - 1].is_ascii_digit()
        })
        .collect();
    if subsequencia(&c, &letras, &inicios) {
        return Some(3);
    }
    let primeira = c.chars().next()?;
    if !n.starts_with(primeira) {
        return None;
    }
    let mut resto = n.chars();
    c.chars()
        .all(|x| resto.by_ref().any(|y| y == x))
        .then_some(4)
}

/// `consulta` casa com as letras de `nome` que abrem palavras (`inicios`),
/// podendo cada trecho continuar pelas letras seguintes da mesma palavra.
/// Busca com retrocesso: `gety` casa `getType` (`get` + `Ty`).
fn subsequencia(consulta: &str, letras: &[char], inicios: &[usize]) -> bool {
    let q: Vec<char> = consulta.chars().collect();
    let igual = |a: char, b: char| a.to_lowercase().eq(b.to_lowercase());
    fn casar(
        q: &[char],
        letras: &[char],
        inicios: &[usize],
        qi: usize,
        pos: usize,
        igual: &dyn Fn(char, char) -> bool,
    ) -> bool {
        if qi == q.len() {
            return true;
        }
        // Continua a palavra corrente.
        if pos > 0
            && pos < letras.len()
            && !inicios.contains(&pos)
            && igual(letras[pos], q[qi])
            && casar(q, letras, inicios, qi + 1, pos + 1, igual)
        {
            return true;
        }
        // Ou abre uma palavra adiante.
        inicios
            .iter()
            .filter(|&&s| s >= pos && igual(letras[s], q[qi]))
            .any(|&s| casar(q, letras, inicios, qi + 1, s + 1, igual))
    }
    casar(&q, letras, inicios, 0, 0, &igual)
}

#[cfg(test)]
mod testes {
    use super::pontuar;

    #[test]
    fn ordem_de_qualidade() {
        assert_eq!(pontuar("car", "carregar"), Some(0));
        assert_eq!(pontuar("CAR", "carregar"), Some(1));
        assert_eq!(pontuar("dados", "carregarDados"), Some(2));
        assert_eq!(pontuar("cD", "carregarDados"), Some(3));
        assert_eq!(pontuar("gety", "getType"), Some(3));
        assert_eq!(pontuar("crg", "carregar"), Some(4));
        assert_eq!(pontuar("xyz", "carregar"), None);
        // A primeira letra precisa abrir o nome ou uma palavra.
        assert_eq!(pontuar("ar", "carregar"), Some(2));
        assert_eq!(pontuar("rg", "carregar"), None);
    }
}
