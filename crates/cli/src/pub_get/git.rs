//! Git package sources with Pub 3.6's cache layout.
//!
//! Like Pub, each repository URL has a bare mirror in
//! `$PUB_CACHE/git/cache/<name>-<sha1(url)>`, and each resolved commit is a
//! checkout in `$PUB_CACHE/git/<name>-<commit>` whose `.git/pub-packages`
//! lists the package paths used from it. The `git` executable does the work;
//! DartForge never reimplements the Git protocol.
use super::{
    hosted::Candidate,
    manifest::{Manifest, yaml_json},
};
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Git {
    cache: PathBuf,
    offline: bool,
    /// Mirrors already created or fetched by this run (Pub's `updatedRepos`).
    updated: BTreeSet<PathBuf>,
}

/// Pub's `_repoName`: the URL's last segment without `.git`, sanitized and
/// shortened to 50 characters.
pub(super) fn repo_name(url: &str) -> String {
    let base = url.trim_end_matches('/').rsplit('/').next().unwrap_or(url);
    let base = base.strip_suffix(".git").unwrap_or(base);
    let name: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    name.chars().take(50).collect()
}

fn run_git(args: &[&str], working: Option<&Path>) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(working) = working {
        command.current_dir(working);
    }
    let output = command.output().map_err(|error| {
        format!("Cannot run git ({error}).\nPlease ensure Git is correctly installed.")
    })?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

impl Git {
    pub fn new(cache: PathBuf, offline: bool) -> Self {
        Self {
            cache,
            offline,
            updated: BTreeSet::new(),
        }
    }

