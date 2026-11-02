//! `dart pub global`: `activate`, `deactivate`, `list` and `run`, over
//! Pub 3.6.2's `PUB_CACHE/global_packages` and `PUB_CACHE/bin`.
//!
//! The layout, lockfiles, package configurations, messages and bin stubs are
//! Pub's. Where Pub precompiles an executable into a kernel snapshot
//! (`bin/<script>.dart-<sdk>.snapshot`), DartForge compiles a native
//! executable (`bin/<script>`) with its own compiler; the bin stubs run that
//! executable, or `dartforge pub global run` when it is missing.
use super::{
    Acquire, Arguments, COMMON, Entrypoint, Failure, Settings, Spec, acquire, data, exit, failure,
    flag,
    manifest::{Dependency, Description, Manifest, Resolution, SdkConstraint, resolve_git_url},
    new_lock, option,
    output::{self, NewLock},
    report,
    solver::{SolveError, SolveType, Solver, WorkspacePackage},
    source::{Cache, LockFile, PackageId},
    tree::{Node, parse_yaml},
    usage,
    version::{Version, VersionSet},
};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    io::IsTerminal,
    path::{Path, PathBuf},
};

const FOOTER: &str = "Run \"dart help\" to see global options.";
const DOCS: &str = "See https://dart.dev/tools/pub/cmd/pub-global for detailed documentation.";
/// The name of the placeholder root package of a global activation.
const ROOT: &str = "pub global activate";

const GLOBAL_USAGE: &str = "Usage: dart pub global [arguments...]
-h, --help    Print this usage information.

Available subcommands:
  activate     Make a package's executables globally available.
  deactivate   Remove a previously activated package.
  list         List globally activated packages.
  run          Run an executable from a globally activated package.";

const ACTIVATE_USAGE: &str = "Usage: dart pub global activate <package> [version-constraint]
-h, --help              Print this usage information.
-s, --source            The source used to find the package.
                        [git, hosted (default), path]
    --git-path          Path of git package in repository
    --git-ref           Git branch or commit to be retrieved
    --no-executables    Do not put executables on PATH.
-x, --executable        Executable(s) to place on PATH.
    --overwrite         Overwrite executables from other packages with the same
                        name.
-u, --hosted-url        A custom pub server URL for the package. Only applies
                        when using the `hosted` source.";

const DEACTIVATE_USAGE: &str = "Usage: dart pub global deactivate <package>
-h, --help    Print this usage information.";

const LIST_USAGE: &str = "Usage: dart pub global list <subcommand> [arguments...]
-h, --help    Print this usage information.";

const RUN_USAGE: &str = "Usage: dart pub global run <package>:<executable> [args...]
-h, --help                              Print this usage information.
    --[no-]enable-asserts               Enable assert statements.
    --enable-experiment=<experiment>    Runs the executable in a VM with the
                                        given experiments enabled. (Will disable
                                        snapshotting, resulting in slower
                                        startup).";

fn usage_error(message: &str, usage_text: &str, docs: bool) -> Failure {
    let mut text = format!("{message}\n\n{usage_text}\n\n{FOOTER}");
    if docs {
        text.push('\n');
        text.push_str(DOCS);
    }
    usage(text)
}

/// Pub's `log.bold`: only on a terminal.
fn bold(text: &str) -> String {
    if std::io::stdout().is_terminal() {
        format!("\u{1b}[1m{text}\u{1b}[0m")
    } else {
        text.to_owned()
    }
}

