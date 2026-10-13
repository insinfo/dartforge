//! Measures the memory consumption, peak and resolution time of the type system and body inference.
//!
//! Run:
//! `cargo run -q --release -p dartforge-types --example memoria_types -- C:/MyDartProjects/new_sali/frontend/web/main.dart`

#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use dartforge_types::{infer_program_bodies, resolve_outline};
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entry_point = if let Some(first) = args.first() {
        PathBuf::from(first)
    } else {
        let default_path = PathBuf::from("C:/MyDartProjects/new_sali/frontend/web/main.dart");
        if default_path.exists() {
            default_path
        } else {
            eprintln!("Uso: memoria <caminho_para_main.dart>");
            return;
        }
    };

    println!("============================================================");
    println!(" Medição de Memória e Desempenho — dartforge-types");
    println!(" Alvo: {}", entry_point.display());
    println!("============================================================");

    let sdk_dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("Falha ao carregar SdkLayout");

    let package_config = entry_point
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join(".dart_tool/package_config.json"))
        .filter(|p| p.exists());

    let base_bytes = dartforge_instrument::live_bytes();
    dartforge_instrument::reset_peak();

    // 1. Loading the Elements
    let t0 = Instant::now();
    let mut interner = Interner::new();
    let (program, elements_diags) = load_lenient(&entry_point, &sdk, package_config.as_deref(), &mut interner);
    let t_elements = t0.elapsed();
    let bytes_after_elements = dartforge_instrument::live_bytes();

    // 2. Initializing the TypeTable and CoreTypes
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);

    // 3. Outline Resolution
    let t1 = Instant::now();
    let (mut outline, outline_diags) = resolve_outline(&program, &interner, &mut table, &core);
    let t_outline = t1.elapsed();
    let bytes_after_outline = dartforge_instrument::live_bytes();

    // 4. Body Inference
    let t2 = Instant::now();
    let (body_types, body_diags) = infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    let t_bodies = t2.elapsed();
    let bytes_after_bodies = dartforge_instrument::live_bytes();

    let peak_bytes = dartforge_instrument::peak_bytes();
    let total_allocations = dartforge_instrument::allocation_count();

    // Statistics count
    let mut total_exprs = 0;
    let mut resolved_exprs = 0;
    for u in &body_types.units {
        total_exprs += u.static_types.len();
        resolved_exprs += u.resolved.iter().filter(|r| r.is_some()).count();
    }

    println!("\n--- Métricas Estruturais ---");
    println!("Bibliotecas carregadas:         {}", program.libraries.len());
    println!("Unidades (.dart):               {}", program.units.len());
    println!("Classes / Mixins / Enums:       {}", program.classes.len());
    println!("Funções e Métodos:              {}", program.functions.len());
    println!("Variáveis e Campos:             {}", program.variables.len());
    println!("Tipos distintos (TypeTable):    {}", table.len());
    println!("Expressões tipadas:             {}", total_exprs);
    println!("Expressões resolvidas:          {} ({:.1}%)",
        resolved_exprs,
        if total_exprs > 0 { (resolved_exprs as f64 / total_exprs as f64) * 100.0 } else { 0.0 }
    );

    println!("\n--- Diagnósticos Emitidos ---");
    println!("Elementos / Parser:             {}", elements_diags.len());
    println!("Outline (Tipos):                {}", outline_diags.len());
    println!("Corpos (Inferência e Fluxo):    {}", body_diags.len());

    println!("\n--- Consumo de Memória ---");
    println!("Memória Base:                   {:.2} MB", base_bytes as f64 / 1_048_576.0);
    println!("Após Carga de Elementos:        {:.2} MB (+{:.2} MB)",
        bytes_after_elements as f64 / 1_048_576.0,
        (bytes_after_elements.saturating_sub(base_bytes)) as f64 / 1_048_576.0
    );
    println!("Após Resolução de Outline:      {:.2} MB (+{:.2} MB)",
        bytes_after_outline as f64 / 1_048_576.0,
        (bytes_after_outline.saturating_sub(bytes_after_elements)) as f64 / 1_048_576.0
    );
    println!("Após Inferência de Corpos:      {:.2} MB (+{:.2} MB)",
        bytes_after_bodies as f64 / 1_048_576.0,
        (bytes_after_bodies.saturating_sub(bytes_after_outline)) as f64 / 1_048_576.0
    );
    println!("Pico de Memória Global:         {:.2} MB", peak_bytes as f64 / 1_048_576.0);
    println!("Alocações Totais:               {}", total_allocations);

    println!("\n--- Tempos de Execução ---");
    println!("Carga e Parse (Elementos):      {:.2?}", t_elements);
    println!("Resolução de Outline:           {:.2?}", t_outline);
    println!("Inferência de Corpos:           {:.2?}", t_bodies);
    println!("Tempo Total de Front-end:       {:.2?}", t_elements + t_outline + t_bodies);
    println!("============================================================");
}
