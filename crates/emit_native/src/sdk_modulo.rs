//! O SDK compilado da fonte, por biblioteca (P5, docs/NATIVO-PLANO.md §7).
//!
//! O `dart:core` do nativo vem da fonte do SDK 3.6.2 — a seção `vm` do
//! `libraries.json` com os patches dela — e de uma sobreposição fina nossa,
//! `native_sdk/` na raiz do repositório, que troca só os arquivos que dependem
//! da máquina interna da VM (`_SuspendState`, `Timer`/microtarefas ligados ao
//! `dart:isolate`, o `Finalizer` do GC fraco). A troca é feita no
//! carregamento (`SdkLayout::load_with_overlay`, `crates/elements`).
//!
//! Este módulo reúne o que é do SDK como unidade de compilação: onde está a
//! sobreposição, quais bibliotecas vêm da fonte, e a medição de 5a — os
//! diagnósticos de inferência nos corpos dessas bibliotecas, que precisam ser
//! zero antes de o lowering compilá-los.

use crate::gerador::{Format, Generation, Gerador};
use dartforge_elements::sdk::SdkLayout;
use std::path::{Path, PathBuf};

/// O alvo da sobreposição em `native_sdk/libraries.json`.
pub const OVERLAY_TARGET: &str = "dartforge_nativo";

/// As bibliotecas que o nativo compila da fonte do SDK (decisão 1 de
/// NATIVO-PLANO §7.1), na ordem de dependência aproximada. `typed_data`
/// entra depois (P9).
pub const SOURCE_LIBRARIES: &[&str] =
    &[
        "_internal",
        "core",
        "_compact_hash",
        "collection",
        "math",
        "convert",
        "async",
        "typed_data",
        "isolate",
        "developer",
        "nativewrappers",
        "ffi",
        "_http",
        "io",
        "mirrors",
    ];

/// Diretório da sobreposição: `DARTFORGE_SDK_NATIVO`, senão o `native_sdk/`
/// do repositório que compilou este binário.
pub fn dir_sobreposicao() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_SDK_NATIVO") {
        return PathBuf::from(d);
    }
    if let Some(d) = dartforge_elements::distribuicao::sobreposicao_nativa() {
        return d;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../native_sdk")
}

/// O SDK do nativo: a seção `vm` de `lib_dir` com a sobreposição.
pub fn load_native_sdk(lib_dir: &Path) -> Result<SdkLayout, String> {
    SdkLayout::load_com_sobreposicao(lib_dir, &dir_sobreposicao(), OVERLAY_TARGET)
}

/// Diagnósticos de inferência nos corpos de uma biblioteca do SDK.
#[derive(Debug, Clone)]
pub struct LibraryInference {
    pub library: String,
    /// Funções (e métodos, construtores, acessores) declaradas nela.
    pub functions: usize,
    /// As mensagens, na ordem.
    pub diagnostics: Vec<String>,
}

/// A medição de 5a: carrega um programa que importa todas as
/// [`SOURCE_LIBRARIES`] com a sobreposição, resolve o outline e infere
/// os corpos de cada biblioteca **separadamente** (os diagnósticos não
/// trazem o arquivo; uma inferência por biblioteca os atribui).
///
/// Depende de `infer_library_bodies` inferir as bibliotecas do SDK
/// pedidas (pedido em docs/NATIVO-PEDIDOS.md); sem isso, só os\n/// inicializadores sem tipo escrito são visitados e os diagnósticos ficam\n/// perto de zero sem dizer nada.
pub fn measure_sdk_inference(lib_dir: &Path) -> Result<Vec<LibraryInference>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-medir-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;

    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {} ({} diagnósticos)", d.message, diags_carga.len()));
    }
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);

    let mut exit = Vec::new();
    for name in SOURCE_LIBRARIES {
        let uri = format!("dart:{name}");
        let Some(idx) = program.libraries.iter().position(|l| l.uri == uri) else {
            return Err(format!("{uri} não carregou"));
        };
        let lib = dartforge_elements::model::LibraryId(idx as u32);
        let functions = program.functions.iter().filter(|f| f.library == lib).count();
        let (_bodies, diags) =
            dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &[lib]);
        exit.push(LibraryInference {
            library: uri,
            functions,
            diagnostics: diags.into_iter().map(|d| d.message).collect(),
        });
    }
    Ok(exit)
}


/// A função que registra no runtime as classes de uma biblioteca do SDK da
/// fonte (nomes, subtipos, tabelas de métodos); a entrada do programa a
/// chama.
pub fn registration_symbol(uri: &str) -> String {
    format!("df.registrar.{}", crate::context::escape_str(uri))
}

/// As funções de registro das [`SOURCE_LIBRARIES`], na ordem.
pub fn sdk_records() -> Vec<String> {
    SOURCE_LIBRARIES.iter().map(|b| registration_symbol(&format!("dart:{b}"))).collect()
}

/// Os ids de classe (do SDK da fonte) dos valores que o runtime representa,
/// na ordem de `runtime/src/selectors.rs` (`CID_*`).
pub fn runtime_cids(ctx: &crate::context::Context) -> Vec<i64> {
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
        ("typed_data", "_Uint8List"),
        ("typed_data", "_Uint8ArrayView"),
        ("typed_data", "_Int64List"),
        ("typed_data", "_Float32x4"),
        ("typed_data", "_Int32x4"),
        ("typed_data", "_Float64x2"),
        ("typed_data", "_Int8List"),
        ("typed_data", "_Uint8ClampedList"),
        ("typed_data", "_Int16List"),
        ("typed_data", "_Uint16List"),
        ("typed_data", "_Int32List"),
        ("typed_data", "_Uint32List"),
        ("typed_data", "_Uint64List"),
        ("typed_data", "_Float32List"),
        ("typed_data", "_Float64List"),
        ("typed_data", "_Int32x4List"),
        ("typed_data", "_Float32x4List"),
        ("typed_data", "_Float64x2List"),
        ("isolate", "_SendPort"),
        ("isolate", "_Capability"),
    ];
    CLASSES
        .iter()
        .map(|(l, c)| ctx.sdk_class(l, c).and_then(|k| ctx.class_id_of(k)).map_or(-1, i64::from))
        .collect()
}

