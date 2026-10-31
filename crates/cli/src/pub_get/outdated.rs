//! `dart pub outdated`: current, upgradable, resolvable and latest versions
//! of every dependency, as Pub 3.6.2 computes and prints them (security
//! advisories are not fetched).
use super::{
    Arguments, Entrypoint, Failure, Settings, data, flag,
    inspect::PrettyJson,
    manifest::{Description, Manifest},
    option,
    report::message,
    solver::SolveType,
    source::{Cache, LockFile, PackageId},
    usage,
    version::{Range, Version, VersionSet},
};
use std::{collections::BTreeSet, path::PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Direct,
    Dev,
    Transitive,
    DevTransitive,
}

#[derive(Clone, PartialEq)]
struct Details {
    version: Version,
    source: String,
    overridden: bool,
}

impl Details {
    fn of(id: &PackageId, overridden: bool) -> Self {
        Self {
            version: id.version.clone(),
            source: id.description.source().to_owned(),
            overridden,
        }
    }

    fn describe(&self) -> String {
        if self.overridden {
            format!("{} (overridden)", self.version)
        } else {
            match self.source.as_str() {
                "sdk" => "(sdk)".into(),
                "git" => format!("{} (git)", self.version),
                "path" => format!("{} (path)", self.version),
                _ => self.version.to_string(),
            }
        }
    }

    fn less(&self, other: &Self) -> bool {
        self.overridden == other.overridden
            && self.source == other.source
            && self.version < other.version
    }
}

struct Row {
    name: String,
    columns: [Option<Details>; 4],
    kind: Kind,
    discontinued: bool,
    replaced_by: Option<String>,
    retracted: bool,
    latest: bool,
}

/// `SystemCache.getLatest`.
fn latest(
    cache: &mut Cache,
    name: &str,
    description: Option<&Description>,
    version: Option<&Version>,
    prereleases: bool,
) -> Option<PackageId> {
    let description = description?;
    let mut available = cache.versions(name, description, None).ok()?;
    if available.is_empty() {
        return None;
    }
    let prioritize =
        |a: &PackageId, b: &PackageId| match (a.version.is_prerelease(), b.version.is_prerelease())
        {
            (false, true) => std::cmp::Ordering::Greater,
            (true, false) => std::cmp::Ordering::Less,
            _ => a.version.cmp(&b.version),
        };
    if prereleases {
        available.sort_by(|a, b| a.version.cmp(&b.version));
    } else {
        available.sort_by(prioritize);
    }
    let mut latest = available.last().cloned()?;
    if let Some(version) = version
        && version.is_prerelease()
        && *version > latest.version
    {
        available.sort_by(|a, b| a.version.cmp(&b.version));
        latest = available.last().cloned()?;
    }
    Some(latest)
}

fn closure(entry: &Entrypoint, cache: &mut Cache, resolution: &[PackageId]) -> BTreeSet<String> {
    let mut result: BTreeSet<String> = entry
        .workspace
        .iter()
        .map(|p| p.manifest.name.clone())
        .collect();
    let mut queue: Vec<String> = entry
        .workspace
        .iter()
        .flat_map(|p| p.manifest.dependencies.iter().map(|d| d.name.clone()))
        .collect();
    while let Some(name) = queue.pop() {
        if !result.insert(name.clone()) {
            continue;
        }
        let Some(id) = resolution.iter().find(|i| i.name == name) else {
            continue;
        };
        if let Ok(manifest) = cache.describe(id) {
            queue.extend(manifest.dependencies.iter().map(|d| d.name.clone()));
        }
    }
    result
}

fn has_override(entry: &Entrypoint, name: &str) -> bool {
    entry
        .workspace
        .iter()
        .any(|p| p.manifest.override_of(name).is_some())
}

fn first_dependency<'a>(entry: &'a Entrypoint, name: &str, dev: bool) -> Option<&'a Description> {
    entry
        .workspace
        .iter()
        .flat_map(|p| {
            if dev {
                p.manifest.dev.iter()
            } else {
                p.manifest.dependencies.iter()
            }
        })
        .find(|d| d.name == name)
        .map(|d| &d.description)
}

fn transform(entry: &Entrypoint, f: impl Fn(&mut Manifest)) -> Entrypoint {
    let mut result = entry.clone();
    for package in &mut result.workspace {
        f(&mut package.manifest);
    }
    result
}

