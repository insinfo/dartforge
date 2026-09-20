//! Mede emissão de AST pronta com classes em uma cadeia de herança reversa.
use dartforge_diagnostics::Span;
use dartforge_hir::lower;
use dartforge_syntax::{Class, Program};
use serde_json::json;
use std::{hint::black_box, time::Instant};

/// Registra comandos de metadados fora da região medida.
fn command(program: &str, arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Executa aquecimento e lotes; inclui emissão e descarte, sem parser ou análise.
fn main() {
    let metadata = json!({
        "stage": std::env::var("DARTFORGE_CLASSES_STAGE").ok(),
        "rustc": command("rustc", &["-Vv"]),
        "git_revision": command("git", &["rev-parse", "HEAD"]),
        "git_status": command("git", &["status", "--porcelain"]),
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "cpu": std::env::var("PROCESSOR_IDENTIFIER").ok(),
        "logical_processors": std::thread::available_parallelism().ok().map(|n| n.get()),
        "rustflags": std::env::var("RUSTFLAGS").ok(),
        "unix_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
    });
    let mut cases = vec![];
    for count in [100u32, 1000] {
        let module = lower(Program {
            types: vec![],
            extensions: vec![],
            classes: (0..count)
                .rev()
                .map(|id| Class {
                    modifier: dartforge_syntax::ClassModifier::None,
                    kind: dartforge_syntax::ClassKind::Class,
                    mixins: vec![],
                    is_mixin_application: false,
                    mixin_origin: None,
                    enum_arguments: vec![],
                    enum_constructor_fields: vec![],
                    is_interface: false,
                    library_id: 0,
                    is_abstract: false,
                    interfaces: vec![],
                    abstract_methods: vec![],
                    enum_values: vec![],
                    id,
                    name: "Synthetic",
                    superclass: id.checked_sub(1),
                    fields: vec![],
                    methods: vec![],
                    span: Span { start: 0, end: 0 },
                })
                .collect(),
            functions: vec![],
            statements: vec![],
        });
        let iterations = if count == 100 { 100 } else { 10 };
        let output_bytes = dartforge_codegen::emit(&module).len();
        for _ in 0..iterations {
            black_box(dartforge_codegen::emit(black_box(&module)));
        }
        let mut samples = vec![];
        for _ in 0..15 {
            let start = Instant::now();
            for _ in 0..iterations {
                black_box(dartforge_codegen::emit(black_box(&module)));
            }
            samples.push(start.elapsed().as_nanos() as f64 / iterations as f64);
        }
        let mut sorted = samples.clone();
        sorted.sort_by(f64::total_cmp);
        cases.push(json!({"classes": count, "iterations_per_sample": iterations, "warmup_iterations": iterations,
            "output_bytes": output_bytes, "samples_ns_per_operation": samples, "median_ns": sorted[7], "p95_ns": sorted[14]}));
    }
    println!("{}", serde_json::to_string_pretty(&json!({"kind": "isolated_codegen_reverse_inheritance", "profile": "bench",
        "metadata": metadata, "cases": cases, "percentile": "nearest_rank_of_batch_means",
        "note": "Corpus sintético AST pronto. Inclui descarte da saída. Não mede compilação completa, runtime JS nem compara DartForge com Dart."})).unwrap());
}
