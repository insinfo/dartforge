//! Paridade de diagnósticos com o `dart analyze` oficial (plano A1/A2).
//!
//! * [`json`]: o JSON v1 do `dart analyze --format=json`, lido e emitido.
//! * [`analise`]: o nosso lado — um lote de arquivos vira um programa,
//!   diagnósticos por arquivo, com código.
//! * [`ponte`]: códigos para os diagnósticos que ainda saem só com texto.
//! * [`filtros`]: `analysis_options.yaml` e `// ignore:`.
//! * [`oraculo`]: rodar e gravar o oráculo (SDK 3.6.2 e 3.13.4).
//! * [`corpus`]: os grupos de `corpus/diagnosticos/`.
//! * [`placar`]: a comparação por código.
//!
//! Regra de publicação (plano §2.3): sintaxe é publicada sempre; um código
//! semântico só é publicado se estiver em `verificados.txt` (100% no corpus e
//! 0 falso positivo nos projetos reais). O resto existe internamente e vai só
//! para o placar.

pub mod analise;
pub mod corpus;
pub mod filtros;
pub mod json;
pub mod oraculo;
pub mod placar;
pub mod ponte;
pub mod projetos;

use analise::{Analise, Motor};
use dartforge_diagnostics::{Diagnostic, TipoErro};
use oraculo::Registro;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A lista versionada de códigos semânticos verificados.
pub const VERIFICADOS: &str = include_str!("../verificados.txt");

/// Os códigos de [`VERIFICADOS`] (sem comentários).
pub fn verificados() -> Vec<&'static str> {
    VERIFICADOS
        .lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .collect()
}

/// O diagnóstico vai para o editor/CLI? `sintaxe`: veio do lexer/parser.
pub fn publicado(d: &Diagnostic, sintaxe: bool) -> bool {
    if sintaxe {
        return true;
    }
    match d.code {
        Some(c) if c.info().tipo == TipoErro::SyntacticError => true,
        Some(c) => verificados().contains(&c.info().nome),
        None => false,
    }
}

/// Os diagnósticos da análise no JSON v1, com os filtros aplicados
/// (`analysis_options.yaml` da raiz e `// ignore:`). `so_publicados` aplica a
/// regra de publicação.
pub fn diagnosticos_json(analise: &Analise, raiz: &Path, opcoes: &filtros::Opcoes, so_publicados: bool) -> Vec<json::DiagJson> {
    let mut out = Vec::new();
    for (p, a) in &analise.arquivos {
        if oraculo::relativo(p, raiz).is_some_and(|rel| opcoes.excluido(&rel)) {
            continue;
        }
        let linhas = json::Linhas::new(&a.texto);
        let ignorados = filtros::Ignorados::de_texto(&a.texto);
        let caminho = p.to_string_lossy();
        for (i, d) in a.diags.iter().enumerate() {
            let sintaxe = i < a.sintaticos;
            if so_publicados && !publicado(d, sintaxe) {
                continue;
            }
            let Some(d) = opcoes.processar(d.clone()) else { continue };
            let linha = linhas.ponto(d.span.start).line;
            if ignorados.ignora(&d, linha) {
                continue;
            }
            out.push(json::para_json(&caminho, &linhas, &d, sintaxe));
        }
    }
    json::ordenar(&mut out);
    out
}

/// Como [`diagnosticos_json`], em registros relativos a `raiz`.
pub fn registros(analise: &Analise, raiz: &Path, opcoes: &filtros::Opcoes, so_publicados: bool) -> Vec<Registro> {
    let mut out: Vec<Registro> = diagnosticos_json(analise, raiz, opcoes, so_publicados)
        .iter()
        .filter_map(|j| Registro::de_json(j, raiz))
        .collect();
    out.sort();
    out
}

/// Resultado do nosso lado para um conjunto de arquivos.
#[derive(Debug, Default)]
pub struct Rodada {
    pub registros: Vec<Registro>,
    pub arquivos: usize,
    pub lotes: usize,
    /// Arquivos cuja análise entrou em pânico, estourou o teto de memória ou
    /// o tempo (isolados um a um).
    pub panicos: Vec<String>,
    pub ambiguos: usize,
}

