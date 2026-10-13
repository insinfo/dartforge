//! Type dump: writes, for each body expression, the static type our inference
//! gave it, in the same format as the `package:analyzer` oracle
//! (`tools/oraculo_tipos/oraculo.dart`). It is an inference diagnostic tool,
//! not a product (the logic lives in `dartforge_types::despejo`).
//!
//! ```text
//! cargo run --release -p dartforge-types --example despejo_tipos -- \
//!     <entrada.dart> --packages <package_config.json> -o <saida.tsv> \
//!     [--arquivos <lista.txt>] [--avisos <avisos.txt>] [--sdk <lib>]
//! ```
//!
//! Line: `caminho \t offset \t comprimento \t nó \t tipo \t resolução`.
//! An expression the inference never visited is output with the type `?`.
//! `--files` receives the list of (non-SDK) units for the oracle;
//! `--warnings` receives the warnings, one per line, with the offset.

use dartforge_elements::sdk::SdkLayout;
use dartforge_types::despejo::dump;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut entry = None;
    let mut packages = None;
    let mut output = None;
    let mut files = None;
    let mut warnings = None;
    let mut sdk_dir = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packages" => packages = it.next().map(PathBuf::from),
            "-o" => output = it.next().map(PathBuf::from),
            "--arquivos" => files = it.next().map(PathBuf::from),
            "--avisos" => warnings = it.next().map(PathBuf::from),
            "--sdk" => sdk_dir = it.next().map(PathBuf::from),
            _ => entry = Some(PathBuf::from(a)),
        }
    }
    let entry = entry.expect("usage: despejo_tipos <entrada.dart> --packages <cfg> -o <saida.tsv>");
    let sdk_dir = sdk_dir
        .or_else(SdkLayout::discover)
        .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("SDK");

    // Deep bodies recurse deeply: own stack, like `compile-js`.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let d = dump(&entry, &sdk, packages.as_deref());
            eprintln!("inference: {:?}; warnings: {}", d.inference_time, d.avisos.len());
            let mut out = std::io::BufWriter::new(std::fs::File::create(output.expect("-o")).unwrap());
            let mut total = 0usize;
            for u in &d.unidades {
                for l in &u.linhas {
                    writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}", u.caminho, l.offset, l.comprimento, l.no, l.tipo, l.resolution).unwrap();
                }
                total += u.linhas.len();
            }
            eprintln!("dump: {} units, {total} expressions, {} not visited", d.unidades.len(), d.not_visited);
            if let Some(a) = files {
                let list: Vec<&str> = d.unidades.iter().map(|u| u.caminho.as_str()).collect();
                std::fs::write(a, list.join("\n")).unwrap();
            }
            if let Some(a) = warnings {
                let mut f = std::io::BufWriter::new(std::fs::File::create(a).unwrap());
                for x in &d.avisos {
                    writeln!(f, "{}\t{}", x.span.start, x.message.replace('\n', " ")).unwrap();
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
