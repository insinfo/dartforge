//! The reporting commands: `deps` and `outdated`.
use super::{
    Acquire, Arguments, Entrypoint, Failure, Settings, acquire, data, flag,
    manifest::Manifest,
    option,
    solver::SolveType,
    source::{Cache, LockFile},
    usage,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    rc::Rc,
};

/// One package of the resolved graph.
pub(super) struct GraphPackage {
    pub manifest: Rc<Manifest>,
    pub directory: PathBuf,
    pub root: bool,
}

/// Pub's `PackageGraph`: the workspace and every locked package.
pub(super) struct Graph {
    pub packages: BTreeMap<String, GraphPackage>,
}

impl Graph {
    /// Loads the graph, resolving first when the lockfile is missing or
    /// older than a pubspec (Pub's `ensureUpToDate`).
    pub fn load(
        entry: &Entrypoint,
        settings: &Settings,
        cache: &mut Cache,
    ) -> Result<Self, Failure> {
        let root = &entry.root().directory;
        let lock_path = root.join("pubspec.lock");
        let config = root.join(".dart_tool/package_config.json");
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let stale = match (modified(&lock_path), modified(&config)) {
            (Some(lock), Some(config_time)) => entry.workspace.iter().any(|p| {
                ["pubspec.yaml", "pubspec_overrides.yaml"].iter().any(|f| {
                    modified(&p.directory.join(f)).is_some_and(|m| m > lock || m > config_time)
                })
            }),
            _ => true,
        };
        if stale {
            acquire(
                entry,
                settings,
                cache.offline,
                SolveType::Get,
                &Acquire {
                    summary_only: true,
                    ..Acquire::default()
                },
            )?;
        }
        let lock = LockFile::read(root, &settings.host).map_err(data)?;
        let mut packages = BTreeMap::new();
        for (name, id) in &lock.packages {
            let manifest = cache.describe(id).map_err(|e| match e {
                super::source::SourceError::NotFound(m)
                | super::source::SourceError::Invalid(m)
                | super::source::SourceError::Fatal(m) => Failure::from(m),
            })?;
            let directory = cache.directory(id)?;
            packages.insert(
                name.clone(),
                GraphPackage {
                    manifest,
                    directory,
                    root: false,
                },
            );
        }
        for package in &entry.workspace {
            packages.insert(
                package.manifest.name.clone(),
                GraphPackage {
                    manifest: Rc::new(package.manifest.clone()),
                    directory: package.directory.clone(),
                    root: true,
                },
            );
        }
        Ok(Self { packages })
    }

    fn get(&self, name: &str) -> Result<&GraphPackage, Failure> {
        self.packages.get(name).ok_or_else(|| {
            data("The pubspec.yaml file has changed since the pubspec.lock file was generated, please run \"dart pub get\" again.")
        })
    }

    /// `PackageGraph.transitiveDependencies`, the package included.
    pub fn transitive(&self, name: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![name.to_owned()];
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(package) = self.packages.get(&current) {
                let next: Vec<String> = if package.root {
                    immediate(&package.manifest)
                } else {
                    package
                        .manifest
                        .dependencies
                        .iter()
                        .map(|d| d.name.clone())
                        .collect()
                };
                stack.extend(next);
            }
        }
        seen
    }
}

/// `Package.immediateDependencies`: dependencies, dev dependencies and
/// overrides, each name once in first position.
fn immediate(manifest: &Manifest) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for dependency in manifest
        .dependencies
        .iter()
        .chain(&manifest.dev)
        .chain(&manifest.overrides)
    {
        if !names.contains(&dependency.name) {
            names.push(dependency.name.clone());
        }
    }
    names
}

fn executables(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory.join("bin"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_file())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".dart").map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

fn label(package: &GraphPackage, name: &str) -> String {
    format!("{name} {}", package.manifest.version)
}

/// `ascii_tree.fromMap`.
#[derive(Default)]
struct Tree(BTreeMap<String, Tree>);

fn draw(
    output: &mut String,
    prefix: &str,
    name: Option<&str>,
    children: &Tree,
    is_last: bool,
    depth: usize,
) {
    if let Some(name) = name {
        output.push_str(prefix);
        if depth > 1 {
            output.push_str(if is_last { "└── " } else { "├── " });
        }
        output.push_str(name);
        output.push('\n');
    }
    let child_prefix = if depth <= 1 {
        ""
    } else if is_last {
        "    "
    } else {
        "│   "
    };
    let count = children.0.len();
    for (index, (child, grandchildren)) in children.0.iter().enumerate() {
        draw(
            output,
            &format!("{prefix}{child_prefix}"),
            Some(child),
            grandchildren,
            index + 1 == count,
            depth + 1,
        );
    }
}

