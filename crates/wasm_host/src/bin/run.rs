//! `dartforge-wasm-run <module.wasm>`: runs a `wasm32-linear` module in the
//! independent executor. Stdout and stderr are the module's; the exit status is
//! `dartforge_run`'s, 70 for an engine trap and 2 when the module did not run.

use std::io::Write;

fn main() {
    let Some(file) = std::env::args().nth(1) else {
        eprintln!("usage: dartforge-wasm-run <module.wasm>");
        std::process::exit(2);
    };
    let bytes = match std::fs::read(&file) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("dartforge-wasm-run: {file}: {e}");
            std::process::exit(2);
        }
    };
    match dartforge_wasm_host::run(&bytes) {
        Ok(outcome) => {
            let _ = std::io::stdout().write_all(&outcome.stdout);
            let _ = std::io::stderr().write_all(&outcome.stderr);
            if std::env::var_os("DARTFORGE_WASM_REPORT").is_some() {
                eprintln!("[dartforge-wasm-run] sha256 {}", outcome.sha256);
            }
            match (outcome.status, outcome.trap) {
                (Some(status), _) => std::process::exit(status),
                (None, Some(trap)) => {
                    eprintln!("dartforge-wasm-run: trap: {trap}");
                    std::process::exit(70);
                }
                (None, None) => std::process::exit(70),
            }
        }
        Err(e) => {
            eprintln!("dartforge-wasm-run: {file}: {e}");
            std::process::exit(2);
        }
    }
}
