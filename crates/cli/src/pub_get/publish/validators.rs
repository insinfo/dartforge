//! The 25 validators of Pub 3.6.2's `Validator.runAll`, in its order, and
//! the report it prints (`validator.dart` and `validator/*.dart`).
use super::files::{self, compare_utf16, dir_exists, file_exists, link_exists, read_text};
use super::paths as p;
use super::{Package, terminal};
use crate::pub_get::{
    Failure, Settings,
    manifest::{Dependency, Description, language_of},
    source::{LockFile, SourceError},
    tree::Node,
    version::{Range, Version},
};
use std::collections::BTreeSet;

/// What the validators found.
#[derive(Default)]
pub(crate) struct Report {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub hints: Vec<String>,
}

fn bullets(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("* {}", item.split('\n').collect::<Vec<_>>().join("\n  ")))
        .collect::<Vec<_>>()
        .join("\n")
}

impl Report {
    /// The end of `runAll`: errors with `log.error`, warnings and hints with
    /// `log.warning` (the final empty line of each list prints nothing).
    pub fn print(&self) {
        let plural = |n: usize| if n > 1 { "s" } else { "" };
        if !self.errors.is_empty() {
            terminal::error(&format!(
                "Package validation found the following error{}:",
                plural(self.errors.len())
            ));
            terminal::error(&bullets(&self.errors));
        }
        if !self.warnings.is_empty() {
            terminal::warning(&format!(
                "Package validation found the following potential issue{}:",
                plural(self.warnings.len())
            ));
            terminal::warning(&bullets(&self.warnings));
        }
        if !self.hints.is_empty() {
            terminal::warning(&format!(
                "Package validation found the following hint{}:",
                plural(self.hints.len())
            ));
            terminal::warning(&bullets(&self.hints));
        }
    }
}

/// The findings of one validator.
#[derive(Default)]
pub(super) struct Found {
    errors: Vec<String>,
    warnings: Vec<String>,
    hints: Vec<String>,
}

/// `ValidationContext`.
pub(super) struct Context<'a> {
    pub package: &'a Package<'a>,
    pub settings: &'a Settings,
    pub size: usize,
    pub server: &'a str,
    pub files: &'a [String],
}

impl Context<'_> {
    /// `Validator.filesBeneath`.
    fn files_beneath(&self, path: &str, recursive: bool) -> Vec<String> {
        let base = p::canonicalize(&p::join(&self.package.dir, path));
        self.files
            .iter()
            .filter(|file| {
                if recursive {
                    let file = p::canonicalize(file);
                    p::is_within(&base, &file) || file == base
                } else {
                    p::canonicalize(&p::dirname(file)) == base
                }
            })
            .cloned()
            .collect()
    }

    pub fn fields(&self) -> &Node {
        &self.package.fields
    }

    fn name(&self) -> &str {
        &self.package.manifest().name
    }

    fn version(&self) -> &Version {
        &self.package.manifest().version
    }
}