fn plural(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// Parses a global subcommand's arguments; `--help` prints its description.
fn parse(
    args: &[String],
    specs: &[Spec],
    command: &str,
    description: &str,
    usage_text: &str,
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
    let arguments = Arguments::parse(args, &all, &format!("global {command}"))?;
    if arguments.is("help") {
        println!("{description}\n\n{usage_text}\n\n{FOOTER}");
        return Ok(None);
    }
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    Ok(Some((arguments, settings)))
}

/// Runs `dart pub global <args>`.
pub(crate) fn run(args: &[String]) -> Result<u8, Failure> {
    let Some(first) = args.first() else {
        return Err(usage_error(
            "Missing subcommand for \"dart pub global\".",
            GLOBAL_USAGE,
            true,
        ));
    };
    let rest = &args[1..];
    match first.as_str() {
        "-h" | "--help" => {
            println!("Work with global packages.\n\n{GLOBAL_USAGE}\n\n{FOOTER}\n{DOCS}");
            Ok(0)
        }
        "activate" => activate(rest),
        "deactivate" => deactivate(rest),
        "list" => list(rest),
        "run" => run_executable(rest),
        other if other.starts_with('-') => Err(usage_error(
            &format!(
                "Could not find an option named \"{}\".",
                other.trim_start_matches('-')
            ),
            GLOBAL_USAGE,
            true,
        )),
        other => Err(usage_error(
            &format!("Could not find a subcommand named \"{other}\" for \"dart pub global\"."),
            GLOBAL_USAGE,
            true,
        )),
    }
}

fn global_dir(settings: &Settings) -> PathBuf {
    settings.cache_root.join("global_packages")
}

fn bin_dir(settings: &Settings) -> PathBuf {
    settings.cache_root.join("bin")
}

/// `listDir`, without hidden entries, in the file system's order.
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

/// `SystemCache.createTempDir`.
fn temp_dir(root: &Path) -> Result<PathBuf, String> {
    let base = root.join("_temp");
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    for attempt in 0..10_000u32 {
        let candidate = base.join(format!("dir{}{attempt}", std::process::id()));
        if std::fs::create_dir(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err("could not create a temporary directory in the pub cache".into())
}

/// The id of an activated package and its lockfile.
struct Active {
    id: PackageId,
    lock: LockFile,
}

fn load_active(settings: &Settings, name: &str) -> Option<Active> {
    let directory = global_dir(settings).join(name);
    let lock = LockFile::read(&directory, &settings.host).ok()?;
    if !directory.join("pubspec.lock").is_file() {
        return None;
    }
    let id = lock.packages.get(name)?.clone();
    Some(Active { id, lock })
}

use report::pretty_url as pretty_git_url;

/// `_formatPackage`.
fn format_package(id: &PackageId) -> String {
    match &id.description {
        Description::Git { url, .. } => format!(
            "{} {} from Git repository \"{}\"",
            bold(&id.name),
            id.version,
            pretty_git_url(url)
        ),
        Description::Path { path, .. } => {
            format!(
                "{} {} at path \"{}\"",
                bold(&id.name),
                id.version,
                path.display()
            )
        }
        _ => format!("{} {}", bold(&id.name), id.version),
    }
}

/// `_describeActive`: prints how `name` is active and returns its lockfile.
fn describe_active(settings: &Settings, name: &str) -> Option<LockFile> {
    let active = load_active(settings, name)?;
    match &active.id.description {
        Description::Git { url, .. } => println!(
            "Package {} is currently active from Git repository \"{}\".",
            bold(name),
            pretty_git_url(url)
        ),
        Description::Path { path, .. } => println!(
            "Package {} is currently active at path \"{}\".",
            bold(name),
            path.display()
        ),
        _ => println!(
            "Package {} is currently active at version {}.",
            bold(name),
            bold(&active.id.version.to_string())
        ),
    }
    Some(active.lock)
}

/// `pub global activate`.
fn activate(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        option("source", Some('s')),
        option("git-path", None),
        option("git-ref", None),
        option("features", Some('f')),
        option("omit-features", Some('F')),
        flag("no-executables", None, false),
        option("executable", Some('x')),
        flag("overwrite", None, false),
        option("hosted-url", Some('u')),
    ];
    let Some((arguments, settings)) = parse(
        args,
        &specs,
        "activate",
        "Make a package's executables globally available.",
        ACTIVATE_USAGE,
    )?
    else {
        return Ok(0);
    };
    let source = arguments.option("source").unwrap_or("hosted");
    if !matches!(source, "git" | "hosted" | "path") {
        return Err(usage_error(
            &format!("\"{source}\" is not an allowed value for option \"source\"."),
            ACTIVATE_USAGE,
            false,
        ));
    }
    let mut executables: Option<Vec<String>> = None;
    if !arguments.options("executable").is_empty() {
        if arguments.is("no-executables") {
            return Err(usage_error(
                "Cannot pass both --no-executables and --executable.",
                ACTIVATE_USAGE,
                false,
            ));
        }
        executables = Some(
            arguments
                .options("executable")
                .iter()
                .flat_map(|v| v.split(',').map(str::to_owned).collect::<Vec<_>>())
                .collect(),
        );
    } else if arguments.is("no-executables") {
        executables = Some(Vec::new());
    }
    let overwrite = arguments.is("overwrite");
    let mut rest = arguments.rest.iter();
    let extra = |rest: std::slice::Iter<'_, String>| -> Result<(), Failure> {
        let unexpected: Vec<String> = rest.map(|a| format!("\"{a}\"")).collect();
        if unexpected.is_empty() {
            return Ok(());
        }
        Err(usage_error(
            &format!(
                "Unexpected {} {}.",
                plural("argument", unexpected.len()),
                super::cache::to_sentence(&unexpected)
            ),
            ACTIVATE_USAGE,
            false,
        ))
    };
    if source != "git"
        && (arguments.option("git-path").is_some() || arguments.option("git-ref").is_some())
    {
        return Err(usage_error(
            "Options `--git-path` and `--git-ref` can only be used with --source=git.",
            ACTIVATE_USAGE,
            false,
        ));
    }
    match source {
        "git" => {
            let Some(repo) = rest.next() else {
                return Err(usage_error(
                    "No Git repository given.",
                    ACTIVATE_USAGE,
                    false,
                ));
            };
            extra(rest)?;
            activate_git(
                &settings,
                repo,
                arguments.option("git-path"),
                arguments.option("git-ref"),
                executables,
                overwrite,
            )
        }
        "hosted" => {
            let Some(package) = rest.next() else {
                return Err(usage_error(
                    "No package to activate given.",
                    ACTIVATE_USAGE,
                    false,
                ));
            };
            let url = match arguments.option("hosted-url") {
                Some(url) => super::hosted::normalize_host(url).map_err(|e| {
                    usage_error(&format!("Invalid hosted-url: {e}"), ACTIVATE_USAGE, false)
                })?,
                None => settings.host.clone(),
            };
            let constraint = match rest.next() {
                Some(text) => VersionSet::parse(text)
                    .map_err(|message| usage_error(&message, ACTIVATE_USAGE, false))?,
                None => VersionSet::any(),
            };
            extra(rest)?;
            if !super::manifest::valid_name(package) {
                let suggestion = if Path::new(package).is_dir() {
                    format!(
                        "\n\nDid you mean `dart pub global activate --source path {}`?",
                        shell_escape(package)
                    )
                } else {
                    String::new()
                };
                return Err(usage_error(
                    &format!("Not a valid package name: \"{package}\"{suggestion}"),
                    ACTIVATE_USAGE,
                    false,
                ));
            }
            let dependency = Dependency {
                name: package.clone(),
                description: Description::Hosted(url),
                constraint,
            };
            install_in_cache(&settings, dependency, executables, overwrite, false)?;
            Ok(0)
        }
        _ => {
            let Some(path) = rest.next() else {
                return Err(usage_error(
                    "No package to activate given.",
                    ACTIVATE_USAGE,
                    false,
                ));
            };
            extra(rest)?;
            activate_path(&settings, path, executables, overwrite)?;
            Ok(0)
        }
    }
}

/// `escapeShellArgument`.
fn shell_escape(text: &str) -> String {
    if text
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"-_=:/.@+,%".contains(&b))
    {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

/// `GlobalPackages.activateGit`.
fn activate_git(
    settings: &Settings,
    repo: &str,
    path: Option<&str>,
    reference: Option<&str>,
    executables: Option<Vec<String>>,
    overwrite: bool,
) -> Result<u8, Failure> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let containing = Description::Root(cwd);
    let (url, relative) = resolve_git_url(repo, &containing)?;
    let reference = reference.unwrap_or("HEAD").to_owned();
    let path = path.unwrap_or(".").to_owned();
    let mut cache = settings.cache(false);
    let commit = cache.git.resolve(&url, &reference)?;
    let pubspec = cache.git.pubspec(&url, &path, &commit)?;
    let name = parse_yaml(&pubspec)
        .ok()
        .and_then(|node| node.get("name").as_str().map(str::to_owned))
        .ok_or_else(|| format!("Could not find a package name in the pubspec of {repo}."))?;
    let dependency = Dependency {
        name,
        description: Description::Git {
            url,
            relative,
            reference,
            path,
        },
        constraint: VersionSet::any(),
    };
    install_in_cache(settings, dependency, executables, overwrite, false)?;
    Ok(0)
}

/// `GlobalPackages.packageForConstraint`: the placeholder root that depends
/// only on the activated package.
fn root_package(dependency: Dependency, directory: PathBuf) -> WorkspacePackage {
    let sdk = VersionSet::parse(">=2.12.0").expect("valid constraint");
    let mut manifest = Manifest::empty(ROOT, Version::new(0, 0, 0));
    manifest.sdks = vec![(
        "dart".into(),
        SdkConstraint {
            original: sdk.clone(),
            effective: sdk,
        },
    )];
    manifest.dependencies = vec![dependency];
    manifest.resolution = Resolution::None;
    WorkspacePackage {
        manifest,
        shown: directory.display().to_string(),
        directory,
        children: Vec::new(),
    }
}

fn global_entrypoint(root: WorkspacePackage) -> Entrypoint {
    Entrypoint {
        workspace: vec![root],
        work: 0,
        location: ".".into(),
        root_dir: ".".into(),
        overrides_file: false,
    }
}

/// Resolves the placeholder root, with Pub's exit codes for a package that
/// does not exist or has no matching version.
fn resolve(
    entry: &Entrypoint,
    cache: &mut Cache,
    lock: &LockFile,
    name: &str,
) -> Result<super::solver::SolveResult, Failure> {
    let sdk = cache.sdk.clone();
    let solver = Solver::new(
        SolveType::Get,
        cache,
        &entry.workspace,
        lock,
        std::iter::empty(),
    );
    match solver.solve() {
        Ok(result) => Ok(result),
        Err(SolveError::Fatal(message)) => Err(message.into()),
        Err(SolveError::Failure {
            incompatibilities,
            root,
        }) => {
            let message = failure::explain(&incompatibilities, root, &sdk);
            let code = if failure::package_not_found(&incompatibilities, root) {
                exit::UNAVAILABLE
            } else if failure::no_versions_of(&incompatibilities, root, name) {
                exit::DATA
            } else {
                1
            };
            Err(Failure { code, message })
        }
    }
}

fn same_packages(lock: &LockFile, new: &NewLock) -> bool {
    lock.packages.len() == new.packages.len()
        && new
            .packages
            .iter()
            .all(|(name, id)| lock.packages.get(name) == Some(id))
}

/// `GlobalPackages._installInCache`.
pub(crate) fn install_in_cache(
    settings: &Settings,
    dependency: Dependency,
    executables: Option<Vec<String>>,
    overwrite: bool,
    silent: bool,
) -> Result<(), Failure> {
    let name = dependency.name.clone();
    let original = describe_active(settings, &name);
    let mut cache = settings.cache(false);
    let temp = temp_dir(&cache.root)?;
    let entry = global_entrypoint(root_package(dependency.clone(), temp.clone()));
    if !silent {
        println!("Resolving dependencies...");
    }
    let result = match resolve(&entry, &mut cache, &LockFile::default(), &name) {
        Ok(result) => result,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temp);
            return Err(error);
        }
    };
    println!("Downloading packages...");
    let new = new_lock(&entry, &mut cache, &result, &BTreeSet::new())?;
    let previous = original.clone().unwrap_or_default();
    let same = original.is_some() && same_packages(&previous, &new);
    let id = new
        .packages
        .get(&name)
        .cloned()
        .ok_or_else(|| format!("{name} is missing from its resolution"))?;
    let package_dir = global_dir(settings).join(&name);
    if same {
        let _ = std::fs::remove_dir_all(&temp);
        report::message(&format!(
            "The package {name} is already activated at newest available version.\nTo recompile executables, first run `dart pub global deactivate {name}`.\n"
        ));
    } else {
        if !silent {
            let overridden = BTreeSet::new();
            report::Report {
                kind: SolveType::Get,
                location: ".",
                root: &entry.root().manifest,
                overrides_file: false,
                overridden: &overridden,
                previous: &previous,
                new: &new,
                available: &result.available,
                dry_run: false,
                enforce_lockfile: false,
                quiet: false,
            }
            .show_changes(&cache);
        }
        output::write_lock(&new, &temp)?;
        output::write_config_file(&temp, &[], &new, &mut cache)?;
        let source = cache.directory(&id)?;
        precompile(
            settings,
            &id.name,
            &source,
            &temp.join(".dart_tool/package_config.json"),
            &temp.join("bin"),
            &package_dir.join("bin"),
            true,
        )?;
        if package_dir.exists() {
            let _ = std::fs::remove_dir_all(&package_dir);
        }
        std::fs::create_dir_all(global_dir(settings)).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &package_dir).map_err(|e| e.to_string())?;
    }
    let source = cache.directory(&id)?;
    update_bin_stubs(
        settings,
        &id,
        &source,
        &package_dir.join("bin"),
        executables.as_deref(),
        overwrite,
    )?;
    if !silent {
        println!("Activated {}.", format_package(&id));
    }
    Ok(())
}

