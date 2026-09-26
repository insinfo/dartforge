//! `dartforge run` e `dartforge reload`: o perfil de desenvolvimento do
//! backend nativo, pelo JIT ORCv2 (`crates/jit`), atrás do feature `jit`.
//!
//! O feature é separado de `nativo` porque liga o `dartforge` ao LLVM da
//! distribuição completa (no Windows, à `LLVM-C.dll`: sem ela o executável
//! não inicia, nem para `compile-js`; no Linux e no macOS, estático ou à
//! `libLLVM` do pacote). Quem só quer o AOT não paga por isso.
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
         (precisa da distribuição completa do LLVM 22.1.8; no Windows, com LLVM-C.dll no PATH; ver docs/JIT.md)"
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
    emitir_geracao(entrada, sdk, packages, None)
}

/// [`emitir`] de uma geração nova do programa em execução: `anterior` é o IR
/// da geração viva, cujo layout de estáticos a nova estende.
#[cfg(feature = "jit")]
fn emitir_geracao(
    entrada: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
    anterior: Option<std::sync::Arc<String>>,
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
                experimentos: Vec::new(),
            };
            dartforge_emit_native::emitir_ir_recarregavel(&entrada, &opcoes, anterior.as_deref().map(String::as_str))
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a emissão do IR abortou".to_string())?
}

/// A biblioteca compartilhada do SDK da fonte que o IR importa:
/// `DARTFORGE_SDK_DLL` se definida, senão a do cache (compilada agora se
/// faltar); `None` quando o IR não usa o SDK da fonte.
#[cfg(feature = "jit")]
fn biblioteca_do_sdk(ir: &str) -> Result<Option<PathBuf>, String> {
    if !dartforge_jit::ir_usa_sdk_da_fonte(ir) {
        return Ok(None);
    }
    if let Some(d) = std::env::var_os("DARTFORGE_SDK_DLL") {
        return Ok(Some(PathBuf::from(d)));
    }
    dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().map(Some)
}

/// `dartforge run [--sdk <lib>] [--packages <cfg>] [--timings] <entrada.dart>
/// [<argumentos do main>…]` ou `dartforge run --ir <programa.ll> [--timings]
/// [<argumentos>…]`. O que vem depois do programa é do `main(List<String>
/// args)`, como no `dart run`; um `--timings` logo depois da entrada continua
/// sendo do dartforge (a forma antiga).
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
    let usage = "usage: dartforge run [--sdk <lib>] [--packages <package_config.json>] [--timings] <input.dart> [<argumentos do main>…]\n       dartforge run --ir <programa.ll> [--timings] [<argumentos do main>…]";
    let mut entrada: Option<PathBuf> = None;
    let mut ir_pronto: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut timings = false;
    // Como no `dart run`: as opções antes do programa são do dartforge; o
    // que vem depois dele é do `main(List<String> args)`.
    let mut do_programa: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if entrada.is_some() || ir_pronto.is_some() {
            match a.to_str() {
                Some("--timings") if do_programa.is_empty() => timings = true,
                _ => do_programa.push(a.to_string_lossy().into_owned()),
            }
            continue;
        }
        match a.to_str() {
            Some("--ir") => ir_pronto = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--sdk") => sdk = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--packages") => packages = Some(PathBuf::from(it.next().ok_or(usage)?)),
            Some("--timings") => timings = true,
            _ => entrada = Some(PathBuf::from(a)),
        }
    }
    dartforge_jit::definir_argumentos(do_programa);
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
    let sdk_dll = biblioteca_do_sdk(&ir)?;
    let relatorio = dartforge_jit::run_ir_com(&ir, sdk_dll.as_deref())?;
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

