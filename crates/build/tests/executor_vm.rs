//! O executor de builders pela VM Dart (`crates/build/src/vm.rs`) contra o
//! oráculo do `build_runner` (`corpus/builders/`): a saída tem de ser byte a
//! byte a do `dart run build_runner build`, e a incrementalidade tem de vir
//! das leituras que o `BuildStep` fez pelo hospedeiro.
//!
//! Pré-requisitos, por isso `#[ignore]`: a VM Dart 3.6.2 (`DARTFORGE_BUILD_DART`,
//! `DARTFORGE_DART_SDK/bin/dart` ou `dart` no `PATH`) e `dart pub get
//! --enforce-lockfile` em cada caso (o `package_config.json`), como o `ci.yml`
//! faz para os outros testes do corpus.
use dartforge_build::consulta::SemBanco;
use dartforge_build::grafo::AssetId;
use dartforge_build::motor::Origem;
use dartforge_build::vm::{ConfigDaVm, ExecutorVm};
use dartforge_build::{Contexto, Demanda, Motor, OpcoesMotor, RelMotor};
use dartforge_elements::config::PackageConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn raiz_do_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders")
}

fn dart() -> PathBuf {
    dartforge_build::vm::dart_do_ambiente()
        .or_else(|| {
            std::env::var_os("DARTFORGE_DART_SDK")
                .map(|s| PathBuf::from(s).join("bin").join("dart"))
        })
        .unwrap_or_else(|| PathBuf::from("dart"))
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
        r.insert(
            id,
            std::fs::read(&arq).unwrap_or_else(|e| panic!("{}: {e}", arq.display())),
        );
    }
    r
}

/// Cópia do caso sem o oráculo, as edições, o `build/` e o `.dart_tool`
/// (só o `package_config.json`): sem apoio do `build_runner` no disco, toda
/// saída tem de vir do executor.
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
                std::fs::copy(
                    p.join("package_config.json"),
                    alvo.join("package_config.json"),
                )
                .unwrap();
                continue;
            }
            copiar(&p, &alvo);
        } else {
            std::fs::copy(&p, &alvo).unwrap();
        }
    }
}

fn motor_vm(dir: &Path) -> Motor {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json"))
        .expect("package_config.json (dart pub get)");
    let mut m = Motor::novo(dir, &cfg, OpcoesMotor::default()).expect("motor");
    m.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        dir,
    ))));
    m
}