/// `pluralize(word, count)`.
fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// Dart's `toString` of a YAML value, for the messages that print one.
pub(crate) fn dart_string(node: &Node) -> String {
    match node {
        Node::Null => "null".into(),
        Node::Bool(b) => b.to_string(),
        Node::Number(n) => n.clone(),
        Node::String(s) => s.clone(),
        Node::List(items) => format!(
            "[{}]",
            items.iter().map(dart_string).collect::<Vec<_>>().join(", ")
        ),
        Node::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{k}: {}", dart_string(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// One validator.
type Validate = fn(&Context<'_>, &mut Found) -> Result<(), Failure>;

/// Runs every validator and collects their findings in Pub's order.
pub(crate) fn run_all(
    package: &Package<'_>,
    settings: &Settings,
    size: usize,
    server: &str,
    files: &[String],
) -> Result<Report, Failure> {
    let context = Context {
        package,
        settings,
        size,
        server,
        files,
    };
    let validators: [Validate; 25] = [
        file_case,
        analyze,
        gitignore,
        git_status,
        pubspec,
        license,
        name,
        pubspec_field,
        dependency,
        dependency_override,
        deprecated_fields,
        directory,
        executable,
        compiled_dartdoc,
        readme,
        changelog,
        sdk_constraint,
        strict_dependencies,
        flutter_constraint,
        flutter_plugin_format,
        relative_version_numbering,
        pubspec_typo,
        super::leaks::validate,
        size_validator,
        devtools_extension,
    ];
    let mut report = Report::default();
    let mut failure = None;
    let mut found_all = Vec::new();
    for validator in validators {
        let mut found = Found::default();
        if let Err(error) = validator(&context, &mut found)
            && failure.is_none()
        {
            failure = Some(error);
        }
        found_all.push(found);
    }
    if let Some(failure) = failure {
        return Err(failure);
    }
    for found in &mut found_all {
        report.hints.append(&mut found.hints);
    }
    for found in &mut found_all {
        report.warnings.append(&mut found.warnings);
    }
    for found in &mut found_all {
        report.errors.append(&mut found.errors);
    }
    Ok(report)
}

impl Found {
    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }
    fn warning(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
    fn hint(&mut self, message: impl Into<String>) {
        self.hints.push(message.into());
    }
}

// ---- FileCaseValidator ----

fn file_case(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let mut sorted: Vec<&String> = c.files.iter().collect();
    sorted.sort_by(|a, b| compare_utf16(a, b));
    let mut seen: std::collections::HashMap<String, &String> = std::collections::HashMap::new();
    for file in sorted {
        let lower = file.to_lowercase();
        if let Some(existing) = seen.get(&lower) {
            found.error(format!(
                "The file {file} and {existing} only differ in capitalization.\n\nThis is not supported across platforms.\n\nTry renaming one of them.\n"
            ));
            break;
        }
        seen.insert(lower, file);
    }
    Ok(())
}

// ---- AnalyzeValidator ----

/// `limitLength` of Pub's `log.dart` (in UTF-16 code units).
fn limit_length(input: &str, limit: usize) -> String {
    let units: Vec<u16> = input.encode_utf16().collect();
    if units.len() <= limit {
        return input.to_owned();
    }
    let snip = "[...]";
    let half = (limit - snip.len()) / 2;
    let extra = (limit - snip.len()) % 2;
    format!(
        "{}{snip}{}",
        String::from_utf16_lossy(&units[..half + extra]),
        String::from_utf16_lossy(&units[units.len() - half..])
    )
}

fn analyze(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let entries: Vec<String> = ["bin", "lib", "build.dart", "link.dart"]
        .iter()
        .map(|e| p::join(&c.package.dir, e))
        .filter(|e| file_exists(e) || dir_exists(e) || link_exists(e))
        .collect();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new(&exe);
    command.arg("analyze");
    command.args(&entries);
    command.arg(p::join(&c.package.dir, "pubspec.yaml"));
    if let Some(root) = &c.settings.sdk_root {
        command.env("DARTFORGE_SDK_LIB", root.join("lib"));
    }
    let output = command.output().map_err(|e| {
        Failure::from(format!(
            "Pub failed to run subprocess `{}`: ProcessException: {e}",
            exe.display()
        ))
    })?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        found.warning(format!(
            "`dart analyze` found the following issue(s):\n{}",
            limit_length(&stdout, 1000)
        ));
    }
    Ok(())
}

// ---- GitignoreValidator ----

fn split_zero_terminated(output: &[u8], skip: usize) -> Option<Vec<&[u8]>> {
    let mut result = Vec::new();
    let mut start = 0;
    for (i, &b) in output.iter().enumerate() {
        if b != 0 {
            continue;
        }
        if start + skip > i {
            return None;
        }
        result.push(&output[start + skip..i]);
        start = i + 1;
    }
    Some(result)
}

fn gitignore(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let dir = &c.package.dir;
    if !files::in_git_repo(dir) {
        return Ok(());
    }
    let Some(output) = files::run_git(
        &[
            "-c",
            "core.quotePath=false",
            "ls-files",
            "-z",
            "--cached",
            "--exclude-standard",
            "--recurse-submodules",
        ],
        dir,
    ) else {
        return Ok(());
    };
    let Some(parts) = split_zero_terminated(&output, 0) else {
        return Ok(());
    };
    let mut checked_in = Vec::new();
    for part in parts {
        match std::str::from_utf8(part) {
            Ok(s) => checked_in.push(s.to_owned()),
            Err(_) => return Ok(()),
        }
    }
    let root = files::repo_root(dir).unwrap_or_else(|| dir.clone());
    let mut beneath = p::split(&p::normalize(&p::relative(dir, &root))).join("/");
    if beneath == "./" {
        beneath = String::new();
    }
    let resolve = |path: &str| p::join(&root, path);
    let mut list_dir = |d: &str| -> Result<Vec<String>, Failure> {
        let directory = resolve(d);
        let mut out = Vec::new();
        for name in files::read_dir_names(&directory)? {
            let entity = if directory.ends_with('/') {
                format!("{directory}{name}")
            } else {
                format!("{directory}/{name}")
            };
            if link_exists(&entity) && dir_exists(&entity) {
                continue;
            }
            out.push(p::split(&p::relative(&entity, &root)).join("/"));
        }
        Ok(out)
    };
    let mut ignore_for_dir = |d: &str| {
        let path = resolve(&format!("{d}/.gitignore"));
        file_exists(&path).then(|| {
            std::rc::Rc::new(super::ignore::Ignore::new(
                &[read_text(&path)],
                false,
                |_, _| {},
            ))
        })
    };
    let mut is_dir = |d: &str| dir_exists(&resolve(d));
    let unignored: BTreeSet<String> = super::ignore::list_files(
        &beneath,
        &mut list_dir,
        &mut ignore_for_dir,
        &mut is_dir,
        false,
    )?
    .iter()
    .map(|f| p::relative(&resolve(f), dir))
    .collect();
    let ignored: Vec<&String> = checked_in
        .iter()
        .filter(|f| !unignored.contains(*f))
        .collect();
    if !ignored.is_empty() {
        let n = ignored.len();
        found.warning(format!(
            "{n} checked-in {} {} ignored by a `.gitignore`.\nPrevious versions of Pub would include those in the published package.\n\nConsider adjusting your `.gitignore` files to not ignore those files, and if you do not wish to\npublish these files use `.pubignore`. See also dart.dev/go/pubignore\n\nFiles that are checked in while gitignored:\n\n{}\n{}\n",
            pluralize("file", n),
            if n == 1 { "is" } else { "are" },
            ignored
                .iter()
                .take(10)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            if n > 10 { "..." } else { "" }
        ));
    }
    Ok(())
}

// ---- GitStatusValidator ----

fn git_status(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let dir = &c.package.dir;
    if !files::in_git_repo(dir) {
        return Ok(());
    }
    let Some(root) = files::repo_root(dir) else {
        return Ok(());
    };
    let Some(output) = files::run_git(
        &["status", "-z", "--no-renames", "--untracked-files=no"],
        dir,
    ) else {
        return Ok(());
    };
    let Some(parts) = split_zero_terminated(&output, 3) else {
        return Ok(());
    };
    let mut modified = Vec::new();
    for part in parts {
        let Ok(name) = std::str::from_utf8(part) else {
            continue;
        };
        let full = p::join(&root, name);
        if !c.files.iter().any(|f| p::equals(&full, f)) {
            continue;
        }
        modified.push(p::relative_to_current(&full));
    }
    if !modified.is_empty() {
        let n = modified.len();
        found.warning(format!(
            "{n} checked-in {} {} modified in git.\n\nUsually you want to publish from a clean git state.\n\nConsider committing these files or reverting the changes.\n\nModified files:\n\n{}\n{}\nRun `git status` for more information.\n",
            pluralize("file", n),
            if n == 1 { "is" } else { "are" },
            modified
                .iter()
                .take(10)
                .map(|f| p::relative_to_current(f))
                .collect::<Vec<_>>()
                .join("\n"),
            if n > 10 { "...\n" } else { "" }
        ));
    }
    Ok(())
}

// ---- PubspecValidator, LicenseValidator, NameValidator ----

fn pubspec(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    if !c
        .files_beneath(".", false)
        .iter()
        .any(|f| p::basename(f) == "pubspec.yaml")
    {
        found.error("The pubspec is hidden, probably by .gitignore or pubignore.");
    }
    Ok(())
}

/// `^(([a-zA-Z0-9]+[-_])?(LICENSE|COPYING)|UNLICENSE)(\..*)?$`.
fn license_like(name: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"^(([a-zA-Z0-9]+[-_])?(LICENSE|COPYING)|UNLICENSE)(\.[^\n\r\u{2028}\u{2029}]*)?$",
        )
        .expect("license pattern")
    })
    .is_match(name)
}

