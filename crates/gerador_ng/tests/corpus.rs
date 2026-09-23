//! O corpus do ngdart: uma forma por arquivo, com o `.template.dart` do
//! compilador oficial ao lado, em `corpus/ngdart/oraculo/`.
//!
//! O contrato é o mesmo do corpus do backend JS: o que geramos **tem** de sair
//! byte a byte igual ao oficial. Forma que ainda não sabemos gerar não é
//! falha; forma que geramos errado, sim.
//!
//! O oráculo se regenera com `scripts/corpus-ngdart.ps1` (roda o
//! `build_runner` oficial uma vez).
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_gerador_ng::resolucao::Resolvedor;
use dartforge_gerador_ng::{Pacote, Placar, caminho_do_template, gerar_em};
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};

fn raiz_do_corpus() -> PathBuf {
    // O diretório do crate é `crates/gerador_ng`.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

/// Casos do corpus que o gerador tem de recusar, com a sub-forma da recusa.
const RECUSADOS: &[(&str, &str)] = &[(
    "g02_select_ng_model.dart",
    "diretiva SelectControlValueAccessor",
)];

#[test]
fn o_que_geramos_e_igual_ao_oficial() {
    let raiz = raiz_do_corpus();
    let oraculo = raiz.join("oraculo");
    if !oraculo.is_dir() {
        eprintln!(
            "sem oráculo em {}; rode scripts/corpus-ngdart.ps1",
            oraculo.display()
        );
        return;
    }
    let pacote = Pacote {
        nome: "corpus_ngdart".into(),
        raiz: raiz.clone(),
    };
    // O teste roda o mesmo caminho da produção: carrega o projeto e gera com
    // banco semântico. Sem ele, casos como `{{ item.nome }}` — que precisam do
    // tipo de um membro noutra classe — ficariam fora da verificação.
    let entrada = raiz.join("lib").join("corpus_ngdart.dart");
    let cfg = raiz.join(".dart_tool").join("package_config.json");
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let Ok(sdk) = SdkLayout::load(&sdk_dir, "dartdevc") else {
        eprintln!("SDK indisponível; o corpus não foi verificado");
        return;
    };
    let mut nomes = Interner::new();
    let (programa, _) = load_lenient(&entrada, &sdk, Some(&cfg), &mut nomes);
    let resolvedor = Resolvedor::novo(&programa, &nomes);
    let mut interner = Interner::new();
    let mut c = dartforge_elements::gerado::Construtor::nova();
    let mut placar = Placar::default();
    gerar_em(
        &pacote,
        &[raiz.join("lib")],
        &mut interner,
        &mut c,
        &mut placar,
        Some(&resolvedor),
    );
    let nossa = c.concluir(1).expect("geração");

    let mut conferidos = 0usize;
    let mut diferentes = Vec::new();
    for (caminho, f) in nossa.iter() {
        let nome = caminho.file_name().unwrap_or_default();
        let oficial = oraculo.join(nome);
        let Ok(esperado) = std::fs::read_to_string(&oficial) else {
            continue;
        };
        conferidos += 1;
        if esperado.replace("\r\n", "\n") != f.conteudo.as_ref() {
            diferentes.push((
                caminho.clone(),
                esperado.replace("\r\n", "\n"),
                f.conteudo.to_string(),
            ));
        }
    }
    for (caminho, esperado, obtido) in &diferentes {
        eprintln!("=== {} ===", caminho.display());
        for (i, (a, b)) in esperado.lines().zip(obtido.lines()).enumerate() {
            if a != b {
                eprintln!("linha {}\n  oficial: {a}\n  nosso:   {b}", i + 1);
            }
        }
        if esperado.lines().count() != obtido.lines().count() {
            eprintln!(
                "  linhas: oficial {}, nosso {}",
                esperado.lines().count(),
                obtido.lines().count()
            );
        }
    }
    assert!(
        diferentes.is_empty(),
        "{} arquivo(s) diferentes do oficial",
        diferentes.len()
    );
    println!(
        "corpus ngdart: {conferidos} conferidos, {} pendentes",
        placar.pendentes.len()
    );
    for (p, c) in placar.pendentes.iter().zip(&placar.conjuntos) {
        let recusas: Vec<String> = c.iter().map(|r| r.texto()).collect();
        println!(
            "  pendente {}: {}",
            p.file_name().unwrap_or_default().to_string_lossy(),
            recusas.join("; ")
        );
    }
    // Os casos que existem para ser recusados: gerar qualquer um deles daria
    // saída errada, e a recusa tem de vir pelo motivo certo.
    for (arquivo, forma) in RECUSADOS {
        let achado = placar
            .pendentes
            .iter()
            .zip(&placar.conjuntos)
            .find(|(p, _)| p.file_name().is_some_and(|n| n == *arquivo));
        let Some((_, conjunto)) = achado else {
            panic!("{arquivo} devia ser recusado ({forma}) e foi gerado");
        };
        assert!(
            conjunto.iter().any(|r| r.forma == *forma),
            "{arquivo} recusado por outro motivo: {conjunto:?}"
        );
    }
    // Não deixar o corpus virar decoração: pelo menos as formas que já
    // declaramos cobertas têm de estar aqui.
    assert!(
        conferidos >= 2,
        "o corpus tem de conferir pelo menos as formas cobertas"
    );
}

/// Todo `.dart` do corpus tem o seu oficial: sem isso o corpus silenciosamente
/// para de cobrir o que diz cobrir.
#[test]
fn todo_arquivo_do_corpus_tem_oraculo() {
    let raiz = raiz_do_corpus();
    let src = raiz.join("lib").join("src");
    let oraculo = raiz.join("oraculo");
    if !src.is_dir() || !oraculo.is_dir() {
        return;
    }
    let mut faltando = Vec::new();
    for e in std::fs::read_dir(&src).expect("lib/src").flatten() {
        let p = e.path();
        let nome = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !nome.ends_with(".dart") || nome.ends_with(".template.dart") {
            continue;
        }
        let esperado = oraculo.join(caminho_do_template(&p).file_name().unwrap_or_default());
        if !esperado.is_file() {
            faltando.push(nome);
        }
    }
    assert!(
        faltando.is_empty(),
        "sem oráculo: {faltando:?} — rode scripts/corpus-ngdart.ps1"
    );
}
