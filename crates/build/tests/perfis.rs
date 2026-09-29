//! Perfis de versão do motor de build (DF-BUILD-011) contra o oráculo do
//! `build_runner` oficial de cada versão: `corpus/builders/perfil_2_4`
//! (`build_runner` 2.4.15, `build_config` 1.1.2, Dart 3.6.2) e
//! `corpus/builders_novos/perfil_2_16` (`build_runner` 2.16.1, `build_config`
//! 1.3.3, Dart 3.13.4). Cada caso tem um oráculo por configuração
//! (`oraculos/<nome>/manifesto.json`, com os argumentos da linha de comando do
//! oficial): padrão, `--release`, `--define`, `--config alt` e
//! `--build-filter`. O `build.yaml` cobre defaults, alvo, global,
//! dev/release, desativação explícita (`enabled: false` e
//! `run_only_if_triggered: false`) e, no 2.16, triggers de anotação (direta,
//! com prefixo, em parte escrita à mão e em parte gerada por fase anterior) e
//! de import (inclusive o que um pacote de apoio acrescenta).
//!
//! A comparação é nos dois sentidos: o conjunto das saídas escritas pelo
//! motor tem de ser exatamente o do oráculo, byte a byte.
//!
//! Pré-requisitos, por isso `#[ignore]`: a VM Dart 3.6.2
//! (`DARTFORGE_BUILD_DART`, `DARTFORGE_DART_SDK/bin/dart` ou `dart` no
//! `PATH`) e `dart pub get --enforce-lockfile` em `corpus/builders/perfil_2_4`
//! (o CI faz). Os builders do caso 2.16 rodam pela mesma VM com as
//! dependências do 2.4 (`build` 2.4.2): o código deles só usa a API comum às
//! duas versões, e o `package_config.json` do 2.4 dá ao motor o mesmo grafo
//! de pacotes (nenhum pacote resolvido além da raiz define builders). O que
//! decide o perfil é o `pubspec.lock` do caso 2.16.
use dartforge_build::consulta::SemBanco;
use dartforge_build::grafo::AssetId;
use dartforge_build::linha_de_comando::OpcoesDoBuild;
use dartforge_build::vm::{ConfigDaVm, ExecutorVm};
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn caso_2_4() -> PathBuf {
    corpus().join("builders/perfil_2_4")
}

fn caso_2_16() -> PathBuf {
    corpus().join("builders_novos/perfil_2_16")
}

fn dart() -> PathBuf {
    dartforge_build::vm::dart_do_ambiente()
        .or_else(|| {
            std::env::var_os("DARTFORGE_DART_SDK")
                .map(|s| PathBuf::from(s).join("bin").join("dart"))
        })
        .unwrap_or_else(|| PathBuf::from("dart"))
}

/// Cópia do caso sem oráculos nem `.dart_tool`: nenhuma saída do oficial no
/// disco, tudo tem de vir do executor.
fn copiar(de: &Path, para: &Path) {
    std::fs::create_dir_all(para).unwrap();
    for e in std::fs::read_dir(de).unwrap().flatten() {
        let nome = e.file_name();
        if nome == "oraculos" || nome == ".dart_tool" || nome == "build" {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            copiar(&p, &para.join(&nome));
        } else {
            std::fs::copy(&p, para.join(&nome)).unwrap();
        }
    }
}

/// O `package_config.json` do caso 2.4 (resolvido pelo `pub get`), com o
/// pacote raiz renomeado para `raiz`.
fn package_config(destino: &Path, raiz: &str) {
    let origem = caso_2_4().join(".dart_tool/package_config.json");
    let texto = std::fs::read_to_string(&origem).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (dart pub get em corpus/builders/perfil_2_4)",
            origem.display()
        )
    });
    let mut v: serde_json::Value = serde_json::from_str(&texto).unwrap();
    for p in v["packages"].as_array_mut().unwrap() {
        if p["name"] == "corpus_perfil_2_4" {
            p["name"] = raiz.into();
        }
    }
    let dir = destino.join(".dart_tool");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("package_config.json"), v.to_string()).unwrap();
}

struct Oraculo {
    argumentos: Vec<String>,
    saidas: BTreeMap<AssetId, Vec<u8>>,
}

fn oraculo(caso: &Path, nome: &str) -> Oraculo {
    let dir = caso.join("oraculos").join(nome);
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifesto.json")).unwrap())
            .unwrap();
    let argumentos = m["argumentos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a.as_str().unwrap().to_string())
        .collect();
    let mut saidas = BTreeMap::new();
    for s in m["saidas"].as_array().unwrap() {
        let id = AssetId::de_texto(s["asset"].as_str().unwrap()).unwrap();
        let arq = match s["build_to"].as_str() {
            Some("source") => dir.join("source").join(id.caminho.as_ref()),
            _ => dir
                .join("cache")
                .join(id.pacote.as_ref())
                .join(id.caminho.as_ref()),
        };
        saidas.insert(id, std::fs::read(&arq).unwrap());
    }
    Oraculo { argumentos, saidas }
}

/// As opções do motor para os argumentos do `build_runner` do oráculo.
fn opcoes(dir: &Path, argumentos: &[String]) -> OpcoesMotor {
    let (mut release, mut config, mut defines, mut filtros) = (false, None, Vec::new(), Vec::new());
    let mut it = argumentos.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--release" => release = true,
            "--config" => config = it.next().cloned(),
            "--define" => defines.push(it.next().unwrap().clone()),
            "--build-filter" => filtros.push(it.next().unwrap().clone()),
            o => panic!("argumento sem tradução: {o}"),
        }
    }
    let mut o = OpcoesMotor {
        release,
        trabalhadores: 1,
        ..OpcoesMotor::default()
    };
    OpcoesDoBuild::do_projeto(dir, config.as_deref(), &defines, &filtros)
        .expect("opções")
        .aplicar(&mut o);
    o
}

