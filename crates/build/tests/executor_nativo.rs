//! O executor de builders **nativo** (`crates/build/src/executor_nativo.rs`,
//! B01) contra o oráculo do `build_runner` (`corpus/builders/`): o bootstrap
//! do `pacotes/build_executor` compilado pelo `compile-native` do próprio
//! DartForge, sem VM, sem `dart` e sem o apoio do `build_runner` no disco. A
//! régua é a do `executor_vm.rs`: saída byte a byte a do oráculo e
//! incremental igual ao do zero em cada edição do caso.
//!
//! Pré-requisitos, por isso `#[ignore]`: um `dartforge` com a feature
//! `nativo` (`DARTFORGE_COMPILADOR`, ou `target/release/dartforge`), o SDK
//! (`DARTFORGE_SDK_LIB`/`DART_SDK`, como o `compile-native`) e o
//! `package_config.json` de cada caso (`dart pub get --enforce-lockfile`).
//! Cada caso compila o seu executável (minutos: o `analyzer` inteiro), que é
//! apagado ao fim do caso — cabem no disco um de cada vez.
//! `DARTFORGE_CORPUS_BUILDERS` troca a raiz do corpus; `DARTFORGE_CASOS`
//! (nomes separados por vírgula) limita os casos.
use dartforge_build::consulta::SemBanco;
use dartforge_build::executor_nativo::{ConfigNativa, ExecutorNativo};
use dartforge_build::grafo::AssetId;
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn raiz_do_corpus() -> PathBuf {
    std::env::var_os("DARTFORGE_CORPUS_BUILDERS")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders"))
}

fn compilador() -> PathBuf {
    std::env::var_os("DARTFORGE_COMPILADOR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/release/dartforge")
                .with_extension(std::env::consts::EXE_EXTENSION)
        })
}

/// Saídas do manifesto com os bytes do oráculo.
fn referencias(dir: &Path) -> BTreeMap<AssetId, Vec<u8>> {
    let texto = std::fs::read_to_string(dir.join("oraculo/manifesto.json")).expect("manifesto");
    let m: serde_json::Value = serde_json::from_str(&texto).expect("manifesto JSON");
    let mut r = BTreeMap::new();
    for s in m["saidas"].as_array().into_iter().flatten() {
        let id = AssetId::de_texto(s["asset"].as_str().expect("asset")).expect("pacote|caminho");
        let arq = match s["build_to"].as_str() {
            Some("source") => dir.join("oraculo/source").join(id.caminho.as_ref()),
            _ => dir
                .join("oraculo/cache")
                .join(id.pacote.as_ref())
                .join(id.caminho.as_ref()),
        };
        r.insert(id, std::fs::read(&arq).unwrap_or_else(|e| panic!("{}: {e}", arq.display())));
    }
    r
}

/// Cópia do caso sem o oráculo, as edições, o `build/` e o `.dart_tool`
/// (só o `package_config.json`).
fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let p = e.path();
        let nome = e.file_name();
        if nome == "oraculo" || nome == "edicoes" || nome == "build" {
            continue;
        }
        let alvo = para.join(&nome);
        if p.is_dir() {
            if nome == ".dart_tool" {
                std::fs::create_dir_all(&alvo).unwrap();
                std::fs::copy(p.join("package_config.json"), alvo.join("package_config.json")).unwrap();
                continue;
            }
            copiar(&p, &alvo);
        } else {
            std::fs::copy(&p, &alvo).unwrap();
        }
    }
}

fn arquivos(dir: &Path, base: &Path, v: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            arquivos(&p, base, v);
        } else {
            v.push(p.strip_prefix(base).unwrap().to_path_buf());
        }
    }
}

/// Aplica uma edição do corpus e devolve os arquivos mudados.
fn aplicar(passo: &Path, copia: &Path) -> Vec<PathBuf> {
    let mut rel = Vec::new();
    arquivos(passo, passo, &mut rel);
    rel.into_iter()
        .map(|r| {
            let alvo = copia.join(&r);
            std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
            std::fs::copy(passo.join(&r), &alvo).unwrap();
            alvo
        })
        .collect()
}

