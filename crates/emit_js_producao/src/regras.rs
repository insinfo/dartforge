//! Regras rti só para os tipos testados (`docs/JS-PRODUCAO-TAMANHO.md` §8.1).
//!
//! `_Universe.addRules` registra, para cada classe `C`, os supertipos dela
//! (`"lib|T": [args]`) e as substituições das variáveis de tipo herdadas
//! (`"T.E": "receita"`). A checagem de subtipo `S <: T` só consulta a entrada
//! `T` da regra de `S` (`rti.dart` `_isSubtype`, o `lookupSupertype` sobre
//! `_findRule(sName)`), e `T` é sempre um tipo que o programa escreveu numa
//! receita: o alvo de um `is`/`as`, de um `catch`, de um argumento de tipo,
//! de uma substituição. O `dart2js` faz o mesmo corte
//! (`RuntimeTypesChecks.requiredChecks`, `js_backend/runtime_types.dart:26-45`).
//!
//! A conta: os nomes `lib|Classe` citados em qualquer parte do arquivo, tirando
//! as chaves das próprias regras (as classes descritas e os supertipos
//! listados), mais os citados nos **valores** das regras (argumentos de
//! supertipo e substituições, que viram alvo quando avaliados). A entrada de
//! supertipo cujo nome não está nesse conjunto nunca é consultada e sai. As
//! substituições (`"T.E"`) e as regras de encaminhamento ficam todas.

use std::collections::HashSet;

const CHAMADAS: &[&str] = &["_Universe.addRules(dart.typeUniverse, JSON.parse(", "_Universe.addOrUpdateRules(dart.typeUniverse, JSON.parse("];

/// Lê a *string* JS com aspas duplas que começa em `ini` (no `"`): o fim
/// (depois da aspa) e o conteúdo sem escapes de JS.
fn string_js(t: &str, ini: usize) -> Option<(usize, String)> {
    let b = t.as_bytes();
    if b.get(ini) != Some(&b'"') {
        return None;
    }
    let mut i = ini + 1;
    let mut out = String::new();
    while i < b.len() {
        match b[i] {
            b'"' => return Some((i + 1, out)),
            b'\\' => {
                let c = *b.get(i + 1)?;
                match c {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let h = t.get(i + 2..i + 6)?;
                        out.push(char::from_u32(u32::from_str_radix(h, 16).ok()?)?);
                        i += 4;
                    }
                    _ => out.push(c as char),
                }
                i += 2;
            }
            _ => {
                let ch = t[i..].chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

/// Os nomes `lib|Classe` de um texto.
fn nomes_de_classe(t: &str, out: &mut HashSet<String>) {
    let b = t.as_bytes();
    let id = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    for (p, _) in t.match_indices('|') {
        let mut i = p;
        while i > 0 && id(b[i - 1]) {
            i -= 1;
        }
        let mut f = p + 1;
        while f < b.len() && id(b[f]) {
            f += 1;
        }
        if i < p && f > p + 1 {
            out.insert(t[i..f].to_string());
        }
    }
}

/// O `addRtiResources(C, ["a|C", "b|I", …])` põe no protótipo de `C` a marca
/// `$is_<receita>` de cada interface da lista, que o teste rápido de `is`
/// (`_isTestViaProperty`) lê. Como as regras, só vale para um alvo citado;
/// o primeiro item (a receita da própria classe) fica sempre.
const RECURSOS: &str = "addRtiResources(";

/// Poda as entradas de supertipo nunca consultadas das regras rti do
/// arquivo e as marcas de interface correspondentes. Devolve o texto
/// inalterado se algo não tiver a forma esperada.
pub fn podar(js: &str) -> String {
    // Os trechos que não contam como citação: os JSON das regras e as listas
    // do `addRtiResources` (início e fim de cada lista `[...]`).
    let mut listas: Vec<(usize, usize)> = Vec::new();
    for (p, _) in js.match_indices(RECURSOS) {
        let resto = &js[p..];
        let Some(a) = resto.find(", [") else { continue };
        let Some(f) = resto[a..].find("]);") else { continue };
        if resto[..a + f].contains('\n') {
            continue;
        }
        listas.push((p + a + 2, p + a + f + 1));
    }
    // As chamadas e os seus JSON.
    let mut achados: Vec<(usize, usize, serde_json::Value)> = Vec::new();
    for c in CHAMADAS {
        for (p, _) in js.match_indices(c) {
            let ini = p + c.len();
            let Some((fim, conteudo)) = string_js(js, ini) else { return js.to_string() };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&conteudo) else { return js.to_string() };
            achados.push((ini, fim, v));
        }
    }
    if achados.is_empty() {
        return js.to_string();
    }
    achados.sort_by_key(|a| a.0);
    // Os citados fora das regras e das listas, e nos valores das regras.
    let mut citados: HashSet<String> = HashSet::new();
    let mut fora: Vec<(usize, usize)> = achados.iter().map(|a| (a.0, a.1)).chain(listas.iter().copied()).collect();
    fora.sort();
    let mut pos = 0;
    for (ini, fim) in &fora {
        if *ini >= pos {
            nomes_de_classe(&js[pos..*ini], &mut citados);
            pos = *fim;
        }
    }
    nomes_de_classe(&js[pos..], &mut citados);
    for (_, _, v) in &achados {
        if let Some(o) = v.as_object() {
            for regra in o.values() {
                let Some(r) = regra.as_object() else { continue };
                for x in r.values() {
                    match x {
                        serde_json::Value::String(s) => nomes_de_classe(s, &mut citados),
                        serde_json::Value::Array(a) => {
                            for s in a.iter().filter_map(|s| s.as_str()) {
                                nomes_de_classe(s, &mut citados);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    // Reescreve cada JSON sem as entradas de supertipo não citadas e cada
    // lista de marcas sem as interfaces não citadas.
    let mut trocas: Vec<(usize, usize, String)> = Vec::new();
    for (ini, fim, mut v) in achados {
        if let Some(o) = v.as_object_mut() {
            for regra in o.values_mut() {
                let Some(r) = regra.as_object_mut() else { continue };
                r.retain(|k, _| !k.contains('|') || citados.contains(k));
            }
            // Regra vazia: sem ela o `_isSubtype` dá o mesmo `false` (regra
            // ausente), e nenhuma variável de tipo é avaliada pela classe.
            o.retain(|_, regra| regra.as_object().is_none_or(|r| !r.is_empty()));
        }
        let json = serde_json::to_string(&v).unwrap_or_default();
        trocas.push((ini, fim, dartforge_emit_js::js::string_literal(&json)));
    }
    for (ini, fim) in listas {
        let itens: Vec<&str> = js[ini + 1..fim - 1].split(", ").collect();
        let mantidos: Vec<&str> = itens
            .iter()
            .enumerate()
            .filter(|(i, x)| *i == 0 || x.strip_prefix('"').and_then(|y| y.strip_suffix('"')).is_none_or(|y| citados.contains(y)))
            .map(|(_, x)| *x)
            .collect();
        trocas.push((ini, fim, format!("[{}]", mantidos.join(", "))));
    }
    trocas.sort_by_key(|t| t.0);
    let mut out = String::with_capacity(js.len());
    let mut pos = 0;
    for (ini, fim, novo) in trocas {
        if ini < pos {
            continue;
        }
        out.push_str(&js[pos..ini]);
        out.push_str(&novo);
        pos = fim;
    }
    out.push_str(&js[pos..]);
    out
}
