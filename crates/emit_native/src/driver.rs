//! Driver de compilação LLVM IR -> Clang -> Link com o runtime em cache.

use crate::alvo::Sistema;
use crate::cache::{RuntimeCache, native_cache_dir};
use crate::object_cache::{self, ObjectCache};
use crate::gerador::{Generation, Gerador};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NativeDriverOptions {
    pub clang: PathBuf,
    pub optimize: bool,
    pub timings: bool,
    /// J05: o objeto leva as tabelas de linha; no Windows a ligação gera o
    /// PDB (`/DEBUG`), que é onde o depurador as procura.
    pub debug_info: bool,
    /// A CPU-alvo (`--cpu`, `gerador::Cpu`); `None`, a base do alvo.
    pub cpu: Option<crate::gerador::Cpu>,
}

impl Default for NativeDriverOptions {
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG").map_or_else(default_clang, PathBuf::from),
            optimize: false,
            timings: false,
            debug_info: false,
            cpu: None,
        }
    }
}

/// O Clang quando `DARTFORGE_CLANG` não está definido: o da distribuição
/// (`lib/llvm/bin`), o `bin` de `DARTFORGE_LLVM_DIR`/`LLVM_SYS_221_PREFIX`,
/// e por fim o `clang` do `PATH` (`scripts/env.ps1` aponta o da máquina de
/// desenvolvimento no Windows).
fn default_clang() -> PathBuf {
    if let Some(c) = dartforge_elements::distribuicao::ferramenta_llvm("clang") {
        return c;
    }
    for var in ["DARTFORGE_LLVM_DIR", "LLVM_SYS_221_PREFIX"] {
        if let Some(d) = std::env::var_os(var) {
            let c = PathBuf::from(d).join("bin").join(crate::alvo::nome_clang());
            if c.is_file() {
                return c;
            }
        }
    }
    PathBuf::from(crate::alvo::nome_clang())
}

#[derive(Debug, Clone, Default)]
pub struct TimingsReport {
    pub frontend: Duration,
    pub hir: Duration,
    pub llvm_ir: Duration,
    pub clang: Duration,
    pub link: Duration,
    pub total: Duration,
    pub peak_memory_bytes: usize,
}

/// Tempos do Clang e da ligação, e se o objeto veio do cache (aí `clang` é
/// só o tempo de achar a entrada).
#[derive(Debug, Clone, Copy, Default)]
pub struct LinkTimes {
    pub clang: Duration,
    pub link: Duration,
    pub objeto_do_cache: bool,
}

