//! `dartforge format`: the SDK 3.6.2 `dart format` (`dart_style` 2.3.8),
//! with the same main options, messages and exit codes
//! (`lib/src/cli/format_command.dart`, `io.dart`, `formatter_options.dart`).
//!
//! Exit codes: 0 without errors; 1 with `--set-exit-if-changed` when some
//! file changes; 65 when some file could not be formatted; 64 on a usage
//! error. As in the original, the last one to happen wins between 1 and 65.
//!
//! The optional fixes (`--fix*`) are not supported.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use dartforge_formatter::{ErroFormatacao, Opcoes, formatar};

const USAGE: &str = "Usage: dart format [options...] <files or directories...>
-h, --help                     Print this usage information.
-v, --verbose                  Show all options and flags with --help.
-o, --output                   Set where to write formatted output.

          [json]               Print code and selection as JSON.
          [none]               Discard output.
          [show]               Print code to terminal.
          [write] (default)    Overwrite formatted files on disk.

    --set-exit-if-changed      Return exit code 1 if there are any formatting changes.

Run \"dart help\" to see global options.";

const VERBOSE_USAGE: &str = "Usage: dart format [options...] <files or directories...>
-h, --help                             Print this usage information.
-v, --verbose                          Show all options and flags with --help.

Output options:
-o, --output                           Set where to write formatted output.

          [json]                       Print code and selection as JSON.
          [none]                       Discard output.
          [show]                       Print code to terminal.
          [write] (default)            Overwrite formatted files on disk.

    --show                             Set which filenames to print.

          [all]                        All visited files and directories.
          [changed] (default)          Only the names of files whose formatting is changed.
          [none]                       No file names or directories.

    --summary                          Show the specified summary after formatting.

          [line] (default)             Single-line summary.
          [none]                       No summary.
          [profile]                    How long it took for format each file.

    --set-exit-if-changed              Return exit code 1 if there are any formatting changes.

