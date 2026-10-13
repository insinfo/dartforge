//! Sessão persistente: um processo, um LLVM inicializado, módulos compilados
//! uma vez e muitas execuções com estado Dart limpo entre elas.
//!
//! É o formato do executor de macros e builders (regra de `PLANO.md`): o
//! código da macro é compilado uma vez e fica em cache, e cada execução paga
//! só a execução. Os testes fixam as duas garantias (estado limpo e cache que
//! entra em várias sessões); a medição, com `--ignored --nocapture`, dá o custo
//! de cada peça.
use dartforge_jit::{JitSession, compile_module};
use std::time::{Duration, Instant};

/// Um programa com um estático: lança se o encontrar já inicializado, e o
/// inicializa. Na primeira execução termina bem; numa segunda que herdasse o
/// estado da primeira, terminaria com exceção não capturada (255).
const WITH_STATIC: &str = "\
@dfg_0 = internal global i64 0
@dfg_0_ok = internal global i8 0
declare void @dartforge_exception_throw(i64, i8)
define void @dartforge_entry() {
entrada:
  %ok = load i8, ptr @dfg_0_ok
  %ja = icmp ne i8 %ok, 0
  br i1 %ja, label %sujo, label %limpo
sujo:
  call void @dartforge_exception_throw(i64 5, i8 1)
  ret void
limpo:
  store i64 42, ptr @dfg_0
  store i8 1, ptr @dfg_0_ok
  ret void
}
";

/// A execução mais curta possível: a entrada retorna.
const TRIVIAL: &str = "define void @dartforge_entry() {\n  ret void\n}\n";

/// Várias execuções na mesma sessão começam com os estáticos zerados.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn sequential_runs_start_clean() {
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("estatico", WITH_STATIC).unwrap();
    for turn in 1..=3 {
        let report_data = session.run_entry().unwrap();
        assert_eq!(report_data.exit_code, 0, "execução {turn} herdou o estado da anterior");
    }
}

/// Um módulo compilado uma vez entra em várias sessões e roda limpo em todas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn compiled_module_enters_several_sessions() {
    let compiled = compile_module("estatico", WITH_STATIC).unwrap();
    assert!(!compiled.object().is_empty());
    for _ in 0..2 {
        let mut session = JitSession::new().unwrap();
        session.add_compiled_module(&compiled).unwrap();
        assert_eq!(session.run_entry().unwrap().exit_code, 0);
        assert_eq!(session.run_entry().unwrap().exit_code, 0);
    }
}

/// O objeto passa pelas mesmas verificações do IR: externo desconhecido cai.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn compiled_module_passes_preverification() {
    let ir = "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  call void @sqlite3_open()\n  ret void\n}\n";
    let compiled = compile_module("externo", ir).unwrap();
    let mut session = JitSession::new().unwrap();
    let error = session.add_compiled_module(&compiled).unwrap_err();
    assert_eq!(error.stage, "símbolos");
    assert!(error.message.contains("sqlite3_open"), "{error}");
}

/// O caminho `main` também reinicia os estáticos do módulo JIT do programa.
/// Esta sessão não carrega a DLL do SDK e não testa os estáticos internos dela.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn main_resets_jit_module_globals_between_runs() {
    let ir = "\
@dfg_0_ok = internal global i8 0
define i32 @main() {
  %anterior = load i8, ptr @dfg_0_ok
  store i8 1, ptr @dfg_0_ok
  %codigo = zext i8 %anterior to i32
  ret i32 %codigo
}
";
    let mut session = JitSession::new().unwrap();
    session.add_ir_module("programa", ir).unwrap();
    assert_eq!(session.run_main().unwrap().exit_code, 0);
    assert_eq!(session.run_main().unwrap().exit_code, 0);
}

