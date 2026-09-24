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

use dartforge_types::despejo::{comparar_bloco, forma, Bloco};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

struct Leitor {
    linhas: std::io::Lines<BufReader<std::fs::File>>,
    pendente: Option<Vec<String>>,
}

impl Leitor {
    fn novo(caminho: &str) -> Leitor {
        let f = std::fs::File::open(caminho).unwrap_or_else(|e| panic!("{caminho}: {e}"));
        Leitor { linhas: BufReader::with_capacity(1 << 20, f).lines(), pendente: None }
    }

    fn proxima(&mut self) -> Option<Vec<String>> {
        if let Some(p) = self.pendente.take() {
            return Some(p);
        }
        for l in self.linhas.by_ref() {
            let l = l.ok()?;
            if l.starts_with('#') {
                continue;
            }
            let campos: Vec<String> = l.split('\t').map(String::from).collect();
            if campos.len() >= 6 {
                return Some(campos);
            }
        }
        None
    }

    /// O próximo bloco (todas as linhas de um mesmo arquivo).
    fn bloco(&mut self) -> Option<(String, Bloco)> {
        let primeira = self.proxima()?;
        let arquivo = primeira[0].clone();
        let mut b = Bloco::new();
        let inserir = |c: Vec<String>, b: &mut Bloco| {
            let k = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
            b.entry(k).or_default().push((c[3].clone(), c[4].replace('*', "")));
        };
        inserir(primeira, &mut b);
        while let Some(c) = self.proxima() {
            if c[0] != arquivo {
                self.pendente = Some(c);
                break;
            }
            inserir(c, &mut b);
        }
        Some((arquivo, b))
    }
}

struct Grupo {
    total: usize,
    exemplos: Vec<String>,
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut nexemplos = 3usize;
    let mut filtro: Option<String> = None;
    if let Some(i) = args.iter().position(|a| a == "--exemplos") {
        nexemplos = args[i + 1].parse().unwrap_or(3);
        args.drain(i..i + 2);
    }
    if let Some(i) = args.iter().position(|a| a == "--grupo") {
        filtro = Some(args[i + 1].clone());
        args.drain(i..i + 2);
    }
    let mut nosso = Leitor::novo(&args[0]);
    let mut oraculo = Leitor::novo(&args[1]);
    let (mut comparadas, mut iguais, mut divergentes, mut ncausas) = (0usize, 0usize, 0usize, 0usize);
    let mut grupos: HashMap<String, Grupo> = HashMap::new();
    let mut bloco_o = oraculo.bloco();
    while let Some((arquivo, b_nosso)) = nosso.bloco() {
        // Alinha o oráculo no mesmo arquivo (os dois na ordem da lista).
        while let Some((a, _)) = &bloco_o {
            if a.as_str() < arquivo.as_str() {
                bloco_o = oraculo.bloco();
            } else {
                break;
            }
        }
        let b_oraculo = match &bloco_o {
            Some((a, b)) if *a == arquivo => b,
            _ => continue,
        };
        let c = comparar_bloco(&b_nosso, b_oraculo);
        comparadas += c.comparadas;
        iguais += c.iguais;
        divergentes += c.divergencias.len();
        if c.divergencias.is_empty() {
            continue;
        }
        let mut fonte: Option<Vec<u16>> = None;
        for i in c.causas() {
            let d = &c.divergencias[i];
            let (ini, fim) = (d.ini, d.fim);
            ncausas += 1;
            let (no, t, ono, ot) = (&d.no, &d.nosso, &d.no_oraculo, &d.oraculo);
            let chave = format!("{no} [{ono}] {}", forma(t, ot));
            let g = grupos.entry(chave).or_insert(Grupo { total: 0, exemplos: Vec::new() });
            g.total += 1;
            if g.exemplos.len() < nexemplos {
                let u = fonte.get_or_insert_with(|| {
                    std::fs::read_to_string(&arquivo).map(|s| s.encode_utf16().collect()).unwrap_or_default()
                });
                let linha = u.iter().take(ini).filter(|&&c| c == u16::from(b'\n')).count() + 1;
                let fim_t = fim.min(ini + 90).min(u.len());
                let txt = String::from_utf16_lossy(u.get(ini.min(u.len())..fim_t).unwrap_or(&[])).replace(['\n', '\r'], " ");
                let nome = arquivo.rsplit('/').next().unwrap_or(&arquivo);
                g.exemplos.push(format!("           {nome}:{linha}  `{txt}`  nós={t}  oráculo={ot}"));
            }
        }
    }
    let mut saida = std::io::BufWriter::new(std::io::stdout().lock());
    writeln!(saida, "expressões comparadas: {comparadas}; iguais: {iguais}; divergentes: {divergentes}; causas: {ncausas}").unwrap();
    let mut lista: Vec<(&String, &Grupo)> = grupos.iter().collect();
    lista.sort_by(|a, b| b.1.total.cmp(&a.1.total).then(a.0.cmp(b.0)));
    for (nome, g) in lista {
        if filtro.as_deref().is_some_and(|f| !nome.contains(f)) {
            continue;
        }
        writeln!(saida, "{:7}  {nome}", g.total).unwrap();
        for e in &g.exemplos {
            writeln!(saida, "{e}").unwrap();
        }
    }
}
