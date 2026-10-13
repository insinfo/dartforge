//! `cargo run -p dartforge-build --example plan -- <project> [build.dart]`
//!
//! Prints the project's canonical plan and, if there is a `build.dart` (the
//! project's own by default), compares it with that file.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = std::path::PathBuf::from(args.first().expect("usage: plan <project> [build.dart]"));
    let cfg_path = root.join(".dart_tool/package_config.json");
    let cfg =
        dartforge_elements::config::PackageConfig::load(&cfg_path).expect("package_config.json");
    let t = std::time::Instant::now();
    let (_g, _c, plan) = dartforge_build::project_plan(&root, &cfg).expect("plan");
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    print!("{}", plan.canonical_text());
    for a in &plan.warnings {
        eprintln!("warning: {a}");
    }
    let bd = args
        .get(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join(".dart_tool/build/entrypoint/build.dart"));
    if let Ok(text) = std::fs::read_to_string(&bd) {
        match dartforge_build::oracle::compare(&plan.applications, &text)
            .expect("readable build.dart")
        {
            Ok(()) => println!(
                "plan EQUAL to {} ({} applications, {ms:.1} ms)",
                bd.display(),
                plan.applications.len()
            ),
            Err(d) => {
                println!("plan DIFFERENT from {}:", bd.display());
                for l in d {
                    println!("{l}");
                }
                std::process::exit(1);
            }
        }
    }
}
