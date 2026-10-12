//! `dartforge format`: o `dart format` do SDK 3.6.2 (`dart_style` 2.3.8),
//! com as mesmas opções principais, mensagens e códigos de saída
//! (`lib/src/cli/format_command.dart`, `io.dart`, `formatter_options.dart`).
//!
//! Códigos de saída: 0 sem erro; 1 com `--set-exit-if-changed` quando algum
//! arquivo muda; 65 quando algum arquivo não pôde ser formatado; 64 em erro de
//! uso. Como no original, o último a acontecer vence entre 1 e 65.
//!
//! As correções opcionais (`--fix*`) não são suportadas.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use dartforge_formatter::{ErroFormatacao, Opcoes, formatar};

const USO: &str = "Usage: dart format [options...] <files or directories...>
-h, --help                     Print this usage information.
-v, --verbose                  Show all options and flags with --help.
-o, --output                   Set where to write formatted output.

          [json]               Print code and selection as JSON.
          [none]               Discard output.
          [show]               Print code to terminal.
          [write] (default)    Overwrite formatted files on disk.

    --set-exit-if-changed      Return exit code 1 if there are any formatting changes.

Run \"dart help\" to see global options.";

const USO_DETALHADO: &str = "Usage: dart format [options...] <files or directories...>
-h, --help                             Print this usage information.
-v, --verbose                          Show all options and flags with --help.

Output options:
-o, --output                           Set where to write formatted output.

          [json]                       Print code and selection as JSON.
          [none]                       Discard output.
          [show]                       Print code to terminal.
          [write] (default)            Overwrite formatted files on disk.

    --show                             Set which filenames to print.

          [all]                        All visited files and directories.
          [changed] (default)          Only the names of files whose formatting is changed.
          [none]                       No file names or directories.

    --summary                          Show the specified summary after formatting.

          [line] (default)             Single-line summary.
          [none]                       No summary.
          [profile]                    How long it took for format each file.

    --set-exit-if-changed              Return exit code 1 if there are any formatting changes.

