//! O ngdart pelo motor (estágio A) é **transparente**: a geração publicada é
//! a mesma de chamar o `gerador_ng` direto (`gerar_com_apoio`), e o que ela
//! gera confere byte a byte com o oráculo de `corpus/ngdart/oraculo/` — o
//! mesmo contrato do `crates/gerador_ng/tests/corpus.rs`, agora pelo motor.
//! Também vale "incremental = do zero" com edições de template.
//!
//! Exige `dart pub get` em `corpus/ngdart` (o `package_config.json`): por
//! isso `#[ignore]`, no grupo que o `ci.yml` satisfaz.
use dartforge_build::consulta::SemBanco;
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor};
use dartforge_elements::config::PackageConfig;
use dartforge_elements::sdk::SdkLayout;
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

fn sdk() -> SdkLayout {
    let dir = SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    SdkLayout::load(&dir, "dartdevc").expect("SDK")
}

fn programa(raiz: &Path) -> (dartforge_elements::model::Program, Interner) {
    let mut nomes = Interner::new();
    let cfg = raiz.join(".dart_tool/package_config.json");
    let (p, _) = dartforge_elements::load::load_lenient(&raiz.join("lib/corpus_ngdart.dart"), &sdk(), Some(&cfg), &mut nomes);
    (p, nomes)
}

fn motor(raiz: &Path, p: &dartforge_elements::model::Program, nomes: &Interner) -> Motor {
    let cfg = PackageConfig::load(&raiz.join(".dart_tool/package_config.json")).expect("package_config.json (dart pub get)");
    let mut m = Motor::novo(raiz, &cfg, OpcoesMotor::default()).expect("motor");
    m.atualizar(&Contexto { banco: &SemBanco, programa: Some((p, nomes)) }, &[], Demanda::Tudo).expect("atualizar");
    m
}

#[test]
#[ignore = "exige `dart pub get` em corpus/ngdart"]
fn ng_pelo_motor_igual_ao_gerador_direto_e_ao_oraculo() {
    let raiz = corpus();
    let (p, nomes) = programa(&raiz);
    let m = motor(&raiz, &p, &nomes);
    let g = m.geracao();

    // Direto, pela API pública do gerador_ng.
    let resolvedor = dartforge_gerador_ng::resolucao::Resolvedor::novo(&p, &nomes);
    let mut i = Interner::new();
    let pacote = dartforge_gerador_ng::Pacote { nome: "corpus_ngdart".into(), raiz: raiz.clone() };
    let (direta, _) = dartforge_gerador_ng::gerar_com_apoio(&pacote, &mut i, None, Some(&resolvedor));
    let esperadas: std::collections::HashSet<&PathBuf> = m.naturais.values().collect();
    let mut iguais = 0;
    for (caminho, f) in direta.iter().filter(|(_, f)| f.gerador == "ngdart") {
        let k = dartforge_elements::gerado::chave(caminho);
        if !esperadas.contains(&k) {
            continue; // fora do plano (o build_runner também não o geraria)
        }
        let pelo_motor = g.obter(&k).unwrap_or_else(|| panic!("{}: gerado direto e ausente no motor", k.display()));
        assert_eq!(pelo_motor.conteudo, f.conteudo, "{}: motor ≠ gerador direto", k.display());
        iguais += 1;
    }
    assert!(iguais > 0, "nenhuma saída nativa conferida");

    // Contra o oráculo do build_runner.
    let oraculo = raiz.join("oraculo");
    let mut conferidos = 0;
    let mut diferentes = Vec::new();
    for (caminho, f) in g.iter().filter(|(_, f)| f.gerador == "ngdart:ngdart") {
        let Some(nome) = caminho.file_name() else { continue };
        let Ok(esperado) = std::fs::read_to_string(oraculo.join(nome)) else { continue };
        conferidos += 1;
        if esperado.replace("\r\n", "\n") != *f.conteudo {
            diferentes.push(caminho.display().to_string());
        }
    }
    println!("ngdart pelo motor: {iguais} iguais ao gerador direto; {conferidos} conferidos com o oráculo, {} diferentes", diferentes.len());
    assert!(diferentes.is_empty(), "diferentes do oráculo: {diferentes:?}");
    assert!(conferidos >= 2);
}

fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let (p, nome) = (e.path(), e.file_name());
        let alvo = para.join(&nome);
        if p.is_dir() {
            if nome == ".dart_tool" {
                std::fs::create_dir_all(&alvo).unwrap();
                std::fs::copy(p.join("package_config.json"), alvo.join("package_config.json")).unwrap();
            } else if nome != "oraculo" && nome != "build" {
                copiar(&p, &alvo);
            }
        } else {
            std::fs::copy(&p, &alvo).unwrap();
        }
    }
}

#[test]
#[ignore = "exige `dart pub get` em corpus/ngdart"]
fn ng_incremental_igual_ao_do_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("corpus_ngdart");
    copiar(&corpus(), &raiz);
    let (p, nomes) = programa(&raiz);
    let mut vivo = motor(&raiz, &p, &nomes);
    drop((p, nomes));
    // Edições: texto num template, estilo, arquivo novo.
    let htmls: Vec<PathBuf> = std::fs::read_dir(raiz.join("lib/src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "html"))
        .take(2)
        .collect();
    let mut passos: Vec<(PathBuf, String)> = htmls.iter().map(|h| (h.clone(), "\n<span>editado</span>\n".to_string())).collect();
    passos.push((raiz.join("lib/src/b07_estilo.css"), "\n.b07-editado { color: red; }\n".to_string()));
    passos.push((raiz.join("lib/src/z_novo.dart"), "class ZNovo {}\n".to_string()));
    for (arq, texto) in passos {
        let mut atual = std::fs::read_to_string(&arq).unwrap_or_default();
        atual.push_str(&texto);
        std::fs::write(&arq, &atual).unwrap();
        let (p, nomes) = programa(&raiz);
        let atual = vivo.atualizar(&Contexto { banco: &SemBanco, programa: Some((&p, &nomes)) }, &[arq.clone()], Demanda::Tudo)
            .expect("atualizar");
        if arq.extension().is_some_and(|e| e == "html" || e == "css") {
            assert_eq!(atual.rel.unidades_nativas, 1, "a edição de um recurso deve regenerar só seu componente");
            assert_eq!(atual.rel.consultas_gerador, 1, "só o digest do recurso mudado deve ser registrado novamente");
        }
        let novo = motor(&raiz, &p, &nomes);
        assert_eq!(vivo.estado_canonico(), novo.estado_canonico(), "incremental ≠ do zero depois de {}", arq.display());
    }
}
