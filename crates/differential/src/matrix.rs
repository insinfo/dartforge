//! The support matrix (V02): the state of each corpus program in each profile,
//! in a form that adds up across corpora, profiles and systems.
//!
//! The report says "234/234 ok", and that mixes different things: a program
//! that the profile runs exactly like the reference, one that the profile
//! **rejects** with a diagnostic just like the reference does (a library that
//! does not exist on the platform), and a feature that the profile rejects and
//! the reference runs. Here each (program, profile) pair has a single state:
//!
//! * `suportado` (supported): it compiled and the output (stdout and exit
//!   code) is the reference's;
//! * `recusado` (rejected): the profile rejected it at compile time, with the
//!   diagnostic — the same as the reference (`como a referência`) or not (the
//!   feature is missing in the profile);
//! * `divergente` (divergent): it ran and the output differs from the
//!   reference (including the ones in `PENDENTES`, marked);
//! * `não medido` (not measured): the reference could not be obtained
//!   (missing tool, timeout), so nothing is claimed.
//!
//! The file is TSV (`program  profile  state  detail`), one line per pair;
//! `scripts/matriz.py` merges the ones of each corpus, profile and system into
//! `docs/MATRIZ.md`. The state names, the profile names and the details are
//! read by that script and end up in that Portuguese document, so they stay
//! in Portuguese.

use std::fmt::Write;

use crate::process::{Output, TIMEOUT_CODE};
use crate::report::{Divergence, ProgramResult, compare};

/// The state of a program in a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Supported,
    Rejected,
    Divergent,
    NotMeasured,
}

impl State {
    /// The name written to the TSV (parsed by `scripts/matriz.py`).
    pub fn name(self) -> &'static str {
        match self {
            State::Supported => "suportado",
            State::Rejected => "recusado",
            State::Divergent => "divergente",
            State::NotMeasured => "não medido",
        }
    }
}

/// The output is that of a rejected compilation: the harness prefixes stderr
/// with the stage (`[compile-js …]`, `[compile-native]`, `[jsprod …]`) when
/// the compiler did not produce a program.
fn rejected_at_compile_time(s: &Output) -> bool {
    ["[compile-js", "[compile-native]", "[jsprod"].iter().any(|p| s.stderr.starts_with(p))
}

/// The reference was not obtained: missing tool (`Output::error`) or timeout.
fn reference_missing(s: &Output) -> bool {
    s.code == -1 || s.code == TIMEOUT_CODE
}

/// A single line, without tabs or line breaks (the detail goes in a TSV column).
fn single_line(s: &str) -> String {
    let t: String = s.lines().next().unwrap_or("").replace('\t', " ");
    if t.chars().count() > 160 {
        let mut c: String = t.chars().take(159).collect();
        c.push('…');
        c
    } else {
        t
    }
}

/// The state of `actual` against the reference, with the detail.
pub fn classify(reference: &Output, actual: &Output, pending: bool) -> (State, String) {
    if reference_missing(reference) {
        return (State::NotMeasured, single_line(reference.first_stderr_line()));
    }
    match compare(reference, actual) {
        None if rejected_at_compile_time(actual) => (State::Rejected, "como a referência".into()),
        None => (State::Supported, String::new()),
        Some(_) if rejected_at_compile_time(actual) => (State::Rejected, single_line(actual.first_stderr_line())),
        Some(d) => {
            let place = match d {
                Divergence::ExitCode => format!("código de saída {} (referência {})", actual.code, reference.code),
                Divergence::Line(n) => format!("stdout linha {n}"),
            };
            (State::Divergent, if pending { format!("PENDENTES; {place}") } else { place })
        }
    }
}

/// The TSV lines of the results: the main executor in profile `profile` and,
/// when present, the JS production profile as `js-prod`.
pub fn tsv(results: &[ProgramResult], profile: &str) -> String {
    let mut out = String::new();
    for r in results {
        let reference = r.reference();
        if let Some(f) = &r.forge {
            let (e, d) = classify(reference, f, r.program.pending);
            let _ = writeln!(out, "{}\t{profile}\t{}\t{d}", r.program.name, e.name());
        }
        if let Some(p) = &r.production {
            let (e, d) = classify(reference, p, r.program.pending);
            let _ = writeln!(out, "{}\tjs-prod\t{}\t{d}", r.program.name, e.name());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(stdout: &str, stderr: &str, code: i32) -> Output {
        Output { stdout: stdout.into(), stderr: stderr.into(), code }
    }

    #[test]
    fn the_four_states() {
        let vm = s("1\n", "", 0);
        assert_eq!(classify(&vm, &s("1\n", "", 0), false).0, State::Supported);
        assert_eq!(classify(&vm, &s("2\n", "", 0), false), (State::Divergent, "stdout linha 1".into()));
        assert_eq!(classify(&vm, &s("1\n", "", 255), true).1, "PENDENTES; código de saída 255 (referência 0)");
        let rejection = s("", "[compile-native] não suportado no backend nativo: X\ndetail", 1);
        assert_eq!(classify(&vm, &rejection, false), (State::Rejected, "[compile-native] não suportado no backend nativo: X".into()));
        // The VM rejects it too (library missing on the platform): equal.
        let vm_rejection = s("", "main.dart:1:8: Error: Dart library 'dart:js' is not available", 254);
        let forge_rejection = s("", "[compile-native] library unavailable", 254);
        assert_eq!(classify(&vm_rejection, &forge_rejection, false), (State::Rejected, "como a referência".into()));
        assert_eq!(classify(&Output::error("dart missing"), &vm, false).0, State::NotMeasured);
    }
}
