//! O SDK compilado da fonte, por biblioteca (P5, docs/NATIVO-PLANO.md §7).
//!
//! O `dart:core` do nativo vem da fonte do SDK 3.6.2 — a seção `vm` do
//! `libraries.json` com os patches dela — e de uma sobreposição fina nossa,
//! `sdk_nativo/` na raiz do repositório, que troca só os arquivos que dependem
//! da máquina interna da VM (`_SuspendState`, `Timer`/microtarefas ligados ao
//! `dart:isolate`, o `Finalizer` do GC fraco). A troca é feita no
//! carregamento (`SdkLayout::load_com_sobreposicao`, `crates/elements`).
//!
//! Este módulo reúne o que é do SDK como unidade de compilação: onde está a
//! sobreposição, quais bibliotecas vêm da fonte, e a medição de 5a — os
//! diagnósticos de inferência nos corpos dessas bibliotecas, que precisam ser
//! zero antes de o lowering compilá-los.

use dartforge_elements::sdk::SdkLayout;
use std::path::{Path, PathBuf};

/// O alvo da sobreposição em `sdk_nativo/libraries.json`.
pub const ALVO_SOBREPOSICAO: &str = "dartforge_nativo";

/// As bibliotecas que o nativo compila da fonte do SDK (decisão 1 de
/// NATIVO-PLANO §7.1), na ordem de dependência aproximada. `typed_data`
/// entra depois (P9).
pub const BIBLIOTECAS_DA_FONTE: &[&str] =
    &["_internal", "core", "_compact_hash", "collection", "math", "convert", "async", "typed_data"];

/// Diretório da sobreposição: `DARTFORGE_SDK_NATIVO`, senão o `sdk_nativo/`
/// do repositório que compilou este binário.
pub fn dir_sobreposicao() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_SDK_NATIVO") {
        return PathBuf::from(d);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk_nativo")
}

/// O SDK do nativo: a seção `vm` de `lib_dir` com a sobreposição.
pub fn carregar_sdk_nativo(lib_dir: &Path) -> Result<SdkLayout, String> {
    SdkLayout::load_com_sobreposicao(lib_dir, &dir_sobreposicao(), ALVO_SOBREPOSICAO)
}

/// Diagnósticos de inferência nos corpos de uma biblioteca do SDK.
#[derive(Debug, Clone)]
pub struct InferenciaDaBiblioteca {
    pub biblioteca: String,
    /// Funções (e métodos, construtores, acessores) declaradas nela.
    pub funcoes: usize,
    /// As mensagens, na ordem.
    pub diagnosticos: Vec<String>,
}

/// A medição de 5a: carrega um programa que importa todas as
/// [`BIBLIOTECAS_DA_FONTE`] com a sobreposição, resolve o outline e infere
/// os corpos de cada biblioteca **separadamente** (os diagnósticos não
/// trazem o arquivo; uma inferência por biblioteca os atribui).
///
/// Depende de `infer_bodies_das_bibliotecas` inferir as bibliotecas do SDK
/// pedidas (pedido em docs/NATIVO-PEDIDOS.md); sem isso, só os\n/// inicializadores sem tipo escrito são visitados e os diagnósticos ficam\n/// perto de zero sem dizer nada.
pub fn medir_inferencia_do_sdk(lib_dir: &Path) -> Result<Vec<InferenciaDaBiblioteca>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = carregar_sdk_nativo(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-medir-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entrada = tmp.join("main.dart");
    let mut fonte = String::new();
    for b in BIBLIOTECAS_DA_FONTE {
        fonte.push_str(&format!("import 'dart:{b}';\n"));
    }
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| format!("{}: {e}", entrada.display()))?;

    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {} ({} diagnósticos)", d.message, diags_carga.len()));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);

    let mut saida = Vec::new();
    for nome in BIBLIOTECAS_DA_FONTE {
        let uri = format!("dart:{nome}");
        let Some(idx) = program.libraries.iter().position(|l| l.uri == uri) else {
            return Err(format!("{uri} não carregou"));
        };
        let lib = dartforge_elements::model::LibraryId(idx as u32);
        let funcoes = program.functions.iter().filter(|f| f.library == lib).count();
        let (_corpos, diags) =
            dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[lib]);
        saida.push(InferenciaDaBiblioteca {
            biblioteca: uri,
            funcoes,
            diagnosticos: diags.into_iter().map(|d| d.message).collect(),
        });
    }
    Ok(saida)
}