Other options:
-i, --indent                           Add this many spaces of leading indentation.
                                       (defaults to \"0\")
    --follow-links                     Follow links to files and directories.
                                       If unset, links will be ignored.
    --version                          Show dart_style version.
    --enable-experiment                Enable one or more experimental features.
                                       See dart.dev/go/experiments.

Options when formatting from stdin:
    --stdin-name                       Use this path in error messages when input is read from stdin.
                                       (defaults to \"stdin\")

Run \"dart help\" to see global options.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Saida {
    Escrever,
    Mostrar,
    Json,
    Nenhuma,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mostrar {
    Todos,
    Alterados,
    Nenhum,
}

struct Config {
    saida: Saida,
    mostrar: Mostrar,
    resumo: bool,
    sair_se_mudou: bool,
    opcoes: Opcoes,
    seguir_links: bool,
}

struct Estado {
    arquivos: usize,
    alterados: usize,
    codigo: u8,
}

fn erro_de_uso(msg: &str) -> ExitCode {
    eprintln!("{msg}\n\n{USO}");
    ExitCode::from(64)
}

/// Executa `dartforge format` com os argumentos depois do subcomando.
pub fn run(args: &[OsString]) -> ExitCode {
    let args: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
    let mut saida = Saida::Escrever;
    let mut saida_dada = false;
    let mut mostrar = Mostrar::Alterados;
    let mut mostrar_dado = false;
    let mut resumo = true;
    let mut resumo_dado = false;
    let mut sair_se_mudou = false;
    let mut largura = "80".to_string();
    let mut recuo = "0".to_string();
    let mut seguir_links = false;
    let mut ajuda = false;
    let mut detalhado = false;
    let mut versao = false;
    let mut nome_stdin: Option<String> = None;
    let mut caminhos: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if a == "--" {
            caminhos.extend(args[i..].iter().cloned());
            break;
        }
        if !a.starts_with('-') || a == "-" {
            caminhos.push(a.clone());
            continue;
        }
        // Separa `--nome=valor`, `-oshow` e as formas com o valor no
        // argumento seguinte.
        let (nome, valor_colado): (String, Option<String>) = if let Some(resto) = a.strip_prefix("--") {
            match resto.split_once('=') {
                Some((n, v)) => (format!("--{n}"), Some(v.to_string())),
                None => (a.clone(), None),
            }
        } else if a.len() > 2 {
            (a[..2].to_string(), Some(a[2..].to_string()))
        } else {
            (a.clone(), None)
        };
        let com_valor = matches!(
            nome.as_str(),
            "-o" | "--output"
                | "--show"
                | "--summary"
                | "-l"
                | "--line-length"
                | "-i"
                | "--indent"
                | "--language-version"
                | "--enable-experiment"
                | "--stdin-name"
                | "--selection"
        );
        let valor = if com_valor {
            match valor_colado {
                Some(v) => Some(v),
                None => {
                    if i >= args.len() {
                        return erro_de_uso(&format!("Missing argument for \"{nome}\"."));
                    }
                    i += 1;
                    Some(args[i - 1].clone())
                }
            }
        } else {
            if valor_colado.is_some() && nome.starts_with("--") {
                return erro_de_uso(&format!("Flag \"{nome}\" does not take a value."));
            }
            None
        };
        let nao_permitido = |v: &str| erro_de_uso(&format!("\"{v}\" is not an allowed value for option \"{nome}\"."));
        match nome.as_str() {
            "-h" | "--help" => ajuda = true,
            "-v" | "--verbose" => detalhado = true,
            "--version" => versao = true,
            "--set-exit-if-changed" => sair_se_mudou = true,
            "--follow-links" => seguir_links = true,
            "-o" | "--output" => {
                let v = valor.expect("valor");
                saida = match v.as_str() {
                    "write" => Saida::Escrever,
                    "show" => Saida::Mostrar,
                    "json" => Saida::Json,
                    "none" => Saida::Nenhuma,
                    _ => return nao_permitido(&v),
                };
                saida_dada = true;
            }
            "--show" => {
                let v = valor.expect("valor");
                mostrar = match v.as_str() {
                    "all" => Mostrar::Todos,
                    "changed" => Mostrar::Alterados,
                    "none" => Mostrar::Nenhum,
                    _ => return nao_permitido(&v),
                };
                mostrar_dado = true;
            }
            "--summary" => {
                let v = valor.expect("valor");
                resumo = match v.as_str() {
                    "line" | "profile" => true,
                    "none" => false,
                    _ => return nao_permitido(&v),
                };
                resumo_dado = true;
            }
            "-l" | "--line-length" => largura = valor.expect("valor"),
            "-i" | "--indent" => recuo = valor.expect("valor"),
            "--language-version" => {
                let v = valor.expect("valor");
                let ok = v == "latest"
                    || v.split_once('.')
                        .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()));
                if !ok {
                    return erro_de_uso(&format!("--language-version must be a version like \"3.2\" or \"latest\", was \"{v}\"."));
                }
            }
            "--enable-experiment" | "--selection" => {}
            "--stdin-name" => nome_stdin = valor,
            n if n.starts_with("--fix") => {
                eprintln!("dartforge format: {n} não é suportado.");
                return ExitCode::from(64);
            }
            _ => {
                let mostrado = if a.starts_with("--") { nome.clone() } else { nome[1..].to_string() };
                if a.starts_with("--") {
                    return erro_de_uso(&format!("Could not find an option named \"{mostrado}\"."));
                }
                return erro_de_uso(&format!("Could not find an option or flag \"-{mostrado}\"."));
            }
        }
    }

    if ajuda {
        println!("Idiomatically format Dart source code.\n\n{}", if detalhado { USO_DETALHADO } else { USO });
        return ExitCode::SUCCESS;
    }
    if versao {
        println!("2.3.8");
        return ExitCode::SUCCESS;
    }
    if !saida_dada && caminhos.is_empty() {
        saida = Saida::Mostrar;
    }
    if !mostrar_dado && (saida == Saida::Mostrar || saida == Saida::Json) {
        mostrar = Mostrar::Nenhum;
    }
    if !resumo_dado && saida == Saida::Json {
        resumo = false;
    }
    if detalhado {
        return erro_de_uso("Can only use --verbose with --help.");
    }
    if saida == Saida::Json && resumo {
        return erro_de_uso("Cannot print a summary with JSON output.");
    }
    let Ok(largura) = largura.parse::<i64>() else {
        return erro_de_uso(&format!("--line-length must be an integer, was \"{largura}\"."));
    };
    let Ok(recuo_n) = recuo.parse::<i64>() else {
        return erro_de_uso(&format!("--indent must be an integer, was \"{recuo}\"."));
    };
    if recuo_n < 0 {
        return erro_de_uso(&format!("--indent must be non-negative, was \"{recuo}\"."));
    }
    if caminhos.is_empty() && std::io::stdin().is_terminal() {
        return erro_de_uso("Missing paths to code to format.");
    }
    if caminhos.is_empty() && saida == Saida::Escrever {
        return erro_de_uso("Cannot use --output=write when reading from stdin.");
    }
    if nome_stdin.is_some() && !caminhos.is_empty() {
        return erro_de_uso("Cannot pass --stdin-name when not reading from stdin.");
    }

    let config = Config {
        saida,
        mostrar,
        resumo,
        sair_se_mudou,
        opcoes: Opcoes {
            largura: largura.max(0) as usize,
            recuo: recuo_n as usize,
            fim_de_linha: None,
        },
        seguir_links,
    };
    let mut estado = Estado {
        arquivos: 0,
        alterados: 0,
        codigo: 0,
    };
    let inicio = Instant::now();
    if caminhos.is_empty() {
        let nome = nome_stdin.unwrap_or_else(|| "stdin".into());
        let mut fonte = String::new();
        if std::io::stdin().read_to_string(&mut fonte).is_err() {
            eprintln!("Could not read stdin.");
            return ExitCode::from(65);
        }
        processar_fonte(&config, &mut estado, None, &nome, &fonte);
        return ExitCode::from(estado.codigo);
    }
    for c in &caminhos {
        let p = Path::new(c);
        if p.is_dir() {
            if !processar_diretorio(&config, &mut estado, c) {
                estado.codigo = 65;
            }
            continue;
        }
        if p.is_file() {
            if !processar_arquivo(&config, &mut estado, p, c) {
                estado.codigo = 65;
            }
        } else {
            eprintln!("No file or directory found at \"{c}\".");
        }
    }
    if config.resumo {
        let tempo = format!("{:.2}", inicio.elapsed().as_millis() as f64 / 1000.0);
        let n = estado.arquivos;
        let m = estado.alterados;
        if n == 0 {
            println!("Formatted no files in {tempo} seconds.");
        } else if n == 1 {
            println!("Formatted {n} file ({m} changed) in {tempo} seconds.");
        } else {
            println!("Formatted {n} files ({m} changed) in {tempo} seconds.");
        }
    }
    ExitCode::from(estado.codigo)
}

