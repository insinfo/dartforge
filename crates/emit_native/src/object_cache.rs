//! Cache de objeto: o `.obj` que o Clang produz de um LLVM IR, por hash.
//!
//! É a disciplina do cache do runtime (`cache.rs`) aplicada ao programa:
//! mesma entrada, mesma chave, mesmo `.obj`, e o Clang não roda de novo
//! (PESQUISA-OTIMIZACAO §6 e §11). A chave é o IR inteiro mais tudo o que
//! muda o objeto sem mudar o IR: a identidade do Clang (`clang --version`
//! completo), as bandeiras, a versão do DartForge e a do formato deste cache.
//!
//! Hoje o módulo é UM por programa, com os corpos do SDK dentro e índices
//! globais de função e de classe, então a unidade de cache é o programa
//! inteiro: acerta ao repetir o mesmo programa (as passadas do harness, uma
//! nova passada depois de mudar só o runtime ou o harness, a iteração local) e
//! erra em qualquer mudança do emissor. O cache por módulo, com o SDK
//! compartilhado entre programas, depende de separar o SDK em módulo próprio
//! com símbolos estáveis (ESTADO.md §2.5).
//!
//! Disco: `<native_cache_dir>/obj/<2 hex>/<32 hex>.obj`. O objeto só aparece
//! com o nome final inteiro (`rename` de um temporário), nunca é
//! sobrescrito, e a poda apaga os mais antigos por data de uso quando o total
//! passa do teto — nunca um objeto que este processo obteve (a ligação ainda
//! vai lê-lo) nem um usado há menos de [`MIN_AGE_TO_PRUNE`] (o de outro
//! processo que ainda liga). Um programa maior que o teto (o executor de
//! builders, com o `analyzer`) fica no cache até a próxima poda depois disso.