/// O SDK da fonte está ligado neste processo? (`DARTFORGE_SDK_DA_FONTE=1`;
/// até a troca de P5d, o caminho padrão é o runtime por nome.)
pub fn sdk_da_fonte_pedido() -> bool {
    std::env::var("DARTFORGE_SDK_DA_FONTE").is_ok_and(|v| v.trim() == "1")
}

/// A função que registra no runtime as classes de uma biblioteca do SDK da
/// fonte (nomes, subtipos, tabelas de métodos); a entrada do programa a
/// chama.
pub fn simbolo_de_registro(uri: &str) -> String {
    format!("df.registrar.{}", crate::context::escapar(uri))
}

/// As funções de registro das [`BIBLIOTECAS_DA_FONTE`], na ordem.
pub fn registros_do_sdk() -> Vec<String> {
    BIBLIOTECAS_DA_FONTE.iter().map(|b| simbolo_de_registro(&format!("dart:{b}"))).collect()
}

/// Os ids de classe (do SDK da fonte) dos valores que o runtime representa,
/// na ordem de `runtime/src/seletores.rs` (`CID_*`).
pub fn cids_do_runtime(ctx: &crate::context::Context) -> Vec<i64> {
    const CLASSES: &[(&str, &str)] = &[
        ("core", "Null"),
        ("core", "_Smi"),
        ("core", "_Mint"),
        ("core", "_Double"),
        ("core", "bool"),
        ("core", "_OneByteString"),
        ("core", "_TwoByteString"),
        ("core", "_GrowableList"),
        ("core", "_List"),
        ("core", "_ImmutableList"),
        ("core", "_Closure"),
        ("core", "_Record"),
    ];
    CLASSES
        .iter()
        .map(|(l, c)| ctx.classe_do_sdk(l, c).and_then(|k| ctx.id_de_classe(k)).map_or(-1, i64::from))
        .collect()
}

/// Um programa mínimo que importa as bibliotecas da fonte, carregado com a
/// sobreposição.
fn carregar_bibliotecas_da_fonte(
    lib_dir: &Path,
) -> Result<(dartforge_elements::Program, dartforge_intern::Interner), String> {
    let sdk = carregar_sdk_nativo(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!(
        "dartforge-sdk-fonte-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entrada = tmp.join("main.dart");
    let mut fonte = String::new();
    for b in BIBLIOTECAS_DA_FONTE {
        fonte.push_str(&format!("import 'dart:{b}';\n"));
    }
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| format!("{}: {e}", entrada.display()))?;
    let mut interner = dartforge_intern::Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {}", d.message));
    }
    Ok((program, interner))
}

/// Uma biblioteca do SDK da fonte baixada e emitida.
#[derive(Debug, Clone)]
pub struct BibliotecaDoSdk {
    /// `dart:core`…
    pub uri: String,
    /// O LLVM IR do módulo dela.
    pub ir: String,
    /// Os membros recusados: (símbolo, motivo).
    pub recusados: Vec<(String, String)>,
}

/// Baixa e emite cada uma das [`BIBLIOTECAS_DA_FONTE`] no seu módulo (P5c):
/// mundo aberto, símbolos estáveis, os membros que não baixam recusados um a
/// um. É função só das fontes do SDK, da sobreposição e do compilador.
pub fn emitir_bibliotecas_do_sdk(lib_dir: &Path) -> Result<Vec<BibliotecaDoSdk>, String> {
    use dartforge_types::table::{CoreTypes, TypeTable};
    let (program, interner) = carregar_bibliotecas_da_fonte(lib_dir)?;
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = BIBLIOTECAS_DA_FONTE
        .iter()
        .filter_map(|b| {
            let uri = format!("dart:{b}");
            program
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| dartforge_elements::model::LibraryId(i as u32))
        })
        .collect();
    if libs.len() != BIBLIOTECAS_DA_FONTE.len() {
        return Err("biblioteca do SDK da fonte fora do programa".to_string());
    }
    let (corpos, _) =
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &libs);
    let base = crate::context::Context::new(&program, &interner, &table, &core, &outline, &corpos);
    let base = base.com_sdk_da_fonte();
    let mut saida = Vec::new();
    for lib in libs {
        let uri = program.library(lib).uri.clone();
        let ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &corpos)
            .com_sdk_da_fonte()
            .so_a_biblioteca(lib);
        debug_assert_eq!(ctx.ids_de_classe, base.ids_de_classe);
        let mut module = crate::lower::lower_program(&ctx);
        module.biblioteca_sdk = true;
        module.registro = Some(simbolo_de_registro(&uri));
        if let Some(e) = module.erros.first() {
            return Err(format!("{uri}: o módulo do SDK tem diagnóstico fora de membro: {e}"));
        }
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        saida.push(BibliotecaDoSdk { uri, ir, recusados: std::mem::take(&mut module.recusados) });
    }
    Ok(saida)
}

