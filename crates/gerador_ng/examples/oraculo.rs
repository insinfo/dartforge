//! Compara o que o gerador produz com o que o compilador oficial do ngdart
//! produziu, arquivo por arquivo.
//!
//! Os `.template.dart` que o `build_runner` já escreveu em
//! `.dart_tool/build/generated` são o oráculo do gerador, como o `dartdevc` é
//! o oráculo do emissor. Enquanto o placar não fecha, o que falta continua
//! vindo do oficial e a aplicação compila do mesmo jeito.
//!
//! ```text
//! cargo run -p dartforge-gerador-ng --example oraculo -- C:/MyDartProjects/new_sali/frontend
//! ```
use dartforge_elements::config::PackageConfig;
use dartforge_elements::gerado::{Construtor, do_build_runner};
use dartforge_elements::load::load_lenient_gerados;
use dartforge_elements::sdk::SdkLayout;
use dartforge_gerador_ng::resolucao::Resolvedor;
use dartforge_gerador_ng::{Pacote, Placar, caminho_do_template, gerar_em};
use dartforge_intern::Interner;
use std::path::PathBuf;

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(raiz) = args.next().map(PathBuf::from) else {
        eprintln!("uso: oraculo <raiz do projeto> [--listar]");
        return std::process::ExitCode::FAILURE;
    };
    // Caminho absoluto: as URIs do `package_config.json` são absolutas, e
    // comparar com um caminho relativo não casa nada.
    let raiz =
        dartforge_elements::config::sem_verbatim(std::fs::canonicalize(&raiz).unwrap_or(raiz));
    let resto: Vec<String> = args.collect();
    let listar = resto.iter().any(|a| a == "--listar");
    // `--despejar <dir>`: grava o nosso e o oficial de cada diferente, para
    // comparar com um diff.
    let despejar = resto
        .iter()
        .position(|a| a == "--despejar")
        .and_then(|i| resto.get(i + 1))
        .map(PathBuf::from);

    let cfg_path = raiz.join(".dart_tool").join("package_config.json");
    let cfg = match PackageConfig::load(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("não foi possível ler {}: {e}", cfg_path.display());
            return std::process::ExitCode::FAILURE;
        }
    };
    let oficial = do_build_runner(&cfg, &[".template.dart", ".css.shim.dart"], None);
    println!(
        "oficial: {} arquivos gerados pelo build_runner",
        oficial.len()
    );

    // Fase 1: carregar o projeto sem os gerados. A carga é tolerante, então
    // os `.template.dart` que faltam viram diagnóstico e o resto do programa
    // fica de pé — que é tudo o que o gerador precisa para resolver nomes.
    let Some(nome_do_pacote) = dartforge_gerador_ng::nome_do_pacote(&raiz) else {
        eprintln!("sem `name:` no pubspec.yaml de {}", raiz.display());
        return std::process::ExitCode::FAILURE;
    };
    let (entrada, sintetica) = entrada_de_todos(&raiz, &nome_do_pacote);
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = match SdkLayout::load(&sdk_dir, "dartdevc") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SDK indisponível ({e}); o placar sai sem resolução de nomes");
            return placar_sem_resolucao(&raiz, &cfg, &oficial);
        }
    };
    let mut nomes = Interner::new();
    let t = std::time::Instant::now();
    let (programa, _) = load_lenient_gerados(
        &entrada,
        &sdk,
        Some(&cfg_path),
        &mut nomes,
        None,
        None,
        Some(sintetica),
    );
    println!(
        "programa: {} bibliotecas em {} ms",
        programa.libraries.len(),
        t.elapsed().as_millis()
    );
    let resolvedor = Resolvedor::novo(&programa, &nomes);

    let mut interner = Interner::new();
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> = ["lib", "web", "test"]
        .iter()
        .map(|d| raiz.join(d))
        .filter(|d| d.is_dir())
        .collect();
    let pacote = Pacote {
        nome: nome_do_pacote,
        raiz: raiz.clone(),
    };
    println!("pacote: {}", pacote.nome);
    gerar_em(
        &pacote,
        &dirs,
        &mut interner,
        &mut c,
        &mut placar,
        Some(&resolvedor),
    );
    let nossa = match c.concluir(1) {
        Ok(g) => g,
        Err(erros) => {
            for m in erros {
                eprintln!("erro: {m}");
            }
            return std::process::ExitCode::FAILURE;
        }
    };

    let (mut iguais, mut diferentes, mut sem_oficial) = (0usize, 0usize, 0usize);
    let mut divergentes: Vec<String> = Vec::new();
    for (caminho, f) in nossa.iter() {
        match oficial.obter(caminho) {
            None => sem_oficial += 1,
            Some(o) if o.conteudo == f.conteudo => iguais += 1,
            Some(o) => {
                diferentes += 1;
                if let Some(d) = &despejar {
                    let nome = caminho
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let _ = std::fs::create_dir_all(d);
                    let _ = std::fs::write(d.join(format!("{nome}.nosso")), f.conteudo.as_bytes());
                    let _ =
                        std::fs::write(d.join(format!("{nome}.oficial")), o.conteudo.as_bytes());
                }
                // A primeira linha que diverge diz mais que o nome do arquivo.
                let esperado = o.conteudo.replace("\r\n", "\n");
                let primeira = esperado
                    .lines()
                    .zip(f.conteudo.lines())
                    .enumerate()
                    .find(|(_, (a, b))| a != b)
                    .map(|(i, (a, b))| {
                        format!(
                            "
    linha {}
    oficial: {a}
    nosso:   {b}",
                            i + 1
                        )
                    })
                    .unwrap_or_default();
                divergentes.push(format!("{}{primeira}", caminho.display()));
            }
        }
    }
    // Pendentes com oficial: as formas que ainda faltam aprender.
    let pendentes_com_oficial = placar
        .pendentes
        .iter()
        .filter(|p| oficial.contem(&caminho_do_template(p)))
        .count();

    println!("examinados: {}", placar.examinados);
    println!("  gerados por nós: {}", placar.gerados);
    println!("    iguais ao oficial: {iguais}");
    println!("    diferentes:        {diferentes}");
    println!("    sem oficial:       {sem_oficial}");
    println!(
        "  pendentes: {} ({pendentes_com_oficial} com oficial)",
        placar.pendentes.len()
    );
    // Quantos arquivos cada motivo e cada sub-forma aparecem, e quantos cada
    // um destrava sozinho (o arquivo em que é a única recusa).
    let mut aparece: std::collections::BTreeMap<_, usize> = Default::default();
    let mut sozinha: std::collections::BTreeMap<_, usize> = Default::default();
    let mut forma_aparece: std::collections::BTreeMap<_, usize> = Default::default();
    let mut forma_sozinha: std::collections::BTreeMap<_, usize> = Default::default();
    for conjunto in &placar.conjuntos {
        let motivos: std::collections::BTreeSet<_> = conjunto.iter().map(|r| r.motivo).collect();
        for m in &motivos {
            *aparece.entry(*m).or_default() += 1;
        }
        if motivos.len() == 1 {
            *sozinha.entry(*motivos.iter().next().unwrap()).or_default() += 1;
        }
        for r in conjunto {
            *forma_aparece.entry(r.clone()).or_default() += 1;
        }
        if conjunto.len() == 1 {
            *forma_sozinha
                .entry(conjunto.iter().next().unwrap().clone())
                .or_default() += 1;
        }
    }
    println!("  motivos (aparece / destrava sozinho):");
    let mut linhas: Vec<_> = aparece.iter().map(|(m, n)| (*n, *m)).collect();
    linhas.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (n, m) in linhas {
        println!(
            "    {n:4} / {:4}  {}",
            sozinha.get(&m).copied().unwrap_or(0),
            m.texto()
        );
    }
    println!("  sub-formas (aparece / destrava sozinha):");
    let mut formas: Vec<_> = forma_aparece.iter().map(|(r, n)| (*n, r.clone())).collect();
    formas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    for (n, r) in formas {
        println!(
            "    {n:4} / {:4}  {}",
            forma_sozinha.get(&r).copied().unwrap_or(0),
            r.texto()
        );
    }
    if !placar.nao_entendidos.is_empty() {
        println!("  formas não entendidas:");
        let mut v: Vec<_> = placar.nao_entendidos.iter().map(|(k, n)| (*n, k)).collect();
        v.sort_by_key(|a| std::cmp::Reverse(a.0));
        for (n, forma) in v.iter().take(12) {
            println!("    {n:4}  {forma}");
        }
    }
    // Os pendentes mais perto de sair: poucas recusas, e quais.
    let mut perto: Vec<(usize, String, String)> = placar
        .pendentes
        .iter()
        .zip(placar.conjuntos.iter())
        .map(|(p, c)| {
            let nome = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let recusas: Vec<String> = c.iter().map(|r| r.texto()).collect();
            (c.len(), nome, recusas.join("; "))
        })
        .collect();
    perto.sort();
    println!("  mais perto de sair:");
    for (n, nome, recusas) in perto.iter().take(15) {
        println!("    {n}  {nome}: {recusas}");
    }
    // Diagnóstico da resolução: sem isto não se sabe se a injeção falha por
    // falta de forma ou porque o arquivo nem está no programa carregado.
    let mut fora_do_programa = 0usize;
    let mut sem_resolver: Vec<String> = Vec::new();
    for p in &placar.pendentes {
        if resolvedor.biblioteca(p).is_none() {
            fora_do_programa += 1;
            if sem_resolver.len() < 5 {
                sem_resolver.push(format!("fora do programa: {}", p.display()));
            }
        }
    }
    println!("  pendentes fora do programa carregado: {fora_do_programa}");
    for l in &sem_resolver {
        println!("    {l}");
    }
    if listar {
        for d in divergentes.iter().take(20) {
            println!("  != {d}");
        }
        for (p, c) in placar.pendentes.iter().zip(placar.conjuntos.iter()) {
            let recusas: Vec<String> = c.iter().map(|r| r.texto()).collect();
            println!("  .. {} :: {}", p.display(), recusas.join("; "));
        }
    }
    if diferentes == 0 {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

/// Entrada da carga que alcança **todos** os `.dart` que o gerador examina
/// (`lib/`, `web/`, `test/`), não só os que o `main` importa: uma biblioteca
/// sintética, só em memória, que importa cada um. Sem ela, um componente que
/// nenhuma página usa fica fora do banco semântico, e a lista `directives:`
/// dele não se resolve.
fn entrada_de_todos(
    raiz: &std::path::Path,
    pacote: &str,
) -> (PathBuf, std::sync::Arc<dartforge_elements::gerado::Geracao>) {
    let lib = raiz.join("lib");
    let entrada = lib.join("__oraculo_todos__.dart");
    let mut texto = String::from("// Gerado pelo oráculo: importa todo o pacote.\n");
    for dir in ["lib", "web", "test"] {
        let mut pilha = vec![raiz.join(dir)];
        let mut arquivos = Vec::new();
        while let Some(d) = pilha.pop() {
            let Ok(entradas) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() {
                    pilha.push(p);
                } else {
                    let n = p.to_string_lossy().to_string();
                    if n.ends_with(".dart") && !n.ends_with(".template.dart") {
                        arquivos.push(p);
                    }
                }
            }
        }
        arquivos.sort();
        for p in arquivos {
            let rel = p
                .strip_prefix(raiz)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            let uri = match rel.strip_prefix("lib/") {
                Some(dentro) => format!("package:{pacote}/{dentro}"),
                None => format!("../{rel}"),
            };
            texto.push_str(&format!("import '{uri}';\n"));
        }
    }
    let mut c = Construtor::nova();
    c.por(entrada.clone(), texto, "oraculo", Vec::new());
    let g = c.concluir(0).unwrap_or_default();
    (entrada, g)
}

/// Sem SDK não há resolução de nomes; o placar ainda vale para o resto.
fn placar_sem_resolucao(
    _raiz: &std::path::Path,
    _cfg: &PackageConfig,
    _oficial: &std::sync::Arc<dartforge_elements::gerado::Geracao>,
) -> std::process::ExitCode {
    std::process::ExitCode::FAILURE
}
