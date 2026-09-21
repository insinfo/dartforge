//! A rota rápida de unidade isolada não pode engolir o prefixo de diretivas.
//!
//! Um arquivo sem `import`/`export` ainda pode declarar `library` ou `part`.
//! Entregá-lo inteiro ao parser faria a diretiva virar erro de sintaxe, porque
//! quem remove o prefixo é o linker. Estes testes fixam o limite da rota rápida.
use dartforge_compiler::{CompileOptions, compile_path_with_options};
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
                "dartforge-isolada-{}-{}",
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

/// Uma entrada com `library` e sem imports compila como qualquer outra.
#[test]
fn a_lone_library_directive_still_compiles() {
    let fixture = Fixture::new();
    let entry = fixture.write(
        "main.dart",
        "library minha;\nint valor() { return 5; }\nvoid main() { print(valor()); }\n",
    );
    let javascript = compile_path_with_options(&entry, CompileOptions::default()).unwrap();
    assert!(javascript.contains("export function main"));
    assert!(javascript.contains("return 5"));
}

/// Uma entrada com `part` e sem imports também não pode cair na rota rápida.
#[test]
fn an_entry_with_a_part_and_no_imports_still_compiles() {
    let fixture = Fixture::new();
    fixture.write(
        "parte.dart",
        "part of 'main.dart';\nint _auxiliar() { return 7; }\n",
    );
    let entry = fixture.write(
        "main.dart",
        "part 'parte.dart';\nvoid main() { print(_auxiliar()); }\n",
    );
    let javascript = compile_path_with_options(&entry, CompileOptions::default()).unwrap();
    assert!(javascript.contains("return 7"));
}

/// Sem diretiva alguma a rota rápida continua valendo e produz o mesmo programa.
#[test]
fn a_file_without_directives_keeps_using_the_fast_path() {
    let fixture = Fixture::new();
    let entry = fixture.write(
        "main.dart",
        "int valor() { return 5; }\nvoid main() { print(valor()); }\n",
    );
    let javascript = compile_path_with_options(&entry, CompileOptions::default()).unwrap();
    assert!(javascript.contains("export function main"));
    assert!(javascript.contains("return 5"));
}
