//! `DARTFORGE_DUPLICATES=1`: a report, on stderr, of the functions of a HIR
//! module whose bodies are the same, measured before choosing whether to merge
//! them in the HIR. Two levels:
//!
//! * exact: the same instructions, types and constants, the values and blocks
//!   renumbered, calling the same symbols;
//! * refined: the same, calling functions that are themselves equivalent — the
//!   largest such partition (refined from the shapes until stable, so mutual
//!   recursion is covered). This is what a vendored copy of a package matches:
//!   its functions call its own copies of the helpers.
//!
//! Nothing here changes the module.

use crate::hir::{Function, Module};
use std::collections::{HashMap, HashSet};

/// The body of `f` with its values and blocks renumbered in order of first
/// appearance, and the function symbols it names, in order (each replaced by
/// `@F` in the text when `abstract_callees`).
fn canonical(
    f: &Function,
    symbols: &HashSet<&str>,
    abstract_callees: bool,
) -> (String, Vec<String>) {
    let params: Vec<_> = f.params.iter().map(|(_, _, t)| *t).collect();
    let raw = format!("{params:?}->{:?}|{:?}", f.return_ty, f.blocks);
    let mut out = String::with_capacity(raw.len());
    let mut callees = Vec::new();
    let mut values: HashMap<&str, usize> = HashMap::new();
    let mut blocks: HashMap<&str, usize> = HashMap::new();
    let b = raw.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let rest = &raw[i..];
        let id = if rest.starts_with("ValueId(") {
            Some(("ValueId(", &mut values))
        } else if rest.starts_with("BlockId(") {
            Some(("BlockId(", &mut blocks))
        } else {
            None
        };
        if let Some((prefix, map)) = id {
            let start = i + prefix.len();
            let end = start + raw[start..].find(')').unwrap_or(0);
            let n = map.len();
            let k = *map.entry(&raw[start..end]).or_insert(n);
            out.push_str(prefix);
            out.push_str(&k.to_string());
            out.push(')');
            i = end + 1;
            continue;
        }
        if b[i] == b'"' {
            let end = i + 1 + raw[i + 1..].find('"').unwrap_or(0);
            let text = &raw[i + 1..end];
            if symbols.contains(text) {
                callees.push(text.to_string());
                out.push_str(if abstract_callees {
                    "@F"
                } else {
                    &raw[i..=end]
                });
            } else {
                out.push_str(&raw[i..=end]);
            }
            i = end + 1;
            continue;
        }
        out.push(b[i] as char);
        i += 1;
    }
    (out, callees)
}

/// Instructions plus one terminator per block.
fn size(f: &Function) -> usize {
    f.blocks.iter().map(|b| b.instructions.len() + 1).sum()
}

/// The classes as groups of function indices, largest saving first.
fn groups(class: &[usize], sizes: &[usize]) -> Vec<Vec<usize>> {
    let mut by: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, c) in class.iter().enumerate() {
        by.entry(*c).or_default().push(i);
    }
    let mut g: Vec<Vec<usize>> = by.into_values().filter(|v| v.len() > 1).collect();
    g.sort_by_key(|v| std::cmp::Reverse(sizes[v[0]] * (v.len() - 1)));
    g
}

fn print(label: &str, level: &str, m: &Module, g: &[Vec<usize>], sizes: &[usize]) {
    let functions: usize = g.iter().map(|v| v.len() - 1).sum();
    let saved: usize = g.iter().map(|v| sizes[v[0]] * (v.len() - 1)).sum();
    eprintln!(
        "[duplicates] {label} {level}: {} groups, {functions} removable functions, {saved} removable instructions",
        g.len()
    );
    for v in g.iter().take(8) {
        let names: Vec<&str> = v
            .iter()
            .take(3)
            .map(|&i| m.functions[i].symbol.as_str())
            .collect();
        eprintln!(
            "[duplicates]   {} x {} instructions: {}{}",
            v.len(),
            sizes[v[0]],
            names.join(", "),
            if v.len() > 3 { ", ..." } else { "" }
        );
    }
}

/// Prints the report of `m` when `DARTFORGE_DUPLICATES=1`.
pub fn report(label: &str, m: &Module) {
    if !std::env::var("DARTFORGE_DUPLICATES").is_ok_and(|v| v == "1") {
        return;
    }
    let symbols: HashSet<&str> = m.functions.iter().map(|f| f.symbol.as_str()).collect();
    let index: HashMap<&str, usize> = m
        .functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.symbol.as_str(), i))
        .collect();
    let sizes: Vec<usize> = m.functions.iter().map(size).collect();
    let total: usize = sizes.iter().sum();
    eprintln!(
        "[duplicates] {label}: {} functions, {total} instructions",
        m.functions.len()
    );
    // Exact: the text with the callee symbols as they are.
    let mut ids: HashMap<String, usize> = HashMap::new();
    let exact: Vec<usize> = m
        .functions
        .iter()
        .map(|f| {
            let (text, _) = canonical(f, &symbols, false);
            let n = ids.len();
            *ids.entry(text).or_insert(n)
        })
        .collect();
    print(label, "exact", m, &groups(&exact, &sizes), &sizes);
    // Refined: start from the shapes with the callees abstracted, then split each
    // class by the classes of its callees until the number of classes is stable.
    let shapes: Vec<(String, Vec<usize>)> = m
        .functions
        .iter()
        .map(|f| {
            let (text, callees) = canonical(f, &symbols, true);
            (text, callees.iter().map(|c| index[c.as_str()]).collect())
        })
        .collect();
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut class: Vec<usize> = shapes
        .iter()
        .map(|(text, _)| {
            let n = ids.len();
            *ids.entry(text.as_str()).or_insert(n)
        })
        .collect();
    let mut count = ids.len();
    loop {
        let mut keys: HashMap<(usize, Vec<usize>), usize> = HashMap::new();
        let next: Vec<usize> = shapes
            .iter()
            .enumerate()
            .map(|(i, (_, callees))| {
                let key = (class[i], callees.iter().map(|&c| class[c]).collect());
                let n = keys.len();
                *keys.entry(key).or_insert(n)
            })
            .collect();
        let n = keys.len();
        class = next;
        if n == count {
            break;
        }
        count = n;
    }
    print(label, "refined", m, &groups(&class, &sizes), &sizes);
}
