//! Tests of the pub commands against a local registry; no Dart or Pub
//! process and no network.
use super::*;
use std::{
    collections::HashMap,
    io::{BufRead, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dartforge-pub-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn package(&self, path: &str, text: &str) -> PathBuf {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.join("lib")).unwrap();
        std::fs::write(path.join("pubspec.yaml"), text).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A tar.gz archive with the given regular files and links.
fn archive(files: &[(&str, &str)], links: &[(&str, &str, tar::EntryType)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    for (path, text) in files {
        let mut header = tar::Header::new_ustar();
        header.set_size(text.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        builder
            .append_data(&mut header, path, text.as_bytes())
            .unwrap();
    }
    for (path, target, kind) in links {
        let mut header = tar::Header::new_ustar();
        header.set_size(0);
        header.set_entry_type(*kind);
        builder.append_link(&mut header, path, target).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// A local package server: `packages` maps names to (version, pubspec
/// YAML) rows; listings and archives are served from memory.
struct Registry {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
}

fn registry(packages: &[(&str, &[(&str, &str)])]) -> Registry {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut routes: HashMap<String, Vec<u8>> = HashMap::new();
    for (name, versions) in packages {
        let mut rows = Vec::new();
        for (version, pubspec) in *versions {
            let bytes = archive(&[("pubspec.yaml", pubspec), ("lib/a.dart", "")], &[]);
            let sha = format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(&bytes));
            let path = format!("/archives/{name}-{version}.tar.gz");
            rows.push(serde_json::json!({
                "version": version,
                "pubspec": yaml_to_json(pubspec),
                "archive_url": format!("{url}{path}"),
                "archive_sha256": sha,
            }));
            routes.insert(path, bytes);
        }
        routes.insert(
            format!("/api/packages/{name}"),
            serde_json::to_vec(&serde_json::json!({"name": name, "versions": rows})).unwrap(),
        );
    }
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
            let mut first = String::new();
            if reader.read_line(&mut first).is_err() {
                continue;
            }
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line != "\r\n" {
                line.clear();
            }
            let path = first.split(' ').nth(1).unwrap_or("/").to_owned();
            log.lock().unwrap().push(path.clone());
            match routes.get(&path) {
                Some(body) => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(body);
                }
                None => {
                    let _ = write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                }
            }
        }
    });
    Registry { url, requests }
}

fn yaml_to_json(text: &str) -> serde_json::Value {
    fn convert(node: &tree::Node) -> serde_json::Value {
        match node {
            tree::Node::Null => serde_json::Value::Null,
            tree::Node::Bool(b) => (*b).into(),
            tree::Node::Number(n) => serde_json::from_str(n).unwrap(),
            tree::Node::String(s) => s.clone().into(),
            tree::Node::List(items) => items.iter().map(convert).collect(),
            tree::Node::Map(entries) => entries
                .iter()
                .map(|(k, v)| (k.clone(), convert(v)))
                .collect::<serde_json::Map<_, _>>()
                .into(),
        }
    }
    convert(&tree::parse_yaml(text).unwrap())
}

fn settings(cache: &Path, host: &str) -> Settings {
    Settings {
        cache_root: cache.to_path_buf(),
        host: host.to_owned(),
        sdk: Version::parse("3.6.2").unwrap(),
        sdk_root: None,
    }
}

fn get(
    directory: &Path,
    settings: &Settings,
    kind: SolveType,
    unlock: &[&str],
) -> Result<(), Failure> {
    let entry = Entrypoint::load(&directory.display().to_string(), settings)?;
    acquire(
        &entry,
        settings,
        false,
        kind,
        &Acquire {
            unlock: unlock.iter().map(|s| s.to_string()).collect(),
            ..Acquire::default()
        },
    )
}

const SDK: &str = "environment:\n  sdk: ^3.5.0\n";

#[test]
fn conflicts_are_learned_and_the_lockfile_is_written_like_pub() {
    let fixture = Fixture::new();
    // The newest `a` needs a `c` that `b` excludes; PubGrub learns that
    // and settles on `a 1.0.0` without trying every combination.
    let registry = registry(&[
        (
            "a",
            &[
                (
                    "1.0.0",
                    "name: a\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  c: ^1.0.0\n",
                ),
                (
                    "2.0.0",
                    "name: a\nversion: 2.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  c: ^2.0.0\n",
                ),
            ],
        ),
        (
            "b",
            &[(
                "1.0.0",
                "name: b\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  c: ^1.0.0\n",
            )],
        ),
        (
            "c",
            &[
                (
                    "1.0.0",
                    "name: c\nversion: 1.0.0\nenvironment:\n  sdk: '>=2.12.0 <3.0.0'\n",
                ),
                (
                    "1.1.0",
                    "name: c\nversion: 1.1.0\nenvironment:\n  sdk: ^3.1.0\n",
                ),
                (
                    "2.0.0",
                    "name: c\nversion: 2.0.0\nenvironment:\n  sdk: ^3.0.0\n",
                ),
            ],
        ),
    ]);
    let app = fixture.package(
        "app",
        &format!("name: app\n{SDK}dependencies:\n  a: any\n  b: any\n"),
    );
    let settings = settings(&fixture.0.join("cache"), &registry.url);
    get(&app, &settings, SolveType::Get, &[]).unwrap();
    let lock = std::fs::read_to_string(app.join("pubspec.lock")).unwrap();
    let url = &registry.url;
    let sha = |name: &str, version: &str| {
        std::fs::read_to_string(fixture.0.join(format!(
            "cache/hosted-hashes/{}/{name}-{version}.sha256",
            hosted::url_to_directory(url)
        )))
        .unwrap()
    };
    assert_eq!(
        lock,
        format!(
            "# Generated by pub\n# See https://dart.dev/tools/pub/glossary#lockfile\npackages:\n  a:\n    dependency: \"direct main\"\n    description:\n      name: a\n      sha256: {}\n      url: \"{url}\"\n    source: hosted\n    version: \"1.0.0\"\n  b:\n    dependency: \"direct main\"\n    description:\n      name: b\n      sha256: {}\n      url: \"{url}\"\n    source: hosted\n    version: \"1.0.0\"\n  c:\n    dependency: transitive\n    description:\n      name: c\n      sha256: {}\n      url: \"{url}\"\n    source: hosted\n    version: \"1.1.0\"\nsdks:\n  dart: \">=3.5.0 <4.0.0\"\n",
            output::quote(&sha("a", "1.0.0")),
            output::quote(&sha("b", "1.0.0")),
            output::quote(&sha("c", "1.1.0"))
        )
    );
    let config = std::fs::read_to_string(app.join(".dart_tool/package_config.json")).unwrap();
    assert!(config.contains("\"generator\": \"pub\""));
    assert!(config.contains("\"generatorVersion\": \"3.6.2\""));
    assert!(config.contains("\"rootUri\": \"../\""));
    // Locked versions are kept; `upgrade` unlocks them.
    std::fs::write(
        app.join("pubspec.yaml"),
        format!("name: app\n{SDK}dependencies:\n  a: any\n"),
    )
    .unwrap();
    get(&app, &settings, SolveType::Get, &[]).unwrap();
    let lock = std::fs::read_to_string(app.join("pubspec.lock")).unwrap();
    assert!(lock.contains("version: \"1.0.0\"") && !lock.contains("  b:"));
    get(&app, &settings, SolveType::Upgrade, &[]).unwrap();
    let lock = std::fs::read_to_string(app.join("pubspec.lock")).unwrap();
    assert!(lock.contains("version: \"2.0.0\""), "{lock}");
    assert!(
        registry
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r == "/api/packages/c")
    );
}

