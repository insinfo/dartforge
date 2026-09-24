//! `dartforge run` e `dartforge reload`: o perfil de desenvolvimento do
//! backend nativo, pelo JIT ORCv2 (`crates/jit`), atrás do feature `jit`.
//!
//! O feature é separado de `nativo` porque liga o `dartforge.exe` à
//! `LLVM-C.dll` da distribuição completa do LLVM: sem ela o executável não
//! inicia, nem para `compile-js`. Quem só quer o AOT não paga por isso.
//!
//! Os dois comandos consomem o **mesmo** LLVM IR que `dartforge aot` entrega ao
//! Clang (`dartforge_emit_native::emitir_ir`). Ver `docs/JIT.md`.
#![cfg_attr(not(feature = "jit"), allow(unused))]
use std::path::{Path, PathBuf};

type Resultado = Result<(), Box<dyn std::error::Error>>;

#[cfg(not(feature = "jit"))]
fn desabilitado(comando: &str) -> Resultado {
    Err(format!(
        "`dartforge {comando}` exige o JIT: compile com `cargo build -p dartforge-cli --features jit` \
         (precisa da distribuição completa do LLVM 22.1.8, com LLVM-C.dll no PATH; ver docs/JIT.md)"
    )
    .into())
}
#[cfg(not(feature = "jit"))]
pub fn run(_args: &[std::ffi::OsString]) -> Resultado {
    desabilitado("run")
}
#[cfg(not(feature = "jit"))]
pub fn reload(_args: &[std::ffi::OsString]) -> Resultado {
    desabilitado("reload")
}

/// Emite o IR de `entrada` numa thread de pilha grande, como `aot` faz: o
/// lowering é recursivo sobre a AST.
#[cfg(feature = "jit")]
fn emitir(
    entrada: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
) -> Result<dartforge_emit_native::IrEmitido, String> {
    let (entrada, sdk, packages) = (entrada.to_path_buf(), sdk.map(Path::to_path_buf), packages.map(Path::to_path_buf));
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let opcoes = dartforge_emit_native::CompileOptions {
                sdk: sdk.as_deref(),
                packages: packages.as_deref(),
                timings: false,
                optimize: false,
                versao_linguagem: None,
            };
            dartforge_emit_native::emitir_ir(&entrada, &opcoes)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a emissão do IR abortou".to_string())?
}

/// `dartforge run <entrada.dart> [--sdk <lib>] [--packages <cfg>] [--timings]`
/// ou `dartforge run --ir <programa.ll> [--timings]`.
///
/// Emite o IR e o executa numa sessão ORCv2 **deste** processo, pelo caminho
/// sem trampolim (`dartforge_jit::run_ir`): as chamadas são diretas, como no
/// executável AOT. O stdout é o do programa, e o código de saída também. O
/// runtime encerra o processo com 101/255 nos mesmos casos que o AOT.
///
/// `--timings` escreve em stderr, depois da execução, um objeto JSON com as
/// fases da emissão e do JIT (`docs/JIT.md`, «Medição por fase»).
#[cfg(feature = "jit")]
pub fn run(args: &[std::ffi::OsString]) -> Resultado {
    let usage = "usage: dartforge run <input.dart> [--sdk <lib>] [--packages <package_config.json>] [--timings]\n       dartforge run --ir <programa.ll> [--timings]";
    let mut entrada: Option<PathBuf> = None;
    let mut ir_pronto: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--ir") => ir_pronto = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            _ if entrada.is_none() && ir_pronto.is_none() => entrada = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let (ir, emissao) = match (entrada, ir_pronto) {
        (Some(entrada), None) => {
            let ir = emitir(&entrada, sdk.as_deref(), packages.as_deref())?;
            let tempos = ir.tempos;
            (ir.texto, Some(tempos))
        }
        (None, Some(arquivo)) => (
            std::fs::read_to_string(&arquivo).map_err(|e| format!("não foi possível ler {}: {e}", arquivo.display()))?,
            None,
        ),
        _ => return Err(usage.into()),
    };
    let relatorio = dartforge_jit::run_ir(&ir)?;
    use std::io::Write;
    let _ = std::io::stdout().flush();
    if timings {
        let (major, minor, patch) = dartforge_jit::JitSession::llvm_version();
        let fase = |d: Option<std::time::Duration>| d.map_or(0, |d| d.as_nanos());
        eprintln!(
            "{{\"llvm\":\"{major}.{minor}.{patch}\",\"frontend_ns\":{},\"hir_ns\":{},\"llvm_ir_ns\":{},\"ir_bytes\":{},\
             \"session_ns\":{},\"parse_ir_ns\":{},\"add_module_ns\":{},\"lookup_ns\":{},\"execute_ns\":{},\"jit_total_ns\":{}}}",
            fase(emissao.map(|t| t.frontend)),
            fase(emissao.map(|t| t.hir)),
            fase(emissao.map(|t| t.llvm_ir)),
            relatorio.module.ir_bytes,
            relatorio.session.as_nanos(),
            relatorio.module.parse_ir.as_nanos(),
            relatorio.module.add_module.as_nanos(),
            relatorio.entry.lookup.as_nanos(),
            relatorio.entry.execute.as_nanos(),
            relatorio.total.as_nanos(),
        );
    }
    if relatorio.entry.exit_code != 0 {
        std::process::exit(relatorio.entry.exit_code);
    }
    Ok(())
}

