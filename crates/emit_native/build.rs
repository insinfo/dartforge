//! A impressão digital do compilador que entra na chave do cache do SDK da
//! fonte (P5c, `src/sdk_modulo.rs`): o blake3 das fontes de quem decide o
//! código de uma biblioteca do SDK — o lowering e o emissor (esta crate), a
//! inferência (`types`), o modelo de elementos (`elements`) e o front-end.
//! Uma mudança em qualquer um deles invalida os objetos do SDK em cache; a
//! versão do pacote sozinha não bastaria (ela não muda a cada commit).
//!
//! E o runtime do AOT **pré-compilado** (`precompilar_runtime`): as duas
//! variantes da `staticlib` (com o `main` C e a da DLL do SDK da fonte) são
//! compiladas aqui, pelo `cargo` do próprio build e para o alvo do build, e
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

/// Compila as duas variantes da `staticlib` do runtime
/// (`crates/runtime_estatico`, features `aot` e `dll`) para o alvo do build,
/// com um `cargo build` próprio (outro diretório de alvo: nada disputa a
/// trava do build de fora), e publica, para o binário, o diretório e os
/// nomes delas. O nome leva o blake3 do fonte do runtime, do `Cargo.lock`,
/// do alvo e da variante: um runtime de outra versão nunca é confundido com
/// o deste compilador. `DARTFORGE_RUNTIME_SEM_PRECOMPILAR=1` pula a
/// compilação (para `cargo check`/`clippy`, que não geram executáveis).
fn precompilar_runtime() {
    println!("cargo::rerun-if-env-changed=DARTFORGE_RUNTIME_SEM_PRECOMPILAR");
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = raiz.join("../..");
    let saida = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let alvo = std::env::var("TARGET").expect("TARGET");
    let windows = alvo.contains("windows");
    let pular = std::env::var("DARTFORGE_RUNTIME_SEM_PRECOMPILAR").is_ok_and(|v| v == "1");

    let mut fontes = Vec::new();
    for d in ["../runtime/src", "../runtime_estatico/src"] {
        juntar(&raiz.join(d), &mut fontes);
    }
    for f in ["../runtime/build.rs", "../runtime/Cargo.toml", "../runtime_estatico/Cargo.toml", "../../Cargo.lock"] {
        let p = raiz.join(f);
        println!("cargo::rerun-if-changed={}", p.display());
        fontes.push(p);
    }
    println!("cargo::rerun-if-changed={}", raiz.join("../runtime_estatico/src").display());
    fontes.sort();

    let arquivo_do_cargo = if windows { "dartforge_runtime_estatico.lib" } else { "libdartforge_runtime_estatico.a" };
    let ext = if windows { "lib" } else { "a" };
    for (variante, feature, prefixo) in [("PRINCIPAL", "aot", "dartforge_runtime_"), ("DLL", "dll", "dartforge_rtdll_")] {
        let mut h = blake3::Hasher::new();
        h.update(b"dartforge-runtime-cargo\0");
        h.update(alvo.as_bytes());
        h.update(b"\0");
        h.update(feature.as_bytes());
        h.update(b"\0");
        for f in &fontes {
            let rel = f.strip_prefix(raiz).unwrap_or(f).to_string_lossy().replace('\\', "/");
            h.update(rel.as_bytes());
            h.update(&[0]);
            let texto: Vec<u8> = std::fs::read(f).unwrap_or_default().into_iter().filter(|&b| b != b'\r').collect();
            h.update(&texto);
            h.update(&[0]);
        }
        let nome = format!("{prefixo}{}.{ext}", &h.finalize().to_hex()[..32]);
        println!("cargo::rustc-env=DARTFORGE_RUNTIME_{variante}={nome}");
        let lib = saida.join(&nome);
        if pular || lib.is_file() {
            continue;
        }
        let dir_alvo = saida.join("cargo-runtime");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        cmd.arg("build")
            .arg("--release")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(workspace.join("Cargo.toml"))
            .args(["-p", "dartforge-runtime-estatico", "--features", feature, "--target", &alvo])
            .arg("--target-dir")
            .arg(&dir_alvo);
        if std::env::var_os("CARGO_NET_OFFLINE").is_some() {
            cmd.arg("--offline");
        }
        // O `clippy-driver` (e qualquer invólucro do workspace) é do build
        // de fora; o diretório de alvo é o daqui.
        for v in ["RUSTC_WORKSPACE_WRAPPER", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET_DIR", "CARGO_BUILD_TARGET"] {
            cmd.env_remove(v);
        }
        // Sem o caminho da máquina que compilou nas mensagens de pânico (e o
        // mesmo runtime em qualquer checkout).
        let mut bandeiras = std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
        if !bandeiras.is_empty() {
            bandeiras.push('\x1f');
        }
        let ws = workspace.canonicalize().unwrap_or_else(|_| workspace.clone());
        bandeiras.push_str(&format!("--remap-path-prefix={}=dartforge", ws.display()));
        cmd.env("CARGO_ENCODED_RUSTFLAGS", bandeiras).env_remove("RUSTFLAGS");
        let status = cmd.status().expect("executar o cargo do build");
        assert!(status.success(), "a compilação do runtime ({feature}) para {alvo} falhou");
        let produzido = dir_alvo.join(&alvo).join("release").join(arquivo_do_cargo);
        let tmp = saida.join(format!("{nome}.tmp"));
        std::fs::copy(&produzido, &tmp).unwrap_or_else(|e| panic!("copiar {}: {e}", produzido.display()));
        std::fs::rename(&tmp, &lib).expect("publicar o runtime pré-compilado");
    }
    println!("cargo::rustc-env=DARTFORGE_RUNTIME_DIR_DO_BUILD={}", saida.display());
}