#[test]
fn failures_are_explained_like_pub() {
    let fixture = Fixture::new();
    let registry = registry(&[
        (
            "a",
            &[(
                "1.0.0",
                "name: a\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\ndependencies:\n  b: ^2.0.0\n",
            )],
        ),
        (
            "b",
            &[
                (
                    "1.0.0",
                    "name: b\nversion: 1.0.0\nenvironment:\n  sdk: ^3.0.0\n",
                ),
                (
                    "2.0.0",
                    "name: b\nversion: 2.0.0\nenvironment:\n  sdk: ^3.0.0\n",
                ),
            ],
        ),
    ]);
    let app = fixture.package(
        "app",
        &format!("name: app\n{SDK}dependencies:\n  a: ^1.0.0\n  b: ^1.0.0\n"),
    );
    let settings = settings(&fixture.0.join("cache"), &registry.url);
    let error = get(&app, &settings, SolveType::Get, &[]).unwrap_err();
    assert_eq!(error.code, 1);
    assert_eq!(
        error.message,
        "Because app depends on a ^1.0.0 which depends on b ^2.0.0, b ^2.0.0 is required.\nSo, because app depends on b ^1.0.0, version solving failed."
    );
    std::fs::write(
        app.join("pubspec.yaml"),
        format!("name: app\n{SDK}dependencies:\n  nope: any\n"),
    )
    .unwrap();
    let error = get(&app, &settings, SolveType::Get, &[]).unwrap_err();
    assert_eq!(error.code, exit::UNAVAILABLE);
    assert_eq!(
        error.message,
        format!(
            "Because app depends on nope any which doesn't exist (could not find package nope at {}), version solving failed.",
            registry.url
        )
    );
}

