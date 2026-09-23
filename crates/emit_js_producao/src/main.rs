//! `dartforge-jsprod` — compilação no perfil de produção.
//!
//! Binário próprio, e não um subcomando do `dartforge`, porque
//! `crates/cli/src/main.rs` é território de outro agente
//! (`docs/BRIEF-JS-PRODUCAO.md` §0.2).

use std::path::PathBuf;

use dartforge_emit_js_producao::{Opcoes, compilar};

const USO: &str = "uso: dartforge-jsprod <entrada.dart> -o <saida.js> \
[--sdk <lib>] [--packages <package_config.json>] [--dart-sdk-js <arquivo>] \
[--sem-poda] [--sem-membros] [--sem-poda-usuario] [--verificar-stub]";

fn main() {
    if let Err(e) = executar() {
        eprintln!("erro: {e}");
        std::process::exit(1);
    }
}

fn executar() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut entrada: Option<PathBuf> = None;
    let mut saida: Option<PathBuf> = None;
    let mut sdk: Option<PathBuf> = None;
    let mut packages: Option<PathBuf> = None;
    let mut dart_sdk_js: Option<PathBuf> = None;
    let mut op = Opcoes::default();
    let mut i = 0;
    while i < args.len() {
        let proximo = |i: &mut usize| -> Result<PathBuf, String> {
            *i += 1;
            args.get(*i).map(PathBuf::from).ok_or_else(|| USO.to_string())
        };
        match args[i].as_str() {
            "-o" => saida = Some(proximo(&mut i)?),
            "--sdk" => sdk = Some(proximo(&mut i)?),
            "--packages" => packages = Some(proximo(&mut i)?),
            "--dart-sdk-js" => dart_sdk_js = Some(proximo(&mut i)?),
            "--sem-poda" => op.podar_sdk = false,
            "--sem-membros" => op.por_membro = false,
            "--sem-poda-usuario" => op.podar_usuario = false,
            "--verificar-stub" => op.stub = true,
            "-h" | "--help" => {
                println!("{USO}");
                return Ok(());
            }
            outro if entrada.is_none() && !outro.starts_with('-') => entrada = Some(PathBuf::from(outro)),
            outro => return Err(format!("argumento desconhecido: {outro}\n{USO}")),
        }
        i += 1;
    }
    let (Some(entrada), Some(saida)) = (entrada, saida) else { return Err(USO.to_string()) };
    let runtime = dart_sdk_js.unwrap_or_else(dartforge_emit_js::dart_sdk_js_padrao);

    // Corpos profundos recursam fundo, como no `compile-js`: pilha própria.
    let (e2, s2, p2, r2) = (entrada.clone(), sdk.clone(), packages.clone(), runtime.clone());
    let prod = std::thread::Builder::new()
        .stack_size(1 << 30)
        .spawn(move || compilar(&e2, s2.as_deref(), p2.as_deref(), &r2, op))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "a compilação abortou")??;

    if let Some(pai) = saida.parent() {
        if !pai.as_os_str().is_empty() {
            std::fs::create_dir_all(pai).map_err(|e| format!("{}: {e}", pai.display()))?;
        }
    }
    std::fs::write(&saida, &prod.js).map_err(|e| format!("{}: {e}", saida.display()))?;
    for c in &prod.ciclos {
        eprintln!("aviso: ciclo de import entre módulos: {c}");
    }
    let kb = |n: usize| n as f64 / 1024.0;
    println!(
        "{} -> {} ({:.0} KB; {} módulos; usuário {:.0} KB; runtime {:.0} -> {:.0} KB, {}/{} unidades vivas)",
        entrada.display(),
        saida.display(),
        kb(prod.js.len()),
        prod.modulos,
        kb(prod.usuario),
        kb(prod.sdk_antes),
        kb(prod.sdk_depois),
        prod.sdk_vivas,
        prod.sdk_unidades,
    );
    if let Some(m) = &prod.mundo {
        let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
        let e = &m.estat;
        println!(
            "mundo: classes {}/{} instanciadas + {} só tipo; funções {}/{}; {} seletores; {} rodada(s); mundo {:.0} ms, emissão {:.0} ms, verificador {:.0} ms",
            e.classes_instanciadas, e.classes_usuario, e.classes_tipo, e.funcoes_vivas, e.funcoes_usuario, e.seletores, m.rodadas,
            ms(m.tempo_mundo), ms(m.tempo_emissao), ms(m.tempo_verificacao),
        );
        let limite = std::env::var("DARTFORGE_JSPROD_CURAS").ok().and_then(|v| v.parse().ok()).unwrap_or(20usize);
        if m.curas.len() > limite {
            eprintln!("aviso: verificador: {} lacuna(s) curada(s); DARTFORGE_JSPROD_CURAS=N mostra mais", m.curas.len());
        }
        for c in m.curas.iter().take(limite) {
            eprintln!("aviso: verificador: {c}");
        }
        for s in m.sem_elemento.iter().take(20) {
            eprintln!("aviso: referência pendente sem elemento: {s}");
        }
        if let Some(n) = m.inconsistencias {
            println!("conferência do mundo: {n} inconsistência(s)");
        }
    }
    Ok(())
}
