//! Compara o nosso Sass com o do `sass_builder`, arquivo por arquivo.
//!
//! O `build_runner` já deixou a saída dele em `.dart_tool/build/generated`;
//! aqui cada `.scss` do projeto é compilado pelos dois e os resultados são
//! comparados **normalizados** — o CSS intermediário não precisa sair com o
//! mesmo espaçamento, precisa ser o mesmo CSS. Quem normaliza é o shim do
//! ngdart, que é exatamente o que consome essa saída.
//!
//! ```text
//! cargo run -p dartforge-gerador-ng --example conferir-sass -- C:/MyDartProjects/new_sali/frontend
//! ```
use dartforge_gerador_ng::{css, sass};
use std::path::{Path, PathBuf};

fn main() -> std::process::ExitCode {
    let Some(raiz) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("uso: conferir-sass <raiz do projeto>");
        return std::process::ExitCode::FAILURE;
    };
    let raiz =
        dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&raiz).unwrap_or(raiz));
    let gerado = raiz.join(".dart_tool/build/generated");
    let pacote = raiz
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut pacotes: Vec<PathBuf> = Vec::new();
    if let Ok(e) = std::fs::read_dir(&gerado) {
        pacotes.extend(e.flatten().map(|x| x.path()).filter(|p| p.is_dir()));
    }

    // `--bytes`: compara o CSS do `compilar_com(.., Comprimido)` byte a byte
    // com o do `sass_builder` (sem o comentário do mapa), em vez de depois
    // do shim.
    let bytes = std::env::args().any(|a| a == "--bytes");
    let (mut iguais, mut diferentes, mut recusados, mut sem_oficial) = (0, 0, 0, 0);
    let mut exemplos: Vec<String> = Vec::new();
    for scss in arquivos(&raiz, "scss") {
        // O vendorado (`web/assets/js/...`) não é do projeto.
        if scss.to_string_lossy().contains("assets") {
            continue;
        }
        let Ok(rel) = scss.strip_prefix(&raiz) else {
            continue;
        };
        let oficial = pacotes
            .iter()
            .map(|p| p.join(rel).with_extension("css"))
            .find(|p| p.is_file())
            .or_else(|| {
                let alt = gerado.join(&pacote).join(rel).with_extension("css");
                alt.is_file().then_some(alt)
            });
        let Some(oficial) = oficial else {
            sem_oficial += 1;
            continue;
        };
        let Ok(fonte) = std::fs::read_to_string(&scss) else {
            continue;
        };
        if bytes {
            let deles = std::fs::read_to_string(&oficial).unwrap_or_default();
            let deles = deles.trim_start_matches('\u{feff}');
            let deles = match deles.find("\n/*# sourceMappingURL=") {
                Some(i) => &deles[..i],
                None => deles,
            };
            match sass::compilar_com(&fonte, scss.parent(), sass::Estilo::Comprimido) {
                Ok((a, _)) if a == deles => iguais += 1,
                Ok((a, _)) => {
                    diferentes += 1;
                    let i = a
                        .char_indices()
                        .zip(deles.chars())
                        .find(|((_, x), y)| x != y)
                        .map(|((i, _), _)| i)
                        .unwrap_or(a.len().min(deles.len()));
                    exemplos.push(format!(
                        "diferente: {} (byte {i})\n    oficial: …{}\n    nosso:   …{}",
                        rel.display(),
                        janela(deles, i),
                        janela(&a, i)
                    ));
                }
                Err(_) => recusados += 1,
            }
            continue;
        }
        let nosso = match sass::compilar_em(&fonte, scss.parent()) {
            Ok(c) => c,
            Err(_) => {
                recusados += 1;
                if exemplos.len() < 30 {
                    exemplos.push(format!("recusado: {}", rel.display()));
                }
                continue;
            }
        };
        let deles = std::fs::read_to_string(&oficial).unwrap_or_default();
        // O BOM e o comentário de source map do sass_builder não são CSS.
        let deles = deles.trim_start_matches('\u{feff}');
        match (css::shim(&nosso), css::shim(deles)) {
            (Ok(a), Ok(b)) if a == b => iguais += 1,
            (Ok(a), Ok(b)) => {
                diferentes += 1;
                // Diferença sempre aparece: é o que quebra o oráculo.
                {
                    let i = a
                        .char_indices()
                        .zip(b.chars())
                        .find(|((_, x), y)| x != y)
                        .map(|((i, _), _)| i)
                        .unwrap_or(a.len().min(b.len()));
                    exemplos.push(format!(
                        "diferente: {} (byte {i})
    oficial: …{}
    nosso:   …{}",
                        rel.display(),
                        janela(&b, i),
                        janela(&a, i)
                    ));
                }
            }
            _ => {
                recusados += 1;
                if exemplos.len() < 30 {
                    exemplos.push(format!("shim recusou: {}", rel.display()));
                }
            }
        }
    }
    println!("iguais ao sass_builder: {iguais}");
    println!("diferentes:             {diferentes}");
    println!("recusados por nós:      {recusados}");
    println!("sem .css oficial:       {sem_oficial}");
    for e in &exemplos {
        println!("  {e}");
    }
    if diferentes == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

fn janela(s: &str, i: usize) -> String {
    let ini = i.saturating_sub(40);
    let ini = (ini..=i).find(|k| s.is_char_boundary(*k)).unwrap_or(i);
    s[ini..].chars().take(110).collect()
}

fn arquivos(raiz: &Path, ext: &str) -> Vec<PathBuf> {
    let mut saida = Vec::new();
    let mut pilha = vec![raiz.join("lib"), raiz.join("web")];
    while let Some(d) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                pilha.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                saida.push(p);
            }
        }
    }
    saida
}