#[test]
fn archives_keep_inner_links_and_skip_escaping_ones() {
    let fixture = Fixture::new();
    let bytes = archive(
        &[("pubspec.yaml", "name: x\n"), ("lib/a.dart", "")],
        &[
            ("lib/b.dart", "a.dart", tar::EntryType::Symlink),
            ("lib/c.dart", "lib/a.dart", tar::EntryType::Link),
            ("lib/out.dart", "../../outside", tar::EntryType::Symlink),
        ],
    );
    let destination = fixture.0.join("x");
    std::fs::create_dir(&destination).unwrap();
    hosted::unpack(&bytes, &destination).unwrap();
    assert_eq!(
        std::fs::read_link(destination.join("lib/b.dart")).unwrap(),
        PathBuf::from("a.dart")
    );
    assert_eq!(
        std::fs::read_link(destination.join("lib/c.dart")).unwrap(),
        PathBuf::from("a.dart")
    );
    assert!(
        !destination.join("lib/out.dart").exists()
            && destination.join("lib/out.dart").symlink_metadata().is_err()
    );
    // A GNU long name in an archive with the POSIX magic.
    let long = format!("lib/{}.dart", "n".repeat(120));
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    let mut name_header = tar::Header::new_ustar();
    name_header.set_entry_type(tar::EntryType::GNULongName);
    name_header.set_size(long.len() as u64 + 1);
    name_header.as_old_mut().name[..13].copy_from_slice(b"././@LongLink");
    name_header.set_cksum();
    builder
        .append(&name_header, format!("{long}\0").as_bytes())
        .unwrap();
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(2);
    header.as_old_mut().name[..10].copy_from_slice(b"lib/short_");
    header.set_mode(0o755);
    header.set_cksum();
    builder.append(&header, b"//".as_slice()).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let destination = fixture.0.join("y");
    std::fs::create_dir(&destination).unwrap();
    hosted::unpack(&bytes, &destination).unwrap();
    assert_eq!(
        std::fs::read_to_string(destination.join(&long)).unwrap(),
        "//"
    );
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::fast(),
    ));
    let mut header = tar::Header::new_ustar();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_size(0);
    header.as_old_mut().name[..7].copy_from_slice(b"../evil");
    header.set_cksum();
    builder.append(&header, b"".as_slice()).unwrap();
    let escaping = builder.into_inner().unwrap().finish().unwrap();
    std::fs::create_dir(fixture.0.join("z")).unwrap();
    assert!(hosted::unpack(&escaping, &fixture.0.join("z")).is_err());
}

