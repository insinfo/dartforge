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
    /// Arquivos cuja análise entrou em pânico (isolados um a um).
    pub panicos: Vec<String>,
    pub ambiguos: usize,
}

/// Analisa `arquivos` de `raiz` em lotes de `tamanho_lote`, com
/// `trabalhadores` threads. Os lotes são definidos pela ordem dos arquivos,
/// não pelos trabalhadores: o resultado é o mesmo com 1, 4 ou 8.
pub fn rodar_nosso(
    motor: &Motor,
    raiz: &Path,
    arquivos: &[PathBuf],
    packages: Option<&Path>,
    opcoes: &filtros::Opcoes,
    trabalhadores: usize,
    tamanho_lote: usize,
    progresso: bool,
) -> Rodada {
    let lotes: Vec<&[PathBuf]> = arquivos.chunks(tamanho_lote.max(1)).collect();
    let resultados: Mutex<Vec<Option<(Vec<Registro>, Vec<String>, usize)>>> =
        Mutex::new((0..lotes.len()).map(|_| None).collect());
    let proximo = AtomicUsize::new(0);
    let feitos = AtomicUsize::new(0);
    let trabalhar = || {
        loop {
            let i = proximo.fetch_add(1, Ordering::SeqCst);
            if i >= lotes.len() {
                break;
            }
            let r = analisar_lote(motor, raiz, lotes[i], packages, opcoes);
            resultados.lock().expect("resultados")[i] = Some(r);
            let n = feitos.fetch_add(1, Ordering::SeqCst) + 1;
            if progresso && (n % 20 == 0 || n == lotes.len()) {
                eprintln!("  {n}/{} lotes", lotes.len());
            }
        }
    };
    std::thread::scope(|s| {
        let mut hs = Vec::new();
        for _ in 0..trabalhadores.max(1) {
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
    rodada.registros.sort();
    rodada.panicos.sort();
    rodada
}

/// Um lote; em pânico, cada arquivo sozinho.
fn analisar_lote(
    motor: &Motor,
    raiz: &Path,
    lote: &[PathBuf],
    packages: Option<&Path>,
    opcoes: &filtros::Opcoes,
) -> (Vec<Registro>, Vec<String>, usize) {
    let tentar = |arqs: &[PathBuf]| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let a = motor.analisar(raiz, arqs, packages);
            (registros(&a, raiz, opcoes, false), a.ambiguos)
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

/// Silencia a mensagem de pânico padrão (os pânicos são contados no relatório).
pub fn silenciar_panicos() {
    std::panic::set_hook(Box::new(|_| {}));
}
