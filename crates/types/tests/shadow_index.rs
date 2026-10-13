//! The scope index behind `annotations::is_shadowed` answers like the walk
//! over the whole tree it replaced (`is_shadowed_walk`), for every identifier
//! of the corpus files (the identifier's own name, at its position and just
//! after it).

use dartforge_frontend::ast::{ExprKind, TypeKind};
use dartforge_types::annotations::{ShadowIndex, is_shadowed_walk};
use std::path::{Path, PathBuf};

fn dart_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            dart_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "dart") {
            out.push(p);
        }
    }
}

#[test]
fn index_answers_like_the_walk() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut files = Vec::new();
    dart_files(&corpus, &mut files);
    // Small files (the walk is quadratic), a spread sample of them.
    files.retain(|p| std::fs::metadata(p).is_ok_and(|m| m.len() < 40_000));
    let step = (files.len() / 4000).max(1);
    let mut checked = 0usize;
    for path in files.iter().step_by(step) {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let mut interner = dartforge_intern::Interner::new();
        let parsed = dartforge_frontend::parser::parse(&text, &mut interner);
        let a = &parsed.ast;
        let index = ShadowIndex::build(a);
        let mut probes: Vec<(usize, dartforge_intern::SymbolId)> = Vec::new();
        for e in a.exprs.iter() {
            if let ExprKind::Identifier(n) = &e.kind {
                probes.push((n.span.start, n.sym));
                probes.push((n.span.end, n.sym));
            }
        }
        for t in a.types.iter() {
            if let TypeKind::Named { name, .. } = &t.kind
                && let Some(n) = name.first()
            {
                probes.push((n.span.start, n.sym));
            }
        }
        for (pos, n) in probes {
            let walk = is_shadowed_walk(a, &interner, pos, n);
            let fast = index.shadows(&interner, pos, n);
            assert_eq!(walk, fast, "{}: `{}` at {pos}", path.display(), interner.resolve(n));
            checked += 1;
        }
    }
    assert!(checked > 1000, "too few probes: {checked}");
}