fn motor(dir: &Path, opcoes: OpcoesMotor) -> Motor {
    let cfg =
        PackageConfig::load(&dir.join(".dart_tool/package_config.json")).expect("package_config");
    let mut m = Motor::novo(dir, &cfg, opcoes).expect("motor");
    m.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        dir,
    ))));
    m
}

fn atualizar(m: &mut Motor, mudados: &[PathBuf]) -> RelMotor {
    let at = m
        .atualizar(
            &Contexto {
                banco: &SemBanco,
                programa: None,
            },
            mudados,
            Demanda::Tudo,
        )
        .expect("atualizar");
    at.rel
}

/// As saídas que o motor escreveu (conteúdo `Some` no registro).
fn escritas(m: &Motor) -> BTreeMap<AssetId, Vec<u8>> {
    let mut v = BTreeMap::new();
    for (id, g) in &m.grafo.gerados {
        if let Some(c) = m
            .registro(g.acao)
            .and_then(|r| r.saidas.iter().find(|(s, _)| s == id))
            .and_then(|(_, c)| c.clone())
        {
            v.insert(id.clone(), c.to_vec());
        }
    }
    v
}

/// Diferenças entre o que o motor escreveu e o oráculo, nos dois sentidos.
fn diferencas(
    obtido: &BTreeMap<AssetId, Vec<u8>>,
    esperado: &BTreeMap<AssetId, Vec<u8>>,
) -> Vec<String> {
    let mut d = Vec::new();
    for (id, b) in esperado {
        match obtido.get(id) {
            None => d.push(format!("falta {}", id.texto())),
            Some(o) if o != b => d.push(format!(
                "difere {}:\n--- oficial\n{}\n--- motor\n{}",
                id.texto(),
                String::from_utf8_lossy(b),
                String::from_utf8_lossy(o)
            )),
            _ => {}
        }
    }
    for id in obtido.keys() {
        if !esperado.contains_key(id) {
            d.push(format!("a mais {}", id.texto()));
        }
    }
    d
}

/// Roda a configuração `nome` do caso e compara com o oráculo.
fn conferir(caso: &Path, raiz: &str, nome: &str) {
    let o = oraculo(caso, nome);
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(raiz);
    copiar(caso, &dir);
    package_config(&dir, raiz);
    let mut m = motor(&dir, opcoes(&dir, &o.argumentos));
    let rel = atualizar(&mut m, &[]);
    println!("{nome}: {}", rel.texto());
    assert!(rel.falhas.is_empty(), "{nome}: {}", rel.texto());
    let d = diferencas(&escritas(&m), &o.saidas);
    assert!(
        d.is_empty(),
        "{nome} ({:?}):\n{}",
        o.argumentos,
        d.join("\n")
    );
}

#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/perfil_2_4"]
fn perfil_2_4_igual_ao_build_runner_2_4_15() {
    for nome in ["padrao", "release", "define", "config_alt", "filtro"] {
        conferir(&caso_2_4(), "corpus_perfil_2_4", nome);
    }
}

#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/perfil_2_4"]
fn perfil_2_16_igual_ao_build_runner_2_16_1() {
    for nome in ["padrao", "release", "define", "config_alt", "filtro"] {
        conferir(&caso_2_16(), "corpus_perfil_2_16", nome);
    }
}

/// Os triggers são reavaliados pelo que leram: a entrada e as partes. Cada
/// edição, depois da atualização incremental, dá o mesmo que um motor novo.
#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/perfil_2_4"]
fn perfil_2_16_triggers_incrementais() {
    let raiz = "corpus_perfil_2_16";
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(raiz);
    copiar(&caso_2_16(), &dir);
    package_config(&dir, raiz);
    let mut m = motor(&dir, opcoes(&dir, &[]));
    atualizar(&mut m, &[]);
    assert_eq!(
        diferencas(&escritas(&m), &oraculo(&caso_2_16(), "padrao").saidas),
        Vec::<String>::new()
    );
    let id = |c: &str| AssetId::novo(raiz, c);
    let edicoes: [(&str, &str, &str, bool); 3] = [
        // O import relativo vira `package:`: o `importacao` passa a disparar.
        (
            "lib/relativo.dart",
            "import 'package:corpus_perfil_2_16/marcador.dart';\n\nconst valor = marcador;\n",
            "lib/relativo.importacao.txt",
            true,
        ),
        // A parte perde a anotação: `com_parte.dart` deixa de disparar.
        (
            "lib/com_parte_detalhe.dart",
            "part of 'com_parte.dart';\n\nclass Detalhe {}\n",
            "lib/com_parte.anotacao.txt",
            false,
        ),
        // O membro vira declaração de topo anotada.
        (
            "lib/membro.dart",
            "import 'anotacoes.dart';\n\n@Gerar()\nvoid metodo() {}\n",
            "lib/membro.anotacao.txt",
            true,
        ),
    ];
    for (arquivo, texto, saida, existe) in edicoes {
        let p = dir.join(arquivo);
        std::fs::write(&p, texto).unwrap();
        let rel = atualizar(&mut m, &[p]);
        println!("{arquivo}: {}", rel.texto());
        let incremental = escritas(&m);
        assert_eq!(
            incremental.contains_key(&id(saida)),
            existe,
            "{arquivo}: {saida}"
        );
        let mut novo = motor(&dir, opcoes(&dir, &[]));
        atualizar(&mut novo, &[]);
        assert_eq!(
            diferencas(&incremental, &escritas(&novo)),
            Vec::<String>::new(),
            "{arquivo}"
        );
    }
}
