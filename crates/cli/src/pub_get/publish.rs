//! `dart pub publish` (alias `lish`) of Pub 3.6.2 (`command/lish.dart`):
//! the file list with the `.pubignore`/`.gitignore` rules, the tree and the
//! `.tar.gz` the VM writes, the 25 validators of `Validator.runAll`, and the
//! upload protocol (`api/packages/versions/new`, the multipart upload and
//! the finalization), with `--dry-run`, `--force`, `--skip-validation` and
//! the hidden `--server`, `--to-archive` and `--from-archive`.
mod archive;
mod deflate;
mod files;
mod ignore;
mod imports;
mod leaks;
mod paths;
mod span;
mod terminal;
mod upload;
mod validators;

use super::{
    Entrypoint, Failure, Settings, acquire, data, exit,
    solver::SolveType,
    tree::{Node, parse_yaml},
    version::Version,
};

const DESCRIPTION: &str = "Publish the current package to pub.dev.";

const USAGE: &str = "Usage: dart pub publish [options]
-h, --help               Print this usage information.
-n, --dry-run            Validate but do not publish the package.
-f, --force              Publish without confirmation if there are no errors.
    --skip-validation    Publish without validation and resolution (this will
                         ignore errors).
-C, --directory=<dir>    Run this in the directory <dir>.

Run \"dart help\" to see global options.
See https://dart.dev/tools/pub/cmd/pub-lish for detailed documentation.";

fn usage_error(message: &str) -> Failure {
    Failure {
        code: exit::USAGE,
        message: format!("{message}\n\n{USAGE}"),
    }
}

/// The options of `publish`, parsed like `package:args` does.
#[derive(Default)]
struct Options {
    help: bool,
    dry_run: bool,
    force: bool,
    skip_validation: bool,
    server: Option<String>,
    to_archive: Option<String>,
    from_archive: Option<String>,
    directory: Option<String>,
    sdk: Option<String>,
    sdk_version: Option<String>,
}

/// `(name, short, takes a value)` of every option.
const SPECS: [(&str, Option<char>, bool); 10] = [
    ("help", Some('h'), false),
    ("dry-run", Some('n'), false),
    ("force", Some('f'), false),
    ("skip-validation", None, false),
    ("server", None, true),
    ("to-archive", None, true),
    ("from-archive", None, true),
    ("directory", Some('C'), true),
    ("sdk", None, true),
    ("sdk-version", None, true),
];

impl Options {
    fn set(&mut self, name: &str, value: Option<String>) {
        match name {
            "help" => self.help = true,
            "dry-run" => self.dry_run = true,
            "force" => self.force = true,
            "skip-validation" => self.skip_validation = true,
            "server" => self.server = value,
            "to-archive" => self.to_archive = value,
            "from-archive" => self.from_archive = value,
            "directory" => self.directory = value,
            "sdk" => self.sdk = value,
            _ => self.sdk_version = value,
        }
    }

