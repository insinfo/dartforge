//! Pub's `SolveReport`: the changed packages, the summary line and the
//! notes on discontinued and outdated packages.
use super::{
    manifest::{Description, Manifest},
    output::NewLock,
    solver::SolveType,
    source::{Cache, LockFile, PackageId},
    version::Version,
};
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
};

pub(crate) struct Report<'a> {
    pub kind: SolveType,
    /// `presentationDir` of the workspace root.
    pub location: &'a str,
    pub root: &'a Manifest,
    /// Whether the root's overrides come from `pubspec_overrides.yaml`.
    pub overrides_file: bool,
    pub overridden: &'a BTreeSet<String>,
    pub previous: &'a LockFile,
    pub new: &'a NewLock,
    pub available: &'a HashMap<String, Vec<Version>>,
    pub dry_run: bool,
    pub enforce_lockfile: bool,
    pub quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Direct,
    Dev,
    None,
}

fn root_kind(root: &Manifest, name: &str) -> Kind {
    if root.dependency(name).is_some() {
        Kind::Direct
    } else if root.dev_dependency(name).is_some() {
        Kind::Dev
    } else {
        Kind::None
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Direct => "direct",
        Kind::Dev => "dev",
        Kind::None => "transitive",
    }
}

/// Pub's `Version.prioritize`: stable versions before prereleases, then by
/// version.
fn prioritize(a: &Version, b: &Version) -> std::cmp::Ordering {
    match (a.is_prerelease(), b.is_prerelease()) {
        (false, true) => std::cmp::Ordering::Greater,
        (true, false) => std::cmp::Ordering::Less,
        _ => a.cmp(b),
    }
}

/// Prints `text` like Pub's `log.message`: nothing when empty.
pub(crate) fn message(text: &str) {
    let text = text.strip_suffix('\n').unwrap_or(text);
    if !text.is_empty() {
        println!("{text}");
    }
}

/// `Description.format` of a resolved id, used after `from <source>`.
pub(crate) fn format_id_description(id: &PackageId) -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    match (&id.description, &id.resolved) {
        (Description::Git { url, path, .. }, Some(commit)) => {
            let mut text = format!("{} at {}", pretty_url(url), &commit[..commit.len().min(6)]);
            if path != "." {
                text.push_str(&format!(" in {path}"));
            }
            text
        }
        (
            Description::Git {
                url,
                reference,
                path,
                ..
            },
            None,
        ) => {
            let mut text = format!("{} at {reference}", pretty_url(url));
            if path != "." {
                text.push_str(&format!(" in {path}"));
            }
            text
        }
        (description, _) => description.format(&cwd),
    }
}

/// `p.prettyUri`: a `file:` URL is shown as a path relative to the
/// working directory when it is inside it.
fn pretty_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url)
        && parsed.scheme() == "file"
        && let Ok(path) = parsed.to_file_path()
    {
        let cwd = std::env::current_dir().unwrap_or_default();
        let relative = super::output::relative_path(&path, &cwd);
        return if relative.starts_with("..") {
            path.display().to_string()
        } else {
            relative
        };
    }
    url.to_owned()
}

fn write_id(id: &PackageId, output: &mut String) {
    output.push_str(&id.version.to_string());
    if !matches!(id.description, Description::Hosted(_)) {
        output.push_str(&format!(
            " from {} {}",
            id.description.source(),
            format_id_description(id)
        ));
    }
}

