//! Conformidade da inferência com o oráculo (`package:analyzer`): despeja
//! cada programa de `corpus/inferencia/` e compara com o `.esperado.tsv`
//! gravado pelo oráculo (docs/INFERENCIA-ESPECIFICACAO.md, §0).
//!
//! As divergências e os avisos conhecidos ficam em
//! `corpus/inferencia/divergencias.txt` (catraca): o teste falha com
//! divergência ou aviso novo, e também com item da lista que deixou de
//! acontecer (a lista só encolhe). `ATUALIZAR_DIVERGENCIAS=1` regrava a lista,
//! preservando as notas do cabeçalho (linhas `#> `), que explicam as
//! divergências mantidas de propósito.

use dartforge_elements::sdk::SdkLayout;
use dartforge_types::despejo::{expected_block, line_block, compare_block, dump};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn sdk() -> Option<SdkLayout> {
    let dir = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        p.join("libraries.json").exists().then_some(p)
    })?;
    SdkLayout::load(&dir, "dartdevc").ok()
}

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

#[test]
fn inference_corpus_against_oracle() {
    let Some(sdk) = sdk() else {
        eprintln!("SDK do Dart ausente: teste pulado");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/inferencia");
    let mut progs = Vec::new();
    programs(&root, &mut progs);
    assert!(!progs.is_empty(), "corpus vazio em {}", root.display());
    let list_path = root.join("divergencias.txt");

    let (current, summary) = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let mut current: BTreeSet<String> = BTreeSet::new();
            let (mut compared, mut divergent, mut warnings, mut equal) = (0usize, 0usize, 0usize, 0usize);
            for prog in &progs {
                let rel = prog.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                let expected = prog.with_extension("esperado.tsv");
                let Ok(text) = std::fs::read_to_string(&expected) else {
                    panic!("{rel}: falta {}", expected.display());
                };
                let d = dump(prog, &sdk, None);
                let path = std::fs::canonicalize(prog).unwrap_or(prog.clone());
                let target = path.to_string_lossy().replace('\\', "/");
                let target = target.trim_start_matches("//?/");
                let Some(u) = d.unidades.iter().find(|u| u.caminho.eq_ignore_ascii_case(target)).or(d.unidades.first()) else {
                    panic!("{rel}: unidade não despejada");
                };
                let c = compare_block(&line_block(&u.linhas), &expected_block(&text));
                compared += c.compared;
                divergent += c.divergences.len();
                for x in &c.divergences {
                    current.insert(format!("{rel}:{}:{}\t{}\tnós={}\toráculo={}", x.ini, x.fim - x.ini, x.no, x.ours, x.oraculo));
                }
                for a in &d.avisos {
                    warnings += 1;
                    current.insert(format!("{rel}:aviso:{}\t{}", a.span.start, a.message.replace(['\n', '\t'], " ")));
                }
                if c.divergences.is_empty() && d.avisos.is_empty() {
                    equal += 1;
                }
            }
            let summary = format!(
                "programas iguais ao oráculo: {equal}/{}; expressões comparadas: {compared}; divergentes: {divergent}; avisos: {warnings}",
                progs.len()
            );
            (current, summary)
        })
        .unwrap()
        .join()
        .unwrap();
    eprintln!("{summary}");

    if std::env::var_os("ATUALIZAR_DIVERGENCIAS").is_some() {
        let mut s = String::from(
            "# Divergências e avisos conhecidos do corpus/inferencia contra o oráculo\n\
             # (catraca de crates/types/tests/corpus_inferencia.rs; só encolhe).\n",
        );
        s.push_str(&format!("# {summary}\n"));
        // Notas (`#> `) sobre divergências mantidas de propósito: preservadas.
        for l in std::fs::read_to_string(&list_path).unwrap_or_default().lines().filter(|l| l.starts_with("#>")) {
            s.push_str(l);
            s.push('\n');
        }
        for l in &current {
            s.push_str(l);
            s.push('\n');
        }
        std::fs::write(&list_path, s).unwrap();
        return;
    }
    let known: BTreeSet<String> = std::fs::read_to_string(&list_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(String::from)
        .collect();
    let new_: Vec<&String> = current.difference(&known).collect();
    let resolved: Vec<&String> = known.difference(&current).collect();
    assert!(
        new_.is_empty() && resolved.is_empty(),
        "corpus/inferencia mudou ({summary}).\nnovas ({}):\n{}\nresolvidas, tirar da lista ({}):\n{}\n\
         (ATUALIZAR_DIVERGENCIAS=1 regrava corpus/inferencia/divergencias.txt)",
        new_.len(),
        new_.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
        resolved.len(),
        resolved.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
    );
}
