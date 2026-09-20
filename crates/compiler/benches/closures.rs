//! Mede compilação de coleções/closures em processo, sem atribuir vantagem sobre Dart.
use dartforge_compiler::{CompileOptions, Optimization, compile_with_options};
use std::{hint::black_box, time::Instant};

/// Mede lotes independentes após aquecimento; inclui frontend, passes e emissão JS.
fn main() {
    let source = include_str!("../../../tests/conformance/cases/collections_closures.dart");
    let mut reports = vec![];
    for optimization in [Optimization::None, Optimization::Constants] {
        for merge_identical_functions in [false, true] {
            let options = CompileOptions {
                optimization,
                merge_identical_functions,
            };
            for _ in 0..50 {
                black_box(compile_with_options(black_box(source), options).unwrap());
            }
            let mut samples = vec![];
            for _ in 0..15 {
                let start = Instant::now();
                for _ in 0..100 {
                    black_box(compile_with_options(black_box(source), options).unwrap());
                }
                samples.push(start.elapsed().as_nanos() as f64 / 100.0);
            }
            let mut sorted = samples.clone();
            sorted.sort_by(f64::total_cmp);
            reports.push(serde_json::json!({"optimization":format!("{optimization:?}"),"merge":merge_identical_functions,"median_ns":sorted[7],"p95_ns":sorted[14],"samples_ns":samples,"output_bytes":compile_with_options(source,options).unwrap().len()}));
        }
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({"source_bytes":source.len(),"iterations_per_sample":100,"samples":15,"target":"Dart 3.6.2 JS subset","os":std::env::consts::OS,"arch":std::env::consts::ARCH,"note":"Warm in-process compilation; excludes CLI startup and execution. No comparison with DDC/dart2js.","results":reports})).unwrap());
}
