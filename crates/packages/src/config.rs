//! Leitura estrita de package_config v2 e resolução de URIs locais.
use crate::{GraphError, error_at};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use url::Url;

/// Diretórios declarados por um pacote, resolvidos como URIs.
struct Package {
    root: Url,
    package: Url,
}

/// Configuração relida em cada carregamento para detectar remapeamentos.
pub(crate) struct Config {
    packages: HashMap<String, Package>,
    /// Arquivo de configuração lido e seu conteúdo exato, quando existe.
    ///
    /// A revalidação incremental precisa provar que a resolução de `package:`
    /// não mudou; comparar o texto é a única verificação que não depende de
    /// mtime nem de tamanho.
    pub(crate) origin: Option<(std::path::PathBuf, String)>,
}
impl Config {
    /// Encontra a configuração explícita ou a mais próxima da entrada.
    pub(crate) fn load(entry: &Path, explicit: Option<&Path>) -> Result<Self, GraphError> {
        let path = if let Some(path) = explicit {
            Some(path.to_path_buf())
        } else {
            entry
                .parent()
                .into_iter()
                .flat_map(Path::ancestors)
                .map(|p| p.join(".dart_tool/package_config.json"))
                .find(|p| p.exists())
        };
        let Some(path) = path else {
            return Ok(Self {
                packages: HashMap::new(),
                origin: None,
            });
        };
        let path =
            std::fs::canonicalize(&path).map_err(|e| error_at(&path, None, e.to_string()))?;
        let fail = |message: &str| error_at(&path, None, message.into());
        let text = std::fs::read_to_string(&path).map_err(|e| fail(&e.to_string()))?;
        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| fail(&format!("package_config JSON inválido: {e}")))?;
        if json.get("configVersion").and_then(|v| v.as_u64()) != Some(2) {
            return Err(fail("package_config exige configVersion inteiro 2"));
        }
        let entries = json
            .get("packages")
            .and_then(|v| v.as_array())
            .ok_or_else(|| fail("packages deve ser uma lista"))?;
        let base = Url::from_directory_path(
            path.parent()
                .expect("configuração canônica possui diretório"),
        )
        .map_err(|_| fail("configuração não possui URI file válida"))?;
        let mut packages = HashMap::new();
        for entry in entries {
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| fail("pacote sem name string"))?;
            if !valid_name(name) {
                return Err(fail("nome de pacote inválido"));
            }
            let root_ref = entry
                .get("rootUri")
                .and_then(|v| v.as_str())
                .ok_or_else(|| fail("pacote sem rootUri string"))?;
            let root = directory(&base, root_ref).map_err(|e| fail(&e))?;
            let package_ref = match entry.get("packageUri") {
                None => "",
                Some(v) => v
                    .as_str()
                    .ok_or_else(|| fail("packageUri deve ser string"))?,
            };
            if package_ref.starts_with('/') || package_ref.contains(':') {
                return Err(fail("packageUri deve ser um caminho URI relativo"));
            }
            let package = directory(&root, package_ref).map_err(|e| fail(&e))?;
            if !package.as_str().starts_with(root.as_str()) {
                return Err(fail("packageUri está fora de rootUri"));
            }
            if let Some(version) = entry.get("languageVersion") {
                let version = version
                    .as_str()
                    .ok_or_else(|| fail("languageVersion deve ser string"))?;
                let components: Vec<_> = version.split('.').collect();
                if components.len() != 2
                    || components.iter().any(|s| {
                        s.is_empty()
                            || !s.bytes().all(|b| b.is_ascii_digit())
                            || (s.len() > 1 && s.starts_with('0'))
                    })
                {
                    return Err(fail("languageVersion inválida"));
                }
                if version != "3.6" {
                    return Err(fail(
                        "languageVersion não suportada: somente 3.6 foi verificada",
                    ));
                }
            }
            if packages
                .insert(name.to_owned(), Package { root, package })
                .is_some()
            {
                return Err(fail("nome de pacote duplicado"));
            }
        }
        let mut ordered: Vec<_> = packages.iter().collect();
        ordered.sort_unstable_by_key(|(name, _)| *name);
        let paths: Vec<(PathBuf, PathBuf)> = ordered
            .iter()
            .map(|(_, package)| {
                Ok((
                    package
                        .root
                        .to_file_path()
                        .map_err(|_| fail("rootUri file inválida"))?,
                    package
                        .package
                        .to_file_path()
                        .map_err(|_| fail("packageUri file inválida"))?,
                ))
            })
            .collect::<Result<_, GraphError>>()?;
        for (i, (left_root, left_package)) in paths.iter().enumerate() {
            for (right_root, right_package) in &paths[i + 1..] {
                if left_root == right_root
                    || (right_root.starts_with(left_root)
                        && (right_root.starts_with(left_package)
                            || left_package.starts_with(right_root)))
                    || (left_root.starts_with(right_root)
                        && (left_root.starts_with(right_package)
                            || right_package.starts_with(left_root)))
                {
                    return Err(fail(
                        "raízes ou diretórios packageUri incompatíveis entre pacotes",
                    ));
                }
            }
        }
        Ok(Self {
            packages,
            origin: Some((path, text)),
        })
    }

    /// Resolve package ou URI de arquivo, preservando codificação e componentes.
    pub(crate) fn resolve(&self, importer: &Path, uri: &str) -> Result<PathBuf, String> {
        validate_uri(uri)?;
        let url = if let Some(rest) = uri.strip_prefix("package:") {
            let (name, path) = rest
                .split_once('/')
                .ok_or("URI package exige nome e caminho")?;
            let package = self
                .packages
                .get(name)
                .ok_or_else(|| format!("pacote desconhecido: {name}"))?;
            let resolved = package.package.join(path).map_err(|e| e.to_string())?;
            if !resolved.as_str().starts_with(package.package.as_str()) {
                return Err("URI package escapa do diretório packageUri".into());
            }
            resolved
        } else {
            let base = Url::from_file_path(importer)
                .map_err(|_| "arquivo importador sem URI file válida")?;
            base.join(uri).map_err(|e| e.to_string())?
        };
        if url.scheme() != "file" {
            return Err(format!(
                "esquema {} não suportado; runtime dart: ainda indisponível",
                url.scheme()
            ));
        }
        url.to_file_path()
            .map_err(|_| "URI file não representa caminho local válido".into())
    }
}

/// Verifica escapes URI completos e rejeita componentes não implementados.
pub(crate) fn validate_uri(uri: &str) -> Result<(), String> {
    if uri.is_empty()
        || uri
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | '?' | '#'))
    {
        return Err("URI vazia ou com escapes Dart/query/fragmento não suportados".into());
    }
    let bytes = uri.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return Err("percent-encoding URI inválido".into());
            }
            let decoded = u8::from_str_radix(&uri[i + 1..i + 3], 16).unwrap();
            if matches!(decoded, 0 | b'/' | b'\\') {
                return Err("separador ou NUL codificado em URI não suportado".into());
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Resolve uma referência de diretório e acrescenta a barra final após resolução.
fn directory(base: &Url, reference: &str) -> Result<Url, String> {
    if !reference.is_empty() {
        validate_uri(reference)?;
    }
    let mut url = base.join(reference).map_err(|e| e.to_string())?;
    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
        return Err("rootUri/packageUri exige URI file sem query ou fragmento".into());
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

/// Aplica as regras de nome da especificação package_config, mais amplas que Pub.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().any(|b| b != b'.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=@".contains(&b))
}