/// `GlobalPackages.activatePath`.
pub(crate) fn activate_path(
    settings: &Settings,
    path: &str,
    executables: Option<Vec<String>>,
    overwrite: bool,
) -> Result<(), Failure> {
    let entry = Entrypoint::load(path, settings)?;
    acquire(&entry, settings, false, SolveType::Get, &Acquire::default())?;
    let root = entry.root();
    let name = root.manifest.name.clone();
    describe_active(settings, &name);
    let full =
        dartforge_elements::config::canonicalize(&root.directory).map_err(|e| e.to_string())?;
    let id = PackageId {
        name: name.clone(),
        version: root.manifest.version.clone(),
        description: Description::Path {
            path: full.clone(),
            relative: false,
        },
        resolved: None,
    };
    let cache = settings.cache(false);
    let temp = temp_dir(&cache.root)?;
    let lock = NewLock {
        packages: BTreeMap::from([(name.clone(), id.clone())]),
        sdks: BTreeMap::from([("dart".to_owned(), VersionSet::any())]),
        main: HashSet::from([name.clone()]),
        dev: HashSet::new(),
        overridden: HashSet::new(),
    };
    output::write_lock(&lock, &temp)?;
    let package_dir = global_dir(settings).join(&name);
    if package_dir.exists() {
        let _ = std::fs::remove_dir_all(&package_dir);
    }
    std::fs::create_dir_all(global_dir(settings)).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &package_dir).map_err(|e| e.to_string())?;
    update_bin_stubs(
        settings,
        &id,
        &full,
        &full.join(".dart_tool/pub/bin").join(&name),
        executables.as_deref(),
        overwrite,
    )?;
    println!("Activated {}.", format_package(&id));
    Ok(())
}

