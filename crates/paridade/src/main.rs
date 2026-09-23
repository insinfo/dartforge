//! `dartforge-paridade`: placar de paridade de diagnósticos com o `dart analyze`.
//!
//! ```text
//! dartforge-paridade placar [--grupo G]... [--trabalhadores N] [--lote N] [--detalhes]
//! dartforge-paridade determinismo [--trabalhadores 1,4,8] [--grupo G]...
//! dartforge-paridade oraculo [--grupo G]...          (regrava; precisa do dart)
//! dartforge-paridade gerar-corpus [--referencias DIR] [--pub-cache DIR]
//! dartforge-paridade projetos [--oraculo] [--mutacoes N] [--regravar] [--trabalhadores N] [--lote N]
//! ```
//!
//! Código de saída: 0 relatório completo; 1 oráculo ausente/desatualizado ou
//! determinismo divergente; 2 uso inválido.

use dartforge_paridade::analise::Motor;
use dartforge_paridade::corpus::{self, Grupo};
use dartforge_paridade::oraculo::{self, Meta, Registro};
use dartforge_paridade::placar::Placar;
use dartforge_paridade::projetos::{self, Mutacao, Projeto};
use dartforge_paridade::{Execucao, filtros, rodar_nosso};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

// O alocador contador dá ao processo filho de cada lote o vigia de memória
// (`executar_lote_filho`): uma análise que dispara não leva a máquina junto.
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

/// Teto de memória viva de um lote isolado (`DARTFORGE_PARIDADE_TETO_MB`, 2048).
fn teto() -> usize {
    std::env::var("DARTFORGE_PARIDADE_TETO_MB").ok().and_then(|v| v.parse().ok()).unwrap_or(2048usize) << 20
}

/// Cada lote num processo filho deste executável, com 90 s no máximo (um lote normal leva poucos segundos).
fn execucao(trabalhadores: usize, tamanho_lote: usize) -> Execucao {
    Execucao {
        trabalhadores,
        tamanho_lote,
        progresso: true,
        isolar: std::env::current_exe().ok(),
        tempo_max: std::time::Duration::from_secs(90),
    }
}

/// `_lote <raiz> <packages|->`, arquivos na entrada padrão (uso interno).
fn lote_filho(args: &[String]) -> ExitCode {
    let (Some(raiz), Some(pk)) = (args.first(), args.get(1)) else { return ExitCode::from(2) };
    let mut entrada = String::new();
    let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut entrada);
    let arquivos: Vec<PathBuf> = entrada.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect();
    let packages = (pk != "-").then(|| PathBuf::from(pk));
    let c = dartforge_paridade::executar_lote_filho(
        Path::new(raiz),
        packages.as_deref(),
        &arquivos,
        dartforge_instrument::live_bytes,
        teto(),
    );
    ExitCode::from(c as u8)
}

struct Args {
    cmd: String,
    grupos: Vec<String>,
    trabalhadores: Vec<usize>,
    lote: usize,
    detalhes: bool,
    referencias: PathBuf,
    pub_cache: PathBuf,
    oraculo: bool,
    mutacoes: usize,
    regravar: bool,
    corpus: PathBuf,
}

fn uso() -> ExitCode {
    eprintln!("{}", include_str!("main.rs").lines().skip(2).take(7).map(|l| l.trim_start_matches("//! ")).collect::<Vec<_>>().join("\n"));
    ExitCode::from(2)
}

fn ler_args() -> Option<Args> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next()?;
    let pub_cache = std::env::var_os("PUB_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(|l| Path::new(&l).join("Pub").join("Cache")))
        .unwrap_or_default()
        .join("hosted")
        .join("pub.dev");
    let mut a = Args {
        cmd,
        grupos: Vec::new(),
        trabalhadores: vec![],
        lote: 0,
        detalhes: false,
        referencias: PathBuf::from("D:/Projects/dartforge/references"),
        pub_cache,
        oraculo: false,
        mutacoes: 0,
        regravar: false,
        corpus: corpus::raiz_corpus(),
    };
    while let Some(x) = it.next() {
        match x.as_str() {
            "--grupo" => a.grupos.push(it.next()?),
            "--trabalhadores" => {
                a.trabalhadores = it.next()?.split(',').map(|n| n.trim().parse().ok()).collect::<Option<Vec<_>>>()?
            }
            "--lote" => a.lote = it.next()?.parse().ok()?,
            "--detalhes" => a.detalhes = true,
            "--referencias" => a.referencias = PathBuf::from(it.next()?),
            "--pub-cache" => a.pub_cache = PathBuf::from(it.next()?),
            "--oraculo" => a.oraculo = true,
            "--mutacoes" => a.mutacoes = it.next()?.parse().ok()?,
            "--regravar" => a.regravar = true,
            "--corpus" => a.corpus = PathBuf::from(it.next()?),
            _ => return None,
        }
    }
    a.corpus = std::path::absolute(&a.corpus).unwrap_or(a.corpus);
    Some(a)
}

