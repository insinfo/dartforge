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
    /// Limite so para EXECUTAR o binario nativo.
    ///
    /// Separado de `limite` porque os dois medem coisas diferentes: o oraculo
    /// `dart run` pode levar segundos so para subir, enquanto um programa do
    /// corpus rodando nativo termina em milissegundos — se demora, entrou em
    /// laco. E laco infinito no nativo nao custa so tempo: enquanto gira, ele
    /// aloca, e varios em paralelo tomam a memoria da maquina. Curto por
    /// padrao; `--limite-exec` ajusta.
    pub limite_nativo: Duration,
    /// Diretórios prefixados ao `PATH` do `dartforge` (a `LLVM-C.dll`; ver `scripts/env.ps1`).
    pub path_extra: Vec<PathBuf>,
}

impl Ambiente {
    /// Detecta tudo a partir do diretório do crate; gera `dart_sdk.js` se faltar.
    pub fn detectar() -> Ambiente {
        // Com `target/` compartilhado entre worktrees, o `CARGO_MANIFEST_DIR`
        // embutido aponta para a árvore que compilou este binário por último,
        // não para a árvore de quem o está executando. A raiz é o diretório
        // corrente quando ele tem o corpus (é o que `cargo run` faz), senão o
        // caminho embutido.
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let raiz = if cwd.join("corpus/js").is_dir() {
            cwd
        } else {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
        };
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
            // Pelo mesmo motivo, o `dartforge` certo é o que está ao lado deste
            // executável (mesmo `target/<perfil>/`), antes de qualquer outro.
            let ao_lado = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(exe)));
            let mut candidatos: Vec<PathBuf> = ao_lado.into_iter().collect();
            candidatos.push(target.join("release").join(exe));
            candidatos.push(target.join("debug").join(exe));
            candidatos.into_iter().find(|p| p.is_file())
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
            limite_nativo: Duration::from_secs(5),
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

/// O DartForge Nativo: compila via `dartforge_emit_native::compilar` e depois executa o binário.
pub fn dartforge_nativo(amb: &Ambiente, programa: &Programa, dir: &Path) -> Saida {
    let _ = std::fs::create_dir_all(dir);
    let exe_nome = if cfg!(windows) { format!("{}.exe", programa.nome) } else { programa.nome.clone() };
    let saida_exe = dir.join(&exe_nome);

    let entrada = programa.entrada.clone();
    let saida = saida_exe.clone();
    let comp_res = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: None,
                packages: None,
                timings: false,
                optimize: false,
            };
            dartforge_emit_native::compilar(&entrada, &saida, &options)
        })
        .map_err(|e| format!("falha ao criar thread de compilação: {e}"))
        .and_then(|h| h.join().map_err(|_| "a thread de compilação abortou".to_string()))
        .and_then(|r| r);

    if let Err(e) = comp_res {
        let primeira = e.lines().next().unwrap_or("").to_string();
        return Saida {
            stdout: String::new(),
            stderr: format!("[compile-native] {primeira}\n{e}"),
            codigo: 1,
        };
    }

    if !saida_exe.is_file() {
        return Saida::erro("[compile-native] devolveu Ok mas não gerou executável");
    }

    let s = executar_com_path(&saida_exe.to_string_lossy(), &[], dir, amb.limite_nativo, &amb.path_extra);
    // O executável já disse o que tinha a dizer; 214 deles ficariam no disco
    // a cada passada. DARTFORGE_KEEP_EXE os mantém para depurar.
    if std::env::var_os("DARTFORGE_KEEP_EXE").is_none() {
        let _ = std::fs::remove_file(&saida_exe);
    }
    s
}

/// Só a emissão do backend nativo: o LLVM IR do programa, sem Clang, ligação
/// nem execução. É o que o modo determinismo compara — o executável é função
/// do IR, da versão do Clang e da `.lib` do runtime, e o que pode variar com a
/// ordem dos trabalhadores é o nosso código, que aparece inteiro no IR.
///
/// Mesma pilha de 1 GiB de [`dartforge_nativo`]. Um pânico na emissão vira
/// `Err` com a mensagem dele (sem o id da thread, que mudaria de execução para
/// execução), para ser comparado como qualquer outro resultado.
pub fn dartforge_nativo_ir(programa: &Programa) -> Result<String, String> {
    let entrada = programa.entrada.clone();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions {
                sdk: None,
                packages: None,
                timings: false,
                optimize: false,
            };
            dartforge_emit_native::emitir_ir(&entrada, &options).map(|ir| ir.texto)
        })
        .map_err(|e| format!("[emitir-ir] falha ao criar thread de emissão: {e}"))?
        .join()
        .map_err(|p| format!("[emitir-ir] a thread abortou: {}", mensagem_de_panico(p.as_ref())))?
        .map_err(|e| format!("[emitir-ir] {e}"))
}

fn mensagem_de_panico(carga: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = carga.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = carga.downcast_ref::<String>() {
        s.clone()
    } else {
        "(pânico sem mensagem)".to_string()
    }
}

// ---------------------------------------------------------------- produção