    fn mirror(&self, url: &str) -> PathBuf {
        let hash: String = Sha1::digest(url.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.cache
            .join("git/cache")
            .join(format!("{}-{hash}", repo_name(url)))
    }

    /// The checkout of `commit`, the directory that holds the package files.
    pub fn checkout(&self, url: &str, commit: &str) -> PathBuf {
        self.cache
            .join("git")
            .join(format!("{}-{commit}", repo_name(url)))
    }

    fn temporary(&self) -> Result<PathBuf, String> {
        let directory = self.cache.join("_temp").join(format!(
            "dartforge-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        Ok(directory)
    }

    /// `git clone` into a temporary directory, then a rename into place.
    fn clone_via_temp(&self, from: &str, to: &Path, mirror: bool) -> Result<(), String> {
        let temporary = self.temporary()?;
        let target = temporary.to_string_lossy().into_owned();
        let mode = if mirror { "--mirror" } else { "--no-checkout" };
        if let Err(error) = run_git(&["clone", mode, from, &target], None) {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(error);
        }
        std::fs::create_dir_all(to.parent().ok_or("git cache has no parent")?)
            .map_err(|e| e.to_string())?;
        if std::fs::rename(&temporary, to).is_err() {
            // Another process installed the same directory meanwhile.
            let _ = std::fs::remove_dir_all(&temporary);
            if !to.is_dir() {
                return Err(format!("could not install {}", to.display()));
            }
        }
        Ok(())
    }

    fn git_dir(mirror: &Path) -> String {
        format!("--git-dir={}", mirror.display())
    }

    /// `_ensureRepoCache`: create the mirror, or fetch it once per run.
    fn ensure_repository(&mut self, url: &str) -> Result<PathBuf, String> {
        let mirror = self.mirror(url);
        if self.updated.contains(&mirror) {
            return Ok(mirror);
        }
        if !mirror.is_dir() {
            if self.offline {
                return Err(format!(
                    "offline: the git repository {url} is not in the package cache"
                ));
            }
            self.clone_via_temp(url, &mirror, true)?;
        } else if !self.offline {
            run_git(&[&Self::git_dir(&mirror), "fetch"], Some(&mirror))?;
        }
        self.updated.insert(mirror.clone());
        Ok(mirror)
    }

    /// `_ensureRevision`: a locked commit only needs a fetch when missing.
    fn ensure_revision(&mut self, url: &str, commit: &str) -> Result<PathBuf, String> {
        let mirror = self.mirror(url);
        if self.updated.contains(&mirror) || !mirror.is_dir() {
            return self.ensure_repository(url);
        }
        if first_revision(&mirror, commit).is_err() {
            if self.offline {
                return Err(format!(
                    "offline: commit {commit} of {url} is not in the package cache"
                ));
            }
            run_git(&[&Self::git_dir(&mirror), "fetch"], Some(&mirror))?;
            self.updated.insert(mirror.clone());
        }
        Ok(mirror)
    }

    /// The single candidate of a Git dependency: the locked commit when the
    /// description is unchanged, otherwise the first revision of `reference`.
    pub fn candidate(
        &mut self,
        name: &str,
        url: &str,
        reference: &str,
        path: &str,
        locked: Option<&str>,
        host: &str,
    ) -> Result<Candidate, String> {
        let (mirror, commit) = match locked {
            Some(commit) => (self.ensure_revision(url, commit)?, commit.to_owned()),
            None => {
                let mirror = self.ensure_repository(url)?;
                let commit = first_revision(&mirror, reference)?;
                (mirror, commit)
            }
        };
        let file = if path == "." {
            "pubspec.yaml".to_owned()
        } else {
            format!("{path}/pubspec.yaml")
        };
        let text = run_git(
            &[&Self::git_dir(&mirror), "show", &format!("{commit}:{file}")],
            Some(&mirror),
        )
        .map_err(|_| format!("Could not find a file named \"{file}\" in {url} {commit}."))?;
        let manifest = Manifest::parse(&yaml_json(&text)?, None, host)?;
        if manifest.name != name {
            return Err(format!(
                "\"name\" field doesn't match expected name \"{name}\" in {url} {commit}"
            ));
        }
        let checkout = self.checkout(url, &commit);
        Ok(Candidate {
            manifest,
            manifest_error: None,
            directory: if path == "." {
                checkout
            } else {
                checkout.join(path)
            },
            archive: None,
            // The resolved commit identifies the content, as an archive hash does.
            digest: Some(commit),
        })
    }

    /// `downloadToSystemCache`: the commit's checkout and its package list.
    pub fn materialize(&mut self, url: &str, path: &str, commit: &str) -> Result<(), String> {
        let checkout = self.checkout(url, commit);
        let list = checkout.join(".git/pub-packages");
        if checkout.is_dir() {
            let mut packages: Vec<String> = match std::fs::read_to_string(&list) {
                Ok(text) => text.split('\n').map(str::to_owned).collect(),
                Err(_) => vec![".".into()],
            };
            if !packages.iter().any(|p| p == path) {
                packages.push(path.into());
                std::fs::write(&list, packages.join("\n")).map_err(|e| e.to_string())?;
            }
            return Ok(());
        }
        let mirror = self.ensure_revision(url, commit)?;
        let temporary = self.temporary()?;
        let target = temporary.to_string_lossy().into_owned();
        let result = (|| {
            run_git(
                &["clone", "--no-checkout", &mirror.to_string_lossy(), &target],
                None,
            )?;
            run_git(&["config", "remote.origin.lfsurl", url], Some(&temporary))?;
            run_git(&["checkout", commit], Some(&temporary))?;
            std::fs::write(temporary.join(".git/pub-packages"), path).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&temporary);
            return Err(error);
        }
        if std::fs::rename(&temporary, &checkout).is_err() {
            let _ = std::fs::remove_dir_all(&temporary);
            if !checkout.is_dir() {
                return Err(format!("could not install {}", checkout.display()));
            }
        }
        Ok(())
    }
}

/// `git rev-list --max-count=1 <reference>` in the mirror.
fn first_revision(mirror: &Path, reference: &str) -> Result<String, String> {
    let output = run_git(
        &[
            &Git::git_dir(mirror),
            "rev-list",
            "--max-count=1",
            reference,
        ],
        Some(mirror),
    )
    .map_err(|error| format!("Could not find git ref '{reference}' ({error})"))?;
    let commit = output.trim();
    if commit.is_empty() {
        return Err(format!("Could not find git ref '{reference}'."));
    }
    Ok(commit.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_names_follow_pub() {
        assert_eq!(
            repo_name("https://github.com/insinfo/pdfbox_dart"),
            "pdfbox_dart"
        );
        assert_eq!(repo_name("https://example.com/a/b.git"), "b");
        assert_eq!(repo_name("git@github.com:org/my repo+x.git"), "my_repo_x");
        assert_eq!(
            repo_name(&format!("https://h/{}", "x".repeat(60))).len(),
            50
        );
        let git = Git::new(PathBuf::from("/cache"), true);
        // The mirror name Pub 3.6.2 used for this URL in a real cache.
        assert_eq!(
            git.mirror("https://github.com/insinfo/pdfbox_dart"),
            PathBuf::from("/cache/git/cache/pdfbox_dart-289981f427ef66c1195e5ee25655769f6fcda882")
        );
    }
}