fn main() -> ExitCode {
    let brutos: Vec<String> = std::env::args().skip(1).collect();
    if brutos.first().is_some_and(|c| c == "_lote") {
        dartforge_paridade::silenciar_panicos();
        return lote_filho(&brutos[1..]);
    }
    let Some(a) = ler_args() else { return uso() };
    dartforge_paridade::silenciar_panicos();
    match a.cmd.as_str() {
        "placar" => {
            let (texto, ok) = placar(&a, a.trabalhadores.first().copied().unwrap_or(4));
            print!("{texto}");
            if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        "determinismo" => determinismo(&a),
        "oraculo" => regravar_oraculo(&a),
        "gerar-corpus" => match corpus::gerar(&a.referencias, &a.corpus, &a.pub_cache) {
            Ok(r) => {
                print!("{r}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("erro: {e}");
                ExitCode::from(1)
            }
        },
        "projetos" => cmd_projetos(&a),
        "encurtar" => {
            for (nome, _) in grupos_escolhidos(&a) {
                match corpus::encurtar(&a.corpus.join(&nome)) {
                    Ok(n) => println!("{nome}: {n} arquivos renomeados"),
                    Err(e) => eprintln!("{nome}: {e}"),
                }
            }
            ExitCode::SUCCESS
        }
        _ => uso(),
    }
}

fn grupos_escolhidos(a: &Args) -> Vec<(String, Grupo)> {
    corpus::grupos(&a.corpus).into_iter().filter(|(n, _)| a.grupos.is_empty() || a.grupos.contains(n)).collect()
}

fn motor() -> Motor {
    Motor::descobrir().unwrap_or_else(|e| {
        eprintln!("erro: SDK: {e}");
        std::process::exit(2)
    })
}

/// O relatório inteiro (determinístico) e se o oráculo estava em dia.
fn placar(a: &Args, trabalhadores: usize) -> (String, bool) {
    let motor = motor();
    let mut total = Placar::default();
    let mut corpo = String::new();
    let mut ok = true;
    let mut arquivos = 0;
    let mut lotes = 0;
    let mut panicos = Vec::new();
    let mut ambiguos = 0;
    let grupos = grupos_escolhidos(a);
    for (nome, g) in &grupos {
        let dir = a.corpus.join(nome);
        let (oraculo, meta) = match oraculo::ler(&dir) {
            Ok(x) => x,
            Err(e) => {
                let _ = writeln!(corpo, "grupo {nome}: SEM ORÁCULO GRAVADO ({e}) — rode `dartforge-paridade oraculo --grupo {nome}`");
                ok = false;
                continue;
            }
        };
        if meta.hash != corpus::hash_fontes(&dir) {
            let _ = writeln!(corpo, "grupo {nome}: ORÁCULO DESATUALIZADO (fontes mudaram) — regravar");
            ok = false;
        }
        let cfg = match corpus::preparar(&dir, nome, g) {
            Ok(c) => c,
            Err(e) => {
                let _ = writeln!(corpo, "grupo {nome}: {e}");
                ok = false;
                continue;
            }
        };
        let fontes = corpus::arquivos_dart(&dir);
        let t = Instant::now();
        eprintln!("grupo {nome}: {} arquivos", fontes.len());
        let lote = if a.lote > 0 { a.lote } else { 48 };
        let r = rodar_nosso(&motor, &dir, &fontes, Some(&cfg), &filtros::Opcoes::ler(&dir), &execucao(trabalhadores, lote));
        eprintln!("grupo {nome}: {:.1} s", t.elapsed().as_secs_f64());
        arquivos += r.arquivos;
        lotes += r.lotes;
        ambiguos += r.ambiguos;
        panicos.extend(r.panicos.iter().map(|p| format!("{nome}/{p}")));
        let mut p = Placar::default();
        p.comparar(&oraculo, &r.registros);
        let tg = p.total();
        let _ = writeln!(
            corpo,
            "grupo {nome} (oráculo {}, {} arquivos): oráculo {} | nosso {} | acertos {} (mensagem errada {}) | posição errada {} | FP {} | FN {}",
            meta.sdk.versao(),
            r.arquivos,
            tg.oraculo,
            tg.nosso,
            tg.acertos,
            tg.mensagem_errada,
            tg.posicao_errada,
            tg.falsos_positivos,
            tg.falsos_negativos
        );
        total.somar(&p);
    }
    let t = total.total();
    let pct = if t.oraculo > 0 { 100.0 * t.acertos as f64 / t.oraculo as f64 } else { 0.0 };
    let mut s = String::new();
    let _ = writeln!(
        s,
        "placar paridade: {}/{} diagnósticos do oráculo na posição exata ({pct:.1}%), {} com mensagem igual; FP {}, FN {}, posição errada {}",
        t.acertos,
        t.oraculo,
        t.acertos - t.mensagem_errada,
        t.falsos_positivos,
        t.falsos_negativos,
        t.posicao_errada
    );
    let _ = writeln!(s, "DartForge paridade — {} grupos, {arquivos} arquivos, {lotes} lotes", grupos.len());
    s.push_str(&corpo);
    s.push('\n');
    s.push_str(&total.tabela("corpus inteiro"));
    // Regra de publicação: um verificado com divergência no corpus é erro.
    let ver = dartforge_paridade::verificados();
    let mut quebrados = Vec::new();
    for c in &ver {
        match total.por_codigo.get(*c) {
            Some(k) if k.perfeito() => {}
            Some(_) => quebrados.push(*c),
            None => {}
        }
    }
    let _ = writeln!(s, "\npublicados: sintaxe + {} códigos verificados {:?}", ver.len(), ver);
    if !quebrados.is_empty() {
        let _ = writeln!(s, "VERIFICADOS COM DIVERGÊNCIA NO CORPUS: {quebrados:?}");
        ok = false;
    }
    let candidatos: Vec<&String> = total.por_codigo.iter().filter(|(k, c)| c.perfeito() && !ver.contains(&k.as_str())).map(|(k, _)| k).collect();
    let _ = writeln!(s, "100% no corpus e fora da lista (candidatos, falta 0 FP nos projetos): {candidatos:?}");
    let _ = writeln!(s, "pânicos: {}{}", panicos.len(), if panicos.is_empty() { String::new() } else { format!(" {:?}", &panicos[..panicos.len().min(20)]) });
    let _ = writeln!(s, "atribuição ambígua (diagnóstico sem unidade, mais de uma candidata): {ambiguos}");
    if a.detalhes {
        s.push_str("\namostras:\n");
        s.push_str(&total.amostras_texto());
    }
    (s, ok)
}

fn determinismo(a: &Args) -> ExitCode {
    let ns = if a.trabalhadores.is_empty() { vec![1, 4, 8] } else { a.trabalhadores.clone() };
    let mut textos = Vec::new();
    for &n in &ns {
        let t = Instant::now();
        eprintln!("determinismo: {n} trabalhador(es)…");
        textos.push(placar(a, n).0);
        eprintln!("  {:.1} s", t.elapsed().as_secs_f64());
    }
    let iguais = textos.windows(2).all(|w| w[0] == w[1]);
    if iguais {
        println!("determinismo: idêntico com {ns:?} trabalhadores");
        print!("{}", textos[0]);
        ExitCode::SUCCESS
    } else {
        println!("determinismo: DIVERGENTE entre {ns:?} trabalhadores");
        for (i, t) in textos.iter().enumerate() {
            println!("--- {} trabalhadores\n{t}", ns[i]);
        }
        ExitCode::from(1)
    }
}

fn cache_oraculo() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/paridade-cache-dart");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn regravar_oraculo(a: &Args) -> ExitCode {
    let mut falhou = false;
    for (nome, g) in grupos_escolhidos(a) {
        let dir = a.corpus.join(&nome);
        if let Err(e) = corpus::preparar(&dir, &nome, &g) {
            eprintln!("{nome}: {e}");
            falhou = true;
            continue;
        }
        let t = Instant::now();
        eprintln!("oráculo {} em {nome}…", g.sdk.versao());
        // Grupos grandes: um `dart analyze` por fatia de subdiretórios (memória).
        let fatias = fatias(&dir, 2500);
        let mut regs = Vec::new();
        let mut excluidos = Vec::new();
        for alvo in &fatias {
            bissectar(g.sdk, &dir, alvo.clone(), &mut regs, &mut excluidos);
        }
        excluidos.sort();
        for e in &excluidos {
            eprintln!("  derruba o oráculo, retirado do grupo: {e}");
            let _ = std::fs::remove_file(dir.join(e));
        }
        // Diagnósticos em arquivos retirados não contam.
        regs.retain(|r| !excluidos.contains(&r.arquivo));
        regs.sort();
        regs.dedup();
        let meta = Meta {
            sdk: g.sdk,
            hash: corpus::hash_fontes(&dir),
            arquivos: corpus::arquivos_dart(&dir).len(),
            diagnosticos: regs.len(),
            excluidos,
        };
        if let Err(e) = oraculo::gravar(&dir, regs, &meta) {
            eprintln!("{nome}: {e}");
            falhou = true;
            continue;
        }
        println!(
            "{nome}: {} arquivos, {} diagnósticos do oráculo {} ({} fatia(s), {:.0} s)",
            meta.arquivos,
            meta.diagnosticos,
            g.sdk.versao(),
            fatias.len(),
            t.elapsed().as_secs_f64()
        );
    }
    if falhou { ExitCode::from(1) } else { ExitCode::SUCCESS }
}

/// Roda o oráculo sobre `alvos`; se o servidor de análise cair, divide ao
/// meio (um diretório sozinho vira os seus filhos) até isolar os arquivos
/// que o derrubam, que vão para `excluidos` (relativos a `dir`).
fn bissectar(
    sdk: oraculo::SdkOraculo,
    dir: &Path,
    alvos: Vec<PathBuf>,
    regs: &mut Vec<Registro>,
    excluidos: &mut Vec<String>,
) {
    match oraculo::rodar(sdk, dir, &alvos, &cache_oraculo()) {
        Ok(ds) => regs.extend(ds.iter().filter_map(|d| Registro::de_json(d, dir))),
        Err(e) => {
            if alvos.len() > 1 {
                let meio = alvos.len() / 2;
                bissectar(sdk, dir, alvos[..meio].to_vec(), regs, excluidos);
                bissectar(sdk, dir, alvos[meio..].to_vec(), regs, excluidos);
            } else if alvos[0].is_dir() {
                let mut filhos: Vec<PathBuf> = std::fs::read_dir(&alvos[0])
                    .map(|r| {
                        r.flatten()
                            .map(|e| e.path())
                            .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x == "dart"))
                            .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                            .collect()
                    })
                    .unwrap_or_default();
                filhos.sort();
                if filhos.is_empty() {
                    eprintln!("  {}: {e}", alvos[0].display());
                } else {
                    eprintln!("  o oráculo caiu em {}; dividindo", alvos[0].display());
                    bissectar(sdk, dir, filhos, regs, excluidos);
                }
            } else {
                excluidos.push(oraculo::relativo(&alvos[0], dir).unwrap_or_default());
            }
        }
    }
}

