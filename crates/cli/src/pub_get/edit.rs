//! Commands that change pubspecs: `add`, `remove`, `upgrade --major-versions`
//! and `--tighten`.
use super::{
    Acquire, Arguments, Entrypoint, Failure, Settings, acquire, data, flag,
    manifest::{Context, Dependency, Description, Manifest, Role},
    option,
    solver::SolveType,
    source::{LockFile, PackageId},
    tree::{Node, parse_yaml},
    usage,
    version::{Range, VersionSet},
    yaml_edit::{Editor, Value},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// For each workspace package (by index), the dependencies to rewrite:
/// old range to new range, in order.
pub(super) type ChangeSet = BTreeMap<usize, Vec<(Dependency, Dependency)>>;

/// Pub's `pubspecDescription`: the YAML that declares `range` in the
/// pubspec of `package`.
fn pubspec_description(
    range: &Dependency,
    settings: &Settings,
    package: &Path,
    language: (u64, u64),
) -> Value {
    let constraint = range.constraint.to_string();
    let description = match &range.description {
        Description::Hosted(url) if *url == settings.host => return Value::Scalar(constraint),
        Description::Hosted(url) => {
            if language >= (2, 15) {
                Value::Scalar(url.clone())
            } else {
                Value::Map(vec![
                    ("url".into(), Value::Scalar(url.clone())),
                    ("name".into(), Value::Scalar(range.name.clone())),
                ])
            }
        }
        Description::Path { path, relative } => Value::Scalar(if *relative {
            super::output::relative_path(path, package)
        } else {
            path.display().to_string()
        }),
        Description::Git {
            url,
            relative,
            reference,
            path,
        } => {
            let url = if *relative {
                url::Url::parse(url)
                    .ok()
                    .and_then(|u| u.to_file_path().ok())
                    .map(|p| super::output::relative_path(&p, package))
                    .unwrap_or_else(|| url.clone())
            } else {
                url.clone()
            };
            if reference == "HEAD" && path == "." {
                Value::Scalar(url)
            } else {
                let mut map = vec![("url".into(), Value::Scalar(url))];
                if reference != "HEAD" {
                    map.push(("ref".into(), Value::Scalar(reference.clone())));
                }
                if path != "." {
                    map.push(("path".into(), Value::Scalar(path.clone())));
                }
                Value::Map(map)
            }
        }
        Description::Sdk(sdk) => Value::Scalar(sdk.clone()),
        Description::Root(_) | Description::Unknown { .. } => Value::Scalar(constraint.clone()),
    };
    let mut map = vec![(range.description.source().to_owned(), description)];
    if !range.constraint.is_any() {
        map.push(("version".into(), Value::Scalar(constraint)));
    }
    Value::Map(map)
}

fn read_text(path: &Path) -> Result<String, Failure> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()).into())
}

/// Parses pubspec text of a workspace package in memory.
fn manifest_from_text(
    text: &str,
    directory: &Path,
    settings: &Settings,
) -> Result<Manifest, Failure> {
    let node = parse_yaml(text).map_err(data)?;
    let overrides = std::fs::read_to_string(directory.join("pubspec_overrides.yaml"))
        .ok()
        .and_then(|t| parse_yaml(&t).ok())
        .filter(|n| matches!(n, Node::Map(_)));
    let description = Description::Root(directory.to_path_buf());
    let context = Context {
        containing: &description,
        host: &settings.host,
        sdk: &settings.sdk,
    };
    Manifest::parse(&node, overrides.as_ref(), &context, Role::Workspace).map_err(data)
}

struct Update {
    dependency: Dependency,
    /// The constraint given on the command line, if any.
    constraint: Option<VersionSet>,
    dev: bool,
    is_override: bool,
}

