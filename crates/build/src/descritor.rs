//! Descritores: o que o motor sabe de cada builder conhecido **em tempo de
//! execução**, que o `build.yaml` não diz.
//!
//! As extensões que valem são as do objeto `Builder` (`buildExtensions`),
//! não as do `build.yaml` (que só servem para ordenar builders): o
//! `templatePlaceholder` do ngdart escreve `.ng_placeholder`, que o
//! `build.yaml` não declara; o `combining_builder` e o `mockBuilder` aceitam
//! `build_extensions` nas opções. Sem descritor, valem as do `build.yaml` —
//! certo por convenção no `source_gen`, e divergência aparece no corpus.
use crate::valor::{Mapa, Valor};

/// Builders que o DartForge substitui em vez de executar
/// (`docs/BUILD-MOTOR.md` §6): ficam no plano, não geram ação.
pub fn substituido(chave: &str) -> Option<&'static str> {
    let pacote = chave.split(':').next().unwrap_or_default();
    match pacote {
        "build_web_compilers" | "build_modules" => {
            Some("compilação para JavaScript: é o compilador que o DartForge substitui (BUILD-RUST.md §4)")
        }
        "build_resolvers" => Some("cache do analyzer: substituído pelo banco semântico"),
        "build_test" => Some("bootstrap de teste: só em `dartforge test`"),
        _ => None,
    }
}

/// Versões do pacote oficial que um gerador nativo imita (conferidas contra
/// o `pubspec.lock`).
pub fn imita(chave: &str) -> &'static [(&'static str, &'static str)] {
    match chave {
        "ngdart:ngdart" => &[("ngdart", "8.0.0-dev.4")],
        "sass_builder:sass_builder" => &[("sass_builder", "2.2.1")],
        _ => &[],
    }
}

fn mapa_de_extensoes(v: &Valor, lista_ou_texto: bool) -> Result<Vec<(String, Vec<String>)>, String> {
    let Valor::Mapa(m) = v else {
        return Err("build_extensions should be a map from inputs to outputs".into());
    };
    let mut r = Vec::new();
    for (k, s) in &m.0 {
        let Some(k) = k.como_texto() else {
            return Err("chave de build_extensions não é texto".into());
        };
        let saidas: Vec<String> = match s {
            Valor::Texto(t) => vec![t.clone()],
            Valor::Lista(l) if lista_ou_texto => {
                l.iter().map(|x| x.como_texto().map(str::to_string)).collect::<Option<_>>().ok_or("saída não é texto")?
            }
            _ => return Err(format!("valor inválido em build_extensions para `{k}`")),
        };
        r.push((k.to_string(), saidas));
    }
    if r.is_empty() {
        return Err("Configured build_extensions must not be empty.".into());
    }
    Ok(r)
}

/// Extensões de execução da fábrica `fabrica` do builder `chave` com as
/// opções efetivas.
pub fn extensoes_de_execucao(
    chave: &str,
    fabrica: &str,
    opcoes: &Mapa,
    declaradas: &[(String, Vec<String>)],
) -> Result<Vec<(String, Vec<String>)>, String> {
    let e = |pares: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        pares.iter().map(|(a, b)| (a.to_string(), b.iter().map(|s| s.to_string()).collect())).collect()
    };
    Ok(match (chave, fabrica) {
        // `ngdart-8.0.0-dev.4/lib/src/build.dart:33-76`,
        // `ngcompiler-3.0.0-dev.3` (Placeholder, Compiler.asBuilder,
        // StylesheetCompiler).
        ("ngdart:ngdart", "templatePlaceholder") => e(&[(".dart", &[".ng_placeholder"])]),
        ("ngdart:ngdart", "templateCompiler") => {
            if opcoes.obter("outline-only").is_some() {
                e(&[(".dart", &[".outline.template.dart"])])
            } else {
                e(&[(".dart", &[".template.dart"])])
            }
        }
        ("ngdart:ngdart", "stylesheetCompiler") => e(&[(".css", &[".css.shim.dart", ".css.dart"])]),
        // `sass_builder-2.2.1/lib/sass_builder.dart:130-132`.
        ("sass_builder:sass_builder", _) => e(&[(".scss", &[".css", ".css.map"]), (".sass", &[".css", ".css.map"])]),
        // `source_gen-2.0.0/lib/builder.dart` (`validatedBuildExtensionsFrom`).
        ("source_gen:combining_builder", _) => match opcoes.obter("build_extensions") {
            Some(v) => mapa_de_extensoes(v, true)?,
            None => e(&[(".dart", &[".g.dart"])]),
        },
        // `mockito-5.4.4/lib/src/builder.dart:2281-2303`.
        ("mockito:mockBuilder", _) => match opcoes.obter("build_extensions") {
            Some(v) => mapa_de_extensoes(v, false)?,
            None => e(&[(".dart", &[".mocks.dart"])]),
        },
        _ => declaradas.to_vec(),
    })
}
