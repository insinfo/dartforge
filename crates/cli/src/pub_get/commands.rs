//! The `dart pub` subcommands.
use super::{
    Acquire, Arguments, COMMON, Entrypoint, Failure, Settings, Spec, acquire, flag, option,
    solver::SolveType, usage,
};
use std::path::PathBuf;

/// Parses a subcommand's arguments with the DartForge options appended.
fn parse(args: &[String], specs: &[Spec], command: &str) -> Result<(Arguments, Settings), Failure> {
    let mut all: Vec<Spec> = Vec::new();
    for spec in specs.iter().chain(COMMON.iter()) {
        all.push(Spec {
            name: spec.name,
            abbr: spec.abbr,
            value: spec.value,
            negatable: spec.negatable,
        });
    }
    let arguments = Arguments::parse(args, &all, command)?;
    let settings = Settings::discover(
        arguments.option("sdk").map(PathBuf::from),
        arguments.option("sdk-version"),
    )?;
    Ok((arguments, settings))
}

fn help(arguments: &Arguments, text: &str) -> bool {
    if arguments.is("help") {
        println!("{text}");
        return true;
    }
    false
}

fn directory(arguments: &Arguments) -> String {
    arguments.option("directory").unwrap_or(".").to_owned()
}

fn packages_dir_warning(arguments: &Arguments) {
    if arguments.flag("packages-dir").is_some() {
        eprintln!("The --packages-dir flag is no longer used and does nothing.");
    }
}

/// The example of the workspace root, when it has one.
fn example(entry: &Entrypoint, settings: &Settings) -> Result<Option<Entrypoint>, Failure> {
    let directory = entry.root().directory.join("example");
    if !directory.join("pubspec.yaml").is_file() {
        return Ok(None);
    }
    Entrypoint::load(&entry.example_dir(), settings).map(Some)
}

const GET_HELP: &str = "Get the current package's dependencies.

Usage: dart pub get <subcommand> [arguments...]
-h, --help                 Print this usage information.
    --[no-]offline         Use cached packages instead of accessing the network.
-n, --dry-run              Report what dependencies would change but don't change any.
    --enforce-lockfile     Enforce pubspec.lock. Fail resolution if pubspec.lock does not satisfy pubspec.yaml
    --[no-]precompile      Build executables in immediate dependencies.
-C, --directory=<dir>      Run this in the directory <dir>.";

pub(super) fn get(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("offline", None, true),
        flag("dry-run", Some('n'), false),
        flag("enforce-lockfile", None, false),
        flag("precompile", None, true),
        flag("packages-dir", None, true),
        flag("example", None, true),
        option("directory", Some('C')),
    ];
    let (arguments, settings) = parse(args, &specs, "get")?;
    if help(&arguments, GET_HELP) {
        return Ok(0);
    }
    if !arguments.rest.is_empty() {
        return Err(usage(format!(
            "Command \"get\" does not take any arguments.\n\n{GET_HELP}"
        )));
    }
    if arguments.is("precompile") {
        return Err("pub get: --precompile is not supported by dartforge".into());
    }
    packages_dir_warning(&arguments);
    let offline = arguments.is("offline");
    let entry = Entrypoint::load(&directory(&arguments), &settings)?;
    let options = Acquire {
        dry_run: arguments.is("dry-run"),
        enforce_lockfile: arguments.is("enforce-lockfile"),
        ..Acquire::default()
    };
    acquire(&entry, &settings, offline, SolveType::Get, &options)?;
    if arguments.flag("example").unwrap_or(true)
        && let Some(example) = example(&entry, &settings)?
    {
        acquire(
            &example,
            &settings,
            offline,
            SolveType::Get,
            &Acquire {
                summary_only: true,
                ..options
            },
        )?;
    }
    Ok(0)
}

const UPGRADE_HELP: &str = "Upgrade the current package's dependencies to latest versions.

Usage: dart pub upgrade [dependencies...]
-h, --help                 Print this usage information.
    --[no-]offline         Use cached packages instead of accessing the network.
-n, --dry-run              Report what dependencies would change but don't change any.
    --[no-]precompile      Precompile executables in immediate dependencies.
    --tighten              Updates lower bounds in pubspec.yaml to match the resolved version.
    --unlock-transitive    Also upgrades the transitive dependencies of the listed [dependencies]
    --major-versions       Upgrades packages to their latest resolvable versions, and updates pubspec.yaml.
-C, --directory=<dir>      Run this in the directory <dir>.";

