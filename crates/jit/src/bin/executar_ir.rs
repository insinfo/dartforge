//! `dartforge-execute_fn-ir <program.ll> [--timings] [-- <arguments do main>]`
//! — executa um LLVM IR do
//! backend nativo pelo JIT ORCv2, num processo só dele.
//!
//! É o executor isolado do perfil de desenvolvimento: o harness diferencial e
//! os testes rodam cada programa num processo deste binário, porque o runtime
//! encerra o processo (`process::exit`) nos mesmos casos em que o executável
//! AOT termina, e aborta num erro interno.
//!
//! Contrato de saída, o mesmo do executável AOT:
//!
//! * stdout é o do programa, e nada mais;
//! * o código de saída é o do programa (0, ou 255 pelo próprio runtime);
//! * `--timings` escreve **em stderr**, depois da execução, um objeto JSON com
//!   os campos `*_ns` de `docs/JIT.md` §Medição por fase e a versão da
//!   `LLVM-C.dll` carregada.
//!
//! Falha do próprio JIT (IR inválido, externo desconhecido, alvo divergente)
//! sai com código 70 (`EX_SOFTWARE`) e a etapa na primeira linha de stderr —
//! distinguível de qualquer código que o programa Dart produza.
use std::io::Write;
use std::process::ExitCode;

/// Código de saída para falha do JIT, e não do programa.
const JIT_FAILURE: u8 = 70;

fn main() -> ExitCode {
    let mut file = None;
    let mut timings = false;
    let mut arguments = std::env::args_os().skip(1);
    let mut from_program = Vec::new();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--timings") => timings = true,
            // O resto é do `main` do programa.
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
                "dartforge-executar-ir: não foi possível ler {}: {error}",
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

/// Mostra o uso e sai com código 2.
fn usage() -> ExitCode {
    eprintln!("uso: dartforge-executar-ir <programa.ll> [--timings] [-- <argumentos do main>]");
    ExitCode::from(2)
}