fn license(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let candidates: Vec<String> = c
        .files_beneath(".", false)
        .into_iter()
        .filter(|f| license_like(&p::basename(f)))
        .collect();
    if let Some(first) = candidates.first() {
        if !candidates.iter().any(|f| p::basename(f) == "LICENSE") {
            found.warning(format!(
                "Please consider renaming {first} to `LICENSE`. See https://dart.dev/tools/pub/publishing#important-files."
            ));
        }
        return Ok(());
    }
    found.error(
        "You must have a LICENSE file in the root directory.\nAn open-source license helps ensure people can legally use your code.",
    );
    Ok(())
}

const RESERVED_WORDS: [&str; 59] = [
    "abstract",
    "as",
    "assert",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "covariant",
    "default",
    "deferred",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "external",
    "false",
    "final",
    "finally",
    "for",
    "hide",
    "if",
    "implements",
    "import",
    "in",
    "inline",
    "interface",
    "is",
    "late",
    "library",
    "mixin",
    "new",
    "null",
    "of",
    "on",
    "operator",
    "part",
    "required",
    "rethrow",
    "return",
    "sealed",
    "set",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "var",
    "void",
    "while",
    "with",
    "yield",
    "",
    "",
];

fn un_camel_case(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::new();
    let mut last = 0;
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i].is_ascii_lowercase() && chars[i + 1].is_ascii_uppercase() {
            out.extend(&chars[last..=i]);
            out.push('_');
            out.push(chars[i + 1].to_ascii_lowercase());
            last = i + 2;
            i += 2;
            continue;
        }
        i += 1;
    }
    out.extend(&chars[last..]);
    out.to_lowercase()
}

fn name(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let name = c.name();
    let description = format!("Package name \"{name}\"");
    let identifier_chars = name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if name.is_empty() {
        found.error(format!("{description} may not be empty."));
    } else if !identifier_chars {
        found.error(format!(
            "{description} may only contain letters, numbers, and underscores.\nUsing a valid Dart identifier makes the name usable in Dart code."
        ));
    } else if !name.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_') {
        found.error(format!(
            "{description} must begin with a letter or underscore.\nUsing a valid Dart identifier makes the name usable in Dart code."
        ));
    } else if RESERVED_WORDS
        .iter()
        .any(|w| !w.is_empty() && *w == name.to_lowercase())
    {
        found.error(format!(
            "{description} may not be a reserved word in Dart.\nUsing a valid Dart identifier makes the name usable in Dart code."
        ));
    } else if name.chars().any(|ch| ch.is_ascii_uppercase()) {
        found.warning(format!(
            "{description} should be lower-case. Maybe use \"{}\"?",
            un_camel_case(name)
        ));
    }
    let lib_dir = p::join(&c.package.dir, "lib");
    let libraries: Vec<String> = c
        .files_beneath("lib", true)
        .iter()
        .map(|f| p::relative(f, &p::dirname(&lib_dir)))
        .filter(|f| !p::split(f).iter().any(|s| s == "src") && p::extension(f) == ".dart")
        .collect();
    if let [library] = libraries.as_slice() {
        let lib_name = p::basename_without_extension(library);
        if lib_name != name {
            found.warning(format!(
                "The name of \"{library}\", \"{lib_name}\", should match the name of the package, \"{name}\".\nThis helps users know what library to import."
            ));
        }
    }
    Ok(())
}

// ---- PubspecFieldValidator ----

/// The lazily checked fields of `Pubspec.allErrors` that loading the
/// workspace does not already reject: `executables`, `false_secrets` and
/// `ignored_advisories`.
fn lazy_errors(fields: &Node) -> Vec<String> {
    let mut errors = Vec::new();
    if let Err(e) = executables(fields) {
        errors.push(e);
    }
    if let Err(e) = false_secrets(fields) {
        errors.push(e.message);
    }
    if let Err(e) = ignored_advisories(fields) {
        errors.push(e);
    }
    errors
}

/// `Pubspec.executables`, and the entries read before an error (Pub keeps
/// them, so a second read does not fail).
fn executables_with_partial(fields: &Node) -> (Vec<(String, String)>, Option<String>) {
    let mut result = Vec::new();
    let yaml = fields.get("executables");
    match yaml {
        Node::Null => (result, None),
        Node::Map(entries) => {
            for (key, value) in entries {
                if !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    || key.is_empty()
                {
                    return (
                        result,
                        Some(
                            "\"executables\" keys may only contain letters, numbers, hyphens and underscores."
                                .into(),
                        ),
                    );
                }
                match value {
                    Node::Null => result.push((key.clone(), key.clone())),
                    Node::String(s) if s.contains(['/', '\\']) => {
                        return (
                            result,
                            Some("\"executables\" values may not contain path separators.".into()),
                        );
                    }
                    Node::String(s) => result.push((key.clone(), s.clone())),
                    _ => {
                        return (
                            result,
                            Some("\"executables\" values must be strings or null.".into()),
                        );
                    }
                }
            }
            (result, None)
        }
        _ => (result, Some("\"executables\" field must be a map.".into())),
    }
}

