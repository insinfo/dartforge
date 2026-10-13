//! Lists the inference warnings with file and line, and a summary per code.
//! An inference diagnostic tool, not a product.
//!
//! ```text
//! # project
//! cargo run --release -p dartforge-types --example avisos -- \
//!     projeto <entrada.dart> <package_config.json>
//! # SDK libraries compiled from source (native backend), with the overlay
//! cargo run --release -p dartforge-types --example avisos -- \
//!     sdk <sdk/lib> <sobreposicao> <alvo> core async ...
//! ```
//!
//! Output: `arquivo:line\tmensagem\ttrecho`, and at the end the total per code
//! (the text of the message template).

use dartforge_elements::load::load_lenient;
use dartforge_elements::model::{LibraryId, Program, UnitId};
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use dartforge_types::{resolve_outline, BodyInferrer};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || executar(a))
        .unwrap()
        .join()
        .unwrap();
}

fn executar(a: Vec<String>) {
    let (sdk, entry, packages, requested): (SdkLayout, PathBuf, Option<PathBuf>, Vec<String>) = match a[0].as_str() {
        "projeto" => {
            let dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
            (SdkLayout::load(&dir, "dartdevc").unwrap(), PathBuf::from(&a[1]), Some(PathBuf::from(&a[2])), Vec::new())
        }
        "sdk" => {
            let sdk = SdkLayout::load_com_sobreposicao(Path::new(&a[1]), Path::new(&a[2]), &a[3]).unwrap();
            let libs: Vec<String> = a[4..].to_vec();
            let tmp = std::env::temp_dir().join(format!("dartforge-avisos-sdk-{}", std::process::id()));
            std::fs::create_dir_all(&tmp).unwrap();
            let entry = tmp.join("main.dart");
            let mut source: String = libs.iter().map(|l| format!("import 'dart:{l}';\n")).collect();
            source.push_str("void main() {}\n");
            std::fs::write(&entry, source).unwrap();
            (sdk, entry, None, libs)
        }
        _ => panic!("usage: avisos projeto <entrada> <cfg> | avisos sdk <lib> <sobreposicao> <alvo> <libs...>"),
    };
    let mut interner = Interner::new();
    let (prog, _) = load_lenient(&entry, &sdk, packages.as_deref(), &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let mut by_code: BTreeMap<String, usize> = BTreeMap::new();
    let mut list_ = |prog: &Program, diags: Vec<dartforge_diagnostics::Diagnostic>, units: Vec<Option<UnitId>>| {
        for (d, u) in diags.iter().zip(units.iter()) {
            let code = d.message.split(": ").next().unwrap_or(&d.message).to_string();
            *by_code.entry(code).or_default() += 1;
            let (file, line, snippet) = match u {
                Some(u) => {
                    let un = prog.unit(*u);
                    let src = &un.source;
                    let start = d.span.start.min(src.len());
                    let end = d.span.end.min(src.len()).min(start + 80);
                    let line = src[..start].matches('\n').count() + 1;
                    let snippet = src.get(start..end).unwrap_or("").replace(['\n', '\r'], " ");
                    (un.uri.clone(), line, snippet)
                }
                None => ("?".into(), 0, String::new()),
            };
            println!("{file}:{line}\t{}\t{snippet}", d.message);
        }
    };
    if requested.is_empty() {
        let inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
        let (_, d, u) = inf.infer_all_with_units();
        list_(&prog, d, u);
    } else {
        for name in &requested {
            let uri = format!("dart:{name}");
            let Some(i) = prog.libraries.iter().position(|l| l.uri == uri) else { continue };
            let mut inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
            inf.apenas_bibliotecas = Some([LibraryId(i as u32).0].into_iter().collect());
            let (_, d, u) = inf.infer_all_with_units();
            list_(&prog, d, u);
        }
    }
    let mut v: Vec<(String, usize)> = by_code.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("# total: {}", v.iter().map(|x| x.1).sum::<usize>());
    for (c, n) in v {
        println!("# {n:6}  {c}");
    }
}