/// Um programa mínimo que importa as bibliotecas da fonte, carregado com a
/// sobreposição.
fn load_source_libraries(
    lib_dir: &Path,
) -> Result<(dartforge_elements::Program, dartforge_intern::Interner), String> {
    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!(
        "dartforge-sdk-fonte-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;
    let mut interner = dartforge_intern::Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    let _ = std::fs::remove_dir_all(&tmp);
    if let Some(d) = diags_carga.first() {
        return Err(format!("carga do SDK com a sobreposição: {}", d.message));
    }
    Ok((program, interner))
}

/// A tabela de ids das classes do SDK da fonte (`context::IdTable`),
/// calculada uma vez por conteúdo do SDK, da sobreposição e do compilador,
/// e guardada em `<cache native>/sdk_ids/<key>.tsv`. O programa a usa para
/// numerar as classes do SDK como o módulo do SDK as numera, sem carregar
/// as bibliotecas do SDK que não importa.
///
/// # Erros
/// Falha ao carregar o SDK com a sobreposição, ou ao gravar o cache.
pub fn sdk_class_ids(lib_dir: &Path) -> Result<std::sync::Arc<crate::context::IdTable>, String> {
    use crate::context::IdTable;
    use std::sync::{Arc, Mutex, OnceLock};
    static MEMO: OnceLock<Mutex<std::collections::HashMap<PathBuf, Arc<IdTable>>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    if let Some(t) = memo.lock().unwrap_or_else(|e| e.into_inner()).get(lib_dir) {
        return Ok(t.clone());
    }
    let key = sdk_key(lib_dir, "ids-de-classe", &[]);
    let dir = crate::cache::native_cache_dir().join("sdk_ids");
    let file = dir.join(format!("{key}.tsv"));
    let table = match std::fs::read_to_string(&file).ok().and_then(|t| IdTable::from_string(&t)) {
        Some(t) => t,
        None => {
            let lib_dir_c = lib_dir.to_path_buf();
            let t = std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn(move || -> Result<IdTable, String> {
                    let (program, interner) = load_source_libraries(&lib_dir_c)?;
                    let compiled: Vec<bool> = program
                        .libraries
                        .iter()
                        .map(|l| l.uri.strip_prefix("dart:").is_some_and(|n| SOURCE_LIBRARIES.contains(&n)))
                        .collect();
                    Ok(crate::context::sdk_class_ids(&program, &interner, &compiled))
                })
                .map_err(|e| e.to_string())?
                .join()
                .map_err(|_| "pânico ao numerar as classes do SDK".to_string())??;
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            // Publicação atômica: outro processo pode estar gravando a mesma
            // chave (o conteúdo é o mesmo).
            let tmp = dir.join(format!("{key}.{}.tmp", std::process::id()));
            std::fs::write(&tmp, t.to_text()).map_err(|e| format!("{}: {e}", tmp.display()))?;
            if std::fs::rename(&tmp, &file).is_err() {
                let _ = std::fs::remove_file(&tmp);
            }
            t
        }
    };
    let t = Arc::new(table);
    memo.lock().unwrap_or_else(|e| e.into_inner()).insert(lib_dir.to_path_buf(), t.clone());
    Ok(t)
}

/// Uma biblioteca do SDK da fonte baixada e emitida.
#[derive(Debug, Clone)]
pub struct SdkLibrary {
    /// `dart:core`…
    pub uri: String,
    /// O LLVM IR do módulo dela.
    pub ir: String,
    /// Os membros recusados: (símbolo, motivo).
    pub rejected: Vec<(String, String)>,
    /// As tabelas de métodos que o módulo cita, mas não define (o perfil de
    /// produção, docs/NATIVO-PODA-DE-TABELAS.md §3.2): vão para o resumo da
    /// biblioteca. Vazio no desenvolvimento.
    pub tables: Vec<crate::pruning::ModuleTable>,
}

/// Baixa e emite cada uma das [`SOURCE_LIBRARIES`] no seu módulo (P5c):
/// mundo aberto, símbolos estáveis, os membros que não baixam recusados um a
/// um. É função só das fontes do SDK, da sobreposição e do compilador.
/// `production`: o perfil de produção ([`SdkProfile::Production`]), que nunca
/// recarrega. O descritor da área de globais vai sem os nomes dos slots
/// (`LlvmEmitter::with_lean_area`), toda função leva `optsize`
/// ([`with_optsize`]) e as tabelas de métodos ficam para a ligação
/// (`LlvmEmitter::with_tables_at_link`, docs/NATIVO-PODA-DE-TABELAS.md).
pub fn emit_sdk_libraries(lib_dir: &Path, production: bool) -> Result<Vec<SdkLibrary>, String> {
    use dartforge_types::table::{CoreTypes, TypeTable};
    let (program, interner) = load_source_libraries(lib_dir)?;
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = SOURCE_LIBRARIES
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
    if libs.len() != SOURCE_LIBRARIES.len() {
        return Err("biblioteca do SDK da fonte fora do programa".to_string());
    }
    let (bodies, _) =
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &libs);
    let base = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies);
    let base = base.with_source_sdk();
    // O SDK sai no modelo de exceções do programa (`alvo::excecoes_por_tabelas`).
    let excecoes_por_tabelas = crate::alvo::excecoes_por_tabelas()?;
    let raizes_por_mapas = crate::alvo::raizes_por_mapas()?;
    // O SDK sai na memória do programa (`alvo::arc_memory`).
    let arc_memory = crate::alvo::arc_memory()?;
    // O rastro simbólico (§13.14): as posições das funções do SDK e a tabela.
    let symbolic_stack_trace = crate::alvo::symbolic_stack_trace()?;
    let arc_prepared = crate::alvo::arc_prepared()?;
    let mut modules = Vec::new();
    for lib in libs {
        let uri = program.library(lib).uri.clone();
        let mut ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies)
            .with_source_sdk()
            .only_the_library(lib);
        ctx.arc_memory = arc_memory;
        if symbolic_stack_trace {
            ctx.link_debug();
            ctx.stack_trace = true;
        }
        debug_assert_eq!(ctx.class_ids, base.class_ids);
        let mut module = crate::lower::lower_program(&ctx);
        module.sdk_library = true;
        // As classes de `null` e do `Smi` em linha (`llvm/mod.rs`, `df.class`).
        module.runtime_cids = runtime_cids(&ctx);
        module.record = Some(registration_symbol(&uri));
        if let Some(e) = module.errors.first() {
            return Err(format!("{uri}: o módulo do SDK tem diagnóstico fora de membro: {e}"));
        }
        // O otimizador da HIR (`optimize/`) também no SDK: é onde fica o
        // código de `List`, `Map`, `String`… que os programas mais chamam.
        crate::optimize::prepare_for_emission(&mut module, arc_memory, excecoes_por_tabelas);
        modules.push((uri, module));
    }
    // A preparação ARC publicada: as bibliotecas juntas (o SDK não chama o
    // programa diretamente), na ordem fixa de `SOURCE_LIBRARIES`.
    if arc_prepared {
        let mut hir: Vec<_> = modules.iter_mut().map(|(_, m)| std::mem::take(m)).collect();
        let mut plans: Vec<std::collections::HashMap<_, _>> = hir.iter().map(|m| {
            m.functions.iter().map(|f| (f.symbol.clone(), crate::optimize::arc::DartFunctionPlan::default())).collect()
        }).collect();
        crate::optimize::arc::prepare_arc_dart_modules(&mut hir, &mut plans)
            .map_err(|e| format!("preparação ARC do SDK da fonte: {e}"))?;
        crate::optimize::arc::certify_arc_module_ports(&mut hir, &plans)
            .map_err(|e| format!("portas ARC do SDK da fonte: {e}"))?;
        for ((_, m), h) in modules.iter_mut().zip(hir) {
            *m = h;
        }
    }
    let mut exit = Vec::new();
    for (uri, mut module) in modules {
        // A DLL do SDK fica carregada o processo inteiro: os literais são
        // objetos estáticos também no JIT (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
        let ir = crate::llvm::LlvmEmitter::new(&module)
            .with_lean_area(production)
            .with_static_objects(true)
            // O SDK de produção só vai ao AOT; o de desenvolvimento (a DLL)
            // também serve ao JIT, que migra objetos na recarga.
            .with_body_without_outside(production)
            .with_tables_at_link(production)
            .with_roots_by_maps(raizes_por_mapas)
            .with_stack_trace(symbolic_stack_trace)
            .emit_all();
        if raizes_por_mapas {
            crate::llvm::verify_maps::verify(&ir).map_err(|e| format!("{uri}: {e}"))?;
        } else if crate::llvm::verify_shadow::linked() {
            crate::llvm::verify_shadow::verify(&ir).map_err(|e| format!("{uri}: {e}"))?;
        }
        let ir = if production { with_optsize(&ir) } else { ir };
        // Tabela de desenrolamento nas exceções por tabelas e nas raízes por
        // mapas (o percurso do coletor; ver `lib.rs`).
        let ir = if (excecoes_por_tabelas || raizes_por_mapas) && cfg!(unix) { with_uwtable(&ir) } else { ir };
        let ir = if cfg!(unix) && !excecoes_por_tabelas && !raizes_por_mapas { with_nounwind(&ir) } else { ir };
        // Só no Unix (macOS medido; o Windows não).
        let ir = if cfg!(unix) && production && !excecoes_por_tabelas && !raizes_por_mapas && !arc_memory {
            with_context_in_tls(&ir, ContextInTls::Free, "df.ctx.tls")
        } else {
            ir
        };
        let tables = if production { std::mem::take(&mut module.method_tables) } else { Vec::new() };
        exit.push(SdkLibrary { uri, ir, rejected: std::mem::take(&mut module.rejected), tables });
    }
    Ok(exit)
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
pub struct CompiledSdk {
    pub objects: Vec<PathBuf>,
    pub dll: PathBuf,
    pub import: PathBuf,
    /// Os resumos da poda das tabelas de métodos, um por biblioteca
    /// (`<library>.pruning`, docs/NATIVO-PODA-DE-TABELAS.md §3.3): só no
    /// perfil de produção.
    pub summaries: Vec<PathBuf>,
    /// Quanto levou para compilar (o custo a frio); `None` quando veio do
    /// cache.
    pub cold: Option<std::time::Duration>,
}