fn parse_update(
    arg: &str,
    arguments: &Arguments,
    settings: &Settings,
    cwd: &Path,
) -> Result<Update, Failure> {
    let mut dev = arguments.is("dev");
    let mut is_override = false;
    let (prefix, rest) = match arg.split_once(':') {
        Some((p @ ("dev" | "override"), rest)) => (Some(p), rest),
        _ => (None, arg),
    };
    let (name, descriptor) = match rest.split_once(':') {
        Some((name, descriptor)) => (name, Some(descriptor)),
        None => (rest, None),
    };
    let valid_chars = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.');
    if !valid_chars {
        if let Some((p, _)) = arg.split_once(':')
            && descriptor.is_some()
            && !matches!(p, "dev" | "override")
        {
            return Err(usage(
                "The only allowed prefixes are \"dev:\" and \"override:\"",
            ));
        }
        return Err(usage(format!("Not a valid package name: \"{name}\"")));
    }
    match prefix {
        Some("dev") => {
            if arguments.is("dev") {
                return Err(usage("Cannot combine 'dev:' with --dev"));
            }
            dev = true;
        }
        Some("override") => {
            if arguments.is("dev") {
                return Err(usage("Cannot combine 'override:' with --dev"));
            }
            is_override = true;
        }
        _ => {}
    }
    if !super::manifest::valid_name(name) {
        return Err(usage(format!("Not a valid package name: \"{name}\"")));
    }
    if is_override && descriptor.is_none() {
        return Err(usage("A dependency override needs an explicit descriptor."));
    }
    let host = match arguments.option("hosted-url") {
        Some(url) => super::hosted::normalize_host(url).map_err(usage)?,
        None => settings.host.clone(),
    };
    let old_style = [
        "git-url",
        "git-ref",
        "git-path",
        "hosted-url",
        "path",
        "sdk",
    ]
    .iter()
    .any(|o| arguments.option(o).is_some())
        || arguments.is("dev");
    let root = Description::Root(cwd.to_path_buf());
    let (description, constraint) = if old_style {
        let constraint = match descriptor {
            None => None,
            Some(text) => Some(VersionSet::parse(text).map_err(|e| {
                if parse_yaml(text).is_ok_and(|n| matches!(n, Node::Map(_))) {
                    usage("--dev, --path, --sdk, --git-url, --git-path and --git-ref cannot be combined with a descriptor.")
                } else {
                    usage(format!("Invalid version constraint: {e}"))
                }
            })?),
        };
        let git = ["git-url", "git-ref", "git-path"]
            .iter()
            .any(|o| arguments.option(o).is_some());
        let description = if git {
            let url = arguments
                .option("git-url")
                .ok_or_else(|| usage("The `--git-url` is required for git dependencies."))?;
            let mut map = vec![("url".to_owned(), Node::String(url.into()))];
            if let Some(r) = arguments.option("git-ref") {
                map.push(("ref".into(), Node::String(r.into())));
            }
            if let Some(p) = arguments.option("git-path") {
                map.push(("path".into(), Node::String(p.into())));
            }
            dependency_description(
                name,
                &Node::Map(vec![("git".into(), Node::Map(map))]),
                settings,
                &root,
                &host,
            )?
        } else if let Some(path) = arguments.option("path") {
            Description::Path {
                path: super::manifest::normalize(&cwd.join(path)),
                relative: Path::new(path).is_relative(),
            }
        } else if let Some(sdk) = arguments.option("sdk") {
            Description::Sdk(sdk.into())
        } else {
            Description::Hosted(host.clone())
        };
        (description, constraint)
    } else {
        match descriptor {
            None => (Description::Hosted(host.clone()), None),
            Some(text) => match VersionSet::parse(text) {
                Ok(constraint) => (Description::Hosted(host.clone()), Some(constraint)),
                Err(_) => {
                    let node = parse_yaml(text)
                        .map_err(|e| usage(format!("Failed parsing package specification: {e}")))?;
                    let description = dependency_description(name, &node, settings, &root, &host)?;
                    let constraint = match &node {
                        Node::String(text) => Some(VersionSet::parse(text).map_err(|e| {
                            usage(format!("Failed parsing package specification: {e}"))
                        })?),
                        Node::Map(_) if node.contains("version") => Some(
                            VersionSet::parse(node.get("version").as_str().unwrap_or("any"))
                                .map_err(|e| {
                                    usage(format!("Failed parsing package specification: {e}"))
                                })?,
                        ),
                        _ => None,
                    };
                    let description = if matches!(node, Node::String(_)) {
                        Description::Hosted(host.clone())
                    } else {
                        description
                    };
                    (description, constraint)
                }
            },
        }
    };
    Ok(Update {
        dependency: Dependency {
            name: name.to_owned(),
            description,
            constraint: constraint.clone().unwrap_or_else(VersionSet::any),
        },
        constraint,
        dev,
        is_override,
    })
}

