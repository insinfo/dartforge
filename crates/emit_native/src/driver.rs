//! Driver de compilação LLVM IR -> Clang -> Link com o runtime em cache.

use crate::alvo::Sistema;
use crate::cache::{RuntimeCache, dir_cache_nativo};
use crate::cache_objeto::{self, CacheObjeto};
use crate::gerador::{Geracao, Gerador};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct NativeDriverOptions {
    pub clang: PathBuf,
    pub optimize: bool,
    pub timings: bool,
    /// J05: o objeto leva as tabelas de linha; no Windows a ligação gera o
    /// PDB (`/DEBUG`), que é onde o depurador as procura.
    pub depuracao: bool,
    /// A CPU-alvo (`--cpu`, `gerador::Cpu`); `None`, a base do alvo.
    pub cpu: Option<crate::gerador::Cpu>,
}

impl Default for NativeDriverOptions {
    fn default() -> Self {
        Self {
            clang: std::env::var_os("DARTFORGE_CLANG").map_or_else(clang_padrao, PathBuf::from),
            optimize: false,
            timings: false,
            depuracao: false,
            cpu: None,
        }
    }
}

/// O Clang quando `DARTFORGE_CLANG` não está definido: o da distribuição
/// (`lib/llvm/bin`), o `bin` de `DARTFORGE_LLVM_DIR`/`LLVM_SYS_221_PREFIX`,
/// e por fim o `clang` do `PATH` (`scripts/env.ps1` aponta o da máquina de
/// desenvolvimento no Windows).
fn clang_padrao() -> PathBuf {
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
pub struct TemposLigacao {
    pub clang: Duration,
    pub link: Duration,
    pub objeto_do_cache: bool,
}

pub fn compile_and_link(
    llvm_ir: &str,
    output: &Path,
    options: &NativeDriverOptions,
) -> Result<TemposLigacao, String> {
    let runtime = RuntimeCache::get_or_compile()?;
    // Programa com o SDK da fonte (P5c): a entrada chama o registro das
    // bibliotecas do SDK, que moram nos objetos em cache.
    let sdk = if llvm_ir.contains("declare void @df.registrar.") {
        let dir = crate::sdk_do_dart()?;
        let perfil = if options.optimize {
            crate::sdk_modulo::PerfilDoSdk::Producao
        } else {
            crate::sdk_modulo::PerfilDoSdk::Desenvolvimento
        };
        let sdk = crate::sdk_modulo::sdk_compilado_no_perfil(&dir, &options.clang, perfil)?;
        if let Some(t) = sdk.frio
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
    let producao = options.optimize && sdk.is_some();
    // Produção: as tabelas de métodos montadas no módulo do programa, só com
    // os pares vivos (docs/NATIVO-PODA-DE-TABELAS.md). O objeto em cache é
    // o deste IR já montado.
    let montado;
    let llvm_ir = match &sdk {
        Some(s) if producao => {
            montado = crate::poda::montar_producao(llvm_ir, &s.resumos, options.timings)?;
            montado.as_str()
        }
        _ => llvm_ir,
    };
    let (ligar_com, sdk_objetos): (Ligacao, Vec<PathBuf>) = match &sdk {
        Some(s) if producao => (Ligacao::Producao(crate::cache::RuntimeCache::para_producao()?.lib_path), s.objetos.clone()),
        Some(s) => (Ligacao::SdkCompartilhado(s.importacao.clone()), Vec::new()),
        None => (Ligacao::Runtime(runtime.lib_path.clone()), Vec::new()),
    };

    // Diretório temporário seguro no target. Com o cache de objeto ele só é
    // usado com DARTFORGE_KEEP_IR ou se a ligação recusar o objeto do cache,
    // e só é criado nesses casos.
    let staging = output.parent().unwrap_or(Path::new(".")).join(".df_tmp");
    let criar_staging = || {
        std::fs::create_dir_all(&staging)
            .map_err(|e| format!("não foi possível criar diretório temporário {}: {e}", staging.display()))
    };

    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let ll_file = staging.join(format!("{stem}.ll"));
    let manter_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    // Produção com o SDK da fonte: bitcode, otimizado junto com o do SDK na
    // ligação (LTO).
    let gerador = Gerador::escolher(&options.clang);
    // `DARTFORGE_PRODUCAO_SEM_LTO=1` (medida, docs/NATIVO-PRODUCAO-GRANDE.md
    // §3.2): as partes do programa vão como objetos `-O2`, e só o bitcode do
    // SDK passa pela LTO do ligador.
    let sem_lto = producao && std::env::var("DARTFORGE_PRODUCAO_SEM_LTO").is_ok_and(|v| v == "1");
    let geracao = Geracao { cpu: options.cpu, ..Geracao::do_programa(options.optimize, producao && !sem_lto) };
    if manter_ir {
        // A cópia em `.df_tmp` só existe para quem pediu DARTFORGE_KEEP_IR
        // (o `determinismo --executar` do harness lê essas cópias).
        criar_staging()?;
        std::fs::write(&ll_file, llvm_ir)
            .map_err(|e| format!("falha ao escrever LLVM IR em {}: {e}", ll_file.display()))?;
    }

    // Fase 1': o módulo grande vai em partes (C9, `particao.rs`): cada uma
    // um objeto, com a sua chave no cache, compiladas poucas por vez.
    let t_clang = Instant::now();
    let cache = CacheObjeto::do_ambiente();
    // Também na produção: cada parte vira um bitcode, e o ThinLTO do
    // ligador as otimiza juntas (o IR já montado com as tabelas podadas).
    if let Some((limiar, alvo)) = crate::particao::limites()
        && llvm_ir.len() > limiar
    {
        let plano = crate::particao::planejar(llvm_ir, alvo);
        // Produção em partes: o bitcode tem de levar o resumo do ThinLTO,
        // que só o Clang escreve (o gerador embutido não: o `lld` faria a
        // LTO completa num módulo só, numa thread — 1 600 s no
        // new_sali/backend, docs/NATIVO-PRODUCAO-GRANDE.md §1.2).
        // `DARTFORGE_GERADOR` escolhe à mão.
        let gerador_das_partes = if producao && !sem_lto && std::env::var_os("DARTFORGE_GERADOR").is_none() {
            Gerador::Clang(options.clang.clone())
        } else {
            gerador.clone()
        };
        let objetos = gerar_partes(&plano, &gerador_das_partes, geracao, cache, &staging, &stem)?;
        drop(plano);
        let clang_duration = t_clang.elapsed();
        let t_link = Instant::now();
        let mut extras: Vec<PathBuf> = objetos[1..].to_vec();
        extras.extend(sdk_objetos.iter().cloned());
        LIGACAO_EM_PARTES.set(true);
        // Raízes por mapas na produção em partes: a LTO sai do ligador (o
        // `lld-link` não roda o passe dos mapas) e vira o ThinLTO
        // distribuído, que entrega objetos nativos com o mapa compacto
        // (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Etapa 3).
        let distribuida = cfg!(feature = "llvm-embutido")
            && producao
            && !sem_lto
            && matches!(crate::alvo::sistema(), Sistema::Windows | Sistema::Linux)
            && crate::alvo::raizes_por_mapas()?;
        let ligou = if distribuida {
            ligar_distribuida(&options.clang, &objetos, &sdk_objetos, &ligar_com, output, options.depuracao, options.cpu, &staging, cache)
        } else {
            ligar(&options.clang, &objetos[0], &extras, &ligar_com, output, options.depuracao, options.cpu)
        };
        LIGACAO_EM_PARTES.set(false);
        ligou?;
        if let Some(s) = sdk.as_ref().filter(|_| !producao) {
            let destino = output.parent().unwrap_or(Path::new(".")).join(s.dll.file_name().unwrap_or_default());
            if !destino.is_file() && std::fs::hard_link(&s.dll, &destino).is_err() {
                std::fs::copy(&s.dll, &destino).map_err(|e| format!("não foi possível pôr a DLL do SDK em {}: {e}", destino.display()))?;
            }
        }
        if options.timings {
            eprintln!("  Partes:    {} objetos", objetos.len());
        }
        if cache.is_none() {
            for o in &objetos {
                let _ = std::fs::remove_file(o);
            }
        }
        return Ok(TemposLigacao { clang: clang_duration, link: t_link.elapsed(), objeto_do_cache: false });
    }

    // Raízes por mapas na produção de um módulo só, pelo ThinLTO distribuído,
    // que roda o passe dos mapas depois da otimização da ligação e entrega
    // objetos com o mapa compacto (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md
    // §3.4): no Windows, porque a LTO do `lld-link` não roda o passe; no
    // Linux, também porque o mapa do LLVM tem relocações absolutas numa seção
    // só de leitura, que o `ld.lld` recusa no executável PIE (Etapa 4).
    // `DARTFORGE_LTO_DISTRIBUIDA=1` (medida, §8): o mesmo caminho com a
    // pilha-sombra, para comparar os modos de raízes no mesmo pipeline.
    let distribuida_unica = cfg!(feature = "llvm-embutido")
        && producao
        && !sem_lto
        && matches!(crate::alvo::sistema(), Sistema::Windows | Sistema::Linux)
        && (crate::alvo::raizes_por_mapas()? || std::env::var("DARTFORGE_LTO_DISTRIBUIDA").is_ok_and(|v| v == "1"));
    // O bitcode que vai ao ThinLTO distribuído tem de levar o resumo do
    // ThinLTO, que só o Clang escreve: sem ele, o backend com o índice da
    // ligação devolve o módulo vazio. `DARTFORGE_GERADOR` escolhe à mão.
    let gerador = if distribuida_unica && std::env::var_os("DARTFORGE_GERADOR").is_none() {
        Gerador::Clang(options.clang.clone())
    } else {
        gerador
    };

    // Fase 1: o gerador compila LLVM IR -> objeto, ou o cache já tem o
    // objeto deste IR com este gerador e esta geração.
    let obj_staging = || -> Result<PathBuf, String> {
        criar_staging()?;
        Ok(staging.join(format!("{stem}.{}", crate::alvo::ext_objeto())))
    };
    let (mut obj_file, mut do_cache) = match cache {
        Some(c) => {
            let descricao = geracao.descricao();
            let chave = cache_objeto::chave(llvm_ir, &gerador.identidade()?, &[descricao.as_str()]);
            let (obj, acerto) = c.obter_ou_criar(chave, |tmp| gerador.gerar(llvm_ir, geracao, tmp))?;
            (obj, Some((c, chave, acerto)))
        }
        None => {
            let obj = obj_staging()?;
            gerador.gerar(llvm_ir, geracao, &obj)?;
            (obj, None)
        }
    };
    let clang_duration = t_clang.elapsed();

    // Fase 2: Link do objeto com o runtime estático
    let t_link = Instant::now();
    let ligar_objeto = |obj: &Path| {
        if distribuida_unica {
            ligar_distribuida(&options.clang, std::slice::from_ref(&obj.to_path_buf()), &sdk_objetos, &ligar_com, output, options.depuracao, options.cpu, &staging, cache)
        } else {
            ligar(&options.clang, obj, &sdk_objetos, &ligar_com, output, options.depuracao, options.cpu)
        }
    };
    let mut ligou = ligar_objeto(&obj_file);
    if ligou.is_err()
        && let Some((c, chave, true)) = do_cache
    {
        // Um objeto do cache que o ligador recusa não pode ficar lá: sai do
        // cache, e a ligação é refeita uma vez com um objeto novo.
        c.remover(chave);
        obj_file = obj_staging()?;
        gerador.gerar(llvm_ir, geracao, &obj_file)?;
        do_cache = None;
        ligou = ligar_objeto(&obj_file);
    }
    ligou?;
    if let Some(s) = sdk.as_ref().filter(|_| !producao) {
        // A biblioteca compartilhada ao lado do executável (o Windows procura
        // primeiro ali; no Linux e no macOS o executável leva o `rpath` do
        // próprio diretório): ligação física, sem cópia; cópia só se o volume
        // for outro.
        let destino = output.parent().unwrap_or(Path::new(".")).join(s.dll.file_name().unwrap_or_default());
        if !destino.is_file() && std::fs::hard_link(&s.dll, &destino).is_err() {
            std::fs::copy(&s.dll, &destino).map_err(|e| format!("não foi possível pôr a DLL do SDK em {}: {e}", destino.display()))?;
        }
    }
    let link_duration = t_link.elapsed();

    // Limpeza de arquivos temporários (mantém se DARTFORGE_KEEP_IR estiver
    // definido); um objeto do cache nunca é apagado aqui.
    for p in arquivos_a_remover(&ll_file, &obj_file, &dir_cache_nativo(), manter_ir) {
        let _ = std::fs::remove_file(p);
    }

    Ok(TemposLigacao {
        clang: clang_duration,
        link: link_duration,
        objeto_do_cache: matches!(do_cache, Some((_, _, true))),
    })
}

/// A ligação da produção em partes com raízes por mapas: o ThinLTO
/// distribuído (`lto_distribuida.rs`) troca cada bitcode pelo objeto nativo
/// dele, e o ligador recebe só objetos, sem LTO. Os índices pedem a ligação
/// inteira, então a biblioteca do runtime entra na lista e sai dela antes
/// da ligação final (que a acrescenta de novo).
#[cfg(feature = "llvm-embutido")]
#[allow(clippy::too_many_arguments)]
fn ligar_distribuida(
    clang: &Path,
    partes: &[PathBuf],
    sdk: &[PathBuf],
    ligacao: &Ligacao,
    output: &Path,
    depuracao: bool,
    cpu: Option<crate::gerador::Cpu>,
    staging: &Path,
    cache: Option<&'static CacheObjeto>,
) -> Result<(), String> {
    let mut entradas: Vec<PathBuf> = partes.to_vec();
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let mut nativos = crate::lto_distribuida::objetos(&crate::lto_distribuida::Pedido { clang, entradas: &entradas, cpu, staging, cache })?;
    nativos.pop();
    let Some((primeiro, resto)) = nativos.split_first() else {
        return Err("a LTO distribuída não devolveu nenhum objeto".to_string());
    };
    SEM_LTO_NA_LIGACAO.set(true);
    let r = ligar(clang, primeiro, resto, ligacao, output, depuracao, cpu);
    SEM_LTO_NA_LIGACAO.set(false);
    r
}

#[cfg(not(feature = "llvm-embutido"))]
#[allow(clippy::too_many_arguments)]
fn ligar_distribuida(
    _clang: &Path,
    _partes: &[PathBuf],
    _sdk: &[PathBuf],
    _ligacao: &Ligacao,
    _output: &Path,
    _depuracao: bool,
    _cpu: Option<crate::gerador::Cpu>,
    _staging: &Path,
    _cache: Option<&'static CacheObjeto>,
) -> Result<(), String> {
    Err("a LTO distribuída exige o gerador embutido do dartforge".to_string())
}

thread_local! {
    /// A ligação em curso nesta thread é a do programa em partes (ThinLTO).
    static LIGACAO_EM_PARTES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// A ligação em curso recebe objetos nativos já otimizados (o ThinLTO
    /// distribuído, `lto_distribuida.rs`): o ligador não faz LTO.
    static SEM_LTO_NA_LIGACAO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// O nível de otimização da LTO da produção (`DARTFORGE_LTO_NIVEL`, 0–3):
/// `-O1` no programa em partes (ThinLTO; docs/NATIVO-PRODUCAO-GRANDE.md
/// §6.2: a otimização cai de 275 para 150 s de CPU no new_sali/backend, o
/// `bench/desempenho` não muda), `-O2` no resto.
pub fn nivel_da_lto() -> u8 {
    let padrao = if LIGACAO_EM_PARTES.get() { 1 } else { 2 };
    std::env::var("DARTFORGE_LTO_NIVEL").ok().and_then(|v| v.trim().parse::<u8>().ok()).filter(|n| *n <= 3).unwrap_or(padrao)
}

/// O diretório do cache do ThinLTO da ligação de produção (ao lado do cache
/// de objetos); `None` com `DARTFORGE_SEM_CACHE_THINLTO=1` ou sem cache de
/// objetos.
pub fn cache_do_thinlto() -> Option<PathBuf> {
    if std::env::var("DARTFORGE_SEM_CACHE_THINLTO").is_ok_and(|v| v == "1") {
        return None;
    }
    CacheObjeto::do_ambiente()?;
    let d = dir_cache_nativo().join("thinlto");
    std::fs::create_dir_all(&d).ok()?;
    Some(d)
}

/// Quantas gerações de código (partes, ou módulos do ThinLTO no ligador) ao
/// mesmo tempo: `DARTFORGE_PARTES_PARALELAS`, senão metade das threads da
/// máquina, no máximo 3.
pub fn tarefas_de_geracao() -> usize {
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
fn gerar_partes(
    plano: &crate::particao::Plano<'_>,
    gerador: &Gerador,
    geracao: Geracao,
    cache: Option<&'static CacheObjeto>,
    staging: &Path,
    stem: &str,
) -> Result<Vec<PathBuf>, String> {
    let paralelas = tarefas_de_geracao();
    let identidade = gerador.identidade()?;
    let descricao = geracao.descricao();
    if cache.is_none() {
        std::fs::create_dir_all(staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    let proxima = std::sync::atomic::AtomicUsize::new(0);
    let n = plano.len();
    let resultados: std::sync::Mutex<Vec<Option<Result<PathBuf, String>>>> =
        std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..paralelas.min(n) {
            s.spawn(|| loop {
                let i = proxima.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let texto = plano.parte(i);
                let ir = texto.as_str();
                let r = match cache {
                    Some(c) => {
                        let chave = cache_objeto::chave(ir, &identidade, &[descricao.as_str()]);
                        c.obter_ou_criar(chave, |tmp| gerador.gerar(ir, geracao, tmp)).map(|(o, _)| o)
                    }
                    None => {
                        let o = staging.join(format!("{stem}.parte{i}.{}", crate::alvo::ext_objeto()));
                        gerador.gerar(ir, geracao, &o).map(|()| o)
                    }
                };
                resultados.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    resultados
        .into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .enumerate()
        .map(|(i, r)| r.unwrap_or_else(|| Err(format!("a parte {i} não foi gerada"))))
        .collect()
}

/// Com o que o objeto do programa é ligado.
enum Ligacao {
    /// Sem o SDK da fonte: o runtime estático (com o `main` C).
    Runtime(PathBuf),
    /// Desenvolvimento com o SDK da fonte: a biblioteca compartilhada (runtime
    /// e SDK) — no Windows, a biblioteca de importação da DLL; nos outros, o
    /// próprio `.so`/`.dylib`.
    SdkCompartilhado(PathBuf),
    /// Produção com o SDK da fonte: o runtime estático sem o `main` C; os
    /// objetos do SDK vêm à parte.
    Producao(PathBuf),
}

impl Ligacao {
    fn biblioteca(&self) -> &Path {
        match self {
            Ligacao::Runtime(p) | Ligacao::SdkCompartilhado(p) | Ligacao::Producao(p) => p,
        }
    }
}

/// A ligação no Linux: o `ld.lld` direto, com o sysroot de ligação
/// (`ligador.rs`) — sem o driver do Clang nem o GCC na máquina.
fn ligar_no_linux(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador;
    let sysroot = ligador::SysrootLinux::localizar(clang)?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    ligador::ligar(
        &ligador::ld_lld(clang),
        sysroot,
        &ligador::Ligacao {
            produto: ligador::Produto::Executavel,
            entradas,
            rpath_origem: matches!(ligacao, Ligacao::SdkCompartilhado(_)),
            lto: producao,
            cpu: cpu.map(crate::gerador::Cpu::nome),
            podar: producao,
            manter_depuracao: depuracao,
            saida: output,
        },
    )
}

/// A ligação no Windows: o `lld-link` direto, com as bibliotecas de
/// importação e a CRT mínima geradas pelo dartforge (`ligador_windows.rs`) —
/// sem o Visual Studio nem o Windows SDK na máquina (N15).
fn ligar_no_windows(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador_windows as lw;
    let sysroot = lw::SysrootWindows::localizar(clang)?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    lw::ligar(
        &lw::lld_link(clang),
        sysroot,
        &lw::Ligacao {
            produto: lw::Produto::Executavel,
            entradas,
            lto: producao && !SEM_LTO_NA_LIGACAO.get(),
            cpu: cpu.map(crate::gerador::Cpu::nome),
            podar: producao,
            depuracao,
            saida: output,
        },
    )
}

/// A ligação no macOS: o `ld64.lld` direto, com os `.tbd` do sysroot de
/// ligação (`ligador_macos.rs`) — sem o `xcrun`, o `ld` da Apple nem as
/// Command Line Tools na máquina (N16).
fn ligar_no_macos(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    use crate::ligador_macos as lm;
    let sysroot = lm::SysrootMacos::localizar()?;
    let mut entradas = vec![obj.to_path_buf()];
    entradas.extend(sdk.iter().cloned());
    entradas.push(ligacao.biblioteca().to_path_buf());
    let producao = matches!(ligacao, Ligacao::Producao(_));
    lm::ligar(
        &lm::ld64_lld(clang),
        sysroot,
        &lm::Ligacao {
            produto: lm::Produto::Executavel,
            entradas,
            rpath_executavel: matches!(ligacao, Ligacao::SdkCompartilhado(_)),
            lto: producao,
            cpu: cpu.map(crate::gerador::Cpu::nome),
            podar: producao,
            manter_depuracao: depuracao,
            saida: output,
        },
    )
}

/// Liga o objeto do programa: o ligador do LLVM direto em cada sistema, com
/// o que o sistema exige vindo do dartforge (nenhum driver de C, nenhum
/// toolchain do sistema). O `lld` é o do mesmo LLVM do gerador — o bitcode
/// da produção (LTO) só é lido pela mesma versão.
fn ligar(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    match crate::alvo::sistema() {
        Sistema::Linux => ligar_no_linux(clang, obj, sdk, ligacao, output, depuracao, cpu),
        // A ligação de antes do N15 (o driver do Clang com o `link.exe` e a
        // CRT do Visual C++), só para comparação: é a referência do
        // `sem-toolchain.yml`, na mesma build.
        Sistema::Windows if ligar_com_clang() => ligar_antigo_com_clang(clang, obj, sdk, ligacao, output, depuracao, cpu),
        Sistema::Windows => ligar_no_windows(clang, obj, sdk, ligacao, output, depuracao, cpu),
        Sistema::MacOs => ligar_no_macos(clang, obj, sdk, ligacao, output, depuracao, cpu),
    }
}

/// Se a ligação no Windows é a de antes do N15 (`DARTFORGE_LIGAR_COM_CLANG`
/// definida): o executável e a DLL do SDK da fonte.
pub fn ligar_com_clang() -> bool {
    cfg!(windows) && std::env::var_os("DARTFORGE_LIGAR_COM_CLANG").is_some()
}

/// A ligação pelo driver do Clang, como era antes do N15 (o toolchain do
/// Visual Studio na máquina): `DARTFORGE_LIGAR_COM_CLANG=1` no Windows. Só
/// para comparar com a ligação sem o toolchain do sistema.
fn ligar_antigo_com_clang(clang: &Path, obj: &Path, sdk: &[PathBuf], ligacao: &Ligacao, output: &Path, depuracao: bool, cpu: Option<crate::gerador::Cpu>) -> Result<(), String> {
    if crate::alvo::sistema() == Sistema::Linux {
        return ligar_no_linux(clang, obj, sdk, ligacao, output, depuracao, cpu);
    }
    let mut cmd = std::process::Command::new(clang);
    cmd.arg(obj).args(sdk).arg(ligacao.biblioteca());
    let sistema = crate::alvo::sistema();
    match ligacao {
        Ligacao::SdkCompartilhado(_) if sistema == Sistema::Windows => {
            // Executável do SDK da fonte (desenvolvimento): o runtime está na
            // DLL, que usa a CRT dinâmica (a do `rustc`); o executável usa a
            // mesma.
            cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt"]);
        }
        Ligacao::SdkCompartilhado(_) => {
            // A biblioteca compartilhada vai ao lado do executável; o `rpath`
            // aponta o carregador para o diretório do próprio executável.
            cmd.arg(if sistema == Sistema::MacOs { "-Wl,-rpath,@executable_path" } else { "-Wl,-rpath,$ORIGIN" });
            // O Clang acrescenta `-lSystem` no macOS: sem a raiz do SDK
            // (`-isysroot`), o ligador não o acha.
            cmd.args(crate::alvo::argumentos_de_ligacao());
        }
        Ligacao::Producao(_) => {
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
            let nome_lld = match sistema {
                Sistema::Windows => "lld-link.exe",
                Sistema::Linux => "ld.lld",
                Sistema::MacOs => "ld64.lld",
            };
            let lld = clang.with_file_name(nome_lld);
            if !lld.is_file() {
                return Err(format!("ThinLTO requer {nome_lld} ao lado de {}", clang.display()));
            }
            if let Some(bin) = clang.parent()
                && !bin.as_os_str().is_empty()
            {
                let mut caminhos = vec![bin.to_path_buf()];
                if let Some(atual) = std::env::var_os("PATH") {
                    caminhos.extend(std::env::split_paths(&atual));
                }
                if let Ok(novo) = std::env::join_paths(caminhos) {
                    cmd.env("PATH", novo);
                }
            }
            cmd.args(["-fuse-ld=lld", "-flto=thin"]);
            cmd.arg(format!("-O{}", nivel_da_lto()));
            // O cache do ThinLTO: numa religação, só os módulos que mudaram
            // (ou cujas importações mudaram) passam de novo pela otimização
            // e pela geração de código (docs/NATIVO-PRODUCAO-GRANDE.md).
            if let Some(d) = cache_do_thinlto() {
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
                cmd.arg(format!("-march={}", c.nome()));
            }
            cmd.args(crate::alvo::argumentos_de_ligacao());
            // O bitcode do gerador embutido não tem o resumo do ThinLTO: o
            // `lld` faz a LTO completa, e a geração de código dela divide-se
            // em partições paralelas (o Mach-O não tem a opção).
            let particoes = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 16);
            match sistema {
                Sistema::Windows => {
                    // `/OPT:REF` liga também o `/OPT:ICF` do `lld-link`, que
                    // funde funções de corpo idêntico num endereço só; o
                    // `/OPT:NOICF` vai logo abaixo, para todo perfil
                    // (`left == right` daria `true`, corpus/js 147 e 184).
                    cmd.args(["-Wl,/NODEFAULTLIB:libcmt", "-lmsvcrt", "-Wl,/OPT:REF"]);
                    cmd.arg(format!("-Wl,/opt:lldltopartitions={particoes}"));
                }
                // Sem a tabela de símbolos, como o `.exe` do Windows (que a
                // deixa no PDB): metade do tamanho no ELF (medido: 9,4 → 4,4 MB).
                Sistema::Linux => {
                    cmd.arg("-Wl,--gc-sections");
                    if !crate::ligador::manter_simbolos() && !depuracao {
                        cmd.arg("-Wl,--strip-all");
                    }
                    cmd.arg(format!("-Wl,--lto-partitions={particoes}"));
                }
                Sistema::MacOs => {
                    cmd.arg("-Wl,-dead_strip");
                    // `-S` tira o mapa de depuração (J05).
                    if !depuracao {
                        cmd.args(["-Wl,-S", "-Wl,-x"]);
                    }
                }
            }
        }
        Ligacao::Runtime(_) => {
            cmd.args(crate::alvo::argumentos_de_ligacao());
        }
    }
    if sistema == Sistema::Windows && depuracao {
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
    let saida = cmd
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|e| format!("falha na ligação com Clang em {clang:?}: {e}"))?;
    if !saida.status.success() {
        // O que o ligador disse vai no erro (sem isso a falha no CI não
        // tem diagnóstico).
        let texto = String::from_utf8_lossy(&saida.stderr);
        let linhas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
        return Err(format!("Clang falhou na ligação do executável ({}):\n{}", saida.status, linhas.join("\n")));
    }
    Ok(())
}

/// O que a limpeza depois da ligação apaga: nada com `manter_ir`, e nunca um
/// caminho dentro do cache — o objeto que veio de lá é de todos.
fn arquivos_a_remover(ll: &Path, obj: &Path, dir_cache: &Path, manter_ir: bool) -> Vec<PathBuf> {
    if manter_ir {
        return Vec::new();
    }
    [ll, obj].into_iter().filter(|p| !p.starts_with(dir_cache)).map(Path::to_path_buf).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limpeza_nunca_apaga_o_cache() {
        let cache = Path::new("D:/x/target/native_cache");
        let ll = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.ll");
        let obj_cache = cache.join("obj/ab/ab00.obj");
        let obj_local = Path::new("D:/x/target/diferencial/nativo/p/.df_tmp/p.obj");
        assert_eq!(arquivos_a_remover(ll, &obj_cache, cache, false), vec![ll.to_path_buf()]);
        assert_eq!(arquivos_a_remover(ll, obj_local, cache, false), vec![ll.to_path_buf(), obj_local.to_path_buf()]);
        assert!(arquivos_a_remover(ll, obj_local, cache, true).is_empty());
    }
}