Other options:
-i, --indent                           Add this many spaces of leading indentation.
                                       (defaults to \"0\")
    --follow-links                     Follow links to files and directories.
                                       If unset, links will be ignored.
    --version                          Show dart_style version.
    --enable-experiment                Enable one or more experimental features.
                                       See dart.dev/go/experiments.

Options when formatting from stdin:
    --stdin-name                       Use this path in error messages when input is read from stdin.
                                       (defaults to \"stdin\")

Run \"dart help\" to see global options.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Write,
    Show,
    Json,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Show {
    All,
    Changed,
    None,
}

struct Config {
    output: Output,
    show: Show,
    summary: bool,
    exit_if_changed: bool,
    options: Opcoes,
    follow_links: bool,
}

struct State {
    files: usize,
    changed: usize,
    code: u8,
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("{msg}\n\n{USAGE}");
    ExitCode::from(64)
}

/// Runs `dartforge format` with the arguments after the subcommand.
pub fn run(args: &[OsString]) -> ExitCode {
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut output = Output::Write;
    let mut output_given = false;
    let mut show = Show::Changed;
    let mut show_given = false;
    let mut summary = true;
    let mut summary_given = false;
    let mut exit_if_changed = false;
    let mut width = "80".to_string();
    let mut indent = "0".to_string();
    let mut follow_links = false;
    let mut help = false;
    let mut verbose = false;
    let mut version = false;
    let mut stdin_name: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == "--" {
            paths.extend(args[i..].iter().cloned());
            break;
        }
        if !a.starts_with('-') || a == "-" {
            paths.push(a.clone());
            continue;
        }
        // Splits `--name=value`, `-oshow` and the forms with the value in the
        // next argument.
        let (name, attached_value): (String, Option<String>) = if let Some(rest) = a.strip_prefix("--") {
            match rest.split_once('=') {
                Some((n, v)) => (format!("--{n}"), Some(v.to_string())),
                None => (a.clone(), None),
            }
        } else if a.len() > 2 {
            (a[..2].to_string(), Some(a[2..].to_string()))
        } else {
            (a.clone(), None)
        };
        let takes_value = matches!(
            name.as_str(),
            "-o" | "--output"
                | "--show"
                | "--summary"
                | "-l"
                | "--line-length"
                | "-i"
                | "--indent"
                | "--language-version"
                | "--enable-experiment"
                | "--stdin-name"
                | "--selection"
        );
        let value = if takes_value {
            match attached_value {
                Some(v) => Some(v),
                None => {
                    if i >= args.len() {
                        return usage_error(&format!("Missing argument for \"{name}\"."));
                    }
                    i += 1;
                    Some(args[i - 1].clone())
                }
            }
        } else {
            if attached_value.is_some() && name.starts_with("--") {
                return usage_error(&format!("Flag \"{name}\" does not take a value."));
            }
            None
        };
        let not_allowed = |v: &str| usage_error(&format!("\"{v}\" is not an allowed value for option \"{name}\"."));
        match name.as_str() {
            "-h" | "--help" => help = true,
            "-v" | "--verbose" => verbose = true,
            "--version" => version = true,
            "--set-exit-if-changed" => exit_if_changed = true,
            "--follow-links" => follow_links = true,
            "-o" | "--output" => {
                let v = value.expect("value");
                output = match v.as_str() {
                    "write" => Output::Write,
                    "show" => Output::Show,
                    "json" => Output::Json,
                    "none" => Output::None,
                    _ => return not_allowed(&v),
                };
                output_given = true;
            }
            "--show" => {
                let v = value.expect("value");
                show = match v.as_str() {
                    "all" => Show::All,
                    "changed" => Show::Changed,
                    "none" => Show::None,
                    _ => return not_allowed(&v),
                };
                show_given = true;
            }
            "--summary" => {
                let v = value.expect("value");
                summary = match v.as_str() {
                    "line" | "profile" => true,
                    "none" => false,
                    _ => return not_allowed(&v),
                };
                summary_given = true;
            }
            "-l" | "--line-length" => width = value.expect("value"),
            "-i" | "--indent" => indent = value.expect("value"),
            "--language-version" => {
                let v = value.expect("value");
                let ok = v == "latest"
                    || v.split_once('.')
                        .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()));
                if !ok {
                    return usage_error(&format!("--language-version must be a version like \"3.2\" or \"latest\", was \"{v}\"."));
                }
            }
            "--enable-experiment" | "--selection" => {}
            "--stdin-name" => stdin_name = value,
            n if n.starts_with("--fix") => {
                eprintln!("dartforge format: {n} is not supported.");
                return ExitCode::from(64);
            }
            _ => {
                let shown = if a.starts_with("--") { name.clone() } else { name[1..].to_string() };
                if a.starts_with("--") {
                    return usage_error(&format!("Could not find an option named \"{shown}\"."));
                }
                return usage_error(&format!("Could not find an option or flag \"-{shown}\"."));
            }
        }
    }

    if help {
        println!("Idiomatically format Dart source code.\n\n{}", if verbose { VERBOSE_USAGE } else { USAGE });
        return ExitCode::SUCCESS;
    }
    if version {
        println!("2.3.8");
        return ExitCode::SUCCESS;
    }
    if !output_given && paths.is_empty() {
        output = Output::Show;
    }
    if !show_given && (output == Output::Show || output == Output::Json) {
        show = Show::None;
    }
    if !summary_given && output == Output::Json {
        summary = false;
    }
    if verbose {
        return usage_error("Can only use --verbose with --help.");
    }
    if output == Output::Json && summary {
        return usage_error("Cannot print a summary with JSON output.");
    }
    let Ok(width) = width.parse::<i64>() else {
        return usage_error(&format!("--line-length must be an integer, was \"{width}\"."));
    };
    let Ok(indent_n) = indent.parse::<i64>() else {
        return usage_error(&format!("--indent must be an integer, was \"{indent}\"."));
    };
    if indent_n < 0 {
        return usage_error(&format!("--indent must be non-negative, was \"{indent}\"."));
    }
    if paths.is_empty() && std::io::stdin().is_terminal() {
        return usage_error("Missing paths to code to format.");
    }
    if paths.is_empty() && output == Output::Write {
        return usage_error("Cannot use --output=write when reading from stdin.");
    }
    if stdin_name.is_some() && !paths.is_empty() {
        return usage_error("Cannot pass --stdin-name when not reading from stdin.");
    }

    let config = Config {
        output,
        show,
        summary,
        exit_if_changed,
        options: Opcoes {
            largura: width.max(0) as usize,
            recuo: indent_n as usize,
            fim_de_linha: None,
        },
        follow_links,
    };
    let mut state = State {
        files: 0,
        changed: 0,
        code: 0,
    };
    let start = Instant::now();
    if paths.is_empty() {
        let name = stdin_name.unwrap_or_else(|| "stdin".into());
        let mut source = String::new();
        if std::io::stdin().read_to_string(&mut source).is_err() {
            eprintln!("Could not read stdin.");
            return ExitCode::from(65);
        }
        process_source(&config, &mut state, None, &name, &source);
        return ExitCode::from(state.code);
    }
    for c in &paths {
        let p = Path::new(c);
        if p.is_dir() {
            if !process_directory(&config, &mut state, c) {
                state.code = 65;
            }
            continue;
        }
        if p.is_file() {
            if !process_file(&config, &mut state, p, c) {
                state.code = 65;
            }
        } else {
            eprintln!("No file or directory found at \"{c}\".");
        }
    }
    if config.summary {
        let time = format!("{:.2}", start.elapsed().as_millis() as f64 / 1000.0);
        let n = state.files;
        let m = state.changed;
        if n == 0 {
            println!("Formatted no files in {time} seconds.");
        } else if n == 1 {
            println!("Formatted {n} file ({m} changed) in {time} seconds.");
        } else {
            println!("Formatted {n} files ({m} changed) in {time} seconds.");
        }
    }
    ExitCode::from(state.code)
}

