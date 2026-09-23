//! Comparação byte a byte e relatório textual.

use std::collections::BTreeMap;
use std::fmt::Write;

use dartforge_emit_native::resumo::ResumoIr;

use crate::corpus::Programa;
use crate::processo::Saida;

/// Resultado de um programa nos executores. `forge` é `None` com
/// `--sem-forge`; `producao` só existe com `--producao`.
#[derive(Debug, Clone)]
pub struct Resultado {
    pub programa: Programa,
    pub dart: Saida,
    pub ddc: Saida,
    pub forge: Option<Saida>,
    /// O `forge` é o backend nativo, e não o JS de desenvolvimento.
    pub nativo: bool,
    /// O perfil de produção do JS (`dartforge-jsprod`, arquivo único e podado).
    pub producao: Option<Saida>,
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
/// runtime traz `thread '<unnamed>' (11220) panicked at ...` e o numero
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

    /// Divergência produção × referência.
    pub fn producao_vs_referencia(&self) -> Option<Divergencia> {
        self.producao.as_ref().and_then(|f| comparar(self.referencia(), f))
    }

    /// Divergência produção × desenvolvimento. Esta é a que mais interessa ao
    /// perfil de produção: se ela aparecer, foi a poda (ou a montagem do
    /// arquivo único) que mudou o comportamento, não a emissão.
    pub fn producao_vs_forge(&self) -> Option<Divergencia> {
        match (&self.forge, &self.producao) {
            (Some(d), Some(p)) => comparar(d, p),
            _ => None,
        }
    }

    /// `true` quando o DartForge reproduz a referência — nos dois perfis que
    /// foram executados.
    pub fn ok(&self) -> bool {
        self.forge.is_some()
            && self.forge_vs_referencia().is_none()
            && self.producao.as_ref().is_none_or(|_| self.producao_vs_referencia().is_none())
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
    let mut ok_prod = 0usize;
    let com_producao = resultados.iter().any(|r| r.producao.is_some());
    let mut grupos: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut grupos_prod: BTreeMap<String, Vec<String>> = BTreeMap::new();
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
                if r.producao.is_none() {
                    let _ = writeln!(out, "ok     {nome}");
                }
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
        let Some(prod) = &r.producao else { continue };
        match r.producao_vs_referencia() {
            None => {
                ok_prod += 1;
                let _ = writeln!(out, "ok     {nome}");
            }
            Some(d) => {
                // Distingue o defeito que importa: produção que difere do
                // outro executor é defeito **da poda**; produção que difere só
                // da VM, com o outro igual, também é — mas quem agrupa é a
                // chave de falha.
                let culpa = if r.producao_vs_forge().is_some() { "produção≠executor" } else { "produção≠VM" };
                let _ = writeln!(out, "PROD!  {nome}  ({}, {culpa})", descrever(d));
                let rotulo = if nativo { "dartforge nativo" } else { "dartforge dev" };
                let _ = write!(out, "{}", lado_a_lado(&[("dart run", &r.dart.stdout), (rotulo, &forge.stdout), ("dartforge prod", &prod.stdout)], foco(d)));
                let _ = writeln!(out, "       códigos: dart={} outro={} prod={}", r.dart.codigo, forge.codigo, prod.codigo);
                // A mesma chave do executor principal, e pelo mesmo motivo: um
                // `dartforge-jsprod` que entra em pânico traz o id da thread na
                // primeira linha, que quebraria um defeito em vários grupos e
                // faria o relatório mudar de execução para execução.
                let chave = truncar(&chave_de_falha(prod), 120);
                let _ = writeln!(out, "       stderr: {chave}");
                grupos_prod.entry(chave).or_default().push(nome.clone());
            }
        }
    }
    let total = resultados.len();
    let _ = writeln!(out);
    if com_forge {
        let label = if nativo { "DartForge Nativo:          " } else { "DartForge desenvolvimento: " };
        let _ = writeln!(out, "{label}{ok}/{total} ok");
    }
    if com_producao {
        let _ = writeln!(out, "DartForge produção:        {ok_prod}/{total} ok");
    }
    if !nativo {
        let _ = writeln!(out, "DDC×VM: {}/{total} batem (sem contar {} com divergência declarada)", total - avisos_ddc - resultados.iter().filter(|r| r.programa.diverge_ddc.is_some()).count(), resultados.iter().filter(|r| r.programa.diverge_ddc.is_some()).count());
    }
    // Dois agrupamentos, porque as causas são diferentes: falha do executor
    // principal é construto que falta no emissor; falha da produção é poda ou
    // montagem do arquivo único.
    let titulo_forge = if nativo { "do DartForge Nativo" } else { "do DartForge" };
    for (titulo, grupos) in [(titulo_forge, grupos), ("do perfil de produção", grupos_prod)] {
        if grupos.is_empty() {
            continue;
        }
        let mut lista: Vec<(String, Vec<String>)> = grupos.into_iter().collect();
        lista.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        let _ = writeln!(out, "\nFalhas por primeira linha do stderr {titulo} (prioridade = tamanho do grupo):");
        for (chave, nomes) in lista {
            let _ = writeln!(out, "  {:>4}  {chave}", nomes.len());
            let _ = writeln!(out, "        {}", nomes.join(", "));
        }
    }
    out
}

