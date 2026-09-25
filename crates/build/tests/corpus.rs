//! Harness do corpus de compatibilidade (`corpus/builders/`, oráculo do
//! `build_runner` oficial). Pré-requisito: `dart pub get --enforce-lockfile`
//! em cada caso (o `package_config.json`), por isso `#[ignore]`: o `ci.yml`
//! roda o `pub get` e depois `--ignored`.
//!
//! * **plano**: forma canônica do motor = a do `oraculo/plano.dart`;
//! * **saídas**: cada saída do manifesto cai em igual / pendente (com
//!   motivo) / diferente; critério: `diferentes == 0`;
//! * **determinismo**: 1, 4 e 8 trabalhadores dão o mesmo estado;
//! * **incremental = do zero**: aplicadas as `edicoes/` em sequência, o motor
//!   vivo e um motor novo dão o mesmo estado.
use dartforge_build::consulta::SemBanco;
use dartforge_build::grafo::AssetId;
use dartforge_build::executor::{Disponibilidade, ErroExecutor, ExecutorDart, PedidoAcao, ResultadoAcao, ScriptDeBuilders, ServicoBuildStep};
use dartforge_build::motor::Origem;
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};

fn raiz_do_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders")
}

fn casos() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(raiz_do_corpus())
        .expect("corpus/builders")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("oraculo/manifesto.json").is_file())
        .collect();
    v.sort();
    v
}

fn cfg_de(dir: &Path) -> Option<PackageConfig> {
    PackageConfig::load(&dir.join(".dart_tool/package_config.json")).ok()
}

/// Saídas do manifesto com os bytes do oráculo.
fn referencias(dir: &Path) -> BTreeMap<AssetId, Vec<u8>> {
    let texto = std::fs::read_to_string(dir.join("oraculo/manifesto.json")).expect("manifesto");
    let m: serde_json::Value = serde_json::from_str(&texto).expect("manifesto JSON");
    let mut r = BTreeMap::new();
    for s in m["saidas"].as_array().into_iter().flatten() {
        let asset = s["asset"].as_str().expect("asset");
        let id = AssetId::de_texto(asset).expect("pacote|caminho");
        let arq = match s["build_to"].as_str() {
            Some("source") => dir.join("oraculo/source").join(id.caminho.as_ref()),
            _ => dir.join("oraculo/cache").join(id.pacote.as_ref()).join(id.caminho.as_ref()),
        };
        r.insert(id, std::fs::read(&arq).unwrap_or_else(|e| panic!("{}: {e}", arq.display())));
    }
    r
}

fn motor(dir: &Path, trabalhadores: usize) -> Result<Motor, String> {
    let cfg = cfg_de(dir).ok_or("sem package_config.json (rode dart pub get)")?;
    let opcoes = OpcoesMotor { trabalhadores, medir_nao_verificados: true, ..Default::default() };
    let mut m = Motor::novo(dir, &cfg, opcoes)?;
    m.atualizar(&Contexto { banco: &SemBanco, programa: None }, &[], Demanda::Tudo)?;
    Ok(m)
}

struct DartFalso { preparos: Arc<AtomicUsize>, chamadas: Arc<AtomicUsize>, fechamentos: Arc<AtomicUsize> }

