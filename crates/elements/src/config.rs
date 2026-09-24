//! Leitura de `.dart_tool/package_config.json` v2 e resolução de URIs `package:`.
use std::collections::HashMap;
use crate::gerado::Geracao;
use dartforge_frontend::LanguageVersion;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use url::Url;

/// Informações sobre um pacote declarado no `package_config.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageInfo {
    pub name: String,
    pub root_uri: Url,
    pub package_uri: Url,
    /// `languageVersion` do pacote: a versão padrão das bibliotecas dele
    /// (`docs/VERSOES-LINGUAGEM.md`). `None` quando a entrada não traz.
    pub language_version: Option<LanguageVersion>,
    /// O texto do `languageVersion` quando ele existe mas não é `x.y`: é
    /// erro de compilação para as bibliotecas do pacote (o CFE diz
    /// `LanguageVersionInvalidInDotPackages`).
    pub language_version_invalida: Option<String>,
}

/// Mapeamento de pacotes para seus diretórios e versões de linguagem.
#[derive(Debug, Clone, Default)]
pub struct PackageConfig {
    /// Caminho do arquivo lido, se houver.
    pub origin: Option<PathBuf>,
    pub packages: HashMap<String, PackageInfo>,
    /// `(nome, diretório do packageUri canônico, sem o prefixo `\\?\`)`,
    /// calculado uma vez na carga: mapear um caminho de arquivo para
    /// `package:x/…` acontece milhares de vezes por programa e
    /// `canonicalize` custa uma chamada ao sistema cada.
    pub package_dirs: Vec<(String, PathBuf)>,
    /// `(nome, diretório do rootUri canônico)`, do mais longo para o mais
    /// curto: diz a que pacote pertence um arquivo fora de `lib/` (`bin/`,
    /// `test/`), para a versão de linguagem padrão dele.
    pub root_dirs: Vec<(String, PathBuf)>,
    /// `<projeto>/.dart_tool/build/generated`, canônico e sem `\?\`, quando
    /// existe: raiz dos arquivos gerados pelo `build_runner`, mapeados para
    /// `package:<pacote>/<rel>` em `canonical_file_uri`.
    pub generated_root: Option<PathBuf>,
    /// Fontes geradas em memória (`.template.dart` do ngdart e afins).
    /// Quando a geração tem o caminho natural de um arquivo, ela manda:
    /// é a autoridade sobre o que ela mesma gera, e o disco nem é
    /// consultado.
    pub gerados: Option<Arc<Geracao>>,
}

/// Tira o prefixo verbatim `\\?\` que `canonicalize` devolve no Windows, para
/// que `strip_prefix` e comparações com caminhos comuns funcionem.
pub fn sem_verbatim(p: PathBuf) -> PathBuf {
    match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(r) => PathBuf::from(r),
        None => p,
    }
}

impl PackageConfig {
    /// O pacote a que pertence a biblioteca de URI `uri` (arquivo em `path`),
    /// pela regra de `accepted/2.8/language-versioning`: `package:x/…`
    /// pertence a `x`; um arquivo pertence ao pacote cuja raiz (`rootUri`) o
    /// contém, a mais interna quando há aninhamento.
    pub fn pacote_da_biblioteca(&self, uri: &str, path: Option<&Path>) -> Option<&PackageInfo> {
        if let Some(resto) = uri.strip_prefix("package:") {
            let nome = resto.split('/').next().unwrap_or("");
            return self.packages.get(nome);
        }
        let path = path?;
        self.root_dirs
            .iter()
            .find(|(_, dir)| path.starts_with(dir))
            .and_then(|(nome, _)| self.packages.get(nome))
    }

    /// Caminho de um arquivo gerado pelo build_runner, quando existe:
    /// `<projeto>/.dart_tool/build/generated/<pacote>/lib/<rel>` (o projeto é o
    /// dono do `package_config.json` lido).
    pub fn generated_path(&self, pkg_name: &str, rel_path: &str) -> Option<PathBuf> {
        let origin = self.origin.as_ref()?;
        let dart_tool = origin.parent()?;
        let candidate = dart_tool.join("build").join("generated").join(pkg_name).join("lib").join(rel_path);
        candidate.is_file().then_some(candidate)
    }

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

            let texto_versao = entry.get("languageVersion").and_then(|v| v.as_str());
            let language_version = texto_versao.and_then(LanguageVersion::parse);
            let language_version_invalida =
                texto_versao.filter(|_| language_version.is_none()).map(str::to_string);

            packages.insert(
                name.to_string(),
                PackageInfo {
                    name: name.to_string(),
                    root_uri,
                    package_uri,
                    language_version,
                    language_version_invalida,
                },
            );
        }

        let mut root_dirs: Vec<(String, PathBuf)> = packages
            .values()
            .filter_map(|pkg| {
                let dir = pkg.root_uri.to_file_path().ok()?;
                let dir = sem_verbatim(std::fs::canonicalize(&dir).unwrap_or(dir));
                Some((pkg.name.clone(), dir))
            })
            .collect();
        root_dirs.sort_by(|a, b| b.1.as_os_str().len().cmp(&a.1.as_os_str().len()).then(a.0.cmp(&b.0)));

        let mut package_dirs: Vec<(String, PathBuf)> = packages
            .values()
            .filter_map(|pkg| {
                let dir = pkg.package_uri.to_file_path().ok()?;
                let dir = sem_verbatim(std::fs::canonicalize(&dir).unwrap_or(dir));
                Some((pkg.name.clone(), dir))
            })
            .collect();
        // Diretório mais longo primeiro: um pacote dentro de outro casa o mais específico.
        package_dirs.sort_by(|a, b| b.1.as_os_str().len().cmp(&a.1.as_os_str().len()).then(a.0.cmp(&b.0)));

        let generated_root = path.parent().map(|dart_tool| dart_tool.join("build").join("generated")).filter(|g| g.is_dir()).map(|g| sem_verbatim(std::fs::canonicalize(&g).unwrap_or(g)));

        Ok(Self {
            origin: Some(path.to_path_buf()),
            packages,
            package_dirs,
            root_dirs,
            generated_root,
            gerados: None,
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

            let path = file_url
                .to_file_path()
                .map_err(|_| format!("URL não representa um arquivo local: {file_url}"))?;
            if self.gerados.as_ref().is_some_and(|g| g.contem(&path)) {
                return Ok(path);
            }
            if path.is_file() {
                return Ok(path);
            }
            // Sobreposição de gerados do build_runner (`.template.dart` do ngdart…):
            // `<raiz do projeto>/.dart_tool/build/generated/<pacote>/lib/<x>`.
            if let Some(gerado) = self.generated_path(pkg_name, rel_path) {
                return Ok(gerado);
            }
            Ok(path)
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
