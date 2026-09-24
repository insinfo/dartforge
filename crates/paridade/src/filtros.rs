//! Os filtros que o analyzer aplica por último (plano §2.2, item 4):
//! `analyzer: exclude:` e `analyzer: errors:` do `analysis_options.yaml`, e os
//! comentários `// ignore:` e `// ignore_for_file:`.
//!
//! Referências (analyzer 6.11.0): `IgnoreInfo` (`src/ignore_comments/
//! ignore_info.dart`: o comentário numa linha só vale para a linha seguinte;
//! no fim de uma linha de código, para ela mesma), `ErrorCode.isIgnorable`
//! (erro de severidade `ERROR` não se ignora por comentário) e o
//! `ErrorProcessor` das opções (`ignore` remove; `info`/`warning`/`error`
//! trocam a severidade).
//!
//! Só o subconjunto de YAML que esses três campos usam é lido; `include:` não
//! é seguido (só traria lints, que são o passo A4).

use dartforge_diagnostics::{Diagnostic, Severidade};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// O que o `analysis_options.yaml` configura para os diagnósticos.
#[derive(Debug, Default, Clone)]
pub struct Opcoes {
    /// Globs de `analyzer: exclude:`, relativos ao diretório das opções.
    pub exclude: Vec<String>,
    /// `analyzer: errors:` — código → `None` (ignore) ou a nova severidade.
    pub errors: BTreeMap<String, Option<Severidade>>,
}

impl Opcoes {
    /// Lê `<raiz>/analysis_options.yaml`, se existir.
    pub fn ler(raiz: &Path) -> Opcoes {
        match std::fs::read_to_string(raiz.join("analysis_options.yaml")) {
            Ok(t) => Opcoes::de_texto(&t),
            Err(_) => Opcoes::default(),
        }
    }

    pub fn de_texto(texto: &str) -> Opcoes {
        let mut o = Opcoes::default();
        // Pilha de chaves por indentação.
        let mut caminho: Vec<(usize, String)> = Vec::new();
        for linha in texto.lines() {
            let sem_coment = match linha.find(" #") {
                Some(i) => &linha[..i],
                None if linha.trim_start().starts_with('#') => "",
                None => linha,
            };
            if sem_coment.trim().is_empty() {
                continue;
            }
            let ind = sem_coment.len() - sem_coment.trim_start().len();
            let t = sem_coment.trim();
            while caminho.last().is_some_and(|(i, _)| *i >= ind) {
                caminho.pop();
            }
            let chaves: Vec<&str> = caminho.iter().map(|(_, k)| k.as_str()).collect();
            if let Some(item) = t.strip_prefix("- ") {
                if chaves == ["analyzer", "exclude"] {
                    o.exclude.push(sem_aspas(item).to_string());
                }
                continue;
            }
            let Some((k, v)) = t.split_once(':') else { continue };
            let (k, v) = (sem_aspas(k.trim()), sem_aspas(v.trim()));
            if v.is_empty() {
                caminho.push((ind, k.to_string()));
            } else if chaves == ["analyzer", "errors"] {
                let sev = match v {
                    "ignore" => None,
                    "info" => Some(Severidade::Info),
                    "warning" => Some(Severidade::Warning),
                    "error" => Some(Severidade::Error),
                    _ => continue,
                };
                o.errors.insert(k.to_lowercase(), sev);
            } else if chaves == ["analyzer"] && k == "exclude" && v.starts_with('[') {
                for g in v.trim_matches(|c| c == '[' || c == ']').split(',') {
                    let g = sem_aspas(g.trim());
                    if !g.is_empty() {
                        o.exclude.push(g.to_string());
                    }
                }
            }
        }
        o
    }

    /// `rel` (relativo à raiz, com `/`) casa algum `exclude`?
    pub fn excluido(&self, rel: &str) -> bool {
        self.exclude.iter().any(|g| glob(g, rel))
    }

    /// Aplica `errors:` a um diagnóstico: `None` se ignorado.
    pub fn processar(&self, mut d: Diagnostic) -> Option<Diagnostic> {
        let Some(c) = d.code else { return Some(d) };
        match self.errors.get(c.info().nome) {
            None => Some(d),
            Some(None) => None,
            Some(Some(s)) => {
                d.severity = *s;
                Some(d)
            }
        }
    }
}