impl ExecutorDart for DartFalso {
    fn disponibilidade(&self) -> Disponibilidade { Disponibilidade::Disponivel }
    fn preparar(&mut self, _: &ScriptDeBuilders) -> Result<(), ErroExecutor> {
        self.preparos.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn executar(&mut self, p: &PedidoAcao, s: &mut dyn ServicoBuildStep) -> Result<ResultadoAcao, ErroExecutor> {
        self.chamadas.fetch_add(1, Ordering::SeqCst);
        let _ = s.ler(&p.entrada);
        for id in &p.saidas_permitidas {
            let bytes: Arc<[u8]> = format!("{}:{}\n", p.chave, p.entrada.texto()).into_bytes().into();
            s.escrever(id, bytes).map_err(|e| ErroExecutor(format!("saída recusada: {}", e.0.texto())))?;
        }
        Ok(ResultadoAcao::default())
    }
    fn encerrar(&mut self) { self.fechamentos.fetch_add(1, Ordering::SeqCst); }
}

#[test]
#[ignore = "exige `dart pub get` em corpus/builders/cadeia_configuracao"]
fn executor_dart_injetado_roda_e_reusa_acoes() {
    let dir = raiz_do_corpus().join("cadeia_configuracao");
    let cfg = cfg_de(&dir).expect("package_config.json");
    let mut m = Motor::novo(&dir, &cfg, OpcoesMotor::default()).unwrap();
    let preparos = Arc::new(AtomicUsize::new(0));
    let chamadas = Arc::new(AtomicUsize::new(0));
    let fechamentos = Arc::new(AtomicUsize::new(0));
    m.definir_executor_dart(Box::new(DartFalso { preparos: preparos.clone(), chamadas: chamadas.clone(), fechamentos: fechamentos.clone() }));
    let ctx = Contexto { banco: &SemBanco, programa: None };
    m.atualizar(&ctx, &[], Demanda::Tudo).unwrap();
    let feitas = chamadas.load(Ordering::SeqCst);
    assert!(feitas > 0, "o executor Dart disponível não recebeu nenhuma ação");
    assert!(m.grafo.acoes.iter().enumerate().any(|(i, _)| m.registro(i).is_some_and(|r| r.origem == Origem::Dart)));
    m.atualizar(&ctx, &[], Demanda::Tudo).unwrap();
    assert_eq!(chamadas.load(Ordering::SeqCst), feitas, "ação limpa foi reexecutada");
    assert_eq!(preparos.load(Ordering::SeqCst), 1, "script de builders recompilado na sessão");
    drop(m);
    assert_eq!(fechamentos.load(Ordering::SeqCst), 1, "executor não foi encerrado ao fim da sessão");
}

fn resumo(linhas: &[String]) {
    let texto = linhas.join("\n");
    println!("{texto}");
    if let Ok(arq) = std::env::var("GITHUB_STEP_SUMMARY") {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(arq) {
            let _ = writeln!(f, "{texto}\n");
        }
    }
}

#[test]
#[ignore = "exige `dart pub get` em cada caso de corpus/builders"]
fn corpus_builders_plano_e_placar() {
    let mut linhas = vec!["### corpus/builders — plano e placar (iguais/pendentes/diferentes)".to_string(), String::new()];
    linhas.push("| caso | plano | iguais | pendentes | diferentes | motivos |".into());
    linhas.push("|---|---|---|---|---|---|".into());
    let mut falhas = Vec::new();
    for dir in casos() {
        let nome = dir.file_name().unwrap().to_string_lossy().to_string();
        let Some(cfg) = cfg_de(&dir) else {
            falhas.push(format!("{nome}: sem package_config.json (dart pub get)"));
            continue;
        };
        if !dartforge_build::detectar(&cfg) {
            // Controle do custo zero: sem build_runner, nada de motor.
            assert!(!dir.join("oraculo/plano.dart").exists(), "{nome}: sem build_runner mas com plano.dart");
            linhas.push(format!("| {nome} | sem builders (motor não construído) | 0 | 0 | 0 | |"));
            continue;
        }
        let plano = match std::fs::read_to_string(dir.join("oraculo/plano.dart")) {
            Ok(texto) => {
                let (_, _, p) = dartforge_build::plano_do_projeto(&dir, &cfg).expect("plano");
                match dartforge_build::oraculo::comparar(&p.aplicacoes, &texto).expect("plano.dart legível") {
                    Ok(()) => format!("igual ({})", p.aplicacoes.len()),
                    Err(d) => {
                        falhas.push(format!("{nome}: plano diferente:\n{}", d.join("\n")));
                        "DIFERENTE".into()
                    }
                }
            }
            Err(_) => "-".into(),
        };
        let m = match motor(&dir, 4) {
            Ok(m) => m,
            Err(e) => {
                falhas.push(format!("{nome}: motor: {e}"));
                continue;
            }
        };
        let p = m.placar(&referencias(&dir));
        let motivos: Vec<String> = p.motivos().into_iter().map(|(k, n)| format!("{n}× {k}")).collect();
        linhas.push(format!(
            "| {nome} | {plano} | {} | {} | {} | {} |",
            p.iguais.len(),
            p.pendentes.len(),
            p.diferentes.len(),
            motivos.join("; ")
        ));
        for (id, m) in &p.diferentes {
            falhas.push(format!("{nome}: diferente {}: {m}", id.texto()));
        }
        for (id, m) in &p.pendentes {
            if m.starts_with("pós-processador") {
                println!("{nome}: {} ({m})", id.texto());
            }
        }
    }
    resumo(&linhas);
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
}

#[test]
#[ignore = "exige `dart pub get` em cada caso de corpus/builders"]
fn corpus_builders_determinismo() {
    for dir in casos() {
        let Some(cfg) = cfg_de(&dir) else { continue };
        if !dartforge_build::detectar(&cfg) {
            continue;
        }
        let estados: Vec<String> =
            [1, 4, 8].iter().map(|&n| motor(&dir, n).expect("motor").estado_canonico()).collect();
        assert_eq!(estados[0], estados[1], "{}: 1 ≠ 4 trabalhadores", dir.display());
        assert_eq!(estados[0], estados[2], "{}: 1 ≠ 8 trabalhadores", dir.display());
    }
}

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

#[test]
#[ignore = "exige `dart pub get` em corpus/builders/sass_builder_compressed"]
fn sass_release_css_sob_demanda_sem_apoio() {
    let origem = raiz_do_corpus().join("sass_builder_compressed");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("sass_builder_compressed");
    copiar(&origem, &dir);
    // Sem o apoio do build_runner: a resposta só pode vir do nativo.
    let scss = dir.join("web/principal.scss");
    std::fs::write(&scss, ".a { color: red; }\n").unwrap();
    let cfg = cfg_de(&dir).expect("package_config.json");
    let mut m = Motor::novo(&dir, &cfg, OpcoesMotor { release: true, ..Default::default() }).unwrap();
    let ctx = Contexto { banco: &SemBanco, programa: None };
    m.atualizar(&ctx, &[], Demanda::Carregador).unwrap();
    let destino = dartforge_elements::gerado::chave(&dir.join("web/principal.css"));
    assert!(
        m.naturais.keys().any(|id| id.caminho.as_ref() == "web/principal.css"),
        "CSS fora do grafo: {destino:?}"
    );
    let css = m.materializar(&ctx, &destino).unwrap_or_else(|| {
        let estado_completo = m.estado_canonico();
        let estado: Vec<_> = estado_completo.lines().filter(|l| l.contains("principal.scss")).collect();
        panic!("CSS nativo sob demanda: {estado:?}")
    });
    assert_eq!(&*css, b".a{color:red}\n");
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

#[test]
#[ignore = "exige `dart pub get` em cada caso de corpus/builders"]
fn corpus_builders_incremental_igual_ao_do_zero() {
    let mut feitos = 0;
    for dir in casos() {
        let edicoes = dir.join("edicoes");
        if !edicoes.is_dir() || cfg_de(&dir).is_none() {
            continue;
        }
        let tmp = tempfile::tempdir().unwrap();
        let copia = tmp.path().join(dir.file_name().unwrap());
        copiar(&dir, &copia);
        let mut vivo = motor(&copia, 4).expect("motor vivo");
        let mut passos: Vec<PathBuf> = std::fs::read_dir(&edicoes).unwrap().flatten().map(|e| e.path()).collect();
        passos.sort();
        for passo in passos {
            let mut rel = Vec::new();
            arquivos(&passo, &passo, &mut rel);
            let mut mudados = Vec::new();
            for r in rel {
                let alvo = copia.join(&r);
                std::fs::create_dir_all(alvo.parent().unwrap()).unwrap();
                std::fs::copy(passo.join(&r), &alvo).unwrap();
                mudados.push(alvo);
            }
            vivo.atualizar(&Contexto { banco: &SemBanco, programa: None }, &mudados, Demanda::Tudo).expect("atualizar");
            let novo = motor(&copia, 4).expect("motor novo");
            assert_eq!(
                vivo.estado_canonico(),
                novo.estado_canonico(),
                "{}: incremental ≠ do zero depois de {}",
                dir.display(),
                passo.display()
            );
            feitos += 1;
        }
    }
    println!("{feitos} edições verificadas");
}
