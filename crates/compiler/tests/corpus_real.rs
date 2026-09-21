//! Aferição contra código Dart de produção, não contra exemplos sintéticos.
//!
//! Uma sondagem de construções isoladas diz quais recursos existem; ela não diz
//! se o compilador aguenta um arquivo de verdade, com dezenas de recursos
//! combinados. Este teste roda o front-end sobre cada arquivo de um pacote real
//! do pub.dev e agrupa os diagnósticos, produzindo uma lista de lacunas ordenada
//! por quantas vezes cada uma aparece — que é a ordem em que vale corrigi-las.
//!
//! O corpus não é versionado: ele é grande e tem licença própria. Baixe com
//! `scripts/corpus.sh` para `references/pub/`. Sem o corpus, o teste é ignorado
//! em vez de falhar, porque a ausência do corpus não é um defeito do compilador.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Localiza a raiz do corpus baixado, se existir.
fn corpus_root() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../references/pub");
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.file_name()?.to_str()?.starts_with("pdf-") {
            return Some(path);
        }
    }
    None
}

/// Coleta recursivamente os arquivos `.dart` do corpus.
fn dart_files(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dart_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "dart") {
            found.push(path);
        }
    }
    found.sort();
}

/// Reduz um diagnóstico à sua forma, descartando nomes e números concretos.
///
/// Sem isso, `Unknown identifier 'a'` e `Unknown identifier 'b'` contariam como
/// lacunas diferentes e o ranking perderia o sentido.
fn shape(message: &str) -> String {
    let mut texto = String::with_capacity(message.len());
    let mut em_aspas = false;
    for caractere in message.chars() {
        match caractere {
            '\'' | '"' | '`' => {
                if !em_aspas {
                    texto.push_str("'…'");
                }
                em_aspas = !em_aspas;
            }
            _ if em_aspas => {}
            _ if caractere.is_ascii_digit() => texto.push('#'),
            _ => texto.push(caractere),
        }
    }
    texto
}

/// Roda o front-end sobre cada arquivo do corpus e resume o resultado.
///
/// Não exige sucesso: exige que o resultado seja **conhecido**. Um compilador
/// em construção falha em código de produção; o que não pode acontecer é falhar
/// sem que se saiba onde, nem entrar em pânico, nem travar.
#[test]
#[ignore = "requer o corpus em references/pub; use scripts/corpus.sh"]
fn a_real_package_reports_known_gaps() {
    let Some(root) = corpus_root() else {
        panic!("corpus ausente: rode scripts/corpus.sh");
    };
    let mut arquivos = Vec::new();
    dart_files(&root, &mut arquivos);
    assert!(!arquivos.is_empty(), "corpus sem arquivos .dart");

    let mut aceitos = 0usize;
    let mut lacunas: BTreeMap<String, (usize, PathBuf)> = BTreeMap::new();
    let mut bytes = 0usize;
    for arquivo in &arquivos {
        let Ok(fonte) = std::fs::read_to_string(arquivo) else {
            continue;
        };
        bytes += fonte.len();
        // A unidade isolada é o recorte certo: o pacote tem imports que o
        // subconjunto ainda não resolve, e o objetivo aqui é medir a linguagem,
        // não a resolução de dependências externas.
        match dartforge_compiler::compile_unit_diagnostics(&fonte) {
            Ok(()) => aceitos += 1,
            Err(erro) => {
                let entrada = lacunas
                    .entry(shape(&erro.message))
                    .or_insert((0, arquivo.clone()));
                entrada.0 += 1;
            }
        }
    }

    let mut ordenadas: Vec<_> = lacunas.iter().collect();
    ordenadas.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    println!(
        "corpus: {} arquivos, {} KB, {} aceitos ({:.1}%)",
        arquivos.len(),
        bytes / 1024,
        aceitos,
        100.0 * aceitos as f64 / arquivos.len() as f64
    );
    println!("lacunas por frequência:");
    for (forma, (quantas, exemplo)) in ordenadas.iter().take(30) {
        println!(
            "  {quantas:4}x  {forma}\n           ex.: {}",
            exemplo.file_name().unwrap().to_string_lossy()
        );
    }

    // O corpus existe para orientar, não para travar a integração contínua.
    // A afirmação testável é que o compilador termina em cada arquivo, com
    // sucesso ou com diagnóstico, sem pânico e sem laço infinito.
    assert_eq!(
        aceitos + ordenadas.iter().map(|(_, (n, _))| n).sum::<usize>(),
        arquivos.len(),
        "todo arquivo precisa terminar com sucesso ou diagnóstico"
    );
}
