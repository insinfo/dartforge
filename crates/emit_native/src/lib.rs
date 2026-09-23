//! Compilador nativo LLVM do DartForge sobre a trilha nova.

pub mod cache;
pub mod cache_objeto;
pub mod context;
pub mod driver;
pub mod hir;
pub mod llvm;
pub mod lower;
pub mod resumo;

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
}

/// Tempo de cada fase da emissão (tudo antes do Clang).
#[derive(Debug, Clone, Copy, Default)]
pub struct TemposEmissao {
    pub frontend: Duration,
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
        eprintln!("  HIR:       {:?}", self.tempos.hir);
        eprintln!("  LLVM IR:   {:?}", self.tempos.llvm_ir);
        let pct = if self.texto.is_empty() { 0.0 } else { 100.0 * self.bytes_sdk as f64 / self.texto.len() as f64 };
        eprintln!("  IR:        {} bytes, {} do SDK ({pct:.1}%)", self.texto.len(), self.bytes_sdk);
    }
}

/// Começo de todo diagnóstico de construto que o lowering não sabe baixar
/// (`FnBuilder::nao_suportado`, N1). [`construtos_do_erro`] é quem o lê.
pub const PREFIXO_NAO_SUPORTADO: &str = "não suportado no backend nativo: ";

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
    // 1. Carregamento e Inferência (Front-end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
    };

    let mut sdk = SdkLayout::load(&sdk_dir, "vm")
        .map_err(|e| format!("falha ao carregar SDK VM: {e}"))?;
    if let Some(v) = options.versao_linguagem {
        sdk.versao_corrente = v;
    }

    let mut interner = Interner::new();
    let (program, elements_diags) = load_lenient(entrada, &sdk, options.packages, &mut interner);
    if let Some((primeiro, resto)) = elements_diags.split_first() {
        let mut msg = format!("erro ao carregar o programa: {primeiro}");
        for d in resto {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    // Erros de linguagem dos recursos 3.7–3.13 abortam (docs/VERSOES-LINGUAGEM.md §3);
    // o resto de `types` é aviso e não aparece aqui.
    let mut erros = outline_diags
        .iter()
        .chain(body_diags.iter())
        .filter(|d| dartforge_types::codes::e_erro_de_linguagem(&d.message));
    if let Some(primeiro) = erros.next() {
        let mut msg = format!("erro: {primeiro}");
        for d in erros {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }

    let ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    let front_duration = t_front.elapsed();

    // 2. Lowering para HIR
    let t_hir = Instant::now();
    let hir_module = lower::lower_program(&ctx);
    let hir_duration = t_hir.elapsed();
    if !hir_module.erros.is_empty() {
        return Err(erro_de_compilacao(&hir_module.erros));
    }

    // 3. Emissão de LLVM IR
    let t_llvm = Instant::now();
    let emitter = llvm::LlvmEmitter::new(&hir_module);
    let llvm_ir = emitter.emit_all();
    let llvm_duration = t_llvm.elapsed();

    let bytes_sdk = bytes_do_sdk(&llvm_ir, &program);
    Ok(IrEmitido {
        texto: llvm_ir,
        tempos: TemposEmissao { frontend: front_duration, hir: hir_duration, llvm_ir: llvm_duration },
        bytes_sdk,
    })
}

/// Bytes do IR que são corpo de função declarada numa biblioteca `dart:`.
///
/// Mede quanto do módulo seria compartilhável entre programas se o SDK virasse
/// um módulo à parte (ESTADO.md §2.5). Cada `define` é atribuído ao elemento
/// pelo índice no símbolo (`df_fn_<índice>_…`, que os fechos locais herdam da
/// função que os contém); o resto — entrada, despacho, declarações do runtime,
/// strings — conta como do programa.
fn bytes_do_sdk(texto: &str, program: &Program) -> usize {
    let mut total = 0usize;
    let mut no_sdk = false;
    for linha in texto.split_inclusive('\n') {
        if let Some(resto) = linha.strip_prefix("define ") {
            no_sdk = resto
                .split_once("@df_fn_")
                .and_then(|(_, s)| s.split('_').next()?.parse::<usize>().ok())
                .and_then(|i| program.functions.get(i))
                .is_some_and(|f| program.library(f.library).uri.starts_with("dart:"));
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
    let t_total = Instant::now();

    let ir = emitir_ir(entrada, options)?;

    // 4. Clang e Ligação
    let driver_opts = driver::NativeDriverOptions {
        clang: std::env::var_os("DARTFORGE_CLANG")
            .map_or_else(|| PathBuf::from("D:/LLVM/22.1.8/bin/clang.exe"), PathBuf::from),
        optimize: options.optimize,
        timings: options.timings,
    };

    let ligacao = driver::compile_and_link(&ir.texto, saida, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Tempos de Compilação Nativa ---");
        ir.imprimir_tempos();
        let origem = if ligacao.objeto_do_cache { " (cache)" } else { "" };
        eprintln!("  Clang:     {:?}{origem}", ligacao.clang);
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
        let options = CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: false, optimize: false, versao_linguagem: None };
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
        std::fs::write(&entrada, "void main() {\n  var f = () => 1;\n  var g = () => 2;\n  print(1);\n}\n").unwrap();
        let options = CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: false, optimize: false, versao_linguagem: None };
        let erro = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || emitir_ir(&entrada, &options).map(|ir| ir.texto))
            .unwrap()
            .join()
            .unwrap()
            .expect_err("closure não é suportada");
        let linhas: Vec<&str> = erro.lines().collect();
        assert_eq!(linhas[0], "erro de compilação: não suportado no backend nativo: closure", "{erro}");
        assert_eq!(linhas[1], "  não suportado no backend nativo: closure (main.dart:2:11)", "{erro}");
        assert_eq!(linhas[2], "  não suportado no backend nativo: closure (main.dart:3:11)", "{erro}");
        assert_eq!(construtos_do_erro(&erro), ["closure", "closure"]);
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
