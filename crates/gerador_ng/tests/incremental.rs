//! A API do motor de build (`docs/BUILD-PEDIDOS-GERADOR-NG.md`): a geração
//! de um arquivo por vez, com o índice mantido incrementalmente, dá o mesmo
//! texto que a geração do pacote inteiro (`gerar_em`), e diz o que
//! consultou. Roda sobre o corpus do ngdart, com o banco semântico.
use dartforge_elements::load::load_lenient;
use dartforge_elements::sdk::SdkLayout;
use dartforge_gerador_ng::resolucao::Resolvedor;
use dartforge_gerador_ng::{
    ConsultaNg, Indice, Pacote, Placar, analisar_arquivo, caminho_do_template, gerar_arquivo,
    gerar_em,
};
use dartforge_intern::Interner;
use std::path::{Path, PathBuf};

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/ngdart")
}

fn fontes(raiz: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(raiz.join("lib").join("src"))
        .expect("lib/src do corpus")
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.ends_with(".dart") && !n.ends_with(".template.dart")
        })
        .collect();
    v.sort();
    v
}

#[test]
fn um_arquivo_por_vez_da_o_mesmo_que_o_pacote() {
    let raiz = raiz();
    let cfg = raiz.join(".dart_tool").join("package_config.json");
    assert!(
        cfg.is_file(),
        "sem {}: rode `dart pub get` em corpus/ngdart",
        cfg.display()
    );
    let sdk_dir =
        SdkLayout::discover().unwrap_or_else(|| PathBuf::from("C:/tools/dartsdk-3.6.2/lib"));
    let Ok(sdk) = SdkLayout::load(&sdk_dir, "dartdevc") else {
        eprintln!("SDK indisponível; a API incremental não foi verificada");
        return;
    };
    let mut nomes = Interner::new();
    let entrada = raiz.join("lib").join("corpus_ngdart.dart");
    let (programa, _) = load_lenient(&entrada, &sdk, Some(&cfg), &mut nomes);
    let resolvedor = Resolvedor::novo(&programa, &nomes);
    let pacote = Pacote {
        nome: "corpus_ngdart".into(),
        raiz: raiz.clone(),
    };

    // O pacote inteiro, pelo caminho de sempre.
    let mut c = dartforge_elements::gerado::Construtor::nova();
    let mut placar = Placar::default();
    let mut i1 = Interner::new();
    gerar_em(
        &pacote,
        &[raiz.join("lib")],
        &mut i1,
        &mut c,
        &mut placar,
        Some(&resolvedor),
    );
    let pacote_inteiro = c.concluir(1).expect("geração");

    // Um por vez: o índice do programa, e cada arquivo reposto por
    // `atualizar` como o motor faz quando ele muda.
    let mut indice = Indice::do_programa(&resolvedor);
    let mut i2 = Interner::new();
    let mut achados = Vec::new();
    for p in fontes(&raiz) {
        let texto = std::fs::read_to_string(&p).unwrap();
        let a = analisar_arquivo(&p, &texto, &mut i2);
        indice.atualizar(&pacote, &p, &a, Some(&resolvedor));
        achados.push((p, a));
    }
    let mut gerados = 0;
    let mut consultas_do_d01 = Vec::new();
    for (p, a) in &achados {
        let esperado = pacote_inteiro
            .iter()
            .find(|(k, _)| k.file_name() == caminho_do_template(p).file_name())
            .map(|(_, f)| f.conteudo.to_string());
        match gerar_arquivo(&pacote, p, a, Some(&resolvedor), &mut i2, &indice) {
            Ok(s) => {
                gerados += 1;
                assert_eq!(
                    Some(s.template.clone()),
                    esperado,
                    "{} difere da geração do pacote",
                    p.display()
                );
                if p.ends_with("d01_dois_filhos.dart") {
                    consultas_do_d01 = s.consultas.clone();
                }
            }
            Err(r) => assert!(
                esperado.is_none(),
                "{} recusado sozinho ({}) e gerado no pacote",
                p.display(),
                r.texto()
            ),
        }
    }
    assert!(gerados > 50, "só {gerados} gerados um por vez");
    assert!(
        consultas_do_d01.iter().any(|c| matches!(
            c,
            ConsultaNg::Filho { classe, seletor, .. }
                if classe == "A02TextoEstatico" && seletor == "a02-texto-estatico"
        )),
        "o d01 consultou os filhos: {consultas_do_d01:?}"
    );

    // Tirar um arquivo do índice some com o que ele declarava, e quem o usava
    // passa a consultar o seletor ausente.
    let filho = raiz.join("lib").join("src").join("a02_texto_estatico.dart");
    assert!(indice.declarante("a02-texto-estatico").is_some());
    indice.remover(&filho);
    assert!(indice.declarante("a02-texto-estatico").is_none());
    let (p, a) = achados
        .iter()
        .find(|(p, _)| p.ends_with("d01_dois_filhos.dart"))
        .unwrap();
    let depois = gerar_arquivo(&pacote, p, a, Some(&resolvedor), &mut i2, &indice);
    assert!(depois.is_err(), "d01 sem o filho no índice ainda gerou");
}