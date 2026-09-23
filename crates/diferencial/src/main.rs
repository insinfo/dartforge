//! `dartforge-diferencial [opções]` — relatório do corpus.
//! `dartforge-diferencial contrato [-o docs/CONTRATO-DDC.md]` — gera o documento do contrato.

use std::path::PathBuf;
use std::time::Duration;

use dartforge_diferencial::{Ambiente, Opcoes, contrato, corpus, executar_corpus, listar, relatorio};

const USO: &str = "uso:
  dartforge-diferencial [--nativo] [--producao] [--corpus DIR] [--filtro TEXTO] [--sem-forge] [--sem-cache] [--jobs N] [--limite SEG] [--limite-exec SEG] [--silencioso]
      roda dart run × [ddc+node ou nativo] × dartforge em cada programa e imprime o relatório
      (código 0 se todos batem; 1 se algum falha)
      --fragmento K/N roda só o fragmento K de N do corpus (índice % N == K-1;
      vale em todos os modos — é como o CI divide o corpus nativo entre máquinas)
      --producao acrescenta o quarto executor: o perfil de produção
      (dartforge-jsprod, arquivo único e podado) — o relatório passa a comparar
      VM × nosso desenvolvimento × nossa produção (docs/JS-PRODUCAO.md)
  dartforge-diferencial contrato [--corpus DIR] [-o ARQUIVO]
      compila cada programa com o dartdevc e escreve docs/CONTRATO-DDC.md
  dartforge-diferencial verificar [--corpus DIR] [--filtro TEXTO]
      só os oráculos: cada programa tem de rodar no dart run e bater com ddc+node
  dartforge-diferencial determinismo [--nativo] [--producao] [--filtro TEXTO] [--trabalhadores 1,4,8]
      roda o mesmo corpus com cada número de trabalhadores e exige relatório
      idêntico (e, no modo nativo, o mesmo LLVM IR emitido)";