/// Os símbolos que um IR define com ligação externa (o que a DLL exporta).
fn defined_symbols(ir: &str, exit: &mut Vec<String>) {
    for l in ir.lines() {
        let Some(r) = l.strip_prefix("define ") else { continue };
        if r.starts_with("internal ") || r.starts_with("private ") || r.starts_with("linkonce_odr ") {
            continue;
        }
        let Some(i) = r.find('@') else { continue };
        let Some(f) = r[i..].find('(') else { continue };
        exit.push(r[i + 1..i + f].to_string());
    }
}

/// A chave do SDK compilado: blake3 das fontes do SDK que entram (as
/// bibliotecas da fonte, os patches `vm`, `libraries.json`), da
/// sobreposição, do compilador (`DARTFORGE_EMISSOR_HASH`, `build.rs`), da
/// identidade do Clang e das bandeiras.
fn sdk_key(lib_dir: &Path, clang_id: &str, args: &[&str]) -> String {
    fn join(dir: &Path, exit: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                join(&p, exit);
            } else if p.extension().is_some_and(|x| x == "dart" || x == "json") {
                exit.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in [
        "core",
        "async",
        "collection",
        "convert",
        "math",
        "internal",
        "typed_data",
        "isolate",
        "developer",
        "html/dartium",
        "_http",
        "io",
        "_internal/vm/lib",
        "_internal/vm/bin",
        "_internal/vm_shared/lib",
    ] {
        join(&lib_dir.join(d), &mut files);
    }
    files.push(lib_dir.join("libraries.json"));
    let overlay = dir_sobreposicao();
    join(&overlay, &mut files);
    files.sort();
    let mut h = blake3::Hasher::new();
    h.update(b"dartforge-sdk-fonte\0");
    h.update(env!("CARGO_PKG_VERSION").as_bytes());
    h.update(b"\0");
    h.update(env!("DARTFORGE_EMISSOR_HASH").as_bytes());
    h.update(b"\0");
    // O rastro de depuração muda o código (`llvm/mod.rs`).
    h.update(std::env::var("DARTFORGE_RASTRO").unwrap_or_default().as_bytes());
    h.update(b"\0");
    // O rastro simbólico acrescenta os rótulos e a tabela (`llvm/stack_trace.rs`);
    // o padrão não entra.
    if std::env::var("DARTFORGE_RASTRO_VM").is_ok_and(|v| v == "simbolico") {
        h.update(b"rastro=simbolico\0");
    }
    // O modelo de exceções muda o código (`optimize/tables.rs`). O padrão
    // não entra na chave: o SDK de sempre continua com a chave de sempre.
    if std::env::var("DARTFORGE_EXCECOES").is_ok_and(|v| v == "tabelas") {
        h.update(b"excecoes=tabelas\0");
    }
    // A memória ARC muda o código (as gravações pelo runtime, a ativação na
    // entrada); o padrão não entra. A versão é a da ABI de memória.
    if std::env::var("DARTFORGE_MEMORIA").is_ok_and(|v| v == "arc") {
        h.update(b"memoria=arc_v1\0");
    }
    // A preparação ARC publicada muda os corpos (tokens, cleanup); o
    // programa preparado só liga com este SDK.
    if crate::alvo::arc_prepared().unwrap_or(false) {
        h.update(b"arc=preparado_v1\0");
    }
    // O modo de raízes muda o código (`llvm/mod.rs`); o padrão não entra.
    if std::env::var("DARTFORGE_RAIZES").is_ok_and(|v| v == "mapas") {
        // `_v2`: com a tabela de desenrolamento no Unix (`with_uwtable`).
        h.update(b"raizes=mapas_v2\0");
        // O teto do orçamento decide que funções saem com mapa, e o build de
        // conferência acrescenta slots ao quadro.
        h.update(crate::alvo::maps_budget().to_string().as_bytes());
        h.update(if crate::alvo::check_roots() { b"conferir\0" } else { b"\0\0\0\0\0\0\0\0\0" });
    }
    // As sabotagens de teste mudam o código (`alvo::sabotage`).
    if let Ok(s) = std::env::var("DARTFORGE_SABOTAGEM")
        && !s.is_empty()
    {
        h.update(b"sabotagem=");
        h.update(s.as_bytes());
        h.update(b"\0");
    }
    // A conferência da tabela de efeitos instrumenta as chamadas às externs.
    if crate::llvm::externs::check_effects() {
        h.update(b"efeitos=conferir\0");
    }
    // As chaves de medida do emissor (`DARTFORGE_SEM_…`, que desligam uma
    // otimização para comparar antes e depois) mudam o código.
    let mut keys: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("DARTFORGE_SEM_"))
        .collect();
    keys.sort();
    for (k, v) in keys {
        h.update(format!("{k}={v}\0").as_bytes());
    }
        // A ligação de comparação (`DARTFORGE_LIGAR_COM_CLANG`) dá outra DLL.
    h.update(if crate::driver::link_with_clang() { b"clang" } else { b"lld" });
    h.update(b"\0");
    h.update(clang_id.as_bytes());
    for a in args {
        h.update(a.as_bytes());
        h.update(b"\0");
    }
    for a in &files {
        let rel = a
            .strip_prefix(lib_dir)
            .or_else(|_| a.strip_prefix(&overlay))
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

/// A geração dos módulos do SDK no perfil pedido.
///
/// Desenvolvimento: objetos `-O0` para a biblioteca compartilhada (código
/// independente de posição fora do Windows). Produção: bitcode `-O2` para a
/// otimização entre módulos na ligação do executável
/// (docs/PESQUISA-LLVM-DART-AOT.md §5: desenvolvimento em módulos separados,
/// produção com LTO).
///
/// ```
/// use dartforge_emit_native::gerador::Formato;
/// use dartforge_emit_native::sdk_modulo::{PerfilDoSdk, geracao_do_sdk};
/// assert_eq!(geracao_do_sdk(PerfilDoSdk::Producao).formato, Formato::Bitcode);
/// assert!(!geracao_do_sdk(PerfilDoSdk::Desenvolvimento).otimizar);
/// ```
pub fn sdk_generation(profile: SdkProfile) -> Generation {
    match profile {
        SdkProfile::Development => Generation { optimize: false, format: Format::Objeto, shared: true, cpu: None, prelink: 2 },
        SdkProfile::Production => Generation { optimize: true, format: Format::Bitcode, shared: false, cpu: None, prelink: 2 },
    }
}

/// O perfil em que o SDK da fonte é compilado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkProfile {
    /// Objetos `-O0` e a DLL (runtime + SDK) que o executável importa.
    Development,
    /// Bitcode ThinLTO `-O2`, ligado estaticamente no executável.
    Production,
}

/// A DLL do SDK da fonte (perfil de desenvolvimento) para o SDK instalado e
/// o Clang de sempre: é o que o JIT carrega (`crates/jit`, `DARTFORGE_SDK_DLL`).
pub fn dll_do_sdk_da_fonte() -> Result<PathBuf, String> {
    let dir = crate::sdk_do_dart()?;
    let clang = crate::driver::NativeDriverOptions::default().clang;
    Ok(compiled_sdk(&dir, &clang)?.dll)
}

/// Os objetos do SDK da fonte: do cache (`<cache native>/sdk/<key>/`), ou
/// compilados agora — uma vez por conteúdo, as bibliotecas em paralelo.
pub fn compiled_sdk(lib_dir: &Path, clang: &Path) -> Result<CompiledSdk, String> {
    compiled_sdk_in_profile(lib_dir, clang, SdkProfile::Development)
}

/// [`compiled_sdk`] no perfil pedido.
pub fn compiled_sdk_in_profile(lib_dir: &Path, clang: &Path, profile: SdkProfile) -> Result<CompiledSdk, String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Na produção, o bitcode do SDK sai do Clang, com o resumo do ThinLTO
    // (como as partes do programa, `driver.rs`): do gerador embutido ele
    // não tem o resumo, e o `lld` fazia a LTO completa do SDK num módulo
    // só, numa thread, antes das partes (docs/NATIVO-PRODUCAO-GRANDE.md).
    // `DARTFORGE_GERADOR` escolhe à mão. Sem o Clang (a distribuição sem o
    // toolchain do sistema), o gerador embutido.
    let gerador = if profile == SdkProfile::Production && std::env::var_os("DARTFORGE_GERADOR").is_none() {
        Gerador::clang_if_any(clang)
    } else {
        Gerador::escolher(clang)
    };
    let generation = sdk_generation(profile);
    let key = sdk_key(lib_dir, &gerador.identity()?, &[generation.description().as_str()]);
    let root = crate::cache::native_cache_dir().join("sdk");
    let dir = root.join(&key);
    let dll_name = format!("dfsdk_{}", &key[..16]);
    let dll_file = crate::alvo::shared_name(&dll_name);
    let ext_obj = crate::alvo::ext_object();
    let ready = |d: &Path, cold| CompiledSdk {
        objects: SOURCE_LIBRARIES.iter().map(|b| d.join(format!("{b}.{ext_obj}"))).collect(),
        // Os dois perfis: a produção poda na ligação (`pruning::assemble`), e os
        // dois podam a HIR do programa (`pruning::prune_hir`).
        summaries: SOURCE_LIBRARIES.iter().map(|b| d.join(format!("{b}.poda"))).collect(),
        dll: d.join(&dll_file),
        // No Windows liga-se contra a biblioteca de importação; no ELF e no
        // Mach-O, contra a própria biblioteca compartilhada.
        import: if crate::alvo::sistema() == crate::alvo::Sistema::Windows {
            d.join(format!("{dll_name}.lib"))
        } else {
            d.join(&dll_file)
        },
        cold,
    };
    if dir.join("pronto").is_file() {
        // A marca registra o último uso (a poda do cache a respeita).
        if let Ok(f) = std::fs::File::options().write(true).open(dir.join("pronto")) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        return Ok(ready(&dir, None));
    }
    let runtime_dll = crate::cache::RuntimeCache::para_dll()?;
    let t0 = std::time::Instant::now();
    let tmp = root.join(format!("{key}.tmp.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let libs = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn({
            let lib_dir = lib_dir.to_path_buf();
            move || emit_sdk_libraries(&lib_dir, profile == SdkProfile::Production)
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "pânico ao emitir o SDK da fonte".to_string())??;
    let mut exported = Vec::new();
    for lib in &libs {
        defined_symbols(&lib.ir, &mut exported);
    }
    exported.extend(
        dartforge_runtime::simbolos::NOMES.iter().filter(|n| **n != "main").map(|n| n.to_string()),
    );
    let mut def = format!("LIBRARY {dll_name}.dll\nEXPORTS\n");
    for s in &exported {
        def.push_str(&format!("  {s}\n"));
    }
    std::fs::write(tmp.join("exportados.def"), def).map_err(|e| e.to_string())?;
    let keep_ir = std::env::var_os("DARTFORGE_KEEP_IR").is_some();
    let mut resumo = String::new();
    for (b, lib) in SOURCE_LIBRARIES.iter().zip(&libs) {
        if keep_ir {
            std::fs::write(tmp.join(format!("{b}.ll")), &lib.ir).map_err(|e| e.to_string())?;
        }
        for (s, m) in &lib.rejected {
            resumo.push_str(&format!("{}\t{s}\t{m}\n", lib.uri));
        }
        // O resumo da poda: do IR que vira o bitcode (produção) ou o objeto
        // da DLL (desenvolvimento).
        let pruning = crate::pruning::summarize(&lib.ir, &lib.uri, &lib.tables);
        std::fs::write(tmp.join(format!("{b}.poda")), pruning.to_text()).map_err(|e| e.to_string())?;
    }
    std::fs::write(tmp.join("recusados.tsv"), resumo).map_err(|e| e.to_string())?;
    // Cada biblioteca num gerador, no máximo 4 ao mesmo tempo (a memória de
    // um módulo grande do LLVM é a medida, não os núcleos).
    let queue = std::sync::Mutex::new(SOURCE_LIBRARIES.iter().zip(&libs).collect::<Vec<_>>());
    let errors = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..4 {
            // A pilha do Clang (8 MB): passes recursivos do LLVM num módulo
            // grande passam da pilha padrão de uma thread do Rust.
            let r = std::thread::Builder::new().stack_size(64 << 20).spawn_scoped(s, || loop {
                let Some((b, lib)) = queue.lock().unwrap().pop() else { break };
                if let Err(e) = gerador.gerar(&lib.ir, generation, &tmp.join(format!("{b}.{ext_obj}"))) {
                    errors.lock().unwrap().push(format!("dart:{b}: {e}"));
                }
            });
            if let Err(e) = r {
                errors.lock().unwrap().push(format!("thread do gerador: {e}"));
            }
        }
    });
    if let Some(e) = errors.into_inner().unwrap().into_iter().next() {
        return Err(e);
    }
    // A DLL (desenvolvimento): os objetos do SDK e o runtime (variante sem
    // `main`). Em produção, os objetos (bitcode) vão direto para o
    // executável.
    if profile == SdkProfile::Production {
        // Nada a ligar: o bitcode entra na ligação de cada executável.
    } else if crate::alvo::sistema() == crate::alvo::Sistema::Linux {
        // O `ld.lld` direto com o sysroot de ligação (`ligador.rs`).
        let exported: Vec<String> =
            dartforge_runtime::simbolos::NOMES.iter().filter(|n| **n != "main").map(|n| n.to_string()).collect();
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        crate::ligador::link(
            &crate::ligador::ld_lld(clang),
            crate::ligador::SysrootLinux::locate(clang)?,
            &crate::ligador::Linkage {
                product: crate::ligador::Product::Shared { soname: &dll_file, exported: &exported },
                entries,
                origin_rpath: false,
                lto: false,
                cpu: None,
                prune: false,
                keep_debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("a ligação da DLL do SDK da fonte falhou: {e}"))?;
    } else if crate::alvo::sistema() == crate::alvo::Sistema::Windows && crate::driver::link_with_clang() {
        // A ligação de antes do N15 (o driver do Clang, `link.exe` e a CRT do
        // Visual C++), só para comparação (`DARTFORGE_LIGAR_COM_CLANG`).
        let exit = std::process::Command::new(clang)
            .current_dir(&tmp)
            .arg("-shared")
            .args(SOURCE_LIBRARIES.iter().map(|b| format!("{b}.{ext_obj}")))
            .arg(&runtime_dll.lib_path)
            .arg("-Wl,/DEF:exportados.def")
            .args(crate::alvo::argumentos_de_ligacao())
            .arg("-o")
            .arg(&dll_file)
            .output()
            .map_err(|e| format!("Clang: {e}"))?;
        if !exit.status.success() {
            let text = String::from_utf8_lossy(&exit.stderr);
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).take(40).collect();
            return Err(format!("a ligação da DLL do SDK da fonte falhou ({}):\n{}", exit.status, lines.join("\n")));
        }
    } else if crate::alvo::sistema() == crate::alvo::Sistema::Windows {
        // O `lld-link` direto, com as bibliotecas de importação e a CRT
        // mínima do dartforge (`ligador_windows.rs`); o `.def` exporta o
        // runtime e o SDK, e a biblioteca de importação sai ao lado da DLL.
        use crate::ligador_windows as lw;
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        lw::link(
            &lw::lld_link(clang),
            lw::SysrootWindows::locate(clang)?,
            &lw::Linkage {
                product: lw::Product::Dll { def: &tmp.join("exportados.def") },
                entries,
                lto: false,
                cpu: None,
                prune: false,
                debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("a ligação da DLL do SDK da fonte falhou: {e}"))?;
    } else {
        // O `ld64.lld` direto com os `.tbd` do sysroot (`ligador_macos.rs`):
        // cada nome do runtime entra como não definido, para o ligador trazer
        // da `staticlib` também o que o SDK não usa (o programa usa).
        use crate::ligador_macos as lm;
        let exported: Vec<String> =
            dartforge_runtime::simbolos::NOMES.iter().filter(|n| **n != "main").map(|n| n.to_string()).collect();
        let mut entries: Vec<PathBuf> = SOURCE_LIBRARIES.iter().map(|b| tmp.join(format!("{b}.{ext_obj}"))).collect();
        entries.push(runtime_dll.lib_path.clone());
        let install_name = format!("@rpath/{dll_file}");
        lm::link(
            &lm::ld64_lld(clang),
            lm::SysrootMacos::locate()?,
            &lm::Linkage {
                product: lm::Product::Dynamic { install_name: &install_name, exported: &exported },
                entries,
                executable_rpath: false,
                lto: false,
                cpu: None,
                prune: false,
                keep_debug_info: false,
                exit: &tmp.join(&dll_file),
            },
        )
        .map_err(|e| format!("a ligação da DLL do SDK da fonte falhou: {e}"))?;
    }
    std::fs::write(tmp.join("pronto"), b"").map_err(|e| e.to_string())?;
    if std::fs::rename(&tmp, &dir).is_err() {
        // Outro processo terminou antes: fica o dele.
        let _ = std::fs::remove_dir_all(&tmp);
        if !dir.join("pronto").is_file() {
            return Err(format!("não foi possível instalar o SDK compilado em {}", dir.display()));
        }
    }
    prune_old_sdks(&root, &dir);
    Ok(ready(&dir, Some(t0.elapsed())))
}