/// Carimbo dos fontes Dart sob `raiz`: nomes, instantes de modificação e
/// tamanhos em ordem estável. Muda quando um arquivo muda, entra, sai ou é
/// renomeado, mesmo que a soma de tamanhos e datas continue igual.
///
/// É deliberadamente simples: o R0 recompila o programa inteiro a cada
/// mudança, então saber **qual** arquivo mudou não serve para nada ainda. A
/// raiz é o diretório da entrada; `.dart_tool`, `build` e diretórios ocultos
/// ficam de fora.
#[cfg(feature = "jit")]
fn carimbo(raiz: &Path) -> (u64, usize) {
    use std::hash::{Hash, Hasher};

    fn andar(dir: &Path, fontes: &mut Vec<(PathBuf, u128, u64)>) {
        let Ok(entradas) = std::fs::read_dir(dir) else { return };
        for e in entradas.flatten() {
            let caminho = e.path();
            let nome = e.file_name();
            let nome = nome.to_string_lossy();
            let Ok(tipo) = e.file_type() else { continue };
            if tipo.is_dir() {
                if !nome.starts_with('.') && nome != "build" {
                    andar(&caminho, fontes);
                }
            } else if nome.ends_with(".dart") {
                let Ok(meta) = e.metadata() else { continue };
                if !meta.is_file() { continue }
                let t = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                fontes.push((caminho, t, meta.len()));
            }
        }
    }
    let mut fontes = Vec::new();
    andar(raiz, &mut fontes);
    fontes.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    fontes.hash(&mut hash);
    (hash.finish(), fontes.len())
}