#[test]
fn workspaces_resolve_together_and_respect_pub_rules() {
    let fixture = Fixture::new();
    let settings = settings(&fixture.0.join("cache"), "https://pub.invalid");
    let root = fixture.package(
        "ws",
        "name: ws\nenvironment:\n  sdk: ^3.5.0\nworkspace:\n  - pkgs/a\n  - pkgs/b\n",
    );
    fixture.package("ws/pkgs/a", "name: a\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\ndependencies:\n  b:\n    path: ../b\n  c:\n    path: ../../../c\n");
    fixture.package(
        "ws/pkgs/b",
        "name: b\nenvironment:\n  sdk: ^3.5.0\nresolution: workspace\n",
    );
    fixture.package("c", "name: c\nenvironment:\n  sdk: ^3.0.0\n");
    std::fs::write(root.join("pkgs/a/pubspec.lock"), "").unwrap();
    get(&root.join("pkgs/a"), &settings, SolveType::Get, &[]).unwrap();
    let lock = std::fs::read_to_string(root.join("pubspec.lock")).unwrap();
    assert!(lock.contains("  c:\n    dependency: transitive\n    description:\n      path: \"../c\"\n      relative: true\n    source: path\n    version: \"0.0.0\"\n"), "{lock}");
    assert!(!lock.contains("  b:"));
    assert!(!root.join("pkgs/a/pubspec.lock").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("pkgs/b/.dart_tool/pub/workspace_ref.json")).unwrap(),
        "{\n  \"workspaceRoot\": \"../../../..\"\n}\n"
    );
    // `resolution: local` is not a workspace root, and globs are paths.
    let local = fixture.package(
        "local",
        "name: local\nenvironment:\n  sdk: ^3.5.0\nresolution: local\n",
    );
    let error = get(&local, &settings, SolveType::Get, &[]).err().unwrap();
    assert!(
        error.message.contains("But it has resolution `local`."),
        "{}",
        error.message
    );
    let glob = fixture.package(
        "glob",
        "name: glob\nenvironment:\n  sdk: ^3.5.0\nworkspace:\n  - pkgs/*\n",
    );
    let error = get(&glob, &settings, SolveType::Get, &[]).err().unwrap();
    assert!(error.message.contains("pkgs/*"), "{}", error.message);
    let old = fixture.package(
        "old",
        "name: old\nenvironment:\n  sdk: ^3.4.0\nworkspace:\n  - a\n",
    );
    let error = get(&old, &settings, SolveType::Get, &[]).err().unwrap();
    assert!(
        error
            .message
            .contains("requires at least language version 3.5"),
        "{}",
        error.message
    );
}

fn git(directory: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(directory)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn relative_local_git_repositories_resolve_like_pub() {
    let fixture = Fixture::new();
    let repository = fixture.package("repo", "name: lib_a\nenvironment:\n  sdk: ^3.0.0\n");
    git(&repository, &["init", "-q"]);
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-q", "-m", "one"]);
    let commit = git(&repository, &["rev-parse", "HEAD"]);
    let app = fixture.package(
        "app",
        &format!("name: app\n{SDK}dependencies:\n  lib_a:\n    git: ../repo\n"),
    );
    let settings = settings(&fixture.0.join("cache"), "https://pub.invalid");
    get(&app, &settings, SolveType::Get, &[]).unwrap();
    let lock = std::fs::read_to_string(app.join("pubspec.lock")).unwrap();
    assert!(
        lock.contains(&format!("    description:\n      path: \".\"\n      ref: HEAD\n      resolved-ref: {}\n      url: \"../repo\"\n    source: git\n", output::quote(&commit))),
        "{lock}"
    );
    assert!(
        fixture
            .0
            .join(format!("cache/git/repo-{commit}/pubspec.yaml"))
            .is_file()
    );
}

#[test]
fn pub_add_and_remove_edit_the_pubspec() {
    let fixture = Fixture::new();
    let registry = registry(&[(
        "a",
        &[(
            "1.2.0",
            "name: a\nversion: 1.2.0\nenvironment:\n  sdk: ^3.0.0\n",
        )],
    )]);
    let app = fixture.package(
        "app",
        &format!("name: app\n{SDK}dependencies:\n  b:\n    path: ../b\n  z:\n    path: ../z\n"),
    );
    fixture.package("b", "name: b\nenvironment:\n  sdk: ^3.0.0\n");
    fixture.package("z", "name: z\nenvironment:\n  sdk: ^3.0.0\n");
    let settings = settings(&fixture.0.join("cache"), &registry.url);
    let entry = Entrypoint::load(&app.display().to_string(), &settings).unwrap();
    let mut editor =
        yaml_edit::Editor::new(&std::fs::read_to_string(app.join("pubspec.yaml")).unwrap());
    let mut cache = settings.cache(false);
    let mut resolution = entry.clone();
    resolution.workspace[0]
        .manifest
        .dependencies
        .push(manifest::Dependency {
            name: "a".into(),
            description: Description::Hosted(registry.url.clone()),
            constraint: VersionSet::any(),
        });
    let result = solve(
        &resolution,
        &mut cache,
        SolveType::Upgrade,
        &LockFile::default(),
        &[],
    )
    .unwrap();
    let a = result.packages.iter().find(|p| p.name == "a").unwrap();
    editor.set(
        "dependencies",
        "a",
        &yaml_edit::Value::Scalar(VersionSet::compatible_with(&a.version).to_string()),
    );
    assert_eq!(
        editor.text(),
        format!(
            "name: app\n{SDK}dependencies:\n  a: ^1.2.0\n  b:\n    path: ../b\n  z:\n    path: ../z\n"
        )
    );
}