fn executables(fields: &Node) -> Result<Vec<(String, String)>, String> {
    match executables_with_partial(fields) {
        (result, None) => Ok(result),
        (_, Some(error)) => Err(error),
    }
}

/// `Pubspec.falseSecrets`.
pub(crate) fn false_secrets(fields: &Node) -> Result<Vec<String>, Failure> {
    let error = || {
        crate::pub_get::data("\"false_secrets\" field must be a list of git-ignore style patterns")
    };
    match fields.get("false_secrets") {
        Node::Null => Ok(Vec::new()),
        Node::List(items) => items
            .iter()
            .map(|n| match n {
                Node::String(s) => Ok(s.clone()),
                _ => Err(error()),
            })
            .collect(),
        _ => Err(error()),
    }
}

fn ignored_advisories(fields: &Node) -> Result<(), String> {
    let message = || "\"ignored_advisories\" field must be a list of advisory IDs".to_owned();
    match fields.get("ignored_advisories") {
        Node::Null => Ok(()),
        Node::List(items) => {
            if items.iter().all(|n| matches!(n, Node::String(_))) {
                Ok(())
            } else {
                Err(message())
            }
        }
        _ => Err(message()),
    }
}

fn pubspec_field(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let fields = c.fields();
    let field_is_string = |found: &mut Found, field: &str| match fields.get(field) {
        Node::Null => found.error(format!("Your pubspec.yaml is missing a \"{field}\" field.")),
        Node::String(_) => {}
        value => found.error(format!(
            "Your pubspec.yaml's \"{field}\" field must be a string, but it was \"{}\".",
            dart_string(value)
        )),
    };
    let field_url = |found: &mut Found, field: &str| match fields.get(field) {
        Node::Null => {}
        Node::String(url) => {
            if !(url.starts_with("http:") || url.starts_with("https:")) {
                found.error(format!(
                    "Your pubspec.yaml's \"{field}\" field must be an \"http:\" or \"https:\" URL, but it was \"{url}\"."
                ));
            }
        }
        value => found.error(format!(
            "Your pubspec.yaml's \"{field}\" field must be a string, but it was \"{}\".",
            dart_string(value)
        )),
    };
    field_is_string(found, "description");
    field_url(found, "homepage");
    field_url(found, "repository");
    let has = |field: &str| !matches!(fields.get(field), Node::Null);
    if !has("homepage") && !has("repository") {
        found.warning(
            "It's strongly recommended to include a \"homepage\" or \"repository\" field in your pubspec.yaml",
        );
    }
    field_url(found, "documentation");
    field_is_string(found, "version");
    for error in lazy_errors(fields) {
        found.error(format!("In your pubspec.yaml, {error}"));
    }
    Ok(())
}

// ---- DependencyValidator ----

/// `Version.primary`.
fn primary(versions: &[Version]) -> Option<Version> {
    let mut best: Option<&Version> = None;
    for v in versions {
        best = match best {
            None => Some(v),
            Some(b)
                if (!v.is_prerelease() && b.is_prerelease())
                    || (v.is_prerelease() == b.is_prerelease() && v > b) =>
            {
                Some(v)
            }
            keep => keep,
        };
    }
    best.cloned()
}

/// The versions of `name` on `url` that are not retracted
/// (`SystemCache.getVersions`).
fn server_versions(c: &Context<'_>, name: &str, url: &str) -> Result<Vec<Listed>, SourceError> {
    let mut cache = c.settings.cache(false);
    let listing = cache
        .hosted
        .listing(name, url, &c.settings.host, &c.settings.sdk)?;
    Ok(listing
        .versions
        .iter()
        .filter(|v| !v.retracted)
        .map(|v| Listed {
            version: v.version.clone(),
            language: language_of(&v.manifest.dart_sdk().original),
        })
        .collect())
}

struct Listed {
    version: Version,
    language: (u64, u64),
}

fn single_range(dependency: &Dependency) -> Option<&Range> {
    match dependency.constraint.ranges() {
        [range] => Some(range),
        _ => None,
    }
}

