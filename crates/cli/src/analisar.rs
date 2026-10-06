//! `dartforge analyze`: os diagnósticos de diretórios ou arquivos, com a
//! saída do `dart analyze` do SDK 3.6.2 nos três formatos e o código de
//! saída dele (docs/ANALYZER-ESPECIFICACAO-INFRA.md §5.3–§5.7, etapa 4 da
//! Parte III). A paridade é a do modo **redirecionado**: sem ANSI, marcador
//! `-`, uma linha por diagnóstico.
//!
//! Publica só o que a regra do plano §2.3 permite: sintaxe sempre, e os
//! códigos semânticos de `crates/analise/verificados.txt`. `--todos` mostra
//! também os não verificados (para depurar, nunca para o editor).
//!
//! Reescrito em 2026-10-04 sem compilar nem executar.
use dartforge_paridade::json::DiagJson;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Formato {
    Padrao,
    Json,
    Maquina,
}

/// A ordem do `AnalysisError.compareTo` do dartdev
/// (`dartdev/lib/src/analysis_server.dart:385-402`): severidade (erro, aviso,
/// informação), caminho absoluto comparado por unidades UTF-16, offset e
/// mensagem. Empates além disso ficam na ordem de chegada.
fn ordenar(v: &mut [DiagJson]) {
    let prioridade = |s: &str| match s {
        "ERROR" => 0u8,
        "WARNING" => 1,
        "INFO" => 2,
        _ => 3,
    };
    v.sort_by(|a, b| {
        prioridade(&a.severity)
            .cmp(&prioridade(&b.severity))
            .then_with(|| a.location.file.encode_utf16().cmp(b.location.file.encode_utf16()))
            .then_with(|| a.location.range.start.offset.cmp(&b.location.range.start.offset))
            .then_with(|| a.problem_message.encode_utf16().cmp(b.problem_message.encode_utf16()))
    });
}

/// O caminho com o separador do sistema.
fn nativo(caminho: &str) -> String {
    caminho.replace('/', std::path::MAIN_SEPARATOR_STR)
}

/// `_relativePath` (`analyze.dart:481-485`): relativo a `base`, usado só se
/// não for mais longo que o absoluto.
fn relativo(arquivo: &str, base: &Path) -> String {
    let absoluto = nativo(arquivo);
    match Path::new(&absoluto).strip_prefix(base) {
        Ok(rel) => {
            let rel = rel.to_string_lossy().into_owned();
            if rel.len() <= absoluto.len() && !rel.is_empty() { rel } else { absoluto }
        }
        Err(_) => absoluto,
    }
}

/// `_escapeForMachineMode` (`analyze.dart:463-478`).
fn escapar_para_maquina(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' => saida.push_str("\\n"),
            '\r' => saida.push_str("\\r"),
            '\\' => saida.push_str("\\\\"),
            '|' => saida.push_str("\\|"),
            outro => saida.push(outro),
        }
    }
    saida
}

