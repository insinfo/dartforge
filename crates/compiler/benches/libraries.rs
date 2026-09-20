//! Mede carregamento, resolução e compilação de bibliotecas em um corpus controlado.
use serde_json::{Value, json};
use std::{
    hint::black_box,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// Diretório exclusivo dos arquivos sintéticos, excluído ao concluir o benchmark.
struct Corpus {
    directory: PathBuf,
    bytes: usize,
    files: usize,
}
impl Corpus {
    /// Cria uma entrada que alcança todas as bibliotecas e símbolos privados homônimos.
    fn new(libraries: usize) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "dartforge-bench-libraries-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut main = String::new();
        let mut bytes = 0;
        for i in 0..libraries {
            main.push_str(&format!("import 'lib{i}.dart';\n"));
            let source = format!(
                "int _value(int x) {{ return x + {i}; }} int value{i}(int x) {{ return _value(x); }}\n"
            );
            bytes += source.len();
            std::fs::write(directory.join(format!("lib{i}.dart")), source).unwrap();
        }
        main.push_str("void main() { var sum=0;\n");
        for i in 0..libraries {
            main.push_str(&format!("sum += value{i}(1);\n"));
        }
        main.push_str("print(sum); }\n");
        bytes += main.len();
        std::fs::write(directory.join("main.dart"), main).unwrap();
        Self {
            directory,
            bytes,
            files: libraries + 1,
        }
    }
}
impl Drop for Corpus {
    /// Remove exclusivamente o diretório criado por esta execução.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// Lê parâmetros positivos e rejeita configuração que eliminaria as amostras.
fn setting(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |s| {
        s.parse::<usize>()
            .ok()
            .filter(|&v| v > 0)
            .expect("inteiro positivo obrigatório")
    })
}

/// Retorna amostras por chamada, com aquecimento fora da janela medida.
fn measure(mut operation: impl FnMut(), samples: usize) -> Value {
    operation();
    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        operation();
        times.push(start.elapsed().as_nanos() as f64);
    }
    let mut sorted = times.clone();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    };
    json!({"samples_ns":times,"median_ns":median,"p95_ns":sorted[(sorted.len()*95).div_ceil(100)-1]})
}

/// Executa medições separadas sem contar a criação do corpus ou o build da ferramenta.
fn main() {
    let count = setting("DARTFORGE_BENCH_LIBRARIES", 100);
    let samples = setting("DARTFORGE_BENCH_SAMPLES", 21);
    let corpus = Corpus::new(count);
    let entry = corpus.directory.join("main.dart");
    let graph = dartforge_packages::load(&entry).unwrap();
    let load = measure(
        || {
            black_box(dartforge_packages::load(black_box(&entry)).unwrap());
        },
        samples,
    );
    let link = measure(
        || {
            black_box(dartforge_linker::compile_graph(black_box(&graph), false).unwrap());
        },
        samples,
    );
    let pipeline = measure(
        || {
            black_box(
                dartforge_compiler::compile_path(
                    black_box(&entry),
                    dartforge_compiler::Optimization::None,
                )
                .unwrap(),
            );
        },
        samples,
    );
    let optimized = measure(
        || {
            black_box(
                dartforge_compiler::compile_path(
                    black_box(&entry),
                    dartforge_compiler::Optimization::Constants,
                )
                .unwrap(),
            );
        },
        samples,
    );
    let mut session = dartforge_compiler::CompilerSession::new();
    session
        .compile_path(&entry, dartforge_compiler::Optimization::None)
        .unwrap();
    let session_hit = measure(
        || {
            let result = session
                .compile_path(black_box(&entry), dartforge_compiler::Optimization::None)
                .unwrap();
            assert!(result.stats.cache_hit);
            assert_eq!(result.stats.compiled_units, 0);
            black_box(result);
        },
        samples,
    );
    let session_miss = measure(
        || {
            session.clear();
            let result = session
                .compile_path(black_box(&entry), dartforge_compiler::Optimization::None)
                .unwrap();
            assert!(!result.stats.cache_hit);
            assert_eq!(result.stats.compiled_units, corpus.files);
            black_box(result);
        },
        samples,
    );
    let rust = std::process::Command::new("rustc")
        .arg("-Vv")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    println!("{}",serde_json::to_string_pretty(&json!({"schema_version":2,"kind":"synthetic_libraries_warm_filesystem","files":corpus.files,"bytes":corpus.bytes,"samples":samples,"warmup_per_phase":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"rustc":rust,"git_revision":revision,"git_status_porcelain":dirty,"load":load,"link_preloaded_graph":link,"pipeline":pipeline,"pipeline_constants":optimized,"session_exact_hit":session_hit,"session_cleared_miss":session_miss,"note":"Cache de saída integral; hit ainda recarrega todas as fontes. Sem recompilação incremental. Carregamento/pipeline incluem leitura; link inclui lexer/parser/análise/emissão, mas não disco. Não compara DDC/dart2js nem executa JS."})).unwrap());
}
