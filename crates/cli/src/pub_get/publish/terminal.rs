//! Pub's `log.dart` at the normal verbosity: messages go to stdout, warnings
//! and errors to stderr, each line of a message written separately (Dart's
//! `LineSplitter`, so a final line break adds no empty line and an empty
//! message prints nothing), and colors only on a terminal.
use std::io::{IsTerminal, Write};
use std::sync::OnceLock;

/// `terminalOutputForStdout`, with `_PUB_FORCE_TERMINAL_OUTPUT` (`0`/`1`).
pub(crate) fn stdout_is_terminal() -> bool {
    match std::env::var("_PUB_FORCE_TERMINAL_OUTPUT").as_deref() {
        Ok("0") => false,
        Ok("1") => true,
        _ => std::io::stdout().is_terminal(),
    }
}

/// `canUseAnsiCodes` with the automatic color option.
pub(crate) fn colors() -> bool {
    static COLORS: OnceLock<bool> = OnceLock::new();
    *COLORS.get_or_init(|| std::env::var_os("NO_COLOR").is_none() && stdout_is_terminal())
}

fn ansi(code: &'static str) -> &'static str {
    if colors() { code } else { "" }
}

/// `_addColor`.
fn add_color(text: &str, code: &'static str) -> String {
    let none = ansi("\u{1b}[0m");
    let no_color = ansi("\u{1b}[39m");
    let code = ansi(code);
    let mut inner = text.to_owned();
    if !none.is_empty() {
        inner = inner.replace(none, &format!("{none}{code}"));
        inner = inner.replace(no_color, &format!("{none}{code}"));
    }
    format!("{code}{inner}{no_color}")
}

pub(crate) fn gray(text: &str) -> String {
    format!("{}{text}{}", ansi("\u{1b}[38;5;245m"), ansi("\u{1b}[0m"))
}

pub(crate) fn bold(text: &str) -> String {
    format!("{}{text}{}", ansi("\u{1b}[1m"), ansi("\u{1b}[0m"))
}

pub(crate) fn red(text: &str) -> String {
    add_color(text, "\u{1b}[31m")
}

pub(crate) fn yellow(text: &str) -> String {
    add_color(text, "\u{1b}[33m")
}

/// `LineSplitter.convert`: lines ended by `\n`, `\r\n` or `\r`; the final
/// terminator does not start a new line.
pub(crate) fn lines(text: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => {
                result.push(&text[start..i]);
                start = i + 1;
            }
            b'\r' => {
                result.push(&text[start..i]);
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < bytes.len() {
        result.push(&text[start..]);
    }
    result
}

fn write_lines(out: &mut dyn Write, text: &str) {
    for line in lines(text) {
        let _ = writeln!(out, "{line}");
    }
    let _ = out.flush();
}

/// `log.message`.
pub(crate) fn message(text: &str) {
    write_lines(&mut std::io::stdout().lock(), text);
}

/// `log.warning`.
pub(crate) fn warning(text: &str) {
    write_lines(&mut std::io::stderr().lock(), text);
}

/// `log.error`.
pub(crate) fn error(text: &str) {
    write_lines(&mut std::io::stderr().lock(), text);
}

/// `log.spinner`: on a terminal the message stays on the line until the
/// task ends and is then erased; otherwise `<message>...` is printed as a
/// line of its own.
pub(crate) struct Spinner {
    erase: Option<usize>,
}

impl Spinner {
    pub fn start(text: &str) -> Self {
        if stdout_is_terminal() {
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "{text}... ");
            let _ = out.flush();
            Spinner {
                erase: Some(text.chars().count() + 4),
            }
        } else {
            message(&format!("{text}..."));
            Spinner { erase: None }
        }
    }

    /// `stopAndClear`.
    pub fn stop_and_clear(self) {
        if let Some(width) = self.erase {
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "{}", "\u{8}".repeat(width));
            let _ = out.flush();
        }
    }
}

/// The lines of `text` as `log.write` prints them, joined again: a final
/// line break is dropped.
pub(crate) fn as_logged(text: &str) -> String {
    lines(text).join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn splits_lines_like_dart() {
        assert_eq!(super::lines("a\n"), ["a"]);
        assert_eq!(super::lines("a\n\n"), ["a", ""]);
        assert_eq!(super::lines("\na"), ["", "a"]);
        assert!(super::lines("").is_empty());
        assert_eq!(super::lines("a\r\nb\rc"), ["a", "b", "c"]);
    }
}