/// `pubspec.executables` of the package in `directory`: executable names
/// and their scripts, as written.
fn executables_of(directory: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(directory.join("pubspec.yaml")) else {
        return Vec::new();
    };
    let Ok(node) = parse_yaml(&text) else {
        return Vec::new();
    };
    let Node::Map(entries) = node.get("executables") else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|(name, script)| {
            let script = script.as_str().map_or_else(|| name.clone(), str::to_owned);
            (name.clone(), script)
        })
        .collect()
}

/// `Package.executablePaths`: the Dart files directly in `bin/`.
fn bin_scripts(directory: &Path) -> Vec<String> {
    let mut scripts: Vec<String> = list_dir(&directory.join("bin"))
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "dart"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    scripts.sort();
    scripts
}

/// Compiles every script of `bin/` natively, where Pub would build kernel
/// snapshots (`Entrypoint.precompileExecutables`). `staging` receives the
/// executables; `final_dir` is where they will live, for messages only.
fn precompile(
    settings: &Settings,
    package: &str,
    source: &Path,
    config: &Path,
    staging: &Path,
    _final_dir: &Path,
    clean: bool,
) -> Result<(), Failure> {
    let scripts = bin_scripts(source);
    if scripts.is_empty() {
        return Ok(());
    }
    println!("Building package executables...");
    if clean {
        let _ = std::fs::remove_dir_all(staging);
    }
    std::fs::create_dir_all(staging).map_err(|e| e.to_string())?;
    for script in scripts {
        match compile(
            settings,
            &source.join("bin").join(format!("{script}.dart")),
            config,
            &staging.join(executable_file(&script)),
            &[],
        ) {
            Ok(()) => println!("Built {package}:{script}."),
            Err(error) => eprintln!("Failed to build {package}:{script}:\n{error}"),
        }
    }
    Ok(())
}

