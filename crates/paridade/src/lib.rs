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
//! semântico só é publicado se estiver em `crates/analise/verificados.txt`
//! (zero falso positivo, posição ou mensagem errada no corpus e nos projetos
//! reais; falso negativo é permitido). O resto existe internamente e vai só
//! para o placar.

pub mod analise;
pub mod corpus;
pub mod filtros;
pub mod json;
pub mod oraculo;
pub mod placar;
pub mod ponte;
pub mod projetos;
pub mod saida;

use analise::{Analise, Motor};
use oraculo::Registro;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use dartforge_analise::publicacao::{VERIFICADOS, publicado, verificados};

/// Os diagnósticos da análise no JSON v1, com os filtros aplicados
/// (`analysis_options.yaml` da raiz e `// ignore:`). `so_publicados` aplica a
/// regra de publicação.
pub fn diagnosticos_json(analise: &Analise, raiz: &Path, opcoes: &filtros::Opcoes, so_publicados: bool) -> Vec<json::DiagJson> {
    let mut out = Vec::new();
    // As opções de subpasta (INFRA §4.5): o `analysis_options.yaml` mais
    // próximo acima do arquivo, abaixo da raiz, vale para ele; os `exclude`
    // dele somam-se aos da raiz.
    let mut de_subpasta: std::collections::HashMap<std::path::PathBuf, filtros::Opcoes> = std::collections::HashMap::new();
    for (p, a) in &analise.arquivos {
        if oraculo::relativo(p, raiz).is_some_and(|rel| opcoes.excluido(&rel)) {
            continue;
        }
        let proprias: Option<&filtros::Opcoes> = match filtros::Opcoes::de_subpasta(p, raiz) {
            Some(arquivo) => {
                if !de_subpasta.contains_key(&arquivo) {
                    let lidas = filtros::Opcoes::ler_arquivo(&arquivo);
                    de_subpasta.insert(arquivo.clone(), lidas);
                }
                de_subpasta.get(&arquivo)
            }
            None => None,
        };
        if proprias.is_some_and(|o| o.exclui_arquivo(p)) {
            continue;
        }
        let opcoes = proprias.unwrap_or(opcoes);
        let linhas = json::Linhas::new(&a.texto);
        let caminho = p.to_string_lossy();
        for (d, sintaxe) in publicaveis(a, opcoes, so_publicados) {
            out.push(json::para_json(&caminho, &linhas, &d, sintaxe));
        }
        // As regras de lint ligadas em `linter: rules:` (INFRA §8), sobre a
        // árvore do texto, com os comentários `// ignore:` e o `errors:` das
        // opções (que cala a regra ou troca a severidade).
        if opcoes.regras.values().any(|ligada| *ligada) {
            let ligada = |regra: &str| opcoes.regras.get(regra).copied().unwrap_or(false);
            let ignorados = filtros::Ignorados::de_texto(&a.texto);
            // Os relatos sobre a árvore do programa, com a semântica; sem
            // ela (arquivo fora do programa), sobre a árvore do texto.
            let mut relatos = match &a.relatos_de_lint {
                Some(r) => r.iter().filter(|x| ligada(x.codigo.nome)).cloned().collect::<Vec<_>>(),
                None => {
                    let mut nomes = dartforge_intern::Interner::new();
                    let analisado = dartforge_frontend::parser::parse(&a.texto, &mut nomes);
                    let unidade = dartforge_analise::Unidade { ast: &analisado.ast, unit: &analisado.unit, fonte: &a.texto };
                    dartforge_analise::lints::executar(unidade, &nomes, &ligada)
                }
            };
            // Os das regras que pedem o programa resolvido, já calculados
            // pelo motor.
            for l in &a.lints_semanticos {
                if let Some(codigo) = dartforge_analise::lints::codigos_g::TODOS.iter().copied().find(|k| k.unico == l.unico)
                    && ligada(codigo.nome)
                {
                    relatos.push(dartforge_analise::lints::regras::RelatoDeLint { codigo, span: l.span, args: l.args.clone() });
                }
            }
            relatos.sort_by_key(|r| (r.span.start, r.span.end, r.codigo.unico));
            // O mesmo código no mesmo lugar com os mesmos argumentos sai uma
            // vez só (`final (a, b) = …` relata a palavra por variável).
            relatos.dedup_by(|x, y| x.span == y.span && x.codigo.unico == y.codigo.unico && x.args == y.args);
            for r in relatos {
                let severidade = match opcoes.errors.get(r.codigo.nome) {
                    Some(None) => continue,
                    Some(Some(s)) => s.nome(),
                    None => "INFO",
                };
                // `cannot-ignore` com o nome do lint: o `// ignore:` não o cala.
                if opcoes.lint_ignoravel(r.codigo.nome, r.codigo.unico)
                    && ignorados.ignora_lint(r.codigo.nome, r.codigo.unico, linhas.ponto(r.span.start).line)
                {
                    continue;
                }
                out.push(json::DiagJson {
                    code: r.codigo.nome.to_string(),
                    severity: severidade.to_string(),
                    tipo: "LINT".to_string(),
                    location: json::Local {
                        file: caminho.to_string(),
                        range: json::Faixa { start: linhas.ponto(r.span.start), end: linhas.ponto(r.span.end.max(r.span.start)) },
                    },
                    problem_message: r.mensagem(),
                    correction_message: r.correcao(),
                    context_messages: Vec::new(),
                    documentation: r.codigo.documentado.then(|| format!("https://dart.dev/diagnostics/{}", r.codigo.nome)),
                });
            }
        }
    }
    json::ordenar(&mut out);
    out
}

