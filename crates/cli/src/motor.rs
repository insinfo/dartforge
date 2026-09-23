//! O motor de geração de código na linha de comando: `dartforge build` e a
//! ligação do motor no `compile-js` (`crates/build`, `docs/BUILD-MOTOR.md`).
use dartforge_build::consulta::SemBanco;
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::gerado::Geracao;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// O projeto da entrada usa builders? Custa ler o `package_config.json`
/// (que o carregador também lê) e uma busca num `HashMap`.
pub fn detectar(entrada: &Path, packages: Option<&Path>) -> Option<(PathBuf, PackageConfig)> {
    let caminho = packages.map(Path::to_path_buf).or_else(|| PackageConfig::discover(entrada))?;
    let cfg = PackageConfig::load(&caminho).ok()?;
    if !dartforge_build::detectar(&cfg) {
        return None;
    }
    Some((dartforge_build::raiz_do_pacote(entrada)?, cfg))
}

/// Uma passada do motor (o `compile-js`): o programa carregado sem os
/// gerados faz o papel do `BuildStep.resolver`.
pub fn gerar_uma_vez(
    raiz: &Path,
    cfg: &PackageConfig,
    programa: &dartforge_elements::model::Program,
    nomes: &dartforge_intern::Interner,
) -> Result<(Arc<Geracao>, String), String> {
    let mut m = Motor::novo(raiz, cfg, OpcoesMotor::default())?;
    let at = m.atualizar(&Contexto { banco: &SemBanco, programa: Some((programa, nomes)) }, &[], Demanda::Carregador)?;
    let mut texto = at.rel.texto();
    let n = at.avisos.len();
    for a in at.avisos.iter().take(5) {
        texto.push_str(&format!("\naviso: {a}"));
    }
    if n > 5 {
        texto.push_str(&format!("\n({} avisos de apoio possivelmente desatualizado)", n));
    }
    Ok((at.geracao, texto))
}

/// `dartforge build [<entrada.dart>] [--raiz <dir>] [--packages <cfg>] [--plano]
/// [--comparar] [--release] [--estrito] [--trabalhadores N]
/// [--escrever-cache <dir>]`.
pub fn run_build(args: &[std::ffi::OsString]) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<std::ffi::OsString> = args.to_vec();
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || build(&args))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "o build abortou")?
        .map_err(|e| e.into())
}

