//! Despejo de tipos: grava, por expressão de corpo, o tipo estático que a
//! nossa inferência deu, no mesmo formato do oráculo do `package:analyzer`
//! (`tools/oraculo_tipos/oraculo.dart`). É ferramenta de diagnóstico da
//! inferência, não produto (a lógica fica em `dartforge_types::despejo`).
//!
//! ```text
//! cargo run --release -p dartforge-types --example despejo_tipos -- \
//!     <entrada.dart> --packages <package_config.json> -o <saida.tsv> \
//!     [--arquivos <lista.txt>] [--avisos <avisos.txt>] [--sdk <lib>]
//! ```
//!
//! Linha: `caminho \t offset \t comprimento \t nó \t tipo \t resolução`.
//! Expressão que a inferência nunca visitou sai com o tipo `?`.
//! `--arquivos` recebe a lista de unidades (não-SDK) para o oráculo;
//! `--avisos` recebe os avisos, um por linha, com o offset.

use dartforge_elements::sdk::SdkLayout;
use dartforge_types::despejo::despejar;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut entrada = None;
    let mut packages = None;
    let mut saida = None;
    let mut arquivos = None;
    let mut avisos = None;
    let mut sdk_dir = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--packages" => packages = it.next().map(PathBuf::from),
            "-o" => saida = it.next().map(PathBuf::from),
            "--arquivos" => arquivos = it.next().map(PathBuf::from),
            "--avisos" => avisos = it.next().map(PathBuf::from),
            "--sdk" => sdk_dir = it.next().map(PathBuf::from),
            _ => entrada = Some(PathBuf::from(a)),
        }
    }
    let entrada = entrada.expect("uso: despejo_tipos <entrada.dart> --packages <cfg> -o <saida.tsv>");
    let sdk_dir = sdk_dir
        .or_else(SdkLayout::discover)
        .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = SdkLayout::load(&sdk_dir, "dartdevc").expect("SDK");

    // Corpos profundos recursam fundo: pilha própria, como o `compile-js`.
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let d = despejar(&entrada, &sdk, packages.as_deref());
            eprintln!("inferência: {:?}; avisos: {}", d.tempo_inferencia, d.avisos.len());
            let mut out = std::io::BufWriter::new(std::fs::File::create(saida.expect("-o")).unwrap());
            let mut total = 0usize;
            for u in &d.unidades {
                for l in &u.linhas {
                    writeln!(out, "{}\t{}\t{}\t{}\t{}\t{}", u.caminho, l.offset, l.comprimento, l.no, l.tipo, l.resolucao).unwrap();
                }
                total += u.linhas.len();
            }
            eprintln!("despejo: {} unidades, {total} expressões, {} não visitadas", d.unidades.len(), d.nao_visitadas);
            if let Some(a) = arquivos {
                let lista: Vec<&str> = d.unidades.iter().map(|u| u.caminho.as_str()).collect();
                std::fs::write(a, lista.join("\n")).unwrap();
            }
            if let Some(a) = avisos {
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
