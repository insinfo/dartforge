//! Compilador nativo LLVM do DartForge sobre a trilha nova.

pub mod alvo;
pub mod erasure;
pub mod cache;
pub mod object_cache;
pub mod context;
pub mod driver;
pub mod source;
mod finalizables;
pub mod gcmap;
pub mod compact_stack_trace;
pub mod gerador;
pub mod ligador;
pub mod ligador_macos;
pub mod ligador_windows;
#[cfg(feature = "llvm-embutido")]
pub mod distributed_lto;
pub mod hir;
pub mod llvm;
pub mod lower;
pub mod native_world;
pub mod partition;
pub mod natives;
pub mod optimize;
pub mod pruning;
pub mod resumo;
pub mod sdk_modulo;

use context::Context;
use dartforge_elements::Program;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Opções de compilação nativa.
pub struct CompileOptions<'a> {
    pub sdk: Option<&'a Path>,
    pub packages: Option<&'a Path>,
    pub timings: bool,
    pub optimize: bool,
    /// Versão de linguagem corrente (`--version-linguagem`, docs/VERSOES-LINGUAGEM.md);
    /// `None` = a da ferramenta (3.13).
    pub versao_linguagem: Option<dartforge_frontend::LanguageVersion>,
    /// Experimentos pedidos explicitamente para bibliotecas na versão corrente.
    pub experimentos: Vec<dartforge_frontend::Feature>,
    /// J05: tabelas de linha para o depurador nativo (`llvm/debug_info.rs`).
    pub depuracao: bool,
    /// Quem produz as fontes geradas do projeto (o motor de build,
    /// `crates/build`) a partir do programa carregado sem elas — o mesmo
    /// contrato do `compile-js` (`dartforge_emit_js::Gerador`). `None`: o
    /// projeto não usa builders, ou quem chama não os liga.
    pub gerador: Option<Gerador<'a>>,
    /// A CPU-alvo do código gerado (`--cpu`, [`gerador::Cpu`]); `None`, a
    /// base do alvo (no x86-64, SSE2).
    pub cpu: Option<gerador::Cpu>,
}

/// Veja [`CompileOptions::gerador`].
pub type Gerador<'a> = &'a (dyn Fn(
    &dartforge_elements::model::Program,
    &Interner,
) -> Result<std::sync::Arc<dartforge_elements::gerado::Geracao>, String>
         + Sync);

/// Tempo de cada fase da emissão (tudo antes do Clang).
#[derive(Debug, Clone, Copy, Default)]
pub struct EmissionTimes {
    pub frontend: Duration,
    /// O mundo fechado do programa (`native_world.rs`).
    pub world: Duration,
    /// Funções do programa vivas no mundo fechado / todas (0/0 sem poda).
    pub live_functions: (usize, usize),
    pub hir: Duration,
    pub llvm_ir: Duration,
}

/// O LLVM IR de um programa, pronto para o Clang.
#[derive(Debug, Clone)]
pub struct IrEmitido {
    pub texto: String,
    pub tempos: EmissionTimes,
    /// Bytes de `text` que são corpo de função do SDK (`dart:`); ver
    /// `sdk_bytes`.
    pub sdk_byte_count: usize,
}

impl IrEmitido {
    /// As linhas de `--timings` da emissão, no formato de [`compilar`].
    pub fn imprimir_tempos(&self) {
        eprintln!("  Front-end: {:?}", self.tempos.frontend);
        let (live_set, all) = self.tempos.live_functions;
        eprintln!("  Mundo:     {:?} ({live_set} de {all} funções do programa vivas)", self.tempos.world);
        eprintln!("  HIR:       {:?}", self.tempos.hir);
        eprintln!("  LLVM IR:   {:?}", self.tempos.llvm_ir);
        let pct = if self.texto.is_empty() { 0.0 } else { 100.0 * self.sdk_byte_count as f64 / self.texto.len() as f64 };
        eprintln!("  IR:        {} bytes, {} do SDK ({pct:.1}%)", self.texto.len(), self.sdk_byte_count);
    }
}

/// Começo de todo diagnóstico de construto que o lowering não sabe baixar
/// (`FnBuilder::unsupported`, N1). [`construtos_do_erro`] é quem o lê.
pub const UNSUPPORTED_PREFIX: &str = "não suportado no backend nativo: ";

/// [`pruning::prune_hir`] com os resumos das bibliotecas do SDK do perfil (o
/// SDK sai do cache, ou é compilado agora, como a ligação faria em
/// seguida). `None` quando o módulo não usa o SDK da fonte.
fn prune_program_hir(module: &mut hir::Module, production: bool) -> Result<Option<pruning::HirPruning>, String> {
    if module.sdk_library || module.sdk_records.is_empty() {
        return Ok(None);
    }
    let profile = if production { sdk_modulo::SdkProfile::Production } else { sdk_modulo::SdkProfile::Development };
    let clang = driver::NativeDriverOptions::default().clang;
    // O mesmo diretório e o mesmo Clang da ligação (`driver::compile_and_link`):
    // a mesma chave de cache.
    let sdk = sdk_modulo::compiled_sdk_in_profile(&sdk_do_dart()?, &clang, profile)?;
    if !sdk.summaries.iter().all(|r| r.is_file()) {
        return Ok(None);
    }
    let summaries = pruning::read_summaries(&sdk.summaries)?;
    let refs: Vec<&pruning::Summary> = summaries.iter().map(|r| r.as_ref()).collect();
    Ok(Some(pruning::prune_hir(module, &refs)))
}

