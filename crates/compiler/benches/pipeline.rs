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

/// Exercita bounds, listas reificadas, testes/casts e captura do argumento de tipo.
fn reified_corpus() -> &'static str {
    r#"
class LoginService { int login() => 7; }
T identity<T extends Object>(T value) => value;
int login<T extends LoginService>(T value) => value.login();
bool Function(Object?) checker<T>() => (Object? value) => value is T;
void main() {
  List<int> values = <int>[1, 2, 3];
  Object? candidate = identity<List<int>>(values);
  print(candidate is List<Object>);
  print((candidate as List<int>)[0]);
  var accepts = checker<List<int>>();
  print(accepts(candidate));
  print(login(LoginService()));
}
"#
}

/// Exercita records tipados, desestruturação e igualdade através de Object.
fn record_corpus() -> &'static str {
    r#"
(T, {T value}) pair<T>(T value) => (value, value: value);
void main() {
  final (first, value: second) = pair<int>(7);
  var records = <(int, {int value})>[(first, value: second)];
  Object erased = records[0];
  print(erased is (int, {int value}));
  print(erased == (value: 7, 7));
}
"#
}

/// Exercita cascatas com campos, chamadas, índices e interrupção por null.
fn cascade_corpus() -> &'static str {
    r#"
class Box {
  int value = 0;
  void set(int next) { value = next; }
}
void main() {
  var box = Box()..value = 1..set(2);
  var values = <int>[0]..[0] = box.value..add(3);
  Box? missing = null;
  missing?..value = values[0]..set(4);
  print(box.value);
  print(values);
}
"#
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
    let reified_source = reified_corpus();
    let record_source = record_corpus();
    let cascade_source = cascade_corpus();
    let macro_source = "@JsonCodable() class User { final String name; final int age; final String? email; } void main(){var user=User.fromJson(<String,Object?>{'name':'Dart','age':15});print(user.toJson()['age'] as int);}";
    let macro_output_bytes = dartforge_compiler::compile(macro_source)
        .expect("corpus de macro deve compilar")
        .len();
    let cascade_output_bytes = dartforge_compiler::compile(cascade_source)
        .expect("corpus de cascatas deve compilar")
        .len();
    let record_output_bytes = dartforge_compiler::compile(record_source)
        .expect("corpus de records deve compilar")
        .len();
    let reified_output_bytes = dartforge_compiler::compile(reified_source)
        .expect("corpus de genéricos reificados deve compilar")
        .len();
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
    let reified_pipeline = measure(
        || {
            black_box(dartforge_compiler::compile(black_box(reified_source)).unwrap());
        },
        iterations,
        samples,
    );
    let record_pipeline = measure(
        || {
            black_box(dartforge_compiler::compile(black_box(record_source)).unwrap());
        },
        iterations,
        samples,
    );
    let cascade_pipeline = measure(
        || {
            black_box(dartforge_compiler::compile(black_box(cascade_source)).unwrap());
        },
        iterations,
        samples,
    );
    let macro_pipeline = measure(
        || {
            black_box(dartforge_compiler::compile(black_box(macro_source)).unwrap());
        },
        iterations,
        samples,
    );
    let baseline_output_bytes = dartforge_compiler::compile(&source).unwrap().len();
    let tree_options = dartforge_compiler::CompileOptions {
        tree_shaking: true,
        ..Default::default()
    };
    let tree_output_bytes = dartforge_compiler::compile_with_options(&source, tree_options)
        .unwrap()
        .len();
    let tree_pipeline = measure(
        || {
            black_box(
                dartforge_compiler::compile_with_options(black_box(&source), tree_options).unwrap(),
            );
        },
        iterations,
        samples,
    );
    println!("{}", serde_json::to_string_pretty(&json!({
        "schema_version": 1, "kind": "warm_in_process_synthetic", "target_dart": "3.6.2",
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH, "metadata": metadata,
        "functions": functions, "source_bytes": source.len(), "javascript_bytes": baseline_output_bytes, "iterations": iterations,
        "samples": samples, "warmup_iterations": iterations,
        "statistic_unit": "batch_mean_ns_per_operation", "percentile_method": "nearest_rank",
        "corpus_reachable_functions": 1,
        "note": "Inclui descarte das alocações; fases isoladas não incluem fases anteriores; não mede processo, disco, SDK, DDC nem otimizações globais. Mediana/p95 são de médias por lote, não latências individuais. Corpus analisa todas as funções, mas main chama apenas f0.",
        "phases": phases,
        "tree_shaking_enabled": { "javascript_bytes": tree_output_bytes, "pipeline": tree_pipeline, "note": "Mesmo corpus e análise completa; main alcança apenas f0. Passe desativado no baseline." },
        "additional_corpora": {
            "json_codable": {
                "source_bytes": macro_source.len(), "javascript_bytes": macro_output_bytes,
                "iterations": iterations, "samples": samples, "warmup_iterations": iterations,
                "phases": { "pipeline": macro_pipeline },
                "note": "Inclui expansão Rust, análise de mapas/fábrica e emissão JS; macro experimental, sem comparação DDC/dart2js."
            },
            "cascades": {
                "source_bytes": cascade_source.len(), "javascript_bytes": cascade_output_bytes,
                "iterations": iterations, "samples": samples, "warmup_iterations": iterations,
                "phases": { "pipeline": cascade_pipeline },
                "note": "Campos, chamadas e índices em cascatas comuns/anuláveis; não mede execução JS nem compara DDC/dart2js."
            },
            "records": {
                "source_bytes": record_source.len(), "javascript_bytes": record_output_bytes,
                "iterations": iterations, "samples": samples, "warmup_iterations": iterations,
                "phases": { "pipeline": record_pipeline },
                "note": "Corpus fixo de records, genéricos, desestruturação e igualdade; não mede execução JS nem compara DDC/dart2js."
            },
            "reified_generics": {
                "source_bytes": reified_source.len(),
                "javascript_bytes": reified_output_bytes,
                "top_level_functions_including_main": 4,
                "classes": 1,
                "features": ["bounded_functions", "nominal_bound_member", "reified_list", "is", "as", "captured_type_argument"],
                "iterations": iterations, "samples": samples, "warmup_iterations": iterations,
                "phases": { "pipeline": reified_pipeline },
                "note": "Corpus fixo separado do baseline escalável. Mede compilação completa em processo aquecido e descarte da saída; não executa JavaScript nem mede DDC/dart2js."
            }
        }
    })).unwrap());
}
