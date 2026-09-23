//! `buildExtensions` de um builder: que saídas uma entrada produz.
//!
//! Reproduz `build-2.4.2/lib/src/generate/expected_outputs.dart`:
//! * sufixo: `.dart → [.g.dart]` troca o fim do caminho;
//! * `^caminho`: casamento exato do caminho inteiro, e as saídas são caminhos;
//! * `{{nome}}`: cada grupo vira `(.+)` numa regex ancorada no fim (e no
//!   começo, com `^`), com o **primeiro** casamento e grupos gulosos; a saída
//!   troca o trecho casado pela saída com os grupos substituídos.
//!
//! Validações (as mesmas mensagens em essência): saída com grupo sem grupo na
//! entrada, grupo repetido, saída que não usa todos os grupos exatamente uma
//! vez, e saída igual à entrada.

#[derive(Debug, Clone, PartialEq)]
enum Regra {
    Sufixo { entrada: String, saidas: Vec<String> },
    Exata { caminho: String, saidas: Vec<String> },
    Captura { ancorada: bool, literais: Vec<String>, nomes: Vec<String>, saidas: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Extensoes {
    regras: Vec<Regra>,
    /// A forma declarada, na ordem: entra na identidade da fase.
    pub declaradas: Vec<(String, Vec<String>)>,
}

/// Grupos `{{nome}}` de um texto: `(início, fim, nome)`.
fn grupos(s: &str) -> Vec<(usize, usize, String)> {
    let mut v = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'{' && b[i + 1] == b'{' {
            let mut j = i + 2;
            while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j + 1 < b.len() && b[j] == b'}' && b[j + 1] == b'}' {
                v.push((i, j + 2, s[i + 2..j].to_string()));
                i = j + 2;
                continue;
            }
        }
        i += 1;
    }
    v
}

impl Extensoes {
    pub fn novas(declaradas: &[(String, Vec<String>)], builder: &str) -> Result<Extensoes, String> {
        let mut regras = Vec::new();
        for (entrada, saidas) in declaradas {
            let gs = grupos(entrada);
            if !gs.is_empty() {
                let ancorada = entrada.starts_with('^');
                let mut pos = usize::from(ancorada);
                let mut literais = Vec::new();
                let mut nomes: Vec<String> = Vec::new();
                for (ini, fim, nome) in &gs {
                    if nomes.contains(nome) {
                        return Err(format!(
                            "o builder `{builder}` declara a entrada \"{entrada}\" com grupos repetidos (`{{{{{nome}}}}}`)"
                        ));
                    }
                    nomes.push(nome.clone());
                    literais.push(entrada[pos..*ini].to_string());
                    pos = *fim;
                }
                literais.push(entrada[pos..].to_string());
                for s in saidas {
                    let mut restantes = nomes.clone();
                    for (_, _, n) in grupos(s) {
                        let Some(i) = restantes.iter().position(|x| *x == n) else {
                            return Err(format!(
                                "o builder `{builder}` declara a saída \"{s}\" com o grupo \"{n}\", que não existe ou aparece mais de uma vez"
                            ));
                        };
                        restantes.remove(i);
                    }
                    if !restantes.is_empty() {
                        return Err(format!(
                            "o builder `{builder}` declara a entrada \"{entrada}\" com grupo, e a saída \"{s}\" não usa {}",
                            restantes.join(", ")
                        ));
                    }
                }
                regras.push(Regra::Captura { ancorada, literais, nomes, saidas: saidas.clone() });
                continue;
            }
            for s in saidas {
                if !grupos(s).is_empty() {
                    return Err(format!(
                        "o builder `{builder}` declara a saída \"{s}\" com grupo, e a entrada \"{entrada}\" não tem grupo"
                    ));
                }
            }
            if let Some(c) = entrada.strip_prefix('^') {
                regras.push(Regra::Exata { caminho: c.to_string(), saidas: saidas.clone() });
            } else {
                regras.push(Regra::Sufixo { entrada: entrada.clone(), saidas: saidas.clone() });
            }
        }
        Ok(Extensoes { regras, declaradas: declaradas.to_vec() })
    }

    pub fn tem_saida(&self, caminho: &str) -> bool {
        self.regras.iter().any(|r| match r {
            Regra::Sufixo { entrada, .. } => caminho.ends_with(entrada.as_str()),
            Regra::Exata { caminho: c, .. } => c == caminho,
            Regra::Captura { ancorada, literais, .. } => primeiro_casamento(caminho, *ancorada, literais).is_some(),
        })
    }

