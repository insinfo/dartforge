//! `cargo run -p dartforge-build --example plano -- <projeto> [build.dart]`
//!
//! Imprime o plano canônico do projeto e, se houver `build.dart` (o do
//! próprio projeto por padrão), compara com ele.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raiz = std::path::PathBuf::from(args.first().expect("uso: plano <projeto> [build.dart]"));
    let cfg_path = raiz.join(".dart_tool/package_config.json");
    let cfg = dartforge_elements::config::PackageConfig::load(&cfg_path).expect("package_config.json");
    let t = std::time::Instant::now();
    let (_g, _c, plano) = dartforge_build::plano_do_projeto(&raiz, &cfg).expect("plano");
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    print!("{}", plano.texto_canonico());
    for a in &plano.avisos {
        eprintln!("aviso: {a}");
    }
    let bd = args.get(1).map(std::path::PathBuf::from).unwrap_or_else(|| raiz.join(".dart_tool/build/entrypoint/build.dart"));
    if let Ok(texto) = std::fs::read_to_string(&bd) {
        match dartforge_build::oraculo::comparar(&plano.aplicacoes, &texto).expect("build.dart legível") {
            Ok(()) => println!("plano IGUAL ao {} ({} aplicações, {ms:.1} ms)", bd.display(), plano.aplicacoes.len()),
            Err(d) => {
                println!("plano DIFERENTE do {}:", bd.display());
                for l in d {
                    println!("{l}");
                }
                std::process::exit(1);
            }
        }
    }
}