/// The description a dependency entry `node` declares, read like Pub reads
/// a dummy pubspec for `pub add`.
fn dependency_description(
    name: &str,
    node: &Node,
    settings: &Settings,
    root: &Description,
    host: &str,
) -> Result<Description, Failure> {
    let pubspec = Node::Map(vec![
        ("name".into(), Node::String("dartforge_add".into())),
        (
            "environment".into(),
            Node::Map(vec![("sdk".into(), Node::String(settings.sdk.to_string()))]),
        ),
        (
            "dependencies".into(),
            Node::Map(vec![(name.to_owned(), node.clone())]),
        ),
    ]);
    let context = Context {
        containing: root,
        host,
        sdk: &settings.sdk,
    };
    let manifest = Manifest::parse(&pubspec, None, &context, Role::Workspace)
        .map_err(|e| usage(format!("Failed parsing package specification: {e}")))?;
    Ok(manifest.dependencies[0].description.clone())
}

fn add_to_manifest(manifest: &mut Manifest, update: &Update) -> Result<(), Failure> {
    let name = &update.dependency.name;
    if update.is_override {
        manifest.overrides.retain(|d| d.name != *name);
        manifest.overrides.push(update.dependency.clone());
    } else if update.dev {
        if manifest.dev_dependency(name).is_some() {
            println!(
                "\"{name}\" is already in \"dev_dependencies\". Will try to update the constraint."
            );
            manifest.dev.retain(|d| d.name != *name);
        }
        if manifest.dependency(name).is_some() {
            return Err(data(format!(
                "\"{name}\" is already in \"dependencies\". Use \"pub remove {name}\" to remove it before adding it to \"dev_dependencies\""
            )));
        }
        manifest.dev.push(update.dependency.clone());
    } else {
        if manifest.dependency(name).is_some() {
            println!(
                "\"{name}\" is already in \"dependencies\". Will try to update the constraint."
            );
            manifest.dependencies.retain(|d| d.name != *name);
        }
        if manifest.dev_dependency(name).is_some() {
            println!(
                "\"{name}\" was found in dev_dependencies. Removing \"{name}\" and adding it to dependencies instead."
            );
            manifest.dev.retain(|d| d.name != *name);
        }
        manifest.dependencies.push(update.dependency.clone());
    }
    Ok(())
}

const ADD_HELP: &str = "Add dependencies to `pubspec.yaml`.

Usage: dart pub add [options] [<section>:]<package>[:descriptor] [<section>:]<package2>[:descriptor] ...]
-h, --help               Print this usage information.
    --[no-]offline       Use cached packages instead of accessing the network.
-n, --dry-run            Report what dependencies would change but don't change any.
    --[no-]precompile    Build executables in immediate dependencies.
-C, --directory=<dir>    Run this in the directory <dir>.";