/// A versão do SDK em `root` como a VM a escreve em `Platform.version`
/// (`VM/runtime/vm/version_in.cc:33`, `{{VERSION_STR}} ({{CHANNEL}})
/// ({{COMMIT_TIME}})`): `3.6.2 (stable) (Wed Jan 29 01:20:39 2025 -0800)`,
/// sem o ` on "<sistema>"` que o runtime põe. A data do commit só o próprio
/// `dart` sabe: a primeira compilação a pergunta (`dart --version`) e a
/// guarda no cache nativo, pela versão; sem o `dart`, só o número do arquivo
/// `version` (o `sqlite3` imprime o `Platform.version`).
fn sdk_version(root: &Path) -> Option<String> {
    let number = std::fs::read_to_string(root.join("version")).ok()?.trim().to_string();
    let saved = cache::native_cache_dir().join(format!("versao_do_sdk_{number}.txt"));
    if let Ok(v) = std::fs::read_to_string(&saved)
        && v.starts_with(&number)
    {
        return Some(v.trim().to_string());
    }
    let dart = root.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let complete = std::process::Command::new(&dart)
        .arg("--version")
        .output()
        .ok()
        .and_then(|s| {
            // `Dart SDK version: 3.6.2 (stable) (…) on "windows_x64"` (na
            // saída padrão desde o 2.15; antes, na de erro).
            let text = format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
            let rest = text.split("Dart SDK version: ").nth(1)?;
            let v = rest.split(" on \"").next()?.trim().to_string();
            v.starts_with(&number).then_some(v)
        });
    match complete {
        Some(v) => {
            let _ = std::fs::create_dir_all(cache::native_cache_dir());
            let _ = std::fs::write(&saved, &v);
            Some(v)
        }
        None => Some(number),
    }
}

/// A partir de quantas funções no módulo HIR do programa a produção o trata
/// como grande (`compile`, campos e ajudantes por chamada): o
/// new_sali/backend tem ~200 mil; os programas do corpus e os benchmarks,
/// poucos milhares.
pub const LARGE_PROGRAM_FUNCTIONS: usize = 50_000;

/// O erro de um módulo com diagnósticos do lowering.
///
/// A primeira linha é `error de compilação: ` e o primeiro diagnóstico **sem a
/// posição**: é a chave de agrupamento do harness, e o mesmo construto em
/// programas diferentes tem de cair no mesmo grupo. Depois vem uma linha por
/// diagnóstico, todos eles, com a posição e recuados dois espaços.
fn compile_error(errors: &[String]) -> String {
    let first_one = &errors[0];
    let resumo = first_one.rsplit_once(" (").map_or(first_one.as_str(), |(a, _)| a);
    let mut text = format!("erro de compilação: {resumo}");
    for e in errors {
        text.push_str("\n  ");
        text.push_str(e);
    }
    text
}

/// Os construtos não suportados de um erro de [`emitir_ir`]/[`compilar`], ou
/// de um texto que o contenha (o stderr que o harness monta com ele): um por
/// diagnóstico, na ordem, sem a posição. Vazio quando o erro não é de
/// construto (carga, pânico, verificador da HIR).
pub fn construtos_do_erro(text: &str) -> Vec<String> {
    text
        .lines()
        .filter_map(|l| {
            let d = l.strip_prefix("  ")?.strip_prefix(UNSUPPORTED_PREFIX)?;
            Some(d.rsplit_once(" (").map_or(d, |(a, _)| a).to_string())
        })
        .collect()
}

/// Carrega, analisa e baixa um programa até o LLVM IR, sem Clang nem ligação.
///
/// É a parte de [`compilar`] que é nossa: o teste de determinismo do harness
/// compara só isto (`dartforge-differential determinismo --native`), e
/// `compile-native --emit-ir` grava isto. Diagnósticos de carga vão na
/// mensagem de erro, e não no stderr, para não se entrelaçarem quando várias
/// emissões rodam no mesmo processo. A primeira linha é o primeiro
/// diagnóstico, porque é ela que o relatório do harness mostra e agrupa — uma
/// contagem ("1 erro(s)") juntava num grupo só causas diferentes.
///
/// Construto não suportado também é `Err`, com **todos** os diagnósticos do
/// módulo (formato em `compile_error`); nenhum IR é emitido.
pub fn emitir_ir(entry: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emit_ir_with(entry, options)
}

