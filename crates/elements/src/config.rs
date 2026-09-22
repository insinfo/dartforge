//! Leitura de `.dart_tool/package_config.json` v2 e resolução de URIs `package:`.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use url::Url;

/// Informações sobre um pacote declarado no `package_config.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: String,
    pub root_uri: Url,
    pub package_uri: Url,
    pub language_version: Option<(u8, u8)>,
}

/// Mapeamento de pacotes para seus diretórios e versões de linguagem.
#[derive(Debug, Clone, Default)]
pub struct PackageConfig {
    /// Caminho do arquivo lido, se houver.
    pub origin: Option<PathBuf>,
    pub packages: HashMap<String, PackageInfo>,
}

impl PackageConfig {
    /// Localiza o arquivo `package_config.json` procurando a partir do diretório
    /// de `entry` e subindo a árvore de diretórios.
    pub fn discover(entry: &Path) -> Option<PathBuf> {
        let parent = if entry.is_dir() {
            entry
        } else {
            entry.parent()?
        };
        for dir in parent.ancestors() {
            let candidate = dir.join(".dart_tool").join("package_config.json");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Carrega e valida o arquivo `.dart_tool/package_config.json` v2.
    ///
    /// # Erros
    /// Retorna mensagem em caso de falha de I/O, JSON inválido ou `configVersion` diferente de 2.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| format!("JSON inválido em {}: {e}", path.display()))?;

        if json.get("configVersion").and_then(|v| v.as_u64()) != Some(2) {
            return Err(format!("configVersion de {} deve ser 2", path.display()));
        }

        let entries = json
            .get("packages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("campo 'packages' ausente em {}", path.display()))?;

        let canonical_dir = path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).to_path_buf());
        let base_url = Url::from_directory_path(&canonical_dir)
            .map_err(|_| format!("caminho inválido para URL: {}", canonical_dir.display()))?;

        let mut packages = HashMap::new();
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("pacote sem nome em {}", path.display()))?;

            let root_str = entry
                .get("rootUri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("pacote '{name}' sem rootUri em {}", path.display()))?;
            // `rootUri` é um diretório mas o pub o escreve sem `/` final
            // (`.../collection-1.19.1`); sem a barra, `join("lib/")` trocaria o
            // último segmento em vez de descer nele.
            let root_dir = if root_str.ends_with('/') {
                root_str.to_string()
            } else {
                format!("{root_str}/")
            };
            let root_uri = base_url
                .join(&root_dir)
                .map_err(|e| format!("rootUri inválida em '{name}': {e}"))?;

            let package_str = entry
                .get("packageUri")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Garante que o package_uri termine com '/' para join de arquivos filhos
            let normalized_pkg = if package_str.is_empty() || package_str.ends_with('/') {
                package_str.to_string()
            } else {
                format!("{package_str}/")
            };
            let package_uri = root_uri
                .join(&normalized_pkg)
                .map_err(|e| format!("packageUri inválida em '{name}': {e}"))?;

            let language_version = entry
                .get("languageVersion")
                .and_then(|v| v.as_str())
                .and_then(|ver| {
                    let mut parts = ver.split('.');
                    let major = parts.next()?.parse::<u8>().ok()?;
                    let minor = parts.next()?.parse::<u8>().ok()?;
                    Some((major, minor))
                });

            packages.insert(
                name.to_string(),
                PackageInfo {
                    name: name.to_string(),
                    root_uri,
                    package_uri,
                    language_version,
                },
            );
        }

        Ok(Self {
            origin: Some(path.to_path_buf()),
            packages,
        })
    }

    /// Resolve uma URI `package:nome/caminho.dart` para um caminho de arquivo local.
    pub fn resolve_package_uri(&self, uri: &str) -> Result<PathBuf, String> {
        let remainder = uri
            .strip_prefix("package:")
            .ok_or_else(|| format!("esperava prefixo 'package:', veio '{uri}'"))?;
        let slash = remainder
            .find('/')
            .ok_or_else(|| format!("URI de pacote inválida (sem '/'): '{uri}'"))?;
        let pkg_name = &remainder[..slash];
        let rel_path = &remainder[slash + 1..];

        if let Some(pkg) = self.packages.get(pkg_name) {
            let file_url = pkg
                .package_uri
                .join(rel_path)
                .map_err(|e| format!("não foi possível compor URI de arquivo para '{uri}': {e}"))?;

            file_url
                .to_file_path()
                .map_err(|_| format!("URL não representa um arquivo local: {file_url}"))
        } else {
            // Fallback: procura em references/pub/<pkg_name>-<versão>/lib/<rel_path>
            let pub_dir = Path::new("references/pub");
            if pub_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(pub_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            if let Some(folder_name) = path.file_name().and_then(|n| n.to_str()) {
                                if folder_name == pkg_name
                                    || folder_name.starts_with(&format!("{pkg_name}-"))
                                {
                                    let candidate = path.join("lib").join(rel_path);
                                    if candidate.exists() {
                                        return candidate.canonicalize().or(Ok(candidate));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let origem = self
                .origin
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "configuração não encontrada".to_string());
            Err(format!("pacote '{pkg_name}' não mapeado em {origem}"))
        }
    }
}
