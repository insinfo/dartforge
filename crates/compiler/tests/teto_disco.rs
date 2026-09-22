//! Todo cache que sobreviva a uma requisição nasce com teto e despejo.
//!
//! O `DiskCache` grava um arquivo por entrada distinta e sobrevive ao processo:
//! sem teto, o diretório cresce sem limite entre execuções. Estes testes enchem
//! além do teto e afirmam que o tamanho não passa dele, com `hits`/`misses`/
//! `evictions` observáveis no molde do cache de macros.
use dartforge_compiler::{CompilerSession, DiskCache, Optimization};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Diretório exclusivo do teste, removido ao final.
struct Fixture(PathBuf);

impl Fixture {
    /// Reserva um caminho novo mesmo após uma execução anterior interrompida.
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "dartforge-teto-{}-{}",
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
    /// Remove o diretório exclusivo; este teste não cria links simbólicos.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Seis entradas distintas com teto de quatro: o diretório não passa de quatro
/// registros e as expulsões aparecem nos contadores.
#[test]
fn gravar_alem_do_teto_mantem_o_limite() {
    let fixture = Fixture::new();
    let cache = DiskCache::new(&fixture.0.join("cache")).with_max_entries(4);
    assert_eq!(cache.max_entries(), 4);
    let mut session = CompilerSession::new().with_disk_cache(cache.clone());
    for indice in 0..6 {
        let entrada = fixture.write(
            &format!("prog{indice}.dart"),
            &format!("void main() {{ print({indice}); }}"),
        );
        session
            .compile_path(&entrada, Optimization::None)
            .expect("entrada do teto deve compilar");
    }
    assert_eq!(cache.entries(), 4);
    assert!(
        cache.stats().evictions >= 2,
        "seis gravações com teto quatro exigem ao menos duas expulsões"
    );
}

/// Teto zero desativa a gravação, como orçamento zero de bytes já fazia.
#[test]
fn teto_zero_nao_retem() {
    let fixture = Fixture::new();
    let cache = DiskCache::new(&fixture.0.join("cache")).with_max_entries(0);
    let mut session = CompilerSession::new().with_disk_cache(cache.clone());
    let entrada = fixture.write("prog.dart", "void main() { print(1); }");
    session.compile_path(&entrada, Optimization::None).unwrap();
    assert_eq!(cache.entries(), 0);
}

/// O acerto atualiza a recência: com teto dois, tocar A antes de gravar C
/// expulsa B, o menos usado — LRU aproximado por mtime, não FIFO.
#[test]
fn acerto_atualiza_recencia_antes_da_expulsao() {
    let fixture = Fixture::new();
    let cache = DiskCache::new(&fixture.0.join("cache")).with_max_entries(2);
    let pausa = || std::thread::sleep(Duration::from_millis(25));
    let compilar = |session: &mut CompilerSession, nome: &str, valor: i32| {
        let entrada = fixture.write(nome, &format!("void main() {{ print({valor}); }}"));
        session
            .compile_path(&entrada, Optimization::None)
            .expect("entrada LRU deve compilar");
        pausa();
    };

    let mut primeira = CompilerSession::new().with_disk_cache(cache.clone());
    compilar(&mut primeira, "a.dart", 1);
    compilar(&mut primeira, "b.dart", 2);
    // Toca A numa sessão nova: sem nada em memória, o acerto só pode vir do disco.
    let mut leitora = CompilerSession::new().with_disk_cache(cache.clone());
    let a = fixture.0.join("a.dart");
    assert!(
        leitora
            .compile_path(&a, Optimization::None)
            .unwrap()
            .stats
            .cache_hit,
        "A deveria acertar no disco antes da expulsão"
    );
    pausa();
    // Gravar C estoura o teto: B, o menos usado, é expulso; A sobrevive.
    compilar(&mut primeira, "c.dart", 3);
    assert_eq!(cache.entries(), 2);
    let mut confere = CompilerSession::new().with_disk_cache(cache.clone());
    assert!(
        confere
            .compile_path(&a, Optimization::None)
            .unwrap()
            .stats
            .cache_hit,
        "A foi tocado e deveria sobreviver à expulsão"
    );
    assert!(
        !confere
            .compile_path(&fixture.0.join("b.dart"), Optimization::None)
            .unwrap()
            .stats
            .cache_hit,
        "B, o menos usado, deveria ter sido expulso"
    );
}
