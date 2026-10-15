//! Emit once, then measure identical saved IR in fresh JIT processes.
use std::path::{Path, PathBuf};
use std::time::Instant;
#[cfg(emit_only)]
use dartforge_emit_native::{CompileOptions, contracts::ContractMode};
#[cfg(not(emit_only))]
use dartforge_jit::JitSession;
use serde_json::json;

#[cfg(emit_only)]
fn emit(entry: PathBuf, sdk: PathBuf, packages: PathBuf, output: PathBuf) -> Result<(), String> {
    let selected = std::env::var_os("DARTFORGE_SDK_LIB").map(PathBuf::from)
        .ok_or("DARTFORGE_SDK_LIB must identify the same selected SDK")?;
    if selected.canonicalize().map_err(|error|error.to_string())? != sdk.canonicalize().map_err(|error|error.to_string())? {
        return Err("The source SDK DLL selection must match the IR emission SDK".into());
    }
    let options = CompileOptions {
        contracts: ContractMode::Verify, sdk: Some(&sdk), packages: Some(&packages),
        timings: false, optimize: false, language_version: None,
        experiments: Vec::new(), debug: false, generator: None, cpu: None,
    };
    let started = Instant::now();
    let ir = dartforge_emit_native::emit_reloadable_ir(&entry, &options, None)?;
    let emission_ms = started.elapsed().as_secs_f64() * 1000.0;
    std::fs::write(&output, &ir.text).map_err(|error| error.to_string())?;
    let dll = dartforge_emit_native::sdk_module::source_sdk_dll()?;
    println!("{}", json!({"phase":"emit","entry":entry,"sdk":sdk,"packages":packages,
        "saved_ir":output,"ir_bytes":ir.text.len(),"emission_ms":emission_ms,"sdk_dll":dll,
        "scope":"IR emission only; no JIT linking or application execution."}));
    Ok(())
}

#[cfg(not(emit_only))]
fn link(input: &Path, dll: Option<&Path>, call: Option<&str>) -> Result<(), String> {
    let ir = std::fs::read_to_string(input).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut session = JitSession::new_for_ir_with(&ir, dll).map_err(|error| error.to_string())?;
    let session_ms = started.elapsed().as_secs_f64() * 1000.0;
    let report = session.add_reloadable_module("probe", &ir).map_err(|error| error.to_string())?;
    let result = call.map(|name| session.stable_entry(name).and_then(|entry| entry.call(&session)))
        .transpose().map_err(|error| error.to_string())?;
    if call.is_some() && result != Some(10) { return Err(format!("unexpected synthetic result: {result:?}")); }
    println!("{}", json!({"phase":"link","saved_ir":input,"ir_bytes":ir.len(),
        "eager_stubs":std::env::var("DARTFORGE_JIT_EAGER_STUBS").ok(),"sdk_dll":dll,
        "session_ms":session_ms,"parse_ir_ms":report.parse_ir.as_secs_f64()*1000.0,
        "contract_ms":report.contract.as_secs_f64()*1000.0,"add_module_ms":report.add_module.as_secs_f64()*1000.0,
        "stubs_ms":report.stubs.as_secs_f64()*1000.0,"link_ms":report.link.as_secs_f64()*1000.0,
        "publish_ms":report.publish.as_secs_f64()*1000.0,"retire_ms":report.retire.as_secs_f64()*1000.0,
        "total_ms":report.total.as_secs_f64()*1000.0,"entries":report.entries,"new_entries":report.new_entries,
        "synthetic_result":result,"scope":"Identical saved IR; application main is never executed."}));
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        #[cfg(emit_only)]
        Some("emit") if args.len()==6 => {
            let (entry,sdk,packages,output)=(PathBuf::from(&args[2]),PathBuf::from(&args[3]),PathBuf::from(&args[4]),PathBuf::from(&args[5]));
            std::thread::Builder::new().stack_size(1<<30).spawn(move||emit(entry,sdk,packages,output))
                .map_err(|error|error.to_string())?.join().map_err(|_|"IR emission thread panicked".to_string())?
        }
        #[cfg(not(emit_only))]
        Some("link") if args.len()==4 => link(Path::new(&args[2]),Some(Path::new(&args[3])),None),
        #[cfg(not(emit_only))]
        Some("small") if args.len()==2 => {
            let ir="define i64 @df_probe_leaf() { ret i64 9 }\ndefine i64 @df_probe_root() {\n %value = call i64 @df_probe_leaf()\n %sum = add i64 %value, 1\n ret i64 %sum\n}\n";
            let path=std::env::temp_dir().join(format!("jit-eager-small-{}.ll",std::process::id()));
            std::fs::write(&path,ir).map_err(|error|error.to_string())?;
            link(&path,None,Some("df_probe_root"))
        }
        _=>Err("usage: probe emit <entry.dart> <sdk/lib> <package_config.json> <saved.ll> | link <saved.ll> <sdk.dylib> | small".into()),
    }
}
