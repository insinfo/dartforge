//! `dart pub cache`: `add`, `clean`, `list`, `preload` and `repair`, over
//! Pub 3.6.2's `PUB_CACHE` layout.
use super::{
    Arguments, COMMON, Failure, Settings, Spec, exit, flag,
    hosted::{ensure_readme, unpack, url_to_directory},
    manifest::{Context, Description, Manifest, Role},
    option,
    source::{Cache, PackageId, SourceError},
    usage,
    version::{Version, VersionSet},
};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

const FOOTER: &str = "Run \"dart help\" to see global options.";
const DOCS: &str = "See https://dart.dev/tools/pub/cmd/pub-cache for detailed documentation.";

const CACHE_USAGE: &str = "Usage: dart pub cache [arguments...]
-h, --help    Print this usage information.

Available subcommands:
  add      Install a package.
  clean    Clears the global PUB_CACHE.
  repair   Reinstall cached packages.";

const ADD_USAGE: &str = "Usage: dart pub cache add <package> [--version <constraint>] [--all]
-h, --help       Print this usage information.
    --all        Install all matching versions.
-v, --version    Version constraint.";

const CLEAN_USAGE: &str = "Usage: dart pub cache clean <subcommand> [arguments...]
-h, --help     Print this usage information.
-f, --force    Don't ask for confirmation.";

const LIST_USAGE: &str = "Usage: dart pub cache list <subcommand> [arguments...]
-h, --help    Print this usage information.";

const PRELOAD_USAGE: &str = "Usage: dart pub cache preload <package1.tar.gz> ...
-h, --help    Print this usage information.";

const REPAIR_USAGE: &str = "Usage: dart pub cache repair <subcommand> [arguments...]
-h, --help    Print this usage information.";

/// The usage text of a command, with Pub's footer.
fn full_usage(usage_text: &str, docs: bool) -> String {
    if docs {
        format!("{usage_text}\n\n{FOOTER}\n{DOCS}")
    } else {
        format!("{usage_text}\n\n{FOOTER}")
    }
}

fn usage_error(message: &str, usage_text: &str, docs: bool) -> Failure {
    usage(format!("{message}\n\n{}", full_usage(usage_text, docs)))
}

