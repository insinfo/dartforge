//! Lista os avisos da inferência com arquivo e linha, e um resumo por código.
//! Ferramenta de diagnóstico da inferência, não produto.
//!
//! ```text
//! # projeto
//! cargo run --release -p dartforge-types --example avisos -- \
//!     projeto <entrada.dart> <package_config.json>
//! # bibliotecas do SDK compiladas da fonte (backend nativo), com a sobreposição
//! cargo run --release -p dartforge-types --example avisos -- \
//!     sdk <sdk/lib> <sobreposicao> <alvo> core async ...
//! ```
//!
//! Saída: `arquivo:linha\tmensagem\ttrecho`, e no fim o total por código
//! (o texto do modelo da mensagem).

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
    let (sdk, entrada, packages, pedidas): (SdkLayout, PathBuf, Option<PathBuf>, Vec<String>) = match a[0].as_str() {
        "projeto" => {
            let dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
            (SdkLayout::load(&dir, "dartdevc").unwrap(), PathBuf::from(&a[1]), Some(PathBuf::from(&a[2])), Vec::new())
        }
        "sdk" => {
            let sdk = SdkLayout::load_com_sobreposicao(Path::new(&a[1]), Path::new(&a[2]), &a[3]).unwrap();
            let libs: Vec<String> = a[4..].to_vec();
            let tmp = std::env::temp_dir().join(format!("dartforge-avisos-sdk-{}", std::process::id()));
            std::fs::create_dir_all(&tmp).unwrap();
            let entrada = tmp.join("main.dart");
            let mut fonte: String = libs.iter().map(|l| format!("import 'dart:{l}';\n")).collect();
            fonte.push_str("void main() {}\n");
            std::fs::write(&entrada, fonte).unwrap();
            (sdk, entrada, None, libs)
        }
        _ => panic!("uso: avisos projeto <entrada> <cfg> | avisos sdk <lib> <sobreposicao> <alvo> <libs...>"),
    };
    let mut interner = Interner::new();
    let (prog, _) = load_lenient(&entrada, &sdk, packages.as_deref(), &mut interner);
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &prog, &interner);
    let (mut outline, _) = resolve_outline(&prog, &interner, &mut table, &core);
    let mut por_codigo: BTreeMap<String, usize> = BTreeMap::new();
    let mut listar = |prog: &Program, diags: Vec<dartforge_diagnostics::Diagnostic>, unidades: Vec<Option<UnitId>>| {
        for (d, u) in diags.iter().zip(unidades.iter()) {
            let codigo = d.message.split(": ").next().unwrap_or(&d.message).to_string();
            *por_codigo.entry(codigo).or_default() += 1;
            let (arq, linha, trecho) = match u {
                Some(u) => {
                    let un = prog.unit(*u);
                    let src = &un.source;
                    let ini = d.span.start.min(src.len());
                    let fim = d.span.end.min(src.len()).min(ini + 80);
                    let linha = src[..ini].matches('\n').count() + 1;
                    let trecho = src.get(ini..fim).unwrap_or("").replace(['\n', '\r'], " ");
                    (un.uri.clone(), linha, trecho)
                }
                None => ("?".into(), 0, String::new()),
            };
            println!("{arq}:{linha}\t{}\t{trecho}", d.message);
        }
    };
    if pedidas.is_empty() {
        let inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
        let (_, d, u) = inf.infer_all_com_unidades();
        listar(&prog, d, u);
    } else {
        for nome in &pedidas {
            let uri = format!("dart:{nome}");
            let Some(i) = prog.libraries.iter().position(|l| l.uri == uri) else { continue };
            let mut inf = BodyInferrer::new(&prog, &interner, &mut table, &core, &mut outline);
            inf.apenas_bibliotecas = Some([LibraryId(i as u32).0].into_iter().collect());
            let (_, d, u) = inf.infer_all_com_unidades();
            listar(&prog, d, u);
        }
    }
    let mut v: Vec<(String, usize)> = por_codigo.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    println!("# total: {}", v.iter().map(|x| x.1).sum::<usize>());
    for (c, n) in v {
        println!("# {n:6}  {c}");
    }
}
