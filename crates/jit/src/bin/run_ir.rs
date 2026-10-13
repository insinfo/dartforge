//! `dartforge-executar-ir <program.ll> [--timings] [-- <main arguments>]`
//! — runs an LLVM IR of the
//! native backend through the ORCv2 JIT, in a process of its own.
//!
//! It is the isolated executor of the development profile: the differential harness and
//! the tests run each program in a process of this binary, because the runtime
//! ends the process (`process::exit`) in the same cases in which the
//! AOT executable ends, and aborts on an internal error.
//!
//! Output contract, the same as the AOT executable's:
//!
//! * stdout is the program's, and nothing else;
//! * the exit code is the program's (0, or 255 from the runtime itself);
//! * `--timings` writes **to stderr**, after execution, a JSON object with
//!   the `*_ns` fields of `docs/JIT.md` §Medição por fase and the version of the
//!   loaded `LLVM-C.dll`.
//!
//! A failure of the JIT itself (invalid IR, unknown external, divergent target)
//! exits with code 70 (`EX_SOFTWARE`) and the stage on the first line of stderr —
//! distinguishable from any code the Dart program produces.
use std::io::Write;
use std::process::ExitCode;

/// Exit code for a JIT failure, not a program failure.
const JIT_FAILURE: u8 = 70;

fn main() -> ExitCode {
    let mut file = None;
    let mut timings = false;
    let mut arguments = std::env::args_os().skip(1);
    let mut from_program = Vec::new();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--timings") => timings = true,
            // The rest belongs to the program's `main`.
            Some("--") => {
                from_program.extend(arguments.by_ref().map(|a| a.to_string_lossy().into_owned()));
            }
            _ if file.is_none() => file = Some(argument),
            _ => return usage(),
        }
    }
    dartforge_jit::definir_argumentos(from_program);
    let Some(file) = file else {
        return usage();
    };
    let ir = match std::fs::read_to_string(&file) {
        Ok(ir) => ir,
        Err(error) => {
            eprintln!(
                "dartforge-executar-ir: could not read {}: {error}",
                std::path::Path::new(&file).display()
            );
            return ExitCode::from(JIT_FAILURE);
        }
    };
    let report_data = match dartforge_jit::run_ir(&ir) {
        Ok(report_data) => report_data,
        Err(error) => {
            let _ = std::io::stdout().flush();
            eprintln!("dartforge-executar-ir: {error}");
            return ExitCode::from(JIT_FAILURE);
        }
    };
    let _ = std::io::stdout().flush();
    if timings {
        let (major, minor, patch) = dartforge_jit::JitSession::llvm_version();
        eprintln!(
            "{{\"llvm\":\"{major}.{minor}.{patch}\",\"ir_bytes\":{},\"session_ns\":{},\"parse_ir_ns\":{},\
             \"add_module_ns\":{},\"lookup_ns\":{},\"execute_ns\":{},\"jit_total_ns\":{}}}",
            report_data.module.ir_bytes,
            report_data.session.as_nanos(),
            report_data.module.parse_ir.as_nanos(),
            report_data.module.add_module.as_nanos(),
            report_data.entry.lookup.as_nanos(),
            report_data.entry.execute.as_nanos(),
            report_data.total.as_nanos(),
        );
    }
    ExitCode::from(u8::try_from(report_data.entry.exit_code).unwrap_or(JIT_FAILURE))
}

/// Shows the usage and exits with code 2.
fn usage() -> ExitCode {
    eprintln!("usage: dartforge-executar-ir <program.ll> [--timings] [-- <main arguments>]");
    ExitCode::from(2)
}
