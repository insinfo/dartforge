//! Conformance of inference with the oracle (`package:analyzer`): dumps each
//! program of `corpus/inferencia/` and compares it with the `.expected.tsv`
//! recorded by the oracle (docs/INFERENCIA-ESPECIFICACAO.md, §0).
//!
//! The known divergences and warnings are kept in
//! `corpus/inferencia/divergences.txt` (a ratchet): the test fails on a new
//! divergence or warning, and also on a list item that stopped happening
//! (the list only shrinks). `UPDATE_DIVERGENCES=1` rewrites the list,
//! preserving the header notes (`#> ` lines), which explain the divergences
//! kept on purpose.

use dartforge_elements::sdk::SdkLayout;
use dartforge_types::dump::{expected_block, line_block, compare_block, dump};
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
        eprintln!("Dart SDK missing: test skipped");
        return;
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/inferencia");
    let mut progs = Vec::new();
    programs(&root, &mut progs);
    assert!(!progs.is_empty(), "empty corpus in {}", root.display());
    let list_path = root.join("divergences.txt");

    let (current, summary) = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let mut current: BTreeSet<String> = BTreeSet::new();
            let (mut compared, mut divergent, mut warnings, mut equal) = (0usize, 0usize, 0usize, 0usize);
            for prog in &progs {
                let rel = prog.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                let expected = prog.with_extension("expected.tsv");
                let Ok(text) = std::fs::read_to_string(&expected) else {
                    panic!("{rel}: falta {}", expected.display());
                };
                let d = dump(prog, &sdk, None);
                let path = std::fs::canonicalize(prog).unwrap_or(prog.clone());
                let target = path.to_string_lossy().replace('\\', "/");
                let target = target.trim_start_matches("//?/");
                let Some(u) = d.units.iter().find(|u| u.path.eq_ignore_ascii_case(target)).or(d.units.first()) else {
                    panic!("{rel}: unit not dumped");
                };
                let c = compare_block(&line_block(&u.lines), &expected_block(&text));
                compared += c.compared;
                divergent += c.divergences.len();
                for x in &c.divergences {
                    current.insert(format!("{rel}:{}:{}\t{}\tnós={}\toráculo={}", x.start, x.end - x.start, x.no, x.ours, x.oracle));
                }
                for a in &d.warnings {
                    warnings += 1;
                    current.insert(format!("{rel}:aviso:{}\t{}", a.span.start, a.message.replace(['\n', '\t'], " ")));
                }
                if c.divergences.is_empty() && d.warnings.is_empty() {
                    equal += 1;
                }
            }
            let summary = format!(
                "programs equal to the oracle: {equal}/{}; expressions compared: {compared}; divergent: {divergent}; warnings: {warnings}",
                progs.len()
            );
            (current, summary)
        })
        .unwrap()
        .join()
        .unwrap();
    eprintln!("{summary}");

    if std::env::var_os("UPDATE_DIVERGENCES").is_some() {
        let mut s = String::from(
            "# Known divergences and warnings of corpus/inferencia against the oracle\n\
             # (ratchet of crates/types/tests/corpus_inference.rs; it only shrinks).\n",
        );
        s.push_str(&format!("# {summary}\n"));
        // Notes (`#> `) about divergences kept on purpose: preserved.
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
        "corpus/inferencia changed ({summary}).\nnew ({}):\n{}\nresolved, remove from the list ({}):\n{}\n\
         (UPDATE_DIVERGENCES=1 rewrites corpus/inferencia/divergences.txt)",
        new_.len(),
        new_.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
        resolved.len(),
        resolved.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
    );
}
