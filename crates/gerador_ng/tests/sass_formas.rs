//! O Sass nativo (`sass::compilar_com`) contra o `sass_builder` 2.2.1 de
//! verdade, forma a forma e nos dois estilos (`tests/sass_formas`, oráculo
//! gravado por `scripts/sass-formas.sh` com o dart-sass 1.102.0 do lock).
//!
//! Critério: para cada forma e estilo, o nativo devolve **exatamente** os
//! bytes que o builder escreveu, ou recusa. Uma forma que o oficial rejeita
//! (`.erro`) tem de ser recusada. Nunca uma saída aproximada.
use dartforge_gerador_ng::sass::{compilar_com, Estilo};
use std::path::Path;

#[test]
fn formas_sass_iguais_ao_sass_builder_ou_recusadas() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/sass_formas");
    let fontes = base.join("fontes");
    let mut formas: Vec<_> = std::fs::read_dir(&fontes)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('f')))
        .collect();
    formas.sort();
    assert!(formas.len() >= 40, "formas: {}", formas.len());
    let mut linhas = Vec::new();
    let mut diferentes = Vec::new();
    for (estilo, nome) in [(Estilo::Comprimido, "compressed"), (Estilo::Expandido, "expanded")] {
        let (mut iguais, mut recusadas) = (0, Vec::new());
        for f in &formas {
            let forma = f.file_stem().unwrap().to_string_lossy().to_string();
            let texto = std::fs::read_to_string(f).unwrap();
            let esperado = std::fs::read(base.join("esperado").join(format!("{forma}.{nome}.css"))).ok();
            match (compilar_com(&texto, Some(&fontes), estilo), esperado) {
                (Ok((css, _)), Some(e)) if css.as_bytes() == e.as_slice() => iguais += 1,
                (Ok((css, _)), Some(e)) => diferentes.push(format!(
                    "{forma} ({nome}): nativo {css:?} ≠ oficial {:?}",
                    String::from_utf8_lossy(&e)
                )),
                (Ok(_), None) => diferentes.push(format!("{forma} ({nome}): o oficial rejeita e o nativo gerou")),
                (Err(_), _) => recusadas.push(forma),
            }
        }
        linhas.push(format!(
            "{nome}: {iguais} iguais, {} recusadas, de {} — recusadas: {}",
            recusadas.len(),
            formas.len(),
            recusadas.join(" ")
        ));
    }
    println!("{}", linhas.join("\n"));
    assert!(diferentes.is_empty(), "{}", diferentes.join("\n"));
}