pub(super) fn add(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("dev", Some('d'), false),
        option("git-url", None),
        option("git-ref", None),
        option("git-path", None),
        option("hosted-url", None),
        option("path", None),
        option("sdk", None),
        flag("offline", None, true),
        flag("dry-run", Some('n'), false),
        flag("precompile", None, true),
        option("directory", Some('C')),
        flag("example", None, true),
    ];
    let mut all: Vec<super::Spec> = specs.into_iter().collect();
    all.push(option("sdk-version", None));
    all.push(flag("help", Some('h'), false));
    let arguments = Arguments::parse(args, &all, "add")?;
    if arguments.is("help") {
        println!("{ADD_HELP}");
        return Ok(0);
    }
    // `--sdk` is Pub's SDK source here; DartForge's SDK selection comes
    // from the environment.
    let settings = Settings::discover(None, arguments.option("sdk-version"))?;
    if arguments.rest.len() > 1 {
        if arguments.option("git-url").is_some() {
            return Err(usage(
                "--git-url cannot be used with multiple packages.\nSpecify multiple git packages with descriptors.",
            ));
        } else if arguments.option("path").is_some() {
            return Err(usage(
                "--path cannot be used with multiple packages.\nSpecify multiple path packages with descriptors.",
            ));
        } else if arguments.option("sdk").is_some() {
            return Err(usage(
                "--sdk cannot be used with multiple packages.\nSpecify multiple sdk packages with descriptors.",
            ));
        }
    }
    if arguments.rest.is_empty() {
        return Err(usage("Must specify at least one package to be added."));
    }
    let directory = arguments.option("directory").unwrap_or(".").to_owned();
    let offline = arguments.is("offline");
    let dry_run = arguments.is("dry-run");
    let entry = Entrypoint::load(&directory, &settings)?;
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let updates = arguments
        .rest
        .iter()
        .map(|arg| parse_update(arg, &arguments, &settings, &cwd))
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolution = entry.clone();
    let work = entry.work;
    for update in &updates {
        add_to_manifest(&mut resolution.workspace[work].manifest, update)?;
    }
    let mut cache = settings.cache(offline);
    let result = super::solve(
        &resolution,
        &mut cache,
        SolveType::Upgrade,
        &LockFile::default(),
        &[],
    )
    .map_err(|f| data(f.message))?;
    let resolved =
        |name: &str| -> Option<&PackageId> { result.packages.iter().find(|p| p.name == name) };
    for update in &updates {
        let name = &update.dependency.name;
        let id = resolved(name).ok_or_else(|| format!("{name} was not resolved"))?;
        if let Some(constraint) = &update.constraint
            && !constraint.allows(&id.version)
            && !resolution.workspace[work].manifest.overrides.is_empty()
        {
            return Err(data(format!(
                "\"{name}\" resolved to \"{}\" which does not satisfy constraint \"{constraint}\". This could be caused by \"dependency_overrides\".",
                id.version
            )));
        }
    }
    let package = &entry.workspace[work];
    let pubspec_path = package.directory.join("pubspec.yaml");
    let mut editor = Editor::new(&read_text(&pubspec_path)?);
    let language = package.manifest.language_version();
    for update in &updates {
        let name = &update.dependency.name;
        let section = if update.dev {
            "dev_dependencies"
        } else if update.is_override {
            "dependency_overrides"
        } else {
            "dependencies"
        };
        let id = resolved(name).expect("checked above");
        let constraint = update.constraint.clone().unwrap_or_else(|| {
            if matches!(update.dependency.description, Description::Hosted(_)) {
                VersionSet::compatible_with(&id.version)
            } else {
                VersionSet::any()
            }
        });
        let range = Dependency {
            constraint,
            ..update.dependency.clone()
        };
        editor.set(
            section,
            name,
            &pubspec_description(&range, &settings, &package.directory, language),
        );
        if !update.dev && !update.is_override && editor.contains("dev_dependencies", name) {
            editor.remove("dev_dependencies", name);
        }
    }
    let text = editor.text();
    if !dry_run {
        std::fs::write(&pubspec_path, &text)
            .map_err(|e| format!("{}: {e}", pubspec_path.display()))?;
    }
    let mut updated = entry.clone();
    updated.workspace[work].manifest = manifest_from_text(&text, &package.directory, &settings)?;
    acquire(
        &updated,
        &settings,
        offline,
        SolveType::Get,
        &Acquire {
            dry_run,
            ..Acquire::default()
        },
    )?;
    if !dry_run && arguments.flag("example").unwrap_or(true) {
        let example = updated.root().directory.join("example");
        if example.join("pubspec.yaml").is_file() {
            let example = Entrypoint::load(&updated.example_dir(), &settings)?;
            acquire(
                &example,
                &settings,
                offline,
                SolveType::Get,
                &Acquire {
                    summary_only: true,
                    ..Acquire::default()
                },
            )?;
        }
    }
    if offline {
        eprintln!(
            "Warning: Packages added when offline may not resolve to the latest compatible version available."
        );
    }
    Ok(0)
}

