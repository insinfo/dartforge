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
    // Edições: texto num template, estilo, `.dart` de componente (corpo,
    // `@Input` novo num filho, seletor alterado, import novo) e arquivo
    // novo. Cada uma é `(arquivo, trecho a trocar ou "" para acrescentar,
    // texto novo)`.
    let htmls: Vec<PathBuf> = std::fs::read_dir(raiz.join("lib/src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "html"))
        .take(2)
        .collect();
    let src = raiz.join("lib/src");
    let mut passos: Vec<(PathBuf, &str, &str)> = htmls.iter().map(|h| (h.clone(), "", "\n<span>editado</span>\n")).collect();
    passos.push((src.join("b07_estilo.css"), "", "\n.b07-editado { color: red; }\n"));
    passos.push((src.join("a02_texto_estatico.dart"), "", "\nint extraA02() => 1;\n"));
    passos.push((src.join("a16_entrada_e_saida.dart"), "  String titulo = '';", "  String titulo = '';\n\n  @Input()\n  String subtitulo = '';"));
    passos.push((src.join("a13_componente_filho.dart"), "selector: 'a13-componente-filho'", "selector: 'a13-filho-renomeado'"));
    passos.push((src.join("a13_componente_filho.dart"), "import 'a02_texto_estatico.dart';", "import 'a02_texto_estatico.dart';\nimport 'a01_interpolacao.dart';"));
    passos.push((src.join("z_novo.dart"), "", "class ZNovo {}\n"));
    for (arq, trecho, texto) in passos {
        let mut atual = std::fs::read_to_string(&arq).unwrap_or_default();
        if trecho.is_empty() {
            atual.push_str(texto);
        } else {
            assert!(atual.contains(trecho), "{}: trecho ausente", arq.display());
            atual = atual.replacen(trecho, texto, 1);
        }
        std::fs::write(&arq, &atual).unwrap();
        let (p, nomes) = programa(&raiz);
        let atual = vivo
            .atualizar(
                &Contexto {
                    banco: &SemBanco,
                    programa: Some((&p, &nomes)),
                },
                &[arq.clone()],
                Demanda::Tudo,
            )
            .expect("atualizar");
        if arq.extension().is_some_and(|e| e == "html" || e == "css") {
            assert_eq!(atual.rel.unidades_nativas, 1, "a edição de um recurso deve regenerar só seu componente");
            assert_eq!(atual.rel.consultas_gerador, 1, "só o digest do recurso mudado deve ser registrado novamente");
        }
        // `.dart` existente: só o arquivo e quem o alcança por import/export
        // (o fecho; o `a02` é filho de 13 casos), não o pacote inteiro (221
        // arquivos).
        if arq.extension().is_some_and(|e| e == "dart") && !arq.ends_with("z_novo.dart") {
            println!("{}: {} unidades regeneradas", arq.display(), atual.rel.unidades_nativas);
            assert!(
                (1..=20).contains(&atual.rel.unidades_nativas),
                "{}: {} unidades regeneradas",
                arq.display(),
                atual.rel.unidades_nativas
            );
        }
        let novo = motor(&raiz, &p, &nomes);
        assert_eq!(vivo.estado_canonico(), novo.estado_canonico(), "incremental ≠ do zero depois de {}", arq.display());
    }
}

/// Varredura do estágio B para `.dart` (B03): cada arquivo do corpus ganha uma
/// declaração de topo, um de cada vez; o motor vivo (que regenera só o fecho
/// de importadores) e um motor novo têm de chegar ao mesmo estado.
#[test]
#[ignore = "exige `dart pub get` em corpus/ngdart; lento (um motor novo por arquivo)"]
fn ng_edicao_dart_em_cada_arquivo_igual_ao_do_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let raiz = tmp.path().join("corpus_ngdart");
    copiar(&corpus(), &raiz);
    let mut arquivos: Vec<PathBuf> = std::fs::read_dir(raiz.join("lib/src"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dart"))
        .collect();
    arquivos.sort();
    let (p, nomes) = programa(&raiz);
    let mut vivo = motor(&raiz, &p, &nomes);
    drop((p, nomes));
    let (mut seletivas, mut total) = (0, 0);
    for (i, arq) in arquivos.iter().enumerate() {
        let mut texto = std::fs::read_to_string(arq).unwrap();
        texto.push_str(&format!("\nint extraDaVarredura{i}() => {i};\n"));
        std::fs::write(arq, &texto).unwrap();
        let (p, nomes) = programa(&raiz);
        let at = vivo
            .atualizar(
                &Contexto {
                    banco: &SemBanco,
                    programa: Some((&p, &nomes)),
                },
                std::slice::from_ref(arq),
                Demanda::Tudo,
            )
            .expect("atualizar");
        total += 1;
        if at.rel.unidades_nativas < 100 {
            seletivas += 1;
        }
        let novo = motor(&raiz, &p, &nomes);
        assert_eq!(vivo.estado_canonico(), novo.estado_canonico(), "incremental ≠ do zero depois de {}", arq.display());
    }
    println!("estágio B em .dart: {seletivas} de {total} edições sem regenerar o pacote inteiro");
}

/// Pacote dependente (B03): `corpus/ngdart_dependente`, uma aplicação que usa
/// o componente de uma dependência `path`. Os `.template.dart` dos dois
/// pacotes saem do nativo iguais ao oráculo do `build_runner`, e edições na
/// dependência e na aplicação dão o estado de um motor do zero.
///
/// Exige `dart pub get` em `corpus/ngdart_dependente/app`.
#[test]
#[ignore = "exige `dart pub get` em corpus/ngdart_dependente/app"]
fn ng_dependencia_pelo_motor_igual_ao_oraculo() {
    let origem = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart_dependente");
    let tmp = tempfile::tempdir().unwrap();
    for d in ["app", "dep"] {
        copiar(&origem.join(d), &tmp.path().join(d));
    }
    let app = tmp.path().join("app");
    let dep = tmp.path().join("dep");
    let programa = |app: &Path| {
        let mut nomes = Interner::new();
        let cfg = app.join(".dart_tool/package_config.json");
        // A biblioteca do componente alcança a dependência; o `web/main.dart`
        // só importa o `.template.dart`, que ainda não existe.
        let (p, _) = dartforge_elements::load::load_lenient(&app.join("lib/app.dart"), &sdk(), Some(&cfg), &mut nomes);
        (p, nomes)
    };
    let novo = |app: &Path, p: &dartforge_elements::model::Program, nomes: &Interner| {
        let cfg = PackageConfig::load(&app.join(".dart_tool/package_config.json")).expect("package_config.json (dart pub get)");
        let mut m = Motor::novo(app, &cfg, OpcoesMotor::default()).expect("motor");
        m.atualizar(&Contexto { banco: &SemBanco, programa: Some((p, nomes)) }, &[], Demanda::Tudo).expect("atualizar");
        m
    };
    let (p, nomes) = programa(&app);
    let mut vivo = novo(&app, &p, &nomes);
    let g = vivo.geracao();
    let mut conferidos = 0;
    for (pacote, raiz) in [("app_ng", &app), ("dep_ng", &dep)] {
        let dir = origem.join("oraculo").join(pacote);
        let mut arquivos = Vec::new();
        let mut pilha = vec![dir.clone()];
        while let Some(d) = pilha.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                if e.path().is_dir() {
                    pilha.push(e.path());
                } else {
                    arquivos.push(e.path());
                }
            }
        }
        for o in arquivos {
            let rel = o.strip_prefix(&dir).unwrap();
            let k = dartforge_elements::gerado::chave(&raiz.join(rel));
            let gerado = g.obter(&k).unwrap_or_else(|| panic!("{}: ausente na geração", k.display()));
            let esperado = std::fs::read_to_string(&o).unwrap();
            assert_eq!(*gerado.conteudo, esperado, "{pacote}/{}: nativo ≠ oráculo", rel.display());
            conferidos += 1;
        }
    }
    assert_eq!(conferidos, 5);
    drop((p, nomes));
    // Edições: recurso e `@Input` na dependência, corpo na aplicação.
    let passos: [(PathBuf, &str, &str); 3] = [
        (dep.join("lib/src/botao.html"), "</button>", "</button><span>editado</span>"),
        (dep.join("lib/src/botao.dart"), "  String rotulo = '';", "  String rotulo = '';\n\n  @Input()\n  bool ativo = false;"),
        (app.join("lib/app.dart"), "  String texto = 'ok';", "  String texto = 'ok';\n  int contador = 0;"),
    ];
    for (arq, trecho, texto) in passos {
        let atual = std::fs::read_to_string(&arq).unwrap();
        assert!(atual.contains(trecho), "{}: trecho ausente", arq.display());
        std::fs::write(&arq, atual.replacen(trecho, texto, 1)).unwrap();
        let (p, nomes) = programa(&app);
        vivo.atualizar(&Contexto { banco: &SemBanco, programa: Some((&p, &nomes)) }, &[arq.clone()], Demanda::Tudo)
            .expect("atualizar");
        let zero = novo(&app, &p, &nomes);
        assert_eq!(vivo.estado_canonico(), zero.estado_canonico(), "incremental ≠ do zero depois de {}", arq.display());
    }
}