use crate::cache::native_cache_dir;
use crate::resumo::Fnv128;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Muda quando o que vai no cache muda de significado (bandeiras implícitas,
/// forma do nome do arquivo): invalida tudo o que foi gravado antes.
pub const FORMAT_VERSION: u32 = 1;

/// Teto padrão do cache em disco; `DARTFORGE_CACHE_OBJ_MB` ajusta. Cabe as
/// partes de um programa grande (o new_sali/backend: ~60 objetos, ~600 MB,
/// docs/NATIVO-PROJETOS-REAIS.md C9) — com 256 MB a poda tirava as partes
/// e a recompilação nunca as achava.
pub const DEFAULT_CAP_MB: u64 = 2048;

/// A cada quantas inserções a poda roda de novo.
const PRUNE_EVERY: usize = 16;

/// Temporários mais velhos que isto são sobra de processo que morreu.
const ORPHAN_TEMP_AGE: Duration = Duration::from_secs(3600);

/// Objeto usado há menos que isto não é podado: outro processo pode tê-lo
/// acabado de obter e ainda não ter ligado (a ligação de um programa grande
/// leva minutos).
pub const MIN_AGE_TO_PRUNE: Duration = Duration::from_secs(10 * 60);

/// `clang --version` inteiro (versão, alvo, modelo de threads, diretório de
/// instalação), uma vez por caminho do Clang e por processo.
pub fn clang_identity(clang: &Path) -> Result<String, String> {
    static IDENTITIES: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let map = IDENTITIES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = map.lock().unwrap().get(clang) {
        return Ok(id.clone());
    }
    let exit = std::process::Command::new(clang)
        .arg("--version")
        .output()
        .map_err(|e| format!("falha ao executar {} --version: {e}", clang.display()))?;
    if !exit.status.success() {
        return Err(format!("{} --version falhou (código {:?})", clang.display(), exit.status));
    }
    let id = String::from_utf8_lossy(&exit.stdout).into_owned();
    map.lock().unwrap().insert(clang.to_path_buf(), id.clone());
    Ok(id)
}

/// A chave de um objeto. Cada componente é separado por `\0`, para que duas
/// divisões diferentes das mesmas bytes não colidam.
pub fn key(ir: &str, clang_id: &str, args: &[&str]) -> u128 {
    bytes_key(ir.as_bytes(), clang_id, args)
}

/// Como [`key`], para uma entrada que não é texto (o bitcode de uma parte
/// do ThinLTO distribuído).
pub fn bytes_key(ir: &[u8], clang_id: &str, args: &[&str]) -> u128 {
    let mut h = Fnv128::default();
    h.write(b"dartforge-obj\0")
        .write(&FORMAT_VERSION.to_le_bytes())
        .write(env!("CARGO_PKG_VERSION").as_bytes())
        .write(b"\0")
        .write(clang_id.as_bytes())
        .write(b"\0");
    for a in args {
        h.write(a.as_bytes()).write(b"\0");
    }
    h.write(ir);
    h.end()
}

/// O cache de objetos de um diretório.
#[derive(Debug)]
pub struct ObjectCache {
    dir: PathBuf,
    cap_bytes: u64,
    /// Chaves sendo criadas neste processo: a segunda thread com a mesma chave
    /// espera a primeira em vez de rodar outro Clang.
    in_progress: Mutex<HashSet<u128>>,
    finished: Condvar,
    insertions: AtomicUsize,
    tmp_counter: AtomicUsize,
    /// Objetos que este processo obteve: a poda não os apaga.
    in_use: Mutex<HashSet<PathBuf>>,
}

impl ObjectCache {
    /// Um cache em `dir` (o diretório `obj/` fica dentro dele).
    pub fn new(dir: PathBuf, cap_bytes: u64) -> ObjectCache {
        let c = ObjectCache {
            dir: dir.join("obj"),
            cap_bytes,
            in_progress: Mutex::new(HashSet::new()),
            finished: Condvar::new(),
            insertions: AtomicUsize::new(0),
            tmp_counter: AtomicUsize::new(0),
            in_use: Mutex::new(HashSet::new()),
        };
        c.prune();
        c
    }

    /// O cache do processo em [`native_cache_dir`], ou `None` com
    /// `DARTFORGE_CACHE_OBJ=0`. O teto vem de `DARTFORGE_CACHE_OBJ_MB`.
    pub fn from_env() -> Option<&'static ObjectCache> {
        static CACHE: OnceLock<Option<ObjectCache>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                if std::env::var("DARTFORGE_CACHE_OBJ").is_ok_and(|v| v.trim() == "0") {
                    return None;
                }
                let mb = std::env::var("DARTFORGE_CACHE_OBJ_MB")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(DEFAULT_CAP_MB);
                Some(ObjectCache::new(native_cache_dir(), mb * 1024 * 1024))
            })
            .as_ref()
    }

    pub fn path(&self, key: u128) -> PathBuf {
        let hex = format!("{key:032x}");
        self.dir.join(&hex[..2]).join(format!("{hex}.obj"))
    }

    /// Devolve o objeto da `key`, criando-o com `create` se ainda não existe.
    ///
    /// `create` recebe o caminho temporário onde deve escrever o objeto, dentro
    /// de um diretório só dele (onde pode deixar o que quiser, como o `.ll`:
    /// o diretório inteiro é apagado depois). O objeto só é publicado se
    /// `create` teve sucesso e escreveu algo. O `bool` diz se foi acerto.
    pub fn get_or_create(
        &self,
        key: u128,
        create: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(PathBuf, bool), String> {
        let final_ = self.path(key);
        {
            let mut progress = self.in_progress.lock().unwrap();
            loop {
                if whole_exists(&final_) {
                    drop(progress);
                    touch(&final_);
                    self.in_use.lock().unwrap_or_else(|e| e.into_inner()).insert(final_.clone());
                    return Ok((final_, true));
                }
                if progress.insert(key) {
                    break;
                }
                progress = self.finished.wait(progress).unwrap();
            }
        }
        // Daqui até o fim a chave é nossa; o guarda a devolve mesmo em pânico.
        let _guard = InProgress { cache: self, key };

        let hex = format!("{key:032x}");
        let n = self.tmp_counter.fetch_add(1, Ordering::Relaxed);
        let dir_tmp = self.dir.join("tmp").join(format!("{hex}.{}.{n}", std::process::id()));
        std::fs::create_dir_all(&dir_tmp)
            .map_err(|e| format!("não foi possível criar {}: {e}", dir_tmp.display()))?;
        let tmp = dir_tmp.join(format!("{hex}.obj"));
        let r = create(&tmp).and_then(|()| self.publish(&tmp, &final_));
        let _ = std::fs::remove_dir_all(&dir_tmp);
        let hit = r?;
        self.in_use.lock().unwrap_or_else(|e| e.into_inner()).insert(final_.clone());
        if !hit && self.insertions.fetch_add(1, Ordering::Relaxed) % PRUNE_EVERY == PRUNE_EVERY - 1 {
            self.prune();
        }
        Ok((final_, hit))
    }

    /// Move o temporário para o nome final. `Ok(true)` quando outro processo
    /// publicou a mesma chave primeiro (vale o dele, que tem o mesmo conteúdo).
    fn publish(&self, tmp: &Path, final_: &Path) -> Result<bool, String> {
        if !std::fs::metadata(tmp).is_ok_and(|m| m.len() > 0) {
            return Err(format!("o objeto {} não foi escrito", tmp.display()));
        }
        if let Some(parent) = final_.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("não foi possível criar {}: {e}", parent.display()))?;
        }
        if whole_exists(final_) {
            return Ok(true);
        }
        match std::fs::rename(tmp, final_) {
            Ok(()) => Ok(false),
            Err(_) if whole_exists(final_) => Ok(true),
            Err(e) => Err(format!("não foi possível publicar {}: {e}", final_.display())),
        }
    }

    /// Tira uma entrada (objeto que o ligador recusou).
    pub fn remove(&self, key: u128) {
        let _ = std::fs::remove_file(self.path(key));
    }

    /// Apaga os objetos menos usados até o total caber no teto, e temporários
    /// órfãos de processos que morreram.
    pub fn prune(&self) {
        let now = SystemTime::now();
        if let Ok(tmps) = std::fs::read_dir(self.dir.join("tmp")) {
            for t in tmps.flatten() {
                let old_one = t
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| now.duration_since(m).is_ok_and(|d| d > ORPHAN_TEMP_AGE));
                if old_one {
                    let _ = std::fs::remove_dir_all(t.path());
                }
            }
        }
        let mut objects: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let in_use = self.in_use.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let Ok(subdirs) = std::fs::read_dir(&self.dir) else { return };
        for sub in subdirs.flatten() {
            if sub.file_name() == "tmp" {
                continue;
            }
            let Ok(files) = std::fs::read_dir(sub.path()) else { continue };
            for a in files.flatten() {
                let p = a.path();
                if p.extension().is_some_and(|e| e == "obj")
                    && let Ok(m) = a.metadata()
                {
                    objects.push((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), p));
                }
            }
        }
        let mut total: u64 = objects.iter().map(|o| o.1).sum();
        if total <= self.cap_bytes {
            return;
        }
        objects.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
        for (data, size, p) in objects {
            if total <= self.cap_bytes {
                break;
            }
            let recent = now.duration_since(data).map_or(true, |d| d < MIN_AGE_TO_PRUNE);
            if recent || in_use.contains(&p) {
                continue;
            }
            if std::fs::remove_file(&p).is_ok() {
                total -= size;
            }
        }
    }
}

