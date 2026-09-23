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
//! Disco: `<dir_cache_nativo>/obj/<2 hex>/<32 hex>.obj`. O objeto só aparece
//! com o nome final inteiro (`rename` de um temporário), nunca é
//! sobrescrito, e a poda apaga os mais antigos por data de uso quando o total
//! passa do teto.

use crate::cache::dir_cache_nativo;
use crate::resumo::Fnv128;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Muda quando o que vai no cache muda de significado (bandeiras implícitas,
/// forma do nome do arquivo): invalida tudo o que foi gravado antes.
pub const VERSAO_FORMATO: u32 = 1;

/// Teto padrão do cache em disco; `DARTFORGE_CACHE_OBJ_MB` ajusta.
pub const TETO_PADRAO_MB: u64 = 256;

/// A cada quantas inserções a poda roda de novo.
const PODAR_A_CADA: usize = 16;

/// Temporários mais velhos que isto são sobra de processo que morreu.
const IDADE_TEMPORARIO_ORFAO: Duration = Duration::from_secs(3600);

/// `clang --version` inteiro (versão, alvo, modelo de threads, diretório de
/// instalação), uma vez por caminho do Clang e por processo.
pub fn identidade_clang(clang: &Path) -> Result<String, String> {
    static IDENTIDADES: OnceLock<Mutex<HashMap<PathBuf, String>>> = OnceLock::new();
    let mapa = IDENTIDADES.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(id) = mapa.lock().unwrap().get(clang) {
        return Ok(id.clone());
    }
    let saida = std::process::Command::new(clang)
        .arg("--version")
        .output()
        .map_err(|e| format!("falha ao executar {} --version: {e}", clang.display()))?;
    if !saida.status.success() {
        return Err(format!("{} --version falhou (código {:?})", clang.display(), saida.status));
    }
    let id = String::from_utf8_lossy(&saida.stdout).into_owned();
    mapa.lock().unwrap().insert(clang.to_path_buf(), id.clone());
    Ok(id)
}

/// A chave de um objeto. Cada componente é separado por `\0`, para que duas
/// divisões diferentes das mesmas bytes não colidam.
pub fn chave(ir: &str, clang_id: &str, args: &[&str]) -> u128 {
    let mut h = Fnv128::default();
    h.escrever(b"dartforge-obj\0")
        .escrever(&VERSAO_FORMATO.to_le_bytes())
        .escrever(env!("CARGO_PKG_VERSION").as_bytes())
        .escrever(b"\0")
        .escrever(clang_id.as_bytes())
        .escrever(b"\0");
    for a in args {
        h.escrever(a.as_bytes()).escrever(b"\0");
    }
    h.escrever(ir.as_bytes());
    h.fim()
}

/// O cache de objetos de um diretório.
#[derive(Debug)]
pub struct CacheObjeto {
    dir: PathBuf,
    teto_bytes: u64,
    /// Chaves sendo criadas neste processo: a segunda thread com a mesma chave
    /// espera a primeira em vez de rodar outro Clang.
    em_andamento: Mutex<HashSet<u128>>,
    terminou: Condvar,
    insercoes: AtomicUsize,
    contador_tmp: AtomicUsize,
}

impl CacheObjeto {
    /// Um cache em `dir` (o diretório `obj/` fica dentro dele).
    pub fn novo(dir: PathBuf, teto_bytes: u64) -> CacheObjeto {
        let c = CacheObjeto {
            dir: dir.join("obj"),
            teto_bytes,
            em_andamento: Mutex::new(HashSet::new()),
            terminou: Condvar::new(),
            insercoes: AtomicUsize::new(0),
            contador_tmp: AtomicUsize::new(0),
        };
        c.podar();
        c
    }

    /// O cache do processo em [`dir_cache_nativo`], ou `None` com
    /// `DARTFORGE_CACHE_OBJ=0`. O teto vem de `DARTFORGE_CACHE_OBJ_MB`.
    pub fn do_ambiente() -> Option<&'static CacheObjeto> {
        static CACHE: OnceLock<Option<CacheObjeto>> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                if std::env::var("DARTFORGE_CACHE_OBJ").is_ok_and(|v| v.trim() == "0") {
                    return None;
                }
                let mb = std::env::var("DARTFORGE_CACHE_OBJ_MB")
                    .ok()
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .unwrap_or(TETO_PADRAO_MB);
                Some(CacheObjeto::novo(dir_cache_nativo(), mb * 1024 * 1024))
            })
            .as_ref()
    }

    pub fn caminho(&self, chave: u128) -> PathBuf {
        let hex = format!("{chave:032x}");
        self.dir.join(&hex[..2]).join(format!("{hex}.obj"))
    }

    /// Devolve o objeto da `chave`, criando-o com `criar` se ainda não existe.
    ///
    /// `criar` recebe o caminho temporário onde deve escrever o objeto, dentro
    /// de um diretório só dele (onde pode deixar o que quiser, como o `.ll`:
    /// o diretório inteiro é apagado depois). O objeto só é publicado se
    /// `criar` teve sucesso e escreveu algo. O `bool` diz se foi acerto.
    pub fn obter_ou_criar(
        &self,
        chave: u128,
        criar: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(PathBuf, bool), String> {
        let final_ = self.caminho(chave);
        {
            let mut andamento = self.em_andamento.lock().unwrap();
            loop {
                if existe_inteiro(&final_) {
                    drop(andamento);
                    tocar(&final_);
                    return Ok((final_, true));
                }
                if andamento.insert(chave) {
                    break;
                }
                andamento = self.terminou.wait(andamento).unwrap();
            }
        }
        // Daqui até o fim a chave é nossa; o guarda a devolve mesmo em pânico.
        let _guarda = Andamento { cache: self, chave };

        let hex = format!("{chave:032x}");
        let n = self.contador_tmp.fetch_add(1, Ordering::Relaxed);
        let dir_tmp = self.dir.join("tmp").join(format!("{hex}.{}.{n}", std::process::id()));
        std::fs::create_dir_all(&dir_tmp)
            .map_err(|e| format!("não foi possível criar {}: {e}", dir_tmp.display()))?;
        let tmp = dir_tmp.join(format!("{hex}.obj"));
        let r = criar(&tmp).and_then(|()| self.publicar(&tmp, &final_));
        let _ = std::fs::remove_dir_all(&dir_tmp);
        let acerto = r?;
        if !acerto && self.insercoes.fetch_add(1, Ordering::Relaxed) % PODAR_A_CADA == PODAR_A_CADA - 1 {
            self.podar();
        }
        Ok((final_, acerto))
    }

    /// Move o temporário para o nome final. `Ok(true)` quando outro processo
    /// publicou a mesma chave primeiro (vale o dele, que tem o mesmo conteúdo).
    fn publicar(&self, tmp: &Path, final_: &Path) -> Result<bool, String> {
        if !std::fs::metadata(tmp).is_ok_and(|m| m.len() > 0) {
            return Err(format!("o objeto {} não foi escrito", tmp.display()));
        }
        if let Some(pai) = final_.parent() {
            std::fs::create_dir_all(pai).map_err(|e| format!("não foi possível criar {}: {e}", pai.display()))?;
        }
        if existe_inteiro(final_) {
            return Ok(true);
        }
        match std::fs::rename(tmp, final_) {
            Ok(()) => Ok(false),
            Err(_) if existe_inteiro(final_) => Ok(true),
            Err(e) => Err(format!("não foi possível publicar {}: {e}", final_.display())),
        }
    }

    /// Tira uma entrada (objeto que o ligador recusou).
    pub fn remover(&self, chave: u128) {
        let _ = std::fs::remove_file(self.caminho(chave));
    }

    /// Apaga os objetos menos usados até o total caber no teto, e temporários
    /// órfãos de processos que morreram.
    pub fn podar(&self) {
        let agora = SystemTime::now();
        if let Ok(tmps) = std::fs::read_dir(self.dir.join("tmp")) {
            for t in tmps.flatten() {
                let velho = t
                    .metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| agora.duration_since(m).is_ok_and(|d| d > IDADE_TEMPORARIO_ORFAO));
                if velho {
                    let _ = std::fs::remove_dir_all(t.path());
                }
            }
        }
        let mut objetos: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let Ok(subdirs) = std::fs::read_dir(&self.dir) else { return };
        for sub in subdirs.flatten() {
            if sub.file_name() == "tmp" {
                continue;
            }
            let Ok(arquivos) = std::fs::read_dir(sub.path()) else { continue };
            for a in arquivos.flatten() {
                let p = a.path();
                if p.extension().is_some_and(|e| e == "obj")
                    && let Ok(m) = a.metadata()
                {
                    objetos.push((m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len(), p));
                }
            }
        }
        let mut total: u64 = objetos.iter().map(|o| o.1).sum();
        if total <= self.teto_bytes {
            return;
        }
        objetos.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.cmp(&b.2)));
        for (_, tamanho, p) in objetos {
            if total <= self.teto_bytes {
                break;
            }
            if std::fs::remove_file(&p).is_ok() {
                total -= tamanho;
            }
        }
    }
}