const DEPS_HELP: &str = "Print package dependencies.

Usage: dart pub deps [arguments...]
-h, --help               Print this usage information.
-s, --style              How output should be displayed.
                         [compact, tree (default), list]
    --[no-]dev           Whether to include dev dependencies.
                         (defaults to on)
    --executables        List all available executables.
    --json               Output dependency information in a json format.
-C, --directory=<dir>    Run this in the directory <dir>.";

pub(super) fn deps(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        option("style", Some('s')),
        flag("dev", None, true),
        flag("executables", None, false),
        flag("json", None, false),
        option("directory", Some('C')),
        option("sdk", None),
        option("sdk-version", None),
        flag("help", Some('h'), false),
    ];
    let arguments = Arguments::parse(args, &specs, "deps")?;
    if arguments.is("help") {
        println!("{DEPS_HELP}");
        return Ok(0);
    }
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    let style = arguments.option("style").unwrap_or("tree");
    if !matches!(style, "compact" | "tree" | "list") {
        return Err(usage(format!(
            "\"{style}\" is not an allowed value for option \"style\".\n\n{DEPS_HELP}"
        )));
    }
    let include_dev = arguments.flag("dev").unwrap_or(true);
    let entry = Entrypoint::load(arguments.option("directory").unwrap_or("."), &settings)?;
    let mut cache = settings.cache(false);
    let mut output = String::new();
    if arguments.is("json") {
        if arguments.flag("dev").is_some() {
            return Err(usage(
                "Cannot combine --json and --dev.\nThe json output contains the dependency type in the output.",
            ));
        }
        if arguments.is("executables") {
            return Err(usage(
                "Cannot combine --json and --executables.\nThe json output always lists available executables.",
            ));
        }
        if arguments.option("style").is_some() {
            return Err(usage("Cannot combine --json and --style."));
        }
        let graph = Graph::load(&entry, &settings, &mut cache)?;
        let lock = LockFile::read(&entry.root().directory, &settings.host).map_err(data)?;
        let workspace: Vec<String> = entry
            .workspace
            .iter()
            .map(|p| p.manifest.name.clone())
            .collect();
        let mut to_visit = workspace.clone();
        let mut visited: Vec<String> = Vec::new();
        let mut packages = Vec::new();
        let root = &entry.root().manifest;
        while let Some(current) = to_visit.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.push(current.clone());
            let package = graph.get(&current)?;
            let is_root = workspace.contains(&current);
            let next: Vec<String> = if is_root {
                immediate(&package.manifest)
            } else {
                package
                    .manifest
                    .dependencies
                    .iter()
                    .map(|d| d.name.clone())
                    .collect()
            };
            let kind = if is_root {
                "root"
            } else if root.dependency(&current).is_some() {
                "direct"
            } else if root.dev_dependency(&current).is_some() {
                "dev"
            } else {
                "transitive"
            };
            let source = lock
                .packages
                .get(&current)
                .map(|id| id.description.source().to_owned())
                .unwrap_or_else(|| "root".into());
            let mut object = serde_json::Map::new();
            object.insert("name".into(), current.clone().into());
            object.insert(
                "version".into(),
                package.manifest.version.to_string().into(),
            );
            object.insert("kind".into(), kind.into());
            object.insert("source".into(), source.into());
            object.insert("dependencies".into(), next.clone().into());
            object.insert(
                "directDependencies".into(),
                package
                    .manifest
                    .dependencies
                    .iter()
                    .map(|d| d.name.clone())
                    .collect::<Vec<_>>()
                    .into(),
            );
            if is_root {
                object.insert(
                    "devDependencies".into(),
                    package
                        .manifest
                        .dev
                        .iter()
                        .map(|d| d.name.clone())
                        .collect::<Vec<_>>()
                        .into(),
                );
            }
            packages.push(object);
            to_visit.extend(next);
        }
        let mut names = vec![root.name.clone()];
        names.extend(immediate(root));
        let mut listed = Vec::new();
        for name in names {
            let package = graph.get(&name)?;
            for executable in executables(&package.directory) {
                listed.push(if name == root.name {
                    format!(":{executable}")
                } else if executable == name {
                    executable
                } else {
                    format!("{name}:{executable}")
                });
            }
        }
        // `JsonEncoder.withIndent('  ')` with Pub's key order.
        let value = PrettyJson::Object(vec![
            ("root".into(), PrettyJson::String(root.name.clone())),
            (
                "packages".into(),
                PrettyJson::Array(
                    packages
                        .into_iter()
                        .map(|o| PrettyJson::from_map(&o))
                        .collect(),
                ),
            ),
            (
                "sdks".into(),
                PrettyJson::Array(vec![PrettyJson::Object(vec![
                    ("name".into(), PrettyJson::String("Dart".into())),
                    (
                        "version".into(),
                        PrettyJson::String(settings.sdk.to_string()),
                    ),
                ])]),
            ),
            (
                "executables".into(),
                PrettyJson::Array(listed.into_iter().map(PrettyJson::String).collect()),
            ),
        ]);
        output.push_str(&value.render(0));
        output.push('\n');
        super::report::message(&output);
        return Ok(0);
    }
    let graph = Graph::load(&entry, &settings, &mut cache)?;
    if arguments.is("executables") {
        let mut seen: Vec<String> = Vec::new();
        for root in &entry.workspace {
            let mut names = vec![root.manifest.name.clone()];
            if include_dev {
                names.extend(immediate(&root.manifest));
            } else {
                names.extend(root.manifest.dependencies.iter().map(|d| d.name.clone()));
            }
            for name in names {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name.clone());
                let package = graph.get(&name)?;
                let mut list = executables(&package.directory);
                if list.is_empty() {
                    continue;
                }
                if list.len() == 1 {
                    output.push_str(&if list[0] != name {
                        format!("{name}:{}\n", list[0])
                    } else {
                        format!("{}\n", list[0])
                    });
                } else {
                    list.sort_by(|a, b| {
                        if *a == name {
                            std::cmp::Ordering::Less
                        } else if *b == name {
                            std::cmp::Ordering::Greater
                        } else {
                            a.cmp(b)
                        }
                    });
                    output.push_str(&format!("{name}: {}\n", list.join(", ")));
                }
            }
        }
        super::report::message(&output);
        return Ok(0);
    }
    output.push_str(&format!("Dart SDK {}\n", settings.sdk));
    let transitive = || -> BTreeSet<String> {
        let mut all: BTreeSet<String> = if include_dev {
            graph.packages.keys().cloned().collect()
        } else {
            entry
                .workspace
                .iter()
                .flat_map(|p| p.manifest.dependencies.iter().chain(&p.manifest.overrides))
                .flat_map(|d| graph.transitive(&d.name))
                .collect()
        };
        for root in &entry.workspace {
            all.remove(&root.manifest.name);
            for d in &root.manifest.dependencies {
                all.remove(&d.name);
            }
            if include_dev {
                for d in &root.manifest.dev {
                    all.remove(&d.name);
                }
            }
            for d in &root.manifest.overrides {
                all.remove(&d.name);
            }
        }
        all
    };
    match style {
        "compact" | "list" => {
            let section = |output: &mut String,
                           title: &str,
                           names: Vec<String>,
                           sort: bool|
             -> Result<(), Failure> {
                if names.is_empty() {
                    return Ok(());
                }
                let mut names = names;
                if sort {
                    names.sort();
                }
                output.push_str(&format!("\n{title}:\n"));
                for name in names {
                    let package = graph.get(&name)?;
                    if style == "compact" {
                        output.push_str(&format!("- {}", label(package, &name)));
                        if package.manifest.dependencies.is_empty() {
                            output.push('\n');
                        } else {
                            let names: Vec<&str> = package
                                .manifest
                                .dependencies
                                .iter()
                                .map(|d| d.name.as_str())
                                .collect();
                            output.push_str(&format!(" [{}]\n", names.join(" ")));
                        }
                    } else {
                        output.push_str(&format!("- {}\n", label(package, &name)));
                        for dependency in &package.manifest.dependencies {
                            output.push_str(&format!(
                                "  - {} {}\n",
                                dependency.name, dependency.constraint
                            ));
                        }
                    }
                }
                Ok(())
            };
            let compact = style == "compact";
            for (index, root) in entry.workspace.iter().enumerate() {
                if index > 0 {
                    output.push('\n');
                }
                let package = graph.get(&root.manifest.name)?;
                output.push_str(&format!("{}\n", label(package, &root.manifest.name)));
                let names = |deps: &[super::manifest::Dependency]| {
                    deps.iter().map(|d| d.name.clone()).collect::<Vec<_>>()
                };
                section(
                    &mut output,
                    "dependencies",
                    names(&root.manifest.dependencies),
                    compact,
                )?;
                if include_dev {
                    section(
                        &mut output,
                        "dev dependencies",
                        names(&root.manifest.dev),
                        compact,
                    )?;
                }
                section(
                    &mut output,
                    "dependency overrides",
                    names(&root.manifest.overrides),
                    compact,
                )?;
            }
            section(
                &mut output,
                "transitive dependencies",
                transitive().into_iter().collect(),
                true,
            )?;
        }
        _ => {
            let workspace: Vec<String> = entry
                .workspace
                .iter()
                .map(|p| p.manifest.name.clone())
                .collect();
            let mut tree = Tree::default();
            let mut visited = BTreeSet::new();
            // Paths into `tree` are kept as label sequences.
            let mut queue: VecDeque<(String, Vec<String>)> =
                workspace.iter().map(|n| (n.clone(), Vec::new())).collect();
            let root_dev: Vec<String> = entry
                .root()
                .manifest
                .dev
                .iter()
                .map(|d| d.name.clone())
                .collect();
            while let Some((name, path)) = queue.pop_front() {
                let mut node = &mut tree;
                for step in &path {
                    node = node.0.get_mut(step).expect("parent was inserted");
                }
                if !visited.insert(name.clone()) {
                    node.0.insert(format!("{name}..."), Tree::default());
                    continue;
                }
                let package = graph.get(&name)?;
                let text = label(package, &name);
                node.0.insert(text.clone(), Tree::default());
                let is_root = workspace.contains(&name);
                let mut children: Vec<String> = if is_root {
                    immediate(&package.manifest)
                } else {
                    package
                        .manifest
                        .dependencies
                        .iter()
                        .map(|d| d.name.clone())
                        .collect()
                };
                if !include_dev {
                    let dev: Vec<String> = package
                        .manifest
                        .dev
                        .iter()
                        .map(|d| d.name.clone())
                        .collect();
                    children.retain(|c| !dev.contains(c));
                }
                let _ = &root_dev;
                let mut child_path = path.clone();
                child_path.push(text);
                for child in children {
                    queue.push_back((child, child_path.clone()));
                }
            }
            draw(&mut output, "", None, &tree, false, 0);
        }
    }
    super::report::message(&output);
    Ok(0)
}

