//! Os três executores: `dart run` (semântica), `dartdevc` + Node (contrato) e o DartForge.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::corpus::Programa;
use crate::processo::{Saida, executar, executar_com_path};

/// Onde estão as ferramentas. Construído uma vez por execução do harness.
#[derive(Debug, Clone)]
pub struct Ambiente {
    /// Raiz do repositório (pai de `crates/`).
    pub raiz: PathBuf,
    /// Raiz do SDK Dart (`C:/tools/dartsdk-3.6.2`); `DARTFORGE_DART_SDK` sobrepõe.
    pub sdk: PathBuf,
    /// `runtime/ddc/dart_sdk.js`.
    pub dart_sdk_js: PathBuf,
    /// Binário `dartforge` quando já compilado (`DARTFORGE_BIN` sobrepõe).
    pub dartforge_bin: Option<PathBuf>,
    /// `target/diferencial/`.
    pub trabalho: PathBuf,
    /// Usar o cache dos oráculos por hash do conteúdo.
    pub usar_cache: bool,
    /// Limite por processo.
    pub limite: Duration,
    /// Diretórios prefixados ao `PATH` do `dartforge` (a `LLVM-C.dll`; ver `scripts/env.ps1`).
    pub path_extra: Vec<PathBuf>,
}

impl Ambiente {
    /// Detecta tudo a partir do diretório do crate; gera `dart_sdk.js` se faltar.
    pub fn detectar() -> Ambiente {
        let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."));
        let raiz = sem_prefixo_verbatim(raiz);
        let sdk = std::env::var("DARTFORGE_DART_SDK").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("C:/tools/dartsdk-3.6.2"));
        let dart_sdk_js = raiz.join("runtime/ddc/dart_sdk.js");
        if !dart_sdk_js.is_file() {
            eprintln!("runtime/ddc/dart_sdk.js ausente; gerando com scripts/gerar-dart-sdk.ps1…");
            let s = executar("pwsh", &[raiz.join("scripts/gerar-dart-sdk.ps1").to_string_lossy().into_owned()], &raiz, Duration::from_secs(300));
            if s.codigo != 0 {
                eprintln!("falhou: {}", s.stderr);
            }
        }
        let target = std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| raiz.join("target"));
        let dartforge_bin = std::env::var("DARTFORGE_BIN").ok().map(PathBuf::from).or_else(|| {
            let exe = if cfg!(windows) { "dartforge.exe" } else { "dartforge" };
            let candidatos = [target.join("release").join(exe), target.join("debug").join(exe)];
            candidatos
                .iter()
                .filter(|p| p.is_file())
                .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
                .cloned()
        });
        let mut path_extra = Vec::new();
        for prefixo in [std::env::var("LLVM_SYS_221_PREFIX").ok(), std::env::var("DARTFORGE_LLVM_DIR").ok(), Some(r"D:\DartSDKs\llvm\clang+llvm-22.1.8-x86_64-pc-windows-msvc".to_string())].into_iter().flatten() {
            let bin = PathBuf::from(prefixo).join("bin");
            if bin.join("LLVM-C.dll").is_file() {
                path_extra.push(bin);
                break;
            }
        }
        Ambiente {
            path_extra,
            trabalho: target.join("diferencial"),
            raiz,
            sdk,
            dart_sdk_js,
            dartforge_bin,
            usar_cache: true,
            limite: Duration::from_secs(120),
        }
    }

    fn dartdevc_snapshot(&self) -> PathBuf {
        self.sdk.join("bin/snapshots/dartdevc.dart.snapshot")
    }

    /// Diretório de saída de um programa para um executor (`ddc`, `forge`, `contrato`).
    pub fn dir_saida(&self, executor: &str, programa: &Programa) -> PathBuf {
        self.trabalho.join(executor).join(&programa.nome)
    }
}

/// No Windows, `canonicalize` devolve `\\?\D:\…`, que confunde ferramentas externas.
fn sem_prefixo_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(r) => PathBuf::from(r),
        None => p,
    }
}

// ---------------------------------------------------------------- cache