/// Remove do cache os SDKs compilados de versões anteriores do dartforge
/// (cada um passa de 1 GB): ficam os quatro mais recentes (os perfis de
/// desenvolvimento e de produção desta versão e da anterior), e nenhum
/// usado na última hora (a marca `ready` guarda o último uso) — outro
/// processo pode estar ligando contra ele. Uma
/// remoção que falha (a biblioteca aberta no Windows) fica para a próxima.
fn prune_old_sdks(root: &Path, current: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else { return };
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p != current && p.join("pronto").is_file())
        .filter_map(|p| Some((std::fs::metadata(p.join("pronto")).ok()?.modified().ok()?, p)))
        .collect();
    dirs.sort_by(|a, b| b.0.cmp(&a.0));
    let hour = std::time::Duration::from_secs(3600);
    for (when, dir) in dirs.into_iter().skip(3) {
        if when.elapsed().is_ok_and(|d| d > hour) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// O resultado do lowering de um membro do SDK da fonte.
#[derive(Debug, Clone)]
pub struct SdkMember {
    pub library: String,
    pub symbol: String,
    /// Diagnósticos do lowering (vazio: baixou), ou o pânico.
    pub diagnostics: Vec<String>,
}

/// Medição de P5c: baixa cada função das [`SOURCE_LIBRARIES`] (mundo
/// aberto) e devolve, por membro, os diagnósticos do lowering.
pub fn measure_sdk_lowering(lib_dir: &Path) -> Result<Vec<SdkMember>, String> {
    use dartforge_intern::Interner;
    use dartforge_types::table::{CoreTypes, TypeTable};

    let sdk = load_native_sdk(lib_dir)?;
    let tmp = std::env::temp_dir().join(format!("dartforge-lower-sdk-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let entry = tmp.join("main.dart");
    let mut source = String::new();
    for b in SOURCE_LIBRARIES {
        source.push_str(&format!("import 'dart:{b}';\n"));
    }
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| format!("{}: {e}", entry.display()))?;
    let mut interner = Interner::new();
    let (program, diags_carga) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
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
        .filter(|(_, l)| l.uri.strip_prefix("dart:").is_some_and(|n| SOURCE_LIBRARIES.contains(&n)))
        .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32))
        .collect();
    let (bodies, _) =
        dartforge_types::infer_bodies_das_bibliotecas(&program, &interner, &mut table, &core, &mut outline, &libs);
    let ctx = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
    let mut exit = Vec::new();
    for f in 0..program.functions.len() {
        if !libs.contains(&program.functions[f].library) {
            continue;
        }
        let symbol = crate::lower::symbol_of(&ctx, f);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut m = crate::hir::Module::new();
            crate::lower::lower_function(&ctx, &mut m, f);
            m.errors
        }));
        let diagnostics = match r {
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
        exit.push(SdkMember {
            library: program.library(program.functions[f].library).uri.clone(),
            symbol,
            diagnostics,
        });
    }
    Ok(exit)
}

