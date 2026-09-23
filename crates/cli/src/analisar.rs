//! `dartforge analyze`: os diagnósticos de um diretório ou arquivo, no mesmo
//! JSON v1 do `dart analyze --format=json` (ou no texto do formato padrão).
//!
//! Publica só o que a regra do plano §2.3 permite: sintaxe sempre, e os
//! códigos semânticos de `crates/paridade/verificados.txt`. `--todos` mostra
//! também os não verificados (para depurar, nunca para o editor).
use std::path::{Path, PathBuf};

/// A raiz do pacote: o diretório mais próximo, subindo, com `pubspec.yaml`.
fn raiz_do_pacote(p: &Path) -> PathBuf {
    let inicio = if p.is_dir() { p.to_path_buf() } else { p.parent().unwrap_or(Path::new(".")).to_path_buf() };
    let mut d = Some(inicio.as_path());
    while let Some(x) = d {
        if x.join("pubspec.yaml").is_file() {
            return x.to_path_buf();
        }
        d = x.parent();
    }
    inicio
}

pub fn run(args: &[std::ffi::OsString]) -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let usage = "usage: dartforge analyze [--format=json] [--todos] [<diretório|arquivo>]";
    let mut json = false;
    let mut todos = false;
    let mut alvo: Option<PathBuf> = None;
    for a in args {
        match a.to_str() {
            Some("--format=json") => json = true,
            Some("--format=default") => json = false,
            Some("--todos") => todos = true,
            Some(s) if s.starts_with('-') => return Err(usage.into()),
            _ if alvo.is_none() => alvo = Some(PathBuf::from(a)),
            _ => return Err(usage.into()),
        }
    }
    let alvo = std::path::absolute(alvo.unwrap_or_else(|| PathBuf::from(".")))?;
    let raiz = raiz_do_pacote(&alvo);
    let opcoes = dartforge_paridade::filtros::Opcoes::ler(&raiz);
    let arquivos: Vec<PathBuf> = if alvo.is_dir() {
        dartforge_paridade::corpus::arquivos_dart(&alvo)
            .into_iter()
            .filter(|a| dartforge_paridade::oraculo::relativo(a, &raiz).is_some_and(|r| !opcoes.excluido(&r)))
            .collect()
    } else {
        vec![alvo.clone()]
    };
    let packages = raiz.join(".dart_tool").join("package_config.json");
    let packages = packages.is_file().then_some(packages);
    let motor = dartforge_paridade::analise::Motor::descobrir()?;
    let (r2, o2) = (raiz.clone(), opcoes.clone());
    // Corpos profundos recursam fundo: pilha própria, como o `compile-js`.
    let diags = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            // Pacotes aninhados (`example/`): cada um com o seu package_config.
            let mut v = Vec::new();
            for (raiz_pkg, fs) in dartforge_paridade::projetos::por_pacote(&r2, &arquivos) {
                let c = raiz_pkg.join(".dart_tool").join("package_config.json");
                let c = if c.is_file() { Some(c) } else { packages.clone() };
                let a = motor.analisar(&r2, &fs, c.as_deref());
                v.extend(dartforge_paridade::diagnosticos_json(&a, &r2, &o2, !todos));
            }
            dartforge_paridade::json::ordenar(&mut v);
            v
        })?
        .join()
        .map_err(|_| "a análise abortou")?;
    let (mut erros, mut avisos) = (false, false);
    for d in &diags {
        match d.severity.as_str() {
            "ERROR" => erros = true,
            "WARNING" => avisos = true,
            _ => {}
        }
    }
    if json {
        print!("{}", dartforge_paridade::json::escrever(diags));
    } else {
        println!("Analyzing {}...\n", alvo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        for d in &diags {
            let rel = dartforge_paridade::oraculo::relativo(Path::new(&d.location.file), &raiz)
                .unwrap_or_else(|| d.location.file.clone());
            let mut msg = d.problem_message.clone();
            if let Some(c) = &d.correction_message {
                msg.push(' ');
                msg.push_str(c);
            }
            println!(
                "{:>7} - {}:{}:{} - {} - {}",
                d.severity.to_lowercase(),
                rel.replace('/', std::path::MAIN_SEPARATOR_STR),
                d.location.range.start.line,
                d.location.range.start.column,
                msg,
                d.code
            );
        }
        if diags.is_empty() {
            println!("No issues found!");
        } else {
            println!("\n{} issue{} found.", diags.len(), if diags.len() == 1 { "" } else { "s" });
        }
    }
    // Os códigos do `dart analyze`: 3 com erro, 2 com aviso (--fatal-warnings é o padrão).
    Ok(std::process::ExitCode::from(if erros {
        3
    } else if avisos {
        2
    } else {
        0
    }))
}