/// O `lib/` do SDK do Dart: o que [`SdkLayout::discover`] acha (a variável
/// de ambiente, a distribuição, o `dart` do `PATH`), ou o erro que diz como
/// apontá-lo.
pub fn sdk_do_dart() -> Result<PathBuf, String> {
    SdkLayout::discover().ok_or_else(|| {
        "SDK do Dart não encontrado: a distribuição do dartforge o leva em lib/dart-sdk; \
         numa árvore de desenvolvimento, defina DARTFORGE_SDK_LIB (o lib/ do SDK) ou DART_SDK"
            .to_string()
    })
}

/// [`emitir_ir`] (o nome fica para quem já o chamava; o SDK vem sempre da
/// fonte, docs/NATIVO-ESPACO-UNIFICADO.md §4.7).
///
/// Literais como objetos estáticos (§2.11) só na produção (`optimize`), que
/// é uma imagem só. No desenvolvimento o SDK mora na DLL, com os estáticos
/// dela: um literal estático no executável seria outro objeto que o igual da
/// DLL (`identical('a'.substring(1, 1), '')` dava falso). Sem estáticos, o
/// literal do programa é internado no heap e canonicalizado contra os
/// estáticos da DLL, como no JIT.
pub fn emit_ir_with(entry: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emit_ir_internal(entry, options, None, None, options.optimize, true)
}

/// [`emitir_ir`] de uma geração nova de um programa em execução (o hot
/// reload do JIT): `previous_ir` é o IR da geração viva, de onde sai o layout
/// da área de globais que a geração nova estende ([`ir_area`]).
pub fn emitir_ir_recarregavel(entry: &Path, options: &CompileOptions, previous_ir: Option<&str>) -> Result<IrEmitido, String> {
    let area = previous_ir.and_then(ir_area);
    // J03: a mesma classe com o mesmo id da geração viva.
    let ids = previous_ir.map(context::ir_ids);
    // O JIT libera a memória de uma geração (J02): sem objetos estáticos nos
    // módulos do programa (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
    emit_ir_internal(entry, options, area, ids, false, false)
}

/// Os nomes (hashes) dos slots da área de globais do programa no IR `ir`:
/// o `@df.area = … [key, n, nome_0…]` que o emissor escreve.
pub fn ir_area(ir: &str) -> Option<Vec<i64>> {
    let line = ir.lines().find(|l| l.starts_with("@df.area = "))?;
    let list = &line[line.rfind('[')? + 1..line.rfind(']')?];
    let values: Vec<i64> = list
        .split(',')
        .map(|v| v.trim().strip_prefix("i64 ").and_then(|n| n.trim().parse().ok()))
        .collect::<Option<_>>()?;
    let n = usize::try_from(*values.get(1)?).ok()?;
    (values.len() == n + 2).then(|| values[2..].to_vec())
}

