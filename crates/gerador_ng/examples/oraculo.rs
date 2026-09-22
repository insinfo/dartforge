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
use dartforge_elements::load::load_lenient;
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
    let listar = args.any(|a| a == "--listar");

    let cfg_path = raiz.join(".dart_tool").join("package_config.json");
    let cfg = match PackageConfig::load(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("não foi possível ler {}: {e}", cfg_path.display());
            return std::process::ExitCode::FAILURE;
        }
    };
    let oficial = do_build_runner(&cfg, &[".template.dart", ".css.shim.dart"], None);
    println!("oficial: {} arquivos gerados pelo build_runner", oficial.len());

    // Fase 1: carregar o projeto sem os gerados. A carga é tolerante, então
    // os `.template.dart` que faltam viram diagnóstico e o resto do programa
    // fica de pé — que é tudo o que o gerador precisa para resolver nomes.
    let entrada = args_entrada(&raiz);
    let sdk_dir = SdkLayout::discover()
        .unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let sdk = match SdkLayout::load(&sdk_dir, "dartdevc") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SDK indisponível ({e}); o placar sai sem resolução de nomes");
            return placar_sem_resolucao(&raiz, &cfg, &oficial);
        }
    };
    let mut nomes = Interner::new();
    let t = std::time::Instant::now();
    let (programa, _) = load_lenient(&entrada, &sdk, Some(&cfg_path), &mut nomes);
    println!(
        "programa: {} bibliotecas em {} ms",
        programa.libraries.len(),
        t.elapsed().as_millis()
    );
    let resolvedor = Resolvedor::novo(&programa, &nomes);

    let mut interner = Interner::new();
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> =
        ["lib", "web", "test"].iter().map(|d| raiz.join(d)).filter(|d| d.is_dir()).collect();
    let Some(nome) = dartforge_gerador_ng::nome_do_pacote(&raiz) else {
        eprintln!("sem `name:` no pubspec.yaml de {}", raiz.display());
        return std::process::ExitCode::FAILURE;
    };
    let pacote = Pacote { nome, raiz: raiz.clone() };
    println!("pacote: {}", pacote.nome);
    gerar_em(&pacote, &dirs, &mut interner, &mut c, &mut placar, Some(&resolvedor));
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
            Some(_) => {
                diferentes += 1;
                divergentes.push(caminho.display().to_string());
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
    println!("  pendentes: {} ({pendentes_com_oficial} com oficial)", placar.pendentes.len());
    // Quantos arquivos cada forma aparece, e quantos ela destrava sozinha.
    let mut aparece: std::collections::BTreeMap<_, usize> = Default::default();
    let mut sozinha: std::collections::BTreeMap<_, usize> = Default::default();
    for conjunto in &placar.conjuntos {
        for m in conjunto {
            *aparece.entry(*m).or_default() += 1;
        }
        if conjunto.len() == 1 {
            *sozinha.entry(*conjunto.iter().next().unwrap()).or_default() += 1;
        }
    }
    println!("  motivos (aparece / destrava sozinha):");
    let mut linhas: Vec<_> = aparece.iter().map(|(m, n)| (*n, *m)).collect();
    linhas.sort_by(|a, b| b.0.cmp(&a.0));
    for (n, m) in linhas {
        println!("    {n:4} / {:4}  {}", sozinha.get(&m).copied().unwrap_or(0), m.texto());
    }
    if !placar.nao_entendidos.is_empty() {
        println!("  formas não entendidas:");
        let mut v: Vec<_> = placar.nao_entendidos.iter().map(|(k, n)| (*n, k)).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        for (n, forma) in v.iter().take(12) {
            println!("    {n:4}  {forma}");
        }
    }
    // Os pendentes mais perto de sair: poucos motivos, e quais.
    let mut perto: Vec<(usize, String, String)> = placar
        .pendentes
        .iter()
        .zip(placar.conjuntos.iter())
        .map(|(p, c)| {
            let nome = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            let motivos: Vec<&str> = c.iter().map(|m| m.texto()).collect();
            (c.len(), nome, motivos.join(", "))
        })
        .collect();
    perto.sort();
    println!("  mais perto de sair:");
    for (n, nome, motivos) in perto.iter().take(10) {
        println!("    {n}  {nome}: {motivos}");
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
        for p in placar.pendentes.iter().take(20) {
            println!("  .. {}", p.display());
        }
    }
    if diferentes == 0 { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE }
}

/// Entrada da carga: `web/main.dart` quando existe (é por onde a aplicação
/// alcança tudo), senão a biblioteca de mesmo nome do pacote.
fn args_entrada(raiz: &std::path::Path) -> PathBuf {
    let web = raiz.join("web").join("main.dart");
    if web.is_file() {
        return web;
    }
    let nome = raiz.file_name().unwrap_or_default().to_string_lossy().to_string();
    raiz.join("lib").join(format!("{nome}.dart"))
}

/// Sem SDK não há resolução de nomes; o placar ainda vale para o resto.
fn placar_sem_resolucao(
    _raiz: &std::path::Path,
    _cfg: &PackageConfig,
    _oficial: &std::sync::Arc<dartforge_elements::gerado::Geracao>,
) -> std::process::ExitCode {
    std::process::ExitCode::FAILURE
}