fn sem_aspas(s: &str) -> &str {
    s.trim_matches(|c| c == '\'' || c == '"')
}

/// Glob do `package:glob` no subconjunto usado em `exclude`: `**` casa
/// qualquer sequência (inclusive `/`), `*` qualquer sequência sem `/`, `?` um
/// caractere.
pub fn glob(padrao: &str, texto: &str) -> bool {
    fn casa(p: &[u8], t: &[u8]) -> bool {
        if p.is_empty() {
            return t.is_empty();
        }
        if p.starts_with(b"**") {
            let resto = p[2..].strip_prefix(b"/").unwrap_or(&p[2..]);
            return (0..=t.len()).any(|i| casa(resto, &t[i..])) || casa(&p[2..], t);
        }
        match p[0] {
            b'*' => (0..=t.len()).take_while(|&i| i == 0 || t[i - 1] != b'/').any(|i| casa(&p[1..], &t[i..])),
            b'?' => !t.is_empty() && t[0] != b'/' && casa(&p[1..], &t[1..]),
            c => !t.is_empty() && t[0] == c && casa(&p[1..], &t[1..]),
        }
    }
    casa(padrao.as_bytes(), texto.as_bytes())
}

/// Os `// ignore:` de um arquivo: por linha (1-based) e para o arquivo todo.
#[derive(Debug, Default)]
pub struct Ignorados {
    por_linha: BTreeMap<usize, BTreeSet<String>>,
    arquivo: BTreeSet<String>,
}

impl Ignorados {
    pub fn de_texto(texto: &str) -> Ignorados {
        let mut ig = Ignorados::default();
        for (i, linha) in texto.lines().enumerate() {
            let n = i + 1;
            let Some(pos) = linha.find("//") else { continue };
            let coment = linha[pos + 2..].trim_start();
            let (lista, para_arquivo) = if let Some(r) = coment.strip_prefix("ignore_for_file:") {
                (r, true)
            } else if let Some(r) = coment.strip_prefix("ignore:") {
                (r, false)
            } else {
                continue;
            };
            let nomes: BTreeSet<String> = lista
                .split(',')
                .map(|s| s.trim().split_whitespace().next().unwrap_or("").to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if para_arquivo {
                ig.arquivo.extend(nomes);
            } else {
                // Sozinho na linha: vale para a próxima; depois de código, para esta.
                let alvo = if linha[..pos].trim().is_empty() { n + 1 } else { n };
                ig.por_linha.entry(alvo).or_default().extend(nomes);
            }
        }
        ig
    }

    /// O diagnóstico na `linha` é ignorado por comentário?
    pub fn ignora(&self, d: &Diagnostic, linha: usize) -> bool {
        let Some(c) = d.code else { return false };
        let info = c.info();
        // `ErrorCode.isIgnorable`: pela severidade do código, não a efetiva.
        if info.severidade == Severidade::Error {
            return false;
        }
        let tipo = format!("type={}", info.tipo.nome().to_lowercase());
        let casa = |s: &BTreeSet<String>| s.contains(info.nome) || s.contains(&tipo);
        casa(&self.arquivo) || self.por_linha.get(&linha).is_some_and(casa)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn opcoes_do_new_sali() {
        let o = Opcoes::de_texto(
            "analyzer:\n  exclude:\n    - build/**\n    - 'lib/**.g.dart'\n  errors:\n    uri_has_not_been_generated: ignore\n    deprecated_member_use: ignore # x\n    dead_code: info\n",
        );
        assert_eq!(o.exclude, vec!["build/**", "lib/**.g.dart"]);
        assert_eq!(o.errors.get("uri_has_not_been_generated"), Some(&None));
        assert_eq!(o.errors.get("dead_code"), Some(&Some(Severidade::Info)));
        assert!(o.excluido("build/x/y.dart"));
        assert!(o.excluido("lib/a/b.g.dart"));
        assert!(!o.excluido("lib/a/b.dart"));
    }

    #[test]
    fn glob_basico() {
        assert!(glob("*.dart", "a.dart"));
        assert!(!glob("*.dart", "x/a.dart"));
        assert!(glob("**/*.dart", "x/y/a.dart"));
        assert!(glob("**", "x/y"));
    }
}
