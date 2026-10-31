//! Native package resolution and cache population; no Dart/Pub subprocess delegation.
mod constraints;
mod git;
mod hosted;
mod manifest;
mod output;
mod resolver;
mod sdk;
#[cfg(test)]
mod tests;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

pub(crate) const USAGE: &str = "usage: dartforge pub get [--offline] [--enforce-lockfile] [--dry-run] [--[no-]example] [--no-precompile] [--directory <dir>] [--sdk <lib>] [--sdk-version <version>]";

/// A failure with the exit code `dart pub get` uses for it.
#[derive(Debug)]
pub(crate) struct Failure {
    pub code: u8,
    pub message: String,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { code: 1, message }
    }
}

impl From<&str> for Failure {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

struct Options {
    directory: PathBuf,
    offline: bool,
    enforce_lockfile: bool,
    dry_run: bool,
    example: bool,
    sdk_version: Option<String>,
    sdk_lib: Option<PathBuf>,
}

fn parse_options(args: &[OsString]) -> Result<Option<Options>, Failure> {
    let usage = || Failure {
        code: 64,
        message: USAGE.into(),
    };
    let mut options = Options {
        directory: std::env::current_dir().map_err(|e| e.to_string())?,
        offline: false,
        enforce_lockfile: false,
        dry_run: false,
        example: true,
        sdk_version: None,
        sdk_lib: None,
    };
    let mut index = 0;
    while index < args.len() {
        let option = args[index]
            .to_str()
            .ok_or("pub get arguments must be UTF-8")?;
        let (name, inline) = match option.split_once('=') {
            Some((name, value)) if name.starts_with("--") => (name, Some(value.to_owned())),
            _ => (option, None),
        };
        let mut value = || -> Result<String, Failure> {
            if let Some(value) = inline.clone() {
                return Ok(value);
            }
            index += 1;
            args.get(index)
                .and_then(|v| v.to_str())
                .map(str::to_owned)
                .ok_or_else(usage)
        };
        match name {
            "--offline" => options.offline = true,
            "--enforce-lockfile" => options.enforce_lockfile = true,
            "--dry-run" | "-n" => options.dry_run = true,
            "--example" => options.example = true,
            "--no-example" => options.example = false,
            // `dart pub get` does not precompile executables by default.
            "--no-precompile" => {}
            "--precompile" => {
                return Err("pub get: --precompile is not supported by dartforge".into());
            }
            "--directory" | "-C" => options.directory = PathBuf::from(value()?),
            "--sdk" => options.sdk_lib = Some(PathBuf::from(value()?)),
            "--sdk-version" => options.sdk_version = Some(value()?),
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(None);
            }
            _ => return Err(usage()),
        }
        index += 1;
    }
    Ok(Some(options))
}

/// Resolves the supported path and hosted package sources and writes Pub
/// artifacts, returning the process exit code.
pub(crate) fn run(args: &[OsString]) -> Result<u8, Failure> {
    let Some(options) = parse_options(args)? else {
        return Ok(0);
    };
    let directory = options
        .directory
        .canonicalize()
        .map_err(|e| format!("{}: {e}", options.directory.display()))?;
    let sdk_lib = options
        .sdk_lib
        .clone()
        .or_else(dartforge_elements::sdk::SdkLayout::discover);
    let sdk_root = sdk_lib
        .map(|lib| {
            let lib = lib
                .canonicalize()
                .map_err(|error| format!("{}: {error}", lib.display()))?;
            if !lib.join("libraries.json").is_file() {
                return Err("--sdk must select the SDK lib directory".to_string());
            }
            lib.parent()
                .map(PathBuf::from)
                .ok_or_else(|| "SDK root is missing".to_string())
        })
        .transpose()?;
    let sdk = match &options.sdk_version {
        Some(version) => constraints::parse_version(version)?,
        None => sdk_root
            .as_deref()
            .map(sdk::version)
            .transpose()?
            .unwrap_or(constraints::parse_version("3.6.2")?),
    };
    let host = hosted::normalize_host(
        &std::env::var("PUB_HOSTED_URL").unwrap_or_else(|_| "https://pub.dev".into()),
    )?;
    let cache = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".pub-cache")))
        .ok_or("set PUB_CACHE to the package cache directory")?;
    let cache = if cache.is_absolute() {
        cache
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(cache)
    };
    let context = Context {
        cache,
        host,
        sdk,
        sdk_root,
        options: &options,
    };
    let workspace = workspace(&directory, &context.host)?;
    get(&context, &workspace, None)?;
    // Like Pub, also resolve the working package's example, unless it is
    // part of the workspace that was just resolved.
    let example = directory.join("example");
    if options.example
        && example.join("pubspec.yaml").is_file()
        && let Ok(example) = example.canonicalize()
        && !workspace.iter().any(|(_, path)| *path == example)
    {
        let example_workspace = self::workspace(&example, &context.host)?;
        get(&context, &example_workspace, Some("./example"))?;
    }
    Ok(0)
}

struct Context<'a> {
    cache: PathBuf,
    host: String,
    sdk: semver::Version,
    sdk_root: Option<PathBuf>,
    options: &'a Options,
}

