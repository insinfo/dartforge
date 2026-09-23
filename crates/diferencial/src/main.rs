//! `dartforge-diferencial [opções]` — relatório do corpus.
//! `dartforge-diferencial contrato [-o docs/CONTRATO-DDC.md]` — gera o documento do contrato.

use std::path::PathBuf;
use std::time::Duration;

use dartforge_diferencial::{
    Ambiente, IrPrograma, Limitador, Opcoes, Programa, contrato, dartforge_nativo_ir, diferencas_ir, em_paralelo,
    emitir_ir_corpus, executar_corpus, listar, relatorio, relatorio_ir,
};

const USO: &str = "uso:
  dartforge-diferencial [--nativo] [--producao] [--corpus DIR] [--filtro TEXTO] [--sem-forge] [--sem-cache] [--jobs N] [--limite SEG] [--limite-exec SEG] [--silencioso]
      roda dart run × [ddc+node ou nativo] × dartforge em cada programa e imprime o relatório
      (código 0 se todos batem; 1 se algum falha)
      --producao acrescenta o quarto executor: o perfil de produção
      (dartforge-jsprod, arquivo único e podado) — o relatório passa a comparar
      VM × nosso desenvolvimento × nossa produção (docs/JS-PRODUCAO.md)
  dartforge-diferencial contrato [--corpus DIR] [-o ARQUIVO]
      compila cada programa com o dartdevc e escreve docs/CONTRATO-DDC.md
  dartforge-diferencial verificar [--corpus DIR] [--filtro TEXTO]
      só os oráculos: cada programa tem de rodar no dart run e bater com ddc+node
  dartforge-diferencial determinismo [--nativo [--executar]] [--producao] [--filtro TEXTO] [--trabalhadores 1,4,8]
      roda o mesmo corpus com cada número de trabalhadores e exige relatório
      idêntico. Com --nativo compara o LLVM IR emitido de cada programa, sem
      Clang, ligação nem execução (DARTFORGE_IR_PARALELO_MAX limita as emissões
      simultâneas; padrão 2); --executar volta a compilar e executar tudo e
      comparar o relatório (e o IR, com DARTFORGE_KEEP_IR=1)";

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

/// Determinismo do backend nativo sem executar (PESQUISA-OTIMIZACAO §11): com
/// cada número de trabalhadores, emite o LLVM IR de todos os programas e exige
/// o mesmo resumo (ou o mesmo erro) programa a programa. O IR é a entrada da
/// chave do cache de objeto: se ele muda com a ordem de conclusão, o cache
/// erra e o Clang roda à toa. O executável é função do IR, da
/// versão do Clang e da `.lib` do runtime; o que pode mudar com a ordem de
/// conclusão é o nosso código, e ele aparece inteiro no IR. A correção do
/// executável é assunto do `--nativo` sem `determinismo` (ESTADO.md §3.2).
fn determinismo_ir(amb: &Ambiente, programas: &[Programa], trabalhadores: &[usize]) {
    let limite = Limitador::do_ambiente();
    eprintln!(
        "modo IR: só a emissão (sem Clang, ligação nem execução), no máximo {} emissão(ões) simultânea(s)",
        limite.max()
    );
    let mut referencia: Option<(usize, Vec<IrPrograma>)> = None;
    let mut primeira: Option<(String, usize)> = None;
    for n in trabalhadores {
        let inicio = std::time::Instant::now();
        let atual = emitir_ir_corpus(programas, *n, &limite);
        let seg = inicio.elapsed().as_secs_f64();
        match &referencia {
            None => {
                eprintln!("{n} trabalhador(es): referência ({} programas, {seg:.1} s)", programas.len());
                referencia = Some((*n, atual));
            }
            Some((n0, r0)) => {
                let dif = diferencas_ir(r0, &atual);
                if dif.is_empty() {
                    eprintln!("{n} trabalhador(es): idêntico a {n0} ({seg:.1} s)");
                    continue;
                }
                eprintln!("{n} trabalhador(es): LLVM IR DIFERENTE de {n0} em {} programa(s) ({seg:.1} s):", dif.len());
                for d in &dif {
                    eprintln!("  {d}");
                }
                if primeira.is_none() {
                    primeira = r0.iter().zip(&atual).find(|(x, y)| x != y).map(|(x, _)| (x.nome.clone(), *n));
                }
            }
        }
    }
    let Some((_, r0)) = referencia else { return };
    print!("{}", relatorio_ir(&r0));
    match primeira {
        None => println!("determinismo (IR): idêntico com {trabalhadores:?} trabalhadores"),
        Some((nome, n)) => {
            if let Some(p) = programas.iter().find(|p| p.nome == nome) {
                investigar_divergencia(amb, p, n, &limite);
            }
            println!("determinismo (IR): FALHOU");
            std::process::exit(1);
        }
    }
}

/// Para o primeiro programa divergente: emite-o sozinho e depois `n` vezes em
/// paralelo, grava os dois textos em `target/diferencial/determinismo/` e
/// mostra a primeira linha diferente. Se a diferença não se reproduz com o
/// programa isolado, ela depende do resto do corpus (estado compartilhado
/// entre emissões), e isso também é dito.
fn investigar_divergencia(amb: &Ambiente, programa: &Programa, n: usize, limite: &Limitador) {
    let dir = amb.trabalho.join("determinismo");
    let _ = std::fs::create_dir_all(&dir);
    let texto = |r: Result<String, String>| r.unwrap_or_else(|e| format!("ERRO: {e}\n"));
    let sozinho = texto(limite.com(|| dartforge_nativo_ir(programa)));
    let copias = vec![programa.clone(); n.max(1)];
    let concorrentes = em_paralelo(&copias, n, |p| texto(limite.com(|| dartforge_nativo_ir(p))), |_| {});
    let outro = concorrentes.iter().find(|t| **t != sozinho).unwrap_or(&concorrentes[0]);
    let (a, b) = (dir.join(format!("{}.1.ll", programa.nome)), dir.join(format!("{}.{n}.ll", programa.nome)));
    let _ = std::fs::write(&a, &sozinho);
    let _ = std::fs::write(&b, outro);
    eprintln!("{}: emitido sozinho em {} e {n}× em paralelo em {}", programa.nome, a.display(), b.display());
    if *outro == sozinho {
        eprintln!("  idênticos isolados: a diferença depende do resto do corpus");
    } else {
        imprimir_primeira_diferenca(&sozinho, outro);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut amb = Ambiente::detectar();
    let mut corpus = amb.raiz.join("corpus/js");
    let mut filtro: Option<String> = None;
    let mut op = Opcoes::default();
    let mut saida_doc = amb.raiz.join("docs/CONTRATO-DDC.md");
    let mut silencioso = false;
    let mut modo = "relatorio";
    let mut trabalhadores: Vec<usize> = vec![1, 4, 8];
    let mut executar = false;
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
            "--executar" => executar = true,
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
    let programas = listar(&corpus, filtro.as_deref());
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
        "determinismo" if op.nativo && !executar => determinismo_ir(&amb, &programas, &trabalhadores),
        "determinismo" => {
            // Sem `--nativo`, ou com `--executar`: o relatório inteiro.
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