pub fn compile_and_link(
    llvm_ir: &str,
    output: &Path,
    options: &NativeDriverOptions,
) -> Result<LinkTimes, String> {
    let runtime = RuntimeCache::get_or_compile()?;
    // Programa com o SDK da fonte (P5c): a entrada chama o registro das
    // bibliotecas do SDK, que moram nos objetos em cache.
    let sdk = if llvm_ir.contains("declare void @df.registrar.") {
        let dir = crate::sdk_do_dart()?;
        let profile = if options.optimize {
            crate::sdk_modulo::SdkProfile::Production
        } else {
            crate::sdk_modulo::SdkProfile::Development
        };
        let sdk = crate::sdk_modulo::compiled_sdk_in_profile(&dir, &options.clang, profile)?;
        if let Some(t) = sdk.cold
            && options.timings
        {
            eprintln!("  SDK frio:  {t:?} (compilado uma vez por conteúdo)");
        }
        Some(sdk)
    } else {
        None
    };
    // Com o SDK da fonte há dois perfis (docs/NATIVO-PLANO.md §7.9):
    // * desenvolvimento e teste: o runtime e o SDK moram na DLL em cache, e o
    //   executável liga só o objeto do programa e a biblioteca de importação
    //   (ligação rápida; a DLL vai ao lado do executável);
    // * produção (`optimize`): UM executável autocontido — o objeto do
    //   programa, os objetos do SDK (em cache, compilados com
    //   `-ffunction-sections`) e o runtime estático, com `/OPT:REF` tirando o
    //   que o programa não alcança.
    let production = options.optimize && sdk.is_some();
    // Produção: as tabelas de métodos montadas no módulo do programa, só com
    // os pares vivos (docs/NATIVO-PODA-DE-TABELAS.md). O objeto em cache é
    // o deste IR já montado.
    let assembled;
    let llvm_ir = match &sdk {
        Some(s) if production => {
            assembled = crate::pruning::assemble_production(llvm_ir, &s.summaries, options.timings)?;
            assembled.as_str()
        }
        _ => llvm_ir,
    };
    let (link_with, sdk_objects): (Linkage, Vec<PathBuf>) = match &sdk {
        Some(s) if production => (Linkage::Production(crate::cache::RuntimeCache::para_producao()?.lib_path), s.objects.clone()),
        Some(s) => (Linkage::SharedSdk(s.import.clone()), Vec::new()),
        None => (Linkage::Runtime(runtime.lib_path.clone()), Vec::new()),
    };

    // Diretório temporário seguro no target. Com o cache de objeto ele só é
    // usado com DARTFORGE_KEEP_IR ou se a ligação recusar o objeto do cache,
    // e só é criado nesses casos.
    let staging = output.parent().unwrap_or(Path::new(".")).join(".df_tmp");
    let create_staging = || {
        std::fs::create_dir_all(&staging)
            .map_err(|e| format!("não foi possível criar diretório temporário {}: {e}", staging.display()))
    };

    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ll_file = staging.join(format!("{stem}.ll"));
    let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    // Produção com o SDK da fonte: bitcode, otimizado junto com o do SDK na
    // ligação (LTO).
    let gerador = Gerador::escolher(&options.clang);
    // `DARTFORGE_PRODUCAO_SEM_LTO=1` (medida, docs/NATIVO-PRODUCAO-GRANDE.md
    // §3.2): as partes do programa vão como objetos `-O2`, e só o bitcode do
    // SDK passa pela LTO do ligador.
    let without_lto = production && std::env::var("DARTFORGE_PRODUCAO_SEM_LTO").is_ok_and(|v| v == "1");
    let generation = Generation { cpu: options.cpu, ..Generation::of_program(options.optimize, production && !without_lto) };
    if keep_ir {
        // A cópia em `.df_tmp` só existe para quem pediu DARTFORGE_KEEP_IR
        // (o `determinismo --run` do harness lê essas cópias).
        create_staging()?;
        std::fs::write(&ll_file, llvm_ir)
            .map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll_file.display()))?;
    }

    // Fase 1': o módulo grande vai em partes (C9, `partition.rs`): cada uma
    // um objeto, com a sua chave no cache, compiladas poucas por vez.
    let t_clang = Instant::now();
    let cache = ObjectCache::from_env();
    // Também na produção: cada parte vira um bitcode, e o ThinLTO do
    // ligador as otimiza juntas (o IR já montado com as tabelas podadas).
    if let Some((threshold, alvo)) = crate::partition::limits()
        && llvm_ir.len() > threshold
    {
        let plan = crate::partition::make_plan(llvm_ir, alvo);
        // Produção em partes: o bitcode tem de levar o resumo do ThinLTO,
        // que só o Clang escreve (o gerador embutido não: o `lld` faria a
        // LTO completa num módulo só, numa thread — 1 600 s no
        // new_sali/backend, docs/NATIVO-PRODUCAO-GRANDE.md §1.2).
        // `DARTFORGE_GERADOR` escolhe à mão.
        let parts_generator = if production && !without_lto && std::env::var_os("DARTFORGE_GERADOR").is_none() {
            Gerador::clang_if_any(&options.clang)
        } else {
            gerador.clone()
        };
        let objects = generate_parts(&plan, &parts_generator, generation, cache, &staging, &stem)?;
        drop(plan);
        let clang_duration = t_clang.elapsed();
        let t_link = Instant::now();
        let mut extras: Vec<PathBuf> = objects[1..].to_vec();
        extras.extend(sdk_objects.iter().cloned());
        LINK_IN_PARTS.set(true);
        // Raízes por mapas na produção em partes: a LTO sai do ligador (o
        // `lld-link` não roda o passe dos mapas) e vira o ThinLTO
        // distribuído, que entrega objetos nativos com o mapa compacto
        // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Etapa 3).
        let distributed = cfg!(feature = "llvm-embutido")
            && production
            && !without_lto
            && matches!(crate::alvo::sistema(), Sistema::Windows | Sistema::Linux)
            && crate::alvo::raizes_por_mapas()?;
        let linked_done = if distributed {
            link_distributed(&options.clang, &objects, &sdk_objects, &link_with, output, options.debug_info, options.cpu, &staging, cache)
        } else {
            link(&options.clang, &objects[0], &extras, &link_with, output, options.debug_info, options.cpu)
        };
        LINK_IN_PARTS.set(false);
        linked_done?;
        if let Some(s) = sdk.as_ref().filter(|_| !production) {
            let dest = output.parent().unwrap_or(Path::new(".")).join(s.dll.file_name().unwrap_or_default());
            if !dest.is_file() && std::fs::hard_link(&s.dll, &dest).is_err() {
                std::fs::copy(&s.dll, &dest).map_err(|e| format!("não foi possível pôr a DLL do SDK em {}: {e}", dest.display()))?;
            }
        }
        if options.timings {
            eprintln!("  Partes:    {} objetos", objects.len());
        }
        if cache.is_none() {
            for o in &objects {
                let _ = std::fs::remove_file(o);
            }
        }
        return Ok(LinkTimes { clang: clang_duration, link: t_link.elapsed(), objeto_do_cache: false });
    }

    // Raízes por mapas na produção de um módulo só, pelo ThinLTO distribuído,
    // que roda o passe dos mapas depois da otimização da ligação e entrega
    // objetos com o mapa compacto (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §3.4): no Windows, porque a LTO do `lld-link` não roda o passe; no
    // Linux, também porque o mapa do LLVM tem relocações absolutas numa seção
    // só de leitura, que o `ld.lld` recusa no executável PIE (Etapa 4).
    // `DARTFORGE_LTO_DISTRIBUIDA=1` (medida, §8): o mesmo caminho com a
    // pilha-sombra, para comparar os modos de raízes no mesmo pipeline.
    let single_distributed = cfg!(feature = "llvm-embutido")
        && production
        && !without_lto
        && matches!(crate::alvo::sistema(), Sistema::Windows | Sistema::Linux)
        && (crate::alvo::raizes_por_mapas()? || std::env::var("DARTFORGE_LTO_DISTRIBUIDA").is_ok_and(|v| v == "1"));
    // O bitcode que vai ao ThinLTO distribuído tem de levar o resumo do
    // ThinLTO, que só o Clang escreve: sem ele, o backend com o índice da
    // ligação devolve o módulo vazio. `DARTFORGE_GERADOR` escolhe à mão.
    let gerador = if single_distributed && std::env::var_os("DARTFORGE_GERADOR").is_none() {
        Gerador::Clang(options.clang.clone())
    } else {
        gerador
    };

    // Fase 1: o gerador compila LLVM IR -> objeto, ou o cache já tem o
    // objeto deste IR com este gerador e esta geração.
    let obj_staging = || -> Result<PathBuf, String> {
        create_staging()?;
        Ok(staging.join(format!("{stem}.{}", crate::alvo::ext_object())))
    };
    let (mut obj_file, mut from_cache) = match cache {
        Some(c) => {
            let description = generation.description();
            let key = object_cache::key(llvm_ir, &gerador.identity()?, &[description.as_str()]);
            let (obj, hit) = c.get_or_create(key, |tmp| gerador.gerar(llvm_ir, generation, tmp))?;
            (obj, Some((c, key, hit)))
        }
        None => {
            let obj = obj_staging()?;
            gerador.gerar(llvm_ir, generation, &obj)?;
            (obj, None)
        }
    };
    let clang_duration = t_clang.elapsed();

    // Fase 2: Link do objeto com o runtime estático
    let t_link = Instant::now();
    let link_object = |obj: &Path| {
        if single_distributed {
            link_distributed(&options.clang, std::slice::from_ref(&obj.to_path_buf()), &sdk_objects, &link_with, output, options.debug_info, options.cpu, &staging, cache)
        } else {
            link(&options.clang, obj, &sdk_objects, &link_with, output, options.debug_info, options.cpu)
        }
    };
    let mut linked_done = link_object(&obj_file);
    if linked_done.is_err()
        && let Some((c, key, true)) = from_cache
    {
        // Um objeto do cache que o ligador recusa não pode ficar lá: sai do
        // cache, e a ligação é refeita uma vez com um objeto novo.
        c.remove(key);
        obj_file = obj_staging()?;
        gerador.gerar(llvm_ir, generation, &obj_file)?;
        from_cache = None;
        linked_done = link_object(&obj_file);
    }
    linked_done?;
    if let Some(s) = sdk.as_ref().filter(|_| !production) {
        // A biblioteca compartilhada ao lado do executável (o Windows procura
        // primeiro ali; no Linux e no macOS o executável leva o `rpath` do
        // próprio diretório): ligação física, sem cópia; cópia só se o volume
        // for outro.
        let dest = output.parent().unwrap_or(Path::new(".")).join(s.dll.file_name().unwrap_or_default());
        if !dest.is_file() && std::fs::hard_link(&s.dll, &dest).is_err() {
            std::fs::copy(&s.dll, &dest).map_err(|e| format!("não foi possível pôr a DLL do SDK em {}: {e}", dest.display()))?;
        }
    }
    let link_duration = t_link.elapsed();

    // Limpeza de arquivos temporários (mantém se DARTFORGE_KEEP_IR estiver
    // definido); um objeto do cache nunca é apagado aqui.
    for p in files_to_remove(&ll_file, &obj_file, &native_cache_dir(), keep_ir) {
        let _ = std::fs::remove_file(p);
    }

    Ok(LinkTimes {
        clang: clang_duration,
        link: link_duration,
        objeto_do_cache: matches!(from_cache, Some((_, _, true))),
    })
}

