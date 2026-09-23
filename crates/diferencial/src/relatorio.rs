//! Comparação byte a byte e relatório textual.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::corpus::Programa;
use crate::processo::Saida;

/// Resultado de um programa nos três executores. `forge` é `None` com `--sem-forge`.
#[derive(Debug, Clone)]
pub struct Resultado {
    pub programa: Programa,
    pub dart: Saida,
    pub ddc: Saida,
    pub forge: Option<Saida>,
    pub nativo: bool,
}

/// Onde duas saídas divergem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Divergencia {
    /// stdout idêntico, código de saída diferente.
    Codigo,
    /// Primeira linha (1-based) do stdout que difere (ou que só existe num dos lados).
    Linha(usize),
}

/// Tira da chave de agrupamento o que muda de execucao para execucao.
///
/// Duas razoes. A primeira e determinismo: o relatorio tem de ser identico
/// com 1, 4 e 8 trabalhadores, e um identificador de processo ou de thread na
/// mensagem faz o texto mudar sozinho. A segunda e utilidade: um panico do
/// runtime traz "thread '<unnamed>' (11220) panicked at ..." e o numero
/// quebrava UM defeito em varios grupos de dois programas, escondendo o
/// tamanho real dele.
///
/// So numeros entre parenteses logo depois de `thread` e caminhos do cache do
/// runtime (que tem o hash do fonte no nome) sao normalizados; o resto da
/// linha fica como esta, porque e o que identifica o defeito.
pub fn chave_de_falha(s: &Saida) -> String {
    let mut linhas = s
        .stderr
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty());
    let Some(primeira) = linhas.next() else {
        return "(stderr vazio)".to_string();
    };
    // Um panico do Rust tem a mensagem util na SEGUNDA linha; a primeira e
    // `thread '<unnamed>' (11220) panicked at <caminho>:346:14:`, que so diz o
    // arquivo do cache do runtime. Agrupar por ela junta 89 programas num
    // grupo so e esconde qual e o defeito.
    if primeira.starts_with("thread '") && primeira.contains("panicked at") {
        if let Some(msg) = linhas.next() {
            return format!("panic no runtime: {}", estabilizar_chave(msg));
        }
    }
    estabilizar_chave(primeira)
}

pub fn estabilizar_chave(linha: &str) -> String {
    let mut saida = String::with_capacity(linha.len());
    let mut resto = linha;
    while !resto.is_empty() {
        // `runtime_cc150463840c488f.rs` -> `runtime_<hash>.rs`
        if let Some(sem) = resto.strip_prefix("runtime_") {
            let hex: usize = sem.chars().take_while(|c| c.is_ascii_hexdigit()).count();
            if hex > 0 {
                saida.push_str("runtime_<hash>");
                resto = &sem[hex..];
                continue;
            }
        }
        // `thread '<unnamed>' (11220) panicked` -> `thread '<unnamed>' (N) panicked`
        if let Some(sem) = resto.strip_prefix('(') {
            let dig: usize = sem.chars().take_while(|c| c.is_ascii_digit()).count();
            if dig > 0 && sem[dig..].starts_with(')') {
                saida.push_str("(N)");
                resto = &sem[dig + 1..];
                continue;
            }
        }
        let c = resto.chars().next().unwrap();
        saida.push(c);
        resto = &resto[c.len_utf8()..];
    }
    saida
}

/// `None` quando stdout e código de saída são idênticos.
pub fn comparar(esperado: &Saida, obtido: &Saida) -> Option<Divergencia> {
    if esperado.stdout != obtido.stdout {
        return Some(Divergencia::Linha(primeira_linha_diferente(&esperado.stdout, &obtido.stdout)));
    }
    if esperado.codigo != obtido.codigo {
        return Some(Divergencia::Codigo);
    }
    None
}

fn primeira_linha_diferente(a: &str, b: &str) -> usize {
    let mut la = a.split_inclusive('\n');
    let mut lb = b.split_inclusive('\n');
    let mut n = 1;
    loop {
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => n += 1,
            _ => return n,
        }
    }
}

impl Resultado {
    /// A saída de referência do DartForge (VM, ou DDC se o cabeçalho declara divergência).
    pub fn referencia(&self) -> &Saida {
        if !self.nativo && self.programa.referencia_e_ddc() { &self.ddc } else { &self.dart }
    }