/// Pub's `log.green`, `log.red` and `log.bold`: colors only on a terminal.
fn ansi(code: &str, text: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\u{1b}[{code}m{text}\u{1b}[0m")
    } else {
        text.to_owned()
    }
}

/// `pluralize`.
fn plural(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// `toSentence`: `a`, `a and b`, `a, b and c`.
pub(crate) fn to_sentence(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// Parses a cache subcommand's arguments; `--help` prints its description.
fn parse(
    args: &[String],
    specs: &[Spec],
    command: &str,
    description: &str,
    usage_text: &str,
    docs: bool,
) -> Result<Option<(Arguments, Settings)>, Failure> {
    let mut all: Vec<Spec> = Vec::new();
    for spec in specs.iter().chain(COMMON.iter()) {
        all.push(Spec {
            name: spec.name,
            abbr: spec.abbr,
            value: spec.value,
            negatable: spec.negatable,
        });
    }
    let arguments = Arguments::parse(args, &all, &format!("cache {command}"))?;
    if arguments.is("help") {
        println!("{description}\n\n{}", full_usage(usage_text, docs));
        return Ok(None);
    }
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    Ok(Some((arguments, settings)))
}

/// `takesArguments => false`.
fn no_arguments(
    arguments: &Arguments,
    command: &str,
    usage_text: &str,
    docs: bool,
) -> Result<(), Failure> {
    if arguments.rest.is_empty() {
        return Ok(());
    }
    Err(usage_error(
        &format!("Command \"{command}\" does not take any arguments."),
        usage_text,
        docs,
    ))
}

/// Runs `dart pub cache <args>`.
pub(crate) fn run(args: &[String]) -> Result<u8, Failure> {
    let Some(first) = args.first() else {
        return Err(usage_error(
            "Missing subcommand for \"dart pub cache\".",
            CACHE_USAGE,
            true,
        ));
    };
    let rest = &args[1..];
    match first.as_str() {
        "-h" | "--help" => {
            println!(
                "Work with the system cache.\n\n{}",
                full_usage(CACHE_USAGE, true)
            );
            Ok(0)
        }
        "add" => add(rest),
        "clean" => clean(rest),
        "list" => list(rest),
        "preload" => preload(rest),
        "repair" => repair(rest),
        other if other.starts_with('-') => Err(usage_error(
            &format!(
                "Could not find an option named \"{}\".",
                other.trim_start_matches('-')
            ),
            CACHE_USAGE,
            true,
        )),
        other => Err(usage_error(
            &format!("Could not find a subcommand named \"{other}\" for \"dart pub cache\"."),
            CACHE_USAGE,
            true,
        )),
    }
}

fn source_message(error: SourceError) -> Failure {
    match error {
        SourceError::NotFound(message) => Failure {
            code: exit::UNAVAILABLE,
            message: format!("Package not available ({message})."),
        },
        SourceError::Invalid(message) | SourceError::Fatal(message) => message.into(),
    }
}

/// `pub cache add`.
fn add(args: &[String]) -> Result<u8, Failure> {
    let specs = [flag("all", None, false), option("version", Some('v'))];
    let Some((arguments, settings)) =
        parse(args, &specs, "add", "Install a package.", ADD_USAGE, true)?
    else {
        return Ok(0);
    };
    if arguments.rest.is_empty() {
        return Err(usage_error("No package to add given.", ADD_USAGE, true));
    }
    if arguments.rest.len() > 1 {
        let unexpected: Vec<String> = arguments.rest[1..]
            .iter()
            .map(|a| format!("\"{a}\""))
            .collect();
        return Err(usage_error(
            &format!(
                "Unexpected {} {}.",
                plural("argument", unexpected.len()),
                to_sentence(&unexpected)
            ),
            ADD_USAGE,
            true,
        ));
    }
    let package = &arguments.rest[0];
    let constraint = match arguments.option("version") {
        Some(text) => {
            VersionSet::parse(text).map_err(|message| usage_error(&message, ADD_USAGE, true))?
        }
        None => VersionSet::any(),
    };
    add_package(&settings, package, &constraint, arguments.is("all"))?;
    Ok(0)
}

/// Downloads the best version of `package` allowed by `constraint`, or all
/// of them in ascending order.
pub(crate) fn add_package(
    settings: &Settings,
    package: &str,
    constraint: &VersionSet,
    all: bool,
) -> Result<(), Failure> {
    let mut cache = settings.cache(false);
    let description = Description::Hosted(settings.host.clone());
    let mut ids: Vec<PackageId> = cache
        .versions(package, &description, None)
        .map_err(source_message)?
        .into_iter()
        .filter(|id| constraint.allows(&id.version))
        .collect();
    if ids.is_empty() {
        return Err(format!("Package {package} has no versions that match {constraint}.").into());
    }
    let mut download = |id: &PackageId| -> Result<(), Failure> {
        let before = cache.hosted.downloaded.len();
        cache.download(id)?;
        if cache.hosted.downloaded.len() == before {
            println!("Already cached {} {}.", id.name, id.version);
        }
        Ok(())
    };
    if all {
        ids.sort_by(|a, b| a.version.cmp(&b.version));
        for id in &ids {
            download(id)?;
        }
    } else {
        ids.sort_by(|a, b| prioritize(&a.version, &b.version));
        let best = ids.last().expect("not empty").clone();
        download(&best)?;
    }
    Ok(())
}

/// `Version.prioritize`: stable versions before prereleases, then by version.
fn prioritize(a: &Version, b: &Version) -> std::cmp::Ordering {
    match (a.is_prerelease(), b.is_prerelease()) {
        (false, true) => std::cmp::Ordering::Greater,
        (true, false) => std::cmp::Ordering::Less,
        _ => a.cmp(b),
    }
}

/// `confirm`: asks on stdout and reads a `y`/`yes` answer from stdin.
fn confirm(message: &str) -> bool {
    print!("{message} (y/N)? ");
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// `pub cache clean`.
fn clean(args: &[String]) -> Result<u8, Failure> {
    let specs = [flag("force", Some('f'), false)];
    let Some((arguments, settings)) = parse(
        args,
        &specs,
        "clean",
        "Clears the global PUB_CACHE.",
        CLEAN_USAGE,
        false,
    )?
    else {
        return Ok(0);
    };
    no_arguments(&arguments, "clean", CLEAN_USAGE, false)?;
    let root = &settings.cache_root;
    if root.is_dir() {
        let display = root.display();
        if arguments.is("force")
            || confirm(&format!(
                "This will remove everything inside {display}.\nYou will have to run `dart pub get` again in each project.\nAre you sure?"
            ))
        {
            println!("Removing pub cache directory {display}.");
            std::fs::remove_dir_all(root).map_err(|e| format!("{display}: {e}"))?;
            std::fs::create_dir_all(root).map_err(|e| format!("{display}: {e}"))?;
            ensure_readme(root);
        }
    } else {
        println!("No pub cache at {}.", root.display());
    }
    Ok(0)
}

/// `listDir`: the entries of `directory` in the order the file system lists
/// them, without hidden ones.
fn list_dir(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .collect()
}

/// `Package.load` of a cached package: its pubspec's name and version.
fn load_package(directory: &Path, settings: &Settings) -> Result<(String, Version), String> {
    let containing = Description::Root(directory.to_path_buf());
    let context = Context {
        containing: &containing,
        host: &settings.host,
        sdk: &settings.sdk,
    };
    let manifest = Manifest::read(directory, &context, Role::Dependency)?;
    Ok((manifest.name, manifest.version))
}

/// `_looksLikePackageDir`: `<name>-<version>` directories.
fn looks_like_package_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.split_once('-') {
        Some((_, version)) => Version::parse(version).is_ok() && path.is_dir(),
        None => false,
    }
}

/// `pub cache list`.
fn list(args: &[String]) -> Result<u8, Failure> {
    let Some((arguments, settings)) = parse(
        args,
        &[],
        "list",
        "List packages in the system cache.",
        LIST_USAGE,
        false,
    )?
    else {
        return Ok(0);
    };
    no_arguments(&arguments, "list", LIST_USAGE, false)?;
    println!("{}", cached_packages_json(&settings));
    Ok(0)
}

/// The JSON of `pub cache list`: the packages of the default server.
pub(crate) fn cached_packages_json(settings: &Settings) -> String {
    let directory = settings
        .cache_root
        .join("hosted")
        .join(url_to_directory(&settings.host));
    let mut packages: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for entry in list_dir(&directory)
        .into_iter()
        .filter(|e| looks_like_package_dir(e))
    {
        let Ok((name, version)) = load_package(&entry, settings) else {
            continue;
        };
        let location = entry.to_string_lossy().into_owned();
        match packages.iter_mut().find(|(n, _)| *n == name) {
            Some((_, versions)) => {
                let version = version.to_string();
                match versions.iter_mut().find(|(v, _)| *v == version) {
                    Some(slot) => slot.1 = location,
                    None => versions.push((version, location)),
                }
            }
            None => packages.push((name, vec![(version.to_string(), location)])),
        }
    }
    let json = |text: &str| serde_json::to_string(text).expect("string serialization");
    let body: Vec<String> = packages
        .iter()
        .map(|(name, versions)| {
            let versions: Vec<String> = versions
                .iter()
                .map(|(v, l)| format!("{}:{{\"location\":{}}}", json(v), json(l)))
                .collect();
            format!("{}:{{{}}}", json(name), versions.join(","))
        })
        .collect();
    format!("{{\"packages\":{{{}}}}}", body.join(","))
}

/// `pub cache preload`.
fn preload(args: &[String]) -> Result<u8, Failure> {
    let Some((arguments, settings)) = parse(
        args,
        &[],
        "preload",
        "Install packages from a .tar.gz archive.",
        PRELOAD_USAGE,
        true,
    )?
    else {
        return Ok(0);
    };
    if arguments.rest.is_empty() {
        return Err(usage_error(
            "No package to preload given.",
            PRELOAD_USAGE,
            true,
        ));
    }
    for path in &arguments.rest {
        if !Path::new(path).is_file() {
            return Err(format!("Could not find file {path}.").into());
        }
    }
    let cache = settings.cache(false);
    for path in &arguments.rest {
        let (name, version) = preload_one(path, &cache, &settings)?;
        let url = &settings.host;
        let from = if url == "https://pub.dev" {
            String::new()
        } else {
            format!(" from {url}")
        };
        println!("Installed {path} in cache as {name} {version}{from}.");
    }
    Ok(0)
}

/// `HostedSource.preloadPackage`: extracts the archive into a temporary
/// directory, reads its pubspec and moves it into the cache with its hash.
pub(crate) fn preload_one(
    path: &str,
    cache: &Cache,
    settings: &Settings,
) -> Result<(String, Version), Failure> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let temporary = temp_dir(&cache.root)?;
    let result = (|| -> Result<(String, Version), Failure> {
        // Dart's `GZipCodec` reports any undecodable stream with zlib's text.
        let mut decoded = Vec::new();
        if std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&bytes[..]), &mut decoded)
            .is_err()
        {
            return Err(Failure {
                code: exit::DATA,
                message: format!("Failed to extract `{path}`: Filter error, bad data."),
            });
        }
        unpack(&bytes, &temporary).map_err(|e| Failure {
            code: exit::DATA,
            message: format!("Failed to extract `{path}`: {e}."),
        })?;
        if !temporary.join("pubspec.yaml").is_file() {
            return Err(format!(
                "Found no `pubspec.yaml` in {path}. Is it a valid pub package archive?"
            )
            .into());
        }
        load_package(&temporary, settings)
            .map_err(|e| format!("Failed to load `pubspec.yaml` from `{path}`: {e}.").into())
    })();
    let (name, version) = match result {
        Ok(found) => found,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };
    let directory = cache.hosted.directory(&name, &version, &settings.host);
    if directory.exists() {
        std::fs::remove_dir_all(&directory).map_err(|e| e.to_string())?;
    }
    if let Some(parent) = directory.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&temporary, &directory).map_err(|e| e.to_string())?;
    let hash = cache
        .root
        .join("hosted-hashes")
        .join(url_to_directory(&settings.host))
        .join(format!("{name}-{version}.sha256"));
    super::hosted::atomic_write(&hash, digest.as_bytes())?;
    Ok((name, version))
}