/// Como os lotes rodam.
#[derive(Debug, Clone)]
pub struct Execucao {
    pub trabalhadores: usize,
    pub tamanho_lote: usize,
    pub progresso: bool,
    /// `Some(exe)`: cada lote num processo filho (`exe _lote ...`), para que
    /// estouro de pilha, falta de memória ou laço infinito numa análise não
    /// derrubem o placar. `None`: na mesma thread (só pânicos são contidos).
    pub isolar: Option<PathBuf>,
    /// Tempo máximo de um lote isolado.
    pub tempo_max: std::time::Duration,
}

/// Analisa `arquivos` de `raiz` em lotes. Os lotes são definidos pela ordem
/// dos arquivos, não pelos trabalhadores: o resultado é o mesmo com 1, 4 ou 8.
pub fn rodar_nosso(
    motor: &Motor,
    raiz: &Path,
    arquivos: &[PathBuf],
    packages: Option<&Path>,
    opcoes: &filtros::Opcoes,
    ex: &Execucao,
) -> Rodada {
    let lotes: Vec<&[PathBuf]> = arquivos.chunks(ex.tamanho_lote.max(1)).collect();
    type Parcial = (Vec<Registro>, Vec<String>, usize);
    let resultados: Mutex<Vec<Option<Parcial>>> = Mutex::new((0..lotes.len()).map(|_| None).collect());
    let proximo = AtomicUsize::new(0);
    let feitos = AtomicUsize::new(0);
    let trabalhar = || {
        loop {
            let i = proximo.fetch_add(1, Ordering::SeqCst);
            if i >= lotes.len() {
                break;
            }
            let r = match &ex.isolar {
                Some(exe) => lote_isolado(exe, raiz, lotes[i], packages, ex.tempo_max),
                None => analisar_lote(motor, raiz, lotes[i], packages),
            };
            resultados.lock().expect("resultados")[i] = Some(r);
            let n = feitos.fetch_add(1, Ordering::SeqCst) + 1;
            if ex.progresso && (n % 20 == 0 || n == lotes.len()) {
                eprintln!("  {n}/{} lotes", lotes.len());
            }
        }
    };
    std::thread::scope(|s| {
        let mut hs = Vec::new();
        for _ in 0..ex.trabalhadores.max(1) {
            // Corpos profundos recursam fundo: pilha própria, como o `compile-js`.
            hs.push(std::thread::Builder::new().stack_size(1 << 30).spawn_scoped(s, trabalhar).expect("thread"));
        }
        for h in hs {
            let _ = h.join();
        }
    });
    let mut rodada = Rodada { arquivos: arquivos.len(), lotes: lotes.len(), ..Rodada::default() };
    for r in resultados.into_inner().expect("resultados").into_iter().flatten() {
        rodada.registros.extend(r.0);
        rodada.panicos.extend(r.1);
        rodada.ambiguos += r.2;
    }
    // `analyzer: exclude/errors` do `analysis_options.yaml`, aplicados aqui
    // (os comentários de ignore já vieram aplicados de cada lote).
    rodada.registros = rodada
        .registros
        .into_iter()
        .filter(|r| !opcoes.excluido(&r.arquivo))
        .filter_map(|mut r| match opcoes.errors.get(&r.code) {
            None => Some(r),
            Some(None) => None,
            Some(Some(s)) => {
                r.severity = s.nome().to_string();
                Some(r)
            }
        })
        .collect();
    rodada.registros.sort();
    rodada.panicos.sort();
    rodada
}

/// Um lote na mesma thread; em pânico, cada arquivo sozinho.
fn analisar_lote(motor: &Motor, raiz: &Path, lote: &[PathBuf], packages: Option<&Path>) -> (Vec<Registro>, Vec<String>, usize) {
    let tentar = |arqs: &[PathBuf]| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let a = motor.analisar(raiz, arqs, packages);
            (registros(&a, raiz, &filtros::Opcoes::default(), false), a.ambiguos)
        }))
    };
    if let Ok((r, amb)) = tentar(lote) {
        return (r, Vec::new(), amb);
    }
    let mut regs = Vec::new();
    let mut panicos = Vec::new();
    let mut amb = 0;
    for a in lote {
        match tentar(std::slice::from_ref(a)) {
            Ok((r, x)) => {
                regs.extend(r);
                amb += x;
            }
            Err(_) => panicos.push(oraculo::relativo(a, raiz).unwrap_or_else(|| a.display().to_string())),
        }
    }
    (regs, panicos, amb)
}