    fn parse(args: &[String]) -> Result<Self, Failure> {
        let mut options = Options::default();
        let mut rest = Vec::new();
        let mut index = 0;
        while index < args.len() {
            let arg = &args[index];
            index += 1;
            if arg == "--" {
                rest.extend(args[index..].iter().cloned());
                break;
            }
            if let Some(long) = arg.strip_prefix("--") {
                let (name, inline) = match long.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_owned())),
                    None => (long, None),
                };
                if let Some(&(name, _, takes_value)) = SPECS.iter().find(|s| s.0 == name) {
                    if takes_value {
                        let value = match inline {
                            Some(value) => value,
                            None => {
                                let value = args.get(index).cloned().ok_or_else(|| {
                                    usage_error(&format!("Missing argument for \"{arg}\"."))
                                })?;
                                index += 1;
                                value
                            }
                        };
                        options.set(name, Some(value));
                    } else {
                        if inline.is_some() {
                            return Err(usage_error(&format!(
                                "Flag option \"--{name}\" should not be given a value."
                            )));
                        }
                        options.set(name, None);
                    }
                    continue;
                }
                if let Some(positive) = name.strip_prefix("no-")
                    && let Some(spec) = SPECS.iter().find(|s| s.0 == positive)
                {
                    return Err(usage_error(&if spec.2 {
                        format!("Cannot negate non-flag option \"--{name}\".")
                    } else {
                        format!("Cannot negate option \"--{name}\".")
                    }));
                }
                return Err(usage_error(&format!(
                    "Could not find an option named \"--{name}\"."
                )));
            }
            if let Some(short) = arg.strip_prefix('-')
                && !short.is_empty()
            {
                for (offset, c) in short.char_indices() {
                    let Some(&(name, _, takes_value)) = SPECS.iter().find(|s| s.1 == Some(c))
                    else {
                        return Err(usage_error(&format!(
                            "Could not find an option or flag \"-{c}\"."
                        )));
                    };
                    if takes_value {
                        let tail = &short[offset + c.len_utf8()..];
                        let value = if tail.is_empty() {
                            let value = args.get(index).cloned().ok_or_else(|| {
                                usage_error(&format!("Missing argument for \"-{c}\"."))
                            })?;
                            index += 1;
                            value
                        } else {
                            tail.to_owned()
                        };
                        options.set(name, Some(value));
                        break;
                    }
                    options.set(name, None);
                }
                continue;
            }
            rest.push(arg.clone());
        }
        if !rest.is_empty() && !options.help {
            return Err(usage_error(
                "Command \"publish\" does not take any arguments.",
            ));
        }
        Ok(options)
    }
}

/// `FormatException.toString()` for a message with the text as its source.
fn format_exception(message: &str, source: &str) -> String {
    let source = if source.chars().count() > 78 {
        format!("{}...", source.chars().take(75).collect::<String>())
    } else {
        source.to_owned()
    };
    format!("FormatException: {message}\n{source}")
}

/// `validateAndNormalizeHostedUrl`, failing with the text of the
/// `FormatException`.
fn normalize_url(text: &str) -> Result<String, String> {
    match super::hosted::normalize_host(text) {
        Ok(url) => Ok(url),
        Err(e) => {
            let message = match e.rfind(" (") {
                Some(index) if e.ends_with(')') && !e.starts_with("invalid url") => {
                    e[..index].to_owned()
                }
                _ => e,
            };
            Err(format_exception(&message, text))
        }
    }
}

/// What will be published: the archive, the counts of the validation and
/// the pubspec's name and version.
struct Publication {
    bytes: Vec<u8>,
    warnings: usize,
    hints: usize,
    name: String,
    version: Version,
    publish_to: Option<String>,
}

fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

impl Publication {
    fn warnings_count_message(&self) -> String {
        let hints = if self.hints == 0 {
            String::new()
        } else {
            format!(" and {} {}", self.hints, pluralize("hint", self.hints))
        };
        format!(
            "\nPackage has {} {}{hints}.",
            self.warnings,
            pluralize("warning", self.warnings)
        )
    }
}

/// `_readableFileSize` of `lish.dart` (no color).
fn readable_size(size: usize) -> String {
    if size >= 1 << 30 {
        format!("{} GB", size / (1 << 30))
    } else if size >= 1 << 20 {
        format!("{} MB", size / (1 << 20))
    } else if size >= 1 << 10 {
        format!("{} KB", size / (1 << 10))
    } else {
        "<1 KB".to_owned()
    }
}

/// The package being published: its directory as Pub spells it, its parsed
/// pubspec and the raw fields.
pub(crate) struct Package<'a> {
    pub dir: String,
    pub entry: &'a Entrypoint,
    pub fields: Node,
}

impl Package<'_> {
    pub fn manifest(&self) -> &super::manifest::Manifest {
        &self.entry.workspace[self.entry.work].manifest
    }

    /// `pubspec.publishTo`.
    pub fn publish_to(&self) -> Result<Option<String>, Failure> {
        match self.fields.get("publish_to") {
            Node::Null => Ok(None),
            Node::String(s) => {
                if s != "none" && url::Url::parse(s).is_err() {
                    return Err(data(
                        "Invalid \"publish_to\" field: must be an absolute URL.",
                    ));
                }
                Ok(Some(s.clone()))
            }
            _ => Err(data("\"publish_to\" field must be a string.")),
        }
    }
}

struct Command<'a> {
    options: &'a Options,
    settings: Settings,
}

