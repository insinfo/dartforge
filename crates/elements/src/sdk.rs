//! Layout do SDK: `libraries.json` diz onde cada `dart:x` vive e quais patches
//! a completam.
//!
//! O arquivo é o mesmo que o CFE lê. A seção escolhida é `dartdevc`, porque a
//! emissão segue o modelo do DDC (docs/FRONTEND-ARQUITETURA.md §5); a seção
//! `dart2js` tem as mesmas bibliotecas de origem com patches diferentes, e
//! trocar de modelo é trocar a seção.
use dartforge_frontend::{Feature, LanguageVersion};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Uma biblioteca `dart:` do SDK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkLibrary {
    /// Nome após `dart:` (`core`, `_js_helper`).
    pub name: String,
    /// Arquivo da biblioteca de origem.
    pub path: PathBuf,
    /// Patch files, na ordem em que o SDK os aplica.
    pub patches: Vec<PathBuf>,
    /// `supported: false` em `libraries.json` (`dart:io` no DDC): importável,
    /// mas todo uso lança `UnsupportedError`.
    pub supported: bool,
}

/// Mapa `dart:x` → arquivos, para um alvo de `libraries.json`.
#[derive(Debug, Clone, Default)]
pub struct SdkLayout {
    /// Diretório `lib/` do SDK.
    pub root: PathBuf,
    pub libraries: HashMap<String, SdkLibrary>,
    /// A versão de linguagem **corrente** da ferramenta: a de uma biblioteca
    /// sem marcador e fora de pacote, e o teto dos marcadores
    /// (`docs/VERSOES-LINGUAGEM.md`, D2). Padrão 3.13; a CLI a troca com
    /// `--versao-linguagem`. As bibliotecas `dart:*` ficam sempre no piso
    /// ([`LanguageVersion::PISO`]), porque são as do SDK 3.6.2 (D1).
    pub versao_corrente: LanguageVersion,
    /// `--enable-experiment=…`: valem só para bibliotecas sem marcador cuja
    /// versão padrão é a corrente.
    pub experimentos: Vec<Feature>,
}

/// Opções de linguagem da linha de comando: `--versao-linguagem x.y` (a
/// versão corrente, D2) e `--enable-experiment=a,b`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Linguagem {
    pub versao_corrente: Option<LanguageVersion>,
    pub experimentos: Vec<Feature>,
}