/// Devolve a chave em andamento e acorda quem espera por ela.
struct InProgress<'a> {
    cache: &'a ObjectCache,
    key: u128,
}

impl Drop for InProgress<'_> {
    fn drop(&mut self) {
        self.cache.in_progress.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.key);
        self.cache.finished.notify_all();
    }
}

fn whole_exists(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// Marca o uso de um objeto (a poda apaga os menos usados primeiro). Falha em
/// silêncio: o objeto pode estar aberto pelo ligador de outro processo.
fn touch(p: &Path) {
    if let Ok(f) = std::fs::File::options().write(true).open(p) {
        let _ = f.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(content: &'static [u8]) -> impl FnOnce(&Path) -> Result<(), String> {
        move |p: &Path| std::fs::write(p, content).map_err(|e| e.to_string())
    }

    fn files_in(dir: &Path) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Ok(es) = std::fs::read_dir(dir) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    v.extend(files_in(&p));
                } else {
                    v.push(p);
                }
            }
        }
        v
    }

    #[test]
    fn key_changes_with_each_component_and_is_stable() {
        let base = key("ir", "clang 22", &["-c", "-O0"]);
        assert_eq!(base, key("ir", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, key("ir2", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, key("ir", "clang 23", &["-c", "-O0"]));
        assert_ne!(base, key("ir", "clang 22", &["-c", "-O2"]));
        assert_ne!(base, key("ir", "clang 22", &["-c"]));
        // A separação entre componentes conta: mover um byte de um para outro
        // não pode dar a mesma chave.
        assert_ne!(key("ir", "clang 22", &["-c-O0"]), key("ir", "clang 22", &["-c", "-O0"]));
    }

    #[test]
    fn hit_does_not_call_the_closer() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        let (p, hit) = c.get_or_create(7, write(b"obj")).unwrap();
        assert!(!hit);
        assert_eq!(std::fs::read(&p).unwrap(), b"obj");
        let (p2, hit) = c.get_or_create(7, |_| panic!("não devia criar de novo")).unwrap();
        assert!(hit);
        assert_eq!(p, p2);
    }

    #[test]
    fn eight_threads_same_key_one_clang() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        let calls = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    c.get_or_create(42, |p| {
                        calls.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(20));
                        std::fs::write(p, b"objeto").map_err(|e| e.to_string())
                    })
                    .unwrap()
                });
            }
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let files = files_in(&dir.path().join("obj"));
        assert_eq!(files, vec![c.path(42)], "sobrou temporário: {files:?}");
    }

    #[test]
    fn failure_does_not_publish() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), u64::MAX);
        assert!(c.get_or_create(1, |_| Err("clang falhou".into())).is_err());
        // Sucesso sem escrever nada também não publica.
        assert!(c.get_or_create(1, |_| Ok(())).is_err());
        assert!(c.get_or_create(1, write(b"")).is_err());
        assert!(files_in(&dir.path().join("obj")).is_empty());
        // E a chave não fica presa: a próxima tentativa cria.
        assert!(!c.get_or_create(1, write(b"x")).unwrap().1);
    }

    #[test]
    fn prune_respects_cap() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), 25);
        for k in 0..5u128 {
            c.get_or_create(k, write(b"0123456789")).unwrap();
            // Datas distintas, para a ordem de uso ser a de criação.
            let f = std::fs::File::options().write(true).open(c.path(k)).unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + k as u64)).unwrap();
        }
        // A poda de outro processo (os objetos deste estão em uso).
        ObjectCache::new(dir.path().to_path_buf(), 25).prune();
        let remaining = files_in(&dir.path().join("obj"));
        let total: u64 = remaining.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
        assert!(total <= 25, "{total} bytes");
        // Ficam os usados por último.
        assert_eq!(remaining.len(), 2);
        assert!(remaining.contains(&c.path(3)) && remaining.contains(&c.path(4)), "{remaining:?}");
    }

    /// Um objeto maior que o teto, obtido por este processo ou usado há
    /// pouco por outro, sobrevive à poda: a ligação ainda vai lê-lo.
    #[test]
    fn prune_spares_what_is_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let c = ObjectCache::new(dir.path().to_path_buf(), 5);
        c.get_or_create(1, write(b"0123456789")).unwrap();
        let f = std::fs::File::options().write(true).open(c.path(1)).unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000)).unwrap();
        c.prune();
        assert!(c.path(1).is_file(), "o objeto deste processo foi podado");
        // Outro processo (um cache novo sobre o mesmo diretório): o antigo
        // sai, o recente fica.
        let other = ObjectCache::new(dir.path().to_path_buf(), 5);
        assert!(!c.path(1).is_file());
        c.get_or_create(2, write(b"0123456789")).unwrap();
        other.prune();
        assert!(c.path(2).is_file(), "objeto recente de outro processo podado");
    }
}