    /// Divergência DDC × VM (só interessa quando o cabeçalho não a declara).
    pub fn ddc_vs_dart(&self) -> Option<Divergencia> {
        comparar(&self.dart, &self.ddc)
    }

    /// Divergência DartForge × referência; `None` também quando não se executou o DartForge.
    pub fn forge_vs_referencia(&self) -> Option<Divergencia> {
        self.forge.as_ref().and_then(|f| comparar(self.referencia(), f))
    }

    /// `true` quando o DartForge reproduz a referência.
    pub fn ok(&self) -> bool {
        self.forge.is_some() && self.forge_vs_referencia().is_none()
    }
}

const LARGURA_COLUNA: usize = 38;
const LINHAS_ANTES: usize = 2;
const LINHAS_DEPOIS: usize = 6;

fn truncar(s: &str, n: usize) -> String {
    let mut t: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        t.pop();
        t.push('…');
    }
    t
}

/// Três stdouts lado a lado, janela em torno da linha `foco` (1-based).
pub fn lado_a_lado(colunas: &[(&str, &str)], foco: usize) -> String {
    let linhas: Vec<Vec<&str>> = colunas.iter().map(|(_, s)| s.lines().collect()).collect();
    let max = linhas.iter().map(Vec::len).max().unwrap_or(0);
    let inicio = foco.saturating_sub(1).saturating_sub(LINHAS_ANTES);
    let fim = (foco + LINHAS_DEPOIS).min(max);
    let mut out = String::new();
    let cab: Vec<String> = colunas.iter().map(|(n, _)| format!("{:<w$}", truncar(n, LARGURA_COLUNA), w = LARGURA_COLUNA)).collect();
    let _ = writeln!(out, "         {}", cab.join(" | "));
    for i in inicio..fim {
        let marca = if i + 1 == foco { ">" } else { " " };
        let celulas: Vec<String> = linhas
            .iter()
            .map(|l| format!("{:<w$}", truncar(l.get(i).copied().unwrap_or("∅"), LARGURA_COLUNA), w = LARGURA_COLUNA))
            .collect();
        let _ = writeln!(out, "  {marca}{:>4}  {}", i + 1, celulas.join(" | "));
    }
    if fim < max {
        let _ = writeln!(out, "         … ({} linhas ao todo)", max);
    }
    out
}

