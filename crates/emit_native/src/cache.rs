//! Gerenciamento e cache do runtime nativo compilado.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct RuntimeCache {
    pub lib_path: PathBuf,
}

impl RuntimeCache {
    /// Localiza ou compila o runtime Rust para uma biblioteca estática (`.lib`),
    /// armazenada sob `target/native_cache/`.
    pub fn get_or_compile() -> Result<Self, String> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir.join("../../target/native_cache");
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("não foi possível criar diretório de cache {}: {e}", target_dir.display()))?;

        // Calcula hash dos fontes do runtime
        let runtime_src = dartforge_runtime::RUNTIME_MAIN;
        let mut hasher = DefaultHasher::new();
        runtime_src.hash(&mut hasher);
        let hash = hasher.finish();

        let lib_name = format!("dartforge_runtime_{hash:016x}.lib");
        let lib_path = target_dir.join(lib_name);

        if !lib_path.exists() {
            let rs_path = target_dir.join(format!("runtime_{hash:016x}.rs"));
            let full_code = format!(
                "#![allow(warnings)]\n{runtime_src}\n"
            );
            std::fs::write(&rs_path, full_code)
                .map_err(|e| format!("falha ao escrever {}: {e}", rs_path.display()))?;

            let rustc = std::env::var("DARTFORGE_RUSTC").unwrap_or_else(|_| "rustc".to_string());
            let status = Command::new(&rustc)
                .arg("--edition=2024")
                .arg("--crate-type=staticlib")
                .arg("-O")
                .arg(&rs_path)
                .arg("-o")
                .arg(&lib_path)
                .status()
                .map_err(|e| format!("falha ao executar {rustc}: {e}"))?;

            if !status.success() {
                return Err(format!("compilação do runtime nativo com {rustc} falhou (código {status:?})"));
            }
        }

        Ok(Self { lib_path })
    }
}

