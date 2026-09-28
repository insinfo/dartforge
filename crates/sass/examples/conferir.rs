//! Confere o compilador contra saídas do dart-sass 1.102.0 gravadas por um
//! oráculo (o `SassBuilder` reproduzido num programa Dart).
//!
//! Uso: `cargo run --release -p dartforge-sass --example conferir --
//! <manifesto> <dir do oráculo> [dir de diferenças]`.
//!
//! O manifesto tem uma linha por `.scss`: `<arquivo>\t<raiz do pacote>\t
//! <nome do pacote>\t<package_config.json ou ->`. O oráculo, para a linha
//! `i`, grava `i.expanded.css`, `i.expanded.css.map`, `i.compressed.css`,
//! `i.compressed.css.map` (o `.css` como o `sass_builder` o escreve, com o
//! comentário `sourceMappingURL`), ou `i.<estilo>.err` quando o dart-sass
//! recusa.
use dartforge_sass::sass_builder::{compilar, Ativo, Disco, Pacotes};
use dartforge_sass::OutputStyle;
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let manifesto = std::fs::read_to_string(&args[1]).expect("manifesto");
    let oraculo = PathBuf::from(&args[2]);
    let difs = args.get(3).map(PathBuf::from);
    if let Some(d) = &difs {
        let _ = std::fs::create_dir_all(d);
    }
    let (mut css_ok, mut map_ok, mut total, mut err_ok) = (0, 0, 0, 0);
    let mut ruins = Vec::new();
    for (i, linha) in manifesto.lines().enumerate() {
        let c: Vec<&str> = linha.split('\t').collect();
        if c.len() < 4 {
            continue;
        }
        let (arquivo, raiz, nome, cfg) = (Path::new(c[0]), Path::new(c[1]), c[2], c[3]);
        let mut pacotes = if cfg == "-" {
            Pacotes::new()
        } else {
            Pacotes::de_package_config(Path::new(cfg)).expect("cfg")
        };
        pacotes.inserir(nome, raiz);
        let caminho = arquivo
            .strip_prefix(raiz)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let entrada = Ativo {
            pacote: nome.into(),
            caminho,
        };
        let fonte = std::fs::read_to_string(arquivo).unwrap_or_default();
        for (estilo, en) in [
            (OutputStyle::Expanded, "expanded"),
            (OutputStyle::Compressed, "compressed"),
        ] {
            total += 1;
            let base = oraculo.join(format!("{i}.{en}"));
            let esperado_css =
                std::fs::read_to_string(base.with_extension(format!("{en}.css"))).ok();
            let esperado_map =
                std::fs::read_to_string(base.with_extension(format!("{en}.css.map"))).ok();
            // Cada compilação numa thread com pilha grande e prazo: um laço
            // infinito ou recursão funda não derruba a conferência.
            let (tx, rx) = std::sync::mpsc::channel();
            {
                let (fonte, entrada, pacotes) = (fonte.clone(), entrada.clone(), pacotes.clone());
                std::thread::Builder::new()
                    .stack_size(256 << 20)
                    .spawn(move || {
                        let r = std::panic::catch_unwind(|| {
                            compilar(&fonte, &entrada, &pacotes, &Disco, estilo, true)
                        })
                        .map(|r| r.map_err(|e| e.to_string()));
                        let _ = tx.send(r);
                    })
                    .expect("thread");
            }
            let r = match rx.recv_timeout(std::time::Duration::from_secs(20)) {
                Ok(r) => r,
                Err(_) => {
                    ruins.push(format!("{i} {en} {}: PRAZO", arquivo.display()));
                    continue;
                }
            };
            let rotulo = format!("{i} {en} {}", arquivo.display());
            match (r, esperado_css) {
                (Ok(Ok(c)), Some(e)) => {
                    // O trailer do mapa entra mesmo antes de o mapa existir.
                    let mut com_mapa = c.clone();
                    com_mapa.mapa.get_or_insert_with(String::new);
                    let nosso = com_mapa.arquivo_css(&entrada);
                    let igual = nosso == e;
                    css_ok += igual as usize;
                    let mapa_igual = c.mapa.as_deref() == esperado_map.as_deref();
                    map_ok += mapa_igual as usize;
                    if !igual || !mapa_igual {
                        ruins.push(format!(
                            "{rotulo}: {}{}",
                            if igual { "" } else { "CSS " },
                            if mapa_igual { "" } else { "MAPA" }
                        ));
                        if let Some(d) = &difs {
                            let _ = std::fs::write(d.join(format!("{i}.{en}.nosso.css")), &nosso);
                            let _ = std::fs::write(d.join(format!("{i}.{en}.oficial.css")), &e);
                            let _ = std::fs::write(
                                d.join(format!("{i}.{en}.nosso.map")),
                                c.mapa.unwrap_or_default(),
                            );
                            let _ = std::fs::write(
                                d.join(format!("{i}.{en}.oficial.map")),
                                esperado_map.unwrap_or_default(),
                            );
                        }
                    }
                }
                (Ok(Err(_)), None) => {
                    err_ok += 1;
                    css_ok += 1;
                    map_ok += 1;
                }
                (Ok(Err(e)), Some(_)) => ruins.push(format!(
                    "{rotulo}: ERRO nosso: {}",
                    e.lines().next().unwrap_or("")
                )),
                (Ok(Ok(_)), None) => {
                    ruins.push(format!("{rotulo}: oficial rejeita, nosso compila"))
                }
                (Err(_), _) => ruins.push(format!("{rotulo}: PÂNICO")),
            }
        }
    }
    for r in &ruins {
        println!("{r}");
    }
    println!(
        "total {total}: css iguais {css_ok}, mapas iguais {map_ok} (erros em ambos: {err_ok})"
    );
}