/// O IR de uma biblioteca do SDK com `optsize` em toda função definida (o
/// perfil de produção): o LLVM pesa tamanho nas decisões de inlining,
/// desenrolamento e vetorização. Medido em 2026-09-29: −453 KB no olá
/// mundo e −532 KB no servidor HTTP, sem diferença além do ruído em
/// `bench/performance/json.dart` e `bench/simd/bin/blend.dart`
/// (docs/PLANO-TAMANHO-DESEMPENHO.md §1). O código do SDK que a LTO
/// embute no programa segue os atributos de quem chama.
///
/// ```
/// use dartforge_emit_native::sdk_modulo::com_optsize;
/// let ir = "define i64 @f(i64 %x) comdat {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline {\n";
/// assert_eq!(
///     com_optsize(ir),
///     "define i64 @f(i64 %x) optsize comdat {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline optsize {\n"
/// );
/// ```
pub fn with_optsize(ir: &str) -> String {
    with_attribute(ir, " optsize")
}

/// O IR com `uwtable` em toda função definida: nas exceções por tabelas dos
/// alvos Itanium (ELF, Mach-O) o desenrolador só atravessa um quadro que
/// tem tabela de desenrolamento, e o LLVM só a emite para a função marcada
/// (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.11). No COFF x64 toda
/// função já tem a sua.
pub fn with_uwtable(ir: &str) -> String {
    with_attribute(ir, " uwtable")
}

