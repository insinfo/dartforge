//! Native differential test harness: `dart run --enable-asserts` × the LLVM native compiler.
//!
//! Runs the programs of `corpus/js/`, compiles them natively and compares the output byte by byte.
//! Usage:
//!   cargo run -p dartforge-emit-native --bin diferencial
//!   cargo run -p dartforge-emit-native --bin diferencial -- 01_print
//!   cargo run -p dartforge-emit-native --bin diferencial --bloco 1

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = args.first().cloned();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus_dir = root.join("corpus/js");
    let out_dir = root.join("target/native_corpus");
    std::fs::create_dir_all(&out_dir).expect("criar target/native_corpus");

    let programs = list_programs(&corpus_dir, filter.as_deref());
    if programs.is_empty() {
        eprintln!("Nenhum programa encontrado em {}", corpus_dir.display());
        std::process::exit(1);
    }

    println!("=== DartForge Nativo (LLVM AOT) — Diferencial contra VM ===");
    println!("Total de programas selecionados: {}", programs.len());
    println!();

    let mut pass = 0usize;
    let mut fail = 0usize;

    for (idx, prog) in programs.iter().enumerate() {
        let name = &prog.name;
        let entry = &prog.entry;
        let exe_output = out_dir.join(format!("{name}.exe"));

        // 1. Oracle: dart run --enable-asserts
        let dart_res = match Command::new("dart")
            .args(["run", "--enable-asserts", entry.to_str().unwrap()])
            .output()
        {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[{}/{}] FAIL {name}: falha ao invocar dart run: {e}", idx + 1, programs.len());
                fail += 1;
                continue;
            }
        };

        // 2. Native compilation
        let i2 = entry.clone();
        let o2 = exe_output.clone();
        let t_comp_start = Instant::now();
        let comp_res = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || {
                let options = dartforge_emit_native::CompileOptions {
                    sdk: None,
                    packages: None,
                    timings: false,
                    optimize: false,
                    // The corpus is `corpus/js`, written for 3.6.
                    versao_linguagem: Some(dartforge_frontend::LanguageVersion::PISO),
                    experimentos: Vec::new(),
                    depuracao: false,
                    gerador: None,
                    cpu: None,
                };
                dartforge_emit_native::compilar(&i2, &o2, &options)
            })
            .expect("spawn thread")
            .join()
            .expect("join thread");

        let comp_duration = t_comp_start.elapsed();

        if let Err(e) = comp_res {
            eprintln!("[{}/{}] FAIL {name} (compilação): {e}", idx + 1, programs.len());
            fail += 1;
            continue;
        }

        // 3. Native execution
        let nat_res = match Command::new(&exe_output).output() {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[{}/{}] FAIL {name} (execução): {e}", idx + 1, programs.len());
                fail += 1;
                continue;
            }
        };

        // 4. Byte-by-byte comparison
        let dart_stdout = String::from_utf8_lossy(&dart_res.stdout).replace("\r\n", "\n");
        let nat_stdout = String::from_utf8_lossy(&nat_res.stdout).replace("\r\n", "\n");

        let dart_stderr = String::from_utf8_lossy(&dart_res.stderr).replace("\r\n", "\n");
        let nat_stderr = String::from_utf8_lossy(&nat_res.stderr).replace("\r\n", "\n");

        let dart_code = dart_res.status.code().unwrap_or(-1);
        let nat_code = nat_res.status.code().unwrap_or(-1);

        let match_stdout = dart_stdout == nat_stdout;
        let match_code = (dart_code == 0 && nat_code == 0) || (dart_code != 0 && nat_code != 0);

        if match_stdout && match_code {
            println!("[{}/{}] PASS {} ({:.1?})", idx + 1, programs.len(), name, comp_duration);
            pass += 1;
        } else {
            println!("[{}/{}] FAIL {}", idx + 1, programs.len(), name);
            if !match_stdout {
                println!("  --- STDOUT DIFF ---");
                println!("  [DART]:\n{}", dart_stdout.trim_end());
                println!("  [NATIVO]:\n{}", nat_stdout.trim_end());
            }
            if !match_code {
                println!("  --- EXIT CODE DIFF: dart={} nativo={} ---", dart_code, nat_code);
            }
            if !dart_stderr.is_empty() {
                println!("  [STDERR DART]:\n{}", dart_stderr.trim_end());
            }
            if !nat_stderr.is_empty() {
                println!("  [STDERR]:\n{}", nat_stderr.trim_end());
            }
            fail += 1;
        }
    }

    println!();
    println!("=== RESULTADO FINAL: {}/{} PASSOU ({:.1}%) ===", pass, programs.len(), (pass as f64 / programs.len() as f64) * 100.0);
    if fail > 0 {
        std::process::exit(1);
    }
}

struct Prog {
    name: String,
    entry: PathBuf,
}

fn list_programs(dir: &Path, filter: Option<&str>) -> Vec<Prog> {
    let mut progs = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return progs };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "dart") {
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            progs.push(Prog { name, entry: p });
        } else if p.is_dir() {
            let main_dart = p.join("main.dart");
            if main_dart.is_file() {
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                progs.push(Prog { name, entry: main_dart });
            }
        }
    }

    if let Some(f) = filter {
        if f.starts_with("--bloco") {
            // Filter by block
        } else {
            progs.retain(|p| p.name.contains(f));
        }
    }

    progs.sort_by(|a, b| {
        let na = number(&a.name);
        let nb = number(&b.name);
        na.cmp(&nb).then_with(|| a.name.cmp(&b.name))
    });

    progs
}

fn number(name: &str) -> u32 {
    name.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(u32::MAX)
}