/// O relatório: uma linha por programa, detalhe nas falhas, resumo e agrupamento.
pub fn relatorio(resultados: &[Resultado]) -> String {
    let mut out = String::new();
    let com_forge = resultados.iter().any(|r| r.forge.is_some());
    let nativo = resultados.first().map_or(false, |r| r.nativo);
    let mut ok = 0usize;
    let mut avisos_ddc = 0usize;
    let mut grupos: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in resultados {
        let nome = &r.programa.nome;
        if r.dart.codigo != 0 {
            let _ = writeln!(out, "DART!  {nome}  (dart run saiu com {}: {})", r.dart.codigo, truncar(r.dart.primeira_linha_stderr(), 90));
        }
        if !nativo {
            match (r.ddc_vs_dart(), &r.programa.diverge_ddc) {
                (Some(d), None) => {
                    avisos_ddc += 1;
                    let _ = writeln!(out, "DDC≠VM {nome}  ({})", descrever(d));
                    let _ = write!(out, "{}", lado_a_lado(&[("dart run", &r.dart.stdout), ("ddc+node", &r.ddc.stdout)], foco(d)));
                    let _ = writeln!(out, "       códigos: dart={} ddc={}  stderr ddc: {}", r.dart.codigo, r.ddc.codigo, truncar(r.ddc.primeira_linha_stderr(), 80));
                }
                (None, Some(motivo)) => {
                    let _ = writeln!(out, "AVISO  {nome}  cabeçalho declara divergência DDC mas as saídas batem: {motivo}");
                }
                _ => {}
            }
        }
        let Some(forge) = &r.forge else { continue };
        match r.forge_vs_referencia() {
            None => {
                ok += 1;
                let _ = writeln!(out, "ok     {nome}");
            }
            Some(d) => {
                let _ = writeln!(out, "FALHA  {nome}  ({})", descrever(d));
                let colunas: Vec<(&str, &str)> = if nativo {
                    vec![("dart run", &r.dart.stdout), ("dartforge nativo", &forge.stdout)]
                } else {
                    vec![("dart run", &r.dart.stdout), ("ddc+node", &r.ddc.stdout), ("dartforge", &forge.stdout)]
                };
                let _ = write!(out, "{}", lado_a_lado(&colunas, foco(d)));
                let codigos = if nativo {
                    format!("códigos: dart={} forge={}", r.dart.codigo, forge.codigo)
                } else {
                    format!("códigos: dart={} ddc={} forge={}", r.dart.codigo, r.ddc.codigo, forge.codigo)
                };
                let _ = writeln!(out, "       {codigos}");
                let chave = truncar(&chave_de_falha(forge), 120);
                let _ = writeln!(out, "       stderr: {chave}");
                grupos.entry(chave).or_default().push(nome.clone());
            }
        }
    }
    let total = resultados.len();
    let _ = writeln!(out);
    if com_forge {
        let label = if nativo { "DartForge Nativo" } else { "DartForge" };
        let _ = writeln!(out, "{label}: {ok}/{total} ok");
    }
    if !nativo {
        let _ = writeln!(out, "DDC×VM: {}/{total} batem (sem contar {} com divergência declarada)", total - avisos_ddc - resultados.iter().filter(|r| r.programa.diverge_ddc.is_some()).count(), resultados.iter().filter(|r| r.programa.diverge_ddc.is_some()).count());
    }
    if !grupos.is_empty() {
        let mut lista: Vec<(String, Vec<String>)> = grupos.into_iter().collect();
        lista.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        let _ = writeln!(out, "\nFalhas por primeira linha do stderr do DartForge (prioridade = tamanho do grupo):");
        for (chave, nomes) in lista {
            let _ = writeln!(out, "  {:>4}  {chave}", nomes.len());
            let _ = writeln!(out, "        {}", nomes.join(", "));
        }
    }
    out
}

fn descrever(d: Divergencia) -> String {
    match d {
        Divergencia::Codigo => "código de saída".into(),
        Divergencia::Linha(n) => format!("stdout linha {n}"),
    }
}

fn foco(d: Divergencia) -> usize {
    match d {
        Divergencia::Codigo => 1,
        Divergencia::Linha(n) => n,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(stdout: &str, codigo: i32) -> Saida {
        Saida { stdout: stdout.into(), stderr: String::new(), codigo }
    }

    #[test]
    fn compara() {
        assert_eq!(comparar(&s("a\nb\n", 0), &s("a\nb\n", 0)), None);
        assert_eq!(comparar(&s("a\nb\n", 0), &s("a\nc\n", 0)), Some(Divergencia::Linha(2)));
        assert_eq!(comparar(&s("a\nb\n", 0), &s("a\n", 0)), Some(Divergencia::Linha(2)));
        assert_eq!(comparar(&s("a\n", 0), &s("a\n", 255)), Some(Divergencia::Codigo));
        assert_eq!(comparar(&s("a\nb", 0), &s("a\nb\n", 0)), Some(Divergencia::Linha(2)));
    }

    #[test]
    fn relatorio_agrupa() {
        let p = |nome: &str| Programa { nome: nome.into(), entrada: "x.dart".into(), arquivos: vec![], diverge_ddc: None };
        let r = vec![
            Resultado { programa: p("a"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(s("1\n", 0)), nativo: false },
            Resultado { programa: p("b"), dart: s("1\n2\n", 0), ddc: s("1\n2\n", 0), forge: Some(Saida { stdout: "1\n".into(), stderr: "erro: X\n".into(), codigo: 1 }), nativo: false },
            Resultado { programa: p("c"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(Saida { stdout: String::new(), stderr: "erro: X\n".into(), codigo: 1 }), nativo: false },
        ];
        let t = relatorio(&r);
        assert!(t.contains("ok     a"), "{t}");
        assert!(t.contains("FALHA  b  (stdout linha 2)"), "{t}");
        assert!(t.contains("DartForge: 1/3 ok"), "{t}");
        assert!(t.contains("   2  erro: X"), "{t}");
        assert!(t.contains("b, c"), "{t}");
    }
}