const REMOVE_HELP: &str = "Removes dependencies from `pubspec.yaml`.

Usage: dart pub remove <package1> [<package2>...]
-h, --help               Print this usage information.
    --[no-]offline       Use cached packages instead of accessing the network.
-n, --dry-run            Report what dependencies would change but don't change any.
    --[no-]precompile    Precompile executables in immediate dependencies.
-C, --directory=<dir>    Run this in the directory <dir>.";

pub(super) fn remove(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("offline", None, true),
        flag("dry-run", Some('n'), false),
        flag("precompile", None, true),
        option("directory", Some('C')),
        flag("example", None, true),
        option("sdk", None),
        option("sdk-version", None),
        flag("help", Some('h'), false),
    ];
    let arguments = Arguments::parse(args, &specs, "remove")?;
    if arguments.is("help") {
        println!("{REMOVE_HELP}");
        return Ok(0);
    }
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    if arguments.rest.is_empty() {
        return Err(usage("Must specify a package to be removed."));
    }
    let directory = arguments.option("directory").unwrap_or(".").to_owned();
    let offline = arguments.is("offline");
    let dry_run = arguments.is("dry-run");
    let entry = Entrypoint::load(&directory, &settings)?;
    let mut targets: Vec<(String, bool)> = Vec::new();
    for descriptor in &arguments.rest {
        let target = match descriptor.strip_prefix("override:") {
            Some(name) => (name.to_owned(), true),
            None => (descriptor.clone(), false),
        };
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    let work = entry.work;
    let package = &entry.workspace[work];
    let pubspec_path = package.directory.join("pubspec.yaml");
    if !dry_run {
        let mut editor = Editor::new(&read_text(&pubspec_path)?);
        for (name, from_override) in &targets {
            let sections: &[&str] = if *from_override {
                &["dependency_overrides"]
            } else {
                &["dependencies", "dev_dependencies"]
            };
            let mut found = false;
            for section in sections {
                if editor.contains(section, name) {
                    editor.remove(section, name);
                    found = true;
                }
            }
            if !found {
                let cwd = std::env::current_dir().unwrap_or_default();
                eprintln!(
                    "Package \"{name}\" was not found in {}!",
                    super::output::relative_path(
                        &pubspec_path,
                        &cwd.canonicalize().unwrap_or(cwd.clone())
                    )
                );
            }
        }
        std::fs::write(&pubspec_path, editor.text())
            .map_err(|e| format!("{}: {e}", pubspec_path.display()))?;
    }
    let mut updated = entry.clone();
    let manifest = &mut updated.workspace[work].manifest;
    for (name, from_override) in &targets {
        if *from_override {
            manifest.overrides.retain(|d| d.name != *name);
        } else {
            manifest.dependencies.retain(|d| d.name != *name);
            manifest.dev.retain(|d| d.name != *name);
        }
    }
    acquire(
        &updated,
        &settings,
        offline,
        SolveType::Get,
        &Acquire {
            dry_run,
            ..Acquire::default()
        },
    )?;
    if !dry_run && arguments.flag("example").unwrap_or(true) {
        let example = updated.root().directory.join("example");
        if example.join("pubspec.yaml").is_file() {
            let example = Entrypoint::load(&updated.example_dir(), &settings)?;
            acquire(
                &example,
                &settings,
                offline,
                SolveType::Get,
                &Acquire {
                    summary_only: true,
                    ..Acquire::default()
                },
            )?;
        }
    }
    Ok(0)
}

