//! The files of a package as Pub publishes them (`Package.listFiles`), the
//! `git` queries it depends on, and the tree `ascii_tree.fromFiles` draws
//! of them.
use super::ignore::{Ignore, list_files as walk};
use super::paths as p;
use super::terminal;
use crate::pub_get::{Failure, data};
use std::process::Command;
use std::rc::Rc;
use std::sync::OnceLock;

/// The `git` executable Pub would use (`git.command`): the first of `git`
/// and `git.cmd` whose `--version` prints `git version x.y`.
pub(crate) fn git_command() -> Option<&'static str> {
    static COMMAND: OnceLock<Option<&'static str>> = OnceLock::new();
    *COMMAND.get_or_init(|| {
        ["git", "git.cmd"].into_iter().find(|command| {
            Command::new(command)
                .arg("--version")
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .is_some_and(|out| {
                    out.lines().next().is_some_and(|line| {
                        line.strip_prefix("git version ").is_some_and(|rest| {
                            let mut parts = rest.split('.');
                            let major = parts.next().is_some_and(|m| {
                                !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit())
                            });
                            let minor = parts.next().is_some_and(|m| {
                                !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit())
                            });
                            major && minor && parts.next().is_some()
                        })
                    })
                })
        })
    })
}

/// Runs `git args` in `dir`; the output when it succeeds.
pub(crate) fn run_git(args: &[&str], dir: &str) -> Option<Vec<u8>> {
    let command = git_command()?;
    let output = Command::new(command)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

/// `git.repoRoot(dir)`.
pub(crate) fn repo_root(dir: &str) -> Option<String> {
    let out = run_git(&["rev-parse", "--show-toplevel"], dir)?;
    Some(p::normalize(String::from_utf8_lossy(&out).trim()))
}

/// `Package.inGitRepo`: `git check-ignore --quiet .` exits with 1 (the
/// directory is in a repository and not ignored).
pub(crate) fn in_git_repo(dir: &str) -> bool {
    let Some(command) = git_command() else {
        return false;
    };
    Command::new(command)
        .args(["check-ignore", "--quiet", "."])
        .current_dir(dir)
        .output()
        .is_ok_and(|o| o.status.code() == Some(1))
}

/// Whether ignore rules match without case on this platform (Pub follows
/// `core.ignoreCase`, which `git init` sets on macOS and Windows).
pub(crate) const IGNORE_CASE: bool = cfg!(any(target_os = "macos", windows));

/// The path of a child as `Directory.listSync` builds it.
fn child_path(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

/// The names in a directory in the order the file system lists them.
pub(crate) fn read_dir_names(dir: &str) -> Result<Vec<String>, Failure> {
    let entries = std::fs::read_dir(dir).map_err(|e| Failure {
        code: crate::pub_get::exit::NO_INPUT,
        message: format!(
            "FileSystemException: Directory listing failed, path = '{dir}' ({})",
            os_error(&e)
        ),
    })?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    Ok(names)
}

/// The `OS Error: <message>, errno = <n>` part of a Dart I/O exception.
pub(crate) fn os_error(e: &std::io::Error) -> String {
    let message = e.to_string();
    let message = message
        .rsplit_once(" (os error")
        .map_or(message.as_str(), |(m, _)| m);
    match e.raw_os_error() {
        Some(code) => format!("OS Error: {message}, errno = {code}"),
        None => message.to_owned(),
    }
}

pub(crate) fn file_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

pub(crate) fn dir_exists(path: &str) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_dir())
}

pub(crate) fn link_exists(path: &str) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// `_basicIgnoreRules` of `Package`.
const BASIC_RULES: [&str; 5] = [
    ".*",
    "!.htaccess",
    "pubspec.lock",
    "!pubspec.lock/",
    "/pubspec_overrides.yaml",
];