/// Alvos de cada execução do `dart analyze`: o grupo inteiro, ou os
/// subdiretórios agrupados até `max` arquivos por execução (memória do
/// servidor de análise). O `dart analyze` aceita vários alvos de uma vez.
fn fatias(dir: &Path, max: usize) -> Vec<Vec<PathBuf>> {
    if corpus::arquivos_dart(dir).len() <= max {
        return vec![vec![dir.to_path_buf()]];
    }
    let mut subs: Vec<(PathBuf, usize)> = Vec::new();
    let mut soltos = false;
    if let Ok(ents) = std::fs::read_dir(dir) {
        for e in ents.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() && !n.starts_with('.') {
                subs.push((p.clone(), corpus::arquivos_dart(&p).len()));
            } else if n.ends_with(".dart") {
                soltos = true;
            }
        }
    }
    if soltos {
        // Arquivos soltos na raiz: o alvo teria de ser a raiz inteira.
        return vec![vec![dir.to_path_buf()]];
    }
    subs.sort();
    let mut out: Vec<Vec<PathBuf>> = Vec::new();
    let mut n_atual = 0;
    for (p, n) in subs.into_iter().filter(|(_, n)| *n > 0) {
        if out.is_empty() || n_atual + n > max {
            out.push(Vec::new());
            n_atual = 0;
        }
        out.last_mut().expect("fatia").push(p);
        n_atual += n;
    }
    out
}
fn ler_projetos(corpus: &Path) -> Vec<(Projeto, Vec<Registro>)> {
    let caminho = corpus.join("projetos.json");
    match std::fs::read_to_string(&caminho).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        Some(v) => v,
        None => projetos::padrao().into_iter().map(|p| (p, Vec::new())).collect(),
    }
}

