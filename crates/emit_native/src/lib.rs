//! Compilador nativo LLVM do DartForge sobre a trilha nova.

pub mod alvo;
pub mod apagamento;
pub mod cache;
pub mod cache_objeto;
pub mod context;
pub mod driver;
pub mod fonte;
pub mod gcmap;
pub mod gerador;
pub mod ligador;
pub mod ligador_macos;
pub mod ligador_windows;
#[cfg(feature = "llvm-embutido")]
pub mod lto_distribuida;
pub mod hir;
pub mod llvm;
pub mod lower;
pub mod mundo_nativo;
pub mod particao;
pub mod nativos;
pub mod otimizar;
pub mod poda;
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
    /// Versão de linguagem corrente (`--versao-linguagem`, docs/VERSOES-LINGUAGEM.md);
    /// `None` = a da ferramenta (3.13).
    pub versao_linguagem: Option<dartforge_frontend::LanguageVersion>,
    /// Experimentos pedidos explicitamente para bibliotecas na versão corrente.
    pub experimentos: Vec<dartforge_frontend::Feature>,
    /// J05: tabelas de linha para o depurador nativo (`llvm/depuracao.rs`).
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
pub struct TemposEmissao {
    pub frontend: Duration,
    /// O mundo fechado do programa (`mundo_nativo.rs`).
    pub mundo: Duration,
    /// Funções do programa vivas no mundo fechado / todas (0/0 sem poda).
    pub funcoes_vivas: (usize, usize),
    pub hir: Duration,
    pub llvm_ir: Duration,
}

/// O LLVM IR de um programa, pronto para o Clang.
#[derive(Debug, Clone)]
pub struct IrEmitido {
    pub texto: String,
    pub tempos: TemposEmissao,
    /// Bytes de `texto` que são corpo de função do SDK (`dart:`); ver
    /// `bytes_do_sdk`.
    pub bytes_sdk: usize,
}

impl IrEmitido {
    /// As linhas de `--timings` da emissão, no formato de [`compilar`].
    pub fn imprimir_tempos(&self) {
        eprintln!("  Front-end: {:?}", self.tempos.frontend);
        let (vivas, todas) = self.tempos.funcoes_vivas;
        eprintln!("  Mundo:     {:?} ({vivas} de {todas} funções do programa vivas)", self.tempos.mundo);
        eprintln!("  HIR:       {:?}", self.tempos.hir);
        eprintln!("  LLVM IR:   {:?}", self.tempos.llvm_ir);
        let pct = if self.texto.is_empty() { 0.0 } else { 100.0 * self.bytes_sdk as f64 / self.texto.len() as f64 };
        eprintln!("  IR:        {} bytes, {} do SDK ({pct:.1}%)", self.texto.len(), self.bytes_sdk);
    }
}

/// Começo de todo diagnóstico de construto que o lowering não sabe baixar
/// (`FnBuilder::nao_suportado`, N1). [`construtos_do_erro`] é quem o lê.
pub const PREFIXO_NAO_SUPORTADO: &str = "não suportado no backend nativo: ";

/// [`poda::podar_hir`] com os resumos das bibliotecas do SDK do perfil (o
/// SDK sai do cache, ou é compilado agora, como a ligação faria em
/// seguida). `None` quando o módulo não usa o SDK da fonte.
fn podar_hir_do_programa(module: &mut hir::Module, producao: bool) -> Result<Option<poda::PodaDaHir>, String> {
    if module.biblioteca_sdk || module.registros_do_sdk.is_empty() {
        return Ok(None);
    }
    let perfil = if producao { sdk_modulo::PerfilDoSdk::Producao } else { sdk_modulo::PerfilDoSdk::Desenvolvimento };
    let clang = driver::NativeDriverOptions::default().clang;
    // O mesmo diretório e o mesmo Clang da ligação (`driver::compile_and_link`):
    // a mesma chave de cache.
    let sdk = sdk_modulo::sdk_compilado_no_perfil(&sdk_do_dart()?, &clang, perfil)?;
    if !sdk.resumos.iter().all(|r| r.is_file()) {
        return Ok(None);
    }
    let resumos = poda::ler_resumos(&sdk.resumos)?;
    let refs: Vec<&poda::Resumo> = resumos.iter().map(|r| r.as_ref()).collect();
    Ok(Some(poda::podar_hir(module, &refs)))
}