fn executable_file(script: &str) -> String {
    if cfg!(windows) {
        format!("{script}.exe")
    } else {
        script.to_owned()
    }
}

/// `dartforge compile-native` of one script.
fn compile(
    settings: &Settings,
    script: &Path,
    config: &Path,
    output: &Path,
    extra: &[String],
) -> Result<(), String> {
    let compiler = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(compiler);
    command
        .arg("compile-native")
        .arg(script)
        .arg("-o")
        .arg(output)
        .arg("--packages")
        .arg(config)
        .arg("--optimize")
        .args(extra);
    if let Some(root) = &settings.sdk_root {
        command.arg("--sdk").arg(root.join("lib"));
    }
    let result = command.output().map_err(|e| e.to_string())?;
    if result.status.success() && output.is_file() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&result.stderr)
            .trim_end()
            .to_owned())
    }
}

/// `_binStubProperty`.
pub(crate) fn stub_property(contents: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}: ");
    let start = contents.find(&prefix)? + prefix.len();
    let value: String = contents[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    (!value.is_empty()).then_some(value)
}

fn stub_files(settings: &Settings) -> Vec<PathBuf> {
    list_dir(&bin_dir(settings))
        .into_iter()
        .filter(|p| p.is_file())
        .collect()
}

/// `_deleteBinStubs`.
fn delete_bin_stubs(settings: &Settings, package: &str) {
    for file in stub_files(settings) {
        let Ok(contents) = std::fs::read_to_string(&file) else {
            continue;
        };
        if stub_property(&contents, "Package").as_deref() == Some(package) {
            let _ = std::fs::remove_file(&file);
        }
    }
}

/// `_updateBinStubs`.
fn update_bin_stubs(
    settings: &Settings,
    id: &PackageId,
    source: &Path,
    executables_dir: &Path,
    executables: Option<&[String]>,
    overwrite: bool,
) -> Result<(), Failure> {
    delete_bin_stubs(settings, &id.name);
    let declared = executables_of(source);
    if executables.is_some_and(<[String]>::is_empty) || declared.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(bin_dir(settings)).map_err(|e| e.to_string())?;
    let mut names: Vec<&(String, String)> = declared.iter().collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    let mut installed = Vec::new();
    let mut collided: Vec<(String, String)> = Vec::new();
    for (executable, script) in names {
        if let Some(only) = executables
            && !only.contains(executable)
        {
            continue;
        }
        let previous = create_bin_stub(
            settings,
            id,
            executable,
            script,
            &executables_dir.join(executable_file(script)),
            overwrite,
        )?;
        if let Some(previous) = previous {
            collided.push((executable.clone(), previous));
            if !overwrite {
                continue;
            }
        }
        installed.push(executable.clone());
    }
    if !installed.is_empty() {
        let names: Vec<String> = installed.iter().map(|n| bold(n)).collect();
        println!(
            "Installed {} {}.",
            plural("executable", names.len()),
            super::cache::to_sentence(&names)
        );
    }
    for (command, package) in &collided {
        if overwrite {
            eprintln!(
                "Replaced {} previously installed from {}.",
                bold(command),
                bold(package)
            );
        } else {
            eprintln!(
                "Executable {} was already installed from {}.",
                bold(command),
                bold(package)
            );
        }
    }
    if !collided.is_empty() && !overwrite {
        eprintln!(
            "Deactivate the other package(s) or activate {} using --overwrite.",
            bold(&id.name)
        );
    }
    if let Some(only) = executables {
        let mut unknown: Vec<String> = only
            .iter()
            .filter(|e| !declared.iter().any(|(name, _)| name == *e))
            .cloned()
            .collect();
        unknown.sort();
        unknown.dedup();
        if !unknown.is_empty() {
            return Err(data(format!(
                "Unknown {} {}.",
                plural("executable", unknown.len()),
                super::cache::to_sentence(&unknown)
            )));
        }
    }
    let scripts = bin_scripts(source);
    for executable in &installed {
        let script = &declared
            .iter()
            .find(|(name, _)| name == executable)
            .expect("installed executables are declared")
            .1;
        if !scripts.contains(script) {
            eprintln!(
                "Warning: Executable \"{executable}\" runs \"bin/{script}.dart\", which was not found in {}.",
                bold(&id.name)
            );
        }
    }
    if let Some(first) = installed.first() {
        suggest_if_not_on_path(settings, first);
    }
    Ok(())
}

/// The stub Pub writes, running DartForge's native executable instead of a
/// snapshot, and `dartforge pub global run` instead of `dart pub global run`.
fn stub_text(
    id: &PackageId,
    executable: &str,
    script: &str,
    binary: &Path,
) -> Result<String, String> {
    let dartforge = std::env::current_exe().map_err(|e| e.to_string())?;
    let dartforge = dartforge.display();
    let binary = binary.display();
    let (name, version) = (&id.name, &id.version);
    Ok(if cfg!(windows) {
        format!(
            "@echo off\nrem This file was created by pub v3.6.2.\nrem Package: {name}\nrem Version: {version}\nrem Executable: {executable}\nrem Script: {script}\nif exist \"{binary}\" (\n  call \"{binary}\" %*\n) else (\n  call \"{dartforge}\" pub global run {name}:{script} %*\n)\n"
        )
    } else {
        format!(
            "#!/usr/bin/env sh\n# This file was created by pub v3.6.2.\n# Package: {name}\n# Version: {version}\n# Executable: {executable}\n# Script: {script}\nif [ -f {binary} ]; then\n  exec \"{binary}\" \"$@\"\nelse\n  exec \"{dartforge}\" pub global run {name}:{script} \"$@\"\nfi\n"
        )
    })
}