/// O **perfil de produção**: `dartforge-jsprod arquivo -o dir/saida.js` e depois
/// `node dir/saida.js`. Um arquivo só, com o `dart_sdk.js` podado pelo mundo
/// fechado (`docs/JS-PRODUCAO.md`).
///
/// Sem cache, pelo mesmo motivo do perfil de desenvolvimento: o que está sendo
/// desenvolvido é justamente este executor.
pub fn dartforge_producao(amb: &Ambiente, programa: &Programa, dir: &Path) -> Saida {
    let _ = std::fs::create_dir_all(dir);
    let entrada = programa.entrada.to_string_lossy().into_owned();
    let saida = dir.join("saida.js");
    let saida_s = saida.to_string_lossy().into_owned();
    let exe = if cfg!(windows) { "dartforge-jsprod.exe" } else { "dartforge-jsprod" };
    // O binário certo é o que está ao lado deste executável (mesmo
    // `target/<perfil>/`), como em `Ambiente::detectar`.
    let bin = std::env::var("DARTFORGE_JSPROD_BIN")
        .ok()
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(exe))).filter(|p| p.is_file()));
    let mut args = vec![entrada, "-o".into(), saida_s];
    let pacotes = programa.diretorio().join(".dart_tool/package_config.json");
    if pacotes.is_file() {
        args.push("--packages".into());
        args.push(pacotes.to_string_lossy().into_owned());
    }
    let s = match &bin {
        Some(b) => executar_com_path(&b.to_string_lossy(), &args, programa.diretorio(), amb.limite, &amb.path_extra),
        None => {
            let mut a: Vec<String> = vec!["run".into(), "-q".into(), "--release".into(), "-p".into(), "dartforge-emit-js-producao".into(), "--".into()];
            a.extend(args);
            executar_com_path("cargo", &a, &amb.raiz, Duration::from_secs(1800), &amb.path_extra)
        }
    };
    if s.codigo != 0 {
        let primeira = s.primeira_linha_stderr().to_string();
        let primeira = if primeira.is_empty() { s.stdout.lines().next().unwrap_or("").to_string() } else { primeira };
        return Saida { stderr: format!("[jsprod código {}] {primeira}
{}", s.codigo, s.stderr), ..s };
    }
    if !saida.is_file() {
        return Saida::erro("[jsprod] devolveu 0 mas não escreveu o arquivo");
    }
    // Um bundle que nem parseia tem de falhar com mensagem própria, não com um
    // erro de execução a dez quadros de profundidade (docs/JS-PRODUCAO.md §5.2).
    let check = executar("node", &["--check".into(), "saida.js".into()], dir, amb.limite);
    if check.codigo != 0 {
        return Saida::erro(format!("[jsprod] node --check reprovou o bundle: {}", check.primeira_linha_stderr()));
    }
    executar("node", &["saida.js".into()], dir, amb.limite)
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

// ---------------------------------------------------------------- JIT

/// Tempo a mais que o processo do JIT recebe, além de `limite_nativo`, por
/// carregar o LLVM e gerar o código dentro do mesmo processo.
const FOLGA_GERACAO_JIT: Duration = Duration::from_secs(10);

/// O executor isolado do JIT (`crates/jit`, binário `dartforge-executar-ir`).
///
/// `DARTFORGE_EXECUTAR_IR` sobrepõe; senão, o executável ao lado deste
/// harness, que é onde `cargo build -p dartforge-jit` o deixa no mesmo
/// `target/`. O harness não liga o LLVM: o JIT é um processo por programa,
/// porque o runtime encerra o processo nos mesmos casos em que o executável
/// AOT termina (`docs/JIT.md`, «Execução e término»).
pub fn executor_jit() -> PathBuf {
    if let Some(p) = std::env::var_os("DARTFORGE_EXECUTAR_IR") {
        return PathBuf::from(p);
    }
    let nome = if cfg!(windows) { "dartforge-executar-ir.exe" } else { "dartforge-executar-ir" };
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(nome)))
        .unwrap_or_else(|| PathBuf::from(nome))
}

