//! O parser novo aceita 100% do `lib/` do SDK 3.6.2 e do corpus de pub.dev.
//!
//! Este é o critério de aceite do passo 1 da meta governante (PLANO.md). O
//! teste é ignorado por padrão porque depende de arquivos fora do repositório:
//! `DARTFORGE_SDK_LIB` (padrão `C:/tools/dartsdk-3.6.2/lib`) e
//! `references/pub` na raiz do workspace.
use std::path::{Path, PathBuf};

fn coletar(dir: &Path, saida: &mut Vec<PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let caminho = entrada.path();
        if caminho.is_dir() {
            coletar(&caminho, saida);
        } else if caminho.extension().is_some_and(|e| e == "dart") {
            saida.push(caminho);
        }
    }
}

/// A versão de linguagem do pacote dono de `arquivo`: o `x.y` do limite
/// inferior de `sdk:` no `pubspec.yaml` mais próximo (`^3.8.0`,
/// `'>=3.8.0 <4.0.0'`), como o `languageVersion` que o `pub get` grava.
fn versao_do_pacote(arquivo: &Path) -> Option<dartforge_frontend::LanguageVersion> {
    let pubspec = arquivo
        .ancestors()
        .skip(1)
        .map(|d| d.join("pubspec.yaml"))
        .find(|p| p.is_file())?;
    let texto = std::fs::read_to_string(pubspec).ok()?;
    let linha = texto.lines().find(|l| l.trim_start().starts_with("sdk:"))?;
    let valor = linha
        .trim_start()
        .trim_start_matches("sdk:")
        .trim()
        .trim_matches(['\'', '"']);
    let inferior = valor
        .trim_start_matches('^')
        .trim_start_matches(">=")
        .trim();
    let mut partes = inferior.split(['.', ' ']);
    let maior: u16 = partes.next()?.parse().ok()?;
    let menor: u16 = partes.next()?.parse().ok()?;
    Some(dartforge_frontend::LanguageVersion::new(maior, menor))
}

fn medir(raiz: &Path) -> (usize, usize, Vec<String>) {
    let mut arquivos = Vec::new();
    coletar(raiz, &mut arquivos);
    arquivos.sort();
    let mut nomes = dartforge_intern::Interner::new();
    let mut total = 0;
    let mut aceitos = 0;
    let mut falhas = Vec::new();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        total += 1;
        // A versão de cada arquivo (docs/VERSOES-LINGUAGEM.md §2): o
        // marcador; senão a do pacote, que o `pub` tira do limite inferior
        // de `environment: sdk:` no pubspec; senão o piso 3.6 (o SDK).
        let versao = dartforge_frontend::features::marcador_versao(&fonte)
            .map(|m| m.0)
            .or_else(|| versao_do_pacote(arquivo))
            .unwrap_or(dartforge_frontend::LanguageVersion::PISO);
        let features = dartforge_frontend::LibraryFeatures::new(versao, &[]);
        let saida = dartforge_frontend::parser::parse_com(&fonte, &mut nomes, features);
        if saida.diagnostics.is_empty() {
            aceitos += 1;
        } else {
            let d = &saida.diagnostics[0];
            let linha = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            falhas.push(format!("{}:{}: {}", arquivo.display(), linha, d.message));
        }
    }
    (aceitos, total, falhas)
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn sdk_lib_inteiro_e_aceito() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let (aceitos, total, falhas) = medir(Path::new(&sdk));
    assert!(total > 0, "SDK não encontrado em {sdk}");
    assert_eq!(
        aceitos,
        total,
        "arquivos do SDK recusados:\n{}",
        falhas.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2 e do corpus em references/pub"]
fn corpus_pub_inteiro_e_aceito() {
    // `DARTFORGE_PUB_CORPUS` aponta outro clone (numa worktree, o do checkout
    // principal: `references/` fica fora do git).
    let raiz = std::env::var("DARTFORGE_PUB_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub"));
    let (aceitos, total, falhas) = medir(&raiz);
    assert!(total > 0, "corpus não encontrado em {}", raiz.display());
    assert_eq!(
        aceitos,
        total,
        "arquivos do corpus recusados:\n{}",
        falhas.join("\n")
    );
}

#[test]
#[ignore = "depende do SDK 3.6.2"]
fn sdk_lib_inteiro_lexa() {
    let sdk = std::env::var("DARTFORGE_SDK_LIB").unwrap_or("C:/tools/dartsdk-3.6.2/lib".into());
    let mut arquivos = Vec::new();
    coletar(Path::new(&sdk), &mut arquivos);
    let mut falhas = Vec::new();
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        if let Err(d) = dartforge_frontend::lexer::lex(&fonte) {
            let linha = fonte[..d.span.start.min(fonte.len())].matches('\n').count() + 1;
            falhas.push(format!("{}:{}: {}", arquivo.display(), linha, d.message));
        }
    }
    assert!(falhas.is_empty(), "{}", falhas.join("\n"));
}