fn hash_fnv(dados: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in dados {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn chave_cache(programa: &Programa) -> String {
    format!("{:016x}", hash_fnv(&programa.conteudo()))
}

fn cache_ler(amb: &Ambiente, executor: &str, chave: &str) -> Option<Saida> {
    if !amb.usar_cache {
        return None;
    }
    let texto = std::fs::read_to_string(amb.trabalho.join("cache").join(format!("{executor}-{chave}.txt"))).ok()?;
    // Formato: `codigo\nlen_stdout\n<stdout><stderr>`.
    let (codigo, resto) = texto.split_once('\n')?;
    let (len, corpo) = resto.split_once('\n')?;
    let len: usize = len.parse().ok()?;
    if !corpo.is_char_boundary(len) {
        return None;
    }
    Some(Saida { stdout: corpo[..len].to_string(), stderr: corpo[len..].to_string(), codigo: codigo.parse().ok()? })
}

fn cache_gravar(amb: &Ambiente, executor: &str, chave: &str, s: &Saida) {
    if !amb.usar_cache || s.codigo < 0 {
        return; // falhas do harness (ferramenta ausente, tempo) não entram no cache
    }
    let dir = amb.trabalho.join("cache");
    let _ = std::fs::create_dir_all(&dir);
    let texto = format!("{}\n{}\n{}{}", s.codigo, s.stdout.len(), s.stdout, s.stderr);
    let _ = std::fs::write(dir.join(format!("{executor}-{chave}.txt")), texto);
}

// ---------------------------------------------------------------- dart run

/// Oráculo de semântica: `dart run --enable-asserts arquivo` no diretório do arquivo.
/// Asserts ligados porque o DDC (e o modo de desenvolvimento do DartForge) os liga.
pub fn oraculo_dart(amb: &Ambiente, programa: &Programa) -> Saida {
    let chave = chave_cache(programa);
    if let Some(s) = cache_ler(amb, "dart-ea", &chave) {
        return s;
    }
    let nome = programa.entrada.file_name().unwrap().to_string_lossy().into_owned();
    let s = executar("dart", &["run".into(), "--enable-asserts".into(), nome], programa.diretorio(), amb.limite);
    cache_gravar(amb, "dart-ea", &chave, &s);
    s
}

// ---------------------------------------------------------------- dartdevc + node

/// Compila o programa com o `dartdevc` (`--modules=es6`) em `dir/<nome>.js` e devolve o JS
/// já apontando para o `dart_sdk.js` do repositório. `Err` traz a saída do compilador.
pub fn compilar_ddc(amb: &Ambiente, programa: &Programa, dir: &Path) -> Result<String, Saida> {
    let _ = std::fs::create_dir_all(dir);
    let js = dir.join(format!("{}.js", programa.nome));
    // `dartdevc` já segue os imports relativos; só a entrada é passada. O cwd é o
    // diretório do programa para o nome do módulo ser o nome do arquivo.
    let mut args = vec![
        amb.dartdevc_snapshot().to_string_lossy().into_owned(),
        "--modules=es6".into(),
        "-o".into(),
        js.to_string_lossy().into_owned(),
    ];
    // Programas com `package:` trazem o seu `.dart_tool/package_config.json` (o `dart run` o
    // encontra sozinho; o `dartdevc` precisa do `--packages`).
    let pacotes = programa.diretorio().join(".dart_tool/package_config.json");
    if pacotes.is_file() {
        args.push(format!("--packages={}", pacotes.to_string_lossy()));
    }
    args.push(programa.entrada.file_name().unwrap().to_string_lossy().into_owned());
    let s = executar("dart", &args, programa.diretorio(), amb.limite);
    if s.codigo != 0 {
        return Err(s);
    }
    let texto = std::fs::read_to_string(&js).map_err(|e| Saida::erro(format!("ler {}: {e}", js.display())))?;
    let url = url_arquivo(&amb.dart_sdk_js);
    let texto = texto.replace("from 'dart_sdk.js'", &format!("from '{url}'"));
    std::fs::write(&js, &texto).map_err(|e| Saida::erro(format!("escrever {}: {e}", js.display())))?;
    Ok(texto)
}

fn url_arquivo(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// Nome exportado da biblioteca de entrada: primeira entrada da linha `export { … };`
/// (`export { $01_print }`, `export { hello as x__hello }`, `export { main, util }`).
pub fn nome_exportado(js: &str) -> Option<String> {
    let linha = js.lines().find(|l| l.starts_with("export {"))?;
    let interior = linha.trim_start_matches("export {").trim_end_matches(';').trim_end_matches('}').trim();
    let primeiro = interior.split(',').next()?.trim();
    let nome = match primeiro.split_once(" as ") {
        Some((_, alias)) => alias.trim(),
        None => primeiro,
    };
    Some(nome.to_string())
}

/// `main.mjs` que executa `main()` do módulo e imita a VM no erro não capturado (código 255).
pub fn main_mjs(modulo: &str, exportado: &str) -> String {
    format!(
        "process.on('uncaughtException', (e) => {{\n  console.error('Unhandled exception:\\n' + e);\n  process.exit(255);\n}});\nimport {{ {exportado} as m }} from './{modulo}.js';\nm.main();\n"
    )
}

/// Executa `node main.mjs` em `dir`.
pub fn executar_node(amb: &Ambiente, dir: &Path) -> Saida {
    executar("node", &["main.mjs".into()], dir, amb.limite)
}

/// Oráculo do contrato: `dartdevc` + Node. Cache por hash do conteúdo.
pub fn oraculo_ddc(amb: &Ambiente, programa: &Programa, dir: &Path) -> Saida {
    let chave = chave_cache(programa);
    if let Some(s) = cache_ler(amb, "ddc", &chave) {
        return s;
    }
    let s = match compilar_ddc(amb, programa, dir) {
        Err(s) => s,
        Ok(js) => match nome_exportado(&js) {
            None => Saida::erro("dartdevc: linha `export { … }` não encontrada"),
            Some(exp) => {
                if std::fs::write(dir.join("main.mjs"), main_mjs(&programa.nome, &exp)).is_err() {
                    return Saida::erro("não foi possível escrever main.mjs");
                }
                executar_node(amb, dir)
            }
        },
    };
    cache_gravar(amb, "ddc", &chave, &s);
    s
}

// ---------------------------------------------------------------- dartforge

/// O DartForge: `dartforge compile-js arquivo -o dir` e depois `node dir/main.mjs`.
/// Sem cache: o emissor muda o tempo todo.
pub fn dartforge(amb: &Ambiente, programa: &Programa, dir: &Path) -> Saida {
    let _ = std::fs::create_dir_all(dir);
    let entrada = programa.entrada.to_string_lossy().into_owned();
    let saida = dir.to_string_lossy().into_owned();
    let s = match &amb.dartforge_bin {
        Some(bin) => executar_com_path(
            &bin.to_string_lossy(),
            &["compile-js".into(), entrada, "-o".into(), saida],
            programa.diretorio(),
            amb.limite,
            &amb.path_extra,
        ),
        None => executar_com_path(
            "cargo",
            &["run".into(), "-q".into(), "-p".into(), "dartforge-cli".into(), "--".into(), "compile-js".into(), entrada, "-o".into(), saida],
            &amb.raiz,
            Duration::from_secs(1800),
            &amb.path_extra,
        ),
    };
    if s.codigo != 0 {
        let primeira = s.primeira_linha_stderr().to_string();
        let primeira = if primeira.is_empty() { s.stdout.lines().next().unwrap_or("").to_string() } else { primeira };
        return Saida { stderr: format!("[compile-js código {}] {primeira}
{}", s.codigo, s.stderr), ..s };
    }
    if !dir.join("main.mjs").is_file() {
        return Saida::erro("[compile-js] devolveu 0 mas não escreveu main.mjs");
    }
    executar_node(amb, dir)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn exportado() {
        assert_eq!(nome_exportado("var a = 1;\nexport { $01_print };\n").as_deref(), Some("$01_print"));
        assert_eq!(nome_exportado("export { hello as target__hello };").as_deref(), Some("target__hello"));
        assert_eq!(nome_exportado("export { main, util };").as_deref(), Some("main"));
        assert_eq!(nome_exportado("nada"), None);
    }

    #[test]
    fn url() {
        assert_eq!(url_arquivo(Path::new(r"D:\x\dart_sdk.js")), "file:///D:/x/dart_sdk.js");
        assert_eq!(url_arquivo(Path::new("/tmp/dart_sdk.js")), "file:///tmp/dart_sdk.js");
    }
}