impl Linguagem {
    /// Consome `arg` (e o valor seguinte, em `resto`) se for uma opção de
    /// linguagem. `Ok(false)` quando `arg` não é dela.
    ///
    /// # Erros
    /// Versão fora do formato `x.y`, acima da corrente ou abaixo de 2.12, e
    /// experimento desconhecido.
    pub fn ler_opcao<'a>(&mut self, arg: &str, resto: &mut impl Iterator<Item = &'a str>) -> Result<bool, String> {
        if arg == "--versao-linguagem" || arg.starts_with("--versao-linguagem=") {
            let valor = match arg.split_once('=') {
                Some((_, v)) => v.to_string(),
                None => resto.next().ok_or("--versao-linguagem exige x.y")?.to_string(),
            };
            let v = LanguageVersion::parse(&valor).ok_or_else(|| format!("--versao-linguagem: '{valor}' não é x.y"))?;
            if v > LanguageVersion::ATUAL || v < LanguageVersion::MINIMA {
                return Err(format!(
                    "--versao-linguagem {v}: fora do intervalo suportado ({} a {})",
                    LanguageVersion::MINIMA,
                    LanguageVersion::ATUAL
                ));
            }
            self.versao_corrente = Some(v);
            return Ok(true);
        }
        if arg == "--enable-experiment" || arg.starts_with("--enable-experiment=") {
            let valor = match arg.split_once('=') {
                Some((_, v)) => v.to_string(),
                None => resto.next().ok_or("--enable-experiment exige nomes")?.to_string(),
            };
            for nome in valor.split(',').map(str::trim).filter(|n| !n.is_empty()) {
                let f = Feature::do_nome(nome).ok_or_else(|| format!("--enable-experiment: experimento desconhecido '{nome}'"))?;
                if !self.experimentos.contains(&f) {
                    self.experimentos.push(f);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Aplica ao layout (o que não foi pedido fica como está).
    pub fn aplicar(&self, sdk: &mut SdkLayout) {
        if let Some(v) = self.versao_corrente {
            sdk.versao_corrente = v;
        }
        for f in &self.experimentos {
            if !sdk.experimentos.contains(f) {
                sdk.experimentos.push(*f);
            }
        }
    }
}

impl SdkLayout {
    /// Lê `<lib>/libraries.json` e a seção `target` (`dartdevc` ou `dart2js`).
    ///
    /// # Erros
    /// Arquivo ausente, JSON inválido ou seção inexistente.
    ///
    /// ```no_run
    /// let sdk = dartforge_elements::sdk::SdkLayout::load(std::path::Path::new("C:/tools/dartsdk-3.6.2/lib"), "dartdevc").unwrap();
    /// assert!(sdk.library("core").is_some());
    /// ```
    pub fn load(lib_dir: &Path, target: &str) -> Result<Self, String> {
        let path = lib_dir.join("libraries.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{} inválido: {e}", path.display()))?;
        let mut libraries = HashMap::new();
        Self::ler_secao(&json, lib_dir, target, &path, &mut libraries, 0)?;
        Ok(SdkLayout {
            root: lib_dir.to_path_buf(),
            libraries,
            versao_corrente: LanguageVersion::ATUAL,
            experimentos: Vec::new(),
        })
    }

    /// Lê uma seção e, antes dela, as que ela inclui (`"include": [{"target":
    /// "vm_common"}]`). A seção `vm` do SDK 3.6.2 só declara `cli`: `core`,
    /// `async`, `collection`… estão em `vm_common`. Sem seguir o `include`, o
    /// backend nativo carregava o programa **sem `dart:core`** — e todo tipo
    /// estático (`int`, `String`…) virava `dynamic`. As bibliotecas da própria
    /// seção sobrescrevem as incluídas, como no `libraries.yaml` do SDK.
    fn ler_secao(
        json: &serde_json::Value,
        lib_dir: &Path,
        target: &str,
        path: &Path,
        libraries: &mut HashMap<String, SdkLibrary>,
        profundidade: usize,
    ) -> Result<(), String> {
        if profundidade > 8 {
            return Err(format!("include cíclico na seção {target} de {}", path.display()));
        }
        let secao = json
            .get(target)
            .ok_or_else(|| format!("seção {target} ausente em {}", path.display()))?;
        if let Some(includes) = secao.get("include").and_then(|i| i.as_array()) {
            for inc in includes {
                if let Some(t) = inc.get("target").and_then(|t| t.as_str()) {
                    Self::ler_secao(json, lib_dir, t, path, libraries, profundidade + 1)?;
                }
            }
        }
        let section = secao
            .get("libraries")
            .and_then(|l| l.as_object())
            .ok_or_else(|| format!("seção {target}.libraries ausente em {}", path.display()))?;
        for (name, entry) in section {
            let uri = entry
                .get("uri")
                .and_then(|u| u.as_str())
                .ok_or_else(|| format!("biblioteca {name} sem uri"))?;
            let patches = match entry.get("patches") {
                None => Vec::new(),
                Some(serde_json::Value::String(one)) => vec![lib_dir.join(one)],
                Some(serde_json::Value::Array(many)) => many
                    .iter()
                    .filter_map(|p| p.as_str())
                    .map(|p| lib_dir.join(p))
                    .collect(),
                Some(_) => return Err(format!("biblioteca {name} com patches inválidos")),
            };
            let supported = entry
                .get("supported")
                .and_then(|s| s.as_bool())
                .unwrap_or(true);
            libraries.insert(
                name.clone(),
                SdkLibrary {
                    name: name.clone(),
                    path: lib_dir.join(uri),
                    patches,
                    supported,
                },
            );
        }
        Ok(())
    }

    pub fn library(&self, name: &str) -> Option<&SdkLibrary> {
        self.libraries.get(name)
    }

    /// Localiza o `lib/` do SDK: `DARTFORGE_SDK_LIB`, `DART_SDK/lib`, ou o
    /// `dart` no `PATH` (`<bin>/../lib`).
    pub fn discover() -> Option<PathBuf> {
        if let Ok(explicit) = std::env::var("DARTFORGE_SDK_LIB") {
            let path = PathBuf::from(explicit);
            if path.join("libraries.json").exists() {
                return Some(path);
            }
        }
        if let Ok(sdk) = std::env::var("DART_SDK") {
            let path = PathBuf::from(sdk).join("lib");
            if path.join("libraries.json").exists() {
                return Some(path);
            }
        }
        let path_var = std::env::var_os("PATH")?;
        for dir in std::env::split_paths(&path_var) {
            for exe in ["dart.exe", "dart"] {
                if dir.join(exe).exists() {
                    let lib = dir.join("..").join("lib");
                    if lib.join("libraries.json").exists() {
                        return Some(lib);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_secao_dartdevc_do_sdk_local() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK não encontrado; teste pulado");
            return;
        };
        let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
        let core = sdk.library("core").unwrap();
        assert!(core.path.ends_with("core/core.dart"));
        assert_eq!(core.patches.len(), 2);
        assert!(!sdk.library("io").unwrap().supported);
        assert!(sdk.library("_runtime").is_some());
    }

    /// A seção `vm` só tem `cli`; `core` vem do `include` de `vm_common`.
    #[test]
    fn secao_vm_segue_o_include() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK não encontrado; teste pulado");
            return;
        };
        let sdk = SdkLayout::load(&lib, "vm").unwrap();
        assert!(sdk.library("cli").is_some());
        let core = sdk.library("core").expect("core de vm_common");
        assert!(core.path.ends_with("core/core.dart"));
        assert!(!core.patches.is_empty());
    }
}
