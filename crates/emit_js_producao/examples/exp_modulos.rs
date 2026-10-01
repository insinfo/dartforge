//! Experimento: grava os módulos emitidos (sem montar o bundle).
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entrada = std::path::PathBuf::from(&args[0]);
    let saida = std::path::PathBuf::from(&args[1]);
    std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || {
            let l = dartforge_elements::sdk::Linguagem::default();
            let (e, _) = dartforge_emit_js::compilar_com(&entrada, None, None, &l, |a| a.emitir(None)).unwrap();
            std::fs::create_dir_all(&saida).unwrap();
            for (p, t) in &e.modulos {
                let d = saida.join(p);
                std::fs::create_dir_all(d.parent().unwrap()).unwrap();
                std::fs::write(d, t).unwrap();
            }
            std::fs::write(saida.join("main.mjs"), &e.entrada).unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}