/// O IR com `nounwind` em toda função definida que ainda não o tem: nas
/// exceções por checagem com a pilha-sombra (o padrão), nada desenrola um
/// quadro do código Dart — a exceção é uma bandeira no contexto, o rastro
/// vem da pilha-sombra e o runtime de produção aborta no pânico. Sem o
/// atributo, o LLVM emite o CFI de toda função que não é folha e, no
/// Mach-O arm64 sem ponteiro de quadro, isso vira DWARF no `__eh_frame`
/// (1,4 MB do `new_sali/backend`). As exceções por tabelas
/// ([`with_uwtable`]) e as raízes por mapas (`_Unwind_Backtrace`)
/// atravessam os quadros e não passam por aqui.
///
/// ```
/// use dartforge_emit_native::sdk_modulo::com_nounwind;
/// let ir = "define i64 @f(i64 %x) {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline nounwind {\n";
/// assert_eq!(
///     com_nounwind(ir),
///     "define i64 @f(i64 %x) nounwind {\n  ret i64 %x\n}\ndefine internal void @g() alwaysinline nounwind {\n"
/// );
/// ```
pub fn with_nounwind(ir: &str) -> String {
    with_absent_attribute(ir, " nounwind")
}

/// Como o código Dart acha o contexto da thread em [`with_context_in_tls`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextInTls {
    /// O ajudante copiado em cada função (`alwaysinline`): o desempenho.
    Inline,
    /// O LLVM decide.
    Free,
    /// Uma chamada (`noinline`): o tamanho, no programa grande.
    PerCall,
}

/// O IR com o contexto da thread (`dartforge_contexto`, uma função Rust com
/// quadro, a variável `thread_local!` e a conferência do limite da pilha)
/// guardado numa variável `thread_local` do próprio módulo: o prólogo de cada função lê a variável (no Mach-O, o
/// `tlv_get_addr`, que não mexe nos outros registradores) e só chama o
/// runtime na primeira vez em cada thread. O endereço do contexto de uma
/// thread não muda enquanto ela vive (`runtime/src/gc_raizes.rs`). Só no
/// AOT: o JIT não tem `thread_local` nos módulos dele. O `fib(32)` do
/// `bench/comparison` passava 1/3 do tempo na função do runtime.
///
/// ```
/// use dartforge_emit_native::sdk_modulo::{com_contexto_em_tls, ContextoEmTls};
/// let ir = "define i64 @f() {\n  %ctx = call ptr @dartforge_contexto()\n  ret i64 0\n}\n";
/// let novo = com_contexto_em_tls(ir, ContextoEmTls::EmLinha, "df.ctx.tls");
/// assert!(novo.contains("%ctx = call ptr @df.contexto()") && novo.contains("@df.ctx.tls = internal thread_local global ptr null"));
/// let sem = "define void @g() {\n  ret void\n}\n";
/// assert_eq!(com_contexto_em_tls(sem, ContextoEmTls::EmLinha, "df.ctx.tls"), sem);
/// ```
///
/// `variable` é o nome da variável: o programa em partes a promove a
/// externa (`partition.rs`), e o nome dela não pode ser o da interna de um
/// módulo do SDK (como o `@df.area_id.program`).
pub fn with_context_in_tls(ir: &str, mode: ContextInTls, variable: &str) -> String {
    const CALL: &str = "call ptr @dartforge_contexto()";
    if !ir.contains(CALL) {
        return ir.to_string();
    }
    let attribute = match mode {
        ContextInTls::Inline => " alwaysinline",
        ContextInTls::Free => "",
        ContextInTls::PerCall => " noinline",
    };
    let mut s = ir.replace(CALL, "call ptr @df.contexto()");
    s.push_str(&format!(
        "\n@{variable} = internal thread_local global ptr null, align 8\n\
         define internal ptr @df.contexto() nounwind{attribute} {{\n\
         \x20 %p = call ptr @llvm.threadlocal.address.p0(ptr @{variable})\n\
         \x20 %c = load ptr, ptr %p, align 8\n\
         \x20 %z = icmp eq ptr %c, null\n\
         \x20 br i1 %z, label %lento, label %pronto\n\
         lento:\n\
         \x20 %n = {CALL}\n\
         \x20 store ptr %n, ptr %p, align 8\n\
         \x20 ret ptr %n\n\
         pronto:\n\
         \x20 ret ptr %c\n\
         }}\n"
    ));
    if !s.contains("declare ptr @llvm.threadlocal.address.p0(ptr") {
        s.push_str("declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)\n");
    }
    s
}

