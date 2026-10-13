//! Structural gate of the zero-cost rule (PLANO.md, "geração de código e
//! macros", item 1): a program without a `macro` class opens no macro
//! session, talks to no executor and leaves the host exactly as it came in —
//! like `dartforge_build::instance_count()` for the build engine.
use dartforge_diagnostics::Diagnostic;
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_macros_host::assembly::ExecutionResult;
use dartforge_macros_host::executor::{Availability, ExecutionRequest, MacroExecutor, QueryService};
use dartforge_macros_host::protocol::Handshake;
use serde_json::Value;
use std::path::Path;

/// An executor that fails the test if it is touched.
struct Forbidden;

impl MacroExecutor for Forbidden {
    fn availability(&self) -> Availability {
        panic!("zero cost: the executor must not be queried")
    }
    fn start(&mut self) -> Result<Handshake, String> {
        panic!("zero cost: the executor must not be started")
    }
    fn instantiate(&mut self, _: &str, _: &str, _: &Value) -> Result<(u64, Vec<String>), String> {
        panic!("zero cost")
    }
    fn execute(&mut self, _: &ExecutionRequest, _: &mut dyn QueryService) -> Result<ExecutionResult, String> {
        panic!("zero cost")
    }
    fn shut_down(&mut self) {
        panic!("zero cost")
    }
}

#[test]
fn no_macro_opens_no_session() {
    let Some(lib) = SdkLayout::discover() else {
        eprintln!("no SDK: skipped");
        return;
    };
    let sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/js/01_print.dart");
    let mut names = Interner::new();
    let (p, d) = load_lenient(&input, &sdk, None, &mut names);
    assert!(d.is_empty(), "{d:?}");
    assert!(!dartforge_macros_host::has_macros(&p), "01_print declares no macro class");
    let libraries = p.libraries.len();
    let mut reloads = 0usize;
    let mut load = |_: &mut Interner, _| -> (dartforge_elements::model::Program, Vec<Diagnostic>) {
        reloads += 1;
        panic!("zero cost: no reload")
    };
    let output = dartforge_macros_host::apply(p, &mut names, None, &mut load, &mut Forbidden)
        .unwrap_or_else(|_| panic!("without macros there is no error"));
    assert_eq!(output.program.libraries.len(), libraries);
    assert!(output.texts.is_empty() && output.generation.is_none() && output.executed_macros == 0);
    assert_eq!(dartforge_macros_host::sessions(), 0);
    assert_eq!(reloads, 0);
}