fn dependency(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let root = c.package.entry.root();
    let lock = LockFile::read(&root.directory, &c.settings.host).unwrap_or_default();
    let locked = |name: &str| lock.packages.get(name).map(|id| id.version.clone());
    let bold_all = terminal::bold("all");
    for dep in &c.package.manifest().dependencies {
        let constraint = &dep.constraint;
        let source = dep.description.source();
        if dep.name == "flutter" {
            if !matches!(dep.description, Description::Sdk(_)) {
                found.error(format!(
                    "Don't depend on \"{}\" from the {source} source. Use the SDK source instead. For example:\n\ndependencies:\n  {}:\n    sdk: {constraint}\n\nThe Flutter SDK is downloaded and managed outside of pub.",
                    dep.name, dep.name
                ));
            } else if let Description::Sdk(sdk) = &dep.description
                && !crate::pub_get::source::SDKS.contains(&sdk.as_str())
            {
                found.error(format!(
                    "Unknown SDK \"{sdk}\" for dependency \"{}\".",
                    dep.name
                ));
            }
        } else if let Description::Sdk(sdk) = &dep.description {
            if !crate::pub_get::source::SDKS.contains(&sdk.as_str()) {
                found.error(format!(
                    "Unknown SDK \"{sdk}\" for dependency \"{}\".",
                    dep.name
                ));
            }
        } else if !matches!(dep.description, Description::Hosted(_)) {
            let versions: Vec<Version> = server_versions(c, &dep.name, &c.settings.host)
                .map(|v| v.into_iter().map(|l| l.version).collect())
                .unwrap_or_default();
            let shown = match primary(&versions) {
                Some(v) => format!("^{v}"),
                None => {
                    let text = constraint.to_string();
                    if !constraint.is_any() && constraint.as_version().is_none() {
                        format!("\"{text}\"")
                    } else {
                        text
                    }
                }
            };
            let message = format!(
                "Don't depend on \"{}\" from the {source} source. Use the hosted source instead. For example:\n\ndependencies:\n  {}: {shown}\n\nUsing the hosted source ensures that everyone can download your package's dependencies along with your package.",
                dep.name, dep.name
            );
            if matches!(dep.description, Description::Path { .. }) {
                found.error(message);
            } else {
                found.warning(message);
            }
        } else if constraint.is_any() {
            let mut message = format!(
                "Your dependency on \"{}\" should have a version constraint.",
                dep.name
            );
            if let Some(v) = locked(&dep.name) {
                message = format!(
                    "{message} For example:\n\ndependencies:\n  {}: ^{v}\n",
                    dep.name
                );
            }
            found.warning(format!(
                "{message}\nWithout a constraint, you're promising to support {bold_all} future versions of \"{}\".",
                dep.name
            ));
        } else if let Some(range) = single_range(dep) {
            if constraint.as_version().is_some() {
                found.warning(format!(
                    "Your dependency on \"{}\" should allow more than one version. For example:\n\ndependencies:\n  {}: ^{constraint}\n\nConstraints that are too tight will make it difficult for people to use your package\nalong with other packages that also depend on \"{}\".",
                    dep.name, dep.name, dep.name
                ));
            } else {
                if let Some(min) = &range.min
                    && min.is_prerelease()
                    && !c.version().is_prerelease()
                {
                    found.warning(format!(
                        "Packages dependent on a pre-release of another package should themselves be published as a pre-release version. If this package needs {} version {min}, consider publishing the package as a pre-release instead.\nSee https://dart.dev/tools/pub/publishing#publishing-prereleases For more information on pre-releases.",
                        dep.name
                    ));
                }
                if range.min.is_none() {
                    let mut message = format!(
                        "Your dependency on \"{}\" should have a lower bound.",
                        dep.name
                    );
                    if let Some(v) = locked(&dep.name) {
                        let shown = if range.max.as_ref() == Some(&v) {
                            format!("^{v}")
                        } else {
                            format!("\">={v} {constraint}\"")
                        };
                        message = format!(
                            "{message} For example:\n\ndependencies:\n  {}: {shown}\n",
                            dep.name
                        );
                    }
                    found.warning(format!(
                        "{message}\nWithout a constraint, you're promising to support {bold_all} previous versions of \"{}\".",
                        dep.name
                    ));
                } else if let (Some(min), None) = (&range.min, &range.max) {
                    let shown = if range.include_min {
                        format!("^{min}")
                    } else {
                        format!("\"{constraint} <{}\"", min.next_breaking())
                    };
                    found.warning(format!(
                        "Your dependency on \"{}\" should have an upper bound. For example:\n\ndependencies:\n  {}: {shown}\n\nWithout an upper bound, you're promising to support {bold_all} future versions of {}.",
                        dep.name, dep.name, dep.name
                    ));
                }
            }
        }
    }
    Ok(())
}

// ---- DependencyOverrideValidator, DeprecatedFieldsValidator ----

fn dependency_override(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let dev: BTreeSet<&str> = c
        .package
        .manifest()
        .dev
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    let overridden = c
        .package
        .entry
        .workspace
        .iter()
        .flat_map(|w| w.manifest.overrides.iter())
        .any(|d| !dev.contains(d.name.as_str()));
    if overridden {
        let work = &c.package.entry.workspace[c.package.entry.work];
        let overrides_file = work.directory.join("pubspec_overrides.yaml");
        let from_file = std::fs::read_to_string(&overrides_file)
            .ok()
            .and_then(|t| crate::pub_get::tree::parse_yaml(&t).ok())
            .is_some_and(|n| n.contains("dependency_overrides"));
        let shown = p::normalize(&p::join(
            &c.package.dir,
            if from_file {
                "pubspec_overrides.yaml"
            } else {
                "pubspec.yaml"
            },
        ));
        found.hint(format!(
            "Non-dev dependencies are overridden in {shown}.\n\nThis indicates you are not testing your package against the same versions of its\ndependencies that users will have when they use it.\n\nThis might be necessary for packages with cyclic dependencies.\n\nPlease be extra careful when publishing."
        ));
    }
    Ok(())
}

fn deprecated_fields(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let fields = c.fields();
    if fields.contains("transformers") {
        found.warning("Your pubspec.yaml includes a \"transformers\" section which is no longer used and may be removed.");
    }
    if fields.contains("web") {
        found.warning("Your pubspec.yaml includes a \"web\" section which is no longer used and may be removed.");
    }
    if fields.contains("author") {
        found.warning("Your pubspec.yaml includes an \"author\" section which is no longer used and may be removed.");
    }
    if fields.contains("authors") {
        found.warning("Your pubspec.yaml includes an \"authors\" section which is no longer used and may be removed.");
    }
    Ok(())
}

// ---- DirectoryValidator, ExecutableValidator, CompiledDartdocValidator ----

fn directory(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    const DOC_REF: &str = "See https://dart.dev/tools/pub/package-layout.";
    let mut visited = BTreeSet::new();
    for file in c.files {
        let first = p::split(&p::relative(file, &c.package.dir))
            .into_iter()
            .next()
            .unwrap_or_default();
        let dir = p::join(&c.package.dir, &first);
        if !visited.insert(dir.clone()) || !dir_exists(&dir) {
            continue;
        }
        let name = p::basename(&dir);
        if ["benchmarks", "docs", "examples", "tests", "tools"].contains(&name.as_str()) {
            let singular = &name[..name.len() - 1];
            found.warning(format!(
                "Rename the top-level \"{name}\" directory to \"{singular}\".\nThe Pub layout convention is to use singular directory names.\nPlural names won't be correctly identified by Pub and other tools.\n{DOC_REF}"
            ));
        }
        if name == "sample" || name == "samples" {
            found.warning(format!(
                "Rename the top-level \"{name}\" directory to \"example\".\nThis allows Pub to find your examples and create \"packages\" directories for them.\n{DOC_REF}"
            ));
        }
    }
    Ok(())
}