/// `_createBinStub`: returns the package that owned an existing stub.
fn create_bin_stub(
    settings: &Settings,
    id: &PackageId,
    executable: &str,
    script: &str,
    binary: &Path,
    overwrite: bool,
) -> Result<Option<String>, Failure> {
    let mut path = bin_dir(settings).join(executable);
    if cfg!(windows) {
        path.set_extension("bat");
    }
    let mut previous = None;
    if let Ok(contents) = std::fs::read_to_string(&path) {
        previous = stub_property(&contents, "Package");
        if previous.is_some() && !overwrite {
            return Ok(previous);
        }
    }
    let text = stub_text(id, executable, script, binary)?;
    let temp = temp_dir(&settings.cache_root)?;
    let staged = temp.join(path.file_name().expect("stub name"));
    let result = (|| -> Result<(), String> {
        std::fs::write(&staged, text).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&staged)
                .map_err(|e| e.to_string())?
                .permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            std::fs::set_permissions(&staged, permissions).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&staged, &path).map_err(|e| e.to_string())
    })();
    let _ = std::fs::remove_dir_all(&temp);
    result?;
    Ok(previous)
}

/// `_suggestIfNotOnPath`.
fn suggest_if_not_on_path(settings: &Settings, installed: &str) {
    let found = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {}", shell_escape(installed)))
        .output()
        .is_ok_and(|o| o.status.success());
    if found {
        return;
    }
    let mut directory = bin_dir(settings).display().to_string();
    if let Ok(home) = std::env::var("HOME")
        && let Some(rest) = directory.strip_prefix(&home)
    {
        directory = format!("$HOME{rest}");
    }
    let files = if cfg!(target_os = "macos") {
        "(.zshrc, .bashrc, .bash_profile, etc.)"
    } else {
        "(.bashrc, .bash_profile, .zshrc etc.)"
    };
    let yellow = |t: &str| {
        if std::io::stderr().is_terminal() {
            format!("\u{1b}[33m{t}\u{1b}[0m")
        } else {
            t.to_owned()
        }
    };
    eprintln!(
        "{} Pub installs executables into {}, which is not on your path.\nYou can fix that by adding this to your shell's config file {files}:\n\n  {}\n",
        yellow("Warning:"),
        bold(&directory),
        bold(&format!("export PATH=\"$PATH\":\"{directory}\""))
    );
}

/// `pub global deactivate`.
fn deactivate(args: &[String]) -> Result<u8, Failure> {
    let Some((arguments, settings)) = parse(
        args,
        &[],
        "deactivate",
        "Remove a previously activated package.",
        DEACTIVATE_USAGE,
    )?
    else {
        return Ok(0);
    };
    let Some(name) = arguments.rest.first() else {
        return Err(usage_error(
            "No package to deactivate given.",
            DEACTIVATE_USAGE,
            false,
        ));
    };
    if arguments.rest.len() > 1 {
        let unexpected: Vec<String> = arguments.rest[1..]
            .iter()
            .map(|a| format!("\"{a}\""))
            .collect();
        return Err(usage_error(
            &format!(
                "Unexpected {} {}.",
                plural("argument", unexpected.len()),
                super::cache::to_sentence(&unexpected)
            ),
            DEACTIVATE_USAGE,
            false,
        ));
    }
    deactivate_package(&settings, name)?;
    Ok(0)
}

/// `GlobalPackages.deactivate`.
pub(crate) fn deactivate_package(settings: &Settings, name: &str) -> Result<(), Failure> {
    let directory = global_dir(settings).join(name);
    let listed = list_dir(&global_dir(settings)).contains(&directory);
    if !listed || !directory.is_dir() {
        return Err(data(format!("No active package {}.", bold(name))));
    }
    delete_bin_stubs(settings, name);
    let active = load_active(settings, name)
        .ok_or_else(|| data(format!("No active package {}.", bold(name))))?;
    println!("Deactivated package {}.", format_package(&active.id));
    std::fs::remove_dir_all(&directory).map_err(|e| e.to_string())?;
    Ok(())
}

/// `pub global list`.
fn list(args: &[String]) -> Result<u8, Failure> {
    let Some((arguments, settings)) = parse(
        args,
        &[],
        "list",
        "List globally activated packages.",
        LIST_USAGE,
    )?
    else {
        return Ok(0);
    };
    if !arguments.rest.is_empty() {
        return Err(usage_error(
            "Command \"list\" does not take any arguments.",
            LIST_USAGE,
            false,
        ));
    }
    for id in active_packages(&settings)? {
        println!("{}", format_package(&id));
    }
    Ok(0)
}

/// `GlobalPackages.listActivePackages`: the activated ids by name.
pub(crate) fn active_packages(settings: &Settings) -> Result<Vec<PackageId>, Failure> {
    let mut ids = Vec::new();
    for entry in list_dir(&global_dir(settings)) {
        let name = entry
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let active = load_active(settings, &name).ok_or_else(|| {
            format!("Pubspec for activated package {name} didn't contain an entry for itself.")
        })?;
        ids.push(active.id);
    }
    ids.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ids)
}