/// Carimbo dos fontes Dart sob `raiz` e da configuração de pacotes usada pelo
/// carregador: nomes, instantes de modificação e tamanhos em ordem estável.
/// Muda quando um arquivo muda, entra, sai ou é renomeado, mesmo que a soma
/// de tamanhos e datas continue igual.
///
/// É deliberadamente simples: o R0 recompila o programa inteiro a cada
/// mudança, então saber **qual** arquivo mudou não serve para nada ainda. A
/// raiz é o diretório da entrada; `.dart_tool`, `build` e diretórios ocultos
/// ficam fora da varredura de fontes. `package_config.json` é observado à parte.
#[cfg(feature = "jit")]
fn carimbo(raiz: &Path, packages: Option<&Path>) -> (u64, usize) {
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
    let config = packages.map(Path::to_path_buf)
        .or_else(|| dartforge_elements::config::PackageConfig::discover(raiz));
    config.hash(&mut hash);
    if let Some(config) = config {
        let estado = std::fs::metadata(&config).ok().map(|meta| {
            let modificado = meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos());
            (modificado, meta.len())
        });
        estado.hash(&mut hash);
    }
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

/// `dartforge reload <entrada.dart> [--sdk] [--packages] [--intervalo <ms>] [--uma-vez] [--timings] [--reiniciar]`
///
/// **Hot reload com estado (padrão).** O programa roda numa thread própria de
/// uma `JitSession`; a cada mudança nos fontes `.dart` do diretório da
/// entrada, o programa é recompilado e a geração nova é publicada no ponto
/// seguro do isolado principal — entre dois eventos do laço —, sem executar
/// o `main` de novo: heap, estáticos, timers, portas e conexões continuam, e o
/// próximo evento já chama o código novo (`dartforge_jit::ProgramaVivo`). Se
/// o programa já terminou, a geração nova é publicada e o `main` roda de novo
/// sobre o mesmo estado.
///
/// Uma edição que não compila não derruba nada: o diagnóstico é mostrado e a
/// geração em execução continua. Uma edição que a recarga não sabe aplicar
/// sobre os objetos vivos (a assinatura de uma função mudou, uma classe
/// ganhou campos ou foi renumerada) é recusada com o motivo, e o programa
/// **reinicia** com o código novo: quem observa é um supervisor, e a sessão
/// roda num processo filho (`--filho`) que ele recria.
///
/// `--reiniciar` é o R0: reinício a quente a cada edição, sem estado, cada
/// geração num processo próprio (`dartforge run --ir`).
#[cfg(feature = "jit")]
pub fn reload(args: &[std::ffi::OsString]) -> Resultado {
    let usage = "usage: dartforge reload <input.dart> [--sdk <lib>] [--packages <package_config.json>] [--intervalo <ms>] [--uma-vez] [--timings] [--reiniciar]";
    let mut entrada: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut intervalo_ms = 300u64;
    let mut uma_vez = false;
    let mut timings = false;
    let mut reiniciar = false;
    let mut filho = false;
    let mut supervisor: Option<u16> = None;
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
            Some("--reiniciar") => reiniciar = true,
            // A forma antiga do hot reload com estado, que agora é o padrão.
            Some("--preservar-estado") => {}
            Some("--filho") => filho = true,
            Some("--supervisor") => {
                supervisor = Some(it.next().and_then(|v| v.to_str()).and_then(|v| v.parse().ok()).ok_or(usage)?);
            }
            _ if entrada.is_none() => entrada = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let entrada = entrada.ok_or(usage)?;
    if !reiniciar {
        if filho {
            if let Some(porta) = supervisor {
                vigiar_supervisor(porta)?;
            }
            return reload_ao_vivo(&entrada, sdk.as_deref(), packages.as_deref(), intervalo_ms, uma_vez, timings);
        }
        return supervisionar(args);
    }
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
        let agora = carimbo(&raiz, packages.as_deref());
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

/// O código de saída com que o processo da sessão pede para ser recriado:
/// uma edição que a recarga não aplica sobre os objetos vivos.
#[cfg(feature = "jit")]
const CODIGO_REINICIAR: i32 = 75;

/// O supervisor do hot reload: roda a sessão num processo filho e o recria
/// quando ele pede (`CODIGO_REINICIAR`); qualquer outro fim é o do programa.
///
/// O filho conecta num soquete local do supervisor e termina quando a
/// conexão fecha: matar o supervisor (Ctrl+C, o `kill` de uma IDE) não deixa
/// o programa rodando órfão.
#[cfg(feature = "jit")]
fn supervisionar(args: &[std::ffi::OsString]) -> Resultado {
    let proprio = std::env::current_exe()?;
    let vigia = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let porta = vigia.local_addr()?.port();
    vigia.set_nonblocking(true)?;
    loop {
        let mut filho = std::process::Command::new(&proprio)
            .arg("reload")
            .args(args)
            .arg("--filho")
            .arg("--supervisor")
            .arg(porta.to_string())
            .spawn()?;
        // A conexão do filho fica aberta enquanto o supervisor vive.
        let mut _conexao = None;
        let status = loop {
            if _conexao.is_none()
                && let Ok((c, _)) = vigia.accept()
            {
                _conexao = Some(c);
            }
            if let Some(status) = filho.try_wait()? {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(if _conexao.is_some() { 100 } else { 10 }));
        };
        match status.code() {
            Some(CODIGO_REINICIAR) => eprintln!("[reload] reiniciando o programa com o código novo"),
            Some(0) => return Ok(()),
            Some(codigo) => std::process::exit(codigo),
            None => return Err("a sessão de recarga terminou por sinal".into()),
        }
    }
}

/// O filho: conecta no supervisor e sai quando a conexão fecha.
#[cfg(feature = "jit")]
fn vigiar_supervisor(porta: u16) -> Resultado {
    use std::io::Read;
    let mut conexao = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, porta))?;
    std::thread::Builder::new().name("dartforge-vigia".into()).spawn(move || {
        let mut byte = [0u8; 1];
        // Nada é escrito nesta conexão: o `read` só volta quando ela fecha.
        let _ = conexao.read(&mut byte);
        std::process::exit(CODIGO_REINICIAR + 1);
    })?;
    Ok(())
}

