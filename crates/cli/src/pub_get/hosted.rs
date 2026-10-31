//! Hosted packages: version listings over HTTP, integrity-checked archives
//! and Pub 3.6.2's cache layout (`hosted/<server>/<name>-<version>` and
//! `hosted-hashes/<server>/<name>-<version>.sha256`).
use super::{
    manifest::{Context, Description, Manifest, Role},
    source::SourceError,
    tree::{Node, parse_json},
    version::Version,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Component, Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};
use url::Url;
static NEXT: AtomicU64 = AtomicU64::new(0);
const DOWNLOAD_LIMIT: u64 = 256 * 1024 * 1024;

/// One version of a registry listing.
#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub version: Version,
    pub manifest: Manifest,
    pub sha256: Option<String>,
    pub archive_url: String,
    pub retracted: bool,
}

/// A registry's answer for one package.
#[derive(Clone, Debug, Default)]
pub(crate) struct Listing {
    pub versions: Vec<Listed>,
    pub discontinued: bool,
    pub replaced_by: Option<String>,
}

pub(crate) struct Hosted {
    pub cache: PathBuf,
    pub offline: bool,
    listings: HashMap<(String, String), Rc<Listing>>,
    /// Packages downloaded by this run (Pub's `didUpdate`).
    pub downloaded: HashSet<(String, String)>,
}

/// `validateAndNormalizeHostedUrl`, written as Pub writes the URL in
/// lockfiles: without a final slash, and `pub.dartlang.org` as `pub.dev`.
pub(crate) fn normalize_host(text: &str) -> Result<String, String> {
    let url = Url::parse(text).map_err(|e| format!("invalid url: {e}"))?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(format!("url scheme must be https:// or http:// ({text})"));
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(format!("url must have a hostname ({text})"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!("user-info is not supported in url ({text})"));
    }
    if url.query().is_some() {
        return Err(format!("querystring is not supported in url ({text})"));
    }
    if url.fragment().is_some() {
        return Err(format!("fragment is not supported in url ({text})"));
    }
    // A server below the root keeps a final slash; the root has no path.
    let mut text = url.to_string();
    if url.path() == "/" {
        text.truncate(text.trim_end_matches('/').len());
    } else if !text.ends_with('/') {
        text.push('/');
    }
    Ok(if text == "https://pub.dartlang.org" {
        "https://pub.dev".into()
    } else {
        text
    })
}

/// `Uri.parse(url).resolve(path)` for a normalized server URL.
pub(crate) fn api_url(url: &str, path: &str) -> String {
    if url.ends_with('/') {
        format!("{url}{path}")
    } else {
        format!("{url}/{path}")
    }
}

/// Pub's `_urlToDirectory`: the server's directory name in the cache.
pub(crate) fn url_to_directory(url: &str) -> String {
    let mut rest = url;
    let mut prefix = String::new();
    for scheme in ["https://", "http://"] {
        if let Some(tail) = url.strip_prefix(scheme) {
            let mut localhost = false;
            for host in ["127.0.0.1", "[::1]", "localhost"] {
                if let Some(after) = tail.strip_prefix(host) {
                    rest = after;
                    localhost = true;
                    break;
                }
            }
            if !localhost {
                rest = tail;
            }
            if localhost {
                prefix.push_str("localhost");
            } else if scheme == "http://" {
                prefix.push_str(scheme);
            }
            break;
        }
    }
    let mut result = String::new();
    for c in prefix.chars().chain(rest.chars()) {
        if "<>:\"\\/|?*%".contains(c) {
            result.push_str(&format!("%{}", c as u32));
        } else {
            result.push(c);
        }
    }
    result
}

fn not_found_or_fatal(url: &str, error: ureq::Error, name: &str, host: &str) -> SourceError {
    match error {
        ureq::Error::StatusCode(404) => {
            SourceError::NotFound(format!("could not find package {name} at {host}"))
        }
        error => SourceError::Fatal(format!(
            "Got socket error trying to find package {name} at {host}.\n({url}: {error})"
        )),
    }
}

