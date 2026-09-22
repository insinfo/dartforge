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
    let oficial = do_build_runner(&cfg, ".template.dart", None);
    println!("oficial: {} arquivos gerados pelo build_runner", oficial.len());

    let mut interner = Interner::new();
    let mut c = Construtor::nova();
    let mut placar = Placar::default();
    let dirs: Vec<PathBuf> =
        ["lib", "web", "test"].iter().map(|d| raiz.join(d)).filter(|d| d.is_dir()).collect();
    let nome = cfg
        .packages
        .iter()
        .find(|(_, p)| {
            p.root_uri
                .to_file_path()
                .is_ok_and(|d| dartforge_elements::config::sem_verbatim(d) == raiz)
        })
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| raiz.file_name().unwrap_or_default().to_string_lossy().to_string());
    let pacote = Pacote { nome, raiz: raiz.clone() };
    println!("pacote: {}", pacote.nome);
    gerar_em(&pacote, &dirs, &mut interner, &mut c, &mut placar);
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