/// O SDK da fonte compilado: uma DLL com o runtime e as bibliotecas da
/// fonte, e a biblioteca de importação que o executável liga.
///
/// **Por que DLL.** Ligar os objetos do SDK (~12 MB) em cada executável
/// custava ~450 ms de ligação por programa (medido; a ligação de antes leva
/// ~60 ms), e as tabelas de métodos, registradas na partida, alcançam todo o
/// código — o `/OPT:REF` não tem o que tirar. É o "grupo = código
/// compartilhado" de docs/NATIVO-PLANO §4.4: o runtime e o SDK são ligados
/// **uma vez por conteúdo**, e o executável liga só o objeto do programa e a
/// biblioteca de importação. A DLL vai ao lado do executável (ligação
/// física, sem cópia).
#[derive(Debug, Clone)]
pub struct SdkCompilado {
    pub objetos: Vec<PathBuf>,
    pub dll: PathBuf,
    pub importacao: PathBuf,
    /// Quanto levou para compilar (o custo a frio); `None` quando veio do
    /// cache.
    pub frio: Option<std::time::Duration>,
}

/// Os símbolos que um IR define com ligação externa (o que a DLL exporta).
fn simbolos_definidos(ir: &str, saida: &mut Vec<String>) {
    for l in ir.lines() {
        let Some(r) = l.strip_prefix("define ") else { continue };
        if r.starts_with("internal ") || r.starts_with("private ") || r.starts_with("linkonce_odr ") {
            continue;
        }
        let Some(i) = r.find('@') else { continue };
        let Some(f) = r[i..].find('(') else { continue };
        saida.push(r[i + 1..i + f].to_string());
    }
}

/// A chave do SDK compilado: blake3 das fontes do SDK que entram (as
/// bibliotecas da fonte, os patches `vm`, `libraries.json`), da
/// sobreposição, do compilador (`DARTFORGE_EMISSOR_HASH`, `build.rs`), da
/// identidade do Clang e das bandeiras.
fn chave_do_sdk(lib_dir: &Path, clang_id: &str, args: &[&str]) -> String {
    fn juntar(dir: &Path, saida: &mut Vec<PathBuf>) {
        let Ok(entradas) = std::fs::read_dir(dir) else { return };
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                juntar(&p, saida);
            } else if p.extension().is_some_and(|x| x == "dart" || x == "json") {
                saida.push(p);
            }
        }
    }
    let mut arquivos = Vec::new();
    for d in ["core", "async", "collection", "convert", "math", "internal", "typed_data", "_internal/vm/lib", "_internal/vm_shared/lib"] {
        juntar(&lib_dir.join(d), &mut arquivos);
    }
    arquivos.push(lib_dir.join("libraries.json"));
    let sobreposicao = dir_sobreposicao();
    juntar(&sobreposicao, &mut arquivos);
    arquivos.sort();
    let mut h = blake3::Hasher::new();
    h.update(b"dartforge-sdk-fonte\0");
    h.update(env!("CARGO_PKG_VERSION").as_bytes());
    h.update(b"\0");
    h.update(env!("DARTFORGE_EMISSOR_HASH").as_bytes());
    h.update(b"\0");
    // O rastro de depuração muda o código (`llvm/mod.rs`).
    h.update(std::env::var("DARTFORGE_RASTRO").unwrap_or_default().as_bytes());
    h.update(b"\0");
    h.update(clang_id.as_bytes());
    for a in args {
        h.update(a.as_bytes());
        h.update(b"\0");
    }
    for a in &arquivos {
        let rel = a
            .strip_prefix(lib_dir)
            .or_else(|_| a.strip_prefix(&sobreposicao))
            .unwrap_or(a)
            .to_string_lossy()
            .replace('\\', "/");
        h.update(rel.as_bytes());
        h.update(b"\0");
        h.update(&std::fs::read(a).unwrap_or_default());
        h.update(b"\0");
    }
    h.finalize().to_hex()[..32].to_string()
}