fn motor_nativo(dir: &Path) -> Motor {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Motor::novo(dir, &cfg, OpcoesMotor::default()).expect("motor");
    let sdk = std::env::var_os("DARTFORGE_SDK_LIB").map(PathBuf::from);
    m.definir_executor_dart(Box::new(ExecutorNativo::novo(ConfigNativa::do_projeto(
        compilador(),
        sdk,
        dir,
    ))));
    m
}

fn atualizar(m: &mut Motor, mudados: &[PathBuf]) -> (RelMotor, f64) {
    let t0 = Instant::now();
    let at = m
        .atualizar(&Contexto { banco: &SemBanco, programa: None }, mudados, Demanda::Tudo)
        .expect("atualizar");
    for a in &at.avisos {
        println!("aviso: {a}");
    }
    (at.rel, t0.elapsed().as_secs_f64() * 1000.0)
}

/// O placar de todos os casos pelo executor nativo: zero diferentes e
/// incremental = do zero em cada edição.
#[test]
#[ignore = "exige o dartforge com a feature nativo e `dart pub get` em cada caso de corpus/builders"]
fn corpus_builders_pelo_executor_nativo() {
    assert!(compilador().is_file(), "compilador {} não existe", compilador().display());
    let filtro: Option<Vec<String>> = std::env::var("DARTFORGE_CASOS")
        .ok()
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let mut casos: Vec<PathBuf> = std::fs::read_dir(raiz_do_corpus())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("oraculo/plano.dart").is_file() && p.join(".dart_tool/package_config.json").is_file())
        .filter(|p| {
            filtro.as_ref().is_none_or(|f| f.iter().any(|n| p.file_name().is_some_and(|x| x == n.as_str())))
        })
        .collect();
    casos.sort();
    let mut linhas = vec![
        "### corpus/builders pelo executor nativo — iguais/pendentes/diferentes".to_string(),
        String::new(),
        "| caso | iguais | pendentes | diferentes | limpo (ms) | sem mudança (ms) | motivos |".into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    let mut diferentes = Vec::new();
    for origem in casos {
        let nome = origem.file_name().unwrap().to_string_lossy().to_string();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(&nome);
        copiar(&origem, &dir);
        let mut m = motor_nativo(&dir);
        let (_, limpo) = atualizar(&mut m, &[]);
        let (_, quente) = atualizar(&mut m, &[]);
        let p = m.placar(&referencias(&origem));
        let motivos: Vec<String> = p
            .motivos()
            .into_iter()
            .map(|(k, n)| format!("{n}× {}", k.lines().next().unwrap_or("")))
            .collect();
        linhas.push(format!(
            "| {nome} | {} | {} | {} | {limpo:.0} | {quente:.1} | {} |",
            p.iguais.len(),
            p.pendentes.len(),
            p.diferentes.len(),
            motivos.join("; ")
        ));
        println!("{}", linhas.last().unwrap());
        for (id, motivo) in &p.diferentes {
            diferentes.push(format!("{nome}: {} ({motivo})", id.texto()));
        }
        if !p.pendentes.is_empty() {
            diferentes.push(format!("{nome}: {} pendentes ({})", p.pendentes.len(), motivos.join("; ")));
        }
        let mut passos: Vec<PathBuf> = std::fs::read_dir(origem.join("edicoes"))
            .map(|l| l.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        passos.sort();
        for passo in passos {
            let mudados = aplicar(&passo, &dir);
            let (rel, ms) = atualizar(&mut m, &mudados);
            // Motor novo, processo novo: o executável vem do cache.
            let mut novo = motor_nativo(&dir);
            atualizar(&mut novo, &[]);
            let nome_passo = passo.file_name().unwrap().to_string_lossy().to_string();
            println!("{nome}/{nome_passo}: {ms:.0} ms, {} ações executadas", rel.acoes_executadas);
            if m.estado_canonico() != novo.estado_canonico() {
                diferentes.push(format!("{nome}: incremental ≠ do zero depois de {nome_passo}"));
            }
        }
    }
    println!("{}", linhas.join("\n"));
    if let Ok(arq) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(arq) {
            let _ = writeln!(f, "{}\n", linhas.join("\n"));
        }
    }
    assert!(diferentes.is_empty(), "{}", diferentes.join("\n"));
}
