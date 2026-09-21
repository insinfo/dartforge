//! Mede o custo completo dos novos recursos em processo após aquecimento.
use dartforge_compiler::{CompileOptions, Optimization, compile_with_options};
use std::{hint::black_box, time::Instant};

/// Registra amostras brutas; não compara modos com semântica ou ferramentas diferentes.
fn main() {
    let mut reports = vec![];
    for (name, source) in [
        (
            "generics_constants",
            include_str!("../../../tests/conformance/cases/generics_constants.dart"),
        ),
        (
            "enhanced_enums_switch",
            include_str!("../../../tests/conformance/cases/enhanced_enums_switch.dart"),
        ),
    ] {
        for optimization in [Optimization::None, Optimization::Constants] {
            let options = CompileOptions {
                optimization,
                merge_identical_functions: false,
                tree_shaking: false,
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
            reports.push(serde_json::json!({
                "fixture":name,"source_bytes":source.len(),"optimization":format!("{optimization:?}"),
                "median_ns":sorted[7],"p95_ns":sorted[14],"samples_ns":samples,
                "output_bytes":compile_with_options(source,options).unwrap().len()
            }));
        }
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "iterations_per_sample":100,"samples":15,"warmup_iterations":50,
        "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "note":"Compilação em processo; inclui frontend, passes e emissão. Exclui startup e execução JS. Não compara com DDC/dart2js.",
        "results":reports
    })).unwrap());
}