/// A ligação da produção em partes com raízes por mapas: o ThinLTO
/// distribuído (`distributed_lto.rs`) troca cada bitcode pelo objeto nativo
/// dele, e o ligador recebe só objetos, sem LTO. Os índices pedem a ligação
/// inteira, então a biblioteca do runtime entra na lista e sai dela antes
/// da ligação final (que a acrescenta de novo).
#[cfg(feature = "llvm-embutido")]
#[allow(clippy::too_many_arguments)]
fn link_distributed(
    clang: &Path,
    parts: &[PathBuf],
    sdk: &[PathBuf],
    linkage: &Linkage,
    output: &Path,
    debug_info: bool,
    cpu: Option<crate::gerador::Cpu>,
    staging: &Path,
    cache: Option<&'static ObjectCache>,
) -> Result<(), String> {
    let mut entries: Vec<PathBuf> = parts.to_vec();
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let mut natives = crate::distributed_lto::objects(&crate::distributed_lto::Request { clang, entries: &entries, cpu, staging, cache })?;
    natives.pop();
    let Some((first_one, rest)) = natives.split_first() else {
        return Err("a LTO distribuída não devolveu nenhum objeto".to_string());
    };
    NO_LTO_AT_LINK.set(true);
    let r = link(clang, first_one, rest, linkage, output, debug_info, cpu);
    NO_LTO_AT_LINK.set(false);
    r
}

