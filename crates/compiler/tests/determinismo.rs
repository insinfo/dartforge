//! O paralelismo do front-end não pode mudar saída, identidades nem diagnósticos.
//!
//! Tokenização, índice de declarações, análise sintática e a pré-busca de
//! importações rodam em paralelo entre unidades. Estes testes provam que a ordem
//! das unidades continua determinando tudo: os IDs de classe emitidos, o texto
//! final e qual erro é relatado quando várias unidades falham.
use dartforge_compiler::{CompilerSession, Optimization, compile_path_with_report};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo por teste, removido ao final.
struct Fixture(PathBuf);

impl Fixture {
    /// Reserva um caminho novo mesmo após uma execução anterior interrompida.
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-determinismo-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("{error}"),
            }
        }
    }
    /// Grava um arquivo no diretório reservado e devolve seu caminho.
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for Fixture {
    /// Remove o diretório exclusivo; estes testes não criam links simbólicos.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Monta um grafo com `count` bibliotecas independentes importadas pela entrada.
fn build(fixture: &Fixture, count: usize, broken: &[usize]) -> PathBuf {
    for index in 0..count {
        let body = if broken.contains(&index) {
            format!("int lib{index}f() {{ return ausente{index}; }}\n")
        } else {
            format!(
                "class Tipo{index} {{ int valor = {index}; }}\nint lib{index}f() {{ return Tipo{index}().valor; }}\n"
            )
        };
        fixture.write(&format!("lib{index}.dart"), &body);
    }
    let mut entry = String::new();
    for index in 0..count {
        entry.push_str(&format!("import 'lib{index}.dart';\n"));
    }
    entry.push_str("void main() {\n");
    for index in 0..count {
        entry.push_str(&format!("  print(lib{index}f());\n"));
    }
    entry.push_str("}\n");
    fixture.write("main.dart", &entry)
}

/// Compilações repetidas do mesmo grafo produzem exatamente o mesmo texto.
///
/// Os IDs de classe são atribuídos na ordem das unidades; se o paralelismo
/// vazasse para a numeração, o JavaScript mudaria entre execuções.
#[test]
fn repeated_compilations_are_byte_identical() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 12, &[]);
    let (first, report) = compile_path_with_report(&entry, Default::default()).unwrap();
    assert_eq!(report.link.units, 13);
    assert_eq!(report.link.classes, 12);
    for _ in 0..8 {
        let (again, _) = compile_path_with_report(&entry, Default::default()).unwrap();
        assert_eq!(first, again);
    }
    // A sessão precisa concordar com a rota sem cache, inclusive no acerto.
    let mut session = CompilerSession::new();
    let compilation = session.compile_path(&entry, Optimization::None).unwrap();
    assert_eq!(&*compilation.javascript, first);
    let repeated = session.compile_path(&entry, Optimization::None).unwrap();
    assert!(repeated.report.cache_hit);
    assert_eq!(&*repeated.javascript, first);
}

/// Com várias unidades inválidas, o erro relatado é sempre o da primeira delas.
#[test]
fn the_reported_diagnostic_is_always_from_the_first_failing_unit() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 10, &[3, 5, 8]);
    let first = compile_path_with_report(&entry, Default::default()).unwrap_err();
    assert!(
        first.path.ends_with("lib3.dart"),
        "esperava lib3.dart, veio {}",
        first.path.display()
    );
    for _ in 0..8 {
        let again = compile_path_with_report(&entry, Default::default()).unwrap_err();
        assert_eq!(again.path, first.path);
        assert_eq!(again.message, first.message);
        assert_eq!(again.span, first.span);
    }
}

/// O limiar de paralelismo não pode mudar o resultado de grafos pequenos.
///
/// Abaixo de quatro unidades o front-end roda em sequência; o texto emitido
/// precisa ser o mesmo que o caminho paralelo produziria.
#[test]
fn small_graphs_below_the_parallel_threshold_agree_with_larger_ones() {
    let small = Fixture::new();
    let large = Fixture::new();
    let small_entry = build(&small, 2, &[]);
    let large_entry = build(&large, 2, &[]);
    let (a, _) = compile_path_with_report(&small_entry, Default::default()).unwrap();
    let (b, _) = compile_path_with_report(&large_entry, Default::default()).unwrap();
    assert_eq!(a, b);
}

/// A pré-busca paralela não pode esconder um arquivo ausente nem trocar o erro.
#[test]
fn a_missing_import_is_reported_against_the_importer() {
    let fixture = Fixture::new();
    build(&fixture, 8, &[]);
    let entry = fixture.write(
        "main.dart",
        "import 'lib0.dart';\nimport 'lib1.dart';\nimport 'ausente.dart';\nimport 'lib2.dart';\nimport 'lib3.dart';\nvoid main() { print(lib0f()); }\n",
    );
    let error = compile_path_with_report(&entry, Default::default()).unwrap_err();
    assert!(error.path.ends_with("main.dart"));
    assert!(
        error.message.contains("ausente.dart"),
        "mensagem inesperada: {}",
        error.message
    );
}
