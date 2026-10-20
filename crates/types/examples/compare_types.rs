//! Streaming comparator of our type dump against the oracle's
//! (`tools/type_oracle/oracle.dart`): both files come sorted by
//! (file, offset, length), and the comparator does a file-by-file merge,
//! keeping in memory only the current file and the per-group counters.
//!
//! ```text
//! cargo run --release -p dartforge-types --example compare_types -- \
//!     <ours.tsv> <oracle.tsv> [--examples N] [--group PATTERN]
//! ```
//!
//! A divergence is a **cause** when the expression diverges and no divergence
//! is strictly contained in it (the divergence starts there; the ones that
//! contain it are cascade). Causes are grouped by our node, by the analyzer's
//! node and by the shape of the difference.

use dartforge_types::dump::{Block, compare_block, shape};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

struct Reader {
    lines: std::io::Lines<BufReader<std::fs::File>>,
    pending: Option<Vec<String>>,
}

impl Reader {
    fn new_(path: &str) -> Reader {
        let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        Reader {
            lines: BufReader::with_capacity(1 << 20, f).lines(),
            pending: None,
        }
    }

    fn next_(&mut self) -> Option<Vec<String>> {
        if let Some(p) = self.pending.take() {
            return Some(p);
        }
        for l in self.lines.by_ref() {
            let l = l.ok()?;
            if l.starts_with('#') {
                continue;
            }
            let fields: Vec<String> = l.split('\t').map(String::from).collect();
            if fields.len() >= 6 {
                return Some(fields);
            }
        }
        None
    }

    /// The next block (all lines of the same file).
    fn block(&mut self) -> Option<(String, Block)> {
        let first = self.next_()?;
        let file = first[0].clone();
        let mut b = Block::new();
        let insert = |c: Vec<String>, b: &mut Block| {
            let k = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
            b.entry(k)
                .or_default()
                .push((c[3].clone(), c[4].replace('*', "")));
        };
        insert(first, &mut b);
        while let Some(c) = self.next_() {
            if c[0] != file {
                self.pending = Some(c);
                break;
            }
            insert(c, &mut b);
        }
        Some((file, b))
    }
}

struct Group {
    total: usize,
    examples: Vec<String>,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut nexamples = 3usize;
    let mut filter: Option<String> = None;
    if let Some(i) = args.iter().position(|a| a == "--examples") {
        nexamples = args[i + 1].parse().unwrap_or(3);
        args.drain(i..i + 2);
    }
    if let Some(i) = args.iter().position(|a| a == "--group") {
        filter = Some(args[i + 1].clone());
        args.drain(i..i + 2);
    }
    let mut ours = Reader::new_(&args[0]);
    let mut oracle = Reader::new_(&args[1]);
    let (mut compared, mut equal, mut divergent, mut ncauses) = (0usize, 0usize, 0usize, 0usize);
    let mut groups: HashMap<String, Group> = HashMap::new();
    let mut block_o = oracle.block();
    while let Some((file, b_ours)) = ours.block() {
        // Aligns the oracle on the same file (both in list order).
        while let Some((a, _)) = &block_o {
            if a.as_str() < file.as_str() {
                block_o = oracle.block();
            } else {
                break;
            }
        }
        let b_oracle = match &block_o {
            Some((a, b)) if *a == file => b,
            _ => continue,
        };
        let c = compare_block(&b_ours, b_oracle);
        compared += c.compared;
        equal += c.equal;
        divergent += c.divergences.len();
        if c.divergences.is_empty() {
            continue;
        }
        let mut source: Option<Vec<u16>> = None;
        for i in c.causes() {
            let d = &c.divergences[i];
            let (start, end) = (d.start, d.end);
            ncauses += 1;
            let (no, t, the_ono, ot) = (&d.no, &d.ours, &d.in_oracle, &d.oracle);
            let key = format!("{no} [{the_ono}] {}", shape(t, ot));
            let g = groups.entry(key).or_insert(Group {
                total: 0,
                examples: Vec::new(),
            });
            g.total += 1;
            if g.examples.len() < nexamples {
                let u = source.get_or_insert_with(|| {
                    std::fs::read_to_string(&file)
                        .map(|s| s.encode_utf16().collect())
                        .unwrap_or_default()
                });
                let line = u
                    .iter()
                    .take(start)
                    .filter(|&&c| c == u16::from(b'\n'))
                    .count()
                    + 1;
                let end_t = end.min(start + 90).min(u.len());
                let txt = String::from_utf16_lossy(u.get(start.min(u.len())..end_t).unwrap_or(&[]))
                    .replace(['\n', '\r'], " ");
                let name = file.rsplit('/').next().unwrap_or(&file);
                g.examples.push(format!(
                    "           {name}:{line}  `{txt}`  ours={t}  oracle={ot}"
                ));
            }
        }
    }
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(output, "compared expressions: {compared}; equal: {equal}; divergent: {divergent}; causes: {ncauses}").unwrap();
    let mut list: Vec<(&String, &Group)> = groups.iter().collect();
    list.sort_by(|a, b| b.1.total.cmp(&a.1.total).then(a.0.cmp(b.0)));
    for (name, g) in list {
        if filter.as_deref().is_some_and(|f| !name.contains(f)) {
            continue;
        }
        writeln!(output, "{:7}  {name}", g.total).unwrap();
        for e in &g.examples {
            writeln!(output, "{e}").unwrap();
        }
    }
}