pub fn run(args: &[std::ffi::OsString]) -> Result<std::process::ExitCode, Box<dyn std::error::Error>> {
    let usage = "usage: dartforge analyze [--format=default|json|machine] [--fatal-infos] [--[no-]fatal-warnings] \
                 [--packages=<arquivo>] [--todos] [<diretório|arquivo>...]";
    let mut formato = Formato::Padrao;
    let mut todos = false;
    let mut fatal_infos = false;
    let mut fatal_avisos = true;
    let mut packages_pedido: Option<PathBuf> = None;
    let mut alvos: Vec<PathBuf> = Vec::new();
    for a in args {
        match a.to_str() {
            Some("--format=json") => formato = Formato::Json,
            Some("--format=default") => formato = Formato::Padrao,
            Some("--format=machine") => formato = Formato::Maquina,
            Some("--todos") => todos = true,
            Some("--fatal-infos") => fatal_infos = true,
            Some("--no-fatal-infos") => fatal_infos = false,
            Some("--fatal-warnings") => fatal_avisos = true,
            Some("--no-fatal-warnings") => fatal_avisos = false,
            // Aceitas e sem efeito aqui: a análise roda em processo, com o
            // SDK que o motor descobre.
            Some("--memory") => {}
            Some(s) if s.starts_with("--cache=") || s.starts_with("--sdk-path=") || s.starts_with("--enable-experiment=") => {}
            Some(s) if s.starts_with("--packages=") => packages_pedido = Some(PathBuf::from(&s["--packages=".len()..])),
            Some(s) if s.starts_with('-') => return Err(usage.into()),
            _ => alvos.push(PathBuf::from(a)),
        }
    }
    // Sem alvo: o diretório corrente, cujo nome vai no cabeçalho.
    let digitados: Vec<PathBuf> = if alvos.is_empty() { vec![std::env::current_dir()?] } else { alvos };
    for d in &digitados {
        if !d.exists() {
            return Err(format!("Directory or file doesn't exist: {}", d.display()).into());
        }
    }
    let absolutos: Vec<PathBuf> = digitados.iter().map(std::path::absolute).collect::<Result<_, _>>()?;
    // `relativeToDir`: com um alvo, a pasta alvo (ou a do arquivo alvo); com
    // vários, o diretório corrente.
    let base_relativa: PathBuf = if absolutos.len() == 1 {
        let unico = &absolutos[0];
        if unico.is_dir() { unico.clone() } else { unico.parent().unwrap_or(Path::new(".")).to_path_buf() }
    } else {
        std::env::current_dir()?
    };
    let motor = dartforge_paridade::analise::Motor::descobrir()?;
    // Corpos profundos recursam fundo: pilha própria, como o `compile-js`.
    let mut diags: Vec<DiagJson> = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let mut v = Vec::new();
            for alvo in &absolutos {
                let raiz = raiz_do_pacote(alvo);
                let opcoes = dartforge_paridade::filtros::Opcoes::ler(&raiz);
                // O `pubspec.yaml` do pacote, quando o alvo é a pasta dele.
                if alvo.is_dir() && *alvo == raiz {
                    v.extend(dartforge_paridade::diagnosticos_do_pubspec(&raiz, &opcoes));
                    v.extend(dartforge_paridade::diagnosticos_das_opcoes(&raiz));
                    v.extend(dartforge_paridade::diagnosticos_do_manifesto(&raiz, &opcoes));
                }
                let arquivos: Vec<PathBuf> = if alvo.is_dir() {
                    dartforge_paridade::corpus::arquivos_dart(alvo)
                        .into_iter()
                        .filter(|a| dartforge_paridade::oraculo::relativo(a, &raiz).is_some_and(|r| !opcoes.excluido(&r)))
                        .collect()
                } else {
                    // O arquivo pedido na linha de comando não é excluído.
                    vec![alvo.clone()]
                };
                let do_pacote = raiz.join(".dart_tool").join("package_config.json");
                let padrao = packages_pedido.clone().or_else(|| do_pacote.is_file().then_some(do_pacote));
                // Pacotes aninhados (`example/`): cada um com o seu package_config.
                for (raiz_pkg, fs) in dartforge_paridade::projetos::por_pacote(&raiz, &arquivos) {
                    let c = raiz_pkg.join(".dart_tool").join("package_config.json");
                    let c = if c.is_file() { Some(c) } else { padrao.clone() };
                    let a = motor.analisar(&raiz, &fs, c.as_deref());
                    v.extend(dartforge_paridade::diagnosticos_json(&a, &raiz, &opcoes, !todos));
                }
            }
            v
        })?
        .join()
        .map_err(|_| "a análise abortou")?;
    // Um `TODO` só sai quando o `errors:` o promoveu (`analyze.dart:191-192`).
    diags.retain(|d| !(d.tipo == "TODO" && d.severity == "INFO"));
    ordenar(&mut diags);
    let (mut erros, mut avisos, mut infos) = (false, false, false);
    for d in &diags {
        match d.severity.as_str() {
            "ERROR" => erros = true,
            "WARNING" => avisos = true,
            "INFO" => infos = true,
            _ => {}
        }
    }
    match formato {
        Formato::Json => {
            // Uma linha, com a quebra do `print` do Dart no fim.
            println!("{}", dartforge_paridade::json::escrever(diags));
        }
        Formato::Maquina => {
            // Sem diagnósticos, nenhuma saída.
            for d in &diags {
                let comprimento = d.location.range.end.offset.saturating_sub(d.location.range.start.offset);
                println!(
                    "{}|{}|{}|{}|{}|{}|{}|{}",
                    d.severity,
                    d.tipo,
                    d.code.to_uppercase(),
                    escapar_para_maquina(&nativo(&d.location.file)),
                    d.location.range.start.line,
                    d.location.range.start.column,
                    comprimento,
                    escapar_para_maquina(&d.problem_message)
                );
            }
        }
        Formato::Padrao => {
            // O basename de cada alvo COMO DIGITADO (`.` fica `.`); sem alvo,
            // o nome do diretório corrente.
            let nomes: Vec<String> = digitados
                .iter()
                .map(|p| p.file_name().map_or_else(|| p.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned()))
                .collect();
            println!("Analyzing {}...", nomes.join(", "));
            if diags.is_empty() {
                // Sem linha em branco antes: ela pertence ao bloco de erros.
                println!("No issues found!");
            } else {
                // PONTO DE EXTENSÃO (etapa 7): o bloco prioritário dos erros de
                // `pubspec.yaml`/`analysis_options.yaml` entra aqui, antes.
                println!();
                for d in &diags {
                    let mut msg = d.problem_message.clone();
                    if let Some(c) = &d.correction_message {
                        msg.push(' ');
                        msg.push_str(c);
                    }
                    println!(
                        "{:>7} - {}:{}:{} - {} - {}",
                        d.severity.to_lowercase(),
                        relativo(&d.location.file, &base_relativa),
                        d.location.range.start.line,
                        d.location.range.start.column,
                        msg,
                        d.code
                    );
                    // As mensagens de contexto: dez espaços, ` - `, a
                    // mensagem sem o ponto final, e o caminho do arquivo DO
                    // ERRO com a linha e a coluna da mensagem
                    // (`analyze.dart:371-378`).
                    for c in &d.context_messages {
                        println!(
                            "{} - {} at {}:{}:{}.",
                            " ".repeat(10),
                            c.message.trim_end_matches('.'),
                            relativo(&d.location.file, &base_relativa),
                            c.location.range.start.line,
                            c.location.range.start.column
                        );
                    }
                }
                println!();
                println!("{} issue{} found.", diags.len(), if diags.len() == 1 { "" } else { "s" });
            }
        }
    }
    // `_Result` (`analyze.dart:489-508`): 3 com erro; 2 com aviso (o
    // `--fatal-warnings` é o padrão); 1 com informação e `--fatal-infos`.
    Ok(std::process::ExitCode::from(if erros {
        3
    } else if avisos && fatal_avisos {
        2
    } else if infos && fatal_infos {
        1
    } else {
        0
    }))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn escape_do_formato_de_maquina() {
        assert_eq!(escapar_para_maquina("a|b\\c\nd"), "a\\|b\\\\c\\nd");
    }

    #[test]
    fn relativo_so_quando_nao_e_mais_longo() {
        let sep = std::path::MAIN_SEPARATOR_STR;
        let base = PathBuf::from(format!("{sep}raiz{sep}pacote"));
        let arquivo = format!("{sep}raiz{sep}pacote{sep}lib{sep}a.dart");
        assert_eq!(relativo(&arquivo, &base), format!("lib{sep}a.dart"));
        let fora = format!("{sep}outro{sep}a.dart");
        assert_eq!(relativo(&fora, &base), fora);
    }
}
