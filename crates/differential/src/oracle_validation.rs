//! Validate corpus oracles without mistaking web-only VM rejection for a result.

use crate::{Output, ProgramResult};

impl ProgramResult {
    /// Reports an invalid corpus oracle, preserving declared web-only divergence.
    ///
    /// JS corpus validation requires DDC to complete normally. The VM must also
    /// complete normally, except for an explicitly declared divergence with a
    /// CFE platform-library rejection. Native validation always requires the VM.
    /// Negative compilation cases have already been normalized by the runners.
    /// This does not replace the separate check that declared divergences occur.
    ///
    /// # Errors
    /// Returns the executor, exit code and diagnostic when an oracle failed.
    ///
    /// ```
    /// use dartforge_differential::{Output, Program, ProgramResult};
    /// let output = Output { stdout: "ok\n".into(), stderr: String::new(), code: 0 };
    /// let result = ProgramResult { program: Program::test("example"),
    ///     dart: output.clone(), ddc: output, forge: None, native: false,
    ///     production: None, jit: None };
    /// assert!(result.oracle_failure().is_none());
    /// ```
    pub fn oracle_failure(&self) -> Option<String> {
        if !self.native && self.ddc.code != 0 {
            return Some(failure("DDC", &self.ddc));
        }
        let declared_web_rejection = !self.native
            && !self.program.compile_error
            && self.program.reference_is_ddc()
            && vm_platform_rejection(&self.dart);
        if self.dart.code != 0 && !declared_web_rejection {
            return Some(failure("Dart VM", &self.dart));
        }
        None
    }
}

fn failure(executor: &str, output: &Output) -> String {
    format!(
        "{executor} exited with {}: {}",
        output.code,
        output.first_stderr_line()
    )
}

fn vm_platform_rejection(output: &Output) -> bool {
    output.code == 254
        && output.stderr.lines().any(|line| {
            let Some((location, message)) = line.split_once(": Error: Dart library '") else {
                return false;
            };
            let Some((_, position)) = location.rsplit_once(".dart:") else {
                return false;
            };
            let Some((line, column)) = position.split_once(':') else {
                return false;
            };
            let valid_position = [line, column]
                .iter()
                .all(|n| n.parse::<usize>().is_ok_and(|n| n > 0));
            valid_position
                && message
                    .strip_suffix("' is not available on this platform.")
                    .is_some_and(|uri| {
                        uri.starts_with("dart:") && uri.len() > "dart:".len() && !uri.contains('\'')
                    })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Program, process::TIMEOUT_CODE};

    fn output(code: i32, stderr: &str) -> Output {
        Output {
            stdout: String::new(),
            stderr: stderr.into(),
            code,
        }
    }

    fn result(dart: Output, ddc: Output, declared: bool, native: bool) -> ProgramResult {
        let mut program = Program::test("oracle-validation");
        program.diverge_ddc = declared.then(|| "web behavior".into());
        ProgramResult {
            program,
            dart,
            ddc,
            forge: None,
            native,
            production: None,
            jit: None,
        }
    }

    fn rejection() -> Output {
        output(
            254,
            "main.dart:8:8: Error: Dart library 'dart:js_interop' is not available on this platform.\n",
        )
    }

    #[test]
    fn declared_platform_rejection_requires_a_successful_web_oracle() {
        let r = result(rejection(), output(0, ""), true, false);
        assert!(r.oracle_failure().is_none());
        assert!(
            r.ddc_vs_dart().is_some(),
            "the VM rejection remains a divergence"
        );
        for code in [-1, TIMEOUT_CODE, 1, 254] {
            let r = result(rejection(), output(code, "DDC unavailable"), true, false);
            assert!(r.oracle_failure().unwrap().starts_with("DDC"));
        }
    }

    #[test]
    fn declaration_does_not_authorize_other_vm_failures() {
        for (code, message) in [
            (-1, "Dart executable missing"),
            (TIMEOUT_CODE, "timeout"),
            (1, "Unhandled exception: broken program"),
            (254, "main.dart:3:4: Error: Expected ';' after this."),
            (
                254,
                "Error: Dart library 'dart:js_interop' is not available on this platform.",
            ),
            (
                254,
                "main.dart:0:8: Error: Dart library 'dart:js_interop' is not available on this platform.",
            ),
        ] {
            let r = result(output(code, message), output(0, ""), true, false);
            assert!(
                r.oracle_failure().unwrap().starts_with("Dart VM"),
                "{code}: {message}"
            );
        }
    }

    #[test]
    fn undeclared_and_native_platform_rejections_stay_invalid() {
        for (declared, native) in [(false, false), (false, true), (true, true)] {
            assert!(
                result(rejection(), output(0, ""), declared, native)
                    .oracle_failure()
                    .is_some()
            );
        }
        let mut negative = result(rejection(), output(0, ""), true, false);
        negative.program.compile_error = true;
        assert!(negative.oracle_failure().is_some());
    }

    #[test]
    fn shared_failures_are_not_valid_oracles() {
        let missing = output(-1, "missing tool");
        assert!(
            result(missing.clone(), missing, false, false)
                .oracle_failure()
                .is_some()
        );
        assert!(
            result(rejection(), rejection(), true, false)
                .oracle_failure()
                .is_some()
        );
        assert!(
            result(output(0, ""), output(-1, "missing DDC"), false, false)
                .oracle_failure()
                .is_some()
        );
    }

    #[test]
    fn oracle_only_report_does_not_hide_a_failed_ddc_behind_a_declared_divergence() {
        let r = result(
            rejection(),
            output(-1, "node executable missing"),
            true,
            false,
        );
        let report = crate::format_report(&[r]);
        assert!(
            report.contains(
                "ORACLE! oracle-validation (DDC exited with -1: node executable missing)"
            )
        );
        assert!(report.contains("DART!"), "the VM rejection remains visible");
    }

    #[test]
    fn successful_oracles_and_normalized_negative_cases_remain_valid() {
        for native in [false, true] {
            assert!(
                result(output(0, ""), output(0, ""), false, native)
                    .oracle_failure()
                    .is_none()
            );
        }
        let normalized = crate::oracles::rejection(Some(3));
        let mut negative = result(normalized.clone(), normalized, false, false);
        negative.program.compile_error = true;
        assert!(negative.oracle_failure().is_none());
        let r = result(output(0, ""), output(0, ""), true, false);
        assert!(r.oracle_failure().is_none());
        assert!(
            r.ddc_vs_dart().is_none(),
            "stale divergence declarations still need the separate gate"
        );
    }
}
