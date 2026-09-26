//! Gerenciamento e cache do runtime nativo compilado.
//!
//! A `.lib` do runtime é compilada uma vez por conteúdo e reaproveitada por
//! todo programa ligado depois. Três propriedades, porque o harness liga
//! vários programas ao mesmo tempo, em threads e em processos:
//!
//! * **chave estável**: FNV-1a de 128 bits (`resumo`) sobre a versão do
//!   `rustc`, as bandeiras e o fonte — não o `DefaultHasher`, que muda entre
//!   versões do Rust, nem só o fonte, que não muda quando o compilador muda;
//! * **publicação atômica**: o `rustc` escreve num nome temporário e a `.lib`
//!   só aparece com o nome final, inteira, por `rename`. Antes, um trabalhador
//!   via o arquivo existir e ligava contra uma `.lib` ainda pela metade;
//! * **uma compilação por processo** (`OnceLock`), e só as duas `.lib` mais
//!   recentes ficam no disco.
//!
//! Normalmente nada disso roda: o runtime vem **pré-compilado** com o
//! dartforge (`build.rs`), em `lib/` da distribuição ou no diretório do build
//! (`runtime_precompilado`), e quem usa o dartforge não precisa de Rust. A
//! compilação com o `rustc` da máquina fica para uma árvore de
//! desenvolvimento compilada com `DARTFORGE_RUNTIME_SEM_PRECOMPILAR=1`.

use crate::resumo::Fnv128;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Bandeiras do `rustc` para o runtime; entram na chave.
const BANDEIRAS_RUSTC: [&str; 3] = ["--edition=2024", "--crate-type=staticlib", "-O"];

/// Quantas `.lib` do runtime ficam no cache (a atual e a anterior, para um
/// processo mais velho ainda rodando não perder a dele no meio da ligação).
const LIBS_MANTIDAS: usize = 2;

/// Diretório dos caches do backend nativo (runtime e objetos):
/// `DARTFORGE_CACHE_NATIVO`, senão `$CARGO_TARGET_DIR/native_cache`, senão
/// `target/native_cache` do repositório que compilou este binário.
pub fn dir_cache_nativo() -> PathBuf {
    if let Some(d) = std::env::var_os("DARTFORGE_CACHE_NATIVO") {
        return PathBuf::from(d);
    }
    if let Some(t) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(t).join("native_cache");
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/native_cache")
}

pub struct RuntimeCache {
    pub lib_path: PathBuf,
}

impl RuntimeCache {
    /// Localiza ou compila o runtime Rust para uma biblioteca estática (`.lib`),
    /// armazenada em [`dir_cache_nativo`]. Compila no máximo uma vez por
    /// processo; as outras threads esperam o resultado da primeira.
    pub fn get_or_compile() -> Result<Self, String> {
        static RUNTIME: OnceLock<Result<PathBuf, String>> = OnceLock::new();
        RUNTIME
            .get_or_init(|| match runtime_precompilado(env!("DARTFORGE_RUNTIME_PRINCIPAL")) {
                Some(p) => Ok(p),
                None => compilar_runtime(&[], "dartforge_runtime_"),
            })
            .clone()
            .map(|lib_path| Self { lib_path })
    }

    /// A variante do runtime que vai para a DLL do SDK da fonte (P5c): sem o
    /// `main` C (cfg `dartforge_runtime_dll`); o `main` é o do executável.
    pub fn para_dll() -> Result<Self, String> {
        static RUNTIME: OnceLock<Result<PathBuf, String>> = OnceLock::new();
        RUNTIME
            .get_or_init(|| match runtime_precompilado(env!("DARTFORGE_RUNTIME_DLL")) {
                Some(p) => Ok(p),
                None => compilar_runtime(&["--cfg", "dartforge_runtime_dll"], "dartforge_rtdll_"),
            })
            .clone()
            .map(|lib_path| Self { lib_path })
    }
}

/// Onde a distribuição guarda as bibliotecas do dartforge: `lib/` ao lado do
/// `bin/` do executável (`<raiz>/bin/dartforge`, `<raiz>/lib/…`), ou
/// `DARTFORGE_LIB`.
pub fn dir_lib_da_distribuicao() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("DARTFORGE_LIB") {
        return Some(PathBuf::from(d));
    }
    let exe = std::env::current_exe().ok()?;
    let raiz = exe.parent()?.parent()?;
    let lib = raiz.join("lib");
    lib.is_dir().then_some(lib)
}