/// The lockfile's SDK constraints must allow the running SDK
/// (`GlobalPackages.find`).
fn check_sdks(settings: &Settings, name: &str) -> Result<(), Failure> {
    let path = global_dir(settings).join(name).join("pubspec.lock");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(());
    };
    let Ok(node) = parse_yaml(&text) else {
        return Ok(());
    };
    let Node::Map(sdks) = node.get("sdks") else {
        return Ok(());
    };
    for (sdk, constraint) in sdks {
        let constraint = constraint
            .as_str()
            .and_then(|c| VersionSet::parse(c).ok())
            .unwrap_or_else(VersionSet::any);
        match sdk.as_str() {
            "dart" => {
                if !constraint.allows(&settings.sdk) {
                    return Err(data(format!(
                        "{} as globally activated doesn't support Dart {}.\n\ntry:\n`dart pub global activate {name}` to reactivate.\n",
                        bold(name),
                        settings.sdk
                    )));
                }
            }
            "flutter" | "fuchsia" => {
                let sdk_name = if sdk == "flutter" {
                    "Flutter"
                } else {
                    "Fuchsia"
                };
                return Err(data(format!(
                    "{} as globally activated requires the {sdk_name} SDK, which is unsupported for global executables.",
                    bold(name)
                )));
            }
            _ => {
                return Err(data(format!(
                    "{} as globally activated requires unknown SDK \"{name}\".",
                    bold(name)
                )));
            }
        }
    }
    Ok(())
}

/// `pub global run`.
fn run_executable(args: &[String]) -> Result<u8, Failure> {
    // Options are only read before the executable (`allowTrailingOptions`).
    let split = args
        .iter()
        .position(|a| !a.starts_with('-'))
        .unwrap_or(args.len());
    let (options, rest) = args.split_at(split);
    let specs = [
        flag("enable-asserts", None, true),
        flag("checked", Some('c'), true),
        option("enable-experiment", None),
        flag("sound-null-safety", None, true),
        option("mode", None),
    ];
    let Some((arguments, settings)) = parse(
        options,
        &specs,
        "run",
        "Run an executable from a globally activated package.",
        RUN_USAGE,
    )?
    else {
        return Ok(0);
    };
    let Some(target) = rest.first() else {
        return Err(usage_error(
            "Must specify an executable to run.",
            RUN_USAGE,
            false,
        ));
    };
    if arguments.flag("sound-null-safety").is_some() {
        return Err(data(
            "The --(no-)sound-null-safety flag is no longer supported.",
        ));
    }
    let (package, executable) = match target.split_once(':') {
        Some((package, executable)) => (package.to_owned(), executable.to_owned()),
        None => (target.clone(), target.clone()),
    };
    if Path::new(&executable).components().count() > 1 {
        return Err(usage_error(
            "Cannot run an executable in a subdirectory of a global package.",
            RUN_USAGE,
            false,
        ));
    }
    if arguments.option("mode").is_some() {
        eprintln!("The --mode flag is deprecated and has no effect.");
    }
    let program_args = &rest[1..];
    let Some(active) = load_active(&settings, &package) else {
        return Err(data(format!("No active package {}.", bold(&package))));
    };
    check_sdks(&settings, &package)?;
    let mut extra = Vec::new();
    if arguments.is("enable-asserts") || arguments.is("checked") {
        extra.push("--enable-asserts".to_owned());
    }
    for experiment in arguments.options("enable-experiment") {
        extra.push(format!("--enable-experiment={experiment}"));
    }
    let cache = settings.cache(false);
    let (source, built, config, global) = match &active.id.description {
        Description::Path { path, .. } => (
            path.clone(),
            path.join(".dart_tool/pub/bin").join(&package),
            path.join(".dart_tool/package_config.json"),
            false,
        ),
        _ => {
            let directory = global_dir(&settings).join(&package);
            (
                cache.directory(&active.id)?,
                directory.join("bin"),
                directory.join(".dart_tool/package_config.json"),
                true,
            )
        }
    };
    let script = source.join("bin").join(format!("{executable}.dart"));
    if !script.is_file() {
        let mut message = format!("Could not find {}", bold(&format!("bin/{executable}.dart")));
        if global {
            message.push_str(&format!(" in package {}", bold(&package)));
        }
        eprintln!("{message}.");
        return Ok(exit::NO_INPUT);
    }
    let binary = built.join(executable_file(&executable));
    // A path package can change at any time, so Pub rebuilds it on every run;
    // a cached one only when its executable is missing.
    let program = if !extra.is_empty() {
        let temp = temp_dir(&settings.cache_root)?;
        let binary = temp.join(executable_file(&executable));
        compile(&settings, &script, &config, &binary, &extra)?;
        Some((binary, temp))
    } else {
        if !binary.is_file() || !global {
            recompile(&settings, &active, &package, &source, global)?;
            std::fs::create_dir_all(&built).map_err(|e| e.to_string())?;
            if let Err(error) = compile(&settings, &script, &config, &binary, &[]) {
                return Err(format!("Failed to build {package}:{executable}:\n{error}").into());
            }
        }
        None
    };
    let to_run = program.as_ref().map_or(&binary, |(b, _)| b);
    let status = std::process::Command::new(to_run)
        .args(program_args)
        .status()
        .map_err(|e| format!("{}: {e}", to_run.display()))?;
    if let Some((_, temp)) = program {
        let _ = std::fs::remove_dir_all(temp);
    }
    Ok(status
        .code()
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(1))
}