/// O LLVM IR emitido para um programa, resumido (modo determinismo sem executar).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrPrograma {
    pub nome: String,
    /// O resumo do IR, ou a mensagem inteira do erro de emissão — um erro
    /// também é resultado, e também tem de ser o mesmo com qualquer número de
    /// trabalhadores.
    pub resultado: Result<ResumoIr, String>,
}

/// Uma linha por programa, na ordem do corpus: `<hash> <bytes> <nome>` ou
/// `ERRO  <nome>: <chave de falha>`, e o rodapé com a contagem. Não tem tempos
/// nem caminhos, para poder ser comparado entre execuções e entre versões do
/// emissor (um `diff` de dois relatórios diz quais programas mudaram de IR).
pub fn relatorio_ir(programas: &[IrPrograma]) -> String {
    let mut out = String::new();
    let mut com_ir = 0usize;
    for p in programas {
        match &p.resultado {
            Ok(r) => {
                com_ir += 1;
                let _ = writeln!(out, "{} {:>9} {}", r.hex(), r.bytes, p.nome);
            }
            Err(e) => {
                let chave = truncar(&chave_de_falha(&Saida { stdout: String::new(), stderr: e.clone(), codigo: 1 }), 120);
                let _ = writeln!(out, "ERRO  {}: {chave}", p.nome);
            }
        }
    }
    let _ = writeln!(out, "\n{} programas: {com_ir} com IR, {} com erro de emissão", programas.len(), programas.len() - com_ir);
    out
}

/// Todos os programas cujo resultado difere entre duas execuções (vazio =
/// idênticas). Compara o resultado inteiro, inclusive a mensagem completa do
/// erro, e não só a linha que o relatório mostra.
pub fn diferencas_ir(a: &[IrPrograma], b: &[IrPrograma]) -> Vec<String> {
    let mut out = Vec::new();
    if a.len() != b.len() {
        out.push(format!("número de programas: {} × {}", a.len(), b.len()));
    }
    for (x, y) in a.iter().zip(b) {
        if x.nome != y.nome {
            out.push(format!("ordem do corpus: {} × {}", x.nome, y.nome));
        } else if x.resultado != y.resultado {
            out.push(format!("{}: {} × {}", x.nome, descrever_ir(&x.resultado), descrever_ir(&y.resultado)));
        }
    }
    out
}

fn descrever_ir(r: &Result<ResumoIr, String>) -> String {
    match r {
        Ok(r) => format!("{} ({} bytes)", r.hex(), r.bytes),
        Err(e) => format!("erro «{}»", truncar(e.lines().next().unwrap_or(""), 80)),
    }
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
            Resultado { programa: p("a"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(s("1\n", 0)), nativo: false, producao: None },
            Resultado { programa: p("b"), dart: s("1\n2\n", 0), ddc: s("1\n2\n", 0), forge: Some(Saida { stdout: "1\n".into(), stderr: "erro: X\n".into(), codigo: 1 }), nativo: false, producao: None },
            Resultado { programa: p("c"), dart: s("1\n", 0), ddc: s("1\n", 0), forge: Some(Saida { stdout: String::new(), stderr: "erro: X\n".into(), codigo: 1 }), nativo: false, producao: None },
        ];
        let t = relatorio(&r);
        assert!(t.contains("ok     a"), "{t}");
        assert!(t.contains("FALHA  b  (stdout linha 2)"), "{t}");
        assert!(t.contains("DartForge desenvolvimento: 1/3 ok"), "{t}");
        assert!(t.contains("   2  erro: X"), "{t}");
        assert!(t.contains("b, c"), "{t}");
    }

    fn ir(nome: &str, resultado: Result<&str, &str>) -> IrPrograma {
        IrPrograma { nome: nome.into(), resultado: resultado.map(ResumoIr::de).map_err(str::to_string) }
    }

    #[test]
    fn relatorio_ir_formato() {
        let r = vec![
            ir("01_a", Ok("define void @dart_main()")),
            ir("02_b", Err("[emitir-ir] a thread abortou: índice fora de faixa\ndetalhe")),
            ir("10_c", Ok("")),
        ];
        let t = relatorio_ir(&r);
        let linhas: Vec<&str> = t.lines().collect();
        let a = ResumoIr::de("define void @dart_main()");
        assert_eq!(linhas[0], format!("{}        24 01_a", a.hex()));
        assert_eq!(linhas[1], "ERRO  02_b: [emitir-ir] a thread abortou: índice fora de faixa");
        assert_eq!(linhas[2], "6c62272e07bb014262b821756295c58d         0 10_c");
        assert_eq!(linhas[4], "3 programas: 2 com IR, 1 com erro de emissão");
    }

    #[test]
    fn diferencas_ir_lista_todas() {
        let a = vec![ir("a", Ok("x")), ir("b", Ok("y")), ir("c", Err("e\n1")), ir("d", Ok("z"))];
        assert!(diferencas_ir(&a, &a.clone()).is_empty());
        let b = vec![ir("a", Ok("x")), ir("b", Ok("Y")), ir("c", Err("e\n2")), ir("d", Ok("Z"))];
        let d = diferencas_ir(&a, &b);
        assert_eq!(d.len(), 3, "{d:?}");
        assert!(d[0].starts_with("b: ") && d[1].starts_with("c: ") && d[2].starts_with("d: "), "{d:?}");
        assert_eq!(diferencas_ir(&a, &b[..3]).len(), 3);
    }
}
