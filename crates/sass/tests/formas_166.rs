//! O modo de compatibilidade com o dart-sass 1.66.0
//! (`VersaoDartSass::V1_66_0`) contra o dart-sass 1.66.0 de verdade, forma a
//! forma e nos dois estilos (`tests/formas_166`, oráculo gravado por
//! `scripts/sass-formas-166.sh`); e as mesmas formas no modo padrão contra o
//! dart-sass 1.102.0 (`esperado_102/`), para as diferenças entre as versões
//! ficarem conferidas dos dois lados.
//!
//! Critério: cada forma `f*` sai **exatamente** com os bytes do oráculo, ou
//! é recusada quando ele a recusa (`.erro`). As formas `r*` o 1.66 compila,
//! mas o modo 1.66 não garante a saída e tem de recusar — nunca gerar outra
//! coisa.
use dartforge_sass::{from_path, Options, OutputStyle, VersaoDartSass};
use std::path::Path;

#[test]
fn formas_iguais_ao_dart_sass_166() {
    conferir("esperado", VersaoDartSass::V1_66_0);
}

/// Formas em que o port do 1.102 ainda sai como o 1.66 (divergências
/// conhecidas do modo padrão, não do modo 1.66): o valor de at-rule
/// desconhecida pelo `almostAnyValue` antigo, que guarda o `//` (o 1.102 usa
/// o `_interpolatedDeclarationValue`, dart-sass 1.77.7), e o `&` na raiz
/// (dart-sass 1.99), que o port recusa. O teste exige que continuem
/// divergindo: quando uma for corrigida, sai daqui.
const PENDENTES_102: &[&str] = &["f24_comentario_at_rule", "f26_pai_na_raiz"];

#[test]
fn formas_iguais_ao_dart_sass_102() {
    conferir("esperado_102", VersaoDartSass::V1_102_0);
}

fn conferir(dir: &str, versao: VersaoDartSass) {
    let pendentes: &[&str] = if versao == VersaoDartSass::V1_102_0 {
        PENDENTES_102
    } else {
        &[]
    };
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/formas_166");
    let mut formas: Vec<_> = std::fs::read_dir(base.join("fontes"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    formas.sort();
    assert!(formas.len() >= 25, "formas: {}", formas.len());
    let mut diferentes = Vec::new();
    let mut iguais = 0;
    let mut pendentes_iguais = Vec::new();
    for f in &formas {
        let forma = f.file_stem().unwrap().to_string_lossy().to_string();
        let pendente = pendentes.contains(&forma.as_str());
        let antes = diferentes.len();
        let recusa_do_modo = forma.starts_with('r') && versao == VersaoDartSass::V1_66_0;
        let erro_do_oficial = base.join(dir).join(format!("{forma}.erro")).is_file();
        for (estilo, nome) in [
            (OutputStyle::Expanded, "expanded"),
            (OutputStyle::Compressed, "compressed"),
        ] {
            let opcoes = Options::default().style(estilo).quiet(true).versao(versao);
            let r = from_path(f, &opcoes);
            let esperado =
                std::fs::read_to_string(base.join(dir).join(format!("{forma}.{nome}.css")))
                    .ok()
                    .map(|mut css| {
                        // O `from_path` termina o `expanded` em `\n`.
                        if !css.is_empty() && estilo == OutputStyle::Expanded {
                            css.push('\n');
                        }
                        css
                    });
            match (r, esperado) {
                (Err(_), _) if recusa_do_modo => iguais += 1,
                (Ok(css), _) if recusa_do_modo => diferentes.push(format!(
                    "{forma} ({nome}): o modo devia recusar e gerou {css:?}"
                )),
                (Err(_), None) if erro_do_oficial => iguais += 1,
                (Ok(css), Some(e)) if css == e => iguais += 1,
                (Ok(css), Some(e)) => {
                    diferentes.push(format!("{forma} ({nome}): modo {css:?} ≠ oráculo {e:?}"))
                }
                (Ok(css), None) => diferentes.push(format!(
                    "{forma} ({nome}): o oráculo recusa e o modo gerou {css:?}"
                )),
                (Err(e), _) => diferentes.push(format!(
                    "{forma} ({nome}): o oráculo compila e o modo recusou: {e}"
                )),
            }
        }
        if pendente {
            if diferentes.len() == antes {
                pendentes_iguais.push(forma);
            }
            diferentes.truncate(antes);
        }
    }
    println!("{iguais} iguais de {}", formas.len() * 2);
    assert!(diferentes.is_empty(), "{}", diferentes.join("\n"));
    assert!(
        pendentes_iguais.is_empty(),
        "pendentes que já saem iguais (tire-os da lista): {pendentes_iguais:?}"
    );
}
