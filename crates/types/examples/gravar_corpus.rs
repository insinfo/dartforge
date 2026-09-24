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

fn programas(dir: &Path, v: &mut Vec<PathBuf>) {
    let mut es: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    es.sort();
    for p in es {
        if p.is_dir() {
            programas(&p, v);
        } else if p.extension().is_some_and(|x| x == "dart") {
            v.push(p);
        }
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let i = args.iter().position(|a| a == "--packages").expect("uso: gravar_corpus --packages <package_config.json> [raiz]");
    let pacotes = args[i + 1].clone();
    args.drain(i..i + 2);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let raiz = match args.first() {
        Some(r) => PathBuf::from(r),
        None => repo.join("corpus/inferencia"),
    };
    let raiz = std::fs::canonicalize(&raiz).unwrap_or(raiz);
    let raiz_txt = raiz.to_string_lossy().trim_start_matches(r"\\?\").replace('\\', "/");
    let raiz = PathBuf::from(&raiz_txt);
    let mut progs = Vec::new();
    programas(&raiz, &mut progs);
    let progs: Vec<String> = progs.iter().map(|p| p.to_string_lossy().replace('\\', "/")).collect();

    let tmp = std::env::temp_dir();
    let lista = tmp.join("corpus_inferencia_arquivos.txt");
    let saida = tmp.join("corpus_inferencia_oraculo.tsv");
    std::fs::write(&lista, progs.join("\n") + "\n").unwrap();
    let oraculo = repo.join("tools/oraculo_tipos/oraculo.dart");
    let st = std::process::Command::new("dart")
        .arg(format!("--packages={pacotes}"))
        .arg(&oraculo)
        .arg(&raiz_txt)
        .arg(&lista)
        .arg(&saida)
        .status()
        .expect("dart");
    assert!(st.success(), "oráculo falhou: {st}");

    // Linhas do oráculo por programa (caminho em minúsculas).
    let mut por: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    for l in BufReader::new(std::fs::File::open(&saida).unwrap()).lines() {
        let l = l.unwrap();
        if l.starts_with('#') {
            panic!("oráculo falhou: {l}");
        }
        let c: Vec<String> = l.split('\t').map(String::from).collect();
        if c.len() >= 6 {
            por.entry(c[0].to_lowercase()).or_default().push(c);
        }
    }
    for prog in &progs {
        let texto = std::fs::read_to_string(prog).unwrap();
        let u16: Vec<u16> = texto.encode_utf16().collect();
        let mut linhas: Vec<(usize, isize, String)> = Vec::new();
        for c in por.get(&prog.to_lowercase()).map(Vec::as_slice).unwrap_or(&[]) {
            let (Ok(ini), Ok(comp)) = (c[1].parse::<usize>(), c[2].parse::<usize>()) else { continue };
            let (no, tipo, el) = (&c[3], &c[4], &c[5]);
            let antes = String::from_utf16_lossy(&u16[..ini.min(u16.len())]);
            let lin = antes.matches('\n').count() + 1;
            let col = antes.chars().count() - antes.rfind('\n').map(|p| antes[..=p].chars().count()).unwrap_or(0) + 1;
            let marca = u8::from(antes.trim_end_matches(' ').ends_with(MARCA));
            let fim = (ini + comp.min(60)).min(u16.len());
            let trecho = String::from_utf16_lossy(&u16[ini.min(fim)..fim]);
            let trecho = trecho.split_whitespace().collect::<Vec<_>>().join(" ");
            linhas.push((ini, -(comp as isize), format!("{lin}\t{col}\t{ini}\t{comp}\t{no}\t{tipo}\t{el}\t{marca}\t{trecho}")));
        }
        linhas.sort();
        let rel = prog.strip_prefix(&format!("{raiz_txt}/")).unwrap_or(prog);
        let destino = format!("{}.esperado.tsv", prog.trim_end_matches(".dart"));
        let mut f = std::io::BufWriter::new(std::fs::File::create(&destino).unwrap());
        writeln!(f, "# {rel} — gravado pelo oráculo (package:analyzer 6.11.0, Dart 3.6.2)").unwrap();
        writeln!(f, "# linha\tcoluna\toffset\tcomprimento\tnó\ttipo\telemento\tmarca\ttrecho").unwrap();
        for (_, _, l) in linhas {
            writeln!(f, "{l}").unwrap();
        }
    }
    println!("{} programas gravados em {raiz_txt}", progs.len());
}