/// Um relato de arquivo não-Dart no JSON v1.
fn relato_em_json(arquivo: &str, linhas: &json::Linhas<'_>, r: &dartforge_analise::naodart::Relato) -> json::DiagJson {
    json::DiagJson {
        code: r.codigo.nome.to_string(),
        severity: r.codigo.severidade.nome().to_string(),
        tipo: r.codigo.tipo.nome().to_string(),
        location: json::Local {
            file: arquivo.to_string(),
            range: json::Faixa { start: linhas.ponto(r.span.start), end: linhas.ponto(r.span.end.max(r.span.start)) },
        },
        problem_message: r.mensagem(),
        correction_message: r.correcao(),
        context_messages: Vec::new(),
        documentation: r.codigo.documentado.then(|| format!("https://dart.dev/diagnostics/{}", r.codigo.nome)),
    }
}

/// Os diagnósticos de cada `AndroidManifest.xml` sob `raiz` (o
/// `ManifestValidator` do analyzer, `dartforge_analise::naodart::manifesto`),
/// no JSON v1. Só com `analyzer: optional-checks: chrome-os-manifest-checks`
/// nas opções; pastas ocultas ficam fora.
pub fn diagnosticos_do_manifesto(raiz: &Path, opcoes: &filtros::Opcoes) -> Vec<json::DiagJson> {
    let mut out = Vec::new();
    if !opcoes.manifesto_do_chrome_os {
        return out;
    }
    let mut pilha = vec![raiz.to_path_buf()];
    let mut achados: Vec<PathBuf> = Vec::new();
    while let Some(pasta) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&pasta) else { continue };
        for e in entradas.flatten() {
            let (p, nome) = (e.path(), e.file_name());
            let nome = nome.to_string_lossy();
            if p.is_dir() {
                if !nome.starts_with('.') {
                    pilha.push(p);
                }
            } else if nome == "AndroidManifest.xml" {
                achados.push(p);
            }
        }
    }
    achados.sort();
    for arquivo in achados {
        let Ok(texto) = std::fs::read_to_string(&arquivo) else { continue };
        let linhas = json::Linhas::new(&texto);
        let caminho = arquivo.to_string_lossy();
        out.extend(dartforge_analise::naodart::manifesto::validar(&texto).iter().map(|r| relato_em_json(&caminho, &linhas, r)));
    }
    out
}

/// Os diagnósticos do `analysis_options.yaml` de `raiz`
/// (`analyzeAnalysisOptions`, `dartforge_analise::naodart::opcoes`), no JSON
/// v1. Um `include:` é relativo ao arquivo que inclui, ou `package:` pelo
/// `package_config.json` achado acima da raiz. A restrição de SDK não é
/// passada: uma regra de lint removida com `since` não é relatada.
pub fn diagnosticos_das_opcoes(raiz: &Path) -> Vec<json::DiagJson> {
    let arquivo = raiz.join("analysis_options.yaml");
    let Ok(texto) = std::fs::read_to_string(&arquivo) else { return Vec::new() };
    let config = dartforge_elements::config::PackageConfig::discover(&arquivo)
        .and_then(|p| dartforge_elements::config::PackageConfig::load(&p).ok());
    let resolver = |de: &Path, uri: &str| -> Option<PathBuf> {
        if uri.starts_with("package:") {
            config.as_ref().and_then(|c| c.resolve_package_uri(uri).ok())
        } else {
            de.parent().map(|p| p.join(uri))
        }
    };
    let raiz_em_texto = raiz.to_string_lossy();
    let ctx = dartforge_analise::naodart::opcoes::Contexto {
        arquivo: &arquivo,
        raiz_do_contexto: raiz_em_texto.as_ref(),
        resolver: &resolver,
        sdk_permite: None,
    };
    let linhas = json::Linhas::new(&texto);
    let caminho = arquivo.to_string_lossy();
    dartforge_analise::naodart::opcoes::analisar(&texto, &ctx).iter().map(|r| relato_em_json(&caminho, &linhas, r)).collect()
}