#[cfg(not(feature = "llvm-embutido"))]
#[allow(clippy::too_many_arguments)]
fn link_distributed(
    _clang: &Path,
    _parts: &[PathBuf],
    _sdk: &[PathBuf],
    _link: &Linkage,
    _output: &Path,
    _debug: bool,
    _cpu: Option<crate::gerador::Cpu>,
    _staging: &Path,
    _cache: Option<&'static ObjectCache>,
) -> Result<(), String> {
    Err("a LTO distribuída exige o gerador embutido do dartforge".to_string())
}

thread_local! {
    /// A ligação em curso nesta thread é a do programa em partes (ThinLTO).
    static LINK_IN_PARTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A ligação em curso recebe objetos nativos já otimizados (o ThinLTO
    /// distribuído, `distributed_lto.rs`): o ligador não faz LTO.
    static NO_LTO_AT_LINK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// O nível de otimização da LTO da produção (`DARTFORGE_LTO_NIVEL`, 0–3):
/// `-O1` no programa em partes (ThinLTO; docs/NATIVO-PRODUCAO-GRANDE.md
/// §6.2: a otimização cai de 275 para 150 s de CPU no new_sali/backend, o
/// `bench/performance` não muda), `-O2` no resto.
pub fn lto_level() -> u8 {
    let pattern = if LINK_IN_PARTS.get() { 1 } else { 2 };
    std::env::var("DARTFORGE_LTO_NIVEL").ok().and_then(|v| v.trim().parse::<u8>().ok()).filter(|n| *n <= 3).unwrap_or(pattern)
}

/// Se a LTO desta ligação roda o *machine outliner* do LLVM em todas as
/// funções (`-enable-machine-outliner=always`): o programa grande (em
/// partes) no modo padrão — exceções por checagem, pilha-sombra, memória
/// por rastreamento, sem o rastro simbólico — no aarch64. O código Dart
/// repete muito as mesmas sequências (a moldura da pilha-sombra, a checagem
/// da exceção pendente, a montagem dos argumentos de um seletor) e o
/// outliner as troca por uma chamada: o `new_sali/backend` cai de 53,1 para
/// 38,5 MB (`__text` 44,6 → 30,0 MB). Fica de fora onde o endereço de
/// retorno de uma chamada importa: as exceções por tabelas e as raízes por
/// mapas desenrolam os quadros, e o rastro simbólico acha a posição pelo
/// rótulo antes da chamada (`llvm/stack_trace.rs`); uma chamada levada para a
/// função extraída teria o retorno fora da função dela. O programa pequeno
/// não paga o salto a mais (a LTO dele já é um módulo só, e o tamanho já
/// fica abaixo do `dart compile exe`). `DARTFORGE_OUTLINER=0` desliga;
/// `=1` liga também no programa pequeno (medida).
pub fn outliner_at_link() -> bool {
    let request = std::env::var("DARTFORGE_OUTLINER").ok();
    if request.as_deref() == Some("0") {
        return false;
    }
    let default_mode = !crate::alvo::excecoes_por_tabelas().unwrap_or(true)
        && !crate::alvo::raizes_por_mapas().unwrap_or(true)
        && !crate::alvo::symbolic_stack_trace().unwrap_or(true)
        && !crate::alvo::arc_memory().unwrap_or(true);
    cfg!(target_arch = "aarch64") && default_mode && (LINK_IN_PARTS.get() || request.as_deref() == Some("1"))
}

/// O diretório do cache do ThinLTO da ligação de produção (ao lado do cache
/// de objetos); `None` com `DARTFORGE_SEM_CACHE_THINLTO=1` ou sem cache de
/// objetos.
pub fn thinlto_cache() -> Option<PathBuf> {
    if std::env::var("DARTFORGE_SEM_CACHE_THINLTO").is_ok_and(|v| v == "1") {
        return None;
    }
    ObjectCache::from_env()?;
    let d = native_cache_dir().join("thinlto");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Quantas gerações de código (partes, ou módulos do ThinLTO no ligador) ao
/// mesmo tempo: `DARTFORGE_PARTES_PARALELAS`, senão metade das threads da
/// máquina, no máximo 3.
pub fn generation_tasks() -> usize {
    std::env::var("DARTFORGE_PARTES_PARALELAS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).clamp(1, 3)))
        .max(1)
}

/// Os objetos das partes de um módulo grande (C9), na ordem: cada parte
/// montada só quando é a vez dela, pelo cache de objetos (ou no `staging`),
/// `DARTFORGE_PARTES_PARALELAS` por vez (padrão: metade das threads da
/// máquina, no máximo 3 — a memória de um gerador, ~44 bytes por byte de
/// IR no `-O0`, é a medida, não os núcleos).
fn generate_parts(
    plan: &crate::partition::Plan<'_>,
    gerador: &Gerador,
    generation: Generation,
    cache: Option<&'static ObjectCache>,
    staging: &Path,
    stem: &str,
) -> Result<Vec<PathBuf>, String> {
    let parallel = generation_tasks();
    let identity = gerador.identity()?;
    let description = generation.description();
    if cache.is_none() {
        std::fs::create_dir_all(staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let next_one = std::sync::atomic::AtomicUsize::new(0);
    let n = plan.len();
    let results: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> =
        std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..parallel.min(n) {
            s.spawn(|| loop {
                let i = next_one.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let text = plan.part(i);
                let ir = text.as_str();
                let r = match cache {
                    Some(c) => {
                        let key = object_cache::key(ir, &identity, &[description.as_str()]);
                        c.get_or_create(key, |tmp| gerador.gerar(ir, generation, tmp)).map(|(o, _)| o)
                    }
                    None => {
                        let o = staging.join(format!("{stem}.parte{i}.{}", crate::alvo::ext_object()));
                        gerador.gerar(ir, generation, &o).map(|()| o)
                    }
                };
                results.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("a parte {i} não foi gerada"))))
        .collect()
}

/// Com o que o objeto do programa é ligado.
enum Linkage {
    /// Sem o SDK da fonte: o runtime estático (com o `main` C).
    Runtime(PathBuf),
    /// Desenvolvimento com o SDK da fonte: a biblioteca compartilhada (runtime
    /// e SDK) — no Windows, a biblioteca de importação da DLL; nos outros, o
    /// próprio `.only`/`.dylib`.
    SharedSdk(PathBuf),
    /// Produção com o SDK da fonte: o runtime estático sem o `main` C; os
    /// objetos do SDK vêm à parte.
    Production(PathBuf),
}

impl Linkage {
    fn library(&self) -> &Path {
        match self {
            Linkage::Runtime(p) | Linkage::SharedSdk(p) | Linkage::Production(p) => p,
        }
    }
}

/// A ligação no Linux: o `ld.lld` direto, com o sysroot de ligação
/// (`ligador.rs`) — sem o driver do Clang nem o GCC na máquina.
fn link_on_linux(clang: &Path, obj: &Path, sdk: &[PathBuf], linkage: &Linkage, output: &Path, debug_info: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador;
    let sysroot = ligador::SysrootLinux::locate(clang)?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    ligador::link(
        &ligador::ld_lld(clang),
        sysroot,
        &ligador::Linkage {
            product: ligador::Product::Executable,
            entries,
            origin_rpath: matches!(linkage, Linkage::SharedSdk(_)),
            lto: production,
            cpu: cpu.map(crate::gerador::Cpu::name),
            prune: production,
            keep_debug_info: debug_info,
            exit: output,
        },
    )
}

/// A ligação no Windows: o `lld-link` direto, com as bibliotecas de
/// importação e a CRT mínima geradas pelo dartforge (`ligador_windows.rs`) —
/// sem o Visual Studio nem o Windows SDK na máquina (N15).
fn link_on_windows(clang: &Path, obj: &Path, sdk: &[PathBuf], linkage: &Linkage, output: &Path, debug_info: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador_windows as lw;
    let sysroot = lw::SysrootWindows::locate(clang)?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    lw::link(
        &lw::lld_link(clang),
        sysroot,
        &lw::Linkage {
            product: lw::Product::Executable,
            entries,
            lto: production && !NO_LTO_AT_LINK.get(),
            cpu: cpu.map(crate::gerador::Cpu::name),
            prune: production,
            debug_info,
            exit: output,
        },
    )
}

/// A ligação no macOS: o `ld64.lld` direto, com os `.tbd` do sysroot de
/// ligação (`ligador_macos.rs`) — sem o `xcrun`, o `ld` da Apple nem as
/// Command Line Tools na máquina (N16).
fn link_on_macos(clang: &Path, obj: &Path, sdk: &[PathBuf], linkage: &Linkage, output: &Path, debug_info: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador_macos as lm;
    let sysroot = lm::SysrootMacos::locate()?;
    let mut entries = vec![obj.to_path_buf()];
    entries.extend(sdk.iter().cloned());
    entries.push(linkage.library().to_path_buf());
    let production = matches!(linkage, Linkage::Production(_));
    lm::link(
        &lm::ld64_lld(clang),
        sysroot,
        &lm::Linkage {
            product: lm::Product::Executable,
            entries,
            executable_rpath: matches!(linkage, Linkage::SharedSdk(_)),
            lto: production,
            cpu: cpu.map(crate::gerador::Cpu::name),
            prune: production,
            keep_debug_info: debug_info,
            exit: output,
        },
    )
}

/// Liga o objeto do programa: o ligador do LLVM direto em cada sistema, com
/// o que o sistema exige vindo do dartforge (nenhum driver de C, nenhum
/// toolchain do sistema). O `lld` é o do mesmo LLVM do gerador — o bitcode
/// da produção (LTO) só é lido pela mesma versão.
fn link(clang: &Path, obj: &Path, sdk: &[PathBuf], linkage: &Linkage, output: &Path, debug_info: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    match crate::alvo::sistema() {
        Sistema::Linux => link_on_linux(clang, obj, sdk, linkage, output, debug_info, cpu),
        // A ligação de antes do N15 (o driver do Clang com o `link.exe` e a
        // CRT do Visual C++), só para comparação: é a referência do
        // `without-toolchain.yml`, na mesma build.
        Sistema::Windows if link_with_clang() => link_old_with_clang(clang, obj, sdk, linkage, output, debug_info, cpu),
        Sistema::Windows => link_on_windows(clang, obj, sdk, linkage, output, debug_info, cpu),
        Sistema::MacOs => link_on_macos(clang, obj, sdk, linkage, output, debug_info, cpu),
    }
}

/// Se a ligação no Windows é a de antes do N15 (`DARTFORGE_LIGAR_COM_CLANG`
/// definida): o executável e a DLL do SDK da fonte.
pub fn link_with_clang() -> bool {
    cfg!(windows) && std::env::var_os("DARTFORGE_LIGAR_COM_CLANG").is_some()
}

/// A ligação pelo driver do Clang, como era antes do N15 (o toolchain do
/// Visual Studio na máquina): `DARTFORGE_LIGAR_COM_CLANG=1` no Windows. Só
/// para comparar com a ligação sem o toolchain do sistema.
fn link_old_with_clang(clang: &Path, obj: &Path, sdk: &[PathBuf], linkage: &Linkage, output: &Path, debug_info: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    if crate::alvo::sistema() == Sistema::Linux {
        return link_on_linux(clang, obj, sdk, linkage, output, debug_info, cpu);
    }
    let mut cmd = std::process::Command::new(clang);
    cmd.arg(obj).args(sdk).arg(linkage.library());
    let sistema = crate::alvo::sistema();
    match linkage {
        Linkage::SharedSdk(_) if sistema == Sistema::Windows => {
            // Executável do SDK da fonte (desenvolvimento): o runtime está na
            // DLL, que usa a CRT dinâmica (a do `rustc`); o executável usa a
            // mesma.
            cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt"]);
        }
        Linkage::SharedSdk(_) => {
            // A biblioteca compartilhada vai ao lado do executável; o `rpath`
            // aponta o carregador para o diretório do próprio executável.
            cmd.arg(if sistema == Sistema::MacOs { "-Wl,-rpath,@executable_path" } else { "-Wl,-rpath,$ORIGIN" });
            // O Clang acrescenta `-lSystem` no macOS: sem a raiz do SDK
            // (`-isysroot`), o ligador não o acha.
            cmd.args(crate::alvo::argumentos_de_ligacao());
        }
        Linkage::Production(_) => {
            // Produção com o SDK da fonte: tudo estático no executável,
            // ThinLTO entre o programa e o SDK (lld), e o ligador tira as
            // seções que nada alcança.
            // O lld tem de ser o do mesmo LLVM do Clang (o bitcode ThinLTO só
            // é lido pela mesma versão). O Clang com ThinLTO exige o literal
            // `lld` em `-fuse-ld=` (caminho absoluto dá `clang: error: LTO
            // requires -fuse-ld=lld` no Windows); então o diretório bin irmão
            // do próprio Clang vai ao PATH só deste spawn, para o `lld`
            // resolvido ser o da mesma versão. Recusar sua ausência evita cair
            // num lld errado do PATH em silêncio (medido: LLVM 20 lendo
            // bitcode 22 — `Unknown attribute kind (105)`).
            let lld_name = match sistema {
                Sistema::Windows => "lld-link.exe",
                Sistema::Linux => "ld.lld",
                Sistema::MacOs => "ld64.lld",
            };
            let lld = clang.with_file_name(lld_name);
            if !lld.is_file() {
                return Err(format!("ThinLTO requer {lld_name} ao lado de {}", clang.display()));
            }
            if let Some(bin) = clang.parent()
                && !bin.as_os_str().is_empty()
            {
                let mut paths = vec![bin.to_path_buf()];
                if let Some(current) = std::env::var_os("PATH") {
                    paths.extend(std::env::split_paths(&current));
                }
                if let Ok(new) = std::env::join_paths(paths) {
                    cmd.env("PATH", new);
                }
            }
            cmd.args(["-fuse-ld=lld", "-flto=thin"]);
            cmd.arg(format!("-O{}", lto_level()));
            // O cache do ThinLTO: numa religação, só os módulos que mudaram
            // (ou cujas importações mudaram) passam de novo pela otimização
            // e pela geração de código (docs/NATIVO-PRODUCAO-GRANDE.md).
            if let Some(d) = thinlto_cache() {
                match sistema {
                    Sistema::Windows => {
                        cmd.arg(format!("-Wl,/lldltocache:{}", d.display()));
                        cmd.arg("-Wl,/lldltocachepolicy:cache_size_bytes=4g:prune_after=168h");
                    }
                    Sistema::Linux => {
                        cmd.arg(format!("-Wl,--thinlto-cache-dir={}", d.display()));
                    }
                    Sistema::MacOs => {
                        cmd.arg(format!("-Wl,-cache_path_lto,{}", d.display()));
                    }
                }
            }
            // A CPU-alvo da geração de código da LTO (`--cpu`).
            if let Some(c) = cpu {
                cmd.arg(format!("-march={}", c.name()));
            }
            cmd.args(crate::alvo::argumentos_de_ligacao());
            // O bitcode do gerador embutido não tem o resumo do ThinLTO: o
            // `lld` faz a LTO completa, e a geração de código dela divide-se
            // em partições paralelas (o Mach-O não tem a opção).
            let partitions = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
            match sistema {
                Sistema::Windows => {
                    // `/OPT:REF` liga também o `/OPT:ICF` do `lld-link`, que
                    // funde funções de corpo idêntico num endereço só; o
                    // `/OPT:NOICF` vai logo abaixo, para todo perfil
                    // (`left == right` daria `true`, corpus/js 147 e 184).
                    cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt", "-Wl,/OPT:REF"]);
                    cmd.arg(format!("-Wl,/opt:lldltopartitions={partitions}"));
                }
                // Sem a tabela de símbolos, como o `.exe` do Windows (que a
                // deixa no PDB): metade do tamanho no ELF (medido: 9,4 → 4,4 MB).
                Sistema::Linux => {
                    cmd.arg("-Wl,--gc-sections");
                    if !crate::ligador::keep_symbols() && !debug_info {
                        cmd.arg("-Wl,--strip-all");
                    }
                    cmd.arg(format!("-Wl,--lto-partitions={partitions}"));
                }
                Sistema::MacOs => {
                    cmd.arg("-Wl,-dead_strip");
                    // `-S` tira o mapa de depuração (J05).
                    if !debug_info {
                        cmd.args(["-Wl,-S", "-Wl,-x"]);
                    }
                }
            }
        }
        Linkage::Runtime(_) => {
            cmd.args(crate::alvo::argumentos_de_ligacao());
        }
    }
    if sistema == Sistema::Windows && debug_info {
        // O `-g` na ligação vira o `/DEBUG` do ligador: o PDB ao lado do
        // executável, com as tabelas CodeView do objeto.
        cmd.arg("-g");
    }
    if sistema == Sistema::Windows {
        // Sem ICF em nenhum perfil: o `link.exe` sem `/DEBUG` (e o `lld-link`
        // com `/OPT:REF`) funde funções de corpo idêntico, e o tear-off de
        // função de topo se compara pelo endereço.
        cmd.arg("-Wl,/OPT:NOICF");
    }
    let exit = cmd
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("falha na ligação com Clang em {clang:?}: {e}"))?;
    if !exit.status.success() {
        // O que o ligador disse vai no erro (sem isso a falha no CI não
        // tem diagnóstico).
        let text = String::from_utf8_lossy(&exit.stderr);
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("Clang falhou na ligação do executável ({}):\n{}", exit.status, lines.join("\n")));
    }
    Ok(())
}

/// O que a limpeza depois da ligação apaga: nada com `keep_ir`, e nunca um
/// caminho dentro do cache — o objeto que veio de lá é de todos.
fn files_to_remove(ll: &Path, obj: &Path, dir_cache: &Path, keep_ir: bool) -> Vec<PathBuf> {
    if keep_ir {
        return Vec::new();
    }
    [ll, obj].into_iter().filter(|p| !p.starts_with(dir_cache)).map(Path::to_path_buf).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_never_erases_cache() {
        let cache = Path::new("D:/x/target/native_cache");
        let ll = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.ll");
        let obj_cache = cache.join("obj/ab/ab00.obj");
        let obj_local = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.obj");
        assert_eq!(files_to_remove(ll, &obj_cache, cache, false), vec![ll.to_path_buf()]);
        assert_eq!(files_to_remove(ll, obj_local, cache, false), vec![ll.to_path_buf(), obj_local.to_path_buf()]);
        assert!(files_to_remove(ll, obj_local, cache, true).is_empty());
    }
}