/// Diretório das gerações de IR do `reload`, apagado em qualquer saída que
/// desempilhe (retorno, erro por `?`, pânico). `process::exit` não desempilha:
/// quem sai assim apaga antes, explicitamente.
#[cfg(feature = "jit")]
struct DiretorioGeracoes(PathBuf);
#[cfg(feature = "jit")]
impl Drop for DiretorioGeracoes {
    /// Remove o diretório inteiro; só este processo escreve nele (o nome leva o pid).
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `dartforge reload <entrada.dart> [--sdk] [--packages] [--intervalo <ms>] [--uma-vez] [--timings]`
///
/// **R0: reinício a quente, não hot reload.** A cada mudança nos fontes `.dart`
/// do diretório da entrada, o programa é recompilado inteiro. Se compila, o
/// processo da geração anterior é encerrado (se ainda estiver rodando) e a
/// geração nova começa do zero, pelo `main`. **Nenhum estado é preservado**:
/// heap, estáticos, nada. Isso está dito na saída de cada geração.
///
/// Se a compilação falha, o diagnóstico é mostrado e a geração em execução
/// **continua**: uma edição quebrada não derruba o que funciona (o mesmo
/// princípio do rollback da VM, `isolate_reload.cc`).
///
/// Cada geração roda num processo próprio (`dartforge run --ir`). Com isso o
/// reinício também vale para programas que ainda estão executando, e um
/// `exit`/`abort` do runtime encerra só aquela geração, não o observador.
///
/// Recarga com estado preservado é o R1 do plano do JIT
/// (`docs/PESQUISA-HOT-RELOAD.md`), e não existe ainda.
#[cfg(feature = "jit")]
pub fn reload(args: &[std::ffi::OsString]) -> Resultado {
    let usage = "usage: dartforge reload <input.dart> [--sdk <lib>] [--packages <package_config.json>] [--intervalo <ms>] [--uma-vez] [--timings]";
    let mut entrada: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut intervalo_ms = 300u64;
    let mut uma_vez = false;
    let mut timings = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.to_str() {
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--intervalo") => {
                intervalo_ms = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (10..=10_000).contains(v))
                    .ok_or("--intervalo exige milissegundos entre 10 e 10000")?;
            }
            Some("--uma-vez") => uma_vez = true,
            Some("--timings") => timings = true,
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let entrada = entrada.ok_or(usage)?;
    let raiz = entrada
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let proprio = std::env::current_exe()?;
    let temporario = DiretorioGeracoes(std::env::temp_dir().join(format!("dartforge-reload-{}", std::process::id())));
    std::fs::create_dir_all(&temporario.0)?;

    let mut geracao = 0u32;
    let mut filho: Option<std::process::Child> = None;
    let mut fim_informado = false;
    let mut ultimo = None;
    loop {
        let agora = carimbo(&raiz);
        if ultimo != Some(agora) {
            ultimo = Some(agora);
            let inicio = std::time::Instant::now();
            match emitir(&entrada, sdk.as_deref(), packages.as_deref()) {
                Err(erro) => {
                    eprintln!("[reload] erro de compilação:\n{erro}");
                    if geracao == 0 {
                        eprintln!("[reload] nenhuma geração em execução; esperando a próxima edição");
                    } else {
                        eprintln!("[reload] a geração {geracao} continua como estava");
                    }
                }
                Ok(ir) => {
                    let emissao = inicio.elapsed();
                    if let Some(mut anterior) = filho.take() {
                        if anterior.try_wait()?.is_none() {
                            let _ = anterior.kill();
                        }
                        let _ = anterior.wait();
                    }
                    // O processo da geração anterior terminou: o IR dela não
                    // serve mais, e uma sessão longa não acumula um `.ll` por edição.
                    if geracao > 0 {
                        let _ = std::fs::remove_file(temporario.0.join(format!("geracao-{geracao}.ll")));
                    }
                    geracao += 1;
                    let arquivo = temporario.0.join(format!("geracao-{geracao}.ll"));
                    std::fs::write(&arquivo, &ir.texto)?;
                    eprintln!(
                        "[reload] geração {geracao}: reinício a quente — o estado NÃO é preservado \
                         (o programa recomeça do main; hot reload com estado ainda não existe)"
                    );
                    if timings {
                        eprintln!(
                            "[reload] geração {geracao}: emissão do IR {:.1} ms ({} bytes)",
                            emissao.as_secs_f64() * 1000.0,
                            ir.texto.len()
                        );
                    }
                    let mut comando = std::process::Command::new(&proprio);
                    comando.arg("run").arg("--ir").arg(&arquivo);
                    if timings {
                        comando.arg("--timings");
                    }
                    filho = Some(comando.spawn()?);
                    fim_informado = false;
                }
            }
        }
        if let Some(atual) = filho.as_mut() {
            if !fim_informado {
                if let Some(status) = atual.try_wait()? {
                    fim_informado = true;
                    let codigo = status.code().unwrap_or(-1);
                    eprintln!("[reload] geração {geracao} terminou com código {codigo}");
                    if uma_vez {
                        if codigo != 0 {
                            drop(temporario);
                            std::process::exit(codigo);
                        }
                        return Ok(());
                    }
                }
            }
        } else if uma_vez && geracao == 0 {
            return Err("a primeira compilação falhou".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(intervalo_ms));
    }
}