/// Os diagnósticos do `pubspec.yaml` de `raiz` (o `PubspecValidator` do
/// analyzer, `dartforge_analise::naodart::pubspec`, e os lints de pubspec
/// ligados nas opções, `dartforge_analise::lints::pubspec`), no JSON v1,
/// sem os calados por `# ignore:`/`# ignore_for_file:` (o
/// `IgnoreInfo.forYaml` do fim de `validatePubspec`). Sem o arquivo, nada.
/// Os avisos do validador saem como ele os dá; os lints levam o `errors:`
/// das opções.
pub fn diagnosticos_do_pubspec(raiz: &Path, opcoes: &filtros::Opcoes) -> Vec<json::DiagJson> {
    let arquivo = raiz.join("pubspec.yaml");
    let Ok(texto) = std::fs::read_to_string(&arquivo) else { return Vec::new() };
    let linhas = json::Linhas::new(&texto);
    let caminho = arquivo.to_string_lossy();
    let ignorados = dartforge_analise::lints::pubspec::IgnoradosYaml::de(&texto);
    let mut saida: Vec<json::DiagJson> = dartforge_analise::naodart::pubspec::validar(&texto, raiz)
        .into_iter()
        .filter(|r| !ignorados.ignora(r.codigo.nome, r.codigo.nome, r.span.start))
        .map(|r| json::DiagJson {
            code: r.codigo.nome.to_string(),
            severity: r.codigo.severidade.nome().to_string(),
            tipo: r.codigo.tipo.nome().to_string(),
            location: json::Local {
                file: caminho.to_string(),
                range: json::Faixa { start: linhas.ponto(r.span.start), end: linhas.ponto(r.span.end.max(r.span.start)) },
            },
            problem_message: r.mensagem(),
            correction_message: r.correcao(),
            context_messages: Vec::new(),
            documentation: r.codigo.documentado.then(|| format!("https://dart.dev/diagnostics/{}", r.codigo.nome)),
        })
        .collect();
    if opcoes.regras.values().any(|ligada| *ligada) {
        let ligada = |regra: &str| opcoes.regras.get(regra).copied().unwrap_or(false);
        for r in dartforge_analise::lints::pubspec::executar(&texto, &ligada) {
            if ignorados.ignora(r.codigo.nome, r.codigo.unico, r.span.start) {
                continue;
            }
            let severidade = match opcoes.errors.get(r.codigo.nome) {
                Some(None) => continue,
                Some(Some(s)) => s.nome(),
                None => "INFO",
            };
            saida.push(json::DiagJson {
                code: r.codigo.nome.to_string(),
                severity: severidade.to_string(),
                tipo: "LINT".to_string(),
                location: json::Local {
                    file: caminho.to_string(),
                    range: json::Faixa { start: linhas.ponto(r.span.start), end: linhas.ponto(r.span.end.max(r.span.start)) },
                },
                problem_message: r.mensagem(),
                correction_message: r.correcao(),
                context_messages: Vec::new(),
                documentation: r.codigo.documentado.then(|| format!("https://dart.dev/diagnostics/{}", r.codigo.nome)),
            });
        }
    }
    saida
}

/// Os diagnósticos de um arquivo que saem para o usuário, com a marca de
/// sintaxe: a regra de publicação (com `so_publicados`), o
/// `analysis_options.yaml` (`opcoes`) e os comentários `// ignore:` do
/// texto. É o filtro comum do `dartforge analyze` e do LSP.
pub fn publicaveis(a: &analise::Arquivo, opcoes: &filtros::Opcoes, so_publicados: bool) -> Vec<(dartforge_diagnostics::Diagnostic, bool)> {
    let linhas = json::Linhas::new(&a.texto);
    let ignorados = filtros::Ignorados::de_texto(&a.texto);
    let mut out = Vec::new();
    for (i, d) in a.diags.iter().enumerate() {
        let sintaxe = i < a.sintaticos;
        if so_publicados && !publicado(d, sintaxe) {
            continue;
        }
        // Os dois `inference_failure_on_*` do `BestPracticesVerifier` só com
        // `strict-inference: true` (INFRA §4.6); o motor os relata sempre.
        if !opcoes.strict_inference
            && d.code.is_some_and(|c| matches!(c.info().nome, "inference_failure_on_untyped_parameter" | "inference_failure_on_function_return_type"))
        {
            continue;
        }
        let Some(d) = opcoes.processar(d.clone()) else { continue };
        let linha = linhas.ponto(d.span.start).line;
        if opcoes.ignoravel(&d) && ignorados.ignora(&d, linha) {
            continue;
        }
        out.push((d, sintaxe));
    }
    // `IgnoreValidator` (a última fase da unidade): os nomes repetidos nos
    // comentários `ignore`.
    for d in ignorados.duplicados() {
        if so_publicados && !publicado(&d, false) {
            continue;
        }
        if let Some(d) = opcoes.processar(d) {
            out.push((d, false));
        }
    }
    out
}

/// Como [`diagnosticos_json`], em registros relativos a `raiz`, com o que o
/// `dart analyze` (o oráculo) imprime: um `TODO` só sai quando o `errors:`
/// o promoveu (`analyze.dart:191-192`, o mesmo filtro do nosso CLI).
pub fn registros(analise: &Analise, raiz: &Path, opcoes: &filtros::Opcoes, so_publicados: bool) -> Vec<Registro> {
    let mut out: Vec<Registro> = diagnosticos_json(analise, raiz, opcoes, so_publicados)
        .iter()
        .filter(|j| !(j.tipo == "TODO" && j.severity == "INFO"))
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