/// O perfil de **desenvolvimento** do backend nativo: o LLVM IR do programa
/// executado pelo JIT ORCv2, num processo do executor.
///
/// Com `com_aot`, o **mesmo texto** de IR também passa pelo driver AOT (Clang +
/// ligação), e o executável é executado. É o contrato de `crates/jit`:
/// os dois perfis têm de dar o mesmo stdout e o mesmo código, programa a
/// programa. Os tempos de cada lado ficam no resultado.
///
/// O erro de emissão sai com o mesmo prefixo `[compile-native]` do modo
/// `--nativo`: é o mesmo emissor, e os placares dos dois modos têm de agrupar
/// as falhas pelas mesmas chaves.
pub fn dartforge_jit(amb: &Ambiente, programa: &Programa, dir: &Path, com_aot: bool) -> (Saida, crate::relatorio::ExecucaoJit) {
    use crate::relatorio::{AotDoMesmoIr, ExecucaoJit};
    let _ = std::fs::create_dir_all(dir);
    let ir = match dartforge_nativo_ir(programa) {
        Ok(ir) => ir,
        Err(e) => {
            let e = e.strip_prefix("[emitir-ir] ").unwrap_or(&e).to_string();
            let primeira = e.lines().next().unwrap_or("").to_string();
            let saida = Saida { stdout: String::new(), stderr: format!("[compile-native] {primeira}\n{e}"), codigo: 1 };
            let aot = com_aot.then(|| AotDoMesmoIr { saida: saida.clone(), ligacao: Duration::ZERO, execucao: Duration::ZERO, objeto_do_cache: false });
            return (saida, ExecucaoJit { com_ir: false, tempo: Duration::ZERO, execucao: None, aot });
        }
    };
    let ll = dir.join(format!("{}.ll", programa.nome));
    if let Err(e) = std::fs::write(&ll, &ir) {
        return (Saida::erro(format!("[jit] não foi possível gravar {}: {e}", ll.display())), ExecucaoJit { com_ir: true, tempo: Duration::ZERO, execucao: None, aot: None });
    }
    let executor = executor_jit();
    // O limite do AOT vale só para executar o .exe, depois do Clang e da
    // ligação. O processo do JIT também carrega a LLVM-C.dll (72 MB) e gera o
    // código antes de executar: frio e com a máquina disputada, isso já passou
    // de 3 s num programa que executa em 5 ms. Sem a folga, o mesmo programa
    // estouraria o tempo só no JIT, e o placar acusaria JIT≠AOT à toa.
    let limite = amb.limite_nativo + FOLGA_GERACAO_JIT;
    let inicio = std::time::Instant::now();
    let mut jit = executar_com_path(
        &executor.to_string_lossy(),
        &[ll.to_string_lossy().into_owned(), "--timings".to_string()],
        dir,
        limite,
        &amb.path_extra,
    );
    let tempo = inicio.elapsed();
    let execucao_jit = separar_tempos_do_executor(&mut jit);
    // Código 70 é o executor recusando o IR (verificador, externo
    // desconhecido, alvo): o equivalente ao Clang recusar o módulo no AOT, e
    // sai com a mesma forma que o `--nativo` dá a esse caso.
    if jit.codigo == CODIGO_FALHA_DO_JIT && jit.stdout.is_empty() {
        let primeira = jit.primeira_linha_stderr().to_string();
        jit = Saida { stdout: String::new(), stderr: format!("[compile-native] {primeira}
{}", jit.stderr), codigo: 1 };
    }

    let aot = com_aot.then(|| {
        let exe = dir.join(if cfg!(windows) { format!("{}.exe", programa.nome) } else { programa.nome.clone() });
        let inicio = std::time::Instant::now();
        let ligado = dartforge_emit_native::driver::compile_and_link(&ir, &exe, &dartforge_emit_native::driver::NativeDriverOptions::default());
        let ligacao = inicio.elapsed();
        match ligado {
            Err(e) => {
                let primeira = e.lines().next().unwrap_or("").to_string();
                let saida = Saida { stdout: String::new(), stderr: format!("[compile-native] {primeira}\n{e}"), codigo: 1 };
                AotDoMesmoIr { saida, ligacao, execucao: Duration::ZERO, objeto_do_cache: false }
            }
            Ok(t) => {
                let inicio = std::time::Instant::now();
                let saida = executar_com_path(&exe.to_string_lossy(), &[], dir, amb.limite_nativo, &amb.path_extra);
                let execucao = inicio.elapsed();
                if std::env::var_os("DARTFORGE_KEEP_EXE").is_none() {
                    let _ = std::fs::remove_file(&exe);
                }
                AotDoMesmoIr { saida, ligacao, execucao, objeto_do_cache: t.objeto_do_cache }
            }
        }
    });
    if std::env::var_os("DARTFORGE_KEEP_IR").is_none() {
        let _ = std::fs::remove_file(&ll);
    }
    (jit, ExecucaoJit { com_ir: true, tempo, execucao: execucao_jit, aot })
}

/// Código com que `dartforge-executar-ir` sai quando o próprio JIT falha
/// (`crates/jit/src/bin/executar_ir.rs`).
const CODIGO_FALHA_DO_JIT: i32 = 70;

/// Tira do stderr a linha JSON de `--timings` do executor e devolve o tempo de
/// execução do programa que ela traz (`execute_ns`).
///
/// A linha sai do stderr para não virar a chave de falha nem entrar em
/// nenhuma comparação: ela só existe no perfil JIT.
fn separar_tempos_do_executor(saida: &mut Saida) -> Option<Duration> {
    let linha = saida.stderr.lines().rev().find(|l| l.starts_with("{\"llvm\""))?.to_string();
    saida.stderr = saida.stderr.replacen(&format!("{linha}\n"), "", 1).replacen(&linha, "", 1);
    let valor = linha.split("\"execute_ns\":").nth(1)?;
    let digitos: String = valor.chars().take_while(|c| c.is_ascii_digit()).collect();
    digitos.parse::<u64>().ok().map(Duration::from_nanos)
}