/// As bandeiras do Clang para os objetos do SDK no perfil de
/// desenvolvimento: as do programa, e cada função na sua seção.
pub const ARGS_CLANG_DO_SDK: &[&str] =
    &["-x", "ir", "-c", "-O0", "-mno-incremental-linker-compatible", "-ffunction-sections", "-fdata-sections"];

/// As do perfil de produção: bitcode para a otimização entre módulos
/// (ThinLTO) na ligação do executável (docs/PESQUISA-LLVM-DART-AOT.md §5:
/// desenvolvimento em módulos separados, produção com ThinLTO).
pub const ARGS_CLANG_DO_SDK_PRODUCAO: &[&str] =
    &["-x", "ir", "-c", "-O2", "-flto=thin", "-mno-incremental-linker-compatible", "-ffunction-sections", "-fdata-sections"];

/// O perfil em que o SDK da fonte é compilado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfilDoSdk {
    /// Objetos `-O0` e a DLL (runtime + SDK) que o executável importa.
    Desenvolvimento,
    /// Bitcode ThinLTO `-O2`, ligado estaticamente no executável.
    Producao,
}

/// Os objetos do SDK da fonte: do cache (`<cache nativo>/sdk/<chave>/`), ou
/// compilados agora — uma vez por conteúdo, as bibliotecas em paralelo.
pub fn sdk_compilado(lib_dir: &Path, clang: &Path) -> Result<SdkCompilado, String> {
    sdk_compilado_no_perfil(lib_dir, clang, PerfilDoSdk::Desenvolvimento)
}

/// [`sdk_compilado`] no perfil pedido.
pub fn sdk_compilado_no_perfil(lib_dir: &Path, clang: &Path, perfil: PerfilDoSdk) -> Result<SdkCompilado, String> {
    static TRAVA: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
    let clang_id = crate::cache_objeto::identidade_clang(clang)?;
    let args = match perfil {
        PerfilDoSdk::Desenvolvimento => ARGS_CLANG_DO_SDK,
        PerfilDoSdk::Producao => ARGS_CLANG_DO_SDK_PRODUCAO,
    };
    let chave = chave_do_sdk(lib_dir, &clang_id, args);
    let raiz = crate::cache::dir_cache_nativo().join("sdk");
    let dir = raiz.join(&chave);
    let nome_dll = format!("dfsdk_{}", &chave[..16]);
    let pronto = |d: &Path, frio| SdkCompilado {
        objetos: BIBLIOTECAS_DA_FONTE.iter().map(|b| d.join(format!("{b}.obj"))).collect(),
        dll: d.join(format!("{nome_dll}.dll")),
        importacao: d.join(format!("{nome_dll}.lib")),
        frio,
    };
    if dir.join("pronto").is_file() {
        return Ok(pronto(&dir, None));
    }
    let runtime_dll = crate::cache::RuntimeCache::para_dll()?;
    let t0 = std::time::Instant::now();
    let tmp = raiz.join(format!("{chave}.tmp.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let libs = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn({
            let lib_dir = lib_dir.to_path_buf();
            move || emitir_bibliotecas_do_sdk(&lib_dir)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "pânico ao emitir o SDK da fonte".to_string())??;
    let mut exportados = Vec::new();
    for lib in &libs {
        simbolos_definidos(&lib.ir, &mut exportados);
    }
    exportados.extend(
        dartforge_runtime::simbolos::NOMES.iter().filter(|n| **n != "main").map(|n| n.to_string()),
    );
    let mut def = format!("LIBRARY {nome_dll}.dll\nEXPORTS\n");
    for s in &exportados {
        def.push_str(&format!("  {s}\n"));
    }
    std::fs::write(tmp.join("exportados.def"), def).map_err(|e| e.to_string())?;
    let mut resumo = String::new();
    for (b, lib) in BIBLIOTECAS_DA_FONTE.iter().zip(&libs) {
        std::fs::write(tmp.join(format!("{b}.ll")), &lib.ir).map_err(|e| e.to_string())?;
        for (s, m) in &lib.recusados {
            resumo.push_str(&format!("{}\t{s}\t{m}\n", lib.uri));
        }
    }
    std::fs::write(tmp.join("recusados.tsv"), resumo).map_err(|e| e.to_string())?;
    // O Clang de cada biblioteca num processo, no máximo 4 ao mesmo tempo.
    let fila = std::sync::Mutex::new(BIBLIOTECAS_DA_FONTE.to_vec());
    let erros = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| loop {
                let Some(b) = fila.lock().unwrap().pop() else { break };
                let st = std::process::Command::new(clang)
                    .current_dir(&tmp)
                    .args(args)
                    .arg(format!("{b}.ll"))
                    .arg("-o")
                    .arg(format!("{b}.obj"))
                    .status();
                match st {
                    Ok(s) if s.success() => {}
                    Ok(s) => erros.lock().unwrap().push(format!("Clang recusou o IR de dart:{b} ({s})")),
                    Err(e) => erros.lock().unwrap().push(format!("Clang: {e}")),
                }
            });
        }
    });
    if let Some(e) = erros.into_inner().unwrap().into_iter().next() {
        return Err(e);
    }
    // A DLL (desenvolvimento): os objetos do SDK e o runtime (variante sem
    // `main`). Em produção, os objetos (bitcode) vão direto para o
    // executável.
    let st = if perfil == PerfilDoSdk::Producao {
        None
    } else {
        Some(std::process::Command::new(clang)
        .current_dir(&tmp)
        .arg("-shared")
        .args(BIBLIOTECAS_DA_FONTE.iter().map(|b| format!("{b}.obj")))
        .arg(&runtime_dll.lib_path)
        .arg("-Wl,/DEF:exportados.def")
        .args(["-lws2_32", "-luserenv", "-lntdll", "-o"])
        .arg(format!("{nome_dll}.dll"))
        .status()
        .map_err(|e| format!("Clang: {e}"))?)
    };
    if let Some(st) = st
        && !st.success()
    {
        return Err(format!("a ligação da DLL do SDK da fonte falhou ({st})"));
    }
    if std::env::var_os("DARTFORGE_KEEP_IR").is_none() {
        for b in BIBLIOTECAS_DA_FONTE {
            let _ = std::fs::remove_file(tmp.join(format!("{b}.ll")));
        }
    }
    std::fs::write(tmp.join("pronto"), b"").map_err(|e| e.to_string())?;
    if std::fs::rename(&tmp, &dir).is_err() {
        // Outro processo terminou antes: fica o dele.
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join("pronto").is_file() {
            return Err(format!("não foi possível instalar o SDK compilado em {}", dir.display()));
        }
    }
    Ok(pronto(&dir, Some(t0.elapsed())))
}