/// `SystemCache.createTempDir`: a fresh directory under `_temp`.
fn temp_dir(root: &Path) -> Result<PathBuf, String> {
    let base = root.join("_temp");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    for attempt in 0..1000u32 {
        let candidate = base.join(format!("dir{}{attempt}", std::process::id()));
        if std::fs::create_dir(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err("could not create a temporary directory in the pub cache".into())
}

/// The result of reinstalling one cached package.
pub(crate) struct Repair {
    pub name: String,
    pub version: Version,
    /// `None` for hosted packages, the source name otherwise.
    pub source: Option<&'static str>,
    pub success: bool,
}

/// `pub cache repair`.
fn repair(args: &[String]) -> Result<u8, Failure> {
    let Some((arguments, settings)) = parse(
        args,
        &[],
        "repair",
        "Reinstall cached packages.",
        REPAIR_USAGE,
        true,
    )?
    else {
        return Ok(0);
    };
    no_arguments(&arguments, "repair", REPAIR_USAGE, true)?;
    let mut cache = settings.cache(false);
    let _ = std::fs::remove_dir_all(cache.root.join("_temp"));
    let mut results = repair_hosted(&mut cache, &settings);
    results.extend(repair_git(&cache, &settings));
    let successes = results.iter().filter(|r| r.success).count();
    let failures: Vec<&Repair> = results.iter().filter(|r| !r.success).collect();
    if successes > 0 {
        println!(
            "Reinstalled {} {}.",
            ansi("32", &successes.to_string()),
            plural("package", successes)
        );
    }
    if !failures.is_empty() {
        let mut text = format!(
            "Failed to reinstall {} {}:\n",
            ansi("31", &failures.len().to_string()),
            plural("package", failures.len())
        );
        for failure in &failures {
            text.push_str(&format!(
                "- {} {}",
                ansi("1", &failure.name),
                failure.version
            ));
            if let Some(source) = failure.source {
                text.push_str(&format!(" from {source}"));
            }
            text.push('\n');
        }
        super::report::message(&text);
    }
    let (reactivated, not_reactivated) = super::global::repair_activated(&settings)?;
    if !reactivated.is_empty() {
        println!(
            "Reactivated {} {}.",
            ansi("32", &reactivated.len().to_string()),
            plural("package", reactivated.len())
        );
    }
    if !not_reactivated.is_empty() {
        println!(
            "Failed to reactivate {} {}:",
            ansi("31", &not_reactivated.len().to_string()),
            plural("package", not_reactivated.len())
        );
        let lines: Vec<String> = not_reactivated
            .iter()
            .map(|name| format!("- {}", ansi("1", name)))
            .collect();
        println!("{}", lines.join("\n"));
    }
    if results.is_empty() {
        println!("No packages in cache, so nothing to repair.");
    }
    if !failures.is_empty() || !not_reactivated.is_empty() {
        return Ok(exit::UNAVAILABLE);
    }
    Ok(0)
}

/// `_directoryToUrl`, the inverse of `_urlToDirectory`: the escaped
/// characters are decoded one by one, then a scheme is added when missing
/// (`http` for a loopback host, `https` otherwise).
pub(crate) fn directory_to_url(directory: &str) -> Option<String> {
    let mut decoded = directory.to_owned();
    for c in "<>:\"\\/|?*%".chars() {
        decoded = decoded.replace(&format!("%{}", c as u32), &c.to_string());
    }
    let url = if decoded.contains("://") {
        decoded
    } else {
        let host = decoded.split(':').next().unwrap_or_default();
        let loopback = host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        format!("{}://{decoded}", if loopback { "http" } else { "https" })
    };
    url::Url::parse(&url).ok().map(|parsed| {
        // Dart's `Uri.toString` keeps the path as written; `url` adds a
        // final slash to an empty one.
        let text = parsed.to_string();
        if url.ends_with('/') {
            text
        } else {
            text.trim_end_matches('/').to_owned()
        }
    })
}

/// `HostedSource.repairCachedPackages`.
pub(crate) fn repair_hosted(cache: &mut Cache, settings: &Settings) -> Vec<Repair> {
    let root = cache.root.join("hosted");
    let mut results = Vec::new();
    for server in list_dir(&root) {
        let directory = server
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(url) = directory_to_url(&directory) else {
            eprintln!("Unable to detect hosted url from directory: {directory}");
            continue;
        };
        let mut packages = Vec::new();
        for entry in list_dir(&server) {
            match load_package(&entry, settings) {
                Ok((name, version)) => packages.push((name, version, entry)),
                Err(error) => {
                    eprintln!("Failed to load package:\n{error}");
                    let base = entry
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let (name, version) = match base.split_once('-') {
                        Some((name, version)) => (
                            name.to_owned(),
                            Version::parse(version).unwrap_or_else(|_| Version::new(0, 0, 0)),
                        ),
                        None => (base.clone(), Version::new(0, 0, 0)),
                    };
                    results.push(Repair {
                        name,
                        version,
                        source: None,
                        success: false,
                    });
                    let _ = remove_entry(&entry);
                }
            }
        }
        let _ = std::fs::remove_dir_all(server.join(".cache"));
        packages.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        for (name, version, directory) in packages {
            let id = PackageId {
                name: name.clone(),
                version: version.clone(),
                description: Description::Hosted(url.clone()),
                resolved: None,
            };
            let _ = remove_entry(&directory);
            match cache.download(&id) {
                Ok(_) => results.push(Repair {
                    name,
                    version,
                    source: None,
                    success: true,
                }),
                Err(error) => {
                    let mut message = format!("Failed to repair {} {version}", ansi("1", &name));
                    if url != settings.host {
                        message.push_str(&format!(" from {url}"));
                    }
                    eprintln!("{message}. Error:\n{error}");
                    let _ = remove_entry(&directory);
                    results.push(Repair {
                        name,
                        version,
                        source: None,
                        success: false,
                    });
                }
            }
        }
    }
    results
}

fn remove_entry(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// `GitSource.repairCachedPackages`: resets every cached checkout.
fn repair_git(cache: &Cache, settings: &Settings) -> Vec<Repair> {
    let root = cache.root.join("git");
    let mut results = Vec::new();
    let mut packages = Vec::new();
    for revision in list_dir(&root)
        .into_iter()
        .filter(|e| e.join(".git").is_dir())
    {
        let list = match std::fs::read_to_string(revision.join(".git/pub-packages")) {
            Ok(text) => text.split('\n').map(str::to_owned).collect(),
            Err(_) => vec![".".to_owned()],
        };
        for relative in list {
            if !revision.is_dir() {
                continue;
            }
            let directory = revision.join(&relative);
            match load_package(&directory, settings) {
                Ok((name, version)) => packages.push((name, version, directory, revision.clone())),
                Err(error) => {
                    eprintln!("Failed to load package:\n{error}");
                    let base = revision
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    results.push(Repair {
                        name: base.split('-').next().unwrap_or_default().to_owned(),
                        version: Version::new(0, 0, 0),
                        source: Some("git"),
                        success: false,
                    });
                    let _ = std::fs::remove_dir_all(&revision);
                }
            }
        }
    }
    packages.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for (name, version, directory, revision) in packages {
        if !directory.is_dir() {
            continue;
        }
        println!(
            "Resetting Git repository for {} {version}...",
            ansi("1", &name)
        );
        let reset = git(&["clean", "-d", "--force", "-x"], &directory)
            .and_then(|_| git(&["reset", "--hard", "HEAD"], &directory));
        match reset {
            Ok(()) => results.push(Repair {
                name,
                version,
                source: Some("git"),
                success: true,
            }),
            Err(error) => {
                eprintln!(
                    "Failed to reset {} {version}. Error:\n{error}",
                    ansi("1", &name)
                );
                let _ = std::fs::remove_dir_all(&revision);
                results.push(Repair {
                    name,
                    version,
                    source: Some("git"),
                    success: false,
                });
            }
        }
    }
    results
}

fn git(args: &[&str], directory: &Path) -> Result<(), String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(directory)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}
