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
    /// Sobreposição (`load_com_sobreposicao`): arquivo do SDK, pelo caminho
    /// normalizado → arquivo que o substitui. Vazio sem sobreposição.
    pub substituicoes: HashMap<PathBuf, PathBuf>,
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
            substituicoes: HashMap::new(),
        })
    }

    /// A seção `base` do SDK com arquivos trocados pela sobreposição de
    /// `dir_sobreposicao` (o `sdk_nativo/` do backend nativo,
    /// docs/NATIVO-PLANO.md §7, P5a).
    ///
    /// `<dir_sobreposicao>/libraries.json` tem uma seção `alvo` com `base`
    /// (a seção do SDK de partida, `vm`) e `substitui`: caminho relativo ao
    /// `lib/` do SDK → arquivo relativo à sobreposição. A troca é por
    /// **conteúdo**: a unidade carregada continua com o caminho do SDK, então
    /// um `part` de um arquivo trocado resolve ao lado do original (e pode
    /// estar trocado também). Assim a sobreposição troca só o que depende da
    /// máquina interna da VM sem copiar o resto do patch.
    ///
    /// # Erros
    /// Os de [`SdkLayout::load`], seção ausente, e arquivo (original ou
    /// substituto) que não existe — uma troca que não acontece em silêncio é
    /// um SDK diferente do declarado.
    pub fn load_com_sobreposicao(lib_dir: &Path, dir_sobreposicao: &Path, alvo: &str) -> Result<Self, String> {
        let path = dir_sobreposicao.join("libraries.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{} inválido: {e}", path.display()))?;
        let secao = json
            .get(alvo)
            .ok_or_else(|| format!("seção {alvo} ausente em {}", path.display()))?;
        let base = secao
            .get("base")
            .and_then(|b| b.as_str())
            .ok_or_else(|| format!("{alvo}.base ausente em {}", path.display()))?;
        let mut sdk = Self::load(lib_dir, base)?;
        if let Some(trocas) = secao.get("substitui").and_then(|s| s.as_object()) {
            for (original, novo) in trocas {
                let novo = novo
                    .as_str()
                    .ok_or_else(|| format!("{alvo}.substitui[{original}] não é texto em {}", path.display()))?;
                let original = crate::load::normalizar(&lib_dir.join(original));
                let novo = crate::load::normalizar(&dir_sobreposicao.join(novo));
                if !original.is_file() {
                    return Err(format!("sobreposição troca {}, que não existe no SDK", original.display()));
                }
                if !novo.is_file() {
                    return Err(format!("sobreposição aponta {}, que não existe", novo.display()));
                }
                sdk.substituicoes.insert(original, novo);
            }
        }
        Ok(sdk)
    }

    /// O arquivo a ler no lugar de `path`, se a sobreposição o troca.
    pub fn substituto(&self, path: &Path) -> Option<&Path> {
        if self.substituicoes.is_empty() {
            return None;
        }
        self.substituicoes.get(&crate::load::normalizar(path)).map(PathBuf::as_path)
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

    /// A sobreposição do nativo (`sdk_nativo/`, P5a) troca por conteúdo os
    /// patches que dependem da máquina interna da VM, e o programa carrega
    /// sem diagnóstico: as `part` do patch trocado resolvem ao lado do
    /// original (e também são trocadas).
    #[test]
    fn sobreposicao_do_nativo_troca_os_patches_e_carrega() {
        let Some(lib) = SdkLayout::discover() else {
            eprintln!("SDK não encontrado; teste pulado");
            return;
        };
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk_nativo");
        let sdk = SdkLayout::load_com_sobreposicao(&lib, &dir, "dartforge_nativo").unwrap();
        assert_eq!(sdk.substituicoes.len(), 9);
        let async_patch = &sdk.library("async").unwrap().patches[0];
        let novo = sdk.substituto(async_patch).expect("async_patch trocado");
        assert!(novo.ends_with("async_patch.dart") && novo.starts_with(crate::load::normalizar(&dir)));
        assert!(sdk.substituto(&sdk.library("core").unwrap().path).is_none());

        let tmp = tempfile::tempdir().unwrap();
        let entrada = tmp.path().join("main.dart");
        std::fs::write(&entrada, "import 'dart:async';\nvoid main() { Timer(Duration.zero, () {}); }\n").unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let (program, diags) = crate::load::load_lenient(&entrada, &sdk, None, &mut interner);
        assert!(diags.is_empty(), "{diags:?}");
        let fontes_async: Vec<&str> = program
            .libraries
            .iter()
            .find(|l| l.uri == "dart:async")
            .unwrap()
            .units
            .iter()
            .map(|u| program.units[u.0 as usize].source.as_str())
            .collect();
        for marca in ["DartForge_scheduleImmediate", "DartForge_Timer_novo", "_envolverCorpo"] {
            assert!(fontes_async.iter().any(|s| s.contains(marca)), "{marca} ausente do dart:async carregado");
        }
        assert!(!fontes_async.iter().any(|s| s.contains("class _SuspendState")), "_SuspendState da VM ainda carregado");
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
