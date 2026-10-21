//! `dartforge-wasm-gate <module.wasm> [--import-module M]... [--export E]... [--json]`:
//! validates a module against the strict `wasm32-mvp` profile and prints its
//! report (or every violation, with exit status 1).

use dartforge_wasm_gate::{Policy, validate};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut file = None;
    let mut policy = Policy::default();
    let mut json = false;
    let mut imports_given = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--import-module" => {
                if !imports_given {
                    policy.import_modules.clear();
                    imports_given = true;
                }
                policy.import_modules.extend(args.next());
            }
            "--export" => policy.required_exports.extend(args.next()),
            "--json" => json = true,
            _ if file.is_none() => file = Some(a),
            _ => {
                eprintln!("dartforge-wasm-gate: unexpected argument {a}");
                std::process::exit(2);
            }
        }
    }
    let Some(file) = file else {
        eprintln!(
            "usage: dartforge-wasm-gate <module.wasm> [--import-module M]... [--export E]... [--json]"
        );
        std::process::exit(2);
    };
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("dartforge-wasm-gate: {file}: {e}");
            std::process::exit(2);
        }
    };
    match validate(&bytes, &policy) {
        Ok(report) if json => println!(
            "{}",
            serde_json::to_string_pretty(&report.to_json()).unwrap()
        ),
        Ok(report) => {
            println!("{file}: valid wasm32-mvp, sha256 {}", report.sha256);
            println!("  imports: {:?}", report.imports);
            println!("  exports: {:?}", report.exports);
            println!(
                "  declared features (informative): {:?}",
                report.declared_features
            );
        }
        Err(errors) => {
            for e in errors {
                eprintln!("{file}: {e}");
            }
            std::process::exit(1);
        }
    }
}