/// The resolution check of `GlobalPackages.runExecutable`'s `recompile`:
/// the activation must still resolve to its lockfile.
fn recompile(
    settings: &Settings,
    active: &Active,
    package: &str,
    source: &Path,
    global: bool,
) -> Result<(), Failure> {
    let mut cache = settings.cache(false);
    let (entry, lock, location) = if global {
        let directory = global_dir(settings).join(package);
        let entry = global_entrypoint(root_package(active.id.to_range(), directory.clone()));
        (entry, active.lock.clone(), directory.display().to_string())
    } else {
        let entry = Entrypoint::load(&source.display().to_string(), settings)?;
        let lock = LockFile::read(&entry.root().directory, &settings.host).map_err(data)?;
        let location = entry.root().directory.display().to_string();
        (entry, lock, location)
    };
    println!("Resolving dependencies...");
    let result = match resolve(&entry, &mut cache, &lock, package) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{}", error.message);
            return Err(format!(
                "The package `{package}` as currently activated cannot resolve.\n\nTry reactivating the package.\n`dart pub global activate {package}`          \n"
            )
            .into());
        }
    };
    println!("Downloading packages...");
    let overridden = super::overridden_names(&entry);
    let new = new_lock(&entry, &mut cache, &result, &overridden)?;
    report::Report {
        kind: SolveType::Get,
        location: &location,
        root: &entry.root().manifest,
        overrides_file: entry.overrides_file,
        overridden: &overridden,
        previous: &lock,
        new: &new,
        available: &result.available,
        dry_run: true,
        enforce_lockfile: true,
        quiet: false,
    }
    .show(&cache);
    if !same_packages(&lock, &new) {
        let sdk_changed = new.packages.values().any(|id| {
            matches!(id.description, Description::Sdk(_))
                && lock.packages.get(&id.name).map(|old| &old.version) != Some(&id.version)
        });
        let what = if sdk_changed {
            "is not compatible with your current SDK"
        } else {
            "cannot resolve to the same set of dependencies"
        };
        return Err(data(format!(
            "The current activation of `{package}` {what}.\n\nTry reactivating the package.\n`dart pub global activate {package}`\n"
        )));
    }
    Ok(())
}

/// `GlobalPackages.repairActivatedPackages`: the names reactivated and the
/// names that failed.
pub(crate) fn repair_activated(settings: &Settings) -> Result<(Vec<String>, Vec<String>), Failure> {
    let mut executables: Vec<(String, Vec<String>)> = Vec::new();
    for entry in stub_files(settings) {
        let contents = std::fs::read_to_string(&entry).unwrap_or_default();
        let base = entry
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match (
            stub_property(&contents, "Package"),
            stub_property(&contents, "Executable"),
        ) {
            (Some(package), Some(executable)) => {
                match executables.iter_mut().find(|(p, _)| *p == package) {
                    Some((_, list)) => list.push(executable),
                    None => executables.push((package, vec![executable])),
                }
            }
            (package, _) => {
                let missing = if package.is_none() {
                    "Package"
                } else {
                    "Executable"
                };
                eprintln!("Error reading binstub for \"{base}\":\nNo '{missing}' property.");
                let _ = std::fs::remove_file(&entry);
            }
        }
    }
    let mut successes = Vec::new();
    let mut failures = Vec::new();
    for entry in list_dir(&global_dir(settings)) {
        let name = entry
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(active) = load_active(settings, &name) else {
            eprintln!("Failed to reactivate {}:\nNo lockfile.", bold(&name));
            failures.push(name);
            let _ = std::fs::remove_dir_all(&entry);
            continue;
        };
        println!("Reactivating {} {}...", bold(&name), active.id.version);
        let package_executables = executables
            .iter()
            .position(|(p, _)| *p == name)
            .map(|i| executables.remove(i).1)
            .unwrap_or_default();
        let result = match &active.id.description {
            Description::Path { path, .. } => activate_path(
                settings,
                &path.display().to_string(),
                Some(package_executables),
                true,
            ),
            _ => {
                let _ = std::fs::remove_dir_all(&entry);
                install_in_cache(
                    settings,
                    active.id.to_range(),
                    Some(package_executables),
                    true,
                    true,
                )
            }
        };
        match result {
            Ok(()) => successes.push(name),
            Err(error) => {
                let mut message =
                    format!("Failed to reactivate {} {}", bold(&name), active.id.version);
                if !matches!(active.id.description, Description::Hosted(_)) {
                    message.push_str(&format!(" from {}", active.id.description.source()));
                }
                eprintln!("{message}:\n{}", error.message);
                failures.push(name);
                let _ = std::fs::remove_dir_all(&entry);
            }
        }
    }
    if !executables.is_empty() {
        let mut message = String::from("Binstubs exist for non-activated packages:\n");
        for (package, names) in &executables {
            for executable in names {
                let _ = std::fs::remove_file(bin_dir(settings).join(executable));
            }
            message.push_str(&format!(
                "  From {}: {}\n",
                bold(package),
                super::cache::to_sentence(names)
            ));
        }
        eprint!("{message}");
    }
    Ok((successes, failures))
}
