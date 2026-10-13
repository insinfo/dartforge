//! Grava os tipos esperados do corpus de conformidade da inferência
//! (`corpus/inferencia/`) com o oráculo (`tools/oraculo_tipos/oraculo.dart`,
//! package:analyzer). Só orquestra: roda UM processo `dart` para o corpus
//! inteiro e reparte a saída por programa.
//!
//! ```text
//! cargo run --release -p dartforge-types --example gravar_corpus -- \
//!     --packages <package_config.json> [corpus/inferencia]
//! ```
//!
//! `<package_config.json>` é qualquer um que resolva `package:analyzer`
//! 6.11.0 e `package:path` (o do `new_sali/frontend`, por exemplo). Para cada
//! `X.dart` escreve `X.esperado.tsv`, uma linha por expressão:
//!
//! ```text
//! linha  coluna  offset  comprimento  nó  tipo  elemento  marca  trecho
//! ```
//!
//! `offset`/`comprimento` são unidades UTF-16 (as do analyzer);
//! `linha`/`coluna` começam em 1. `marca` é 1 quando a expressão começa logo
//! depois de um comentário `/*@*/`. Ver docs/INFERENCIA-ESPECIFICACAO.md, §0.
//! O teste `tests/corpus_inferencia.rs` compara o nosso despejo com esses
//! arquivos.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

const MARCA: &str = "/*@*/";

fn programs(dir: &Path, v: &mut Vec<PathBuf>) {
    let mut es: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    es.sort();
    for p in es {
        if p.is_dir() {
            programs(&p, v);
        } else if p.extension().is_some_and(|x| x == "dart") {
            v.push(p);
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let i = args.iter().position(|a| a == "--packages").expect("uso: gravar_corpus --packages <package_config.json> [raiz]");
    let packages = args[i + 1].clone();
    args.drain(i..i + 2);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = match args.first() {
        Some(r) => PathBuf::from(r),
        None => repo.join("corpus/inferencia"),
    };
    let root = std::fs::canonicalize(&root).unwrap_or(root);
    let root_txt = root.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
    let root = PathBuf::from(&root_txt);
    let mut progs = Vec::new();
    programs(&root, &mut progs);
    let progs: Vec<String> = progs.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();

    let tmp = std::env::temp_dir();
    let list = tmp.join("corpus_inferencia_arquivos.txt");
    let output = tmp.join("corpus_inferencia_oraculo.tsv");
    std::fs::write(&list, progs.join("\n") + "\n").unwrap();
    let oracle = repo.join("tools/oraculo_tipos/oraculo.dart");
    let st = std::process::Command::new("dart")
        .arg(format!("--packages={packages}"))
        .arg(&oracle)
        .arg(&root_txt)
        .arg(&list)
        .arg(&output)
        .status()
        .expect("dart");
    assert!(st.success(), "oráculo falhou: {st}");

    // Linhas do oráculo por programa (caminho em minúsculas).
    let mut by: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    for l in BufReader::new(std::fs::File::open(&output).unwrap()).lines() {
        let l = l.unwrap();
        if l.starts_with('#') {
            panic!("oráculo falhou: {l}");
        }
        let c: Vec<String> = l.split('\t').map(String::from).collect();
        if c.len() >= 6 {
            by.entry(c[0].to_lowercase()).or_default().push(c);
        }
    }
    for prog in &progs {
        let text = std::fs::read_to_string(prog).unwrap();
        let u16: Vec<u16> = text.encode_utf16().collect();
        let mut lines: Vec<(usize, isize, String)> = Vec::new();
        for c in by.get(&prog.to_lowercase()).map(Vec::as_slice).unwrap_or(&[]) {
            let (Ok(start), Ok(comp)) = (c[1].parse::<usize>(), c[2].parse::<usize>()) else { continue };
            let (no, ty, el) = (&c[3], &c[4], &c[5]);
            let before = String::from_utf16_lossy(&u16[..start.min(u16.len())]);
            let lin = before.matches('\n').count() + 1;
            let col = before.chars().count() - before.rfind('\n').map(|p| before[..=p].chars().count()).unwrap_or(0) + 1;
            let mark = u8::from(before.trim_end_matches(' ').ends_with(MARCA));
            let end = (start + comp.min(60)).min(u16.len());
            let snippet = String::from_utf16_lossy(&u16[start.min(end)..end]);
            let snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
            lines.push((start, -(comp as isize), format!("{lin}\t{col}\t{start}\t{comp}\t{no}\t{ty}\t{el}\t{mark}\t{snippet}")));
        }
        lines.sort();
        let rel = prog.strip_prefix(&format!("{root_txt}/")).unwrap_or(prog);
        let destination = format!("{}.esperado.tsv", prog.trim_end_matches(".dart"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&destination).unwrap());
        writeln!(f, "# {rel} — gravado pelo oráculo (package:analyzer 6.11.0, Dart 3.6.2)").unwrap();
        writeln!(f, "# linha\tcoluna\toffset\tcomprimento\tnó\ttipo\telemento\tmarca\ttrecho").unwrap();
        for (_, _, l) in lines {
            writeln!(f, "{l}").unwrap();
        }
    }
    println!("{} programas gravados em {root_txt}", progs.len());
}