fn executable(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let bin: Vec<String> = c
        .files_beneath("bin", false)
        .iter()
        .map(|f| p::relative(f, &c.package.dir))
        .collect();
    let (executables, _) = executables_with_partial(c.fields());
    for (name, script) in executables {
        let path = p::join("bin", &format!("{script}.dart"));
        if bin.contains(&path) {
            continue;
        }
        found.warning(format!(
            "Your pubspec.yaml lists an executable \"{name}\" that points to a script \"{path}\" that does not exist."
        ));
    }
    Ok(())
}

fn compiled_dartdoc(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    for entry in c.files {
        if p::basename(entry) != "nav.json" {
            continue;
        }
        let dir = p::dirname(entry);
        let all = [
            entry.clone(),
            p::join(&dir, "index.html"),
            p::join(&dir, "styles.css"),
            p::join(&dir, "dart-logo-small.png"),
            p::join(&dir, "client-live-nav.js"),
        ]
        .iter()
        .all(|f| file_exists(f));
        if all {
            found.warning(format!(
                "Avoid putting generated documentation in {}.\nGenerated documentation bloats the package with redundant data.",
                p::relative_to_current(&dir)
            ));
        }
    }
    Ok(())
}

// ---- ReadmeValidator, ChangelogValidator ----

/// `^<name>($|\.)` without case.
fn starts_like(base: &str, name: &str) -> bool {
    base.len() >= name.len()
        && base.is_char_boundary(name.len())
        && base[..name.len()].eq_ignore_ascii_case(name)
        && (base.len() == name.len() || base.as_bytes()[name.len()] == b'.')
}

fn readme(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let readmes: Vec<String> = c
        .files_beneath(".", false)
        .into_iter()
        .filter(|f| starts_like(&p::basename(f), "README"))
        .collect();
    let Some(mut chosen) = readmes.first().cloned() else {
        found.warning("Please add a README.md file that describes your package.");
        return Ok(());
    };
    for other in &readmes[1..] {
        let dots = |s: &str| p::basename(s).matches('.').count();
        let mut ordering = dots(&chosen).cmp(&dots(other));
        if ordering == std::cmp::Ordering::Equal {
            ordering = compare_utf16(&chosen, other);
        }
        if ordering == std::cmp::Ordering::Greater {
            chosen = other.clone();
        }
    }
    if p::basename(&chosen) != "README.md" {
        found.warning(format!(
            "Please consider renaming {chosen} to `README.md`. See https://dart.dev/tools/pub/publishing#important-files."
        ));
    }
    if std::str::from_utf8(&std::fs::read(&chosen).unwrap_or_default()).is_err() {
        found.warning(format!(
            "{chosen} contains invalid UTF-8.\nThis will cause it to be displayed incorrectly on the Pub site (https://pub.dev)."
        ));
    }
    Ok(())
}

fn changelog(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let Some(changelog) = c
        .files_beneath(".", false)
        .into_iter()
        .find(|f| starts_like(&p::basename(f), "CHANGELOG"))
    else {
        found.warning("Please add a `CHANGELOG.md` to your package. See https://dart.dev/tools/pub/publishing#important-files.");
        return Ok(());
    };
    if p::basename(&changelog) != "CHANGELOG.md" {
        found.warning(format!(
            "Please consider renaming {changelog} to `CHANGELOG.md`. See https://dart.dev/tools/pub/publishing#important-files."
        ));
    }
    let bytes = std::fs::read(&changelog).unwrap_or_default();
    let Ok(contents) = std::str::from_utf8(&bytes) else {
        found.warning(format!(
            "{changelog} contains invalid UTF-8.\nThis will cause it to be displayed incorrectly on the Pub site (https://pub.dev)."
        ));
        return Ok(());
    };
    let version = c.version().to_string();
    if !contents.contains(&version) {
        found.warning(format!(
            "{changelog} doesn't mention current version ({version}).\nConsider updating it with notes on this version prior to publication."
        ));
    }
    Ok(())
}

// ---- SdkConstraintValidator ----

fn sdk_constraint(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let dart = c.package.manifest().dart_sdk();
    let original = &dart.original;
    let effective = &dart.effective;
    let [range] = original.ranges() else {
        return Ok(());
    };
    let before = found.errors.len() + found.warnings.len();
    if range.max.is_none() {
        found.error("Published packages should have an upper bound constraint on the Dart SDK (typically this should restrict to less than the next major version to guard against breaking changes).\nSee https://dart.dev/tools/pub/pubspec#sdk-constraints for instructions on setting an sdk version constraint.");
    }
    if let Some(min) = &range.min
        && min.is_prerelease()
        && !c.version().is_prerelease()
    {
        found.warning(format!(
            "Packages with an SDK constraint on a pre-release of the Dart SDK should themselves be published as a pre-release version. If this package needs Dart version {min}, consider publishing the package as a pre-release instead.\nSee https://dart.dev/tools/pub/publishing#publishing-prereleases For more information on pre-releases."
        ));
    }
    if found.errors.len() + found.warnings.len() == before && original != effective {
        found.hint(format!(
            "The declared SDK constraint is '{original}', this is interpreted as '{effective}'.\n\nConsider updating the SDK constraint to:\n\nenvironment:\n  sdk: '{effective}'\n"
        ));
    }
    Ok(())
}

// ---- StrictDependenciesValidator ----

