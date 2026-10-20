//! Measures the memory consumption, peak and resolution time of the type system and body inference.
//!
//! Run:
//! `cargo run -q --release -p dartforge-types --example memory_types -- C:/MyDartProjects/new_sali/frontend/web/main.dart`

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
            eprintln!("Usage: memoria <path_to_main.dart>");
            return;
        }
    };

    println!("============================================================");
    println!(" Memory and Performance Measurement — dartforge-types");
    println!(" Target: {}", entry_point.display());
    println!("============================================================");

    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("Failed to load SdkLayout");

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
    let (program, elements_diags) =
        load_lenient(&entry_point, &sdk, package_config.as_deref(), &mut interner);
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
    let (body_types, body_diags) =
        infer_program_bodies(&program, &interner, &mut table, &core, &mut outline);
    let t_bodies = t2.elapsed();
    let bytes_after_bodies = dartforge_instrument::live_bytes();

    let peak_bytes = dartforge_instrument::peak_bytes();
    let total_allocations = dartforge_instrument::allocation_count();

    // Statistics count
    let mut total_exprs = 0;
    let mut resolved_exprs = 0;
    for u in &body_types.units {
        total_exprs += u.expression_count();
        resolved_exprs += u.resolved_targets().count();
    }

    println!("\n--- Structural Metrics ---");
    println!(
        "Loaded libraries:               {}",
        program.libraries.len()
    );
    println!("Units (.dart):                  {}", program.units.len());
    println!("Classes / Mixins / Enums:       {}", program.classes.len());
    println!(
        "Functions and Methods:          {}",
        program.functions.len()
    );
    println!(
        "Variables and Fields:           {}",
        program.variables.len()
    );
    println!("Distinct types (TypeTable):     {}", table.len());
    println!("Typed expressions:              {}", total_exprs);
    println!(
        "Resolved expressions:           {} ({:.1}%)",
        resolved_exprs,
        if total_exprs > 0 {
            (resolved_exprs as f64 / total_exprs as f64) * 100.0
        } else {
            0.0
        }
    );

    println!("\n--- Emitted Diagnostics ---");
    println!("Elements / Parser:              {}", elements_diags.len());
    println!("Outline (Types):                {}", outline_diags.len());
    println!("Bodies (Inference and Flow):    {}", body_diags.len());

    println!("\n--- Memory Usage ---");
    println!(
        "Base Memory:                    {:.2} MB",
        base_bytes as f64 / 1_048_576.0
    );
    println!(
        "After Loading Elements:         {:.2} MB (+{:.2} MB)",
        bytes_after_elements as f64 / 1_048_576.0,
        (bytes_after_elements.saturating_sub(base_bytes)) as f64 / 1_048_576.0
    );
    println!(
        "After Outline Resolution:       {:.2} MB (+{:.2} MB)",
        bytes_after_outline as f64 / 1_048_576.0,
        (bytes_after_outline.saturating_sub(bytes_after_elements)) as f64 / 1_048_576.0
    );
    println!(
        "After Body Inference:           {:.2} MB (+{:.2} MB)",
        bytes_after_bodies as f64 / 1_048_576.0,
        (bytes_after_bodies.saturating_sub(bytes_after_outline)) as f64 / 1_048_576.0
    );
    println!(
        "Global Peak Memory:             {:.2} MB",
        peak_bytes as f64 / 1_048_576.0
    );
    println!("Total Allocations:              {}", total_allocations);

    println!("\n--- Execution Times ---");
    println!("Load and Parse (Elements):      {:.2?}", t_elements);
    println!("Outline Resolution:             {:.2?}", t_outline);
    println!("Body Inference:                 {:.2?}", t_bodies);
    println!(
        "Total Front-end Time:           {:.2?}",
        t_elements + t_outline + t_bodies
    );
    println!("============================================================");
}