    /// `expectedOutputs(builder, input)`: caminhos (no mesmo pacote), na
    /// ordem das regras e das saídas. Saída igual à entrada é erro.
    pub fn saidas(&self, caminho: &str) -> Result<Vec<String>, String> {
        let mut v = Vec::new();
        for r in &self.regras {
            match r {
                Regra::Sufixo { entrada, saidas } => {
                    if let Some(base) = caminho.strip_suffix(entrada.as_str()) {
                        v.extend(saidas.iter().map(|s| format!("{base}{s}")));
                    }
                }
                Regra::Exata { caminho: c, saidas } => {
                    if c == caminho {
                        v.extend(saidas.iter().cloned());
                    }
                }
                Regra::Captura { ancorada, literais, nomes, saidas } => {
                    if let Some((ini, caps)) = primeiro_casamento(caminho, *ancorada, literais) {
                        for s in saidas {
                            let mut r = String::new();
                            let mut pos = 0;
                            for (a, b, n) in grupos(s) {
                                r.push_str(&s[pos..a]);
                                let i = nomes.iter().position(|x| *x == n).unwrap_or(0);
                                r.push_str(&caps[i]);
                                pos = b;
                            }
                            r.push_str(&s[pos..]);
                            v.push(format!("{}{r}", &caminho[..ini]));
                        }
                    }
                }
            }
        }
        if v.iter().any(|s| s == caminho) {
            return Err(format!("a saída \"{caminho}\" é igual à entrada, o que não é permitido"));
        }
        Ok(v)
    }

    /// Todas as saídas declaradas (para `required_inputs`).
    pub fn todas_as_saidas(&self) -> impl Iterator<Item = &String> {
        self.declaradas.iter().flat_map(|(_, s)| s)
    }
}

/// Primeiro casamento de `lit0(.+)lit1(.+)…litN$` (com `^` se ancorada):
/// o início mais à esquerda, grupos gulosos. Devolve o início e os grupos.
fn primeiro_casamento(t: &str, ancorada: bool, literais: &[String]) -> Option<(usize, Vec<String>)> {
    let fronteiras: Vec<usize> = t.char_indices().map(|(i, _)| i).chain(std::iter::once(t.len())).collect();
    let inicios: Vec<usize> = if ancorada { vec![0] } else { fronteiras.clone() };
    for ini in inicios {
        let mut caps = Vec::new();
        if casar(t, &fronteiras, ini, literais, 0, &mut caps) {
            return Some((ini, caps));
        }
    }
    None
}

fn casar(t: &str, fr: &[usize], pos: usize, lits: &[String], k: usize, caps: &mut Vec<String>) -> bool {
    let lit = &lits[k];
    if !t[pos..].starts_with(lit.as_str()) {
        return false;
    }
    let pos = pos + lit.len();
    if k + 1 == lits.len() {
        return pos == t.len();
    }
    // `(.+)` guloso: o maior trecho primeiro, pelo menos um caractere.
    for &fim in fr.iter().rev() {
        if fim <= pos {
            break;
        }
        caps.push(t[pos..fim].to_string());
        if casar(t, fr, fim, lits, k + 1, caps) {
            return true;
        }
        caps.pop();
    }
    false
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ext(pares: &[(&str, &[&str])]) -> Result<Extensoes, String> {
        let v: Vec<(String, Vec<String>)> =
            pares.iter().map(|(a, b)| (a.to_string(), b.iter().map(|s| s.to_string()).collect())).collect();
        Extensoes::novas(&v, "teste")
    }

    // Casos do `build-2.4.2/test/generate/expected_outputs_test.dart`.
    #[test]
    fn sufixo() {
        let e = ext(&[(".dart", &[".g.dart", ".json"])]).unwrap();
        assert_eq!(e.saidas("lib/a.dart").unwrap(), vec!["lib/a.g.dart", "lib/a.json"]);
        assert!(e.saidas("lib/a.txt").unwrap().is_empty());
        assert!(e.tem_saida("lib/a.dart"));
    }

    #[test]
    fn exata() {
        let e = ext(&[("^pubspec.yaml", &["lib/versao.dart"])]).unwrap();
        assert_eq!(e.saidas("pubspec.yaml").unwrap(), vec!["lib/versao.dart"]);
        assert!(e.saidas("x/pubspec.yaml").unwrap().is_empty());
    }

    #[test]
    fn captura() {
        let e = ext(&[("{{}}.dart", &["{{}}.g.dart"])]).unwrap();
        assert_eq!(e.saidas("lib/a.dart").unwrap(), vec!["lib/a.g.dart"]);
        let e = ext(&[("^lib/{{}}.dart", &["lib/generated/{{}}.dart"])]).unwrap();
        assert_eq!(e.saidas("lib/a/b.dart").unwrap(), vec!["lib/generated/a/b.dart"]);
        assert!(e.saidas("test/a.dart").unwrap().is_empty());
        let e = ext(&[("{{dir}}/models/{{file}}.dart", &["{{dir}}/gen/{{file}}.g.dart"])]).unwrap();
        assert_eq!(e.saidas("lib/src/models/p.dart").unwrap(), vec!["lib/src/gen/p.g.dart"]);
    }

    #[test]
    fn validacoes() {
        assert!(ext(&[(".dart", &["{{}}.g.dart"])]).is_err());
        assert!(ext(&[("{{a}}{{a}}.dart", &["{{a}}.g"])]).is_err());
        assert!(ext(&[("{{a}}.dart", &["x.g"])]).is_err());
        assert!(ext(&[("{{a}}.dart", &["{{a}}{{a}}.g"])]).is_err());
        let e = ext(&[("^a", &["a"])]).unwrap();
        assert!(e.saidas("a").is_err());
    }
}