/// A versão do SDK em `raiz` como a VM a escreve em `Platform.version`
/// (`VM/runtime/vm/version_in.cc:33`, `{{VERSION_STR}} ({{CHANNEL}})
/// ({{COMMIT_TIME}})`): `3.6.2 (stable) (Wed Jan 29 01:20:39 2025 -0800)`,
/// sem o ` on "<sistema>"` que o runtime põe. A data do commit só o próprio
/// `dart` sabe: a primeira compilação a pergunta (`dart --version`) e a
/// guarda no cache nativo, pela versão; sem o `dart`, só o número do arquivo
/// `version` (o `sqlite3` imprime o `Platform.version`).
fn versao_do_sdk(raiz: &Path) -> Option<String> {
    let numero = std::fs::read_to_string(raiz.join("version")).ok()?.trim().to_string();
    let guardada = cache::dir_cache_nativo().join(format!("versao_do_sdk_{numero}.txt"));
    if let Ok(v) = std::fs::read_to_string(&guardada)
        && v.starts_with(&numero)
    {
        return Some(v.trim().to_string());
    }
    let dart = raiz.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" });
    let completa = std::process::Command::new(&dart)
        .arg("--version")
        .output()
        .ok()
        .and_then(|s| {
            // `Dart SDK version: 3.6.2 (stable) (…) on "windows_x64"` (na
            // saída padrão desde o 2.15; antes, na de erro).
            let texto = format!("{}{}", String::from_utf8_lossy(&s.stdout), String::from_utf8_lossy(&s.stderr));
            let resto = texto.split("Dart SDK version: ").nth(1)?;
            let v = resto.split(" on \"").next()?.trim().to_string();
            v.starts_with(&numero).then_some(v)
        });
    match completa {
        Some(v) => {
            let _ = std::fs::create_dir_all(cache::dir_cache_nativo());
            let _ = std::fs::write(&guardada, &v);
            Some(v)
        }
        None => Some(numero),
    }
}

/// A partir de quantas funções no módulo HIR do programa a produção o trata
/// como grande (`compile`, campos e ajudantes por chamada): o
/// new_sali/backend tem ~200 mil; os programas do corpus e os benchmarks,
/// poucos milhares.
pub const FUNCOES_DO_PROGRAMA_GRANDE: usize = 50_000;

/// O erro de um módulo com diagnósticos do lowering.
///
/// A primeira linha é `erro de compilação: ` e o primeiro diagnóstico **sem a
/// posição**: é a chave de agrupamento do harness, e o mesmo construto em
/// programas diferentes tem de cair no mesmo grupo. Depois vem uma linha por
/// diagnóstico, todos eles, com a posição e recuados dois espaços.
fn erro_de_compilacao(erros: &[String]) -> String {
    let primeiro = &erros[0];
    let resumo = primeiro.rsplit_once(" (").map_or(primeiro.as_str(), |(a, _)| a);
    let mut texto = format!("erro de compilação: {resumo}");
    for e in erros {
        texto.push_str("\n  ");
        texto.push_str(e);
    }
    texto
}

/// Os construtos não suportados de um erro de [`emitir_ir`]/[`compilar`], ou
/// de um texto que o contenha (o stderr que o harness monta com ele): um por
/// diagnóstico, na ordem, sem a posição. Vazio quando o erro não é de
/// construto (carga, pânico, verificador da HIR).
pub fn construtos_do_erro(texto: &str) -> Vec<String> {
    texto
        .lines()
        .filter_map(|l| {
            let d = l.strip_prefix("  ")?.strip_prefix(PREFIXO_NAO_SUPORTADO)?;
            Some(d.rsplit_once(" (").map_or(d, |(a, _)| a).to_string())
        })
        .collect()
}