fn strict_dependencies(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let manifest = c.package.manifest();
    let mut deps: BTreeSet<&str> = manifest
        .dependencies
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    deps.insert(&manifest.name);
    let dev: BTreeSet<&str> = manifest.dev.iter().map(|d| d.name.as_str()).collect();
    let language = manifest.language_version();
    // Pub reads the directives lazily, so an invalid URL is reported where
    // its directive is, between the findings of the other directives.
    let mut each = |paths: &[&str], check: &mut dyn FnMut(super::imports::Usage, &mut Found)| {
        for path in paths {
            for file in c.files_beneath(path, true) {
                if p::extension(&file) != ".dart" {
                    continue;
                }
                super::imports::directives(&file, language, &mut |usage| match usage {
                    Err(message) => found.error(message),
                    Ok(usage) => check(usage, found),
                });
            }
        }
    };
    each(
        &["bin", "hook/build.dart", "hook/link.dart", "lib"],
        &mut |usage, found| {
            if deps.contains(usage.package.as_str()) {
                return;
            }
            if dev.contains(usage.package.as_str()) {
                let short = p::split(&p::relative_to_current(&usage.file))
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                found.error(usage.message(&format!(
                    "{} is in the `dev_dependencies` section of `pubspec.yaml`. Packages used in {short}/ must be declared in the `dependencies` section.",
                    usage.package
                )));
            } else {
                found.error(usage.message(&format!(
                    "This package does not have {} in the `dependencies` section of `pubspec.yaml`.",
                    usage.package
                )));
            }
        },
    );
    each(&["benchmark", "test", "tool"], &mut |usage, found| {
        if !deps.contains(usage.package.as_str()) && !dev.contains(usage.package.as_str()) {
            found.warning(usage.message(&format!(
                "This package does not have {} in the `dependencies` or `dev_dependencies` section of `pubspec.yaml`.",
                usage.package
            )));
        }
    });
    Ok(())
}

// ---- FlutterConstraintValidator, FlutterPluginFormatValidator ----

fn flutter_constraint(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let Node::String(text) = c.fields().get("environment").get("flutter") else {
        return Ok(());
    };
    let Ok(constraint) = crate::pub_get::version::VersionSet::parse(text) else {
        return Ok(());
    };
    if let [range] = constraint.ranges()
        && range.max.is_some()
    {
        let replacement = match &range.min {
            None => "You can replace the constraint with `any`.".to_owned(),
            Some(min) => format!("You can replace that with just the lower bound: `>={min}`."),
        };
        found.warning(format!(
            "The Flutter constraint should not have an upper bound.\nIn your pubspec.yaml the constraint is currently `{text}`.\n\n{replacement}\n\nSee https://dart.dev/go/flutter-upper-bound-deprecation"
        ));
    }
    Ok(())
}

fn flutter_plugin_format(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    const DOCS: &str =
        "https://flutter.dev/docs/development/packages-and-plugins/developing-packages#plugin";
    let Node::Map(_) = c.fields().get("flutter") else {
        return Ok(());
    };
    let plugin = c.fields().get("flutter").get("plugin");
    let Node::Map(_) = plugin else {
        return Ok(());
    };
    let old = ["androidPackage", "iosPrefix", "pluginClass"]
        .iter()
        .any(|k| plugin.contains(k));
    let new = !matches!(plugin.get("platforms"), Node::Null);
    let flutter = c
        .package
        .manifest()
        .sdks
        .iter()
        .find(|(name, _)| name == "flutter")
        .map(|(_, constraint)| &constraint.effective);
    let old_sdks = crate::pub_get::version::VersionSet::from_range(Range::user(
        Some(Version::new(0, 0, 0)),
        true,
        Some(Version::new(1, 10, 0)),
        false,
    ));
    if new && flutter.is_none_or(|f| f.allows_any(&old_sdks)) {
        found.error(format!(
            "pubspec.yaml allows Flutter SDK version 1.9.x, which does not support the flutter.plugin.platforms key.\nPlease consider increasing the Flutter SDK requirement to ^1.10.0 (environment.sdk.flutter)\n\nSee {DOCS}"
        ));
        return Ok(());
    }
    if old {
        found.error(format!(
            "In pubspec.yaml the flutter.plugin.{{androidPackage,iosPrefix,pluginClass}} keys are deprecated. Instead use the flutter.plugin.platforms key introduced in Flutter 1.10.0\n\nSee {DOCS}"
        ));
    }
    Ok(())
}

// ---- RelativeVersionNumberingValidator ----

fn relative_version_numbering(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let mut existing = match server_versions(c, c.name(), c.server) {
        Ok(v) => v,
        Err(SourceError::NotFound(_)) => Vec::new(),
        Err(SourceError::Fatal(m) | SourceError::Invalid(m)) => {
            return Err(Failure {
                code: crate::pub_get::exit::UNAVAILABLE,
                message: m,
            });
        }
    };
    existing.sort_by(|a, b| a.version.cmp(&b.version));
    let current = c.version();
    if let Some(latest) = existing.last()
        && latest.version > *current
    {
        found.hint(format!(
            "The latest published version is {}.\nYour version {current} is earlier than that.",
            latest.version
        ));
    }
    let Some(previous) = existing.iter().rev().find(|l| l.version < *current) else {
        return Ok(());
    };
    let pv = &previous.version;
    let no_pre = Version::new(current.major, current.minor, current.patch);
    let without_build = current.without_build();
    if no_pre != pv.next_major()
        && no_pre != pv.next_minor()
        && no_pre != pv.next_patch()
        && without_build != *pv
    {
        let suggestion = if pv.major == 0 {
            format!(
                "* {} for a first major release.\n* {} for a breaking release.\n* {} for a minor release.\n",
                pv.next_major(),
                pv.next_breaking(),
                pv.next_patch()
            )
        } else {
            format!(
                "* {} for a breaking release.\n* {} for a minor release.\n* {} for a patch release.",
                pv.next_breaking(),
                pv.next_minor(),
                pv.next_patch()
            )
        };
        found.hint(format!(
            "The previous version is {pv}.\n\nIt seems you are not publishing an incremental update.\n\nConsider one of:\n{suggestion}"
        ));
    }
    let current_opted = c.package.manifest().language_version() >= (2, 12);
    let previous_opted = previous.language >= (2, 12);
    if current_opted && !previous_opted {
        found.hint(format!(
            "You're about to publish a package that opts into null safety.\nThe previous version ({pv}) isn't opted in.\nSee https://dart.dev/null-safety/migration-guide for best practices."
        ));
    } else if !current_opted && previous_opted {
        found.hint(format!(
            "You're about to publish a package that doesn't opt into null safety,\nbut the previous version ({pv}) was opted in.\nThis change is likely to be backwards incompatible.\nSee https://dart.dev/tools/pub/versioning#semantic-versions for information about versioning."
        ));
    }
    Ok(())
}