/// Simplified `p.normalize(path)`: removes `.` and `a/..`, joins slashes.
fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !absolute {
                    parts.push("..");
                }
            }
            _ => parts.push(part),
        }
    }
    let s = parts.join("/");
    if absolute {
        format!("/{s}")
    } else if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

/// `_processDirectory`: every `.dart` below, in path order, without links
/// (unless `--follow-links`) and without hidden directories.
fn process_directory(config: &Config, state: &mut State, dir: &str) -> bool {
    let mut entries: Vec<(String, bool, bool)> = Vec::new();
    list(Path::new(dir), dir, config.follow_links, &mut entries);
    entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let mut success = true;
    for (path, is_file, is_link) in entries {
        if is_link || !is_file || !path.ends_with(".dart") {
            continue;
        }
        let relative = path.strip_prefix(dir).unwrap_or(&path).trim_start_matches('/');
        if relative.split('/').any(|p| p.starts_with('.')) {
            continue;
        }
        let shown = normalize(&path);
        if !process_file(config, state, Path::new(&path), &shown) {
            success = false;
        }
    }
    success
}

fn list(dir: &Path, prefix: &str, follow: bool, out: &mut Vec<(String, bool, bool)>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for e in read.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = if prefix.ends_with('/') {
            format!("{prefix}{name}")
        } else {
            format!("{prefix}/{name}")
        };
        let p: PathBuf = e.path();
        let is_link = p.is_symlink();
        if is_link && !follow {
            out.push((path, false, true));
            continue;
        }
        if p.is_dir() {
            out.push((path.clone(), false, false));
            list(&p, &path, follow, out);
        } else {
            out.push((path, true, false));
        }
    }
}