fn cmd_projetos(a: &Args) -> ExitCode {
    let motor = motor();
    let mut registro = ler_projetos(&a.corpus);
    let trab = a.trabalhadores.first().copied().unwrap_or(1);
    let lote = if a.lote > 0 { a.lote } else { 400 };
    let mut total = Placar::default();
    let mut s = String::new();
    for (p, oraculo_regs) in registro.iter_mut() {
        if !p.caminho.join("pubspec.yaml").is_file() {
            let _ = writeln!(s, "{}: ausente em {} (pulado)", p.nome, p.caminho.display());
            continue;
        }
        let opcoes = filtros::Opcoes::ler(&p.caminho);
        if a.oraculo {
            let t = Instant::now();
            eprintln!("{}: oráculo {}…", p.nome, p.sdk.versao());
            match oraculo::rodar(p.sdk, &p.caminho, &[p.caminho.clone()], &cache_oraculo()) {
                Ok(ds) => {
                    *oraculo_regs = ds.iter().filter_map(|d| Registro::de_json(d, &p.caminho)).collect();
                    oraculo_regs.sort();
                    p.oraculo = Some(oraculo_regs.len());
                    eprintln!("  {} diagnósticos, {:.0} s", oraculo_regs.len(), t.elapsed().as_secs_f64());
                }
                Err(e) => {
                    let _ = writeln!(s, "{}: oráculo falhou: {e}", p.nome);
                    continue;
                }
            }
        }
        let arquivos = projetos::arquivos(&p.caminho, &opcoes);
        let cfg = p.caminho.join(".dart_tool/package_config.json");
        let t = Instant::now();
        eprintln!("{}: nosso lado, {} arquivos…", p.nome, arquivos.len());
        // Pacotes aninhados (`example/` com pubspec próprio) são contextos de
        // análise à parte no analyzer: cada um com o seu package_config.
        let mut r = dartforge_paridade::Rodada::default();
        for (raiz_pkg, fs) in projetos::por_pacote(&p.caminho, &arquivos) {
            let c = raiz_pkg.join(".dart_tool/package_config.json");
            let c = if c.is_file() { c } else { cfg.clone() };
            let parcial = rodar_nosso(&motor, &p.caminho, &fs, Some(&c), &opcoes, &execucao(trab, lote));
            r.registros.extend(parcial.registros);
            r.arquivos += parcial.arquivos;
            r.lotes += parcial.lotes;
            r.panicos.extend(parcial.panicos);
            r.ambiguos += parcial.ambiguos;
        }
        r.registros.sort();
        let mut pl = Placar::default();
        pl.comparar(oraculo_regs, &r.registros);
        let tg = pl.total();
        let publicados = r
            .registros
            .iter()
            .filter(|x| x.tipo == "SYNTACTIC_ERROR" || dartforge_paridade::verificados().contains(&x.code.as_str()))
            .count();
        let _ = writeln!(
            s,
            "{} (oráculo {}: {} diagnósticos; {} arquivos, {:.0} s): nosso {} | FP {} | acertos {} | publicados pela regra {} | pânicos {}",
            p.nome,
            p.sdk.versao(),
            p.oraculo.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            r.arquivos,
            t.elapsed().as_secs_f64(),
            tg.nosso,
            tg.falsos_positivos,
            tg.acertos,
            publicados,
            r.panicos.len()
        );
        total.somar(&pl);
    }
    s.push('\n');
    s.push_str(&total.tabela("projetos reais (todo diagnóstico nosso sem par no oráculo é FP)"));
    if a.oraculo {
        let texto = serde_json::to_string_pretty(&registro).expect("JSON") + "\n";
        if let Err(e) = std::fs::write(a.corpus.join("projetos.json"), texto) {
            eprintln!("projetos.json: {e}");
        }
    }
    if a.mutacoes > 0 || a.regravar {
        s.push('\n');
        s.push_str(&mutacoes(a, &motor, &registro.iter().map(|x| x.0.clone()).collect::<Vec<_>>()));
    }
    print!("{s}");
    ExitCode::SUCCESS
}

