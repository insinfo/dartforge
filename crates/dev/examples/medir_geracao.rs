//! Latência de edição com o motor de geração ligado (plano do motor §6.5).
//!
//! `cargo run --release -p dartforge-dev --example medir_geracao -- <entrada.dart> <package_config.json> [saida] [repeticoes]`
//!
//! Abre a sessão, compila, escolhe um componente cujo `.template.dart` sai
//! do gerador nativo (com `.html` e `.scss` ao lado) e um `.dart` do pacote
//! sem Angular, e mede cada edição aplicada e revertida (o arquivo volta ao
//! texto original no fim de cada uma): o relógio vai da detecção até os
//! módulos escritos, que é o `compilar` da sessão.
use dartforge_dev::geracao::{etapa_de_build, EtapaDeGeracao};
use dartforge_dev::{Relatorio, Sessao};
use std::path::{Path, PathBuf};

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn linha(nome: &str, r: &Relatorio) {
    let m = r.motor.as_ref();
    println!(
        "{nome:<34}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>9.1}{:>8}{:>8}{:>9}{:>9}",
        ms(r.total()),
        m.map(|m| ms(m.tempo)).unwrap_or(0.0),
        m.map(|m| ms(m.tempo_nativo)).unwrap_or(0.0),
        ms(r.recarga_geracao),
        ms(r.escrita),
        m.map(|m| m.acoes_executadas).unwrap_or(0),
        m.map(|m| m.saidas_alteradas).unwrap_or(0),
        r.unidades_reanalisadas,
        r.modulos_escritos,
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let entrada = PathBuf::from(args.first().expect("uso: medir_geracao <entrada.dart> <package_config.json> [saida] [repeticoes]"));
    let packages = PathBuf::from(args.get(1).expect("package_config.json"));
    let saida = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| "target/dev-medir-geracao".into()));
    let repeticoes: usize = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(3);

    // `sem-motor`: a mesma sessão sem etapa (a linha de base da regra de
    // custo zero), com os arquivos de `MEDIR_COMPONENTE`/`MEDIR_SEM_ANGULAR`.
    let sem_motor = args.get(4).is_some_and(|a| a == "sem-motor");
    let etapa = if sem_motor {
        None
    } else {
        match etapa_de_build(&entrada, Some(&packages)) {
            Some(Ok(e)) => Some(e),
            Some(Err(e)) => panic!("motor: {e}"),
            None => panic!("o projeto não usa builders"),
        }
    };
    let motor = etapa.as_ref().map(|e| e.motor());
    let etapas: Vec<Box<dyn EtapaDeGeracao>> = etapa.into_iter().map(|e| Box::new(e) as Box<dyn EtapaDeGeracao>).collect();
    let mut sessao = Sessao::com_etapas(&entrada, None, Some(&packages), &saida, etapas).expect("sessão");
    let t = std::time::Instant::now();
    let r = sessao.compilar().expect("primeira compilação");
    println!("primeira compilação: {:.1} ms (relógio {:.1} ms)", ms(r.total()), ms(t.elapsed()));
    print!("{}", r.texto());

    // Componente coberto: saída `.template.dart` nativa com `.html` e `.scss`.
    let raiz = dartforge_build::raiz_do_pacote(&entrada).expect("raiz");
    let (componente, sem_angular) = if let Ok(c) = std::env::var("MEDIR_COMPONENTE") {
        let dart = PathBuf::from(c);
        ((dart.clone(), dart.with_extension("html"), dart.with_extension("scss")), std::env::var("MEDIR_SEM_ANGULAR").ok().map(PathBuf::from))
    } else {
        let motor = motor.clone().expect("sem motor: defina MEDIR_COMPONENTE");
        let m = motor.lock().unwrap();
        let mut comp = None;
        let mut nativos = std::collections::BTreeSet::new();
        for (i, a) in m.grafo.acoes.iter().enumerate() {
            let Some(r) = m.registro(i) else { continue };
            let f = &m.fases[a.fase];
            if !matches!(r.origem, dartforge_build::motor::Origem::Nativo(_)) || f.fabrica != "templateCompiler" {
                continue;
            }
            let dart = raiz.join(a.entrada.caminho.as_ref());
            nativos.insert(dart.clone());
            let html = dart.with_extension("html");
            let scss = dart.with_extension("scss");
            let nome_html = html.file_name().unwrap().to_string_lossy().to_string();
            let usa = std::fs::read_to_string(&dart)
                .is_ok_and(|t| t.contains("@Component") && t.contains(&nome_html) && t.contains("styleUrls"));
            if comp.is_none() && a.entrada.caminho.starts_with("lib/") && html.is_file() && scss.is_file() && usa {
                comp = Some((dart, html, scss));
            }
        }
        // Um `.dart` pequeno do pacote, sem Angular: o gerador o considera
        // trivial.
        let sem = nativos
            .iter()
            .find(|p| {
                p.to_string_lossy().contains("/src/")
                    && std::fs::metadata(p).is_ok_and(|m| m.len() < 8000)
                    && std::fs::read_to_string(p).is_ok_and(|t| {
                        !t.contains("@Component")
                            && !t.contains("@Directive")
                            && !t.contains("@Pipe")
                            && !t.contains("@Injectable")
                            && !t.contains("GenerateInjector")
                            && !t.contains("part ")
                            && t.contains("class ")
                    })
            })
            .cloned();
        (comp.expect("nenhum componente coberto com .html e .scss"), sem)
    };
    let (dart, html, scss) = componente;
    let _ = sem_motor;
    println!("componente: {}", dart.display());
    if let Some(s) = &sem_angular {
        println!("sem Angular: {}", s.display());
    }
    println!(
        "{:<34}{:>9}{:>9}{:>9}{:>9}{:>9}{:>8}{:>8}{:>9}{:>9}",
        "edição", "total", "motor", "nativo", "recarga", "escrita", "ações", "saídas", "unidades", "módulos"
    );
    let mut edicoes: Vec<(&str, PathBuf, String)> = vec![
        ("texto no .html", html.clone(), "\n<span>medida</span>\n".into()),
        ("propriedade no .scss", scss.clone(), "\n.medida-x { color: red; }\n".into()),
        ("corpo fora do template (.dart)", dart.clone(), "\nvoid _medida() { var x = 1; x++; }\n".into()),
    ];
    if let Some(s) = sem_angular {
        edicoes.push(("corpo em .dart sem Angular", s, "\nvoid _medidaSemAngular() { var x = 1; x++; }\n".into()));
    }
    for (nome, arq, acrescimo) in edicoes {
        let original = std::fs::read(&arq).expect("ler");
        for i in 0..repeticoes {
            let mut novo = original.clone();
            novo.extend_from_slice(acrescimo.as_bytes());
            std::fs::write(&arq, &novo).unwrap();
            for p in sessao.mudancas() {
                sessao.arquivo_mudou(&p);
            }
            sessao.arquivo_mudou(&arq);
            let r = sessao.compilar().expect("edição");
            linha(&format!("{nome} #{i}"), &r);
            if let (true, Some(motor)) = (std::env::var("MEDIR_DEPURAR").is_ok(), motor.as_ref()) {
                let m = motor.lock().unwrap();
                for (a, acao) in m.grafo.acoes.iter().enumerate() {
                    if raiz.join(acao.entrada.caminho.as_ref()) == dart || raiz.join(acao.entrada.caminho.as_ref()).with_extension("") == dart.with_extension("") {
                        if let Some(r) = m.registro(a) {
                            println!("    {} {:?} {:?}", acao.entrada.caminho, r.origem, r.motivo);
                        }
                    }
                }
                print!("{}", r.texto());
            }
            std::fs::write(&arq, &original).unwrap();
            for p in sessao.mudancas() {
                sessao.arquivo_mudou(&p);
            }
            sessao.arquivo_mudou(&arq);
            let r = sessao.compilar().expect("reversão");
            linha(&format!("  revertida #{i}"), &r);
        }
    }
    let _ = Path::new("");
}
