//! Times the engine over a whole package, in this process: the measuring
//! stick of the analysis's speed and memory on real projects.
//!
//! ```text
//! cargo run --release -p dartforge-parity --example analyze_project -- <package root> [repetitions]
//! ```
//!
//! Analyzes every `.dart` of the package (outside the `analysis_options.yaml`
//! exclusions) in a single `Engine::analyze_with`, like `dartforge analyze`,
//! and prints the wall time, the number of diagnostics and the counting
//! allocator's peak. `DARTFORGE_ENGINE_TIMES=1` adds the time of each engine
//! phase (on standard error). Measure the CPU time with `/usr/bin/time -l`:
//! with a loaded machine the wall time is not comparable.
#[global_allocator]
static ALLOCATOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_parity::analysis::Engine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(root) = args.first().map(PathBuf::from) else {
        eprintln!("usage: analyze_project <package root> [repetitions]");
        std::process::exit(2);
    };
    let reps: usize = args.get(1).and_then(|r| r.parse().ok()).unwrap_or(1);
    let engine = Engine::discover().expect("SDK");
    let options = dartforge_parity::filters::AnalysisOptions::read(&root);
    let files = dartforge_parity::projects::files(&root, &options);
    let packages = root.join(".dart_tool/package_config.json");
    let packages = packages.is_file().then_some(packages);
    // Deep bodies recurse deeply: own stack, like the scoreboard's workers.
    let h = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            for rep in 0..reps {
                dartforge_instrument::reset_peak();
                let base = dartforge_instrument::live_bytes();
                let t = Instant::now();
                let r = engine
                    .analyze_with(&root, &files, packages.as_deref(), &HashMap::new(), &|| false)
                    .expect("not cancelled");
                let diags: usize = r.files.values().map(|f| f.diags.len()).sum();
                println!(
                    "run {}: {} files, {} diagnostics, {:.2} s, peak {:.1} MiB above the base",
                    rep + 1,
                    files.len(),
                    diags,
                    t.elapsed().as_secs_f64(),
                    (dartforge_instrument::peak_bytes().saturating_sub(base)) as f64 / (1024.0 * 1024.0)
                );
            }
        })
        .expect("thread");
    h.join().expect("analysis");
}