fn atualizar(m: &mut Motor, mudados: &[PathBuf]) -> (RelMotor, f64) {
    let t0 = Instant::now();
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
    for a in &at.avisos {
        println!("aviso: {a}");
    }
    (at.rel, t0.elapsed().as_secs_f64() * 1000.0)
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

#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/json_serializable"]
fn json_serializable_pela_vm_igual_ao_build_runner_e_incremental() {
    let origem = raiz_do_corpus().join("json_serializable");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("json_serializable");
    copiar(&origem, &dir);

    // Build limpo: as duas saídas do manifesto, byte a byte.
    let mut m = motor_vm(&dir);
    let (rel, ms) = atualizar(&mut m, &[]);
    println!("limpo: {ms:.0} ms — {}", rel.texto());
    // json_serializable e combining_builder em lib/modelos.dart e bin/main.dart
    // (este sem anotação: nenhuma saída, como no oficial), e o
    // `source_gen:part_cleanup` na parte escrita (a âncora da parte que não
    // foi escrita é omitida, como no oficial).
    assert_eq!(
        (rel.dart, rel.acoes_executadas, rel.saidas_alteradas),
        (5, 5, 2),
        "{}",
        rel.texto()
    );
    let p = m.placar(&referencias(&origem));
    assert!(
        p.diferentes.is_empty() && p.pendentes.is_empty(),
        "placar {}: {:?} {:?}",
        p.resumo(),
        p.diferentes,
        p.pendentes
    );
    assert_eq!(p.iguais.len(), 2);
    // O `source_gen:part_cleanup` apaga a parte oculta: ela some da geração
    // publicada (o `FinalizedReader` do oficial), mas o `.g.dart` continua.
    assert!(!publicado(&m, "lib/modelos.json_serializable.g.part"));
    assert!(publicado(&m, "lib/modelos.g.dart"));

    // Nada mudou: nada executa.
    let (rel, ms) = atualizar(&mut m, &[]);
    println!("sem mudança: {ms:.1} ms — {}", rel.texto());
    assert_eq!(rel.acoes_executadas, 0);

    // As edições do corpus, cumulativas; cada estado igual ao de um motor novo.
    let esperado: [(&str, usize, usize); 3] = [
        // Corpo de método: reexecutam as ações que leram `modelos.dart` (o
        // json_serializable dele e o de `bin/main.dart`, que o importa, e o
        // combining_builder, que resolve a biblioteca); nenhuma saída muda.
        ("01-corpo-de-metodo", 3, 0),
        // Campo novo: as mesmas três e a limpeza da parte, que mudou; a
        // parte e o `.g.dart` mudam.
        ("02-campo-novo", 4, 2),
        // Arquivo novo: só as duas ações dele e a limpeza da parte nova (a
        // listagem de partes das outras não mudou).
        ("03-arquivo-novo", 3, 2),
    ];
    for (passo, executadas, alteradas) in esperado {
        let mudados = aplicar(&origem.join("edicoes").join(passo), &dir);
        let (rel, ms) = atualizar(&mut m, &mudados);
        println!("{passo}: {ms:.0} ms — {}", rel.texto());
        assert_eq!(
            (rel.acoes_executadas, rel.dart, rel.saidas_alteradas),
            (executadas, executadas, alteradas),
            "{passo}: {}",
            rel.texto()
        );
        // Motor novo, processo novo, kernel reaproveitado do disco.
        let mut novo = motor_vm(&dir);
        let (_, ms) = atualizar(&mut novo, &[]);
        println!("{passo}: do zero com o kernel reaproveitado: {ms:.0} ms");
        assert_eq!(
            m.estado_canonico(),
            novo.estado_canonico(),
            "{passo}: incremental ≠ do zero"
        );
    }
    let produto = m
        .grafo
        .gerados
        .keys()
        .find(|id| id.caminho.as_ref() == "lib/produto.g.dart")
        .and_then(|id| m.registro(m.grafo.gerados[id].acao))
        .expect("ação do produto.g.dart");
    assert_eq!(produto.origem, Origem::Dart);
    let texto = produto
        .saidas
        .iter()
        .find_map(|(_, c)| c.clone())
        .expect("produto.g.dart escrito");
    let texto = std::str::from_utf8(&texto).unwrap();
    assert!(
        texto.contains("'codigo_interno'"),
        "fieldRename.snake aplicado:\n{texto}"
    );

    // Demanda do carregador (`dev`/`serve`/`compile-js`): a parte oculta não
    // é `.dart`, mas o combining_builder a lê por glob — sem ela o `.g.dart`
    // sairia vazio. Com o executor Dart, só o opcional fica preguiçoso.
    let mut c = motor_vm(&dir);
    c.atualizar(
        &Contexto {
            banco: &SemBanco,
            programa: None,
        },
        &[],
        Demanda::Carregador,
    )
    .expect("atualizar");
    let g = |m: &Motor| {
        let id = AssetId::novo("corpus_json_serializable", "lib/modelos.g.dart");
        m.registro(m.grafo.gerados[&id].acao)
            .and_then(|r| r.saidas.iter().find(|(s, _)| *s == id)?.1.clone())
    };
    assert!(g(&c).is_some(), "modelos.g.dart na demanda do carregador");
    assert_eq!(g(&c), g(&m));
}

/// O placar de todos os casos pela VM (com a tabela no resumo do CI): a
/// mesma régua do `corpus.rs`, com o executor Dart real e sem o apoio do
/// `build_runner` no disco — zero diferentes, e incremental = do zero em
/// cada edição do caso.
#[test]
#[ignore = "exige a VM Dart e `dart pub get` em cada caso de corpus/builders"]
fn corpus_builders_pela_vm() {
    let mut casos: Vec<PathBuf> = std::fs::read_dir(raiz_do_corpus())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.join("oraculo/plano.dart").is_file()
                && p.join(".dart_tool/package_config.json").is_file()
        })
        .collect();
    casos.sort();
    let mut linhas = vec![
        "### corpus/builders pela VM — iguais/pendentes/diferentes".to_string(),
        String::new(),
        "| caso | iguais | pendentes | diferentes | limpo (ms) | sem mudança (ms) | motivos |"
            .into(),
        "|---|---|---|---|---|---|---|".into(),
    ];
    let mut diferentes = Vec::new();
    for origem in casos {
        let nome = origem.file_name().unwrap().to_string_lossy().to_string();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(&nome);
        copiar(&origem, &dir);
        let mut m = motor_vm(&dir);
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
        for (id, motivo) in &p.diferentes {
            diferentes.push(format!("{nome}: {} ({motivo})", id.texto()));
        }
        // Incremental = do zero nas edições do caso, com o executor real.
        let mut passos: Vec<PathBuf> = std::fs::read_dir(origem.join("edicoes"))
            .map(|l| l.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        passos.sort();
        for passo in passos {
            let mudados = aplicar(&passo, &dir);
            let (rel, ms) = atualizar(&mut m, &mudados);
            let mut novo = motor_vm(&dir);
            atualizar(&mut novo, &[]);
            let nome_passo = passo.file_name().unwrap().to_string_lossy().to_string();
            println!(
                "{nome}/{nome_passo}: {ms:.0} ms, {} ações executadas",
                rel.acoes_executadas
            );
            if m.estado_canonico() != novo.estado_canonico() {
                diferentes.push(format!(
                    "{nome}: incremental ≠ do zero depois de {nome_passo}"
                ));
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

/// Estado salvo entre processos com o executor real (B04): o segundo processo
/// não executa nenhum builder, chega ao estado de um motor do zero e o
/// código dos builders vem do depfile do bootstrap.
#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/json_serializable"]
fn estado_salvo_pela_vm() {
    let origem = raiz_do_corpus().join("json_serializable");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("json_serializable");
    copiar(&origem, &dir);
    let processo = |persistir: bool| {
        let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
        let opcoes = OpcoesMotor {
            persistir,
            ..OpcoesMotor::default()
        };
        let mut m = Motor::novo(&dir, &cfg, opcoes).unwrap();
        m.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
            dart(),
            &dir,
        ))));
        let t0 = Instant::now();
        let at = m
            .atualizar(
                &Contexto {
                    banco: &SemBanco,
                    programa: None,
                },
                &[],
                Demanda::Tudo,
            )
            .expect("atualizar");
        // Como a CLI: as saídas `source` que mudaram vão ao disco.
        for (p, c) in m.saidas_source_nativas(&at.alterados) {
            std::fs::write(&p, &c[..]).unwrap();
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        (m.estado_canonico(), at.rel, ms)
    };
    let (primeiro, rel, ms) = processo(true);
    println!("primeiro processo: {ms:.0} ms — {}", rel.texto());
    assert!(rel.dart > 0);
    let estado = std::fs::read_to_string(dir.join(".dart_tool/dartforge/build/estado/estado.json"))
        .expect("estado gravado");
    assert!(
        estado.contains("builders.dart") || estado.contains("\"codigo_dart\":["),
        "código dos builders registrado"
    );
    let (segundo, rel, ms) = processo(true);
    println!("segundo processo: {ms:.0} ms — {}", rel.texto());
    assert_eq!(rel.acoes_executadas, 0, "{}", rel.texto());
    let (zero, _, _) = processo(false);
    assert_eq!(primeiro, zero);
    assert_eq!(segundo, zero, "restaurado ≠ do zero");
    assert_eq!(m_placar_iguais(&dir, &origem), 2);
}

/// Iguais ao oráculo num processo novo que restaura o estado.
fn m_placar_iguais(dir: &Path, origem: &Path) -> usize {
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let opcoes = OpcoesMotor {
        persistir: true,
        ..OpcoesMotor::default()
    };
    let mut m = Motor::novo(dir, &cfg, opcoes).unwrap();
    atualizar(&mut m, &[]);
    let p = m.placar(&referencias(origem));
    assert!(p.diferentes.is_empty(), "{:?}", p.diferentes);
    p.iguais.len()
}

/// A saída de pós-processador `notas.rascunho.resumo` no registro da âncora.
fn resumo(m: &Motor) -> Option<std::sync::Arc<[u8]>> {
    (0..m.grafo.acoes.len())
        .filter(|&a| m.grafo.acoes[a].pos)
        .filter_map(|a| m.registro(a))
        .flat_map(|r| r.saidas.iter())
        .find(|(s, _)| s.caminho.as_ref() == "lib/entrada/notas.rascunho.resumo")
        .and_then(|(_, c)| c.clone())
}

/// A geração publicada tem um arquivo que termina em `fim`?
fn publicado(m: &Motor, fim: &str) -> bool {
    m.geracao()
        .iter()
        .any(|(k, _)| k.to_string_lossy().replace('\\', "/").ends_with(fim))
}

/// Pós-processadores (`post_process_builders`) pela VM: a âncora roda o
/// `PostProcessBuildStep` de verdade, a saída é a do `build_runner` byte a
/// byte, só a âncora cuja entrada mudou reexecuta, a saída some com a
/// entrada, e o `deletePrimaryInput` (opção de release do caso) é registrado
/// e recusado no modo estrito — o DartForge não tem o diretório mesclado em
/// que ele teria efeito.
#[test]
#[ignore = "exige a VM Dart e `dart pub get` em corpus/builders/cadeia_configuracao"]
fn pos_processador_pela_vm() {
    let origem = raiz_do_corpus().join("cadeia_configuracao");
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("cadeia_configuracao");
    copiar(&origem, &dir);
    let esperado = std::fs::read(
        origem.join("oraculo/cache/corpus_cadeia_configuracao/lib/entrada/notas.rascunho.resumo"),
    )
    .unwrap();
    let saida = "lib/entrada/notas.rascunho.resumo";

    let mut m = motor_vm(&dir);
    atualizar(&mut m, &[]);
    assert_eq!(resumo(&m).as_deref(), Some(esperado.as_slice()));
    let ancoras: Vec<usize> = (0..m.grafo.acoes.len())
        .filter(|&a| m.grafo.acoes[a].pos)
        .collect();
    assert_eq!(ancoras.len(), 1, "uma âncora: a única entrada .rascunho");
    assert!(
        m.registro(ancoras[0])
            .is_some_and(|r| r.apagados.is_empty()),
        "apagar: false no dev"
    );
    assert!(publicado(&m, saida), "saída publicada na geração");

    // Segunda linha mudou: só a âncora reexecuta, a saída é a mesma.
    let notas = dir.join("lib/entrada/notas.rascunho");
    std::fs::write(
        &notas,
        "nota do post_process_builder limpeza\noutra segunda linha",
    )
    .unwrap();
    let (rel, _) = atualizar(&mut m, std::slice::from_ref(&notas));
    assert_eq!(
        (rel.acoes_executadas, rel.saidas_alteradas),
        (1, 0),
        "{}",
        rel.texto()
    );

    // Primeira linha mudou: a saída muda; incremental = do zero.
    std::fs::write(&notas, "nota nova\nsegunda").unwrap();
    let (rel, _) = atualizar(&mut m, std::slice::from_ref(&notas));
    assert_eq!(
        (rel.acoes_executadas, rel.saidas_alteradas),
        (1, 1),
        "{}",
        rel.texto()
    );
    let texto = resumo(&m).unwrap();
    assert!(
        std::str::from_utf8(&texto)
            .unwrap()
            .ends_with("---\nnota nova\n")
    );
    let mut novo = motor_vm(&dir);
    atualizar(&mut novo, &[]);
    assert_eq!(
        m.estado_canonico(),
        novo.estado_canonico(),
        "incremental ≠ do zero"
    );

    // Entrada apagada: a âncora e a saída somem.
    std::fs::remove_file(&notas).unwrap();
    let (rel, _) = atualizar(&mut m, std::slice::from_ref(&notas));
    assert!(rel.saidas_alteradas >= 1, "{}", rel.texto());
    assert!(resumo(&m).is_none());
    assert!(!publicado(&m, saida), "saída retirada da geração");
    let mut novo = motor_vm(&dir);
    atualizar(&mut novo, &[]);
    assert_eq!(
        m.estado_canonico(),
        novo.estado_canonico(),
        "incremental ≠ do zero"
    );

    // Release: `apagar: true` chama `deletePrimaryInput`.
    std::fs::write(&notas, "nota do post_process_builder limpeza\n").unwrap();
    let cfg = PackageConfig::load(&dir.join(".dart_tool/package_config.json")).unwrap();
    let release = OpcoesMotor {
        release: true,
        ..OpcoesMotor::default()
    };
    let contexto = Contexto {
        banco: &SemBanco,
        programa: None,
    };
    let mut r = Motor::novo(&dir, &cfg, release.clone()).unwrap();
    r.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        &dir,
    ))));
    let at = r
        .atualizar(&contexto, &[], Demanda::Tudo)
        .expect("atualizar em release");
    let ancora = (0..r.grafo.acoes.len())
        .find(|&a| r.grafo.acoes[a].pos)
        .unwrap();
    assert_eq!(
        r.registro(ancora).unwrap().apagados,
        vec![AssetId::novo(
            "corpus_cadeia_configuracao",
            "lib/entrada/notas.rascunho"
        )]
    );
    assert!(
        at.avisos.iter().any(|a| a.contains("deletePrimaryInput")),
        "{:?}",
        at.avisos
    );
    let mut estrito = Motor::novo(
        &dir,
        &cfg,
        OpcoesMotor {
            estrito: true,
            ..release
        },
    )
    .unwrap();
    estrito.definir_executor_dart(Box::new(ExecutorVm::novo(ConfigDaVm::do_projeto(
        dart(),
        &dir,
    ))));
    let erro = estrito
        .atualizar(&contexto, &[], Demanda::Tudo)
        .err()
        .expect("o estrito recusa o apagamento sem efeito");
    assert!(erro.contains("deletePrimaryInput"), "{erro}");
}