const HELP: &str = "Analyze your dependencies to find which ones can be upgraded.

Usage: dart pub outdated [options]
-h, --help                         Print this usage information.
    --[no-]dependency-overrides    Show resolutions with `dependency_overrides`.
                                   (defaults to on)
    --[no-]dev-dependencies        Take dev dependencies into account.
                                   (defaults to on)
    --json                         Output the results using a json format.
    --[no-]prereleases             Include prereleases in latest version.
    --[no-]show-all                Include dependencies that are already fulfilling --mode.
    --[no-]transitive              Show transitive dependencies.
                                   (defaults to on)
-C, --directory=<dir>              Run this in the directory <dir>.";

pub(super) fn run(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("dependency-overrides", None, true),
        flag("dev-dependencies", None, true),
        flag("json", None, true),
        option("mode", None),
        flag("prereleases", None, true),
        flag("pre-releases", None, true),
        flag("show-all", None, true),
        flag("up-to-date", None, true),
        flag("transitive", None, true),
        option("directory", Some('C')),
        option("sdk", None),
        option("sdk-version", None),
        flag("help", Some('h'), false),
    ];
    let arguments = Arguments::parse(args, &specs, "outdated")?;
    if arguments.is("help") {
        println!("{HELP}");
        return Ok(0);
    }
    if !arguments.rest.is_empty() {
        return Err(usage(format!(
            "Command \"outdated\" does not take any arguments.\n\n{HELP}"
        )));
    }
    match arguments.option("mode") {
        None | Some("outdated") => {}
        Some("null-safety") => {
            return Err(data(
                "The `--mode=null-safety` option is no longer supported.\nConsider using the Dart 2.19 sdk to migrate to null safety.",
            ));
        }
        Some(other) => {
            return Err(usage(format!(
                "\"{other}\" is not an allowed value for option \"mode\"."
            )));
        }
    }
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    let include_dev = arguments.flag("dev-dependencies").unwrap_or(true);
    let include_overrides = arguments.flag("dependency-overrides").unwrap_or(true);
    let json = arguments.is("json");
    if json && arguments.flag("transitive").is_some() {
        return Err(usage(
            "Cannot specify both `--json` and `--transitive`\nThe json report always includes transitive dependencies.",
        ));
    }
    let prereleases = arguments
        .flag("prereleases")
        .or(arguments.flag("pre-releases"))
        .unwrap_or(false);
    let show_transitive = arguments.flag("transitive").unwrap_or(true);
    let show_all = arguments.is("show-all") || arguments.is("up-to-date");
    let directory = arguments.option("directory").unwrap_or(".").to_owned();
    let entry = Entrypoint::load(&directory, &settings)?;
    let lock_path = entry.root().directory.join("pubspec.lock");
    let lock = LockFile::read(&entry.root().directory, &settings.host).map_err(data)?;
    let base = if include_overrides {
        entry.clone()
    } else {
        transform(&entry, |m| m.overrides.clear())
    };
    let upgradable_ws = if include_dev {
        base.clone()
    } else {
        transform(&base, |m| m.dev.clear())
    };
    let resolvable_ws = transform(&upgradable_ws, |m| {
        for dependency in m.dependencies.iter_mut().chain(m.dev.iter_mut()) {
            dependency.constraint = match dependency.constraint.ranges().first() {
                None => VersionSet::empty(),
                Some(range) => VersionSet::from_range(Range {
                    min: range.min.clone(),
                    include_min: range.include_min,
                    max: None,
                    include_max: false,
                }),
            };
        }
    });
    let mut cache = settings.cache(false);
    let upgradable = super::solve(&upgradable_ws, &mut cache, SolveType::Upgrade, &lock, &[])
        .ok()
        .map(|r| r.packages);
    let resolvable = super::solve(&resolvable_ws, &mut cache, SolveType::Upgrade, &lock, &[])
        .ok()
        .map(|r| r.packages);
    let (has_upgradable, has_resolvable) = (upgradable.is_some(), resolvable.is_some());
    let upgradable = upgradable.unwrap_or_default();
    let resolvable = resolvable.unwrap_or_default();
    let current: Vec<PackageId> = lock.packages.values().cloned().collect();
    let mut non_dev = closure(&entry, &mut cache, &current);
    non_dev.extend(closure(&entry, &mut cache, &upgradable));
    non_dev.extend(closure(&entry, &mut cache, &resolvable));
    let mut visited: BTreeSet<String> = entry
        .workspace
        .iter()
        .map(|p| p.manifest.name.clone())
        .collect();
    let mut rows: Vec<Row> = Vec::new();
    for id in current.iter().chain(&upgradable).chain(&resolvable) {
        if !visited.insert(id.name.clone()) {
            continue;
        }
        let name = id.name.as_str();
        let current = lock.packages.get(name);
        let upgradable_id = upgradable.iter().find(|p| p.name == name);
        let resolvable_id = resolvable.iter().find(|p| p.name == name);
        let mut latest_id = None;
        let mut latest_overridden = false;
        if !has_override(&entry, name) {
            latest_id = latest(
                &mut cache,
                name,
                current.map(|c| &c.description),
                current.map(|c| &c.version),
                prereleases,
            );
        }
        if latest_id.is_none() {
            latest_id = latest(
                &mut cache,
                name,
                first_dependency(&base, name, false),
                None,
                prereleases,
            );
        }
        if latest_id.is_none() {
            latest_id = latest(
                &mut cache,
                name,
                first_dependency(&base, name, true),
                None,
                prereleases,
            );
        }
        if latest_id.is_none() && !has_override(&upgradable_ws, name) {
            latest_id = latest(
                &mut cache,
                name,
                upgradable_id.map(|c| &c.description),
                upgradable_id.map(|c| &c.version),
                prereleases,
            );
        }
        if latest_id.is_none() && !has_override(&resolvable_ws, name) {
            latest_id = latest(
                &mut cache,
                name,
                resolvable_id.map(|c| &c.description),
                resolvable_id.map(|c| &c.version),
                prereleases,
            );
        }
        if latest_id.is_none() {
            let id = current.or(upgradable_id).or(resolvable_id);
            latest_id = latest(
                &mut cache,
                name,
                id.map(|c| &c.description),
                id.map(|c| &c.version),
                prereleases,
            );
            latest_overridden = true;
        }
        let (mut discontinued, mut replaced_by, mut retracted) = (false, None, false);
        if let Some(current) = current
            && let Description::Hosted(url) = &current.description
            && let Ok(listing) = cache.listing(name, url)
        {
            discontinued = listing.discontinued;
            replaced_by = listing.replaced_by.clone();
            retracted = listing
                .versions
                .iter()
                .any(|l| l.version == current.version && l.retracted);
        }
        let current_details =
            current.map(|c| Details::of(c, entry.root().manifest.override_of(name).is_some()));
        let latest_details = latest_id
            .as_ref()
            .map(|l| Details::of(l, latest_overridden));
        let is_latest = current_details == latest_details;
        let kind = if entry
            .workspace
            .iter()
            .any(|p| p.manifest.dependency(name).is_some())
        {
            Kind::Direct
        } else if entry
            .workspace
            .iter()
            .any(|p| p.manifest.dev_dependency(name).is_some())
        {
            Kind::Dev
        } else if non_dev.contains(name) {
            Kind::Transitive
        } else {
            Kind::DevTransitive
        };
        rows.push(Row {
            name: name.to_owned(),
            columns: [
                current_details,
                upgradable_id.map(|u| Details::of(u, has_override(&upgradable_ws, name))),
                resolvable_id.map(|r| Details::of(r, has_override(&resolvable_ws, name))),
                latest_details,
            ],
            kind,
            discontinued,
            replaced_by,
            retracted,
            latest: is_latest,
        });
    }
    if !include_dev {
        rows.retain(|r| r.kind != Kind::Dev);
    }
    rows.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name)));
    let marked = |row: &Row| -> Vec<String> {
        let mut columns: Vec<String> = row
            .columns
            .iter()
            .enumerate()
            .map(|(index, details)| match details {
                None => "-".to_owned(),
                Some(details) => {
                    let is_latest = Some(details) == row.columns[3].as_ref();
                    let mut text =
                        format!("{}{}", if is_latest { "" } else { "*" }, details.describe());
                    if !is_latest && index == 0 && row.retracted {
                        text.push_str(" (retracted)");
                    }
                    text
                }
            })
            .collect();
        if row.discontinued {
            columns.push("(discontinued)".into());
        }
        columns
    };
    if json {
        if !show_all {
            rows.retain(|r| !r.latest);
        }
        if !include_dev {
            rows.retain(|r| r.kind != Kind::Dev && r.kind != Kind::DevTransitive);
        }
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        let version = |d: &Option<Details>| match d {
            None => PrettyJson::Null,
            Some(d) => {
                let mut entries = vec![(
                    "version".to_owned(),
                    PrettyJson::String(d.version.to_string()),
                )];
                if d.overridden {
                    entries.push(("overridden".into(), PrettyJson::Bool(true)));
                }
                PrettyJson::Object(entries)
            }
        };
        let packages = rows
            .iter()
            .map(|r| {
                PrettyJson::Object(vec![
                    ("package".into(), PrettyJson::String(r.name.clone())),
                    (
                        "kind".into(),
                        PrettyJson::String(
                            match r.kind {
                                Kind::Direct => "direct",
                                Kind::Dev => "dev",
                                _ => "transitive",
                            }
                            .into(),
                        ),
                    ),
                    ("isDiscontinued".into(), PrettyJson::Bool(r.discontinued)),
                    ("isCurrentRetracted".into(), PrettyJson::Bool(r.retracted)),
                    (
                        "isCurrentAffectedByAdvisory".into(),
                        PrettyJson::Bool(false),
                    ),
                    ("current".into(), version(&r.columns[0])),
                    ("upgradable".into(), version(&r.columns[1])),
                    ("resolvable".into(), version(&r.columns[2])),
                    ("latest".into(), version(&r.columns[3])),
                ])
            })
            .collect();
        let text =
            PrettyJson::Object(vec![("packages".into(), PrettyJson::Array(packages))]).render(0);
        message(&text);
        return Ok(0);
    }
    let shown = std::path::Path::new(&directory);
    let normalized = super::manifest::normalize(shown).display().to_string();
    let directory_description = if normalized == "." {
        String::new()
    } else {
        format!(" in {normalized}")
    };
    message(&format!(
        "Showing outdated packages{directory_description}.\n[*] indicates versions that are not the latest available.\n\n"
    ));
    let formatted: Vec<(Kind, Vec<String>)> = rows
        .iter()
        .filter(|r| show_all || !r.latest)
        .map(|r| {
            let mut columns = vec![r.name.clone()];
            columns.extend(marked(r));
            (r.kind, columns)
        })
        .collect();
    let visible: Vec<&Row> = rows.iter().filter(|r| show_all || !r.latest).collect();
    if visible.is_empty() {
        message("Found no outdated packages");
        return Ok(0);
    }
    let not_sdk = |dev: bool| {
        base.workspace.iter().any(|p| {
            (if dev {
                &p.manifest.dev
            } else {
                &p.manifest.dependencies
            })
            .iter()
            .any(|d| !matches!(d.description, Description::Sdk(_)))
        })
    };
    let of = |kind: Kind| {
        formatted
            .iter()
            .filter(move |(k, _)| *k == kind)
            .map(|(_, c)| c.clone())
    };
    let direct: Vec<Vec<String>> = of(Kind::Direct).collect();
    let dev: Vec<Vec<String>> = of(Kind::Dev).collect();
    let transitive: Vec<Vec<String>> = of(Kind::Transitive).collect();
    let dev_transitive: Vec<Vec<String>> = of(Kind::DevTransitive).collect();
    let mut table: Vec<Vec<String>> = vec![
        [
            "Package Name",
            "Current",
            "Upgradable",
            "Resolvable",
            "Latest",
        ]
        .map(String::from)
        .to_vec(),
    ];
    if not_sdk(false) {
        table.push(vec![if direct.is_empty() {
            "\ndirect dependencies: all up-to-date.".into()
        } else {
            "\ndirect dependencies:".into()
        }]);
        table.extend(direct.iter().cloned());
    }
    if include_dev && not_sdk(true) {
        table.push(vec![if dev.is_empty() {
            "\ndev_dependencies: all up-to-date.".into()
        } else {
            "\ndev_dependencies:".into()
        }]);
        table.extend(dev.iter().cloned());
    }
    if show_transitive {
        if !transitive.is_empty() {
            table.push(vec!["\ntransitive dependencies:".into()]);
        }
        table.extend(transitive);
        if include_dev {
            if !dev_transitive.is_empty() {
                table.push(vec!["\ntransitive dev_dependencies:".into()]);
            }
            table.extend(dev_transitive);
        }
    }
    let mut widths: Vec<usize> = Vec::new();
    for row in &table {
        if row.len() > 1 {
            for (index, cell) in row.iter().enumerate() {
                if widths.len() <= index {
                    widths.push(0);
                }
                widths[index] = widths[index].max(cell.chars().count());
            }
        }
    }
    for row in &table {
        let mut line = String::new();
        for (index, cell) in row.iter().enumerate() {
            line.push_str(cell);
            let width = widths.get(index).copied().unwrap_or(0) + 2;
            line.push_str(&" ".repeat(width.saturating_sub(cell.chars().count())));
        }
        message(&line);
    }
    let visible_kind = |r: &&&Row| show_transitive || matches!(r.kind, Kind::Direct | Kind::Dev);
    let upgradable_count = visible
        .iter()
        .filter(visible_kind)
        .filter(|r| match (&r.columns[0], &r.columns[1]) {
            (Some(c), Some(u)) => c.less(u),
            _ => false,
        })
        .count();
    let lock_exists = lock_path.is_file();
    let not_at_resolvable = visible
        .iter()
        .filter(visible_kind)
        .filter(|r| {
            (r.columns[0].is_some() || !lock_exists)
                && match (&r.columns[1], &r.columns[2]) {
                    (Some(u), Some(re)) => u.less(re),
                    _ => false,
                }
        })
        .count();
    if !has_upgradable || !has_resolvable {
        message(
            "No resolution was found. Try running `dart pub upgrade --dry-run` to explore why.",
        );
    } else if lock_exists {
        if upgradable_count == 1 {
            message(
                "\n1 upgradable dependency is locked (in pubspec.lock) to an older version.\nTo update it, use `dart pub upgrade`.",
            );
        } else if upgradable_count > 1 {
            message(&format!(
                "\n{upgradable_count} upgradable dependencies are locked (in pubspec.lock) to older versions.\nTo update these dependencies, use `dart pub upgrade`."
            ));
        }
        if not_at_resolvable == 0
            && upgradable_count == 0
            && (!direct.is_empty() || !dev.is_empty())
        {
            message(
                "You are already using the newest resolvable versions listed in the 'Resolvable' column.\nNewer versions, listed in 'Latest', may not be mutually compatible.",
            );
        } else if direct.is_empty() && dev.is_empty() {
            message("all dependencies are up-to-date.");
        }
    } else {
        message(
            "\nNo pubspec.lock found. There are no Current versions.\nRun `dart pub get` to create a pubspec.lock with versions matching your pubspec.yaml.",
        );
    }
    if not_at_resolvable == 1 {
        message(
            "\n1 dependency is constrained to a version that is older than a resolvable version.\nTo update it, edit pubspec.yaml, or run `dart pub upgrade --major-versions`.",
        );
    } else if not_at_resolvable > 1 {
        message(&format!(
            "\n{not_at_resolvable}  dependencies are constrained to versions that are older than a resolvable version.\nTo update these dependencies, edit pubspec.yaml, or run `dart pub upgrade --major-versions`."
        ));
    }
    let extra: Vec<&&Row> = visible
        .iter()
        .filter(|r| r.discontinued || r.retracted)
        .collect();
    if !extra.is_empty() {
        message("\n");
        for row in extra {
            message(&row.name);
            if row.discontinued {
                let replaced = row
                    .replaced_by
                    .as_ref()
                    .map(|r| format!(", replaced by {r}."))
                    .unwrap_or_else(|| ".".into());
                message(&format!(
                    "    Package {} has been discontinued{replaced} See https://dart.dev/go/package-discontinue",
                    row.name
                ));
            }
            if row.retracted
                && let Some(current) = &row.columns[0]
            {
                message(&format!(
                    "    Version {} is retracted. See https://dart.dev/go/package-retraction",
                    current.version
                ));
            }
        }
    }
    Ok(0)
}
