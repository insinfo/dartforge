//! Compilador nativo LLVM do DartForge sobre a trilha nova.

pub mod cache;
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

/// Carrega, analisa e baixa um programa até o LLVM IR, sem Clang nem ligação.
///
/// É a parte de [`compilar`] que é nossa: o teste de determinismo do harness
/// compara só isto (`dartforge-diferencial determinismo --nativo`), e
/// `compile-native --emit-ir` grava isto. Diagnósticos de carga vão na
/// mensagem de erro, e não no stderr, para não se entrelaçarem quando várias
/// emissões rodam no mesmo processo; a primeira linha continua sendo a
/// contagem, que é o que o relatório do harness agrupa.
pub fn emitir_ir(entrada: &Path, options: &CompileOptions) -> Result<IrEmitido, String> {
    // 1. Carregamento e Inferência (Front-end)
    let t_front = Instant::now();
    let sdk_dir = match options.sdk {
        Some(p) => p.to_path_buf(),
        None => SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib")),
    };

    let sdk = SdkLayout::load(&sdk_dir, "vm")
        .map_err(|e| format!("falha ao carregar SDK VM: {e}"))?;

    let mut interner = Interner::new();
    let (program, elements_diags) = load_lenient(entrada, &sdk, options.packages, &mut interner);
    if !elements_diags.is_empty() {
        let mut msg = format!("{} erro(s) ao carregar o programa", elements_diags.len());
        for d in &elements_diags {
            msg.push_str(&format!("\nerro: {d}"));
        }
        return Err(msg);
    }

    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _outline_diags) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
    let (bodies, _body_diags) =
        dartforge_types::infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);

    let ctx = Context::new(&program, &interner, &table, &core, &outline, &bodies);
    let front_duration = t_front.elapsed();

    // 2. Lowering para HIR
    let t_hir = Instant::now();
    let hir_module = lower::lower_program(&ctx);
    let hir_duration = t_hir.elapsed();

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

    let (clang_duration, link_duration) = driver::compile_and_link(&ir.texto, saida, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Tempos de Compilação Nativa ---");
        ir.imprimir_tempos();
        eprintln!("  Clang:     {:?}", clang_duration);
        eprintln!("  Link:      {:?}", link_duration);
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
        let options = CompileOptions { sdk: Some(Path::new(SDK)), packages: None, timings: false, optimize: false };
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
}
