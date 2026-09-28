//! Conferência do shim de CSS (`css::shim`, o porte do `shimShadowCss`)
//! contra o compilador oficial, byte a byte. Os dois testes dependem de
//! arquivos de fora do repositório e ficam `#[ignore]`:
//!
//! - `folhas_de_um_projeto_iguais_ao_oficial`: percorre a saída de uma build
//!   oficial (`DARTFORGE_SHIM_GERADO`, o `.dart_tool/build/generated`) e, para
//!   cada `X.css.shim.dart`, gera o nosso com [`gerar_folha`] a partir do
//!   `X.css` ao lado (saída do `sass_builder`) ou, se não houver, do `X.css`
//!   no pacote (`DARTFORGE_SHIM_RAIZES`, `pacote=dir` separados por `,`).
//!
//!   ```text
//!   DARTFORGE_SHIM_GERADO=…/example/.dart_tool/build/generated \
//!   DARTFORGE_SHIM_RAIZES=limitless_ui=…/limitless_ui,limitless_ui_example=…/example \
//!   cargo test --release -p dartforge-gerador-ng --test shim_conferencia -- --ignored --nocapture
//!   ```
//!
//! - `shim_igual_ao_oraculo_dart`: lê a saída de um programa Dart que chama o
//!   `shimShadowCss` oficial numa lista de `.css` (`DARTFORGE_SHIM_ORACULO`;
//!   registros `=== caminho`, `OK`/`ERR`, o texto e `\0`) e compara: saída
//!   igual, ou recusa onde o oficial lança.
use std::path::{Path, PathBuf};

use dartforge_gerador_ng::{Pacote, css, gerar_folha, shadow_css};

fn arquivos(dir: &Path, sufixo: &str, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, sufixo, saida);
        } else if p.to_string_lossy().ends_with(sufixo) {
            saida.push(p);
        }
    }
}

/// A primeira posição em que `a` e `b` diferem, com um trecho de cada.
fn diferenca(a: &str, b: &str) -> String {
    let i = a
        .char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map(|((i, _), _)| i)
        .unwrap_or(a.len().min(b.len()));
    let trecho = |s: &str| {
        let ini = s.floor_char_boundary(i.saturating_sub(60));
        let fim = s.ceil_char_boundary((i + 80).min(s.len()));
        s[ini..fim].to_string()
    };
    format!(
        "byte {i}\n  oficial: {}\n  nosso:   {}",
        trecho(a),
        trecho(b)
    )
}

#[test]
#[ignore]
fn folhas_de_um_projeto_iguais_ao_oficial() {
    let Ok(gerado) = std::env::var("DARTFORGE_SHIM_GERADO") else {
        eprintln!("defina DARTFORGE_SHIM_GERADO");
        return;
    };
    let gerado = PathBuf::from(gerado);
    let raizes: Vec<(String, PathBuf)> = std::env::var("DARTFORGE_SHIM_RAIZES")
        .unwrap_or_default()
        .split(',')
        .filter_map(|par| {
            let (n, d) = par.split_once('=')?;
            Some((n.to_string(), PathBuf::from(d)))
        })
        .collect();
    let mut shims = Vec::new();
    arquivos(&gerado, ".css.shim.dart", &mut shims);
    shims.sort();
    let (mut iguais, mut diferentes, mut recusados) = (0, Vec::new(), Vec::new());
    for shim in &shims {
        let oficial = std::fs::read_to_string(shim)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let texto = shim.to_string_lossy();
        let mut css = PathBuf::from(texto.trim_end_matches(".shim.dart"));
        if !css.is_file() {
            // Folha do próprio pacote: `generated/<pacote>/<caminho>`.
            let relativo = css.strip_prefix(&gerado).map(Path::to_path_buf);
            if let Ok(rel) = relativo {
                let mut partes = rel.components();
                let pacote = partes
                    .next()
                    .map(|c| c.as_os_str().to_string_lossy().to_string());
                if let Some((_, raiz)) = raizes.iter().find(|(n, _)| Some(n) == pacote.as_ref()) {
                    css = raiz.join(partes.as_path());
                }
            }
        }
        let pacote = Pacote::default();
        let saidas = gerar_folha(&pacote, &css);
        let nossa = saidas
            .into_iter()
            .find(|(p, _)| p.to_string_lossy().ends_with(".shim.dart"))
            .map(|(_, r)| r);
        match nossa {
            Some(Ok(t)) if t == oficial => iguais += 1,
            Some(Ok(t)) => diferentes.push((shim.clone(), diferenca(&oficial, &t))),
            Some(Err(r)) => recusados.push((shim.clone(), format!("{r:?}"))),
            None => recusados.push((shim.clone(), "sem saída".into())),
        }
    }
    for (p, d) in &diferentes {
        eprintln!("DIFERENTE {}\n{d}", p.display());
    }
    for (p, r) in &recusados {
        eprintln!("RECUSADO {}: {r}", p.display());
    }
    println!(
        "shim do projeto: {} folhas, {iguais} iguais, {} diferentes, {} recusadas",
        shims.len(),
        diferentes.len(),
        recusados.len()
    );
    assert!(diferentes.is_empty() && recusados.is_empty());
}

