//! Experiment: writes the emitted modules (without assembling the bundle).
//!
//! `cargo run -p dartforge-emit-js-production --example exp_modules -- <input.dart> <output-dir>`
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = std::path::PathBuf::from(&args[0]);
    let output = std::path::PathBuf::from(&args[1]);
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let l = dartforge_elements::sdk::LanguageOptions::default();
            let (e, _) = dartforge_emit_js::compile_with(&input, None, None, &l, |a| a.emit(None)).unwrap();
            std::fs::create_dir_all(&output).unwrap();
            for (p, t) in &e.modules {
                let d = output.join(p);
                std::fs::create_dir_all(d.parent().unwrap()).unwrap();
                std::fs::write(d, t).unwrap();
            }
            std::fs::write(output.join("main.mjs"), &e.entry).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
