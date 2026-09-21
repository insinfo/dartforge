//! Validade do reuso: compilar incrementalmente tem de dar o mesmo resultado
//! que compilar do zero.
//!
//! A sessão passou a reaproveitar a estrutura do grafo quando só os corpos dos
//! arquivos mudaram. Esse reuso não pode alterar nada observável: nem o
//! JavaScript emitido, nem os diagnósticos, nem o arquivo e o span de um erro.
//! Os testes abaixo percorrem sequências de edições, exclusões e renomeações,
//! não apenas uma edição isolada, porque é numa sequência que um cache errado
//! costuma sobreviver.
use dartforge_compiler::{CompilerSession, Optimization, compile_path_with_report};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo por teste, removido ao final.
struct Fixture(PathBuf);

impl Fixture {
    /// Reserva um caminho novo mesmo após uma execução anterior interrompida.
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-incremental-{}-{}",
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
    /// Remove um arquivo do diretório reservado.
    fn remove(&self, name: &str) {
        std::fs::remove_file(self.0.join(name)).unwrap();
    }
}

impl Drop for Fixture {
    /// Remove o diretório exclusivo; estes testes não criam links simbólicos.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Compila pela sessão e por uma rota limpa, exigindo o mesmo texto.
///
/// É a propriedade central do cache: o resultado observável de uma compilação
/// incremental é idêntico ao de uma compilação limpa das mesmas entradas.
fn agree(session: &mut CompilerSession, entry: &Path) -> String {
    let incremental = session.compile_path(entry, Optimization::None).unwrap();
    let (clean, _) = compile_path_with_report(entry, Default::default()).unwrap();
    assert_eq!(
        &*incremental.javascript, &clean,
        "saída incremental divergiu da compilação limpa"
    );
    clean
}

/// Uma sequência longa de edições de corpo nunca serve um resultado antigo.
#[test]
fn a_sequence_of_body_edits_always_matches_a_clean_build() {
    let fixture = Fixture::new();
    fixture.write("dep.dart", "int valor() { return 1; }");
    let entry = fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(valor()); }",
    );
    let mut session = CompilerSession::new();
    let mut seen = Vec::new();
    for revision in 1..=8 {
        fixture.write("dep.dart", &format!("int valor() {{ return {revision}; }}"));
        let javascript = agree(&mut session, &entry);
        assert!(javascript.contains(&format!("return {revision}")));
        seen.push(javascript);
    }
    // Cada revisão produziu um texto próprio: nenhuma foi servida do cache.
    for (index, javascript) in seen.iter().enumerate() {
        assert!(
            seen.iter().skip(index + 1).all(|other| other != javascript),
            "duas revisões distintas produziram o mesmo JavaScript"
        );
    }
}

/// Acrescentar e remover um import muda a estrutura e não pode ser reaproveitado.
#[test]
fn adding_and_removing_an_import_reloads_the_graph() {
    let fixture = Fixture::new();
    fixture.write("a.dart", "int umA() { return 1; }");
    fixture.write("b.dart", "int umB() { return 2; }");
    let entry = fixture.write(
        "main.dart",
        "import 'a.dart';\nvoid main() { print(umA()); }",
    );
    let mut session = CompilerSession::new();
    let so_a = agree(&mut session, &entry);
    assert!(!so_a.contains("umB"));
    fixture.write(
        "main.dart",
        "import 'a.dart';\nimport 'b.dart';\nvoid main() { print(umA() + umB()); }",
    );
    let com_b = agree(&mut session, &entry);
    assert!(com_b.contains("umB"));
    fixture.write(
        "main.dart",
        "import 'a.dart';\nvoid main() { print(umA()); }",
    );
    let sem_b = agree(&mut session, &entry);
    assert_eq!(sem_b, so_a);
}

/// Uma diretiva acrescentada logo após as anteriores precisa ser percebida.
///
/// Comparar apenas o prefixo de diretivas do texto anterior deixaria este caso
/// passar: os primeiros bytes continuam idênticos e a diretiva nova vem depois.
#[test]
fn a_directive_appended_after_the_previous_prefix_is_detected() {
    let fixture = Fixture::new();
    fixture.write("a.dart", "int umA() { return 1; }");
    fixture.write("b.dart", "int umB() { return 2; }");
    let entry = fixture.write(
        "main.dart",
        "import 'a.dart';\nvoid main() { print(umA()); }",
    );
    let mut session = CompilerSession::new();
    agree(&mut session, &entry);
    fixture.write(
        "main.dart",
        "import 'a.dart';\nimport 'b.dart';\nvoid main() { print(umB()); }",
    );
    let depois = agree(&mut session, &entry);
    assert!(depois.contains("umB"));
}