fn build(args: &[std::ffi::OsString]) -> Result<(), String> {
    let uso = "uso: dartforge build [<entrada.dart>] [--raiz <dir>] [--packages <cfg>] [--sdk <lib>] [--plano] [--comparar] [--release] [--estrito] [--trabalhadores N] [--escrever-cache <dir>]";
    let (mut entrada, mut raiz, mut packages, mut sdk, mut cache) = (None, None, None, None, None);
    let (mut plano, mut comparar, mut release, mut estrito) = (false, false, false, false);
    let mut trabalhadores = OpcoesMotor::default().trabalhadores;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let proximo = |it: &mut std::slice::Iter<'_, std::ffi::OsString>| it.next().map(PathBuf::from).ok_or(uso);
        match a.to_str() {
            Some("--raiz") => raiz = Some(proximo(&mut it)?),
            Some("--packages") => packages = Some(proximo(&mut it)?),
            Some("--sdk") => sdk = Some(proximo(&mut it)?),
            Some("--escrever-cache") => cache = Some(proximo(&mut it)?),
            Some("--plano") => plano = true,
            Some("--comparar") => comparar = true,
            Some("--release") => release = true,
            Some("--estrito") => estrito = true,
            Some("--trabalhadores") => {
                trabalhadores = it
                    .next()
                    .and_then(|v| v.to_str())
                    .and_then(|v| v.parse().ok())
                    .filter(|&n: &usize| n >= 1)
                    .ok_or("--trabalhadores exige um número ≥ 1")?;
            }
            _ if entrada.is_none() && !a.to_string_lossy().starts_with("--") => entrada = Some(PathBuf::from(a)),
            _ => return Err(uso.into()),
        }
    }
    let base = entrada.clone().or_else(|| std::env::current_dir().ok()).ok_or(uso)?;
    let raiz = raiz.or_else(|| dartforge_build::raiz_do_pacote(&base)).ok_or("pubspec.yaml não encontrado")?;
    let caminho_cfg = packages.clone().unwrap_or_else(|| raiz.join(".dart_tool/package_config.json"));
    let cfg = PackageConfig::load(&caminho_cfg)?;
    if !dartforge_build::detectar(&cfg) {
        println!("{}: o projeto não usa builders (sem build_runner no package_config.json)", raiz.display());
        return Ok(());
    }
    if plano {
        let (_, _, p) = dartforge_build::plano_do_projeto(&raiz, &cfg)?;
        print!("{}", p.texto_canonico());
        for a in &p.avisos {
            eprintln!("aviso: {a}");
        }
        let bd = raiz.join(".dart_tool/build/entrypoint/build.dart");
        if let Ok(texto) = std::fs::read_to_string(&bd) {
            match dartforge_build::oraculo::comparar(&p.aplicacoes, &texto)? {
                Ok(()) => println!("plano igual ao {} ({} aplicações)", bd.display(), p.aplicacoes.len()),
                Err(d) => return Err(format!("plano diferente do {}:\n{}", bd.display(), d.join("\n"))),
            }
        }
        return Ok(());
    }
    let opcoes = OpcoesMotor { release, trabalhadores, estrito, medir_nao_verificados: comparar };
    let mut motor = Motor::novo(&raiz, &cfg, opcoes)?;
    // O programa serve de `BuildStep.resolver` aos geradores nativos.
    let entrada = entrada.or_else(|| Some(raiz.join("web/main.dart")).filter(|p| p.is_file()));
    let mut nomes = dartforge_intern::Interner::new();
    let programa = match &entrada {
        Some(e) => {
            let dir = sdk
                .or_else(dartforge_elements::sdk::SdkLayout::discover)
                .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
            let layout = dartforge_elements::sdk::SdkLayout::load(&dir, "dartdevc")?;
            Some(dartforge_elements::load::load_lenient(e, &layout, packages.as_deref(), &mut nomes).0)
        }
        None => None,
    };
    let at = motor.atualizar(
        &Contexto { banco: &SemBanco, programa: programa.as_ref().map(|p| (p, &nomes)) },
        &[],
        Demanda::Tudo,
    )?;
    // Saídas `build_to: source` de gerador nativo que mudaram vão ao disco.
    for (p, c) in motor.saidas_source_nativas(&at.alterados) {
        if std::fs::read(&p).ok().as_deref() != Some(&c[..]) {
            std::fs::write(&p, &c[..]).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    println!("{}", at.rel.texto());
    for a in &at.avisos {
        eprintln!("aviso: {a}");
    }
    if let Some(dir) = cache {
        let mut n = 0;
        for (id, g) in &motor.grafo.gerados {
            if !g.oculto {
                continue;
            }
            let Some(c) = motor.registro(g.acao).and_then(|r| r.saidas.iter().find(|(s, _)| s == id)).and_then(|(_, c)| c.clone())
            else {
                continue;
            };
            let destino = dir.join(id.pacote.as_ref()).join(id.caminho.as_ref());
            if let Some(pai) = destino.parent() {
                std::fs::create_dir_all(pai).map_err(|e| e.to_string())?;
            }
            std::fs::write(&destino, &c[..]).map_err(|e| e.to_string())?;
            n += 1;
        }
        println!("{n} saídas de cache escritas em {}", dir.display());
    }
    if comparar {
        // Referência: o que o `build_runner` deixou no disco.
        let mut referencia = BTreeMap::new();
        for (id, g) in &motor.grafo.gerados {
            if let Ok(b) = std::fs::read(motor.caminho_de_apoio(id, g.oculto)) {
                referencia.insert(id.clone(), b);
            }
        }
        let p = motor.placar(&referencia);
        println!("placar contra o apoio do build_runner: {}", p.resumo());
        let mut por_ext: BTreeMap<String, [usize; 3]> = BTreeMap::new();
        let ext = |c: &str| {
            let nome = c.rsplit('/').next().unwrap_or(c);
            nome.find('.').map(|i| nome[i..].to_string()).unwrap_or_default()
        };
        for id in &p.iguais {
            por_ext.entry(ext(&id.caminho)).or_default()[0] += 1;
        }
        for (id, _) in &p.pendentes {
            por_ext.entry(ext(&id.caminho)).or_default()[1] += 1;
        }
        for (id, _) in &p.diferentes {
            por_ext.entry(ext(&id.caminho)).or_default()[2] += 1;
        }
        for (e, [i, pe, d]) in &por_ext {
            println!("  {e:<24} {i:>6} iguais {pe:>6} pendentes {d:>6} diferentes");
        }
        let mut motivos: Vec<(String, usize)> = p.motivos().into_iter().collect();
        motivos.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (m, n) in motivos.iter().take(15) {
            println!("  {n:>6}× {m}");
        }
        if !p.diferentes.is_empty() {
            for (id, m) in p.diferentes.iter().take(20) {
                eprintln!("diferente: {} ({m})", id.texto());
            }
            return Err(format!("{} saídas diferentes do oficial", p.diferentes.len()));
        }
    }
    Ok(())
}
