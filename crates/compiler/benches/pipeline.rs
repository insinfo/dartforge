//! Microbenchmarks reproduzíveis por etapa; não comparam compiladores com recursos distintos.
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

/// Mede lotes após aquecimento e retorna mediana/p95 por operação em nanossegundos.
fn measure(mut operation: impl FnMut(), iterations: usize, samples: usize) -> Value {
    for _ in 0..iterations {
        operation();
    }
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        times.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let midpoint = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[midpoint - 1] + sorted[midpoint]) / 2.0
    } else {
        sorted[midpoint]
    };
    json!({"median_ns": median, "p95_ns": sorted[(sorted.len()*95).div_ceil(100)-1], "samples_ns": times})
}

/// Cria um corpus sintético de funções independentes e uma entrada pequena.
fn corpus(functions: usize) -> String {
    let mut source = String::new();
    for index in 0..functions {
        source.push_str(&format!("int f{index}(int n) {{ var sum = 0; for(var i=0;i<n;i++) {{ sum += i; }} return sum; }}\n"));
    }
    source.push_str("void main() { print(f0(10)); }\n");
    source
}

/// Lê um inteiro positivo da configuração ou aplica o padrão informado.
fn setting(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |value| {
        value
            .parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .expect("Configuração do benchmark deve ser inteiro positivo")
    })
}

/// Coleta metadados fora das regiões cronometradas, preservando ausência como null.
fn command_text(program: &str, arguments: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Identifica CPU por plataforma sem supor que arquitetura equivale a hardware.
fn cpu_model() -> Option<String> {
    std::env::var("PROCESSOR_IDENTIFIER").ok().or_else(|| {
        if cfg!(target_os = "linux") {
            std::fs::read_to_string("/proc/cpuinfo")
                .ok()?
                .lines()
                .find_map(|line| {
                    line.strip_prefix("model name")
                        .and_then(|rest| {
                            rest.strip_prefix(":")
                                .or_else(|| rest.trim_start().strip_prefix(":"))
                        })
                        .map(|name| name.trim().to_owned())
                })
        } else if cfg!(target_os = "macos") {
            command_text("sysctl", &["-n", "machdep.cpu.brand_string"])
        } else {
            None
        }
    })
}

/// Executa as fases isoladas e o pipeline completo, escrevendo relatório JSON no stdout.
fn main() {
    let iterations = setting("DARTFORGE_BENCH_ITERATIONS", 100);
    let samples = setting("DARTFORGE_BENCH_SAMPLES", 21);
    let functions = setting("DARTFORGE_BENCH_FUNCTIONS", 100);
    let metadata = json!({
        "rustc": command_text("rustc", &["-Vv"]),
        "git_revision": command_text("git", &["rev-parse", "HEAD"]),
        "git_status_porcelain": command_text("git", &["status", "--porcelain"]),
        "cpu_model": cpu_model(),
        "logical_processors": std::thread::available_parallelism().ok().map(|n| n.get()),
        "unix_timestamp_seconds": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|value| value.as_secs()),
        "profile": "bench", "rustflags": std::env::var("RUSTFLAGS").ok(),
    });
    let source = corpus(functions);
    let tokens = dartforge_lexer::lex(&source).unwrap();
    let ast = dartforge_parser::parse(&tokens, source.len()).unwrap();
    dartforge_semantic::validate(&ast).unwrap();
    let module = dartforge_hir::lower(dartforge_parser::parse(&tokens, source.len()).unwrap());
    let mut phases = serde_json::Map::new();
    phases.insert(
        "lex".into(),
        measure(
            || {
                black_box(dartforge_lexer::lex(black_box(&source)).unwrap());
            },
            iterations,
            samples,
        ),
    );
    phases.insert(
        "parse".into(),
        measure(
            || {
                black_box(dartforge_parser::parse(black_box(&tokens), source.len()).unwrap());
            },
            iterations,
            samples,
        ),
    );
    phases.insert(
        "semantic".into(),
        measure(
            || {
                dartforge_semantic::validate(black_box(&ast)).unwrap();
            },
            iterations,
            samples,
        ),
    );
    phases.insert(
        "emit".into(),
        measure(
            || {
                black_box(dartforge_codegen::emit(black_box(&module)));
            },
            iterations,
            samples,
        ),
    );
    phases.insert(
        "pipeline".into(),
        measure(
            || {
                black_box(dartforge_compiler::compile(black_box(&source)).unwrap());
            },
            iterations,
            samples,
        ),
    );
    phases.insert(
        "pipeline_constants".into(),
        measure(
            || {
                black_box(
                    dartforge_compiler::compile_with_optimization(
                        black_box(&source),
                        dartforge_compiler::Optimization::Constants,
                    )
                    .unwrap(),
                );
            },
            iterations,
            samples,
        ),
    );
    println!("{}", serde_json::to_string_pretty(&json!({
        "schema_version": 1, "kind": "warm_in_process_synthetic", "target_dart": "3.6.2",
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "metadata": metadata,
        "functions": functions, "source_bytes": source.len(), "iterations": iterations,
        "samples": samples, "warmup_iterations": iterations,
        "statistic_unit": "batch_mean_ns_per_operation", "percentile_method": "nearest_rank",
        "corpus_reachable_functions": 1,
        "note": "Inclui descarte das alocações; fases isoladas não incluem fases anteriores; não mede processo, disco, SDK, DDC nem otimizações globais. Mediana/p95 são de médias por lote, não latências individuais. Corpus analisa todas as funções, mas main chama apenas f0.",
        "phases": phases
    })).unwrap());
}