impl Hosted {
    pub fn new(cache: PathBuf, offline: bool) -> Self {
        Self {
            cache,
            offline,
            listings: HashMap::new(),
            downloaded: HashSet::new(),
        }
    }

    pub fn directory(&self, name: &str, version: &Version, url: &str) -> PathBuf {
        self.cache
            .join("hosted")
            .join(url_to_directory(url))
            .join(format!("{name}-{version}"))
    }

    fn hash_path(&self, name: &str, version: &Version, url: &str) -> PathBuf {
        self.cache
            .join("hosted-hashes")
            .join(url_to_directory(url))
            .join(format!("{name}-{version}.sha256"))
    }

    /// The content hash recorded for a downloaded package.
    /// The listing fetched by this run, if any.
    pub fn fetched(&self, name: &str, url: &str) -> Option<Rc<Listing>> {
        self.listings
            .get(&(name.to_owned(), url.to_owned()))
            .cloned()
    }

    pub fn cached_hash(&self, name: &str, version: &Version, url: &str) -> Option<String> {
        std::fs::read_to_string(self.hash_path(name, version, url))
            .ok()
            .map(|text| text.trim().to_ascii_lowercase())
            .filter(|text| text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit()))
    }

    /// The registry's version listing of `name` on `url`, fetched once per
    /// run.
    pub fn listing(
        &mut self,
        name: &str,
        url: &str,
        default_host: &str,
        sdk: &Version,
    ) -> Result<Rc<Listing>, SourceError> {
        let key = (name.to_owned(), url.to_owned());
        if let Some(listing) = self.listings.get(&key) {
            return Ok(listing.clone());
        }
        if self.offline {
            return Err(SourceError::NotFound(format!(
                "could not find package {name} in cache"
            )));
        }
        let api = api_url(url, &format!("api/packages/{name}"));
        let bytes = download(&api, Some("application/vnd.pub.v2+json"))
            .map_err(|e| not_found_or_fatal(&api, e, name, url))?;
        let body = parse_json(&bytes).map_err(|e| {
            SourceError::Fatal(format!(
                "Got badly formatted response trying to find package {name} at {url}: {e}"
            ))
        })?;
        let bad = |what: &str| {
            SourceError::Fatal(format!(
                "Got badly formatted response trying to find package {name} at {url}: {what}"
            ))
        };
        let versions = body
            .get("versions")
            .as_list()
            .ok_or_else(|| bad("versions must be a list"))?;
        let containing = Description::Hosted(url.to_owned());
        let context = Context {
            containing: &containing,
            host: default_host,
            sdk,
        };
        let mut result = Vec::with_capacity(versions.len());
        for row in versions {
            let pubspec = row.get("pubspec");
            if !matches!(pubspec, Node::Map(_)) {
                return Err(bad("pubspec must be a map"));
            }
            let manifest = Manifest::parse(pubspec, None, &context, Role::Dependency)
                .map_err(SourceError::Fatal)?;
            if manifest.name != name {
                return Err(SourceError::Fatal(format!(
                    "\"name\" field doesn't match expected name \"{name}\" in the listing of {url}."
                )));
            }
            let sha256 = match row.get("archive_sha256") {
                Node::Null => None,
                Node::String(hash) => Some(hash.to_ascii_lowercase()),
                _ => return Err(bad("archive_sha256 must be a String")),
            };
            let archive_url = row
                .get("archive_url")
                .as_str()
                .ok_or_else(|| bad("archive_url must be a String"))?
                .to_owned();
            let retracted = match row.get("retracted") {
                Node::Null => false,
                Node::Bool(b) => *b,
                _ => return Err(bad("retracted must be a bool")),
            };
            result.push(Listed {
                version: manifest.version.clone(),
                manifest,
                sha256,
                archive_url,
                retracted,
            });
        }
        let discontinued = match body.get("isDiscontinued") {
            Node::Null => false,
            Node::Bool(b) => *b,
            _ => return Err(bad("isDiscontinued must be a bool")),
        };
        let replaced_by = match body.get("replacedBy") {
            Node::Null => None,
            Node::String(s) => Some(s.clone()),
            _ => return Err(bad("replacedBy must be a String")),
        };
        let result = Rc::new(Listing {
            versions: result,
            discontinued,
            replaced_by,
        });
        self.listings.insert(key, result.clone());
        Ok(result)
    }

    /// Pub's `downloadToSystemCache`: reuses a cached package whose recorded
    /// hash matches the listing, otherwise downloads and verifies it.
    /// Returns the content hash.
    pub fn download(
        &mut self,
        name: &str,
        version: &Version,
        url: &str,
        listed: Option<&Listed>,
    ) -> Result<String, String> {
        let directory = self.directory(name, version, url);
        let hash_path = self.hash_path(name, version, url);
        let expected = listed.and_then(|l| l.sha256.clone());
        let cached = self.cached_hash(name, version, url);
        match (&cached, &expected) {
            (None, _) if directory.exists() && !self.offline => {
                remove(&directory)?;
            }
            (Some(cached), Some(expected)) if cached != expected => {
                eprintln!("Cached version of {name}-{version} has wrong hash - redownloading.");
                if self.offline {
                    return Err(
                        "Cannot redownload while offline. Try again without --offline.".into(),
                    );
                }
                remove(&directory)?;
            }
            _ => {}
        }
        if directory.is_dir() {
            return cached.ok_or_else(|| format!("{}: missing content hash", hash_path.display()));
        }
        if self.offline {
            return Err(format!(
                "Missing package {name}-{version}. Try again without --offline."
            ));
        }
        let archive = listed
            .map(|l| l.archive_url.clone())
            .unwrap_or_else(|| api_url(url, &format!("packages/{name}/versions/{version}.tar.gz")));
        let archive_url = Url::parse(&archive).map_err(|e| e.to_string())?;
        if !matches!(archive_url.scheme(), "http" | "https") {
            return Err("invalid archive transport".into());
        }
        let bytes = download(&archive, None).map_err(|e| format!("download {archive}: {e}"))?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        if let Some(expected) = &expected
            && *expected != digest
        {
            return Err(format!(
                "Downloaded archive for {name}-{version} had wrong content-hash.\n\nThis indicates a problem on the package repository: `{url}`."
            ));
        }
        let parent = directory.parent().ok_or("invalid cache path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temporary = self.cache.join("_temp");
        std::fs::create_dir_all(&temporary).map_err(|e| e.to_string())?;
        let staging = temporary.join(format!(
            "dartforge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&staging).map_err(|e| e.to_string())?;
        let result = (|| {
            unpack(&bytes, &staging)?;
            if directory.exists() {
                remove(&directory)?;
            }
            std::fs::rename(&staging, &directory).map_err(|e| e.to_string())?;
            atomic_write(&hash_path, digest.as_bytes())
        })();
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        result?;
        self.downloaded
            .insert((name.to_owned(), version.to_string()));
        Ok(digest)
    }
}

fn remove(path: &Path) -> Result<(), String> {
    std::fs::remove_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn download(url: &str, accept: Option<&str>) -> Result<Vec<u8>, ureq::Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .https_only(url.starts_with("https:"))
        .user_agent(format!("Dart pub {}", env!("CARGO_PKG_VERSION")))
        .build()
        .into();
    let mut request = agent.get(url);
    if let Some(accept) = accept {
        request = request.header("Accept", accept);
    }
    let response = request.call()?;
    let mut reader = response.into_body().into_reader().take(DOWNLOAD_LIMIT + 1);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    if bytes.len() as u64 > DOWNLOAD_LIMIT {
        return Err(ureq::Error::BodyExceedsLimit(DOWNLOAD_LIMIT));
    }
    Ok(bytes)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
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

/// Lexical `p.join` + `p.normalize` of a relative archive path onto
/// `base`; `None` when it would leave `base`.
fn inside(base: &Path, relative: &Path) -> Option<PathBuf> {
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let mut path = base.to_path_buf();
    path.extend(parts);
    Some(path)
}

/// `p.relative(target, from: directory)` for two paths under one root.
fn relative_to(target: &Path, directory: &Path) -> PathBuf {
    let target: Vec<_> = target.components().collect();
    let base: Vec<_> = directory.components().collect();
    let shared = target.iter().zip(&base).take_while(|(a, b)| a == b).count();
    let mut result = PathBuf::new();
    for _ in shared..base.len() {
        result.push("..");
    }
    for part in &target[shared..] {
        result.push(part.as_os_str());
    }
    if result.as_os_str().is_empty() {
        result.push(".");
    }
    result
}

/// Pub's `extractTarGz`: regular files get mode 0644 plus the archive's
/// executable bits, symbolic links are kept when their target stays inside
/// the package (hard links become symbolic links), and other entries are
/// skipped.
pub(crate) fn unpack(bytes: &[u8], destination: &Path) -> Result<(), String> {
    let reader = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(reader);
    let mut total = 0u64;
    let mut names = HashSet::new();
    // GNU long names and link names, which `tar` only applies to archives
    // with the GNU magic; Pub's reader accepts them in any archive.
    let mut long_name: Option<PathBuf> = None;
    let mut long_link: Option<PathBuf> = None;
    let read_long = |entry: &mut tar::Entry<'_, _>| -> Result<PathBuf, String> {
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        Ok(PathBuf::from(String::from_utf8_lossy(&bytes).into_owned()))
    };
    for (index, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        if index >= 200_000 {
            return Err("archive has too many entries".into());
        }
        let mut entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.header().entry_type();
        if kind.is_gnu_longname() {
            long_name = Some(read_long(&mut entry)?);
            continue;
        }
        if kind.is_gnu_longlink() {
            long_link = Some(read_long(&mut entry)?);
            continue;
        }
        if kind.is_pax_global_extensions() || kind.is_pax_local_extensions() {
            continue;
        }
        let name = match long_name.take() {
            Some(name) => name,
            None => entry.path().map_err(|e| e.to_string())?.into_owned(),
        };
        let link_override = long_link.take();
        let Some(path) = inside(destination, &name) else {
            return Err(format!("Invalid tar entry: `{}`", name.display()));
        };
        if !names.insert(path.clone()) {
            return Err(format!(
                "Tar file contained duplicate path {}",
                name.display()
            ));
        }
        if path == destination && !kind.is_dir() {
            return Err(format!("Invalid tar entry: `{}`", name.display()));
        }
        let parent = path.parent().unwrap_or(destination).to_path_buf();
        if kind.is_dir() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        } else if kind.is_file() || kind == tar::EntryType::Continuous {
            total = total
                .checked_add(entry.size())
                .ok_or("archive size overflow")?;
            if total > 2 * 1024 * 1024 * 1024 {
                return Err("expanded archive exceeds 2 GiB limit".into());
            }
            if path.is_symlink() {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
            std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
            let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut file).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = 0o644 | (entry.header().mode().unwrap_or(0) & 0o111);
                if mode != 0o644 {
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| e.to_string())?;
                }
            }
        } else if kind.is_symlink() || kind.is_hard_link() {
            let link = match link_override {
                Some(link) => link,
                None => match entry.link_name().map_err(|e| e.to_string())? {
                    Some(link) => link.into_owned(),
                    None => continue,
                },
            };
            let target = if kind.is_symlink() {
                let relative = parent
                    .strip_prefix(destination)
                    .unwrap_or(Path::new(""))
                    .join(&link);
                inside(destination, &relative)
            } else {
                inside(destination, &link)
            };
            // Links that leave the package are skipped, like Pub does.
            let Some(target) = target.filter(|t| t != destination) else {
                continue;
            };
            std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(relative_to(&target, &parent), &path)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            #[cfg(not(unix))]
            {
                let _ = target;
                return Err("archive links are only supported on Unix".into());
            }
        }
    }
    Ok(())
}
