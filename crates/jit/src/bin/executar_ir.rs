//! `dartforge-executar-ir <programa.ll> [--timings]` — executa um LLVM IR do
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
//! * o código de saída é o do programa (0, ou 101/255 pelo próprio runtime);
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
const FALHA_DO_JIT: u8 = 70;

fn main() -> ExitCode {
    let mut arquivo = None;
    let mut timings = false;
    for argumento in std::env::args_os().skip(1) {
        match argumento.to_str() {
            Some("--timings") => timings = true,
            _ if arquivo.is_none() => arquivo = Some(argumento),
            _ => return uso(),
        }
    }
    let Some(arquivo) = arquivo else {
        return uso();
    };
    let ir = match std::fs::read_to_string(&arquivo) {
        Ok(ir) => ir,
        Err(erro) => {
            eprintln!(
                "dartforge-executar-ir: não foi possível ler {}: {erro}",
                std::path::Path::new(&arquivo).display()
            );
            return ExitCode::from(FALHA_DO_JIT);
        }
    };
    let relatorio = match dartforge_jit::run_ir(&ir) {
        Ok(relatorio) => relatorio,
        Err(erro) => {
            let _ = std::io::stdout().flush();
            eprintln!("dartforge-executar-ir: {erro}");
            return ExitCode::from(FALHA_DO_JIT);
        }
    };
    let _ = std::io::stdout().flush();
    if timings {
        let (major, minor, patch) = dartforge_jit::JitSession::llvm_version();
        eprintln!(
            "{{\"llvm\":\"{major}.{minor}.{patch}\",\"ir_bytes\":{},\"session_ns\":{},\"parse_ir_ns\":{},\
             \"add_module_ns\":{},\"lookup_ns\":{},\"execute_ns\":{},\"jit_total_ns\":{}}}",
            relatorio.module.ir_bytes,
            relatorio.session.as_nanos(),
            relatorio.module.parse_ir.as_nanos(),
            relatorio.module.add_module.as_nanos(),
            relatorio.entry.lookup.as_nanos(),
            relatorio.entry.execute.as_nanos(),
            relatorio.total.as_nanos(),
        );
    }
    ExitCode::from(u8::try_from(relatorio.entry.exit_code).unwrap_or(FALHA_DO_JIT))
}

/// Mostra o uso e sai com código 2.
fn uso() -> ExitCode {
    eprintln!("uso: dartforge-executar-ir <programa.ll> [--timings]");
    ExitCode::from(2)
}