/// Devolve a chave em andamento e acorda quem espera por ela.
struct Andamento<'a> {
    cache: &'a CacheObjeto,
    chave: u128,
}

impl Drop for Andamento<'_> {
    fn drop(&mut self) {
        self.cache.em_andamento.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.chave);
        self.cache.terminou.notify_all();
    }
}

fn existe_inteiro(p: &Path) -> bool {
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// Marca o uso de um objeto (a poda apaga os menos usados primeiro). Falha em
/// silêncio: o objeto pode estar aberto pelo ligador de outro processo.
fn tocar(p: &Path) {
    if let Ok(f) = std::fs::File::options().write(true).open(p) {
        let _ = f.set_modified(SystemTime::now());
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn escrever(conteudo: &'static [u8]) -> impl FnOnce(&Path) -> Result<(), String> {
        move |p: &Path| std::fs::write(p, conteudo).map_err(|e| e.to_string())
    }

    fn arquivos_em(dir: &Path) -> Vec<PathBuf> {
        let mut v = Vec::new();
        if let Ok(es) = std::fs::read_dir(dir) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    v.extend(arquivos_em(&p));
                } else {
                    v.push(p);
                }
            }
        }
        v
    }

    #[test]
    fn chave_muda_com_cada_componente_e_e_estavel() {
        let base = chave("ir", "clang 22", &["-c", "-O0"]);
        assert_eq!(base, chave("ir", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, chave("ir2", "clang 22", &["-c", "-O0"]));
        assert_ne!(base, chave("ir", "clang 23", &["-c", "-O0"]));
        assert_ne!(base, chave("ir", "clang 22", &["-c", "-O2"]));
        assert_ne!(base, chave("ir", "clang 22", &["-c"]));
        // A separação entre componentes conta: mover um byte de um para outro
        // não pode dar a mesma chave.
        assert_ne!(chave("ir", "clang 22", &["-c-O0"]), chave("ir", "clang 22", &["-c", "-O0"]));
    }

    #[test]
    fn acerto_nao_chama_o_fechamento() {
        let dir = tempfile::tempdir().unwrap();
        let c = CacheObjeto::novo(dir.path().to_path_buf(), u64::MAX);
        let (p, acerto) = c.obter_ou_criar(7, escrever(b"obj")).unwrap();
        assert!(!acerto);
        assert_eq!(std::fs::read(&p).unwrap(), b"obj");
        let (p2, acerto) = c.obter_ou_criar(7, |_| panic!("não devia criar de novo")).unwrap();
        assert!(acerto);
        assert_eq!(p, p2);
    }

    #[test]
    fn oito_threads_mesma_chave_um_clang() {
        let dir = tempfile::tempdir().unwrap();
        let c = CacheObjeto::novo(dir.path().to_path_buf(), u64::MAX);
        let chamadas = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    c.obter_ou_criar(42, |p| {
                        chamadas.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(20));
                        std::fs::write(p, b"objeto").map_err(|e| e.to_string())
                    })
                    .unwrap()
                });
            }
        });
        assert_eq!(chamadas.load(Ordering::SeqCst), 1);
        let arquivos = arquivos_em(&dir.path().join("obj"));
        assert_eq!(arquivos, vec![c.caminho(42)], "sobrou temporário: {arquivos:?}");
    }

    #[test]
    fn falha_nao_publica() {
        let dir = tempfile::tempdir().unwrap();
        let c = CacheObjeto::novo(dir.path().to_path_buf(), u64::MAX);
        assert!(c.obter_ou_criar(1, |_| Err("clang falhou".into())).is_err());
        // Sucesso sem escrever nada também não publica.
        assert!(c.obter_ou_criar(1, |_| Ok(())).is_err());
        assert!(c.obter_ou_criar(1, escrever(b"")).is_err());
        assert!(arquivos_em(&dir.path().join("obj")).is_empty());
        // E a chave não fica presa: a próxima tentativa cria.
        assert!(!c.obter_ou_criar(1, escrever(b"x")).unwrap().1);
    }

    #[test]
    fn podar_respeita_o_teto() {
        let dir = tempfile::tempdir().unwrap();
        let c = CacheObjeto::novo(dir.path().to_path_buf(), 25);
        for k in 0..5u128 {
            c.obter_ou_criar(k, escrever(b"0123456789")).unwrap();
            // Datas distintas, para a ordem de uso ser a de criação.
            let f = std::fs::File::options().write(true).open(c.caminho(k)).unwrap();
            f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1000 + k as u64)).unwrap();
        }
        c.podar();
        let restantes = arquivos_em(&dir.path().join("obj"));
        let total: u64 = restantes.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
        assert!(total <= 25, "{total} bytes");
        // Ficam os usados por último.
        assert_eq!(restantes.len(), 2);
        assert!(restantes.contains(&c.caminho(3)) && restantes.contains(&c.caminho(4)), "{restantes:?}");
    }
}