fn mutacoes(a: &Args, motor: &Motor, lista: &[Projeto]) -> String {
    let arquivo = a.corpus.join("mutacoes.jsonl");
    let mut gravadas: Vec<Mutacao> = std::fs::read_to_string(&arquivo)
        .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
        .unwrap_or_default();
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch-paridade/projetos");
    let mut s = String::new();
    let mut placar = Placar::default();
    let mut desatualizadas = 0;
    let mut novas = Vec::new();
    for p in lista {
        if !p.caminho.join("pubspec.yaml").is_file() {
            continue;
        }
        let copia = scratch.join(&p.nome);
        let cfg = match projetos::copiar(p, &copia) {
            Ok(c) => c,
            Err(e) => {
                let _ = writeln!(s, "{}: cópia falhou: {e}", p.nome);
                continue;
            }
        };
        let opcoes = filtros::Opcoes::ler(&copia);
        let arquivos = projetos::arquivos(&copia, &opcoes);
        let mut ms: Vec<Mutacao> = if a.regravar {
            projetos::escolher(p, &copia, &arquivos, a.mutacoes.max(1))
        } else {
            gravadas.iter().filter(|m| m.projeto == p.nome).cloned().collect()
        };
        for m in ms.iter_mut() {
            let alvo = copia.join(&m.arquivo);
            let Ok(original) = std::fs::read_to_string(&alvo) else { continue };
            if projetos::hash_texto(&original) != m.hash_original {
                desatualizadas += 1;
                continue;
            }
            let mutante = m.aplicar(&original);
            if std::fs::write(&alvo, &mutante).is_err() {
                continue;
            }
            if a.regravar {
                eprintln!("mutação {} {} {}…", p.nome, m.tipo, m.arquivo);
                match oraculo::rodar(p.sdk, &copia, std::slice::from_ref(&alvo), &cache_oraculo()) {
                    Ok(ds) => {
                        m.oraculo = ds
                            .iter()
                            .filter_map(|d| Registro::de_json(d, &copia))
                            .filter(|r| r.arquivo == m.arquivo)
                            .collect();
                        m.oraculo.sort();
                    }
                    Err(e) => eprintln!("  oráculo falhou: {e}"),
                }
            }
            let r = motor.analisar(&copia, std::slice::from_ref(&alvo), Some(&cfg));
            let nossos: Vec<Registro> = dartforge_paridade::registros(&r, &copia, &opcoes, false)
                .into_iter()
                .filter(|x| x.arquivo == m.arquivo)
                .collect();
            placar.comparar(&m.oraculo, &nossos);
            let _ = std::fs::write(&alvo, &original);
        }
        novas.extend(ms);
    }
    if a.regravar {
        let mut t = String::new();
        for m in &novas {
            t.push_str(&serde_json::to_string(m).expect("JSON"));
            t.push('\n');
        }
        let _ = std::fs::write(&arquivo, t);
        gravadas = novas;
    }
    let _ = writeln!(s, "mutações: {} registradas, {desatualizadas} desatualizadas", gravadas.len());
    s.push_str(&placar.tabela("mutações dos projetos reais"));
    s.push_str(&placar.amostras_texto());
    s
}
