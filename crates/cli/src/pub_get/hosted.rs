//! Rust HTTP, integrity-checked archives and staged hosted package caching.
use super::manifest::Manifest;
use semver::Version;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use url::Url;
static NEXT: AtomicU64 = AtomicU64::new(0);
const DOWNLOAD_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct Candidate {
    pub manifest: Manifest,
    pub manifest_error: Option<String>,
    pub directory: PathBuf,
    pub archive: Option<String>,
    pub digest: Option<String>,
}
pub(super) struct Hosted {
    pub cache: PathBuf,
    pub offline: bool,
    pub default_host: String,
    metadata: HashMap<(String, String), Vec<Candidate>>,
}
pub(super) fn normalize_host(text: &str) -> Result<String, String> {
    let mut url = Url::parse(text).map_err(|e| format!("invalid hosted URL: {e}"))?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("hosted URL must be HTTP(S), without credentials, query or fragment".into());
    }
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&path);
    let normalized = url.as_str().trim_end_matches('/');
    Ok(if normalized == "https://pub.dartlang.org" {
        "https://pub.dev"
    } else {
        normalized
    }
    .into())
}
impl Hosted {
    pub fn new(cache: PathBuf, offline: bool, default_host: String) -> Self {
        Self {
            cache,
            offline,
            default_host,
            metadata: HashMap::new(),
        }
    }
    fn host_key(host: &str) -> String {
        if host == "https://pub.dev" {
            return "pub.dev".into();
        }
        let hash = Sha256::digest(host.as_bytes());
        format!("dartforge-{:x}", hash)
    }
    pub fn candidates(&mut self, name: &str, host: &str) -> Result<Vec<Candidate>, String> {
        let key = (name.into(), host.into());
        if let Some(value) = self.metadata.get(&key) {
            return Ok(value.clone());
        }
        let directory = self.cache.join("hosted").join(Self::host_key(host));
        let metadata = self
            .cache
            .join("dartforge")
            .join(Self::host_key(host))
            .join(format!("{name}-versions.json"));
        let bytes = if self.offline {
            std::fs::read(&metadata)
                .map_err(|_| format!("offline: no cached hosted metadata for {name} at {host}"))?
        } else {
            download(&format!("{host}/api/packages/{name}"))?
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| {
            if self.offline {
                format!("invalid cached registry metadata: {e}")
            } else {
                format!("{name}: invalid registry JSON: {e}")
            }
        })?;
        if value["name"].as_str() != Some(name) {
            return Err(format!("registry returned a different package for {name}"));
        }
        let versions = value["versions"]
            .as_array()
            .ok_or("registry response missing versions")?;
        let mut result = Vec::new();
        let mut unique = HashSet::new();
        for row in versions {
            let version = Version::parse(
                row["version"]
                    .as_str()
                    .ok_or("registry version is missing")?,
            )
            .map_err(|e| e.to_string())?;
            if !unique.insert(version.clone()) {
                return Err(format!("duplicate registry version for {name}: {version}"));
            }
            if row["retracted"].as_bool() == Some(true) {
                continue;
            }
            let (manifest, manifest_error) =
                match Manifest::parse(&row["pubspec"], None, &self.default_host) {
                    Ok(manifest) => (manifest, None),
                    Err(error) => {
                        // Defer unsupported dependency features until this version is actually tried.
                        // Older published versions must not prevent selecting a supported newer one.
                        let name = row["pubspec"]["name"]
                            .as_str()
                            .ok_or("registry pubspec name missing")?
                            .to_string();
                        let version = Version::parse(
                            row["pubspec"]["version"]
                                .as_str()
                                .ok_or("registry pubspec version missing")?,
                        )
                        .map_err(|e| e.to_string())?;
                        let sdk = super::constraints::Constraint::parse(
                            row["pubspec"]["environment"]["sdk"]
                                .as_str()
                                .unwrap_or("any"),
                        )?;
                        (
                            Manifest {
                                name,
                                version,
                                sdk,
                                dependencies: std::collections::BTreeMap::new(),
                                dev: std::collections::BTreeMap::new(),
                                overrides: std::collections::BTreeMap::new(),
                                workspace: Vec::new(),
                                member: false,
                            },
                            Some(error),
                        )
                    }
                };
            if manifest.name != name || manifest.version != version {
                return Err(format!(
                    "registry pubspec identity mismatch for {name} {version}"
                ));
            }
            let digest = row["archive_sha256"]
                .as_str()
                .ok_or("registry archive SHA256 is required")?;
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("invalid archive SHA256".into());
            }
            let archive = row["archive_url"]
                .as_str()
                .ok_or("registry archive URL missing")?;
            let archive_url = Url::parse(archive).map_err(|e| e.to_string())?;
            if !matches!(archive_url.scheme(), "http" | "https")
                || !archive_url.username().is_empty()
                || archive_url.password().is_some()
                || (host.starts_with("https:") && archive_url.scheme() != "https")
            {
                return Err("invalid archive transport".into());
            }
            result.push(Candidate {
                manifest_error,
                directory: directory.join(format!("{name}-{version}")),
                manifest,
                archive: Some(archive.into()),
                digest: Some(digest.to_ascii_lowercase()),
            });
        }
        // Stable releases first, then prereleases, each in descending precedence.
        result.sort_by(|a, b| {
            a.manifest
                .version
                .pre
                .is_empty()
                .cmp(&b.manifest.version.pre.is_empty())
                .reverse()
                .then_with(|| {
                    super::constraints::compare_versions(&b.manifest.version, &a.manifest.version)
                })
        });
        // A rejected response must not replace the last usable offline snapshot.
        // Publish only after validating every candidate, including its identity
        // and archive transport; JSON syntax alone is insufficient.
        if !self.offline {
            atomic_write(&metadata, &bytes)?;
        }
        self.metadata.insert(key, result.clone());
        Ok(result)
    }
    pub fn materialize(&self, candidate: &Candidate, host: &str) -> Result<(), String> {
        let name = &candidate.manifest.name;
        let version = &candidate.manifest.version;
        let digest = candidate.digest.as_ref().ok_or("missing archive digest")?;
        let hash_file = self
            .cache
            .join("hosted-hashes")
            .join(Self::host_key(host))
            .join(format!("{name}-{version}.sha256"));
        if candidate.directory.is_dir()
            && std::fs::read_to_string(&hash_file).is_ok_and(|s| s.trim() == digest)
        {
            let manifest = Manifest::read_hosted(&candidate.directory, &self.default_host)?;
            if manifest_matches(&manifest, &candidate.manifest) {
                return Ok(());
            }
            return Err(format!(
                "cached package identity mismatch: {name} {version}"
            ));
        }
        if self.offline {
            return Err(format!(
                "offline: verified archive is not cached for {name} {version}"
            ));
        }
        let bytes = download(candidate.archive.as_ref().ok_or("missing archive URL")?)?;
        if format!("{:x}", Sha256::digest(&bytes)) != *digest {
            return Err(format!("archive SHA256 mismatch: {name} {version}"));
        }
        let parent = candidate.directory.parent().ok_or("invalid cache path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let staging = parent.join(format!(
            ".dartforge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let result = (|| {
            unpack(&bytes, &staging)?;
            let manifest = Manifest::read_hosted(&staging, &self.default_host)?;
            if manifest.name != *name || manifest.version != *version {
                return Err("downloaded archive pubspec identity mismatch".into());
            }
            if !manifest_matches(&manifest, &candidate.manifest) {
                return Err("archive/registry dependency mismatch".into());
            }
            if candidate.directory.exists() {
                return Err(format!(
                    "unverified cache directory already exists: {}",
                    candidate.directory.display()
                ));
            }
            std::fs::rename(&staging, &candidate.directory).map_err(|e| e.to_string())?;
            atomic_write(&hash_file, format!("{digest}\n").as_bytes())
        })();
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        result
    }
}
fn download(url: &str) -> Result<Vec<u8>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(90)))
        .https_only(url.starts_with("https:"))
        .build()
        .into();
    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("download {url}: {e}"))?;
    let mut reader = response.into_body().into_reader().take(DOWNLOAD_LIMIT + 1);
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| format!("download {url}: {e}"))?;
    if bytes.len() as u64 > DOWNLOAD_LIMIT {
        return Err("package download exceeds 128 MiB limit".into());
    }
    Ok(bytes)
}
pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("output has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(
        ".dartforge-write-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    let result = std::fs::rename(&temporary, path).map_err(|e| e.to_string());
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
pub(super) fn unpack(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let reader = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(reader);
    let mut total = 0u64;
    let mut names = HashSet::new();
    for (index, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        if index >= 100_000 {
            return Err("archive has too many entries".into());
        }
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if path.is_absolute()
            || path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir | std::path::Component::Prefix(_)
                )
            })
        {
            return Err("archive contains an escaping path".into());
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err("archive links and special files are not supported".into());
        }
        if !names.insert(path) {
            return Err("archive contains duplicate paths".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("archive size overflow")?;
        if total > 1024 * 1024 * 1024 {
            return Err("expanded archive exceeds 1 GiB limit".into());
        }
        if !entry.unpack_in(destination).map_err(|e| e.to_string())? {
            return Err("archive entry escaped extraction directory".into());
        }
    }
    Ok(())
}

fn manifest_matches(actual: &Manifest, expected: &Manifest) -> bool {
    actual.name == expected.name
        && actual.version == expected.version
        && actual.sdk.text == expected.sdk.text
        && actual.dependencies.len() == expected.dependencies.len()
        && actual.dependencies.iter().all(|(name, dependency)| {
            expected.dependencies.get(name).is_some_and(|other| {
                other.source == dependency.source
                    && other.constraint.text == dependency.constraint.text
            })
        })
}
