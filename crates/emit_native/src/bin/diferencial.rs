//! Harness de teste diferencial nativo: `dart run --enable-asserts` × Compilador Nativo LLVM.
//!
//! Executa os programas de `corpus/js/`, compila nativo e compara a saída byte a byte.
//! Uso:
//!   cargo run -p dartforge-emit-native --bin diferencial
//!   cargo run -p dartforge-emit-native --bin diferencial -- 01_print
//!   cargo run -p dartforge-emit-native --bin diferencial --bloco 1

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filtro = args.first().cloned();

    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus_dir = raiz.join("corpus/js");
    let out_dir = raiz.join("target/native_corpus");
    std::fs::create_dir_all(&out_dir).expect("criar target/native_corpus");

    let mut programas = listar_programas(&corpus_dir, filtro.as_deref());
    if programas.is_empty() {
        eprintln!("Nenhum programa encontrado em {}", corpus_dir.display());
        std::process::exit(1);
    }

    println!("=== DartForge Nativo (LLVM AOT) — Diferencial contra VM ===");
    println!("Total de programas selecionados: {}", programas.len());
    println!();

    let mut pass = 0usize;
    let mut fail = 0usize;

    for (idx, prog) in programas.iter().enumerate() {
        let nome = &prog.nome;
        let entrada = &prog.entrada;
        let exe_saida = out_dir.join(format!("{nome}.exe"));

        // 1. Oráculo: dart run --enable-asserts
        let dart_res = match Command::new("dart")
            .args(["run", "--enable-asserts", entrada.to_str().unwrap()])
            .output()
        {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[{}/{}] FAIL {nome}: falha ao invocar dart run: {e}", idx + 1, programas.len());
                fail += 1;
                continue;
            }
        };

        // 2. Compilação Nativa
        let i2 = entrada.clone();
        let o2 = exe_saida.clone();
        let t_comp_start = Instant::now();
        let comp_res = std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn(move || {
                let options = dartforge_emit_native::CompileOptions {
                    sdk: None,
                    packages: None,
                    timings: false,
                    optimize: false,
                    // O corpus é o `corpus/js`, escrito para a 3.6.
                    versao_linguagem: Some(dartforge_frontend::LanguageVersion::PISO),
                    experimentos: Vec::new(),
                };
                dartforge_emit_native::compilar(&i2, &o2, &options)
            })
            .expect("spawn thread")
            .join()
            .expect("join thread");

        let comp_duration = t_comp_start.elapsed();

        if let Err(e) = comp_res {
            eprintln!("[{}/{}] FAIL {nome} (compilação): {e}", idx + 1, programas.len());
            fail += 1;
            continue;
        }

        // 3. Execução Nativa
        let nat_res = match Command::new(&exe_saida).output() {
            Ok(out) => out,
            Err(e) => {
                eprintln!("[{}/{}] FAIL {nome} (execução): {e}", idx + 1, programas.len());
                fail += 1;
                continue;
            }
        };

        // 4. Comparação Byte a Byte
        let dart_stdout = String::from_utf8_lossy(&dart_res.stdout).replace("\r\n", "\n");
        let nat_stdout = String::from_utf8_lossy(&nat_res.stdout).replace("\r\n", "\n");

        let dart_stderr = String::from_utf8_lossy(&dart_res.stderr).replace("\r\n", "\n");
        let nat_stderr = String::from_utf8_lossy(&nat_res.stderr).replace("\r\n", "\n");

        let dart_code = dart_res.status.code().unwrap_or(-1);
        let nat_code = nat_res.status.code().unwrap_or(-1);

        let match_stdout = dart_stdout == nat_stdout;
        let match_code = (dart_code == 0 && nat_code == 0) || (dart_code != 0 && nat_code != 0);

        if match_stdout && match_code {
            println!("[{}/{}] PASS {} ({:.1?})", idx + 1, programas.len(), nome, comp_duration);
            pass += 1;
        } else {
            println!("[{}/{}] FAIL {}", idx + 1, programas.len(), nome);
            if !match_stdout {
                println!("  --- STDOUT DIFF ---");
                println!("  [DART]:\n{}", dart_stdout.trim_end());
                println!("  [NATIVO]:\n{}", nat_stdout.trim_end());
            }
            if !match_code {
                println!("  --- EXIT CODE DIFF: dart={} nativo={} ---", dart_code, nat_code);
            }
            if !nat_stderr.is_empty() {
                println!("  [STDERR]:\n{}", nat_stderr.trim_end());
            }
            fail += 1;
        }
    }

    println!();
    println!("=== RESULTADO FINAL: {}/{} PASSOU ({:.1}%) ===", pass, programas.len(), (pass as f64 / programas.len() as f64) * 100.0);
    if fail > 0 {
        std::process::exit(1);
    }
}

struct Prog {
    nome: String,
    entrada: PathBuf,
}

fn listar_programas(dir: &Path, filtro: Option<&str>) -> Vec<Prog> {
    let mut progs = Vec::new();
    let Ok(entradas) = std::fs::read_dir(dir) else { return progs };
    for e in entradas.flatten() {
        let p = e.path();
        if p.is_file() && p.extension().is_some_and(|ext| ext == "dart") {
            let nome = p.file_stem().unwrap().to_string_lossy().to_string();
            progs.push(Prog { nome, entrada: p });
        } else if p.is_dir() {
            let main_dart = p.join("main.dart");
            if main_dart.is_file() {
                let nome = p.file_name().unwrap().to_string_lossy().to_string();
                progs.push(Prog { nome, entrada: main_dart });
            }
        }
    }

    if let Some(f) = filtro {
        if f.starts_with("--bloco") {
            // Suporte a filtro por bloco
        } else {
            progs.retain(|p| p.nome.contains(f));
        }
    }

    progs.sort_by(|a, b| {
        let na = numero(&a.nome);
        let nb = numero(&b.nome);
        na.cmp(&nb).then_with(|| a.nome.cmp(&b.nome))
    });

    progs
}

fn numero(nome: &str) -> u32 {
    nome.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(u32::MAX)
}