/// `Package.listFiles(includeDirs:)` for the package in `package_dir` (as
/// Pub spells it: `.` or the `--directory` argument): every file (and
/// directory) of the package, from the repository root when it is in a git
/// repository, with the ignore files of every directory (`.pubignore` wins
/// over `.gitignore`) and Pub's base rules at the package root.
pub(crate) fn list_files(package_dir: &str, include_dirs: bool) -> Result<Vec<String>, Failure> {
    let root = repo_root(package_dir).unwrap_or_else(|| package_dir.to_owned());
    let mut beneath = p::to_uri_path(&p::normalize(&p::relative(
        &p::join(package_dir, "."),
        &root,
    )));
    if beneath == "./" {
        beneath = ".".into();
    }
    let resolve = |path: &str| p::join(&root, path);
    let mut list_dir = |dir: &str| -> Result<Vec<String>, Failure> {
        let directory = resolve(dir);
        let mut result = Vec::new();
        for name in read_dir_names(&directory)? {
            let entity = child_path(&directory, &name);
            if link_exists(&entity) {
                let target = std::fs::read_link(&entity)
                    .map(|t| t.display().to_string())
                    .unwrap_or_default();
                if dir_exists(&entity) {
                    return Err(data(format!(
                        "Pub does not support publishing packages with directory symlinks: `{entity}`."
                    )));
                }
                if !file_exists(&entity) {
                    return Err(data(format!(
                        "Pub does not support publishing packages with non-resolving symlink: `{entity}` => `{target}`."
                    )));
                }
            }
            result.push(p::relative(&entity, &root));
        }
        Ok(result)
    };
    let mut ignore_for_dir = |dir: &str| -> Option<Rc<Ignore>> {
        let pub_ignore = resolve(&format!("{dir}/.pubignore"));
        let git_ignore = resolve(&format!("{dir}/.gitignore"));
        let file = if file_exists(&pub_ignore) {
            Some(pub_ignore)
        } else if file_exists(&git_ignore) {
            Some(git_ignore)
        } else {
            None
        };
        let mut rules: Vec<String> = Vec::new();
        if dir == beneath {
            rules.extend(BASIC_RULES.iter().map(|r| (*r).to_owned()));
        }
        if let Some(file) = &file {
            rules.push(read_text(file));
        }
        if rules.is_empty() {
            return None;
        }
        let shown = file.clone().unwrap_or_else(|| "null".into());
        Some(Rc::new(Ignore::new(
            &rules,
            IGNORE_CASE,
            |pattern, message| {
                terminal::warning(&format!("{shown} had invalid pattern {pattern}. {message}"));
            },
        )))
    };
    let mut is_dir = |dir: &str| dir_exists(&resolve(dir));
    let files = walk(
        &beneath,
        &mut list_dir,
        &mut ignore_for_dir,
        &mut is_dir,
        include_dirs,
    )?;
    Ok(files.iter().map(|f| resolve(f)).collect())
}

/// `readTextFile`: the file as UTF-8 (malformed sequences replaced).
pub(crate) fn read_text(path: &str) -> String {
    std::fs::read(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// `listDir(dir)` of Pub's `io.dart` is empty: no entry that is not hidden
/// and not a broken link.
pub(crate) fn dir_is_empty(dir: &str) -> bool {
    let Ok(names) = read_dir_names(dir) else {
        return true;
    };
    !names.iter().any(|name| {
        let path = child_path(dir, name);
        let broken_link = link_exists(&path) && std::fs::metadata(&path).is_err();
        !broken_link && !name.starts_with('.')
    })
}

/// Dart's `String.compareTo`: by UTF-16 code units.
pub(crate) fn compare_utf16(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The size label of `ascii_tree._readableFileSize`.
fn readable_size(size: i64) -> String {
    if size >= 1 << 30 {
        terminal::red(&format!("({} GB)", size / (1 << 30)))
    } else if size >= 1 << 20 {
        terminal::yellow(&format!("({} MB)", size / (1 << 20)))
    } else if size >= 1 << 10 {
        terminal::gray(&format!("({} KB)", size / (1 << 10)))
    } else {
        terminal::gray("(<1 KB)")
    }
}

/// A node of the tree: children by name, in insertion order.
#[derive(Default)]
struct Node {
    children: Vec<(String, Node)>,
}

impl Node {
    fn child(&mut self, name: &str) -> &mut Node {
        let index = match self.children.iter().position(|(n, _)| n == name) {
            Some(index) => index,
            None => {
                self.children.push((name.to_owned(), Node::default()));
                self.children.len() - 1
            }
        };
        &mut self.children[index].1
    }
}

/// `ascii_tree.fromFiles(files, baseDir:, showFileSizes: true)`.
pub(crate) fn tree(files: &[String], base_dir: &str) -> String {
    let mut root = Node::default();
    for file in files {
        let relative = p::relative(file, base_dir);
        let mut parts = p::split(&relative);
        let size = match std::fs::metadata(p::normalize(file)) {
            Ok(m) if m.is_dir() => None,
            Ok(m) => Some(m.len() as i64),
            Err(_) => Some(-1),
        };
        if let (Some(size), Some(last)) = (size, parts.last_mut()) {
            *last = format!("{last} {}", readable_size(size));
        }
        let mut node = &mut root;
        for part in &parts {
            node = node.child(part);
        }
    }
    let mut out = String::new();
    draw(&mut out, "", None, &root, false, 1);
    out
}

fn draw(
    out: &mut String,
    prefix: &str,
    name: Option<&str>,
    node: &Node,
    is_last: bool,
    depth: usize,
) {
    if let Some(name) = name {
        out.push_str(prefix);
        if depth > 1 {
            out.push_str(&terminal::gray(if is_last {
                "└── "
            } else {
                "├── "
            }));
        }
        out.push_str(name);
        out.push('\n');
    }
    let mut names: Vec<&(String, Node)> = node.children.iter().collect();
    names.sort_by(|a, b| compare_utf16(&a.0, &b.0));
    let child_prefix = if depth <= 1 {
        String::new()
    } else if is_last {
        "    ".to_owned()
    } else {
        terminal::gray("│   ")
    };
    let count = names.len();
    for (i, (child, grand)) in names.into_iter().enumerate() {
        draw(
            out,
            &format!("{prefix}{child_prefix}"),
            Some(child),
            grand,
            i == count - 1,
            depth + 1,
        );
    }
}