/// `p.normalize(path)` simplificado: remove `.` e `a/..`, junta barras.
fn normalizar(caminho: &str) -> String {
    let absoluto = caminho.starts_with('/');
    let mut partes: Vec<&str> = Vec::new();
    for parte in caminho.split('/') {
        match parte {
            "" | "." => {}
            ".." => {
                if partes.last().is_some_and(|p| *p != "..") {
                    partes.pop();
                } else if !absoluto {
                    partes.push("..");
                }
            }
            _ => partes.push(parte),
        }
    }
    let s = partes.join("/");
    if absoluto {
        format!("/{s}")
    } else if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

/// `_processDirectory`: todos os `.dart` abaixo, em ordem de caminho, sem
/// links (a menos de `--follow-links`) e sem diretórios ocultos.
fn processar_diretorio(config: &Config, estado: &mut Estado, dir: &str) -> bool {
    let mut entradas: Vec<(String, bool, bool)> = Vec::new();
    listar(Path::new(dir), dir, config.seguir_links, &mut entradas);
    entradas.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
    let mut sucesso = true;
    for (caminho, e_arquivo, e_link) in entradas {
        if e_link || !e_arquivo || !caminho.ends_with(".dart") {
            continue;
        }
        let relativo = caminho.strip_prefix(dir).unwrap_or(&caminho).trim_start_matches('/');
        if relativo.split('/').any(|p| p.starts_with('.')) {
            continue;
        }
        let exibido = normalizar(&caminho);
        if !processar_arquivo(config, estado, Path::new(&caminho), &exibido) {
            sucesso = false;
        }
    }
    sucesso
}

fn listar(dir: &Path, prefixo: &str, seguir: bool, saida: &mut Vec<(String, bool, bool)>) {
    let Ok(ler) = std::fs::read_dir(dir) else {
        return;
    };
    for e in ler.flatten() {
        let nome = e.file_name().to_string_lossy().into_owned();
        let caminho = if prefixo.ends_with('/') {
            format!("{prefixo}{nome}")
        } else {
            format!("{prefixo}/{nome}")
        };
        let p: PathBuf = e.path();
        let e_link = p.is_symlink();
        if e_link && !seguir {
            saida.push((caminho, false, true));
            continue;
        }
        if p.is_dir() {
            saida.push((caminho.clone(), false, false));
            listar(&p, &caminho, seguir, saida);
        } else {
            saida.push((caminho, true, false));
        }
    }
}

/// `_processFile`.
fn processar_arquivo(config: &Config, estado: &mut Estado, arquivo: &Path, exibido: &str) -> bool {
    let fonte = match std::fs::read(arquivo) {
        Ok(b) => match String::from_utf8(b) {
            Ok(s) => s,
            Err(_) => {
                eprintln!("Hit a bug in the formatter when formatting {exibido}.\nPlease report at github.com/dart-lang/dart_style/issues.\nFormatException: Invalid UTF-8 byte");
                return false;
            }
        },
        Err(e) => {
            eprintln!("Hit a bug in the formatter when formatting {exibido}.\nPlease report at github.com/dart-lang/dart_style/issues.\n{e}");
            return false;
        }
    };
    processar_fonte(config, estado, Some(arquivo), exibido, &fonte)
}

fn processar_fonte(config: &Config, estado: &mut Estado, arquivo: Option<&Path>, exibido: &str, fonte: &str) -> bool {
    let resultado = std::panic::catch_unwind(|| formatar(fonte, &config.opcoes));
    let saida = match resultado {
        Ok(Ok(s)) => s,
        Ok(Err(ErroFormatacao::Sintaxe(e))) => {
            eprintln!("{}", mensagem_de_erro(fonte, exibido, &e));
            if arquivo.is_none() {
                estado.codigo = 65;
            }
            return false;
        }
        Ok(Err(e @ ErroFormatacao::SaidaInesperada { .. })) => {
            eprintln!("Hit a bug in the formatter when formatting {exibido}.\n{e}\nPlease report at github.com/dart-lang/dart_style/issues.");
            if arquivo.is_none() {
                estado.codigo = 70;
            }
            return false;
        }
        Err(_) => {
            eprintln!("Hit a bug in the formatter when formatting {exibido}.\nPlease report at github.com/dart-lang/dart_style/issues.");
            if arquivo.is_none() {
                estado.codigo = 70;
            }
            return false;
        }
    };
    let mudou = saida != fonte;
    estado.arquivos += 1;
    if mudou {
        estado.alterados += 1;
    }
    let mut sobrescrito = false;
    if mudou && config.saida == Saida::Escrever {
        sobrescrito = true;
        if let Some(a) = arquivo
            && let Err(e) = std::fs::write(a, &saida)
        {
            eprintln!(
                "Could not overwrite {exibido}: {} (error code {})",
                e,
                e.raw_os_error().unwrap_or(0)
            );
        }
    }
    let exibir = match config.mostrar {
        Mostrar::Todos => {
            if mudou {
                println!("{} {exibido}", if sobrescrito { "Formatted" } else { "Changed" });
            } else {
                println!("Unchanged {exibido}");
            }
            true
        }
        Mostrar::Alterados => {
            if mudou {
                println!("{} {exibido}", if sobrescrito { "Formatted" } else { "Changed" });
            }
            mudou
        }
        Mostrar::Nenhum => true,
    };
    if exibir {
        match config.saida {
            Saida::Mostrar => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(saida.as_bytes());
                let _ = out.flush();
            }
            Saida::Json => {
                let caminho = serde_json::to_string(exibido).unwrap_or_default();
                let texto = serde_json::to_string(&saida).unwrap_or_default();
                println!("{{\"path\":{caminho},\"source\":{texto},\"selection\":{{\"offset\":-1,\"length\":-1}}}}");
            }
            _ => {}
        }
    }
    if config.sair_se_mudou && mudou {
        estado.codigo = 1;
    }
    true
}

/// A mensagem do `FormatterException`: o trecho com linha e coluna, no
/// formato do `SourceSpan.message` do pacote `source_span`.
fn mensagem_de_erro(fonte: &str, exibido: &str, e: &dartforge_formatter::ErroSintaxe) -> String {
    let pos = e.posicao.min(fonte.len());
    let inicio_linha = fonte[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let linha = fonte[..pos].matches('\n').count() + 1;
    let coluna = fonte[inicio_linha..pos].chars().count() + 1;
    let fim_linha = fonte[pos..].find('\n').map(|i| pos + i).unwrap_or(fonte.len());
    let texto_linha = fonte[inicio_linha..fim_linha].trim_end_matches('\r');
    let largura_num = linha.to_string().len();
    let margem = " ".repeat(largura_num);
    let marca = "^".repeat(fonte[pos..(pos + e.comprimento).min(fim_linha.max(pos + 1)).min(fonte.len()).max(pos)].chars().count().max(1));
    format!(
        "Could not format because the source could not be parsed:\n\nline {linha}, column {coluna} of {exibido}: {}\n{margem} ╷\n{linha} │ {texto_linha}\n{margem} │ {}{marca}\n{margem} ╵",
        e.mensagem,
        " ".repeat(coluna - 1)
    )
}