/// `upgrade [packages]`, with `--unlock-transitive` adding the transitive
/// dependencies of the named packages (from the current lockfile).
pub(super) fn packages_to_upgrade(
    entry: &Entrypoint,
    settings: &Settings,
    arguments: &Arguments,
) -> Result<Vec<String>, Failure> {
    if !arguments.is("unlock-transitive") {
        return Ok(arguments.rest.clone());
    }
    let lock = LockFile::read(&entry.root().directory, &settings.host).map_err(data)?;
    let mut cache = settings.cache(arguments.is("offline"));
    let mut result: Vec<String> = Vec::new();
    for package in &arguments.rest {
        let mut stack = vec![package.clone()];
        while let Some(name) = stack.pop() {
            if result.contains(&name) {
                continue;
            }
            result.push(name.clone());
            let dependencies: Vec<String> =
                if let Some(root) = entry.workspace.iter().find(|p| p.manifest.name == name) {
                    root.manifest
                        .dependencies
                        .iter()
                        .chain(&root.manifest.dev)
                        .map(|d| d.name.clone())
                        .collect()
                } else if let Some(id) = lock.packages.get(&name) {
                    cache
                        .describe(id)
                        .map(|m| m.dependencies.iter().map(|d| d.name.clone()).collect())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
            stack.extend(dependencies.into_iter().rev());
        }
    }
    Ok(result)
}

/// `stripUpperBound`.
fn strip_upper_bound(constraint: &VersionSet) -> VersionSet {
    match constraint.ranges().first() {
        None => VersionSet::empty(),
        Some(range) => VersionSet::from_range(Range {
            min: range.min.clone(),
            include_min: range.include_min,
            max: None,
            include_max: false,
        }),
    }
}

/// `upgrade --major-versions`.
pub(super) fn upgrade_major_versions(
    entry: &Entrypoint,
    settings: &Settings,
    offline: bool,
    arguments: &Arguments,
    unlock: &[String],
) -> Result<(), Failure> {
    let mut direct: Vec<String> = Vec::new();
    for package in &entry.workspace {
        for dependency in package
            .manifest
            .dependencies
            .iter()
            .chain(&package.manifest.dev)
        {
            if !direct.contains(&dependency.name) {
                direct.push(dependency.name.clone());
            }
        }
    }
    let to_upgrade: Vec<String> = if unlock.is_empty() {
        direct.clone()
    } else {
        unlock.to_vec()
    };
    let not_direct: Vec<&String> = to_upgrade.iter().filter(|n| !direct.contains(n)).collect();
    if arguments.rest.iter().any(|r| not_direct.contains(&r)) {
        return Err(usage(format!(
            "Dependencies specified in `dart pub upgrade --major-versions <dependencies>` must\nbe direct 'dependencies' or 'dev_dependencies', following packages are not:\n - {}\n",
            not_direct
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n - ")
        )));
    }
    let mut stripped = entry.clone();
    for package in &mut stripped.workspace {
        for dependency in package
            .manifest
            .dependencies
            .iter_mut()
            .chain(package.manifest.dev.iter_mut())
        {
            dependency.constraint = strip_upper_bound(&dependency.constraint);
        }
    }
    let mut cache = settings.cache(offline);
    let result = super::solve(
        &stripped,
        &mut cache,
        SolveType::Upgrade,
        &LockFile::default(),
        &[],
    )?;
    let mut overridden_deps: Vec<String> = Vec::new();
    let mut changes: ChangeSet = ChangeSet::new();
    for (index, package) in entry.workspace.iter().enumerate() {
        for dependency in package
            .manifest
            .dependencies
            .iter()
            .chain(&package.manifest.dev)
        {
            if !matches!(dependency.description, Description::Hosted(_)) {
                continue;
            }
            let Some(resolved) = result.packages.iter().find(|p| p.name == dependency.name) else {
                continue;
            };
            if !to_upgrade.contains(&dependency.name) {
                continue;
            }
            if entry
                .root()
                .manifest
                .override_of(&dependency.name)
                .is_some()
            {
                overridden_deps.push(dependency.name.clone());
                continue;
            }
            if dependency.constraint.allows(&resolved.version) {
                continue;
            }
            changes.entry(index).or_default().push((
                dependency.clone(),
                Dependency {
                    constraint: VersionSet::compatible_with(&resolved.version),
                    ..dependency.clone()
                },
            ));
        }
    }
    let dry_run = arguments.is("dry-run");
    if arguments.is("tighten") {
        let applied = with_changes(entry, &changes);
        let mut cache = settings.cache(offline);
        let result = super::solve(
            &applied,
            &mut cache,
            SolveType::Upgrade,
            &LockFile::default(),
            &[],
        )?;
        changes = tighten(entry, settings, unlock, &changes, Some(&result.packages))?;
    }
    let kind = if unlock.is_empty() {
        SolveType::Upgrade
    } else {
        SolveType::Get
    };
    apply_changes(entry, &changes, dry_run)?;
    let updated = with_changes(entry, &changes);
    acquire(
        &updated,
        settings,
        offline,
        kind,
        &Acquire {
            unlock: unlock.to_vec(),
            dry_run,
            ..Acquire::default()
        },
    )?;
    let overrides: Vec<&String> = to_upgrade
        .iter()
        .filter(|n| {
            entry
                .workspace
                .iter()
                .any(|p| p.manifest.override_of(n).is_some())
        })
        .collect();
    if !overrides.is_empty() {
        eprintln!(
            "Warning: dependency_overrides prevents upgrades for: {}",
            overrides
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    super::commands::offline_warning(offline);
    Ok(())
}

fn with_changes(entry: &Entrypoint, changes: &ChangeSet) -> Entrypoint {
    let mut updated = entry.clone();
    for (index, list) in changes {
        let manifest = &mut updated.workspace[*index].manifest;
        for (_, to) in list {
            if let Some(dependency) = manifest.dependencies.iter_mut().find(|d| d.name == to.name) {
                *dependency = to.clone();
            } else if let Some(dependency) = manifest.dev.iter_mut().find(|d| d.name == to.name) {
                *dependency = to.clone();
            }
        }
    }
    updated
}

/// `Entrypoint.tighten`.
pub(super) fn tighten(
    entry: &Entrypoint,
    settings: &Settings,
    packages: &[String],
    existing: &ChangeSet,
    versions: Option<&[PackageId]>,
) -> Result<ChangeSet, Failure> {
    let lock = LockFile::read(&entry.root().directory, &settings.host).map_err(data)?;
    let mut result = existing.clone();
    for (index, package) in entry.workspace.iter().enumerate() {
        let ranges: Vec<&Dependency> = if packages.is_empty() {
            package
                .manifest
                .dependencies
                .iter()
                .chain(&package.manifest.dev)
                .collect()
        } else {
            packages
                .iter()
                .filter_map(|n| {
                    package
                        .manifest
                        .dependency(n)
                        .or_else(|| package.manifest.dev_dependency(n))
                })
                .collect()
        };
        for range in ranges {
            let list = result.entry(index).or_default();
            let constraint = list
                .iter()
                .find(|(from, _)| from == range)
                .map(|(_, to)| to.constraint.clone())
                .unwrap_or_else(|| range.constraint.clone());
            let resolved = versions
                .and_then(|v| {
                    v.iter()
                        .find(|p| p.name == range.name)
                        .map(|p| p.version.clone())
                })
                .or_else(|| lock.packages.get(&range.name).map(|p| p.version.clone()))
                .or_else(|| {
                    entry
                        .workspace
                        .iter()
                        .find(|p| p.manifest.name == range.name)
                        .map(|p| p.manifest.version.clone())
                });
            let Some(resolved) = resolved else {
                continue;
            };
            let new_constraint =
                if matches!(range.description, Description::Hosted(_)) && constraint.is_any() {
                    Some(VersionSet::compatible_with(&resolved))
                } else if let [r] = constraint.ranges()
                    && let Some(min) = &r.min
                    && *min < resolved
                {
                    Some(
                        VersionSet::from_range(Range::user(
                            Some(resolved.clone()),
                            true,
                            r.max.clone(),
                            r.include_max,
                        ))
                        .as_compatible_with_if_possible(),
                    )
                } else {
                    None
                };
            if let Some(new_constraint) = new_constraint {
                let to = Dependency {
                    constraint: new_constraint,
                    ..range.clone()
                };
                match list.iter_mut().find(|(from, _)| from == range) {
                    Some(entry) => entry.1 = to,
                    None => list.push((range.clone(), to)),
                }
            }
        }
    }
    result.retain(|_, list| !list.is_empty());
    Ok(result)
}

/// `Entrypoint.applyChanges`: rewrites the pubspecs and prints the summary.
pub(super) fn apply_changes(
    entry: &Entrypoint,
    changes: &ChangeSet,
    dry_run: bool,
) -> Result<(), Failure> {
    let settings = Settings::discover(None, None)?;
    if !dry_run {
        for (index, list) in changes {
            if list.is_empty() {
                continue;
            }
            let package = &entry.workspace[*index];
            let path = package.directory.join("pubspec.yaml");
            let mut editor = Editor::new(&read_text(&path)?);
            let language = package.manifest.language_version();
            for (_, to) in list {
                let section = if package.manifest.dependency(&to.name).is_some() {
                    "dependencies"
                } else {
                    "dev_dependencies"
                };
                editor.set(
                    section,
                    &to.name,
                    &pubspec_description(to, &settings, &package.directory, language),
                );
            }
            std::fs::write(&path, editor.text()).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    let plural = |n: usize| if n == 1 { "constraint" } else { "constraints" };
    if entry.root().children.is_empty() {
        let list = changes.get(&0).map(Vec::as_slice).unwrap_or_default();
        if list.is_empty() {
            let would = if dry_run { "would be made to" } else { "to" };
            println!("\nNo changes {would} pubspec.yaml!");
        } else {
            let changed = if dry_run { "Would change" } else { "Changed" };
            println!(
                "\n{changed} {} {} in pubspec.yaml:",
                list.len(),
                plural(list.len())
            );
            for (from, to) in list {
                println!("  {}: {} -> {}", from.name, from.constraint, to.constraint);
            }
        }
    } else {
        if changes.is_empty() {
            let would = if dry_run { "would be made to" } else { "to" };
            println!("\nNo changes {would} any pubspec.yaml!");
        }
        for (index, package) in entry.workspace.iter().enumerate() {
            let Some(list) = changes.get(&index).filter(|l| !l.is_empty()) else {
                continue;
            };
            let changed = if dry_run { "Would change" } else { "Changed" };
            println!(
                "\n{changed} {} {} in {}:",
                list.len(),
                plural(list.len()),
                package.directory.join("pubspec.yaml").display()
            );
            for (from, to) in list {
                println!("  {}: {} -> {}", from.name, from.constraint, to.constraint);
            }
        }
    }
    Ok(())
}