/// Encerra a sessão pedindo ao supervisor que a recrie. Sai já: o programa
/// em execução (uma thread da sessão) não tem como ser interrompido, e o
/// processo é a unidade de reinício — portas e arquivos abertos fecham com ele.
#[cfg(feature = "jit")]
fn pedir_reinicio() -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    std::process::exit(CODIGO_REINICIAR);
}

/// Milissegundos, para os relatórios.
#[cfg(feature = "jit")]
fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// A sessão do hot reload (o processo filho do supervisor).
#[cfg(feature = "jit")]
fn reload_ao_vivo(
    entrada: &Path,
    sdk: Option<&Path>,
    packages: Option<&Path>,
    intervalo_ms: u64,
    uma_vez: bool,
    timings: bool,
) -> Resultado {
    let raiz = entrada.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let intervalo = std::time::Duration::from_millis(intervalo_ms);
    // A primeira geração: a sessão depende do IR (o runtime embutido ou a
    // biblioteca do SDK da fonte). Uma primeira compilação que falha espera
    // a próxima edição.
    let mut ultimo = None;
    let (mut sessao, ir) = loop {
        let agora = carimbo(raiz, packages);
        if ultimo != Some(agora) {
            ultimo = Some(agora);
            match emitir(entrada, sdk, packages) {
                Ok(ir) => {
                    let sdk_dll = biblioteca_do_sdk(&ir.texto)?;
                    let sessao = dartforge_jit::JitSession::new_for_ir_com(&ir.texto, sdk_dll.as_deref())?;
                    break (sessao, ir);
                }
                Err(erro) if uma_vez => return Err(erro.into()),
                Err(erro) => {
                    eprintln!("[reload] erro de compilação:\n{erro}");
                    eprintln!("[reload] nenhuma geração em execução; esperando a próxima edição");
                }
            }
        }
        std::thread::sleep(intervalo);
    };
    let da_fonte = sessao.usa_sdk_da_fonte();
    let mut vivo_ir = std::sync::Arc::new(ir.texto);
    let resultado = sessao.com_programa_vivo(|vivo| -> Result<i32, Box<dyn std::error::Error>> {
        vivo.ao_esperar_ponto_seguro(|| {
            eprintln!("[reload] esperando o programa chegar a um ponto seguro (entre dois eventos)…");
        });
        let relatorio = vivo.publicar("app", &vivo_ir)?;
        eprintln!("[reload] geração {}: estado preservado na mesma sessão", relatorio.generation);
        vivo.executar_main()?;
        loop {
            if let Some(fim) = vivo.terminou() {
                if uma_vez {
                    return Ok(fim.exit_code);
                }
                eprintln!("[reload] o programa terminou com código {}; esperando edições", fim.exit_code);
            }
            let agora = carimbo(raiz, packages);
            if ultimo != Some(agora) {
                ultimo = Some(agora);
                let inicio = std::time::Instant::now();
                let ir = match emitir_geracao(entrada, sdk, packages, Some(vivo_ir.clone())) {
                    Ok(ir) => ir,
                    Err(erro) => {
                        eprintln!("[reload] erro de compilação:\n{erro}");
                        eprintln!("[reload] a versão em execução continua como estava");
                        std::thread::sleep(intervalo);
                        continue;
                    }
                };
                let emissao = inicio.elapsed();
                if dartforge_jit::ir_usa_sdk_da_fonte(&ir.texto) != da_fonte {
                    eprintln!("[reload] a edição mudou o perfil de runtime do programa");
                    pedir_reinicio();
                }
                let executando = vivo.executando();
                match vivo.publicar("app", &ir.texto) {
                    Ok(relatorio) => {
                        vivo_ir = std::sync::Arc::new(ir.texto);
                        if executando {
                            eprintln!(
                                "[reload] geração {}: publicada no programa em execução (estado preservado)",
                                relatorio.generation
                            );
                        } else {
                            eprintln!("[reload] geração {}: estado preservado na mesma sessão", relatorio.generation);
                        }
                        if timings {
                            eprintln!(
                                "[reload] geração {}: emissão {:.1} ms; recarga {:.1} ms (análise {:.1}, contrato {:.1}, módulo {:.1}, \
                                 trampolins {:.1}, ligação {:.1}, publicação {:.1}, espera do ponto seguro {:.1}); gerações retidas {}",
                                relatorio.generation,
                                ms(emissao),
                                ms(relatorio.total),
                                ms(relatorio.parse_ir),
                                ms(relatorio.contract),
                                ms(relatorio.add_module),
                                ms(relatorio.stubs),
                                ms(relatorio.link),
                                ms(relatorio.publish),
                                ms(relatorio.safepoint_wait),
                                relatorio.retained_generations
                            );
                        }
                        if !vivo.executando() {
                            // O programa tinha terminado: roda de novo, sobre
                            // o mesmo estado.
                            vivo.executar_main()?;
                        }
                    }
                    Err(erro) if matches!(erro.stage, "contract" | "poisoned") => {
                        eprintln!("[reload] a edição não pode ser aplicada ao programa em execução: {erro}");
                        pedir_reinicio();
                    }
                    Err(erro) => {
                        eprintln!("[reload] a recarga falhou: {erro}");
                        eprintln!("[reload] a versão em execução continua como estava");
                    }
                }
            }
            std::thread::sleep(intervalo);
        }
    })?;
    let codigo = resultado?;
    use std::io::Write;
    let _ = std::io::stdout().flush();
    if codigo != 0 {
        std::process::exit(codigo);
    }
    Ok(())
}
