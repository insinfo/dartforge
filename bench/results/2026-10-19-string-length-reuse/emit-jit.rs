use dartforge_emit_native::{CompileOptions, contracts::ContractMode, emit_reloadable_ir};
use std::path::Path;
fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(|| {
        let options = CompileOptions {
            contracts: ContractMode::Verify,
            sdk: Some(Path::new("/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib")),
            packages: None, timings: false, optimize: false,
            language_version: None, experiments: Vec::new(), debug: false,
            generator: None, cpu: None,
        };
        let ir = emit_reloadable_ir(Path::new("/tmp/forge-string-length-reuse/lengths.dart"), &options, None).expect("emit JIT IR");
        std::fs::write("/tmp/forge-string-length-reuse/jit.ll", ir.text).expect("write JIT IR");
    }).unwrap().join().unwrap();
}