/// O resultado do lowering de um membro do SDK da fonte.
#[derive(Debug, Clone)]
pub struct MembroDoSdk {
    pub biblioteca: String,
    pub simbolo: String,
    /// Diagnósticos do lowering (vazio: baixou), ou o pânico.
    pub diagnosticos: Vec<String>,
}

/// Medição de P5c: baixa cada função das [`BIBLIOTECAS_DA_FONTE`] (mundo
/// aberto) e devolve, por membro, os diagnósticos do lowering.
pub fn medir_lowering_do_sdk(lib_dir: &Path) -> Result<Vec<MembroDoSdk>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = carregar_sdk_nativo(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-lower-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entrada = tmp.join("main.dart");
    let mut fonte = String::new();
    for b in BIBLIOTECAS_DA_FONTE {
        fonte.push_str(&format!("import 'dart:{b}';\n"));
    }
    fonte.push_str("void main() {}\n");
    std::fs::write(&entrada, fonte).map_err(|e| format!("{}: {e}", entrada.display()))?;
    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entrada, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {}", d.message));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| l.uri.strip_prefix("dart:").is_some_and(|n| BIBLIOTECAS_DA_FONTE.contains(&n)))
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (corpos, _) =
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &libs);
    let ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &corpos).com_sdk_da_fonte();
    let mut saida = Vec::new();
    for f in 0..program.functions.len() {
        if !libs.contains(&program.functions[f].library) {
            continue;
        }
        let simbolo = crate::lower::simbolo_de(&ctx, f);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = crate::hir::Module::new();
            crate::lower::lower_funcao(&ctx, &mut m, f);
            m.erros
        }));
        let diagnosticos = match r {
            Ok(e) => e,
            Err(p) => {
                let msg = p
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                vec![format!("pânico: {msg}")]
            }
        };
        saida.push(MembroDoSdk {
            biblioteca: program.library(program.functions[f].library).uri.clone(),
            simbolo,
            diagnosticos,
        });
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

    /// A sobreposição existe, troca os quatro arquivos e aponta para arquivos
    /// que existem.
    #[test]
    fn sobreposicao_carrega() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let sdk = carregar_sdk_nativo(Path::new(SDK)).unwrap();
        assert_eq!(sdk.substituicoes.len(), 9);
        for b in BIBLIOTECAS_DA_FONTE {
            assert!(sdk.library(b).is_some(), "dart:{b} fora do layout");
        }
    }

    /// A medição de 5a (docs/NATIVO-PLANO.md §7.4): imprime, por biblioteca,
    /// as funções e os diagnósticos de inferência dos corpos. Lenta (infere o
    /// SDK inteiro); roda à parte:
    /// `cargo test -p dartforge-emit-native medir_inferencia -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn medir_inferencia_das_bibliotecas_da_fonte() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let tabela = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| medir_inferencia_do_sdk(Path::new(SDK)).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut total = 0;
        for b in &tabela {
            println!("{:<18} funções {:>5}  diagnósticos {:>5}", b.biblioteca, b.funcoes, b.diagnosticos.len());
            total += b.diagnosticos.len();
            let mut por_codigo: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for d in &b.diagnosticos {
                let chave: String = d.split(':').next().unwrap_or(d).chars().take(60).collect();
                *por_codigo.entry(chave).or_default() += 1;
            }
            let mut v: Vec<_> = por_codigo.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1));
            for (c, n) in v.iter().take(8) {
                println!("    {n:>5}  {c}");
            }
        }
        println!("total de diagnósticos: {total}");
    }

    /// P5c: emite os módulos do SDK da fonte e mostra, por biblioteca, o
    /// tamanho do IR e os membros recusados (por motivo). Com
    /// `DARTFORGE_SDK_COMPILAR=1`, compila e instala os objetos no cache.
    /// `cargo test -p dartforge-emit-native emitir_o_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn emitir_o_sdk() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            return;
        }
        let t = std::time::Instant::now();
        let libs = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| emitir_bibliotecas_do_sdk(Path::new(SDK)).unwrap())
            .unwrap()
            .join()
            .unwrap();
        println!("emissão: {:?}", t.elapsed());
        let mut motivos: std::collections::BTreeMap<String, usize> = Default::default();
        let mut total = 0;
        for l in &libs {
            println!("{:<18} IR {:>9} bytes  recusados {:>5}", l.uri, l.ir.len(), l.recusados.len());
            total += l.recusados.len();
            for (_, m) in &l.recusados {
                *motivos.entry(m.chars().take(100).collect()).or_default() += 1;
            }
        }
        println!("recusados: {total}");
        let mut v: Vec<_> = motivos.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (m, n) in v.iter().take(60) {
            println!("{n:>5}  {m}");
        }
        if let Ok(dir) = std::env::var("DARTFORGE_SDK_IR_DIR") {
            for (b, l) in BIBLIOTECAS_DA_FONTE.iter().zip(&libs) {
                std::fs::write(Path::new(&dir).join(format!("{b}.ll")), &l.ir).unwrap();
                let r: String = l.recusados.iter().map(|(s, m)| format!("{s}\t{m}\n")).collect();
                std::fs::write(Path::new(&dir).join(format!("{b}.recusados.tsv")), r).unwrap();
            }
        }
        if std::env::var("DARTFORGE_SDK_COMPILAR").is_ok_and(|v| v == "1") {
            let clang = std::env::var_os("DARTFORGE_CLANG")
                .map_or_else(|| PathBuf::from("D:/LLVM/22.1.8/bin/clang.exe"), PathBuf::from);
            let s = sdk_compilado(Path::new(SDK), &clang).unwrap();
            println!("dll: {:?} (frio: {:?})", s.dll, s.frio);
        }
    }

    /// Compila e executa `DARTFORGE_PROGRAMA` com o SDK da fonte (P5c):
    /// `cargo test -p dartforge-emit-native compilar_com_o_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "manual; roda à parte"]
    fn compilar_com_o_sdk_da_fonte() {
        let Ok(p) = std::env::var("DARTFORGE_PROGRAMA") else { return };
        let entrada = PathBuf::from(p);
        let saida = entrada.with_extension("exe");
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let otimizar = std::env::var("DARTFORGE_OTIMIZAR").is_ok_and(|v| v == "1");
                let opcoes = crate::CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: true, optimize: otimizar };
                crate::compilar(&entrada, &saida, &opcoes).map(|_| saida)
            })
            .unwrap()
            .join()
            .unwrap();
        match r {
            Ok(exe) => {
                let o = std::process::Command::new(&exe).output().unwrap();
                println!("--- código {:?}\n--- stdout\n{}--- stderr\n{}", o.status.code(), String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            }
            Err(e) => println!("ERRO: {e}"),
        }
    }

    /// Perfil de produção com o SDK da fonte: UM executável autocontido. O
    /// `.exe` é copiado sozinho para uma pasta vazia e roda sem nenhuma DLL
    /// do DartForge (docs/NATIVO-PLANO.md §7.7).
    #[test]
    #[ignore = "compila o SDK da fonte (lento a frio); roda no CI"]
    fn producao_e_um_executavel_autocontido() {
        // O SDK instalado (no CI, o do `DART_HOME`); sem ele, o teste só é
        // pulado fora do CI — no CI, ausência é falha, não sucesso vazio.
        let sdk_dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from(SDK));
        if !sdk_dir.join("libraries.json").is_file() {
            assert!(std::env::var_os("CI").is_none(), "SDK do Dart ausente no CI ({})", sdk_dir.display());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let entrada = dir.path().join("main.dart");
        std::fs::write(&entrada, "void main() {\n  print('oi');\n  print([1, 2, 3].map((x) => x * 2).toList());\n  print({'a': 1});\n}\n").unwrap();
        let exe = dir.path().join("prog.exe");
        let (e2, x2) = (entrada.clone(), exe.clone());
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let opcoes = crate::CompileOptions { sdk: Some(&sdk_dir), packages: None, timings: false, optimize: true };
                crate::compilar_com(&e2, &x2, &opcoes, true)
            })
            .unwrap()
            .join()
            .unwrap();
        r.expect("compilar em produção");
        let sozinho = tempfile::tempdir().unwrap();
        let copia = sozinho.path().join("prog.exe");
        std::fs::copy(&exe, &copia).unwrap();
        let so_um: Vec<_> = std::fs::read_dir(sozinho.path()).unwrap().collect();
        assert_eq!(so_um.len(), 1, "a pasta tem só o executável");
        let o = std::process::Command::new(&copia).current_dir(sozinho.path()).output().unwrap();
        assert!(o.status.success(), "código {:?}; stderr: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"), "oi\n[2, 4, 6]\n{a: 1}\n");
    }

    /// A medição de P5c: quantos membros do SDK da fonte o lowering baixa.
    /// `cargo test -p dartforge-emit-native medir_lowering -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn medir_lowering_das_bibliotecas_da_fonte() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            return;
        }
        std::panic::set_hook(Box::new(|_| {}));
        let membros = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| medir_lowering_do_sdk(Path::new(SDK)).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut por_lib: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
        let mut por_construto: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for m in &membros {
            let e = por_lib.entry(&m.biblioteca).or_default();
            e.0 += 1;
            if m.diagnosticos.is_empty() {
                e.1 += 1;
            }
            let mut vistos = std::collections::BTreeSet::new();
            for d in &m.diagnosticos {
                let d = d.strip_prefix(crate::PREFIXO_NAO_SUPORTADO).unwrap_or(d);
                let chave: String = d.rsplit_once(" (").map_or(d, |(a, _)| a).chars().take(90).collect();
                let c = por_construto.entry(chave.clone()).or_default();
                c.1 += 1;
                if vistos.insert(chave) {
                    c.0 += 1;
                }
            }
        }
        for (l, (n, ok)) in &por_lib {
            println!("{l:<20} membros {n:>5}  baixados {ok:>5}");
        }
        let mut v: Vec<_> = por_construto.into_iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        for (c, (membros, ocorr)) in v.iter().take(80) {
            println!("{membros:>5} {ocorr:>6}  {c}");
        }
        if let Ok(arq) = std::env::var("DARTFORGE_MEDIR_SAIDA") {
            let mut t = String::new();
            for m in &membros {
                t.push_str(&format!("{}\t{}\t{}\n", m.biblioteca, m.simbolo, m.diagnosticos.join(" | ")));
            }
            std::fs::write(arq, t).unwrap();
        }
    }
}
