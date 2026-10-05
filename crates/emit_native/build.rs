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
    // A tabela de efeitos das externs decide onde a exceção pendente é
    // conferida: muda o código do SDK como uma fonte.
    let efeitos = raiz.join("../runtime/efeitos.tsv");
    println!("cargo::rerun-if-changed={}", efeitos.display());
    arquivos.push(efeitos);
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

/// O perfil de uma variante da `staticlib` do runtime, por cima do `release`
/// do workspace (entra no nome dela). `panic=abort`, nas variantes que só o
/// AOT liga (a do executável com o `main` C e a de produção): sem as tabelas
/// de desenrolamento e os caminhos de limpeza do Rust (~0,6 MB a menos por
/// executável). Quase nada muda: um pânico no código que o Dart chama já
/// encerrava o processo, porque chega a uma fronteira `extern "C"` (as
/// entradas do runtime, o `main` C, o código Dart que a thread de um isolado
/// roda) e o Rust aborta ali. O que muda é o pânico no Rust puro de uma
/// thread auxiliar (a preparação de um isolado, as threads do `dart:io`):
/// antes matava só a thread, agora encerra o processo. Nenhum `catch_unwind`
/// nem `join` do runtime conta com o desenrolamento (docs/NATIVO.md). A
/// variante da DLL de desenvolvimento, que o JIT também carrega, fica como
/// era.
const ABORTAR: &[&str] = &["--config", "profile.release.panic=\"abort\""];

/// Compila as três variantes da `staticlib` do runtime
/// (`crates/runtime_estatico`): a do executável sem o SDK da fonte (feature
/// `aot`, com o `main` C), a da DLL do SDK da fonte (feature `dll`; o perfil
/// de desenvolvimento e o JIT) e a de produção (feature `dll` com
/// [`ABORTAR`], ligada estaticamente no executável), para o alvo do build,
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
    // macOS: o C das dependências (zlib, `ring`) sai para o mesmo mínimo que
    // o `ld64.lld` declara no executável (`ligador_macos.rs`); sem isto o
    // `cc` usa o do SDK da máquina (26.x), e o programa poderia chamar o que
    // um macOS mais antigo não tem.
    println!("cargo::rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let minimo_macos = alvo.contains("apple-darwin").then(|| {
        std::env::var("MACOSX_DEPLOYMENT_TARGET")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| if alvo.starts_with("aarch64") { "11.0" } else { "10.12" }.to_string())
    });

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
    for (variante, feature, prefixo, perfil) in [
        ("PRINCIPAL", "aot", "dartforge_runtime_", ABORTAR),
        ("DLL", "dll", "dartforge_rtdll_", &[][..]),
        ("PRODUCAO", "dll", "dartforge_rtprod_", ABORTAR),
    ] {
        let mut h = blake3::Hasher::new();
        h.update(b"dartforge-runtime-cargo\0");
        h.update(alvo.as_bytes());
        h.update(b"\0");
        h.update(feature.as_bytes());
        h.update(b"\0");
        if let Some(m) = &minimo_macos {
            h.update(m.as_bytes());
            h.update(b"\0");
        }
        for c in perfil {
            h.update(c.as_bytes());
            h.update(b"\0");
        }
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
        // Um diretório de alvo por perfil: as dependências de um não
        // invalidam as do outro.
        let dir_alvo = saida.join(if perfil.is_empty() { "cargo-runtime" } else { "cargo-runtime-abort" });
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut cmd = std::process::Command::new(cargo);
        cmd.arg("build")
            .arg("--release")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(workspace.join("Cargo.toml"))
            .args(["-p", "dartforge-runtime-estatico", "--features", feature, "--target", &alvo])
            .args(perfil)
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
        // Tabelas de desenrolamento em todo quadro do runtime nos alvos do
        // desenrolador Itanium, também com `panic=abort`: o percurso das
        // raízes por mapas (`_Unwind_Backtrace`, `runtime/src/heap.rs`) e o
        // desenrolamento das exceções por tabelas atravessam os quadros do
        // runtime; sem a tabela, o desenrolador para no primeiro deles
        // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §3.6).
        if alvo.contains("linux") || alvo.contains("apple") {
            bandeiras.push_str("\x1f-Cforce-unwind-tables=yes");
        }
        cmd.env("CARGO_ENCODED_RUSTFLAGS", bandeiras).env_remove("RUSTFLAGS");
        if let Some(m) = &minimo_macos {
            cmd.env("MACOSX_DEPLOYMENT_TARGET", m);
        }
        let status = cmd.status().expect("executar o cargo do build");
        assert!(status.success(), "a compilação do runtime ({feature}) para {alvo} falhou");
        let produzido = dir_alvo.join(&alvo).join("release").join(arquivo_do_cargo);
        let tmp = saida.join(format!("{nome}.tmp"));
        std::fs::copy(&produzido, &tmp).unwrap_or_else(|e| panic!("copiar {}: {e}", produzido.display()));
        std::fs::rename(&tmp, &lib).expect("publicar o runtime pré-compilado");
    }
    println!("cargo::rustc-env=DARTFORGE_RUNTIME_DIR_DO_BUILD={}", saida.display());
}