// ---- PubspecTypoValidator ----

const VALID_KEYS: [&str; 22] = [
    "name",
    "version",
    "description",
    "homepage",
    "repository",
    "issue_tracker",
    "documentation",
    "dependencies",
    "dev_dependencies",
    "dependency_overrides",
    "environment",
    "executables",
    "publish_to",
    "false_secrets",
    "flutter",
    "screenshots",
    "platforms",
    "funding",
    "topics",
    "ignored_advisories",
    "workspace",
    "resolution",
];

/// `levenshteinDistance` of Pub, by UTF-16 code units.
fn levenshtein(s: &str, t: &str) -> usize {
    let s: Vec<u16> = s.encode_utf16().collect();
    let t: Vec<u16> = t.encode_utf16().collect();
    let (a, b) = if s.len() > t.len() { (s, t) } else { (t, s) };
    let mut distances: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        distances[0] = i;
        let mut holder = i - 1;
        for j in 1..=b.len() {
            let new = (1 + distances[j])
                .min(1 + distances[j - 1])
                .min(holder + usize::from(a[i - 1] != b[j - 1]));
            holder = distances[j];
            distances[j] = new;
        }
    }
    distances[b.len()]
}

fn pubspec_typo(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let Node::Map(entries) = c.fields() else {
        return Ok(());
    };
    let mut count = 0;
    for (key, _) in entries {
        if VALID_KEYS.contains(&key.as_str()) {
            continue;
        }
        let mut best = 100.0f64;
        let mut closest = "";
        let key_len = key.encode_utf16().count();
        for valid in VALID_KEYS {
            let ratio = levenshtein(key, valid) as f64 / (valid.len() + key_len) as f64;
            if ratio < best {
                best = ratio;
                closest = valid;
            }
        }
        if best > 0.0 && best < 0.21 {
            found.warning(format!(
                "\"{key}\" is not a key recognized by pub - did you mean \"{closest}\"?"
            ));
            count += 1;
            if count == 3 {
                break;
            }
        }
    }
    Ok(())
}

// ---- SizeValidator, DevtoolsExtensionValidator ----

/// `toStringAsPrecision(4)` for a positive number.
fn to_string_as_precision4(value: f64) -> String {
    let exponent = value.abs().log10().floor() as i32;
    if (-7..21).contains(&exponent) && exponent < 4 {
        let decimals = (3 - exponent).max(0) as usize;
        format!("{value:.decimals$}")
    } else {
        let mantissa = value / 10f64.powi(exponent);
        format!("{mantissa:.3}e+{exponent}")
    }
}

fn size_validator(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    const MAX: usize = 100 * 1024 * 1024;
    if c.size <= MAX {
        return Ok(());
    }
    let mb = to_string_as_precision4(c.size as f64 / f64::from(1 << 20));
    let ignore = file_exists(&p::join(&c.package.dir, ".gitignore"));
    let in_git = files::in_git_repo(&c.package.dir);
    let mut hint = format!(
        "Your package is {mb} MB.\n\nConsider the impact large downloads can have on the package consumer."
    );
    if ignore && !in_git {
        hint.push_str("\nYour .gitignore has no effect since your project does not appear to be in version control.");
    } else if !ignore && in_git {
        hint.push_str("\nConsider adding a .gitignore to avoid including temporary files.");
    }
    found.hint(hint);
    Ok(())
}

fn devtools_extension(c: &Context<'_>, found: &mut Found) -> Result<(), Failure> {
    let devtools = p::join(&p::join(&c.package.dir, "extension"), "devtools");
    if !dir_exists(&devtools) {
        return Ok(());
    }
    let config = p::join(&devtools, "config.yaml");
    let build = p::join(&devtools, "build");
    if !c.files.iter().any(|f| p::equals(f, &config))
        || !c.files.iter().any(|f| p::is_within(&build, f))
    {
        found.warning("It looks like you are making a devtools extension!\n\nThe folder `extension/devtools` should contain both a\n* `config.yaml` file and a\n* non-empty `build` directory'\n\nSee https://docs.flutter.dev/tools/devtools/extensions.\n");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_follow_pub() {
        assert_eq!(levenshtein("dependecies", "dependencies"), 1);
        assert_eq!(un_camel_case("FooBar"), "foo_bar");
        assert_eq!(un_camel_case("fooBarBaz"), "foo_bar_baz");
        assert_eq!(limit_length("abcdefghij", 7), "a[...]j");
        assert_eq!(limit_length("abcdefghij", 8), "ab[...]j");
        assert_eq!(to_string_as_precision4(100.5), "100.5");
        assert_eq!(to_string_as_precision4(1024.0), "1024");
        assert!(license_like("LICENSE"));
        assert!(license_like("MIT-LICENSE.txt"));
        assert!(license_like("UNLICENSE"));
        assert!(!license_like("license"));
        assert!(starts_like("readme.md", "README"));
        assert!(!starts_like("READMEX", "README"));
    }
}