fn emit_ir_internal(
    entry: &Path,
    options: &CompileOptions,
    previous_area: Option<Vec<i64>>,
    previous_ids: Option<std::collections::HashMap<(String, String), u32>>,
    static_objects: bool,
    prune: bool,
) -> Result<IrEmitido, String> {
    // O modelo de exceções (`alvo::excecoes_por_tabelas`). O JIT (o único
    // que emite sem a poda) não tem as tabelas: o código dele não registra
    // as tabelas de desenrolamento no sistema.
    let excecoes_por_tabelas = alvo::excecoes_por_tabelas()?;
    if excecoes_por_tabelas && !prune {
        return Err("as exceções por tabelas (DARTFORGE_EXCECOES=tabelas) não existem no JIT: use compile-native".to_string());
    }
    // O modo de raízes (`alvo::raizes_por_mapas`). No JIT (o único que emite
    // sem a poda) o mapa de cada objeto é achado e registrado pelo
    // gerenciador de memória da sessão (`crates/jit`, §3.8).
    let raizes_por_mapas = alvo::raizes_por_mapas()?;
    // A política de memória (`alvo::arc_memory`), no AOT e no JIT: a
    // migração de instâncias da recarga conta pela foto
    // (`Heap::trocar_campos`, docs/ARC-IMPLEMENTACAO.md).
    let arc_memory = alvo::arc_memory()?;
    let maps_in_jit = raizes_por_mapas && !prune;
    // O rastro simbólico (§13.14). No JIT a tabela de cada objeto é
    // registrada pelo gerenciador de memória da sessão (`crates/jit`), e o
    // módulo sai sem os símbolos do ligador e sem a chamada de registro.
    let symbolic_stack_trace = alvo::symbolic_stack_trace()?;
    let stack_trace_in_jit = symbolic_stack_trace && !prune;
    // 1. Carregamento e Inferência (Front-end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => sdk_do_dart()?,
    };

    // A seção `vm` com a sobreposição `native_sdk/` (P5a): as bibliotecas de
    // `SOURCE_LIBRARIES` são as dela, ligadas como objetos em cache
    // (`sdk_modulo::compiled_sdk`). `mut`: a
    // versão de linguagem corrente pode vir de `--version-linguagem` (Dart
    // moderno, P2).
    let mut sdk = sdk_modulo::load_native_sdk(&sdk_dir)
        .map_err(|e| format!("falha ao carregar SDK VM: {e}"))?;
    if let Some(v) = options.versao_linguagem {
        sdk.versao_corrente = v;
    }
    for experiment in &options.experimentos {
        if !sdk.experimentos.contains(experiment) {
            sdk.experimentos.push(*experiment);
        }
    }

    let mut interner = Interner::new();
    // Com o motor de build (DF-BUILD-009): uma carga tolerante sem os
    // gerados serve de `BuildStep.resolver`; a geração volta em memória e
    // a carga de verdade a lê antes do disco.
    let (program, elements_diags) = match options.gerador {
        Some(gerar) => {
            let mut resolution_names = Interner::new();
            let (resolution, _) = load_lenient(entry, &sdk, options.packages, &mut resolution_names);
            let generation = gerar(&resolution, &resolution_names)?;
            dartforge_elements::load::load_lenient_gerados(
                entry,
                &sdk,
                options.packages,
                &mut interner,
                None,
                None,
                Some(generation),
            )
        }
        None => load_lenient(entry, &sdk, options.packages, &mut interner),
    };
    if let Some((first_one, rest)) = elements_diags.split_first() {
        let mut msg = format!("erro ao carregar o programa: {first_one}");
        for d in rest {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }
    // As bibliotecas do SDK compiladas da fonte (P5c) são módulos à parte, em
    // cache: o programa não as baixa de novo; `dart:async` usado liga o laço
    // de eventos.
    let uses_dart_async = !source::source_libraries(&program, &interner).is_empty();
    let source_libraries: Vec<dartforge_elements::model::LibraryId> = Vec::new();

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    // Erros de linguagem dos recursos 3.7–3.13 (docs/VERSOES-LINGUAGEM.md §3) e
    // leitura de local não definitivamente atribuído abortam
    // (`codes::is_compile_error`); o resto de `types` é aviso e não aparece aqui.
    let mut errors = outline_diags
        .iter()
        .chain(body_diags.iter())
        .filter(|d| dartforge_types::codes::e_erro_de_compilacao(&d.message));
    if let Some(first_one) = errors.next() {
        let mut msg = format!("erro: {first_one}");
        for d in errors {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }

    let te = erasure::compute(&program, &outline, &mut table);
    let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    ctx.arc_memory = arc_memory;
    ctx.te = te;
    if options.depuracao {
        ctx.link_debug();
        ctx.dwarf = true;
    } else if symbolic_stack_trace {
        // As posições sem o DWARF: só a tabela do rastro (§13.14).
        ctx.link_debug();
    }
    ctx.stack_trace = symbolic_stack_trace;
    ctx.from_source = source_libraries.into_iter().collect();
    ctx.uses_dart_async = uses_dart_async;
    // O mundo fechado do programa (C7, `native_world.rs`): o que o `main`
    // não alcança não é baixado.
    let t_world = Instant::now();
    ctx.world = native_world::compute(&program, &interner, &table, &outline, &bodies);
    let world_duration = t_world.elapsed();
    let world_stat = ctx.world.as_ref().map_or((0, 0), |m| (m.estat.funcoes_vivas, m.estat.funcoes_usuario));
    // Um programa sem `main` na biblioteca de entrada não executa: a VM
    // recusa ("Invoked Dart programs must have a 'main' function defined").
    // Na recarga, é o arquivo lido no meio de uma gravação (vazio), que não
    // pode virar uma geração.
    let has_main = program.functions.iter().any(|f| {
        f.class.is_none()
            && f.extension.is_none()
            && f.kind == dartforge_elements::model::FunctionKind::Function
            && Some(f.library) == ctx.entry_lib
            && interner.resolve(f.name) == "main"
    });
    if !has_main {
        return Err("erro de compilação: o programa não define a função `main` \
                    (Invoked Dart programs must have a 'main' function defined)"
            .to_string());
    }
    // Os ids das classes do SDK são os do SDK compilado, não os do que este
    // programa carregou (`context::IdTable`).
    let ids = sdk_modulo::sdk_class_ids(&sdk_dir)?;
    let ctx = ctx.with_source_sdk_and_ids(ids);
    let ctx = match previous_ids {
        Some(ids) => ctx.with_previous_ids(ids),
        None => ctx,
    };
    let front_duration = t_front.elapsed();

    // 2. Lowering para HIR
    let t_hir = Instant::now();
    let mut hir_module = lower::lower_program(&ctx);
    hir_module.program_ids = ctx.program_ids();
    hir_module.program_fields = ctx.program_fields();
    hir_module.sdk_records = sdk_modulo::sdk_records();
    hir_module.runtime_cids = sdk_modulo::runtime_cids(&ctx);
    hir_module.sdk_version = sdk_dir.parent().and_then(sdk_version);
    if !hir_module.errors.is_empty() {
        return Err(compile_error(&hir_module.errors));
    }
    // 2a. A poda da HIR do programa pelo grafo de símbolos e seletores com
    // os resumos do SDK (`pruning::prune_hir`): o que a ligação de produção
    // tiraria depois sai antes de otimizar e emitir. Não no JIT (a geração
    // seguinte de uma recarga pode chamar qualquer seletor).
    // `DARTFORGE_SEM_PODA_HIR=1` desliga.
    if prune && !std::env::var("DARTFORGE_SEM_PODA_HIR").is_ok_and(|v| v == "1") {
        let t_prune = Instant::now();
        if let Some(e) = prune_program_hir(&mut hir_module, options.optimize)?
            && options.timings
        {
            eprintln!(
                "  Poda HIR:  {} de {} funções, {} de {} pares, {} assinaturas FFI ({:?})",
                e.live_functions,
                e.functions,
                e.live_pairs,
                e.pairs,
                if e.ffi.1 { e.ffi.0.to_string() } else { "todas as".to_string() },
                t_prune.elapsed()
            );
        }
    }
    // 2b. Otimização da HIR (inlining, substituição escalar: `optimize/`).
    let t_optimize = Instant::now();
    // O mesmo pipeline prepara as bibliotecas do SDK e materializa tabelas
    // somente depois das transformações da HIR.
    optimize::prepare_for_emission(&mut hir_module, arc_memory, excecoes_por_tabelas);
    // A preparação ARC publicada (`DARTFORGE_ARC_PREPARADO=1`): o programa
    // sozinho, chamando o SDK preparado (`sdk_modulo`, a mesma opção na
    // chave do cache) pelos resumos das próprias chamadas.
    if alvo::arc_prepared()? {
        hir_module.arc_external_callees = true;
        hir_module.arc_external_layouts = lower::sdk_arc_layouts(&ctx);
        let mut plans: std::collections::HashMap<_, _> = hir_module
            .functions
            .iter()
            .map(|f| (f.symbol.clone(), optimize::arc::DartFunctionPlan::default()))
            .collect();
        // Com tabelas, os planos partem dos invokes e pousos já materializados.
        if excecoes_por_tabelas {
            optimize::arc::prepare_arc_tabled_module(&mut hir_module, &mut plans)
        } else {
            optimize::arc::prepare_arc_dart_module(&mut hir_module, &mut plans)
        }
        .map_err(|e| format!("preparação ARC do programa: {e}"))?;
        optimize::arc::certify_arc_aot_ports(&mut hir_module, &plans)
            .map_err(|e| format!("portas ARC do programa: {e}"))?;
    }
    if options.timings {
        eprintln!("  HIR:       baixar {:?}, otimizar {:?}", t_optimize.duration_since(t_hir), t_optimize.elapsed());
    }
    let hir_duration = t_hir.elapsed();

    // 3. Emissão de LLVM IR
    let t_llvm = Instant::now();
    // Produção (`--optimize`) nunca recarrega: o descritor da área vai sem
    // os nomes dos slots (`with_lean_area`). O JIT emite sem `optimize`.
    let lean = options.optimize && previous_area.is_none();
    // O programa grande (o new_sali/backend: ~180 mil funções HIR) na
    // produção: o endereço dos campos e os ajudantes (despacho, alocação,
    // barreira, área, subtipo) por chamada, fora de linha — o executável
    // cai de 94,6 para ~71 MB e a geração de 707 para ~460 s
    // (docs/NATIVO-PRODUCAO-GRANDE.md §5). O preço é o desempenho do código
    // que chama muito por despacho dinâmico ou lê muitos campos
    // (`bench/performance/calls.dart`: 1,4–2,5× mais lento); com
    // `DARTFORGE_AJUDANTES_FORA=` (vazio) e `DARTFORGE_CAMPOS_POR_CHAMADA=0`
    // o programa grande volta todo em linha. O programa pequeno continua em
    // linha (a LTO tira o que sobra).
    let large_size = hir_module.functions.len() >= LARGE_PROGRAM_FUNCTIONS;
    let large = options.optimize && large_size;
    if large && options.timings {
        eprintln!("  Programa grande: {} funções HIR (campos e ajudantes por chamada)", hir_module.functions.len());
    }
    let fields_per_call = match std::env::var("DARTFORGE_CAMPOS_POR_CHAMADA") {
        Ok(v) => v == "1",
        Err(_) => large,
    };
    let emitter = llvm::LlvmEmitter::new(&hir_module)
        .with_previous_area(previous_area)
        .with_lean_area(lean)
        .with_static_objects(static_objects)
        // O AOT não recarrega: nenhum objeto ganha corpo de fora.
        .with_body_without_outside(prune)
        // Desenvolvimento (`-O0`): o endereço dos campos por chamada (C9).
        .with_fields_per_call(!options.optimize || fields_per_call)
        .with_outlined_helpers(llvm::out_of_line_helpers(!options.optimize && large_size, large))
        // `optsize` nas funções do programa grande: −8,7% no executável do
        // backend, sem mudar o tempo (docs/NATIVO-PRODUCAO-GRANDE.md §5);
        // `DARTFORGE_OPTSIZE_PROGRAMA=0` desliga.
        .with_optimize_size(large && !std::env::var("DARTFORGE_OPTSIZE_PROGRAMA").is_ok_and(|v| v == "0"))
        .with_production(options.optimize)
        // Sem otimização não há inlining: um quadro de raízes por quadro
        // nativo, e o runtime confere a ordem (o quadro morto).
        .with_ordered_frames(!options.optimize)
        .with_roots_by_maps(raizes_por_mapas)
        .with_maps_in_jit(maps_in_jit)
        .with_stack_trace(symbolic_stack_trace)
        .with_stack_trace_in_jit(stack_trace_in_jit);
    let llvm_ir = emitter.emit_all();
    // O verificador do modo mapas (§7.4), antes do passe dos mapas.
    if raizes_por_mapas {
        llvm::verify_maps::verify(&llvm_ir)?;
    } else if llvm::verify_shadow::linked() {
        // O conferidor de dominância do modo sombra (§7.4).
        llvm::verify_shadow::verify(&llvm_ir)?;
    }
    // Nos alvos Itanium, toda função precisa da tabela de desenrolamento:
    // nas exceções por tabelas (o desenrolamento) e nas raízes por mapas (o
    // percurso do coletor pelo `_Unwind_Backtrace`, que para no quadro sem
    // tabela — a libunwind do macOS o encerra ali, e as raízes dos quadros
    // de fora ficavam sem visita).
    let llvm_ir = if (excecoes_por_tabelas || raizes_por_mapas) && cfg!(unix) { sdk_modulo::with_uwtable(&llvm_ir) } else { llvm_ir };
    // Nas exceções por checagem com a pilha-sombra nada desenrola o código
    // Dart: sem o CFI de cada função (`sdk_modulo::with_nounwind`).
    // Só nos alvos Itanium: no COFF x64 toda função tem tabela de
    // desenrolamento de qualquer jeito, e o Windows não foi medido.
    let llvm_ir = if cfg!(unix) && !excecoes_por_tabelas && !raizes_por_mapas { sdk_modulo::with_nounwind(&llvm_ir) } else { llvm_ir };
    // O contexto da thread numa variável `thread_local` do módulo, no AOT
    // padrão (`sdk_modulo::with_context_in_tls`): em linha no programa
    // pequeno, uma chamada no grande.
    let llvm_ir = if cfg!(unix) && prune && !excecoes_por_tabelas && !raizes_por_mapas && !arc_memory {
        let mode = if large { sdk_modulo::ContextInTls::PerCall } else { sdk_modulo::ContextInTls::Inline };
        sdk_modulo::with_context_in_tls(&llvm_ir, mode, "df.ctx.tls.programa")
    } else {
        llvm_ir
    };
    let llvm_duration = t_llvm.elapsed();

    let sdk_byte_count = sdk_bytes(&llvm_ir, &program);
    Ok(IrEmitido {
        texto: llvm_ir,
        tempos: EmissionTimes {
            frontend: front_duration,
            world: world_duration,
            live_functions: world_stat,
            hir: hir_duration,
            llvm_ir: llvm_duration,
        },
        sdk_byte_count,
    })
}

/// Bytes do IR que são corpo de função declarada numa biblioteca `dart:`.
///
/// Mede quanto do módulo seria compartilhável entre programas se o SDK virasse
/// um módulo à parte (ESTADO.md §2.5). Cada `define` é atribuído ao elemento
/// pelo nome da biblioteca no símbolo estável (`df.<library>.…`, que os
/// fechos locais herdam da função que os contém); o resto — entrada, despacho, declarações do runtime,
/// strings — conta como do programa.
fn sdk_bytes(text: &str, program: &Program) -> usize {
    let mut total = 0usize;
    let mut in_sdk = false;
    for line in text.split_inclusive('\n') {
        if let Some(rest) = line.strip_prefix("define ") {
            // O símbolo estável começa pelo nome da biblioteca (P2):
            // `@df.dart$3a…` é função de uma biblioteca `dart:`.
            let _ = program;
            in_sdk = rest.contains("@df.dart$3a");
        }
        if in_sdk {
            total += line.len();
            if line.trim_end() == "}" {
                in_sdk = false;
            }
        }
    }
    total
}

/// Compila um programa Dart para um executável nativo.
pub fn compilar(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    compile_with(entry, exit, options)
}

/// [`compilar`] escolhendo o SDK (ver [`emit_ir_with`]).
pub fn compile_with(
    entry: &Path,
    exit: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    let t_total = Instant::now();

    let ir = emit_ir_with(entry, options)?;

    // 4. Clang e Ligação
    let driver_opts = driver::NativeDriverOptions {
        clang: driver::NativeDriverOptions::default().clang,
        optimize: options.optimize,
        timings: options.timings,
        debug_info: options.depuracao,
        cpu: options.cpu,
    };

    let linkage = driver::compile_and_link(&ir.texto, exit, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Tempos de Compilação Nativa ---");
        ir.imprimir_tempos();
        let origin = if linkage.objeto_do_cache { " (cache)" } else { "" };
        eprintln!("  Objeto:    {:?}{origin}", linkage.clang);
        eprintln!("  Link:      {:?}", linkage.link);
        eprintln!("  Total:     {:?}", total_duration);
        eprintln!("  Pico Mem:  {} KB", peak_memory / 1024);
    }

    Ok(exit.to_path_buf())
}

#[cfg(test)]
/// Os mesmos caminhos da emissão real, também nos runners Unix.
pub(crate) fn sdk_tests() -> &'static Path {
    static SDK: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    SDK.get_or_init(|| {
        let sdk = std::env::var_os("DARTFORGE_TEST_SDK_LIB")
            .map(PathBuf::from)
            .unwrap_or_else(|| sdk_do_dart().expect("SDK necessário aos testes de emissão"));
        assert!(
            sdk.join("libraries.json").is_file(),
            "SDK dos testes sem libraries.json: {}",
            sdk.display()
        );
        sdk
    }).as_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit(entry: &Path) -> IrEmitido {
        let options = CompileOptions { sdk: Some(sdk_tests()), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        emitir_ir(entry, &options).expect("emitir IR")
    }

    #[test]
    fn bigint_operator_excludes_smi_without_removing_num_and_dynamic_path() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry, "BigInt big(BigInt a, BigInt b) => a - b;\nnum numero(num a, num b) => a - b;\ndynamic aberto(dynamic a, dynamic b) => a - b;\nBigInt composto(BigInt a, BigInt b) { a += b; return a; }\nnum compostoNumero(num a, num b) { a += b; return a; }\nvoid main() { print(big(BigInt.from(9), BigInt.from(4))); print(numero(9, 4)); print(aberto(9, 4)); print(composto(BigInt.from(9), BigInt.from(4))); print(compostoNumero(9, 4)); }\n").unwrap();
            let ir = emit(&entry).texto;
            let body = |name: &str| {
                let symbol = format!("@df.main$2edart..{name}(");
                let start = ir.lines().find(|l| l.starts_with("define ") && l.contains(&symbol))
                    .unwrap_or_else(|| panic!("função {name} não emitida"));
                let pos = ir.find(start).unwrap();
                ir[pos..].split("\n}").next().unwrap()
            };
            for name in ["big", "composto"] { assert!(!body(name).contains("ashr i64"), "{}", body(name)); }
            assert!(body("big").contains("call "));
            for name in ["numero", "aberto", "compostoNumero"] {
                assert!(body(name).contains("ashr i64"), "{}", body(name));
            }
        }).unwrap().join().unwrap();
    }

    /// O mesmo programa emitido duas vezes em sequência e quatro vezes ao
    /// mesmo tempo dá o mesmo IR: nada da emissão depende de estado global,
    /// de endereço ou de ordem de conclusão.
    #[test]
    fn emit_ir_is_deterministic() {
        sdk_tests();
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() { print(1); }\n").unwrap();
        let a = emit(&entry);
        assert!(a.texto.contains("@dart_main"), "{}", a.texto);
        assert!(a.sdk_byte_count <= a.texto.len());
        assert_eq!(a.texto, emit(&entry).texto);
        let parallel: Vec<String> = std::thread::scope(|s| {
            let alcas: Vec<_> = (0..4)
                .map(|_| {
                    std::thread::Builder::new()
                        .stack_size(64 << 20)
                        .spawn_scoped(s, || emit(&entry).texto)
                        .unwrap()
                })
                .collect();
            alcas.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for p in parallel {
            assert_eq!(p, a.texto);
        }
    }

    /// Construto não suportado é `Err` de `emitir_ir`, com todos os
    /// diagnósticos: nenhum IR, e nenhum executável que só os imprime.
    #[test]
    fn unsupported_construct_is_error_with_all_diagnostics() {
        sdk_tests();
        let dir = tempfile::tempdir().unwrap();
        let entry = dir.path().join("main.dart");
        std::fs::write(&entry, "void main() {\n  int? k = 1;\n  var a = {?k: 1};\n  var b = {?k: 2};\n  print(a.length + b.length);\n}\n").unwrap();
        let options = CompileOptions { sdk: Some(sdk_tests()), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        let error = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || emitir_ir(&entry, &options).map(|ir| ir.texto))
            .unwrap()
            .join()
            .unwrap()
            .expect_err("entrada de mapa null-aware é recusada no nativo");
        let lines: Vec<&str> = error.lines().collect();
        assert_eq!(lines[0], "erro de compilação: não suportado no backend nativo: entrada de mapa null-aware", "{error}");
        assert_eq!(lines[1], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:3:11)", "{error}");
        assert_eq!(lines[2], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:4:11)", "{error}");
        assert_eq!(construtos_do_erro(&error), ["entrada de mapa null-aware", "entrada de mapa null-aware"]);
    }

    /// Os símbolos definidos (`define … @<símbolo>(`) de um IR.
    fn symbols(ir: &str) -> std::collections::BTreeSet<String> {
        ir.lines()
            .filter_map(|l| {
                let r = l.strip_prefix("define ")?;
                let i = r.find('@')? + 1;
                let f = r[i..].find('(')? + i;
                Some(r[i..f].to_string())
            })
            .collect()
    }

    /// T-ID (PESQUISA-HOT-RELOAD §4.2, P2): o símbolo vem do caminho da
    /// declaração. Inserir uma função e uma classe não muda o símbolo de
    /// nenhuma outra declaração, e o mesmo programa em outro diretório tem os
    /// mesmos símbolos.
    #[test]
    fn t_id_stable_symbols() {
        sdk_tests();
        let a = "int a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g);\n}\n";
        let b = "int z() => 0;\nclass D {}\nint a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint h = 6;\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g + z() + h);\n}\n";
        let emit_into = |source: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entry = dir.path().join("main.dart");
                    std::fs::write(&entry, source).unwrap();
                    emit(&entry).texto
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let (ia, ib, ia2) = (emit_into(a), emit_into(b), emit_into(a));
        let (sa, sb) = (symbols(&ia), symbols(&ib));
        for s in ["df.main$2edart..a", "df.main$2edart.C.m", "df.main$2edart.C.s", "df.main$2edart.C.new", "df.main$2edart..g", "dart_main"] {
            assert!(sa.contains(s), "{s} em {sa:?}");
        }
        assert!(sa.is_subset(&sb), "símbolos de A que sumiram em B: {:?}", sa.difference(&sb).collect::<Vec<_>>());
        // O mesmo programa em outro diretório dá o mesmo IR, salvo as
        // constantes de dados (`@df.arr.*`): a tabela da RTI leva a URI
        // `file:` da biblioteca de cada classe (`dart:mirrors`), que depende
        // do diretório por definição. A tabela vai comprimida
        // (`dartforge_runtime::rti_table`), então o tamanho de uma
        // constante (o tipo `[N x i64]`) e o argumento da chamada que
        // registra a tabela também podem mudar.
        let (la, la2): (Vec<_>, Vec<_>) = (ia.lines().collect(), ia2.lines().collect());
        assert_eq!(la.len(), la2.len(), "o mesmo programa em outro diretório dá o mesmo IR");
        let rti_table = |l: &str| l.contains("@dartforge_rti_iniciar_tabela(");
        for (x, y) in la.iter().zip(&la2) {
            if x != y {
                assert!(
                    (x.starts_with("@df.arr.") && y.starts_with("@df.arr.")) || (rti_table(x) && rti_table(y)),
                    "o mesmo programa em outro diretório dá o mesmo IR:\n{x}\n{y}"
                );
            }
        }
    }

    /// P6 e a regra de custo zero: só o programa que usa `dart:async` compila
    /// o `dart:async` da fonte e liga o laço de eventos; o que não usa não
    /// tem nada disso no IR.
    #[test]
    fn source_dart_async_only_for_users() {
        sdk_tests();
        let emit_source = |source: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entry = dir.path().join("main.dart");
                    std::fs::write(&entry, source).unwrap();
                    emit(&entry)
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let synchronous = emit_source("void main() { print(1); }\n");
        assert!(!synchronous.texto.contains("call void @dartforge_laco_de_eventos"));
        assert!(!synchronous.texto.contains("@df.dart$3aasync"));
        assert_eq!(synchronous.sdk_byte_count, 0);

        let asynchronous = emit_source("Future<int> f() async { await null; return 2; }\nFuture<void> main() async { print(await f()); }\n");
        assert!(asynchronous.texto.contains("call void @dartforge_laco_de_eventos(ptr @dartforge_chamar_dart0)"));
        assert!(asynchronous.texto.contains("define i64 @df.main$2edart..f$async$q"), "o corpo da máquina de estados");
        // O `dart:async` vem do módulo do SDK em cache (`sdk_modulo.rs`): o IR
        // do programa só o chama, não define as funções dele (`sdk_byte_count`
        // pode ser 0).
        assert!(asynchronous.texto.contains("@df.dart$3aasync.._asyncAwait("));
    }

    #[test]
    fn error_constructs_ignores_non_constructs() {
        let text = "[compile-native] erro de compilação: não suportado no backend nativo: membro `hash`\n\
                     erro de compilação: não suportado no backend nativo: membro `hash`\n  \
                     não suportado no backend nativo: membro `hash` (a.dart:1:2)\n  \
                     verificador da HIR (main): tag 3 para um valor I64\n  \
                     não suportado no backend nativo: chamada `sort` (a.dart:3:4)";
        assert_eq!(construtos_do_erro(text), ["membro `hash`", "chamada `sort`"]);
        assert!(construtos_do_erro("erro ao carregar o programa: x").is_empty());
    }
}