pub(super) fn upgrade(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("offline", None, true),
        flag("dry-run", Some('n'), false),
        flag("precompile", None, true),
        flag("null-safety", None, false),
        flag("nullsafety", None, false),
        flag("packages-dir", None, true),
        flag("tighten", None, false),
        flag("unlock-transitive", None, false),
        flag("major-versions", None, false),
        flag("example", None, true),
        option("directory", Some('C')),
    ];
    let (arguments, settings) = parse(args, &specs, "upgrade")?;
    if help(&arguments, UPGRADE_HELP) {
        return Ok(0);
    }
    if arguments.is("null-safety") || arguments.is("nullsafety") {
        return Err(super::data(
            "The `--null-safety` flag is no longer supported.\nConsider using the Dart 2.19 sdk to migrate to null safety.",
        ));
    }
    packages_dir_warning(&arguments);
    let offline = arguments.is("offline");
    let entry = Entrypoint::load(&directory(&arguments), &settings)?;
    let dry_run = arguments.is("dry-run");
    let unlock = super::edit::packages_to_upgrade(&entry, &settings, &arguments)?;
    let has_example =
        arguments.flag("example").unwrap_or(true) && example(&entry, &settings)?.is_some();
    if arguments.is("major-versions") {
        if has_example {
            eprintln!(
                "Running `upgrade --major-versions` only in `{}`. Run `dart pub upgrade --major-versions --directory example/` separately.",
                entry.root_dir
            );
        }
        super::edit::upgrade_major_versions(&entry, &settings, offline, &arguments, &unlock)?;
    } else {
        acquire(
            &entry,
            &settings,
            offline,
            SolveType::Upgrade,
            &Acquire {
                unlock: unlock.clone(),
                dry_run,
                ..Acquire::default()
            },
        )?;
        offline_warning(offline);
        if arguments.is("tighten") {
            if has_example {
                eprintln!(
                    "Running `upgrade --tighten` only in `{}`. Run `dart pub upgrade --tighten --directory example/` separately.",
                    entry.root_dir
                );
            }
            let entry = Entrypoint::load(&directory(&arguments), &settings)?;
            let changes =
                super::edit::tighten(&entry, &settings, &unlock, &Default::default(), None)?;
            super::edit::apply_changes(&entry, &changes, dry_run)?;
        }
    }
    if has_example {
        let entry = Entrypoint::load(&directory(&arguments), &settings)?;
        if let Some(example) = example(&entry, &settings)? {
            acquire(
                &example,
                &settings,
                offline,
                SolveType::Upgrade,
                &Acquire {
                    unlock,
                    dry_run,
                    summary_only: true,
                    ..Acquire::default()
                },
            )?;
            offline_warning(offline);
        }
    }
    Ok(0)
}

pub(super) fn offline_warning(offline: bool) {
    if offline {
        eprintln!(
            "Warning: Upgrading when offline may not update you to the latest versions of your dependencies."
        );
    }
}

const DOWNGRADE_HELP: &str = "Downgrade the current package's dependencies to oldest versions.

Usage: dart pub downgrade [dependencies...]
-h, --help                 Print this usage information.
    --[no-]offline         Use cached packages instead of accessing the network.
-n, --dry-run              Report what dependencies would change but don't change any.
    --tighten              Updates lower bounds in pubspec.yaml to match the resolved version.
-C, --directory=<dir>      Run this in the directory <dir>.";

pub(super) fn downgrade(args: &[String]) -> Result<u8, Failure> {
    let specs = [
        flag("offline", None, true),
        flag("dry-run", Some('n'), false),
        flag("packages-dir", None, true),
        flag("tighten", None, false),
        flag("example", None, true),
        option("directory", Some('C')),
    ];
    let (arguments, settings) = parse(args, &specs, "downgrade")?;
    if help(&arguments, DOWNGRADE_HELP) {
        return Ok(0);
    }
    packages_dir_warning(&arguments);
    let offline = arguments.is("offline");
    let dry_run = arguments.is("dry-run");
    let entry = Entrypoint::load(&directory(&arguments), &settings)?;
    acquire(
        &entry,
        &settings,
        offline,
        SolveType::Downgrade,
        &Acquire {
            unlock: arguments.rest.clone(),
            dry_run,
            ..Acquire::default()
        },
    )?;
    let example_entry = if arguments.flag("example").unwrap_or(true) {
        example(&entry, &settings)?
    } else {
        None
    };
    if let Some(example) = &example_entry {
        acquire(
            example,
            &settings,
            offline,
            SolveType::Get,
            &Acquire {
                unlock: arguments.rest.clone(),
                dry_run,
                summary_only: true,
                ..Acquire::default()
            },
        )?;
    }
    if arguments.is("tighten") {
        if example_entry.is_some() {
            eprintln!(
                "Running `downgrade --tighten` only in `{}`. Run `dart pub upgrade --tighten --directory example/` separately.",
                entry.root_dir
            );
        }
        let entry = Entrypoint::load(&directory(&arguments), &settings)?;
        let changes = super::edit::tighten(&entry, &settings, &[], &Default::default(), None)?;
        super::edit::apply_changes(&entry, &changes, dry_run)?;
    }
    if offline {
        eprintln!(
            "Warning: Downgrading when offline may not update you to the oldest versions of your dependencies."
        );
    }
    Ok(0)
}

pub(super) fn add(args: &[String]) -> Result<u8, Failure> {
    super::edit::add(args)
}

pub(super) fn remove(args: &[String]) -> Result<u8, Failure> {
    super::edit::remove(args)
}

pub(super) fn deps(args: &[String]) -> Result<u8, Failure> {
    super::inspect::deps(args)
}

pub(super) fn outdated(args: &[String]) -> Result<u8, Failure> {
    super::inspect::outdated(args)
}