/// Um lote num processo filho; se ele morrer, cada arquivo num filho.
fn lote_isolado(
    exe: &Path,
    raiz: &Path,
    lote: &[PathBuf],
    packages: Option<&Path>,
    tempo_max: std::time::Duration,
) -> (Vec<Registro>, Vec<String>, usize) {
    if let Some((r, amb)) = filho(exe, raiz, lote, packages, tempo_max) {
        return (r, Vec::new(), amb);
    }
    let mut regs = Vec::new();
    let mut panicos = Vec::new();
    let mut amb = 0;
    for a in lote {
        match filho(exe, raiz, std::slice::from_ref(a), packages, tempo_max) {
            Some((r, x)) => {
                regs.extend(r);
                amb += x;
            }
            None => panicos.push(oraculo::relativo(a, raiz).unwrap_or_else(|| a.display().to_string())),
        }
    }
    (regs, panicos, amb)
}

/// O executável com o subcomando `_lote`, os arquivos na entrada padrão.
/// `None` se o filho não terminar bem dentro do tempo.
fn filho(
    exe: &Path,
    raiz: &Path,
    lote: &[PathBuf],
    packages: Option<&Path>,
    tempo_max: std::time::Duration,
) -> Option<(Vec<Registro>, usize)> {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let pk = packages.map(|p| p.as_os_str().to_owned()).unwrap_or_else(|| "-".into());
    let mut c = Command::new(exe)
        .arg("_lote")
        .arg(raiz)
        .arg(pk)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    {
        let mut entrada = c.stdin.take()?;
        let lista: String = lote.iter().map(|a| format!("{}\n", a.display())).collect();
        entrada.write_all(lista.as_bytes()).ok()?;
    }
    // A saída é lida numa thread para o filho não travar com o cano cheio.
    let mut saida = c.stdout.take()?;
    let leitor = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = saida.read_to_string(&mut s);
        s
    });
    let inicio = std::time::Instant::now();
    let status = loop {
        match c.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if inicio.elapsed() > tempo_max => {
                let _ = c.kill();
                let _ = c.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(_) => break None,
        }
    };
    let texto = leitor.join().ok()?;
    if !status?.success() {
        return None;
    }
    let mut regs = Vec::new();
    let mut amb = None;
    for l in texto.lines() {
        if let Some(n) = l.strip_prefix("#ambiguos ") {
            amb = n.trim().parse().ok();
        } else if !l.is_empty() {
            regs.push(serde_json::from_str(l).ok()?);
        }
    }
    Some((regs, amb?))
}

/// O lado filho de [`filho`]: analisa e escreve os registros (com os
/// comentários de ignore; as opções o pai aplica) e `#ambiguos N`. Um vigia
/// encerra o processo se a memória viva (`vivos`) passar de `teto` bytes.
pub fn executar_lote_filho(
    raiz: &Path,
    packages: Option<&Path>,
    arquivos: &[PathBuf],
    vivos: fn() -> usize,
    teto: usize,
) -> i32 {
    std::thread::spawn(move || {
        loop {
            if vivos() > teto {
                std::process::exit(98);
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    });
    let motor = match Motor::descobrir() {
        Ok(m) => m,
        Err(_) => return 2,
    };
    let (r2, p2, a2) = (raiz.to_path_buf(), packages.map(Path::to_path_buf), arquivos.to_vec());
    let feito = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let a = motor.analisar(&r2, &a2, p2.as_deref());
            (registros(&a, &r2, &filtros::Opcoes::default(), false), a.ambiguos)
        })
        .expect("thread")
        .join();
    match feito {
        Ok((regs, amb)) => {
            let mut s = String::new();
            for r in regs {
                s.push_str(&serde_json::to_string(&r).expect("JSON"));
                s.push('\n');
            }
            s.push_str(&format!("#ambiguos {amb}\n"));
            use std::io::Write;
            let _ = std::io::stdout().write_all(s.as_bytes());
            0
        }
        Err(_) => 3,
    }
}

/// Silencia a mensagem de pânico padrão (os pânicos são contados no relatório).
pub fn silenciar_panicos() {
    std::panic::set_hook(Box::new(|_| {}));
}