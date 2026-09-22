//! Mede a sessão residente: primeira compilação, edição de corpo, edição de
//! API pública e o platô de memória depois de N edições.
//!
//! `cargo run -q --release -p dartforge-dev --example medir -- <entrada.dart> [package_config.json] [arquivo_a_editar] [N]`
//!
//! O arquivo editado é restaurado ao fim (o exemplo trunca o que acrescentou).
#[global_allocator]
static ALOCADOR: dartforge_instrument::CountingAllocator = dartforge_instrument::CountingAllocator;

use dartforge_dev::{Relatorio, Sessao};
use std::io::Write;
use std::path::{Path, PathBuf};

const MB: f64 = 1_048_576.0;

/// Acrescenta texto ao fim do arquivo e devolve o tamanho anterior, para
/// restaurá-lo depois.
fn acrescentar(path: &Path, texto: &str) -> std::io::Result<u64> {
    let antes = std::fs::metadata(path)?.len();
    let mut f = std::fs::OpenOptions::new().append(true).open(path)?;
    f.write_all(texto.as_bytes())?;
    f.flush()?;
    Ok(antes)
}

fn restaurar(path: &Path, tamanho: u64) {
    if let Ok(f) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = f.set_len(tamanho);
    }
}

fn linha(nome: &str, r: &Relatorio) {
    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!(
        "{nome:<22}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>10.1}{:>8}{:>9}{:>8}",
        ms(r.carregar),
        ms(r.outline),
        ms(r.corpos),
        ms(r.emissao),
        ms(r.hashes),
        ms(r.escrita),
        ms(r.total()),
        r.unidades_reanalisadas,
        r.modulos_escritos,
        r.corpo_alterado.len(),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entrada = PathBuf::from(
        args.first().cloned().unwrap_or_else(|| "C:/MyDartProjects/new_sali/core/test/arvore_processo_item_test.dart".into()),
    );
    let packages = args.get(1).map(PathBuf::from).or_else(|| {
        let p = PathBuf::from("C:/MyDartProjects/new_sali/core/.dart_tool/package_config.json");
        p.exists().then_some(p)
    });
    let alvo = args.get(2).map(PathBuf::from).unwrap_or_else(|| entrada.clone());
    let edicoes: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(20);
    let saida = PathBuf::from("target/dev-medir");
    if !entrada.exists() {
        eprintln!("entrada não existe: {}", entrada.display());
        return;
    }

    let mut sessao = match Sessao::nova(&entrada, None, packages.as_deref(), &saida) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return;
        }
    };

    println!("== sessão residente — {}", entrada.display());
    println!(
        "{:<22}{:>9}{:>9}{:>9}{:>9}{:>9}{:>9}{:>10}{:>8}{:>9}{:>8}",
        "compilação", "carga", "outline", "corpos", "emissão", "hashes", "escrita", "total ms", "reanal", "escritos", "corpo≠"
    );

    let r = sessao.compilar().expect("primeira compilação");
    linha("primeira", &r);
    let detalhe = |n: &str, r: &Relatorio| {
        let c = &r.carga_detalhe;
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        println!(
            "   {n}: leitura+lex {:.1} par / {:.1} série, parse {:.1}, diretivas {:.1}, outline {:.1}+{:.1}+{:.1}+{:.1} ms ({} arquivos lidos)",
            ms(c.leitura_lex_paralelo), ms(c.leitura), ms(c.parse), ms(c.diretivas),
            ms(c.outline_declaracoes), ms(c.outline_reexports), ms(c.outline_escopos), ms(c.outline_supertipos),
            c.arquivos_lidos
        );
    };
    detalhe("carga da primeira", &r);
    let vivos_apos_primeira = dartforge_instrument::live_bytes();

    // 1. Edição de corpo. Duas funções são acrescentadas e depois um
    // `print` é inserido **no corpo da primeira**: é a edição real (uma
    // tecla dentro de um método) e desloca tudo o que vem depois, que era
    // onde o hash de API por faixa de texto escorregava.
    let base = acrescentar(&alvo, "\nvoid _forjaA() { print('a'); }\nvoid _forjaB() { print('b'); }\n").expect("edição");
    sessao.arquivo_mudou(&alvo);
    sessao.compilar().expect("preparo da edição de corpo");
    restaurar(&alvo, base);
    let tamanho = acrescentar(
        &alvo,
        "\nvoid _forjaA() { print('a'); print('x'); }\nvoid _forjaB() { print('b'); }\n",
    )
    .expect("edição");
    sessao.arquivo_mudou(&alvo);
    let r = sessao.compilar().expect("edição de corpo");
    linha("edição de corpo", &r);
    detalhe("carga da edição de corpo", &r);
    println!(
        "   corpo alterado: {} | API alterada: {} (+{} dependentes)",
        r.corpo_alterado.len(),
        r.api_alterada.len(),
        r.dependentes_invalidados
    );
    let api_mudou_no_corpo = r.api_alterada.len();
    restaurar(&alvo, base);
    let _ = tamanho;

    // 2. Edição de API pública: função de topo pública nova.
    let tamanho = acrescentar(&alvo, "\nvoid forjaApi0() { print('api 0'); }\n").expect("edição");
    sessao.arquivo_mudou(&alvo);
    let r = sessao.compilar().expect("edição de API");
    linha("edição de API", &r);
    println!(
        "   API alterada: {} biblioteca(s), {} dependente(s) invalidado(s)",
        r.api_alterada.len(),
        r.dependentes_invalidados
    );
    restaurar(&alvo, tamanho);

    // 3. Platô: N edições de corpo seguidas.
    println!("\n== platô de memória ({edicoes} edições de corpo)");
    dartforge_instrument::reset_peak();
    let mut vivos = Vec::with_capacity(edicoes);
    let mut soma_ms = 0.0;
    for i in 0..edicoes {
        let tamanho = acrescentar(&alvo, &format!("\nvoid _forjaPlato{i}() {{ print('p{i}'); }}\n")).expect("edição");
        sessao.arquivo_mudou(&alvo);
        let r = sessao.compilar().expect("compilação do platô");
        soma_ms += r.total().as_secs_f64() * 1000.0;
        restaurar(&alvo, tamanho);
        let v = dartforge_instrument::live_bytes();
        vivos.push(v);
        if i < 3 || i + 1 == edicoes {
            println!(
                "  edição {:>2}: vivos {:>8.2} MB | unidades no cache {} | retido pela sessão {:.2} MB | {:.0} ms",
                i + 1,
                v as f64 / MB,
                sessao.unidades_em_cache(),
                sessao.bytes_retidos() as f64 / MB,
                r.total().as_secs_f64() * 1000.0
            );
        }
    }
    let primeiro = vivos[0] as f64 / MB;
    let ultimo = *vivos.last().unwrap() as f64 / MB;
    println!(
        "vivos após 1ª edição {primeiro:.2} MB → após {edicoes}ª {ultimo:.2} MB (crescimento {:+.2} MB); pico {:.2} MB; média {:.0} ms/edição; vivos após a primeira compilação {:.2} MB",
        ultimo - primeiro,
        dartforge_instrument::peak_bytes() as f64 / MB,
        soma_ms / edicoes as f64,
        vivos_apos_primeira as f64 / MB
    );
    if api_mudou_no_corpo != 0 {
        println!("aviso: a edição de corpo mexeu na API — o hash de API não está separando corpo de assinatura");
    }
}