/// Acrescenta um atributo de função (com o espaço na frente) a cada linha
/// `define`.
fn with_attribute(ir: &str, attribute: &str) -> String {
    with_attribute_if(ir, attribute, false)
}

/// [`with_attribute`] só nas linhas `define` que ainda não têm o atributo.
fn with_absent_attribute(ir: &str, attribute: &str) -> String {
    with_attribute_if(ir, attribute, true)
}

fn with_attribute_if(ir: &str, attribute: &str, only_absent: bool) -> String {
    let mut s = String::with_capacity(ir.len() + ir.len() / 64);
    for l in ir.split_inclusive('\n') {
        if only_absent && l.starts_with("define ") && l.contains(attribute) {
            s.push_str(l);
            continue;
        }
        // Os atributos vêm depois dos parâmetros e antes do `comdat` (ou da
        // chave que abre o corpo).
        // O `gc "…"` (raízes por mapas) e a personalidade (`personality ptr
        // @…`, das exceções por tabelas) vêm depois dos atributos.
        let point = l
            .starts_with("define ")
            .then(|| l.find(" comdat").or_else(|| l.find(" gc \"")).or_else(|| l.find(" personality ")).or_else(|| l.rfind(" {")))
            .flatten();
        match point {
            Some(i) => {
                s.push_str(&l[..i]);
                s.push_str(attribute);
                s.push_str(&l[i..]);
            }
            None => s.push_str(l),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;



    /// A sobreposição existe, troca os quatro arquivos e aponta para arquivos
    /// que existem.
    #[test]
    fn overlay_loads() {
        crate::sdk_tests();
        let sdk = load_native_sdk(crate::sdk_tests()).unwrap();
        // 26 com o `mirrors_patch.dart` (o `reflectClass` do executor de
        // builders, corpus/native/45); 29 com `double.dart`, `integers.dart`
        // e `convert_patch.dart` (o JSON, docs/NATIVO-PLANO.md); 30 com o
        // `convert/json.dart` (a escrita em UTF-8, §13.2).
        assert_eq!(sdk.substituicoes.len(), 30);
        for b in SOURCE_LIBRARIES {
            assert!(sdk.library(b).is_some(), "dart:{b} fora do layout");
        }
    }

    /// A medição de 5a (docs/NATIVO-PLANO.md §7.4): imprime, por biblioteca,
    /// as funções e os diagnósticos de inferência dos corpos. Lenta (infere o
    /// SDK inteiro); roda à parte:
    /// `cargo test -p dartforge-emit-native medir_inferencia -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn measure_source_library_inference() {
        crate::sdk_tests();
        let table = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| measure_sdk_inference(crate::sdk_tests()).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut total = 0;
        for b in &table {
            println!("{:<18} funções {:>5}  diagnósticos {:>5}", b.library, b.functions, b.diagnostics.len());
            total += b.diagnostics.len();
            let mut by_code: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for d in &b.diagnostics {
                let key: String = d.split(':').next().unwrap_or(d).chars().take(60).collect();
                *by_code.entry(key).or_default() += 1;
            }
            let mut v: Vec<_> = by_code.into_iter().collect();
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
    /// `cargo test -p dartforge-emit-native emit_the_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn emit_the_sdk() {
        crate::sdk_tests();
        let t = std::time::Instant::now();
        let libs = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| emit_sdk_libraries(crate::sdk_tests(), false).unwrap())
            .unwrap()
            .join()
            .unwrap();
        println!("emissão: {:?}", t.elapsed());
        let mut reasons: std::collections::BTreeMap<String, usize> = Default::default();
        let mut total = 0;
        for l in &libs {
            println!("{:<18} IR {:>9} bytes  recusados {:>5}", l.uri, l.ir.len(), l.rejected.len());
            total += l.rejected.len();
            for (_, m) in &l.rejected {
                *reasons.entry(m.chars().take(100).collect()).or_default() += 1;
            }
        }
        println!("recusados: {total}");
        let mut v: Vec<_> = reasons.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        for (m, n) in v.iter().take(60) {
            println!("{n:>5}  {m}");
        }
        if let Ok(dir) = std::env::var("DARTFORGE_SDK_IR_DIR") {
            for (b, l) in SOURCE_LIBRARIES.iter().zip(&libs) {
                std::fs::write(Path::new(&dir).join(format!("{b}.ll")), &l.ir).unwrap();
                let r: String = l.rejected.iter().map(|(s, m)| format!("{s}\t{m}\n")).collect();
                std::fs::write(Path::new(&dir).join(format!("{b}.recusados.tsv")), r).unwrap();
            }
        }
        if std::env::var("DARTFORGE_SDK_COMPILAR").is_ok_and(|v| v == "1") {
            let clang = crate::driver::NativeDriverOptions::default().clang;
            let s = compiled_sdk(crate::sdk_tests(), &clang).unwrap();
            println!("dll: {:?} (frio: {:?})", s.dll, s.cold);
        }
    }

    /// Compila e executa `DARTFORGE_PROGRAMA` com o SDK da fonte (P5c):
    /// `cargo test -p dartforge-emit-native compilar_com_o_sdk -- --ignored --nocapture`.
    #[test]
    #[ignore = "manual; roda à parte"]
    fn compile_with_source_sdk() {
        let Ok(p) = std::env::var("DARTFORGE_PROGRAMA") else { return };
        let entry = PathBuf::from(p);
        let exit = entry.with_extension("exe");
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let optimize = std::env::var("DARTFORGE_OTIMIZAR").is_ok_and(|v| v == "1");
                let options = crate::CompileOptions { sdk: Some(crate::sdk_tests()), packages: None, timings: true, optimize: optimize, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
                crate::compilar(&entry, &exit, &options).map(|_| exit)
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
    /// do DartForge (docs/NATIVO-PLANO.md §7.9).
    #[test]
    #[ignore = "compila o SDK da fonte (lento a frio); roda no CI"]
    fn producao_e_um_executavel_autocontido() {
        // O SDK instalado (no CI, o do `DART_HOME`); sem ele, o teste só é
        // pulado fora do CI — no CI, ausência é falha, não sucesso vazio.
        let sdk_dir = crate::sdk_tests().to_path_buf();
        if !sdk_dir.join("libraries.json").is_file() {
            assert!(std::env::var_os("CI").is_none(), "SDK do Dart ausente no CI ({})", sdk_dir.display());
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() {\n  print('oi');\n  print([1, 2, 3].map((x) => x * 2).toList());\n  print({'a': 1});\n}\n").unwrap();
        let exe = dir.path().join(crate::alvo::executable_name("prog"));
        let (e2, x2) = (entry.clone(), exe.clone());
        let r = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {

                let options = crate::CompileOptions { sdk: Some(&sdk_dir), packages: None, timings: false, optimize: true, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
                crate::compile_with(&e2, &x2, &options)
            })
            .unwrap()
            .join()
            .unwrap();
        r.expect("compilar em produção");
        let alone = tempfile::tempdir().unwrap();
        let copy_inst = alone.path().join(crate::alvo::executable_name("prog"));
        std::fs::copy(&exe, &copy_inst).unwrap();
        let only_one: Vec<_> = std::fs::read_dir(alone.path()).unwrap().collect();
        assert_eq!(only_one.len(), 1, "a pasta tem só o executável");
        let o = std::process::Command::new(&copy_inst).current_dir(alone.path()).output().unwrap();
        assert!(o.status.success(), "código {:?}; stderr: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        assert_eq!(String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"), "oi\n[2, 4, 6]\n{a: 1}\n");
    }

    /// O teste de ausência de código (docs/NATIVO-PODA-DE-TABELAS.md §4, o
    /// `deadstrip.test.ts` do scriptc): `corpus/native/81_poda_de_tabelas.dart`
    /// tem uma classe instanciada com membros que nada usa. No executável de
    /// produção (o mapa da ligação) não fica símbolo nenhum deles — nem o
    /// corpo nem as entradas `$c`/`$tc`/`$g`/`$s`/`$tearm` —, ficam as
    /// entradas dos usados, e a saída é a da VM.
    #[test]
    #[ignore = "compila o SDK da fonte (lento a frio); roda no CI"]
    fn pruning_removes_unused_members() {
        let sdk_dir = crate::sdk_tests().to_path_buf();
        if !sdk_dir.join("libraries.json").is_file() {
            assert!(std::env::var_os("CI").is_none(), "SDK do Dart ausente no CI ({})", sdk_dir.display());
            return;
        }
        let entry = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/native/81_poda_de_tabelas.dart");
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join(crate::alvo::executable_name("poda"));
        crate::ligador::request_link_maps();
        let (e2, x2) = (entry.clone(), exe.clone());
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || {
                let options = crate::CompileOptions { sdk: Some(&sdk_dir), packages: None, timings: false, optimize: true, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
                crate::compile_with(&e2, &x2, &options)
            })
            .unwrap()
            .join()
            .unwrap()
            .expect("compilar em produção");
        let map = std::fs::read_to_string(exe.with_extension("map")).expect("o mapa da ligação");
        let remained: Vec<&str> = map.lines().filter(|l| l.contains("naoUsadoPoda")).collect();
        assert!(remained.is_empty(), "membros não usados no executável:\n{}", remained.join("\n"));
        // As entradas dos usados, uma de cada forma (o setter `x=` leva o
        // `=` escapado; o tear-off estático é o `$tearm`, o dinâmico passa
        // pelo `$g`).
        let used_set = [
            "Poda.usadoDinamicoPoda$c",
            "PodaFilha.usadoDinamicoPoda$c",
            "lidoPorGetterPoda$g",
            "gravadoPorSetterPoda$3d$s",
            "tearoffUsadoPoda$tearm",
            "tearoffDinamicoPoda$g",
            "genericoUsadoPoda$c",
        ];
        for used in used_set {
            assert!(map.contains(used), "{used} fora do mapa da ligação");
        }
        let o = std::process::Command::new(&exe).output().unwrap();
        assert!(o.status.success(), "código {:?}; stderr: {}", o.status.code(), String::from_utf8_lossy(&o.stderr));
        assert_eq!(
            String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n"),
            "7\n13\n13\n7\n101\nt\nPoda(3, 9)\n4\n102\n13\n7\nnsm Symbol(\"qualquerCoisaPoda\") [1, 2] 1\nNoSuchMethodError\n"
        );
    }

    /// A medição de P5c: quantos membros do SDK da fonte o lowering baixa.
    /// `cargo test -p dartforge-emit-native medir_lowering -- --ignored --nocapture`.
    #[test]
    #[ignore = "medição; roda à parte"]
    fn measure_source_library_lowering() {
        crate::sdk_tests();
        // Sem trocar o gancho de pânico do processo: um gancho silencioso
        // aqui (como era) calava o pânico de TODOS os testes que rodam junto
        // — no job do macOS, `producao_e_um_executavel_autocontido` e
        // `pruning_removes_unused_members` falhavam sem mensagem nenhuma. Os
        // pânicos do lowering pegos por `catch_unwind` só fazem barulho.
        let members = std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(|| measure_sdk_lowering(crate::sdk_tests()).unwrap())
            .unwrap()
            .join()
            .unwrap();
        let mut by_lib: std::collections::BTreeMap<&str, (usize, usize)> = Default::default();
        let mut by_construct: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
        for m in &members {
            let e = by_lib.entry(&m.library).or_default();
            e.0 += 1;
            if m.diagnostics.is_empty() {
                e.1 += 1;
            }
            let mut seen_set = std::collections::BTreeSet::new();
            for d in &m.diagnostics {
                let d = d.strip_prefix(crate::UNSUPPORTED_PREFIX).unwrap_or(d);
                let key: String = d.rsplit_once(" (").map_or(d, |(a, _)| a).chars().take(90).collect();
                let c = by_construct.entry(key.clone()).or_default();
                c.1 += 1;
                if seen_set.insert(key) {
                    c.0 += 1;
                }
            }
        }
        for (l, (n, ok)) in &by_lib {
            println!("{l:<20} membros {n:>5}  baixados {ok:>5}");
        }
        let mut v: Vec<_> = by_construct.into_iter().collect();
        v.sort_by(|a, b| b.1.0.cmp(&a.1.0));
        for (c, (members, occ)) in v.iter().take(80) {
            println!("{members:>5} {occ:>6}  {c}");
        }
        if let Ok(file) = std::env::var("DARTFORGE_MEDIR_SAIDA") {
            let mut t = String::new();
            for m in &members {
                t.push_str(&format!("{}\t{}\t{}\n", m.library, m.symbol, m.diagnostics.join(" | ")));
            }
            std::fs::write(file, t).unwrap();
        }
    }
}
