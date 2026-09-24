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
/// estado da primeira, terminaria com exceção não capturada (101).
const COM_ESTATICO: &str = "\
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
fn execucoes_em_sequencia_comecam_limpas() {
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("estatico", COM_ESTATICO).unwrap();
    for vez in 1..=3 {
        let relatorio = sessao.run_entry().unwrap();
        assert_eq!(relatorio.exit_code, 0, "execução {vez} herdou o estado da anterior");
    }
}

/// Um módulo compilado uma vez entra em várias sessões e roda limpo em todas.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn modulo_compilado_entra_em_varias_sessoes() {
    let compilado = compile_module("estatico", COM_ESTATICO).unwrap();
    assert!(!compilado.object().is_empty());
    for _ in 0..2 {
        let mut sessao = JitSession::new().unwrap();
        sessao.add_compiled_module(&compilado).unwrap();
        assert_eq!(sessao.run_entry().unwrap().exit_code, 0);
        assert_eq!(sessao.run_entry().unwrap().exit_code, 0);
    }
}

/// O objeto passa pelas mesmas verificações do IR: externo desconhecido cai.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn modulo_compilado_passa_pela_pre_verificacao() {
    let ir = "declare void @sqlite3_open()\ndefine void @dartforge_entry() {\n  call void @sqlite3_open()\n  ret void\n}\n";
    let compilado = compile_module("externo", ir).unwrap();
    let mut sessao = JitSession::new().unwrap();
    let erro = sessao.add_compiled_module(&compilado).unwrap_err();
    assert_eq!(erro.stage, "símbolos");
    assert!(erro.message.contains("sqlite3_open"), "{erro}");
}

/// O caminho `main` também reinicia os estáticos do módulo JIT do programa.
/// Esta sessão não carrega a DLL do SDK e não testa os estáticos internos dela.
#[test]
#[ignore = "requer LLVM-C.dll alcançável pelo carregador; use scripts/env.ps1"]
fn main_reinicia_globais_do_modulo_jit_entre_execucoes() {
    let ir = "\
@dfg_0_ok = internal global i8 0
define i32 @main() {
  %anterior = load i8, ptr @dfg_0_ok
  store i8 1, ptr @dfg_0_ok
  %codigo = zext i8 %anterior to i32
  ret i32 %codigo
}
";
    let mut sessao = JitSession::new().unwrap();
    sessao.add_ir_module("programa", ir).unwrap();
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);
    assert_eq!(sessao.run_main().unwrap().exit_code, 0);
}

/// Mediana e p95 de `n` amostras de `f`, depois de 3 aquecimentos.
fn medir(n: usize, mut f: impl FnMut() -> Duration) -> (Duration, Duration) {
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
fn medicao_sessao_persistente() {
    let n = 30;
    let (p50, p95) = medir(n, || {
        let t = Instant::now();
        let sessao = JitSession::new().unwrap();
        let d = t.elapsed();
        drop(sessao);
        d
    });
    println!("(i)   criar a sessão (LLJIT + símbolos do runtime): mediana {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));

    /// Apaga o diretório no `Drop`, também quando a emissão entra em pânico.
    struct Diretorio(std::path::PathBuf);
    impl Drop for Diretorio {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Diretorio(std::env::temp_dir().join(format!("dartforge-sessao-{}", std::process::id())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let fonte = dir.0.join("macro.dart");
    std::fs::write(&fonte, "int dobro(int x) { return x * 2; }\nvoid main() {\n  int y = dobro(21);\n}\n").unwrap();
    let emitido = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let opcoes = dartforge_emit_native::CompileOptions { sdk: None, packages: None, timings: false, optimize: false, versao_linguagem: None };
            dartforge_emit_native::emitir_ir(&fonte, &opcoes).unwrap().texto
        })
        .unwrap()
        .join()
        .unwrap();
    drop(dir);

    for (rotulo, ir) in [("trivial", TRIVIAL), ("IR do emissor", emitido.as_str())] {
        let (p50, p95) = medir(n, || {
            let mut sessao = JitSession::new().unwrap();
            let t = Instant::now();
            sessao.add_ir_module("m", ir).unwrap();
            sessao.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  carregar pelo IR, {rotulo} ({} bytes): análise + verificação + geração, mediana {:.3} ms, p95 {:.3} ms",
            ir.len(),
            ms(p50),
            ms(p95)
        );
        let (c50, c95) = medir(n, || {
            let t = Instant::now();
            compile_module("m", ir).unwrap();
            t.elapsed()
        });
        println!("      compilar para o cache, {rotulo}: mediana {:.3} ms, p95 {:.3} ms (uma vez por módulo)", ms(c50), ms(c95));
        let compilado = compile_module("m", ir).unwrap();
        let (p50, p95) = medir(n, || {
            let mut sessao = JitSession::new().unwrap();
            let t = Instant::now();
            sessao.add_compiled_module(&compilado).unwrap();
            sessao.lookup(dartforge_jit::ENTRY_SYMBOL).unwrap();
            t.elapsed()
        });
        println!(
            "(ii)  carregar do cache, {rotulo} ({} bytes de objeto): mediana {:.3} ms, p95 {:.3} ms",
            compilado.object().len(),
            ms(p50),
            ms(p95)
        );
        let mut sessao = JitSession::new().unwrap();
        sessao.add_compiled_module(&compilado).unwrap();
        let (p50, p95) = medir(100, || sessao.run_entry().unwrap().total);
        println!("(iii) uma execução, {rotulo}, sessão aquecida: mediana {:.3} ms, p95 {:.3} ms", ms(p50), ms(p95));
    }
}