#[test]
#[ignore]
fn shim_igual_ao_oraculo_dart() {
    let Ok(oraculo) = std::env::var("DARTFORGE_SHIM_ORACULO") else {
        eprintln!("defina DARTFORGE_SHIM_ORACULO");
        return;
    };
    let texto = std::fs::read_to_string(&oraculo).expect("oráculo");
    let (mut total, mut iguais, mut erros_iguais) = (0, 0, 0);
    let mut diferentes = Vec::new();
    let mut motivos = std::collections::BTreeMap::new();
    for registro in texto.split("\0\n") {
        let Some(resto) = registro.strip_prefix("=== ") else {
            continue;
        };
        let Some((caminho, resto)) = resto.split_once('\n') else {
            continue;
        };
        let Ok(css) = std::fs::read_to_string(caminho) else {
            continue;
        };
        // O `readAsStringSync` do oráculo (o `utf8.decode`, como o
        // `readAsString` do builder) tira o BOM do início.
        let css = css.strip_prefix('\u{FEFF}').unwrap_or(&css);
        total += 1;
        let nossa = css::shim(css);
        if let Some(esperado) = resto.strip_prefix("OK\n") {
            let esperado = esperado.strip_suffix('\n').unwrap_or(esperado);
            match nossa {
                Ok(t) if t == esperado => iguais += 1,
                Ok(t) => diferentes.push(format!("{caminho}\n{}", diferenca(esperado, &t))),
                Err(_) => diferentes.push(format!("{caminho}\n  oficial gera, nós recusamos")),
            }
        } else {
            match nossa {
                Err(_) => {
                    erros_iguais += 1;
                    let nosso = shadow_css::shim_shadow_css(css, "_ngcontent-%ID%", "_nghost-%ID%")
                        .err()
                        .map(|e| e.to_string())
                        .unwrap_or_default();
                    let oficial = resto.lines().nth(1).unwrap_or("").to_string();
                    *motivos.entry((oficial, nosso)).or_insert(0usize) += 1;
                }
                Ok(_) => diferentes.push(format!(
                    "{caminho}\n  oficial lança ({}), nós geramos",
                    resto.trim()
                )),
            }
        }
    }
    for d in &diferentes {
        eprintln!("DIFERENTE {d}");
    }
    for ((oficial, nosso), n) in &motivos {
        println!("  exceção oficial {oficial:<20} nossa {nosso:<45} {n}");
    }
    println!(
        "oráculo dart: {total} folhas, {iguais} iguais, {erros_iguais} recusadas como no oficial, {} diferentes",
        diferentes.len()
    );
    assert!(diferentes.is_empty());
}