/// Resumo de todo o LLVM IR emitido, para comparar execuções.
///
/// O `.ll` de cada programa fica em `<saída>/.df_tmp/*.ll` quando
/// `DARTFORGE_KEEP_IR` está definido. Ordena por caminho antes de somar, senão
/// a ordem de leitura do diretório entraria no resumo e o teste acusaria
/// diferença onde não há.
fn digest_ir(raiz: &std::path::Path) -> u64 {
    let mut arquivos: Vec<PathBuf> = Vec::new();
    fn juntar(dir: &std::path::Path, acc: &mut Vec<PathBuf>) {
        let Ok(entradas) = std::fs::read_dir(dir) else { return };
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                juntar(&p, acc);
            } else if p.extension().is_some_and(|x| x == "ll") {
                acc.push(p);
            }
        }
    }
    juntar(raiz, &mut arquivos);
    arquivos.sort();
    let mut h: u64 = 0xcbf29ce484222325;
    for a in &arquivos {
        let nome = a.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let conteudo = std::fs::read(a).unwrap_or_default();
        for b in nome.as_bytes().iter().chain(conteudo.iter()) {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

fn imprimir_primeira_diferenca(a: &str, b: &str) {
    let mut la = a.lines();
    let mut lb = b.lines();
    let mut n = 1;
    loop {
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => n += 1,
            (x, y) => {
                eprintln!("  linha {n}:");
                eprintln!("    a: {}", x.unwrap_or("(fim)"));
                eprintln!("    b: {}", y.unwrap_or("(fim)"));
                return;
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut amb = Ambiente::detectar();
    let mut corpus = amb.raiz.join("corpus/js");
    let mut filtro: Option<String> = None;
    let mut fragmento: Option<(usize, usize)> = None;
    let mut op = Opcoes::default();
    let mut saida_doc = amb.raiz.join("docs/CONTRATO-DDC.md");
    let mut silencioso = false;
    let mut modo = "relatorio";
    let mut trabalhadores: Vec<usize> = vec![1, 4, 8];
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "contrato" | "verificar" | "determinismo" => modo = match args[i].as_str() {
                "contrato" => "contrato",
                "verificar" => "verificar",
                _ => "determinismo",
            },
            "--trabalhadores" => {
                i += 1;
                trabalhadores = args[i]
                    .split(',')
                    .map(|n| n.trim().parse().expect("--trabalhadores 1,4,8"))
                    .collect();
            }
            "--corpus" => {
                i += 1;
                corpus = PathBuf::from(&args[i]);
            }
            "--filtro" => {
                i += 1;
                filtro = Some(args[i].clone());
            }
            "--fragmento" => {
                i += 1;
                fragmento = Some(corpus::ler_fragmento(&args[i]).unwrap_or_else(|e| {
                    eprintln!("{e}");
                    std::process::exit(2)
                }));
            }
            "-o" => {
                i += 1;
                saida_doc = PathBuf::from(&args[i]);
            }
            "--jobs" => {
                i += 1;
                op.threads = args[i].parse().expect("--jobs N");
            }
            "--limite" => {
                i += 1;
                amb.limite = Duration::from_secs(args[i].parse().expect("--limite SEG"));
            }
            "--limite-exec" => {
                i += 1;
                amb.limite_nativo =
                    Duration::from_secs(args[i].parse().expect("--limite-exec SEG"));
            }
            "--sem-forge" => op.com_forge = false,
            "--nativo" => op.nativo = true,
            "--producao" => op.com_producao = true,
            "--sem-cache" => amb.usar_cache = false,
            "--silencioso" => silencioso = true,
            "-h" | "--help" => {
                println!("{USO}");
                return;
            }
            outro => {
                eprintln!("argumento desconhecido: {outro}\n{USO}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    // No modo nativo cada programa vira um processo que aloca no heap proprio;
    // seis em paralelo ja tomaram a memoria da maquina inteira. Enquanto o
    // backend nativo nao esta estavel, o padrao e dois — quem quiser mais passa
    // `--jobs` explicitamente e assume o risco.
    // O mesmo vale para o perfil de produção do JS: cada programa vira um
    // processo `dartforge-jsprod`, que carrega o SDK e classifica os 7 MB do
    // `dart_sdk.js`. A máquina tem 7,7 GB e é compartilhada.
    if (op.nativo || op.com_producao) && op.threads == 0 {
        op.threads = 2;
    }
    let mut programas = listar(&corpus, filtro.as_deref());
    if let Some((k, n)) = fragmento {
        programas = corpus::fragmento(programas, k, n);
        eprintln!("fragmento {k}/{n}: {} programas", programas.len());
    }
    if programas.is_empty() {
        eprintln!("nenhum programa em {}", corpus.display());
        std::process::exit(2);
    }
    match modo {
        "contrato" => {
            let doc = contrato::gerar(&amb, &programas);
            std::fs::write(&saida_doc, doc).expect("escrever o documento");
            println!("{} programas → {}", programas.len(), saida_doc.display());
        }
        "determinismo" => {
            // O teste de determinismo do mold (PESQUISA-OTIMIZACAO §11): mesma
            // entrada e mesma configuração têm de dar a mesma saída com
            // qualquer número de trabalhadores. Aqui isso vale para o relatório
            // e, no modo nativo, para o LLVM IR emitido — se a ordem de
            // conclusão mudar o IR, o cache de objeto por hash erra e o Clang
            // roda à toa, que é justamente o que domina o tempo de compilação.
            // O `.ll` so sobrevive com DARTFORGE_KEEP_IR definido, e a variavel
            // tem de vir do ambiente: `set_var` e `unsafe` desde a edicao 2024
            // (outra thread pode estar lendo o ambiente) e o workspace nega
            // `unsafe_code`. Sem ela, a comparacao do IR e pulada e dito aqui.
            let comparar_ir = op.nativo && std::env::var_os("DARTFORGE_KEEP_IR").is_some();
            if op.nativo && !comparar_ir {
                eprintln!("aviso: defina DARTFORGE_KEEP_IR=1 para comparar tambem o LLVM IR emitido");
            }
            let mut referencia: Option<(usize, String, u64)> = None;
            let mut igual = true;
            for n in &trabalhadores {
                op.threads = *n;
                let resultados = executar_corpus(&amb, &programas, op, |_| {});
                let texto = relatorio(&resultados);
                let ir = if comparar_ir { digest_ir(&amb.trabalho.join("nativo")) } else { 0 };
                match &referencia {
                    None => {
                        eprintln!("{n} trabalhador(es): referência ({} programas)", programas.len());
                        referencia = Some((*n, texto, ir));
                    }
                    Some((n0, t0, ir0)) => {
                        let rel_ok = *t0 == texto;
                        let ir_ok = *ir0 == ir;
                        if rel_ok && ir_ok {
                            eprintln!("{n} trabalhador(es): idêntico a {n0}");
                        } else {
                            igual = false;
                            if !rel_ok {
                                eprintln!("{n} trabalhador(es): RELATÓRIO DIFERENTE de {n0}");
                                imprimir_primeira_diferenca(t0, &texto);
                            }
                            if !ir_ok {
                                eprintln!("{n} trabalhador(es): LLVM IR DIFERENTE de {n0} ({ir0:016x} != {ir:016x})");
                            }
                        }
                    }
                }
            }
            if igual {
                println!("determinismo: saída idêntica com {trabalhadores:?} trabalhadores");
            } else {
                println!("determinismo: FALHOU");
                std::process::exit(1);
            }
        }
        _ => {
            if modo == "verificar" {
                op.com_forge = false;
            }
            if op.com_forge {
                match &amb.dartforge_bin {
                    Some(b) => eprintln!("dartforge: {}", b.display()),
                    None => eprintln!("dartforge: binário não encontrado; usando `cargo run -p dartforge-cli`"),
                }
            }
            let inicio = std::time::Instant::now();
            let resultados = executar_corpus(&amb, &programas, op, |r| {
                if silencioso {
                    return;
                }
                let estado = if r.producao.is_some() {
                    if r.ok() { "ok" } else if r.forge_vs_referencia().is_some() { "FALHA" } else { "PROD!" }
                } else if r.forge.is_some() {
                    if r.ok() { "ok" } else { "FALHA" }
                } else if r.dart.codigo != 0 {
                    "DART!"
                } else if r.nativo {
                    "DART"
                } else if r.ddc_vs_dart().is_none() == r.programa.diverge_ddc.is_none() {
                    "ok"
                } else {
                    "DDC≠VM"
                };
                eprintln!("  {estado:<6} {}", r.programa.nome);
            });
            print!("{}", relatorio(&resultados));
            println!("({} programas em {:.1} s)", resultados.len(), inicio.elapsed().as_secs_f64());
            let todos_ok = if op.com_forge {
                resultados.iter().all(|r| r.ok())
            } else if op.nativo {
                resultados.iter().all(|r| r.dart.codigo == 0)
            } else {
                resultados.iter().all(|r| r.dart.codigo == 0 && (r.ddc_vs_dart().is_none() != r.programa.diverge_ddc.is_some()))
            };
            if !todos_ok {
                std::process::exit(1);
            }
        }
    }
}