impl Command<'_> {
    fn compute_host(&self, publish_to: Option<&str>) -> Result<String, Failure> {
        if let Some(server) = &self.options.server {
            return normalize_url(server).map_err(|e| usage_error(&format!("Invalid server: {e}")));
        }
        if let Some(publish_to) = publish_to
            && publish_to != "none"
        {
            return normalize_url(publish_to).map_err(|e| data(format!("Invalid publish_to: {e}")));
        }
        Ok(self.settings.host.clone())
    }

    fn private_error() -> Failure {
        data(
            "A private package cannot be published.\nYou can enable this by changing the \"publish_to\" field in your pubspec.",
        )
    }

    fn publication_from_entrypoint(&self) -> Result<Publication, Failure> {
        let directory = self.options.directory.clone().unwrap_or_else(|| ".".into());
        if !files::dir_exists(&directory) {
            return Err(Failure {
                code: 1,
                message: format!("The directory `{directory}` does not exist."),
            });
        }
        let entry = Entrypoint::load(&directory, &self.settings)?;
        let work = &entry.workspace[entry.work];
        let fields = std::fs::read_to_string(work.directory.join("pubspec.yaml"))
            .ok()
            .and_then(|t| parse_yaml(&t).ok())
            .unwrap_or(Node::Null);
        let package = Package {
            dir: work.shown.clone(),
            entry: &entry,
            fields,
        };
        let publish_to = package.publish_to()?;
        if !self.options.dry_run
            && self.options.to_archive.is_none()
            && publish_to.as_deref() == Some("none")
        {
            return Err(Self::private_error());
        }
        if self.options.skip_validation {
            terminal::warning("Running with `skip-validation`. No client-side validation is done.");
        } else {
            acquire(
                &entry,
                &self.settings,
                false,
                SolveType::Get,
                &Default::default(),
            )?;
        }
        let files_and_dirs = files::list_files(&package.dir, true)?;
        let mut files = Vec::new();
        let mut shown = Vec::new();
        for entry in &files_and_dirs {
            if files::dir_exists(entry) {
                if files::dir_is_empty(entry) {
                    shown.push(entry.clone());
                }
            } else {
                files.push(entry.clone());
                shown.push(entry.clone());
            }
        }
        let host = self.compute_host(publish_to.as_deref())?;
        let manifest = package.manifest();
        terminal::message(&format!(
            "Publishing {} {} to {host}:\n{}",
            manifest.name,
            manifest.version,
            files::tree(&shown, &package.dir)
        ));
        let bytes = archive::create_tar_gz(&files_and_dirs, &package.dir)?;
        terminal::message(&format!(
            "\nTotal compressed archive size: {}.\n",
            readable_size(bytes.len())
        ));
        let (warnings, hints) = if self.options.skip_validation {
            (0, 0)
        } else {
            self.validate(&package, bytes.len(), &host, &files)?
        };
        if self.options.dry_run {
            terminal::message("The server may enforce additional checks.");
        }
        Ok(Publication {
            bytes,
            warnings,
            hints,
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            publish_to,
        })
    }

    fn validate(
        &self,
        package: &Package<'_>,
        size: usize,
        host: &str,
        files: &[String],
    ) -> Result<(usize, usize), Failure> {
        let spinner = terminal::Spinner::start("Validating package");
        let result = validators::run_all(package, &self.settings, size, host, files);
        spinner.stop_and_clear();
        let report = result?;
        report.print();
        if !report.errors.is_empty() {
            return Err(data(format!(
                "Sorry, your package is missing {} and can't be published yet.\nFor more information, see: https://dart.dev/tools/pub/cmd/pub-lish.\n",
                if report.errors.len() > 1 {
                    "some requirements"
                } else {
                    "a requirement"
                }
            )));
        }
        Ok((report.warnings.len(), report.hints.len()))
    }

    fn publication_from_archive(&self, archive: &str) -> Result<Publication, Failure> {
        terminal::message(&format!("Publishing from archive: {archive}"));
        let bytes = std::fs::read(archive).map_err(|e| {
            data(format!(
                "Failed reading archive file: {}: Cannot open file, path = '{archive}' ({}))",
                if e.kind() == std::io::ErrorKind::NotFound {
                    "PathNotFoundException"
                } else {
                    "FileSystemException"
                },
                files::os_error(&e)
            ))
        })?;
        let pubspec = upload::pubspec_from_archive(&bytes)
            .map_err(|e| data(format!("Failed to read pubspec.yaml from archive: {e}")))?;
        let fields = parse_yaml(&pubspec)
            .map_err(|e| data(format!("Failed to read pubspec.yaml from archive: {e}")))?;
        let name = match fields.get("name") {
            Node::String(name) => name.clone(),
            Node::Null => {
                return Err(data(
                    "Failed to read pubspec.yaml from archive: Missing the required \"name\" field.",
                ));
            }
            _ => {
                return Err(data(
                    "Failed to read pubspec.yaml from archive: \"name\" field must be a string.",
                ));
            }
        };
        let version = match fields.get("version") {
            Node::Null => Version::new(0, 0, 0),
            Node::String(text) => Version::parse(text)
                .map_err(|e| data(format!("Failed to read pubspec.yaml from archive: {e}")))?,
            _ => {
                return Err(data(
                    "Failed to read pubspec.yaml from archive: \"version\" field must be a string.",
                ));
            }
        };
        let publish_to = match fields.get("publish_to") {
            Node::String(s) => Some(s.clone()),
            _ => None,
        };
        if !self.options.dry_run
            && self.options.to_archive.is_none()
            && publish_to.as_deref() == Some("none")
        {
            return Err(Self::private_error());
        }
        let host = self.compute_host(publish_to.as_deref())?;
        terminal::message(&format!("Publishing {name} {version} to {host}."));
        Ok(Publication {
            bytes,
            warnings: 0,
            hints: 0,
            name,
            version,
            publish_to,
        })
    }
}