/// JSON printed like Dart's `JsonEncoder.withIndent('  ')`, keeping key
/// order.
pub(super) enum PrettyJson {
    String(String),
    Number(String),
    Bool(bool),
    Null,
    Array(Vec<PrettyJson>),
    Object(Vec<(String, PrettyJson)>),
}

impl PrettyJson {
    fn from_map(map: &serde_json::Map<String, serde_json::Value>) -> Self {
        // `serde_json` maps are sorted; restore insertion order for the
        // keys Pub writes.
        let order = [
            "name",
            "version",
            "kind",
            "source",
            "dependencies",
            "directDependencies",
            "devDependencies",
        ];
        let mut entries: Vec<(String, PrettyJson)> = Vec::new();
        for key in order {
            if let Some(value) = map.get(key) {
                entries.push((key.to_owned(), Self::from_value(value)));
            }
        }
        PrettyJson::Object(entries)
    }

    pub fn from_value(value: &serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => PrettyJson::Null,
            serde_json::Value::Bool(b) => PrettyJson::Bool(*b),
            serde_json::Value::Number(n) => PrettyJson::Number(n.to_string()),
            serde_json::Value::String(s) => PrettyJson::String(s.clone()),
            serde_json::Value::Array(items) => {
                PrettyJson::Array(items.iter().map(Self::from_value).collect())
            }
            serde_json::Value::Object(map) => PrettyJson::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), Self::from_value(v)))
                    .collect(),
            ),
        }
    }

    pub fn render(&self, depth: usize) -> String {
        let pad = "  ".repeat(depth + 1);
        let close = "  ".repeat(depth);
        let quote = |s: &str| serde_json::to_string(s).expect("string serialization");
        match self {
            PrettyJson::String(s) => quote(s),
            PrettyJson::Number(n) => n.clone(),
            PrettyJson::Bool(b) => b.to_string(),
            PrettyJson::Null => "null".into(),
            PrettyJson::Array(items) if items.is_empty() => "[]".into(),
            PrettyJson::Object(entries) if entries.is_empty() => "{}".into(),
            PrettyJson::Array(items) => format!(
                "[\n{}\n{close}]",
                items
                    .iter()
                    .map(|i| format!("{pad}{}", i.render(depth + 1)))
                    .collect::<Vec<_>>()
                    .join(",\n")
            ),
            PrettyJson::Object(entries) => format!(
                "{{\n{}\n{close}}}",
                entries
                    .iter()
                    .map(|(k, v)| format!("{pad}{}: {}", quote(k), v.render(depth + 1)))
                    .collect::<Vec<_>>()
                    .join(",\n")
            ),
        }
    }
}

pub(super) fn outdated(args: &[String]) -> Result<u8, Failure> {
    super::outdated::run(args)
}