/// O runtime pré-compilado `nome` (o nome leva o hash do fonte: só o deste
/// compilador casa): na distribuição, senão no diretório do build.
fn runtime_precompilado(nome: &str) -> Option<PathBuf> {
    let candidatos = [dir_lib_da_distribuicao(), Some(PathBuf::from(env!("DARTFORGE_RUNTIME_DIR_DO_BUILD")))];
    candidatos.into_iter().flatten().map(|d| d.join(nome)).find(|p| p.is_file())
}

fn compilar_runtime(extras: &[&str], prefixo: &str) -> Result<PathBuf, String> {
    let dir = dir_cache_nativo();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("não foi possível criar diretório de cache {}: {e}", dir.display()))?;

    let rustc = std::env::var("DARTFORGE_RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let versao = Command::new(&rustc)
        .arg("-vV")
        .output()
        .map_err(|e| format!("falha ao executar {rustc} -vV: {e}"))?;
    if !versao.status.success() {
        return Err(format!("{rustc} -vV falhou (código {:?})", versao.status));
    }

    let runtime_src = dartforge_runtime::RUNTIME_MAIN;
    let mut h = Fnv128::default();
    h.escrever(b"dartforge-runtime\0").escrever(&versao.stdout);
    for b in BANDEIRAS_RUSTC.iter().chain(extras) {
        h.escrever(b.as_bytes()).escrever(b"\0");
    }
    h.escrever(runtime_src.as_bytes());
    let hash = format!("{:032x}", h.fim());

    let lib_path = dir.join(format!("{prefixo}{hash}.{}", crate::alvo::ext_estatica()));
    if lib_path.is_file() {
        return Ok(lib_path);
    }

    // O fonte vai para o nome final antes de compilar: o caminho dele aparece
    // nas mensagens de pânico do runtime, e o relatório do harness normaliza
    // `runtime_<hash>`, não um sufixo temporário. O conteúdo é função do
    // hash, então quem sobrescrever escreve os mesmos bytes; o `rename` só
    // evita que um `rustc` leia o arquivo pela metade.
    let pid = std::process::id();
    let rs_path = dir.join(format!("runtime_{hash}.rs"));
    let rs_tmp = dir.join(format!("runtime_{hash}.{pid}.rs.tmp"));
    std::fs::write(&rs_tmp, format!("#![allow(warnings)]\n{runtime_src}\n"))
        .map_err(|e| format!("falha ao escrever {}: {e}", rs_tmp.display()))?;
    if std::fs::rename(&rs_tmp, &rs_path).is_err() {
        let _ = std::fs::remove_file(&rs_tmp);
        if !rs_path.is_file() {
            return Err(format!("não foi possível publicar {}", rs_path.display()));
        }
    }

    let lib_tmp = dir.join(format!("{prefixo}{hash}.{pid}.tmp.{}", crate::alvo::ext_estatica()));
    let status = Command::new(&rustc)
        .args(BANDEIRAS_RUSTC)
        .args(extras)
        .arg(&rs_path)
        .arg("-o")
        .arg(&lib_tmp)
        .status()
        .map_err(|e| format!("falha ao executar {rustc}: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&lib_tmp);
        return Err(format!("compilação do runtime nativo com {rustc} falhou (código {status:?})"));
    }
    if std::fs::rename(&lib_tmp, &lib_path).is_err() {
        // Outro processo publicou a mesma chave primeiro: vale a dele.
        let _ = std::fs::remove_file(&lib_tmp);
        if !lib_path.is_file() {
            return Err(format!("não foi possível publicar {}", lib_path.display()));
        }
    }

    if prefixo == "dartforge_runtime_" {
        podar_runtimes(&dir, &lib_path);
    }
    Ok(lib_path)
}

/// Apaga as `.lib` do runtime (e os `.rs` correspondentes) além das
/// [`LIBS_MANTIDAS`] mais recentes; a atual nunca.
fn podar_runtimes(dir: &Path, atual: &Path) {
    let Ok(entradas) = std::fs::read_dir(dir) else { return };
    let mut libs: Vec<(std::time::SystemTime, PathBuf)> = entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let nome = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            nome.starts_with("dartforge_runtime_") && nome.ends_with(&format!(".{}", crate::alvo::ext_estatica())) && !nome.contains(".tmp")
        })
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    libs.sort_by_key(|l| std::cmp::Reverse(l.0));
    for (_, lib) in libs.into_iter().skip(LIBS_MANTIDAS) {
        if lib == atual {
            continue;
        }
        let _ = std::fs::remove_file(&lib);
        if let Some(hash) = lib
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("dartforge_runtime_"))
            .and_then(|n| n.strip_suffix(crate::alvo::ext_estatica()))
            .and_then(|n| n.strip_suffix('.'))
        {
            let _ = std::fs::remove_file(dir.join(format!("runtime_{hash}.rs")));
        }
    }
}
