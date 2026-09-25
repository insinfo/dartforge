//! A impressão digital do compilador que entra na chave do cache do SDK da
//! fonte (P5c, `src/sdk_modulo.rs`): o blake3 das fontes de quem decide o
//! código de uma biblioteca do SDK — o lowering e o emissor (esta crate), a
//! inferência (`types`), o modelo de elementos (`elements`) e o front-end.
//! Uma mudança em qualquer um deles invalida os objetos do SDK em cache; a
//! versão do pacote sozinha não bastaria (ela não muda a cada commit).

use std::path::{Path, PathBuf};

fn juntar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_dir() {
            juntar(&p, saida);
        } else if p.extension().is_some_and(|x| x == "rs") {
            saida.push(p);
        }
    }
}

fn main() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut arquivos = Vec::new();
    for d in ["src", "../types/src", "../elements/src", "../frontend/src", "../runtime/src"] {
        let dir = raiz.join(d);
        println!("cargo::rerun-if-changed={}", dir.display());
        juntar(&dir, &mut arquivos);
    }
    arquivos.sort();
    let mut h = blake3::Hasher::new();
    for a in &arquivos {
        let rel = a.strip_prefix(raiz).unwrap_or(a).to_string_lossy().replace('\\', "/");
        h.update(rel.as_bytes());
        h.update(&[0]);
        // Fim de linha normalizado: o mesmo compilador em checkouts com
        // `core.autocrlf` diferentes tem a mesma impressão.
        let texto = std::fs::read(a).unwrap_or_default();
        let texto: Vec<u8> = texto.into_iter().filter(|&b| b != b'\r').collect();
        h.update(&texto);
        h.update(&[0]);
    }
    println!("cargo::rustc-env=DARTFORGE_EMISSOR_HASH={}", h.finalize().to_hex());
}