/// Runs `dartforge pub publish <args>`.
pub(crate) fn run(args: &[String]) -> Result<u8, Failure> {
    // Pub logs a failure line by line, so a final line break is dropped.
    run_publish(args).map_err(|f| Failure {
        code: f.code,
        message: terminal::as_logged(&f.message),
    })
}

fn run_publish(args: &[String]) -> Result<u8, Failure> {
    let options = Options::parse(args)?;
    if options.help {
        println!("{DESCRIPTION}\n\n{USAGE}");
        return Ok(0);
    }
    if options.force && options.dry_run {
        return Err(usage_error("Cannot use both --force and --dry-run."));
    }
    if options.from_archive.is_some() && options.to_archive.is_some() {
        return Err(usage_error(
            "Cannot use both --from-archive and --to-archive.",
        ));
    }
    if options.from_archive.is_some() && options.dry_run {
        return Err(usage_error("Cannot use both --from-archive and --dry-run."));
    }
    if options.to_archive.is_some() && options.force {
        return Err(usage_error("Cannot use both --to-archive and --force."));
    }
    if options.server.is_some() && terminal::stdout_is_terminal() {
        terminal::message(
            "The --server option is deprecated. Use `publish_to` in your pubspec.yaml or set\nthe $PUB_HOSTED_URL environment variable.",
        );
    }
    let settings = Settings::discover(
        options.sdk.as_ref().map(std::path::PathBuf::from),
        options.sdk_version.as_deref(),
    )?;
    let command = Command {
        options: &options,
        settings,
    };
    let publication = match &options.from_archive {
        Some(archive) => command.publication_from_archive(archive)?,
        None => command.publication_from_entrypoint()?,
    };
    if options.dry_run {
        terminal::warning(&publication.warnings_count_message());
        return Ok(if publication.warnings != 0 {
            exit::DATA
        } else {
            0
        });
    }
    match &options.to_archive {
        None => {
            let host = command.compute_host(publication.publish_to.as_deref())?;
            upload::confirm(&options, &publication, &host)?;
            upload::publish(&command.settings, &publication.bytes, &host)?;
        }
        Some(archive) => {
            std::fs::write(archive, &publication.bytes).map_err(|e| {
                data(format!(
                    "Failed writing archive: FileSystemException: Cannot open file, path = '{archive}' ({})",
                    files::os_error(&e)
                ))
            })?;
            terminal::message(&format!("Wrote package archive at {archive}"));
        }
    }
    Ok(0)
}