/// `_processFile`.
fn process_file(config: &Config, state: &mut State, file: &Path, shown: &str) -> bool {
    let source = match std::fs::read(file) {
        Ok(b) => match String::from_utf8(b) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Hit a bug in the formatter when formatting {shown}.\nPlease report at github.com/dart-lang/dart_style/issues.\nFormatException: Invalid UTF-8 byte");
                return false;
            }
        },
        Err(e) => {
            eprintln!("Hit a bug in the formatter when formatting {shown}.\nPlease report at github.com/dart-lang/dart_style/issues.\n{e}");
            return false;
        }
    };
    process_source(config, state, Some(file), shown, &source)
}

fn process_source(config: &Config, state: &mut State, file: Option<&Path>, shown: &str, source: &str) -> bool {
    let result = std::panic::catch_unwind(|| formatar(source, &config.options));
    let output = match result {
        Ok(Ok(s)) => s,
        Ok(Err(ErroFormatacao::Sintaxe(e))) => {
            eprintln!("{}", error_message(source, shown, &e));
            if file.is_none() {
                state.code = 65;
            }
            return false;
        }
        Ok(Err(e @ ErroFormatacao::SaidaInesperada { .. })) => {
            eprintln!("Hit a bug in the formatter when formatting {shown}.\n{e}\nPlease report at github.com/dart-lang/dart_style/issues.");
            if file.is_none() {
                state.code = 70;
            }
            return false;
        }
        Err(_) => {
            eprintln!("Hit a bug in the formatter when formatting {shown}.\nPlease report at github.com/dart-lang/dart_style/issues.");
            if file.is_none() {
                state.code = 70;
            }
            return false;
        }
    };
    let changed = output != source;
    state.files += 1;
    if changed {
        state.changed += 1;
    }
    let mut overwritten = false;
    if changed && config.output == Output::Write {
        overwritten = true;
        if let Some(a) = file
            && let Err(e) = std::fs::write(a, &output)
        {
            eprintln!(
                "Could not overwrite {shown}: {} (error code {})",
                e,
                e.raw_os_error().unwrap_or(0)
            );
        }
    }
    let display = match config.show {
        Show::All => {
            if changed {
                println!("{} {shown}", if overwritten { "Formatted" } else { "Changed" });
            } else {
                println!("Unchanged {shown}");
            }
            true
        }
        Show::Changed => {
            if changed {
                println!("{} {shown}", if overwritten { "Formatted" } else { "Changed" });
            }
            changed
        }
        Show::None => true,
    };
    if display {
        match config.output {
            Output::Show => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(output.as_bytes());
                let _ = out.flush();
            }
            Output::Json => {
                let path = serde_json::to_string(shown).unwrap_or_default();
                let text = serde_json::to_string(&output).unwrap_or_default();
                println!("{{\"path\":{path},\"source\":{text},\"selection\":{{\"offset\":-1,\"length\":-1}}}}");
            }
            _ => {}
        }
    }
    if config.exit_if_changed && changed {
        state.code = 1;
    }
    true
}

/// The `FormatterException` message: the excerpt with line and column, in
/// the format of the `source_span` package's `SourceSpan.message`.
fn error_message(source: &str, shown: &str, e: &dartforge_formatter::ErroSintaxe) -> String {
    let pos = e.posicao.min(source.len());
    let line_start = source[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = source[..pos].matches('\n').count() + 1;
    let column = source[line_start..pos].chars().count() + 1;
    let line_end = source[pos..].find('\n').map(|i| pos + i).unwrap_or(source.len());
    let line_text = source[line_start..line_end].trim_end_matches('\r');
    let number_width = line.to_string().len();
    let margin = " ".repeat(number_width);
    let caret = "^".repeat(source[pos..(pos + e.comprimento).min(line_end.max(pos + 1)).min(source.len()).max(pos)].chars().count().max(1));
    format!(
        "Could not format because the source could not be parsed:\n\nline {line}, column {column} of {shown}: {}\n{margin} ╷\n{line} │ {line_text}\n{margin} │ {}{caret}\n{margin} ╵",
        e.mensagem,
        " ".repeat(column - 1)
    )
}
