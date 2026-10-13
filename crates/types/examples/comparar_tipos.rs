//! Comparador em fluxo do despejo de tipos com o do oráculo
//! (`tools/oraculo_tipos/oraculo.dart`): os dois arquivos vêm ordenados por
//! (arquivo, offset, comprimento), e o comparador faz um merge arquivo a
//! arquivo, com memória só do arquivo corrente e dos contadores por grupo.
//!
//! ```text
//! cargo run --release -p dartforge-types --example comparar_tipos -- \
//!     <nosso.tsv> <oraculo.tsv> [--exemplos N] [--grupo PADRAO]
//! ```
//!
//! Uma divergência é **causa** quando a expressão diverge e nenhuma
//! divergência está estritamente contida nela (a divergência começa ali; as
//! que a contêm são cascata). As causas são agrupadas pelo nó nosso, pelo nó
//! do analyzer e pela forma da diferença.

use dartforge_types::despejo::{compare_block, forma, Bloco};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

struct Reader {
    linhas: std::io::Lines<BufReader<std::fs::File>>,
    pending: Option<Vec<String>>,
}

impl Reader {
    fn novo(path: &str) -> Reader {
        let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        Reader { linhas: BufReader::with_capacity(1 << 20, f).lines(), pending: None }
    }

    fn proxima(&mut self) -> Option<Vec<String>> {
        if let Some(p) = self.pending.take() {
            return Some(p);
        }
        for l in self.linhas.by_ref() {
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

    /// O próximo bloco (todas as linhas de um mesmo arquivo).
    fn bloco(&mut self) -> Option<(String, Bloco)> {
        let first = self.proxima()?;
        let file = first[0].clone();
        let mut b = Bloco::new();
        let insert = |c: Vec<String>, b: &mut Bloco| {
            let k = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
            b.entry(k).or_default().push((c[3].clone(), c[4].replace('*', "")));
        };
        insert(first, &mut b);
        while let Some(c) = self.proxima() {
            if c[0] != file {
                self.pending = Some(c);
                break;
            }
            insert(c, &mut b);
        }
        Some((file, b))
    }
}

struct Grupo {
    total: usize,
    examples: Vec<String>,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut nexamples = 3usize;
    let mut filter: Option<String> = None;
    if let Some(i) = args.iter().position(|a| a == "--exemplos") {
        nexamples = args[i + 1].parse().unwrap_or(3);
        args.drain(i..i + 2);
    }
    if let Some(i) = args.iter().position(|a| a == "--grupo") {
        filter = Some(args[i + 1].clone());
        args.drain(i..i + 2);
    }
    let mut ours = Reader::novo(&args[0]);
    let mut oracle = Reader::novo(&args[1]);
    let (mut compared, mut equal, mut divergent, mut ncauses) = (0usize, 0usize, 0usize, 0usize);
    let mut groups: HashMap<String, Grupo> = HashMap::new();
    let mut block_o = oracle.bloco();
    while let Some((file, b_ours)) = ours.bloco() {
        // Alinha o oráculo no mesmo arquivo (os dois na ordem da lista).
        while let Some((a, _)) = &block_o {
            if a.as_str() < file.as_str() {
                block_o = oracle.bloco();
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
        equal += c.iguais;
        divergent += c.divergences.len();
        if c.divergences.is_empty() {
            continue;
        }
        let mut source: Option<Vec<u16>> = None;
        for i in c.causas() {
            let d = &c.divergences[i];
            let (start, end) = (d.ini, d.fim);
            ncauses += 1;
            let (no, t, the_ono, ot) = (&d.no, &d.ours, &d.in_oracle, &d.oraculo);
            let key = format!("{no} [{the_ono}] {}", forma(t, ot));
            let g = groups.entry(key).or_insert(Grupo { total: 0, examples: Vec::new() });
            g.total += 1;
            if g.examples.len() < nexamples {
                let u = source.get_or_insert_with(|| {
                    std::fs::read_to_string(&file).map(|s| s.encode_utf16().collect()).unwrap_or_default()
                });
                let line = u.iter().take(start).filter(|&&c| c == u16::from(b'\n')).count() + 1;
                let end_t = end.min(start + 90).min(u.len());
                let txt = String::from_utf16_lossy(u.get(start.min(u.len())..end_t).unwrap_or(&[])).replace(['\n', '\r'], " ");
                let name = file.rsplit('/').next().unwrap_or(&file);
                g.examples.push(format!("           {name}:{line}  `{txt}`  nós={t}  oráculo={ot}"));
            }
        }
    }
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(output, "expressões comparadas: {compared}; iguais: {equal}; divergentes: {divergent}; causas: {ncauses}").unwrap();
    let mut list: Vec<(&String, &Grupo)> = groups.iter().collect();
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