impl Report<'_> {
    fn status(&self, cache: &Cache, id: &PackageId) -> (bool, bool, Option<String>) {
        let Description::Hosted(url) = &id.description else {
            return (false, false, None);
        };
        match cache.hosted.fetched(&id.name, url) {
            Some(listing) => (
                listing
                    .versions
                    .iter()
                    .any(|l| l.version == id.version && l.retracted),
                listing.discontinued,
                listing.replaced_by.clone(),
            ),
            None => (false, false, None),
        }
    }

    fn report_package(
        &self,
        cache: &Cache,
        name: &str,
        output: &mut String,
        always_show: bool,
    ) -> bool {
        let new_id = self.new.packages.get(name);
        let old_id = self.previous.packages.get(name);
        let id = new_id.or(old_id).expect("a reported package exists");
        let overridden = self.overridden.contains(name);
        let mut changed = false;
        let mut added_or_removed = false;
        let icon = if overridden {
            "! "
        } else if let (None, _) | (_, None) = (new_id, old_id) {
            added_or_removed = true;
            if new_id.is_none() { "- " } else { "+ " }
        } else if let (Some(new), Some(old)) = (new_id, old_id) {
            if old.description != new.description {
                changed = true;
                "* "
            } else if old.version < new.version {
                changed = true;
                "> "
            } else if old.version > new.version {
                changed = true;
                "< "
            } else if old != new {
                changed = true;
                "~ "
            } else {
                "  "
            }
        } else {
            "  "
        };
        let mut note: Option<String> = None;
        if let Some(new_id) = new_id
            && self.kind != SolveType::Downgrade
        {
            let empty = Vec::new();
            let versions = self.available.get(&new_id.name).unwrap_or(&empty);
            let newer_stable = versions
                .iter()
                .any(|v| v > &new_id.version && !v.is_prerelease());
            let newer_unstable = versions
                .iter()
                .any(|v| v > &new_id.version && v.is_prerelease());
            let (retracted, discontinued, replaced_by) = self.status(cache, id);
            let best = || versions.iter().max_by(|a, b| prioritize(a, b));
            let newest = || versions.iter().max();
            let mut notes = Vec::new();
            if retracted {
                if newer_stable {
                    notes.push(format!("retracted, {} available", best().expect("newer")));
                } else if new_id.version.is_prerelease() && newer_unstable {
                    notes.push(format!("retracted, {} available", newest().expect("newer")));
                } else {
                    notes.push("retracted".into());
                }
            } else if discontinued && root_kind(self.root, name) != Kind::None {
                match replaced_by {
                    None => notes.push("discontinued".into()),
                    Some(other) => notes.push(format!("discontinued replaced by {other}")),
                }
            } else if newer_stable {
                notes.push(format!("{} available", best().expect("newer")));
            } else if new_id.version.is_prerelease() && newer_unstable {
                notes.push(format!("{} available", newest().expect("newer")));
            }
            if !notes.is_empty() {
                note = Some(format!("({})", notes.join(", ")));
            }
        }
        let lock_kind = |main: bool, dev: bool| {
            if main {
                Kind::Direct
            } else if dev {
                Kind::Dev
            } else {
                Kind::None
            }
        };
        let old_kind = lock_kind(
            self.previous.kinds.get(name).map(String::as_str) == Some("direct main"),
            self.previous.kinds.get(name).map(String::as_str) == Some("direct dev"),
        );
        let new_kind = lock_kind(self.new.main.contains(name), self.new.dev.contains(name));
        let kind_changed = old_id.is_some() && new_id.is_some() && old_kind != new_kind;
        let counted = changed || added_or_removed || kind_changed;
        if !(always_show || counted || note.is_some() || overridden) {
            return counted;
        }
        output.push_str(icon);
        output.push_str(&id.name);
        output.push(' ');
        write_id(id, output);
        if changed {
            output.push_str(" (was ");
            write_id(old_id.expect("changed"), output);
            output.push(')');
        }
        if kind_changed {
            output.push_str(&format!(
                " (from {} dependency to {} dependency)",
                kind_name(old_kind),
                kind_name(new_kind)
            ));
        }
        if overridden {
            let location = if self.overrides_file {
                format!(
                    " in {}",
                    Path::new(self.location)
                        .join("pubspec_overrides.yaml")
                        .display()
                )
            } else {
                String::new()
            };
            output.push_str(&format!(" (overridden{location})"));
        }
        if let Some(note) = note {
            output.push_str(&format!(" {note}"));
        }
        output.push('\n');
        counted
    }

    /// `SolveReport.show(summary: true)`; returns the number of changes.
    pub fn show(&self, cache: &Cache) -> usize {
        let mut output = String::new();
        let mut names: Vec<&String> = self
            .new
            .packages
            .keys()
            .filter(|n| **n != self.root.name)
            .collect();
        names.sort();
        let mut changes = 0;
        for name in &names {
            if self.report_package(cache, name, &mut output, false) {
                changes += 1;
            }
        }
        let removed: BTreeSet<&String> = self
            .previous
            .packages
            .keys()
            .filter(|n| !self.new.packages.contains_key(*n) && **n != self.root.name)
            .collect();
        if !removed.is_empty() {
            output.push_str("These packages are no longer being depended on:\n");
            for name in removed {
                self.report_package(cache, name, &mut output, true);
                changes += 1;
            }
        }
        if !self.quiet {
            message(&output);
        }
        self.check_content_hashes();
        self.summarize(cache, changes);
        changes
    }

    fn warning(&self, text: &str) {
        if !self.quiet {
            eprintln!("{}", text.strip_suffix('\n').unwrap_or(text));
        }
    }

    fn check_content_hashes(&self) {
        let mut issues = Vec::new();
        for (name, new_id) in &self.new.packages {
            let Some(old_id) = self.previous.packages.get(name) else {
                continue;
            };
            let (Description::Hosted(url), Description::Hosted(_)) =
                (&new_id.description, &old_id.description)
            else {
                continue;
            };
            if new_id.version != old_id.version {
                continue;
            }
            if let (Some(new_hash), Some(old_hash)) = (&new_id.resolved, &old_id.resolved)
                && new_hash != old_hash
            {
                issues.push(format!("{name}-{} from \"{url}\"", new_id.version));
            }
        }
        if !issues.is_empty() {
            let updated = if self.dry_run || self.enforce_lockfile {
                ""
            } else {
                "\nThe content-hashes in pubspec.lock has been updated."
            };
            self.warning(&format!(
                "The existing content-hash from pubspec.lock doesn't match contents for:\n * {}\n\nThis indicates one of:\n * The content has changed on the server since you created the pubspec.lock.\n * The pubspec.lock has been corrupted.\n{updated}\n\nFor more information see:\nhttps://dart.dev/go/content-hashes\n",
                issues.join("\n * ")
            ));
        }
    }

    fn summarize(&self, cache: &Cache, changes: usize) {
        let suffix = if self.location == "." {
            String::new()
        } else {
            format!(" in `{}`", self.location)
        };
        let plural = |n: usize| if n == 1 { "dependency" } else { "dependencies" };
        if self.quiet {
            if self.dry_run {
                message(&format!("Would get dependencies{suffix}."));
            } else if self.enforce_lockfile {
                if changes == 0 {
                    message(&format!("Got dependencies{suffix}."));
                }
            } else {
                message(&format!("Got dependencies{suffix}."));
            }
            return;
        }
        if self.dry_run || (self.enforce_lockfile && changes > 0) {
            if changes == 0 {
                message(&format!("No dependencies would change{suffix}."));
            } else {
                message(&format!(
                    "Would change {changes} {}{suffix}.",
                    plural(changes)
                ));
            }
        } else if self.enforce_lockfile || changes == 0 {
            if self.kind == SolveType::Get || self.enforce_lockfile {
                message(&format!("Got dependencies{suffix}!"));
            } else {
                message(&format!("No dependencies changed{suffix}."));
            }
        } else {
            message(&format!("Changed {changes} {}{suffix}!", plural(changes)));
        }
        let discontinued = self
            .new
            .packages
            .values()
            .filter(|id| self.status(cache, id).1 && root_kind(self.root, &id.name) != Kind::None)
            .count();
        match discontinued {
            0 => {}
            1 => message("1 package is discontinued."),
            n => message(&format!("{n} packages are discontinued.")),
        }
        let outdated = self
            .new
            .packages
            .values()
            .filter(|id| {
                self.available.get(&id.name).is_some_and(|versions| {
                    versions.iter().any(|v| {
                        v > &id.version && (id.version.is_prerelease() || !v.is_prerelease())
                    })
                })
            })
            .count();
        if outdated > 0 {
            let count = if outdated == 1 {
                "1 package has".to_owned()
            } else {
                format!("{outdated} packages have")
            };
            message(&format!(
                "{count} newer versions incompatible with dependency constraints.\nTry `dart pub outdated` for more information."
            ));
        }
    }
}
