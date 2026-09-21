//! O cache em disco tem de valer para um processo novo sem afrouxar garantias.
//!
//! Cada teste simula o cenário real: uma sessão grava o registro e **outra
//! sessão, criada do zero**, tenta reaproveitá-lo. Uma sessão nova não tem nada
//! em memória, então um acerto aqui só pode ter vindo do disco.
use dartforge_compiler::{CompilerSession, DiskCache, Optimization, compile_path_with_report};
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
                "dartforge-disco-{}-{}",
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
    /// Diretório onde os registros do cache são gravados.
    fn cache(&self) -> PathBuf {
        self.0.join("cache")
    }
}

impl Drop for Fixture {
    /// Remove o diretório exclusivo; estes testes não criam links simbólicos.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Cria uma sessão nova ligada ao cache do fixture.
///
/// Sessão nova é o ponto do teste: sem estado em memória, qualquer acerto
/// necessariamente veio do disco.
fn fresh(fixture: &Fixture) -> CompilerSession {
    CompilerSession::new().with_disk_cache(DiskCache::new(&fixture.cache()))
}

/// Monta um programa de duas bibliotecas e devolve a entrada.
fn build(fixture: &Fixture, valor: i32) -> PathBuf {
    fixture.write("dep.dart", &format!("int valor() {{ return {valor}; }}"));
    fixture.write(
        "main.dart",
        "import 'dep.dart';\nvoid main() { print(valor()); }",
    )
}

/// Um processo novo reaproveita o registro e não compila nada.
#[test]
fn a_fresh_session_reuses_the_recorded_output() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 1);
    let primeira = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    assert!(!primeira.report.cache_hit);
    assert!(primeira.report.link.analyze_ns > 0);

    let segunda = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    assert!(segunda.report.cache_hit, "esperava acerto vindo do disco");
    assert_eq!(segunda.stats.compiled_units, 0);
    // Nenhuma fase do front-end rodou: o registro dispensou o pipeline inteiro.
    assert_eq!(segunda.report.link.analyze_ns, 0);
    assert_eq!(segunda.report.link.parse_ns, 0);
    assert_eq!(&*segunda.javascript, &*primeira.javascript);
}

/// A saída do disco é idêntica à de uma compilação limpa.
#[test]
fn the_recorded_output_matches_a_clean_build() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 7);
    fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    let do_disco = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    let (limpa, _) = compile_path_with_report(&entry, Default::default()).unwrap();
    assert_eq!(&*do_disco.javascript, &limpa);
}

/// Editar preservando mtime e tamanho continua invalidando o registro.
///
/// É a garantia central do projeto: a verificação é por conteúdo exato, nunca
/// por metadados. O cache em disco não pode ter aberto uma exceção.
#[test]
fn an_edit_with_identical_size_and_mtime_invalidates_the_record() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 1);
    let dep = fixture.0.join("dep.dart");
    let antes = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    let metadata = std::fs::metadata(&dep).unwrap();
    let modified = metadata.modified().unwrap();

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

    let depois = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    assert!(!depois.report.cache_hit, "metadados não podem bastar");
    assert_ne!(&*depois.javascript, &*antes.javascript);
    assert!(depois.javascript.contains("return 2"));
}

/// Opções diferentes não podem compartilhar registro.
#[test]
fn a_different_option_never_reads_the_other_record() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 3);
    fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    let constantes = fresh(&fixture)
        .compile_path(&entry, Optimization::Constants)
        .unwrap();
    assert!(!constantes.report.cache_hit);
    // Depois de gravado, o registro de Constants passa a valer para ele mesmo.
    let de_novo = fresh(&fixture)
        .compile_path(&entry, Optimization::Constants)
        .unwrap();
    assert!(de_novo.report.cache_hit);
    // E o registro anterior continua válido para a sua própria opção.
    let sem = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    assert!(sem.report.cache_hit);
}

/// Um arquivo importado que some invalida o registro em vez de servi-lo.
#[test]
fn a_missing_dependency_is_not_served_from_disk() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 1);
    fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    std::fs::remove_file(fixture.0.join("dep.dart")).unwrap();
    assert!(
        fresh(&fixture)
            .compile_path(&entry, Optimization::None)
            .is_err()
    );
}

/// Um registro corrompido é ignorado e a compilação segue normalmente.
#[test]
fn a_corrupted_record_falls_back_to_compiling() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 5);
    fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    for arquivo in std::fs::read_dir(fixture.cache()).unwrap() {
        let arquivo = arquivo.unwrap().path();
        let mut bytes = std::fs::read(&arquivo).unwrap();
        bytes.truncate(bytes.len() / 2);
        std::fs::write(&arquivo, bytes).unwrap();
    }
    let depois = fresh(&fixture)
        .compile_path(&entry, Optimization::None)
        .unwrap();
    assert!(!depois.report.cache_hit);
    assert!(depois.javascript.contains("return 5"));
}

/// Orçamento zero desliga a gravação sem desligar a compilação.
#[test]
fn a_zero_budget_disables_recording_only() {
    let fixture = Fixture::new();
    let entry = build(&fixture, 4);
    let cache = DiskCache::new(&fixture.cache()).with_max_entry_bytes(0);
    let mut sessao = CompilerSession::new().with_disk_cache(cache);
    assert!(sessao.compile_path(&entry, Optimization::None).is_ok());
    assert!(!Path::new(&fixture.cache()).exists());
}
