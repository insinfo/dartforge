//! Mede custo real do cache de planos; ASTs e corpos continuam materializados em cada operação.
use dartforge_compiler::{
    CompileOptions, MacroSession, compile_with_macro_session, compile_with_options,
};
use dartforge_macros::{ExpansionReport, MacroCacheStats};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

/// Lê parâmetros positivos e falha cedo para configurações inválidas.
fn setting(name: &str, default: usize) -> usize {
    std::env::var(name).map_or(default, |s| {
        s.parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .expect("Configuração deve ser positiva")
    })
}

/// Mede lotes após um lote de aquecimento; inclui criação e descarte dos resultados.
fn measure(mut operation: impl FnMut(), iterations: usize, samples: usize) -> Value {
    for _ in 0..iterations {
        operation();
    }
    let mut values = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..iterations {
            operation();
        }
        values.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let median = if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };
    json!({"median_ns":median,"p95_ns":sorted[(n*95).div_ceil(100)-1],"samples_ns":values})
}

/// Altera somente o nome de um campo; as variantes têm o mesmo comprimento e forma.
fn source(fields: usize, variant: char) -> String {
    let mut result = String::from("@JsonCodable() class Item{");
    for i in 0..fields {
        result.push_str(&format!("final int field{variant}{i};"));
    }
    result.push_str("}void main(){}");
    result
}

/// Serializa estatísticas acumuladas, incluindo priming e aquecimento explicitados no relatório.
fn stats(s: MacroCacheStats) -> Value {
    json!({"hits":s.hits,"misses":s.misses,"evictions":s.evictions,"entries":s.entries,"payload_bytes":s.payload_bytes})
}

/// Expõe trabalho rematerializado e barreiras, sem presumir que acerto evita geração de AST.
fn report(r: &ExpansionReport) -> Value {
    json!({"applications":r.applications,"generated_declarations":r.generated_declarations,"materialized_nodes":r.materialized_nodes,"plan_hits":r.plan_hits,"plan_misses":r.plan_misses,"plan_evictions":r.plan_evictions,"phases":r.phases.iter().map(|p|json!({"phase":format!("{:?}",p.phase),"applications":p.applications,"generated_declarations":p.generated_declarations})).collect::<Vec<_>>()})
}

/// Recolhe versões fora da região medida; indisponibilidade permanece explícita.
fn command(program: &str, args: &[&str]) -> Option<String> {
    std::process::Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

/// Executa os mesmos estágios de compilação e mede a expansão isolada com clone explícito.
fn main() {
    let iterations = setting("DARTFORGE_BENCH_ITERATIONS", 100);
    let samples = setting("DARTFORGE_BENCH_SAMPLES", 15);
    let fields = setting("DARTFORGE_MACRO_FIELDS", 8);
    let sources = [source(fields, 'a'), source(fields, 'b')];
    let options = CompileOptions::default();
    let baseline_js = compile_with_options(&sources[0], options).unwrap();
    let mut warm = MacroSession::with_limits(1, 1024 * 1024);
    assert_eq!(
        baseline_js,
        compile_with_macro_session(&sources[0], options, &mut warm).unwrap()
    );
    let pipeline_uncached = measure(
        || {
            black_box(compile_with_options(black_box(&sources[0]), options).unwrap());
        },
        iterations,
        samples,
    );
    let pipeline_hit = measure(
        || {
            black_box(
                compile_with_macro_session(black_box(&sources[0]), options, &mut warm).unwrap(),
            );
        },
        iterations,
        samples,
    );
    let mut miss = MacroSession::with_limits(1, 1024 * 1024);
    let mut next = 0;
    let pipeline_miss = measure(
        || {
            let source = &sources[next];
            next ^= 1;
            black_box(compile_with_macro_session(black_box(source), options, &mut miss).unwrap());
        },
        iterations,
        samples,
    );
    let tokens = dartforge_lexer::lex(&sources[0]).unwrap();
    let ast = dartforge_parser::parse(&tokens, sources[0].len()).unwrap();
    let mut expanded = ast.clone();
    let baseline_report = dartforge_macros::expand(&mut expanded, sources[0].len()).unwrap();
    let mut isolated = MacroSession::with_limits(1, 1024 * 1024);
    isolated.expand(&mut ast.clone(), sources[0].len()).unwrap();
    let expansion_uncached = measure(
        || {
            let mut program = black_box(&ast).clone();
            black_box(dartforge_macros::expand(&mut program, sources[0].len()).unwrap());
        },
        iterations,
        samples,
    );
    let expansion_hit = measure(
        || {
            let mut program = black_box(&ast).clone();
            black_box(isolated.expand(&mut program, sources[0].len()).unwrap());
        },
        iterations,
        samples,
    );
    let hit_report = isolated.expand(&mut ast.clone(), sources[0].len()).unwrap();
    println!("{}",serde_json::to_string_pretty(&json!({
        "schema_version":1,"kind":"macro_plan_cache","experimental_builtin":"JsonCodable","runtime_semantics_target":"Dart 3.6.2",
        "iterations":iterations,"samples":samples,"warmup_operations_per_scenario":iterations,"fields":fields,"source_bytes":sources[0].len(),"javascript_bytes":baseline_js.len(),
        "environment":{"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"logical_parallelism":std::thread::available_parallelism().ok().map(|n|n.get()),"processor_identifier":std::env::var("PROCESSOR_IDENTIFIER").ok(),"hardware_note":std::env::var("DARTFORGE_BENCH_HARDWARE").ok(),"rustc":command("rustc",&["--version"]),"git_revision":command("git",&["rev-parse","HEAD"]),"crate_version":env!("CARGO_PKG_VERSION")},
        "pipeline":{"uncached":pipeline_uncached,"warm_plan_hit":pipeline_hit,"alternating_schema_miss":pipeline_miss},
        "expansion_only_with_input_clone":{"uncached":expansion_uncached,"warm_plan_hit":expansion_hit},
        "cache_totals":{"pipeline_hit":stats(warm.stats()),"pipeline_miss":stats(miss.stats()),"expansion_hit":stats(isolated.stats())},
        "expansion_reports":{"uncached":report(&baseline_report),"warm_hit":report(&hit_report)},
        "methodology":["Mesmas opções padrão sem folding ou fusão em todos os cenários.","Pipeline inclui lexing, parsing, expansão, análise e emissão JS; não executa JS.","Expansão isolada exclui parsing, inclui clone da AST de entrada e clone transacional interno, geração e descarte.","Miss alterna dois esquemas pré-construídos em sessão com capacidade de um plano; não mede construção da fonte.","Contadores incluem aquecimento e priming: um priming por hit; expansão tem uma leitura de relatório adicional.","Ordem fixa pode sofrer deriva térmica; repetir a execução isolada. Não compara DDC/dart2js nem promete aceleração."]
    })).unwrap());
}