/// Remover um arquivo importado produz erro, e restaurá-lo recompila.
#[test]
fn deleting_and_restoring_a_dependency_never_serves_stale_output() {
    let fixture = Fixture::new();
    fixture.write("dep.dart", "int valor() { return 1; }");
    let entry = fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(valor()); }",
    );
    let mut session = CompilerSession::new();
    let antes = agree(&mut session, &entry);
    fixture.remove("dep.dart");
    assert!(session.compile_path(&entry, Optimization::None).is_err());
    fixture.write("dep.dart", "int valor() { return 9; }");
    let depois = agree(&mut session, &entry);
    assert_ne!(antes, depois);
    assert!(depois.contains("return 9"));
}

/// Renomear o arquivo e a diretiva preserva o comportamento e o texto emitido.
#[test]
fn renaming_a_library_keeps_the_observable_result() {
    let fixture = Fixture::new();
    fixture.write("antigo.dart", "int valor() { return 7; }");
    let entry = fixture.write(
        "main.dart",
        "import 'antigo.dart';\nvoid main() { print(valor()); }",
    );
    let mut session = CompilerSession::new();
    let antes = agree(&mut session, &entry);
    fixture.write("novo.dart", "int valor() { return 7; }");
    fixture.remove("antigo.dart");
    fixture.write(
        "main.dart",
        "import 'novo.dart';\nvoid main() { print(valor()); }",
    );
    let depois = agree(&mut session, &entry);
    assert_eq!(antes, depois);
}

/// Um erro semântico introduzido por edição aponta o arquivo e o span certos.
#[test]
fn an_error_introduced_by_an_edit_keeps_its_file_and_span() {
    let fixture = Fixture::new();
    fixture.write("dep.dart", "int valor() { return 1; }");
    let entry = fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(valor()); }",
    );
    let mut session = CompilerSession::new();
    agree(&mut session, &entry);
    fixture.write("dep.dart", "int valor() { return ausente; }");
    let incremental = session
        .compile_path(&entry, Optimization::None)
        .unwrap_err();
    let clean = compile_path_with_report(&entry, Default::default()).unwrap_err();
    assert_eq!(incremental.path, clean.path);
    assert_eq!(incremental.message, clean.message);
    assert_eq!(incremental.span, clean.span);
    assert!(incremental.path.ends_with("dep.dart"));
}

/// Editar um arquivo preservando mtime e tamanho continua invalidando o cache.
///
/// A revalidação rápida relê o conteúdo de todos os arquivos conhecidos; ela não
/// pode ter introduzido um atalho por metadados do sistema de arquivos.
#[test]
fn an_edit_with_identical_size_and_mtime_is_still_detected() {
    let fixture = Fixture::new();
    let dep = fixture.write("dep.dart", "int valor() { return 1; }");
    let entry = fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(valor()); }",
    );
    let metadata = std::fs::metadata(&dep).unwrap();
    let modified = metadata.modified().unwrap();
    let mut session = CompilerSession::new();
    let antes = agree(&mut session, &entry);
    fixture.write("dep.dart", "int valor() { return 2; }");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&dep)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let agora = std::fs::metadata(&dep).unwrap();
    assert_eq!(agora.len(), metadata.len());
    assert_eq!(agora.modified().unwrap(), modified);
    let depois = agree(&mut session, &entry);
    assert_ne!(antes, depois);
}

/// Uma parte editada pertence à biblioteca declarante e invalida com ela.
#[test]
fn editing_a_part_invalidates_its_declaring_library() {
    let fixture = Fixture::new();
    fixture.write(
        "lib.dart",
        "part 'parte.dart';\nint publico() { return _privado(); }",
    );
    fixture.write(
        "parte.dart",
        "part of 'lib.dart';\nint _privado() { return 1; }",
    );
    let entry = fixture.write(
        "main.dart",
        "import 'lib.dart';\nvoid main() { print(publico()); }",
    );
    let mut session = CompilerSession::new();
    let antes = agree(&mut session, &entry);
    fixture.write(
        "parte.dart",
        "part of 'lib.dart';\nint _privado() { return 4; }",
    );
    let depois = agree(&mut session, &entry);
    assert_ne!(antes, depois);
    assert!(depois.contains("return 4"));
}
