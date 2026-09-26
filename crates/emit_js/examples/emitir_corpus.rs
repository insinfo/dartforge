//! Emite cada programa dado e grava todos os módulos de cada um, concatenados
//! em ordem, num único arquivo por programa — para comparar a saída do
//! emissor antes e depois de uma mudança (`diff -r`), sem Node.
//!
//! `cargo run --release -p dartforge-emit-js --example emitir_corpus -- <dir_saida> corpus/js/*.dart`
//!
//! Com `DARTFORGE_JS_CONFERIR_TIPOS` ligado, cada compilação também confere
//! os tipos do emissor contra a inferência comum (`crates/emit_js/src/conferencia.rs`).

fn main() {
    let mut args = std::env::args().skip(1);
    let saida = std::path::PathBuf::from(
        args.next()
            .expect("uso: emitir_corpus <dir_saida> <arquivo.dart>..."),
    );
    std::fs::create_dir_all(&saida).expect("criar diretório de saída");
    let mut falhas = 0usize;
    for arquivo in args {
        let caminho = std::path::PathBuf::from(&arquivo);
        let nome = caminho
            .file_stem()
            .expect("nome do arquivo")
            .to_string_lossy()
            .to_string();
        let texto = match dartforge_emit_js::compilar(&caminho, None, None) {
            Ok(e) => {
                let mut modulos = e.modulos;
                modulos.sort();
                let mut t = String::new();
                for (m, corpo) in modulos {
                    t.push_str(&format!("//// {m}\n{corpo}"));
                }
                t
            }
            Err(e) => {
                falhas += 1;
                eprintln!("{arquivo}: falhou: {e}");
                format!("//// ERRO\n{e}\n")
            }
        };
        std::fs::write(saida.join(format!("{nome}.js")), texto).expect("gravar saída");
    }
    if falhas > 0 {
        eprintln!("{falhas} programa(s) não compilaram");
    }
}
