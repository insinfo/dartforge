use std::{env, fs, path::PathBuf, process::ExitCode};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        println!(
            "DartForge bootstrap\nUsage: dartforge compile <input.dart> <output.mjs>\nSupported subset: void main(), local variables, blocks, int/String/bool expressions and print."
        );
        return Ok(());
    }
    if args.len() != 3 || args[0] != "compile" {
        return Err("usage: dartforge compile <input.dart> <output.mjs>".into());
    }
    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let source = fs::read_to_string(&input)?;
    let js = dartforge_compiler::compile(&source)?;
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(js.as_bytes())?;
    println!(
        "{} -> {} ({} bytes)",
        input.display(),
        output.display(),
        js.len()
    );
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
