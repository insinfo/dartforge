//! Compilador nativo LLVM do DartForge sobre a trilha nova.

pub mod cache;
pub mod context;
pub mod driver;
pub mod hir;
pub mod llvm;
pub mod lower;

use context::Context;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Opções de compilação nativa.
pub struct CompileOptions<'a> {
    pub sdk: Option<&'a Path>,
    pub packages: Option<&'a Path>,
    pub timings: bool,
    pub optimize: bool,
}

/// Compila um programa Dart para um executável nativo.
pub fn compilar(
    entrada: &Path,
    saida: &Path,
    options: &CompileOptions,
) -> Result<PathBuf, String> {
    let t_total = Instant::now();

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
        for d in &elements_diags {
            eprintln!("erro: {d}");
        }
        return Err(format!("{} erro(s) ao carregar o programa", elements_diags.len()));
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

    // 4. Clang e Ligação
    let driver_opts = driver::NativeDriverOptions {
        clang: std::env::var_os("DARTFORGE_CLANG")
            .map_or_else(|| PathBuf::from("D:/LLVM/22.1.8/bin/clang.exe"), PathBuf::from),
        optimize: options.optimize,
        timings: options.timings,
    };

    let (clang_duration, link_duration) = driver::compile_and_link(&llvm_ir, saida, &driver_opts)?;

    let total_duration = t_total.elapsed();
    let peak_memory = dartforge_instrument::peak_bytes();

    if options.timings {
        eprintln!("--- Tempos de Compilação Nativa ---");
        eprintln!("  Front-end: {:?}", front_duration);
        eprintln!("  HIR:       {:?}", hir_duration);
        eprintln!("  LLVM IR:   {:?}", llvm_duration);
        eprintln!("  Clang:     {:?}", clang_duration);
        eprintln!("  Link:      {:?}", link_duration);
        eprintln!("  Total:     {:?}", total_duration);
        eprintln!("  Pico Mem:  {} KB", peak_memory / 1024);
    }

    Ok(saida.to_path_buf())
}