/// Mediana e p95 de `n` amostras de `f`, depois de 3 aquecimentos.
fn measure(n: usize, mut f: impl FnMut() -> Duration) -> (Duration, Duration) {
    for _ in 0..3 {
        f();
    }
    let mut v: Vec<Duration> = (0..n).map(|_| f()).collect();
    v.sort();
    (v[n / 2], v[(n * 95 / 100).min(n - 1)])
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Custo de cada peça da sessão persistente, para `docs/JIT.md`.
///
/// Rodar com:
/// `cargo test -p dartforge-jit --release --test sessao_persistente medicao -- --ignored --nocapture`
///
/// (i) criar a sessão; (ii) carregar um módulo e gerar o código até a entrada
/// estar resolvida, pelo IR e pelo objeto em cache; (iii) uma execução trivial
/// numa sessão já aquecida. O módulo «IR do emissor» é o de um programa Dart
/// pequeno, emitido por `dartforge_emit_native::emitir_ir`.
#[test]
#[ignore = "medição: requer LLVM-C.dll e o SDK Dart 3.6.2; rode com --nocapture"]
fn persistent_session_measurement() {
    let n = 30;
    let (p50, p95) = measure(n, || {
        let t = Instant::now();
        let session = JitSession::new().unwrap();
        let d = t.elapsed();
        drop(session);
        d
    });
    println!("(i)   criar a sessão (LLJIT + símbolos do runtime): mediana {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));

    /// Apaga o diretório no `Drop`, também quando a emissão entra em pânico.
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Directory(std::env::temp_dir().join(format!("dartforge-sessao-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let source = dir.0.join("macro.dart");
    std::fs::write(&source, "int dobro(int x) { return x * 2; }\nvoid main() {\n  int y = dobro(21);\n}\n").unwrap();
    let emitted = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let options = dartforge_emit_native::CompileOptions { sdk: None, packages: None, timings: false, optimize: false, versao_linguagem: None, experimentos: Vec::new(), depuracao: false, gerador: None, cpu: None };
            // Com o SDK da fonte (o único modo do emissor desde o espaço
            // unificado): o programa importa o runtime e o SDK da biblioteca
            // compartilhada, que a sessão carrega (`JitSession::new_for_ir_com`).
            dartforge_emit_native::emitir_ir_recarregavel(&source, &options, None).unwrap().texto
        })
        .unwrap()
        .join()
        .unwrap();
    drop(dir);
    let dll = dartforge_emit_native::sdk_modulo::dll_do_sdk_da_fonte().expect("biblioteca do SDK da fonte");
    // A sessão do IR: o runtime embutido para o trivial, a biblioteca do SDK
    // para o emitido; e a execução pela entrada de cada perfil.
    let new_session = |ir: &str| JitSession::new_for_ir_com(ir, Some(&dll)).unwrap();
    let execute_fn = |s: &JitSession| if s.usa_sdk_da_fonte() { s.run_main().unwrap().total } else { s.run_entry().unwrap().total };

    for (label_name, ir) in [("trivial", TRIVIAL), ("IR do emissor", emitted.as_str())] {
        let (p50, p95) = measure(n, || {
            let mut session = new_session(ir);
            let t = Instant::now();
            session.add_ir_module("m", ir).unwrap();
            session.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  carregar pelo IR, {label_name} ({} bytes): análise + verificação + geração, mediana {:.3} ms, p95 {:.3} ms",
            ir.len(),
            ms(p50),
            ms(p95)
        );
        let (c50, c95) = measure(n, || {
            let t = Instant::now();
            compile_module("m", ir).unwrap();
            t.elapsed()
        });
        println!("      compilar para o cache, {label_name}: mediana {:.3} ms, p95 {:.3} ms (uma vez por módulo)", ms(c50), ms(c95));
        let compiled = compile_module("m", ir).unwrap();
        let (p50, p95) = measure(n, || {
            let mut session = new_session(ir);
            let t = Instant::now();
            session.add_compiled_module(&compiled).unwrap();
            session.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  carregar do cache, {label_name} ({} bytes de objeto): mediana {:.3} ms, p95 {:.3} ms",
            compiled.object().len(),
            ms(p50),
            ms(p95)
        );
        let mut session = new_session(ir);
        session.add_compiled_module(&compiled).unwrap();
        let (p50, p95) = measure(100, || execute_fn(&session));
        println!("(iii) uma execução, {label_name}, sessão aquecida: mediana {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));
    }
}