/// Carrega, analisa e baixa um programa até o LLVM IR, sem Clang nem ligação.
///
/// É a parte de [`compilar`] que é nossa: o teste de determinismo do harness
/// compara só isto (`dartforge-diferencial determinismo --nativo`), e
/// `compile-native --emit-ir` grava isto. Diagnósticos de carga vão na
/// mensagem de erro, e não no stderr, para não se entrelaçarem quando várias
/// emissões rodam no mesmo processo. A primeira linha é o primeiro
/// diagnóstico, porque é ela que o relatório do harness mostra e agrupa — uma
/// contagem ("1 erro(s)") juntava num grupo só causas diferentes.
///
/// Construto não suportado também é `Err`, com **todos** os diagnósticos do
/// módulo (formato em `erro_de_compilacao`); nenhum IR é emitido.
pub fn emitir_ir(entrada: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emitir_ir_com(entrada, options)
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
pub fn emitir_ir_com(entrada: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    emitir_ir_interno(entrada, options, None, None, options.optimize, true)
}

/// [`emitir_ir`] de uma geração nova de um programa em execução (o hot
/// reload do JIT): `ir_anterior` é o IR da geração viva, de onde sai o layout
/// da área de globais que a geração nova estende ([`area_do_ir`]).
pub fn emitir_ir_recarregavel(entrada: &Path, options: &CompileOptions, ir_anterior: Option<&str>) -> Result<IrEmitido, String> {
    let area = ir_anterior.and_then(area_do_ir);
    // J03: a mesma classe com o mesmo id da geração viva.
    let ids = ir_anterior.map(context::ids_do_ir);
    // O JIT libera a memória de uma geração (J02): sem objetos estáticos nos
    // módulos do programa (docs/NATIVO-ESPACO-UNIFICADO.md §2.11).
    emitir_ir_interno(entrada, options, area, ids, false, false)
}

/// Os nomes (hashes) dos slots da área de globais do programa no IR `ir`:
/// o `@df.area = … [chave, n, nome_0…]` que o emissor escreve.
pub fn area_do_ir(ir: &str) -> Option<Vec<i64>> {
    let linha = ir.lines().find(|l| l.starts_with("@df.area = "))?;
    let lista = &linha[linha.rfind('[')? + 1..linha.rfind(']')?];
    let valores: Vec<i64> = lista
        .split(',')
        .map(|v| v.trim().strip_prefix("i64 ").and_then(|n| n.trim().parse().ok()))
        .collect::<Option<_>>()?;
    let n = usize::try_from(*valores.get(1)?).ok()?;
    (valores.len() == n + 2).then(|| valores[2..].to_vec())
}

fn emitir_ir_interno(
    entrada: &Path,
    options: &CompileOptions,
    area_anterior: Option<Vec<i64>>,
    ids_anteriores: Option<std::collections::HashMap<(String, String), u32>>,
    objetos_estaticos: bool,
    podar: bool,
) -> Result<IrEmitido, String> {
    // O modelo de exceções (`alvo::excecoes_por_tabelas`). O JIT (o único
    // que emite sem a poda) não tem as tabelas: o código dele não registra
    // as tabelas de desenrolamento no sistema.
    let excecoes_por_tabelas = alvo::excecoes_por_tabelas()?;
    if excecoes_por_tabelas && !podar {
        return Err("as exceções por tabelas (DARTFORGE_EXCECOES=tabelas) não existem no JIT: use compile-native".to_string());
    }
    // O modo de raízes (`alvo::raizes_por_mapas`). No JIT (o único que emite
    // sem a poda) o mapa de cada objeto é achado e registrado pelo
    // gerenciador de memória da sessão (`crates/jit`, §3.8).
    let raizes_por_mapas = alvo::raizes_por_mapas()?;
    let mapas_no_jit = raizes_por_mapas && !podar;
    // O rastro simbólico (§13.14). O JIT não emite a tabela: os objetos dele
    // não passam pelo registro das seções da imagem.
    let rastro_simbolico = alvo::rastro_simbolico()? && podar;
    // 1. Carregamento e Inferência (Front-end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => sdk_do_dart()?,
    };

    // A seção `vm` com a sobreposição `sdk_nativo/` (P5a): as bibliotecas de
    // `BIBLIOTECAS_DA_FONTE` são as dela, ligadas como objetos em cache
    // (`sdk_modulo::sdk_compilado`). `mut`: a
    // versão de linguagem corrente pode vir de `--versao-linguagem` (Dart
    // moderno, P2).
    let mut sdk = sdk_modulo::carregar_sdk_nativo(&sdk_dir)
        .map_err(|e| format!("falha ao carregar SDK VM: {e}"))?;
    if let Some(v) = options.versao_linguagem {
        sdk.versao_corrente = v;
    }
    for experimento in &options.experimentos {
        if !sdk.experimentos.contains(experimento) {
            sdk.experimentos.push(*experimento);
        }
    }

    let mut interner = Interner::new();
    // Com o motor de build (DF-BUILD-009): uma carga tolerante sem os
    // gerados serve de `BuildStep.resolver`; a geração volta em memória e
    // a carga de verdade a lê antes do disco.
    let (program, elements_diags) = match options.gerador {
        Some(gerar) => {
            let mut nomes_resolucao = Interner::new();
            let (resolucao, _) = load_lenient(entrada, &sdk, options.packages, &mut nomes_resolucao);
            let geracao = gerar(&resolucao, &nomes_resolucao)?;
            dartforge_elements::load::load_lenient_gerados(
                entrada,
                &sdk,
                options.packages,
                &mut interner,
                None,
                None,
                Some(geracao),
            )
        }
        None => load_lenient(entrada, &sdk, options.packages, &mut interner),
    };
    if let Some((primeiro, resto)) = elements_diags.split_first() {
        let mut msg = format!("erro ao carregar o programa: {primeiro}");
        for d in resto {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }
    // As bibliotecas do SDK compiladas da fonte (P5c) são módulos à parte, em
    // cache: o programa não as baixa de novo; `dart:async` usado liga o laço
    // de eventos.
    let usa_dart_async = !fonte::bibliotecas_da_fonte(&program, &interner).is_empty();
    let bibliotecas_da_fonte: Vec<dartforge_elements::model::LibraryId> = Vec::new();

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    // Erros de linguagem dos recursos 3.7–3.13 (docs/VERSOES-LINGUAGEM.md §3) e
    // leitura de local não definitivamente atribuído abortam
    // (`codes::e_erro_de_compilacao`); o resto de `types` é aviso e não aparece aqui.
    let mut erros = outline_diags
        .iter()
        .chain(body_diags.iter())
        .filter(|d| dartforge_types::codes::e_erro_de_compilacao(&d.message));
    if let Some(primeiro) = erros.next() {
        let mut msg = format!("erro: {primeiro}");
        for d in erros {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }

    let te = apagamento::calcular(&program, &outline, &mut table);
    let mut ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    ctx.te = te;
    if options.depuracao {
        ctx.ligar_depuracao();
        ctx.dwarf = true;
    } else if rastro_simbolico {
        // As posições sem o DWARF: só a tabela do rastro (§13.14).
        ctx.ligar_depuracao();
    }
    ctx.da_fonte = bibliotecas_da_fonte.into_iter().collect();
    ctx.usa_dart_async = usa_dart_async;
    // O mundo fechado do programa (C7, `mundo_nativo.rs`): o que o `main`
    // não alcança não é baixado.
    let t_mundo = Instant::now();
    ctx.mundo = mundo_nativo::calcular(&program, &interner, &table, &outline, &bodies);
    let mundo_duracao = t_mundo.elapsed();
    let estat_mundo = ctx.mundo.as_ref().map_or((0, 0), |m| (m.estat.funcoes_vivas, m.estat.funcoes_usuario));
    // Um programa sem `main` na biblioteca de entrada não executa: a VM
    // recusa ("Invoked Dart programs must have a 'main' function defined").
    // Na recarga, é o arquivo lido no meio de uma gravação (vazio), que não
    // pode virar uma geração.
    let tem_main = program.functions.iter().any(|f| {
        f.class.is_none()
            && f.extension.is_none()
            && f.kind == dartforge_elements::model::FunctionKind::Function
            && Some(f.library) == ctx.entry_lib
            && interner.resolve(f.name) == "main"
    });
    if !tem_main {
        return Err("erro de compilação: o programa não define a função `main` \
                    (Invoked Dart programs must have a 'main' function defined)"
            .to_string());
    }
    // Os ids das classes do SDK são os do SDK compilado, não os do que este
    // programa carregou (`context::TabelaDeIds`).
    let ids = sdk_modulo::ids_de_classe_do_sdk(&sdk_dir)?;
    let ctx = ctx.com_sdk_da_fonte_e_ids(ids);
    let ctx = match ids_anteriores {
        Some(ids) => ctx.com_ids_anteriores(ids),
        None => ctx,
    };
    let front_duration = t_front.elapsed();

    // 2. Lowering para HIR
    let t_hir = Instant::now();
    let mut hir_module = lower::lower_program(&ctx);
    hir_module.ids_do_programa = ctx.ids_do_programa();
    hir_module.campos_do_programa = ctx.campos_do_programa();
    hir_module.registros_do_sdk = sdk_modulo::registros_do_sdk();
    hir_module.cids_do_runtime = sdk_modulo::cids_do_runtime(&ctx);
    hir_module.versao_do_sdk = sdk_dir.parent().and_then(versao_do_sdk);
    if !hir_module.erros.is_empty() {
        return Err(erro_de_compilacao(&hir_module.erros));
    }
    // 2a. A poda da HIR do programa pelo grafo de símbolos e seletores com
    // os resumos do SDK (`poda::podar_hir`): o que a ligação de produção
    // tiraria depois sai antes de otimizar e emitir. Não no JIT (a geração
    // seguinte de uma recarga pode chamar qualquer seletor).
    // `DARTFORGE_SEM_PODA_HIR=1` desliga.
    if podar && !std::env::var("DARTFORGE_SEM_PODA_HIR").is_ok_and(|v| v == "1") {
        let t_poda = Instant::now();
        if let Some(e) = podar_hir_do_programa(&mut hir_module, options.optimize)?
            && options.timings
        {
            eprintln!(
                "  Poda HIR:  {} de {} funções, {} de {} pares, {} assinaturas FFI ({:?})",
                e.funcoes_vivas,
                e.funcoes,
                e.pares_vivos,
                e.pares,
                if e.ffi.1 { e.ffi.0.to_string() } else { "todas as".to_string() },
                t_poda.elapsed()
            );
        }
    }
    // 2b. Otimização da HIR (inlining, substituição escalar: `otimizar/`).
    let t_otimizar = Instant::now();
    otimizar::otimizar(&mut hir_module);
    // 2c. As exceções por tabelas: o último passe, sobre a HIR já otimizada.
    if excecoes_por_tabelas {
        otimizar::excecoes_por_tabelas(&mut hir_module);
    }
    if options.timings {
        eprintln!("  HIR:       baixar {:?}, otimizar {:?}", t_otimizar.duration_since(t_hir), t_otimizar.elapsed());
    }
    let hir_duration = t_hir.elapsed();

    // 3. Emissão de LLVM IR
    let t_llvm = Instant::now();
    // Produção (`--optimize`) nunca recarrega: o descritor da área vai sem
    // os nomes dos slots (`com_area_enxuta`). O JIT emite sem `optimize`.
    let enxuta = options.optimize && area_anterior.is_none();
    // O programa grande (o new_sali/backend: ~180 mil funções HIR) na
    // produção: o endereço dos campos e os ajudantes (despacho, alocação,
    // barreira, área, subtipo) por chamada, fora de linha — o executável
    // cai de 94,6 para ~71 MB e a geração de 707 para ~460 s
    // (docs/NATIVO-PRODUCAO-GRANDE.md §5). O preço é o desempenho do código
    // que chama muito por despacho dinâmico ou lê muitos campos
    // (`bench/desempenho/chamadas.dart`: 1,4–2,5× mais lento); com
    // `DARTFORGE_AJUDANTES_FORA=` (vazio) e `DARTFORGE_CAMPOS_POR_CHAMADA=0`
    // o programa grande volta todo em linha. O programa pequeno continua em
    // linha (a LTO tira o que sobra).
    let tamanho_grande = hir_module.functions.len() >= FUNCOES_DO_PROGRAMA_GRANDE;
    let grande = options.optimize && tamanho_grande;
    if grande && options.timings {
        eprintln!("  Programa grande: {} funções HIR (campos e ajudantes por chamada)", hir_module.functions.len());
    }
    let campos_por_chamada = match std::env::var("DARTFORGE_CAMPOS_POR_CHAMADA") {
        Ok(v) => v == "1",
        Err(_) => grande,
    };
    let emitter = llvm::LlvmEmitter::new(&hir_module)
        .com_area_anterior(area_anterior)
        .com_area_enxuta(enxuta)
        .com_objetos_estaticos(objetos_estaticos)
        // Desenvolvimento (`-O0`): o endereço dos campos por chamada (C9).
        .com_campos_por_chamada(!options.optimize || campos_por_chamada)
        .com_ajudantes_fora(llvm::ajudantes_fora_de_linha(!options.optimize && tamanho_grande, grande))
        // `optsize` nas funções do programa grande: −8,7% no executável do
        // backend, sem mudar o tempo (docs/NATIVO-PRODUCAO-GRANDE.md §5);
        // `DARTFORGE_OPTSIZE_PROGRAMA=0` desliga.
        .com_otimizar_tamanho(grande && !std::env::var("DARTFORGE_OPTSIZE_PROGRAMA").is_ok_and(|v| v == "0"))
        .com_producao(options.optimize)
        .com_raizes_por_mapas(raizes_por_mapas)
        .com_mapas_no_jit(mapas_no_jit)
        .com_rastro(rastro_simbolico);
    let llvm_ir = emitter.emit_all();
    // O verificador do modo mapas (§7.4), antes do passe dos mapas.
    if raizes_por_mapas {
        llvm::verificar_mapas::verificar(&llvm_ir)?;
    }
    // Nos alvos Itanium, toda função precisa da tabela de desenrolamento.
    let llvm_ir = if excecoes_por_tabelas && cfg!(unix) { sdk_modulo::com_uwtable(&llvm_ir) } else { llvm_ir };
    let llvm_duration = t_llvm.elapsed();

    let bytes_sdk = bytes_do_sdk(&llvm_ir, &program);
    Ok(IrEmitido {
        texto: llvm_ir,
        tempos: TemposEmissao {
            frontend: front_duration,
            mundo: mundo_duracao,
            funcoes_vivas: estat_mundo,
            hir: hir_duration,
            llvm_ir: llvm_duration,
        },
        bytes_sdk,
    })
}

/// Bytes do IR que são corpo de função declarada numa biblioteca `dart:`.
///
/// Mede quanto do módulo seria compartilhável entre programas se o SDK virasse
/// um módulo à parte (ESTADO.md §2.5). Cada `define` é atribuído ao elemento
/// pelo nome da biblioteca no símbolo estável (`df.<biblioteca>.…`, que os
/// fechos locais herdam da função que os contém); o resto — entrada, despacho, declarações do runtime,
/// strings — conta como do programa.
fn bytes_do_sdk(texto: &str, program: &Program) -> usize {
    let mut total = 0usize;
    let mut no_sdk = false;
    for linha in texto.split_inclusive('\n') {
        if let Some(resto) = linha.strip_prefix("define ") {
            // O símbolo estável começa pelo nome da biblioteca (P2):
            // `@df.dart$3a…` é função de uma biblioteca `dart:`.
            let _ = program;
            no_sdk = resto.contains("@df.dart$3a");
        }
        if no_sdk {
            total += linha.len();
            if linha.trim_end() == "}" {
                no_sdk = false;
            }
        }
    }
    total
}

/// Compila um programa Dart para um executável nativo.
pub fn compilar(
    entrada: &Path,
    saida: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    compilar_com(entrada, saida, options)
}

/// [`compilar`] escolhendo o SDK (ver [`emitir_ir_com`]).
pub fn compilar_com(
    entrada: &Path,
    saida: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    let t_total = Instant::now();

    let ir = emitir_ir_com(entrada, options)?;

    // 4. Clang e Ligação
    let driver_opts = driver::NativeDriverOptions {
        clang: driver::NativeDriverOptions::default().clang,
        optimize: options.optimize,
        timings: options.timings,
        depuracao: options.depuracao,
        cpu: options.cpu,
    };

    let ligacao = driver::compile_and_link(&ir.texto, saida, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Tempos de Compilação Nativa ---");
        ir.imprimir_tempos();
        let origem = if ligacao.objeto_do_cache { " (cache)" } else { "" };
        eprintln!("  Objeto:    {:?}{origem}", ligacao.clang);
        eprintln!("  Link:      {:?}", ligacao.link);
        eprintln!("  Total:     {:?}", total_duration);
        eprintln!("  Pico Mem:  {} KB", peak_memory / 1024);
    }

    Ok(saida.to_path_buf())
}

#[cfg(test)]
mod testes {
    use super::*;

    const SDK: &str = "C:/tools/dartsdk-3.6.2/lib";

    fn emitir(entrada: &Path) -> IrEmitido {
        let options = CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        emitir_ir(entrada, &options).expect("emitir IR")
    }

    /// O mesmo programa emitido duas vezes em sequência e quatro vezes ao
    /// mesmo tempo dá o mesmo IR: nada da emissão depende de estado global,
    /// de endereço ou de ordem de conclusão.
    #[test]
    fn emitir_ir_e_deterministico() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let entrada = dir.path().join("main.dart");
        std::fs::write(&entrada, "void main() { print(1); }\n").unwrap();
        let a = emitir(&entrada);
        assert!(a.texto.contains("@dart_main"), "{}", a.texto);
        assert!(a.bytes_sdk <= a.texto.len());
        assert_eq!(a.texto, emitir(&entrada).texto);
        let paralelos: Vec<String> = std::thread::scope(|s| {
            let alcas: Vec<_> = (0..4)
                .map(|_| {
                    std::thread::Builder::new()
                        .stack_size(64 << 20)
                        .spawn_scoped(s, || emitir(&entrada).texto)
                        .unwrap()
                })
                .collect();
            alcas.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for p in paralelos {
            assert_eq!(p, a.texto);
        }
    }

    /// Construto não suportado é `Err` de `emitir_ir`, com todos os
    /// diagnósticos: nenhum IR, e nenhum executável que só os imprime.
    #[test]
    fn construto_nao_suportado_e_erro_com_todos_os_diagnosticos() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let entrada = dir.path().join("main.dart");
        std::fs::write(&entrada, "void main() {\n  int? k = 1;\n  var a = {?k: 1};\n  var b = {?k: 2};\n  print(a.length + b.length);\n}\n").unwrap();
        let options = CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
        let erro = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || emitir_ir(&entrada, &options).map(|ir| ir.texto))
            .unwrap()
            .join()
            .unwrap()
            .expect_err("entrada de mapa null-aware é recusada no nativo");
        let linhas: Vec<&str> = erro.lines().collect();
        assert_eq!(linhas[0], "erro de compilação: não suportado no backend nativo: entrada de mapa null-aware", "{erro}");
        assert_eq!(linhas[1], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:3:11)", "{erro}");
        assert_eq!(linhas[2], "  não suportado no backend nativo: entrada de mapa null-aware (main.dart:4:11)", "{erro}");
        assert_eq!(construtos_do_erro(&erro), ["entrada de mapa null-aware", "entrada de mapa null-aware"]);
    }

    /// Os símbolos definidos (`define … @<símbolo>(`) de um IR.
    fn simbolos(ir: &str) -> std::collections::BTreeSet<String> {
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
    fn t_id_simbolos_estaveis() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let a = "int a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g);\n}\n";
        let b = "int z() => 0;\nclass D {}\nint a() => 1;\nclass C {\n  int v = 3;\n  int m() => v;\n  static int s() => 4;\n}\nint h = 6;\nint g = 5;\nvoid main() {\n  var f = () => a();\n  print(f());\n  print(C().m() + C.s() + g + z() + h);\n}\n";
        let emitir_em = |fonte: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entrada = dir.path().join("main.dart");
                    std::fs::write(&entrada, fonte).unwrap();
                    emitir(&entrada).texto
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let (ia, ib, ia2) = (emitir_em(a), emitir_em(b), emitir_em(a));
        let (sa, sb) = (simbolos(&ia), simbolos(&ib));
        for s in ["df.main$2edart..a", "df.main$2edart.C.m", "df.main$2edart.C.s", "df.main$2edart.C.new", "df.main$2edart..g", "dart_main"] {
            assert!(sa.contains(s), "{s} em {sa:?}");
        }
        assert!(sa.is_subset(&sb), "símbolos de A que sumiram em B: {:?}", sa.difference(&sb).collect::<Vec<_>>());
        // O mesmo programa em outro diretório dá o mesmo IR, salvo as
        // constantes de dados (`@df.arr.*`): a tabela da RTI leva a URI
        // `file:` da biblioteca de cada classe (`dart:mirrors`), que depende
        // do diretório por definição. Essas linhas só mudam de conteúdo, não
        // de tipo (`[N x i64]`).
        let (la, la2): (Vec<_>, Vec<_>) = (ia.lines().collect(), ia2.lines().collect());
        assert_eq!(la.len(), la2.len(), "o mesmo programa em outro diretório dá o mesmo IR");
        for (x, y) in la.iter().zip(&la2) {
            if x != y {
                let tipo = |l: &str| l.split(']').next().map(str::to_string);
                assert!(x.starts_with("@df.arr.") && tipo(x) == tipo(y), "o mesmo programa em outro diretório dá o mesmo IR:\n{x}\n{y}");
            }
        }
    }

    /// P6 e a regra de custo zero: só o programa que usa `dart:async` compila
    /// o `dart:async` da fonte e liga o laço de eventos; o que não usa não
    /// tem nada disso no IR.
    #[test]
    fn dart_async_da_fonte_so_para_quem_usa() {
        if !Path::new(SDK).join("libraries.json").is_file() {
            eprintln!("SDK ausente em {SDK}; teste pulado");
            return;
        }
        let emitir_fonte = |fonte: &'static str| {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let dir = tempfile::tempdir().unwrap();
                    let entrada = dir.path().join("main.dart");
                    std::fs::write(&entrada, fonte).unwrap();
                    emitir(&entrada)
                })
                .unwrap()
                .join()
                .unwrap()
        };
        let sincrono = emitir_fonte("void main() { print(1); }\n");
        assert!(!sincrono.texto.contains("call void @dartforge_laco_de_eventos"));
        assert!(!sincrono.texto.contains("@df.dart$3aasync"));
        assert_eq!(sincrono.bytes_sdk, 0);

        let assincrono = emitir_fonte("Future<int> f() async { await null; return 2; }\nFuture<void> main() async { print(await f()); }\n");
        assert!(assincrono.texto.contains("call void @dartforge_laco_de_eventos(ptr @dartforge_chamar_dart0)"));
        assert!(assincrono.texto.contains("define i64 @df.main$2edart..f$async$q"), "o corpo da máquina de estados");
        // O `dart:async` vem do módulo do SDK em cache (`sdk_modulo.rs`): o IR
        // do programa só o chama, não define as funções dele (`bytes_sdk`
        // pode ser 0).
        assert!(assincrono.texto.contains("@df.dart$3aasync.._asyncAwait("));
    }

    #[test]
    fn construtos_do_erro_ignora_o_que_nao_e_construto() {
        let texto = "[compile-native] erro de compilação: não suportado no backend nativo: membro `hash`\n\
                     erro de compilação: não suportado no backend nativo: membro `hash`\n  \
                     não suportado no backend nativo: membro `hash` (a.dart:1:2)\n  \
                     verificador da HIR (main): tag 3 para um valor I64\n  \
                     não suportado no backend nativo: chamada `sort` (a.dart:3:4)";
        assert_eq!(construtos_do_erro(texto), ["membro `hash`", "chamada `sort`"]);
        assert!(construtos_do_erro("erro ao carregar o programa: x").is_empty());
    }
}
