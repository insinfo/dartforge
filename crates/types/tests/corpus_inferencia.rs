//! Conformidade da inferência com o oráculo (`package:analyzer`): despeja
//! cada programa de `corpus/inferencia/` e compara com o `.esperado.tsv`
//! gravado pelo oráculo (docs/INFERENCIA-ESPECIFICACAO.md, §0).
//!
//! As divergências e os avisos conhecidos ficam em
//! `corpus/inferencia/divergencias.txt` (catraca): o teste falha com
//! divergência ou aviso novo, e também com item da lista que deixou de
//! acontecer (a lista só encolhe). `ATUALIZAR_DIVERGENCIAS=1` regrava a lista.

use dartforge_elements::sdk::SdkLayout;
use dartforge_types::despejo::{bloco_de_esperado, bloco_de_linhas, comparar_bloco, despejar};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn sdk() -> Option<SdkLayout> {
    let dir = SdkLayout::discover().or_else(|| {
        let p = PathBuf::from("C:/tools/dartsdk-3.6.2/lib");
        p.join("libraries.json").exists().then_some(p)
    })?;
    SdkLayout::load(&dir, "dartdevc").ok()
}

fn programas(dir: &Path, v: &mut Vec<PathBuf>) {
    let mut es: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
    es.sort();
    for p in es {
        if p.is_dir() {
            programas(&p, v);
        } else if p.extension().is_some_and(|x| x == "dart") {
            v.push(p);
        }
    }
}

#[test]
fn corpus_inferencia_contra_o_oraculo() {
    let Some(sdk) = sdk() else {
        eprintln!("SDK do Dart ausente: teste pulado");
        return;
    };
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/inferencia");
    let mut progs = Vec::new();
    programas(&raiz, &mut progs);
    assert!(!progs.is_empty(), "corpus vazio em {}", raiz.display());
    let lista_path = raiz.join("divergencias.txt");

    let (atual, resumo) = std::thread::Builder::new()
        .stack_size(1 << 28)
        .spawn(move || {
            let mut atual: BTreeSet<String> = BTreeSet::new();
            let (mut comparadas, mut divergentes, mut avisos, mut iguais) = (0usize, 0usize, 0usize, 0usize);
            for prog in &progs {
                let rel = prog.strip_prefix(&raiz).unwrap().to_string_lossy().replace('\\', "/");
                let esperado = prog.with_extension("esperado.tsv");
                let Ok(texto) = std::fs::read_to_string(&esperado) else {
                    panic!("{rel}: falta {}", esperado.display());
                };
                let d = despejar(prog, &sdk, None);
                let caminho = std::fs::canonicalize(prog).unwrap_or(prog.clone());
                let alvo = caminho.to_string_lossy().replace('\\', "/");
                let alvo = alvo.trim_start_matches("//?/");
                let Some(u) = d.unidades.iter().find(|u| u.caminho.eq_ignore_ascii_case(alvo)).or(d.unidades.first()) else {
                    panic!("{rel}: unidade não despejada");
                };
                let c = comparar_bloco(&bloco_de_linhas(&u.linhas), &bloco_de_esperado(&texto));
                comparadas += c.comparadas;
                divergentes += c.divergencias.len();
                for x in &c.divergencias {
                    atual.insert(format!("{rel}:{}:{}\t{}\tnós={}\toráculo={}", x.ini, x.fim - x.ini, x.no, x.nosso, x.oraculo));
                }
                for a in &d.avisos {
                    avisos += 1;
                    atual.insert(format!("{rel}:aviso:{}\t{}", a.span.start, a.message.replace(['\n', '\t'], " ")));
                }
                if c.divergencias.is_empty() && d.avisos.is_empty() {
                    iguais += 1;
                }
            }
            let resumo = format!(
                "programas iguais ao oráculo: {iguais}/{}; expressões comparadas: {comparadas}; divergentes: {divergentes}; avisos: {avisos}",
                progs.len()
            );
            (atual, resumo)
        })
        .unwrap()
        .join()
        .unwrap();
    eprintln!("{resumo}");

    if std::env::var_os("ATUALIZAR_DIVERGENCIAS").is_some() {
        let mut s = String::from(
            "# Divergências e avisos conhecidos do corpus/inferencia contra o oráculo\n\
             # (catraca de crates/types/tests/corpus_inferencia.rs; só encolhe).\n",
        );
        s.push_str(&format!("# {resumo}\n"));
        for l in &atual {
            s.push_str(l);
            s.push('\n');
        }
        std::fs::write(&lista_path, s).unwrap();
        return;
    }
    let conhecidas: BTreeSet<String> = std::fs::read_to_string(&lista_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(String::from)
        .collect();
    let novas: Vec<&String> = atual.difference(&conhecidas).collect();
    let resolvidas: Vec<&String> = conhecidas.difference(&atual).collect();
    assert!(
        novas.is_empty() && resolvidas.is_empty(),
        "corpus/inferencia mudou ({resumo}).\nnovas ({}):\n{}\nresolvidas, tirar da lista ({}):\n{}\n\
         (ATUALIZAR_DIVERGENCIAS=1 regrava corpus/inferencia/divergencias.txt)",
        novas.len(),
        novas.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
        resolvidas.len(),
        resolvidas.iter().map(|s| format!("  {s}")).collect::<Vec<_>>().join("\n"),
    );
}