/// Resolves one workspace and writes its outputs according to the options.
fn get(
    context: &Context<'_>,
    workspace: &[(manifest::Manifest, PathBuf)],
    label: Option<&str>,
) -> Result<(), Failure> {
    let options = context.options;
    let root_directory = workspace[0].1.clone();
    let lock_path = root_directory.join("pubspec.lock");
    if options.enforce_lockfile && !lock_path.is_file() {
        return Err(
            "Retrieving dependencies failed.\nCannot do `--enforce-lockfile` without an existing `pubspec.lock`.\n\nTry running `dart pub get` to create `pubspec.lock`."
                .into(),
        );
    }
    let locked = output::read_lock(&root_directory, &context.host)?;
    let locked_digests = output::read_lock_digests(&root_directory)?;
    let mut resolver = resolver::Resolver::new(
        hosted::Hosted::new(context.cache.clone(), options.offline, context.host.clone()),
        context.sdk.clone(),
        locked.clone(),
    );
    resolver.sdk_root = context.sdk_root.clone();
    resolver.locked_digests = locked_digests.clone();
    let selected = resolver.resolve_workspace(workspace)?;
    let changes = changed(&locked, &locked_digests, &selected);
    let place = label.map(|l| format!(" in `{l}`")).unwrap_or_default();
    if options.dry_run {
        match changes {
            0 => println!("No dependencies would change{place}."),
            1 => println!("Would change 1 dependency{place}."),
            n => println!("Would change {n} dependencies{place}."),
        }
        return Ok(());
    }
    if options.enforce_lockfile && changes > 0 {
        return Err(Failure {
            code: 65,
            message: "Unable to satisfy `pubspec.yaml` using `pubspec.lock`.\n\nTo update `pubspec.lock` run `dart pub get` without\n`--enforce-lockfile`.".into(),
        });
    }
    for value in selected.values() {
        match &value.source {
            manifest::Source::Hosted(host) => {
                resolver
                    .hosted
                    .materialize(&value.candidate, host)
                    .map_err(|error| {
                        format!(
                            "{} {}: {error}",
                            value.candidate.manifest.name, value.candidate.manifest.version
                        )
                    })?;
            }
            manifest::Source::Git { url, path, .. } => {
                let commit = value
                    .candidate
                    .digest
                    .as_deref()
                    .ok_or("missing resolved git commit")?;
                resolver.git.materialize(url, path, commit)?;
            }
            manifest::Source::Path(_) | manifest::Source::Sdk(_) => {}
        }
    }
    output::write_workspace(workspace, &selected, &context.sdk)?;
    if workspace.len() > 1 || !workspace[0].0.workspace.is_empty() {
        println!(
            "Resolved {} dependencies of the workspace at {} with DartForge.",
            selected.len(),
            root_directory.display()
        );
    } else {
        println!(
            "Resolved {} dependencies{place} with DartForge.",
            selected.len()
        );
    }
    Ok(())
}

/// Pub's `samePackageIds`: names, versions and resolved descriptions (path,
/// SDK, hosted URL and content hash, or Git commit) are compared with the
/// lockfile.
fn changed(
    locked: &BTreeMap<String, (manifest::Source, semver::Version)>,
    digests: &BTreeMap<String, String>,
    selected: &BTreeMap<String, resolver::Selected>,
) -> usize {
    let mut count = locked
        .keys()
        .filter(|name| !selected.contains_key(*name))
        .count();
    for (name, value) in selected {
        let same = locked.get(name).is_some_and(|(source, version)| {
            *source == value.source
                && *version == value.candidate.manifest.version
                && match &value.source {
                    manifest::Source::Hosted(_) | manifest::Source::Git { .. } => {
                        digests.get(name).map(String::as_str) == value.candidate.digest.as_deref()
                    }
                    _ => true,
                }
        });
        if !same {
            count += 1;
        }
    }
    count
}

/// Finds the workspace that `directory` belongs to, like Pub 3.6: the
/// nearest enclosing package without `resolution: workspace` whose
/// transitive workspace contains it. The root comes first, followed by its
/// members depth-first in written order.
fn workspace(
    directory: &std::path::Path,
    host: &str,
) -> Result<Vec<(manifest::Manifest, PathBuf)>, String> {
    let mut first: Option<PathBuf> = None;
    for ancestor in directory.ancestors() {
        if !ancestor.join("pubspec.yaml").is_file() {
            continue;
        }
        let manifest = manifest::Manifest::read(ancestor, host, true)?;
        first.get_or_insert_with(|| ancestor.to_path_buf());
        if manifest.member {
            continue;
        }
        let mut packages = Vec::new();
        expand(manifest, ancestor.to_path_buf(), host, &mut packages)?;
        if packages.iter().any(|(_, path)| path == directory) {
            return Ok(packages);
        }
    }
    Err(match first {
        None => format!(
            "Found no `pubspec.yaml` file in `{}` or parent directories",
            directory.display()
        ),
        Some(path) => format!(
            "Found a pubspec.yaml at {}. But it has resolution `workspace`.\nBut found no workspace root including it in parent directories.\n\nSee https://dart.dev/go/pub-workspaces for more information.",
            path.display()
        ),
    })
}

fn expand(
    manifest: manifest::Manifest,
    directory: PathBuf,
    host: &str,
    packages: &mut Vec<(manifest::Manifest, PathBuf)>,
) -> Result<(), String> {
    if packages.iter().any(|(_, path)| *path == directory) {
        return Err(format!(
            "{} is included twice in the workspace",
            directory.display()
        ));
    }
    let members = manifest.workspace.clone();
    packages.push((manifest, directory));
    for member in members {
        let child = manifest::Manifest::read(&member, host, true)?;
        if !child.member {
            return Err(format!(
                "{} is a workspace member but its pubspec.yaml lacks `resolution: workspace`",
                member.display()
            ));
        }
        expand(child, member, host, packages)?;
    }
    Ok(())
}

/// Shares the selected SDK mapping with distribution assembly.
#[cfg(feature = "native")]
pub(crate) fn sdk_package_paths(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<String, PathBuf>, String> {
    sdk::declared_package_paths(root)
}
