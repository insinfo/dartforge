//! A impressão digital do compilador que entra na chave do cache do SDK da
//! fonte (P5c, `src/sdk_modulo.rs`): o blake3 das fontes de quem decide o
//! código de uma biblioteca do SDK — o lowering e o emissor (esta crate), a
//! inferência (`types`), o modelo de elementos (`elements`) e o front-end.
//! Uma mudança em qualquer um deles invalida os objetos do SDK em cache; a
//! versão do pacote sozinha não bastaria (ela não muda a cada commit).
//!
//! E o runtime do AOT **pré-compilado** (`precompilar_runtime`): as duas
//! variantes da `staticlib` (com o `main` C e a da DLL do SDK da fonte) são
//! compiladas aqui, pelo `rustc` do próprio build e para o alvo do build, e
//! vão para a distribuição ao lado do executável. Quem usa o dartforge não
//! precisa de Rust instalado.

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
    precompilar_runtime();
}

/// Bandeiras do `rustc` para o runtime (as mesmas de `src/cache.rs`).
const BANDEIRAS_RUSTC: [&str; 3] = ["--edition=2024", "--crate-type=staticlib", "-O"];

/// Compila as duas variantes do runtime para o alvo e publica, para o
/// binário, o diretório e os nomes delas. O nome leva o blake3 do fonte e
/// das bandeiras: um runtime de outra versão nunca é confundido com o deste
/// compilador. `DARTFORGE_RUNTIME_SEM_PRECOMPILAR=1` pula a etapa (o AOT cai
/// na compilação sob demanda com o `rustc` da máquina, como antes).
fn precompilar_runtime() {
    println!("cargo::rerun-if-env-changed=DARTFORGE_RUNTIME_SEM_PRECOMPILAR");
    let saida = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let alvo = std::env::var("TARGET").expect("TARGET");
    let windows = alvo.contains("windows");
    let ext = if windows { "lib" } else { "a" };
    let fonte = format!("#![allow(warnings)]\n{}\n", dartforge_runtime::RUNTIME_MAIN);
    let pular = std::env::var("DARTFORGE_RUNTIME_SEM_PRECOMPILAR").is_ok_and(|v| v == "1");
    for (variante, extras) in [("PRINCIPAL", &[][..]), ("DLL", &["--cfg", "dartforge_runtime_dll"][..])] {
        let mut h = blake3::Hasher::new();
        h.update(b"dartforge-runtime\0");
        h.update(alvo.as_bytes());
        for b in BANDEIRAS_RUSTC.iter().chain(extras) {
            h.update(b.as_bytes());
            h.update(&[0]);
        }
        h.update(fonte.as_bytes());
        let hash = &h.finalize().to_hex()[..32];
        let prefixo = if variante == "DLL" { "dartforge_rtdll_" } else { "dartforge_runtime_" };
        let nome = format!("{prefixo}{hash}.{ext}");
        println!("cargo::rustc-env=DARTFORGE_RUNTIME_{variante}={nome}");
        let lib = saida.join(&nome);
        if pular || lib.is_file() {
            continue;
        }
        // O fonte com o nome `runtime_<hash>.rs`: é o caminho que as
        // mensagens de pânico do runtime mostram, e o harness o normaliza.
        let rs = saida.join(format!("runtime_{hash}.rs"));
        std::fs::write(&rs, &fonte).expect("escrever o fonte do runtime");
        let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
        let tmp = saida.join(format!("{nome}.tmp"));
        let status = std::process::Command::new(&rustc)
            .args(BANDEIRAS_RUSTC)
            .args(extras)
            .args(["--target", &alvo])
            .arg(&rs)
            .arg("-o")
            .arg(&tmp)
            .status()
            .expect("executar o rustc do build");
        assert!(status.success(), "a compilação do runtime ({variante}) para {alvo} falhou");
        std::fs::rename(&tmp, &lib).expect("publicar o runtime pré-compilado");
    }
    println!("cargo::rustc-env=DARTFORGE_RUNTIME_DIR_DO_BUILD={}", saida.display());
}
