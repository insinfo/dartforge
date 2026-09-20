//! Mede custo de compilação e tamanho emitido com e sem fusão estrutural.
use dartforge_compiler::{CompileOptions, compile_with_options};
use std::{hint::black_box, time::Instant};
/// Executa corpus fixo e pequeno; resultados descrevem esta máquina e execução.
fn main() {
    let mut source = String::new();
    for n in 0..200 {
        source.push_str(&format!(
            "int f{n}(int x) {{ var v = x + 1; return v * 2; }}\n"
        ));
    }
    source.push_str("void main() { print(f0(1)); print(f199(2)); }");
    let mut rows = Vec::new();
    for enabled in [false, true] {
        let options = CompileOptions {
            merge_identical_functions: enabled,
            ..Default::default()
        };
        let output = compile_with_options(&source, options).unwrap();
        for _ in 0..3 {
            black_box(compile_with_options(black_box(&source), options).unwrap());
        }
        let mut samples = Vec::new();
        for _ in 0..15 {
            let start = Instant::now();
            for _ in 0..10 {
                black_box(compile_with_options(black_box(&source), options).unwrap());
            }
            samples.push(start.elapsed().as_nanos() / 10);
        }
        samples.sort_unstable();
        rows.push(serde_json::json!({"merge_identical_functions":enabled,"median_compile_ns":samples[7],"samples_ns":samples,"javascript_bytes":output.len(),"emitted_functions_including_main":output.matches("function ").count()}));
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"schema_version":1,"corpus_functions":200,"source_bytes":source.len(),"samples":15,"iterations_per_sample":10,"results":rows})).unwrap());
}
