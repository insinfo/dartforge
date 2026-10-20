//! Descriptors: what the engine knows about each known builder **at run
//! time**, which `build.yaml` does not say.
//!
//! The extensions that count are those of the `Builder` object
//! (`buildExtensions`), not those of `build.yaml` (which only serve to order
//! builders): ngdart's `templatePlaceholder` writes `.ng_placeholder`, which
//! `build.yaml` does not declare; `combining_builder` and `mockBuilder` accept
//! `build_extensions` in the options. Without a descriptor, the `build.yaml`
//! ones apply — correct by convention in `source_gen`, and any divergence
//! shows up in the corpus.
use crate::value::{Map, Value};

/// Builders that DartForge replaces instead of running
/// (`docs/BUILD-MOTOR.md` §6): they stay in the plan but produce no action.
pub fn replaced_by(key: &str) -> Option<&'static str> {
    let package = key.split(':').next().unwrap_or_default();
    match package {
        "build_web_compilers" | "build_modules" => Some(
            "compilation to JavaScript: this is the compiler DartForge replaces (BUILD-RUST.md §4)",
        ),
        "build_resolvers" => Some("analyzer cache: replaced by the semantic database"),
        "build_test" => Some("test bootstrap: only in `dartforge test`"),
        _ => None,
    }
}

/// Versions of the official package that a native generator imitates
/// (checked against `pubspec.lock`): for each package, the accepted versions
/// — the generator only covers the phase when the lock brings one of them,
/// and it picks its mode by the one that came
/// ([`crate::executor::NativeRequest::versions`]).
pub fn imitates(key: &str) -> &'static [(&'static str, &'static [&'static str])] {
    match key {
        "ngdart:ngdart" => &[("ngdart", &["8.0.0-dev.4"])],
        // The fork with `package:web` (`ngx_compiler` 9.0.0-dev.2): the same
        // generator, in dialect 9 (`docs/NGDART-COMPILADOR-DE-VISOES.md`).
        "ngx_dart:ngx_dart" => &[("ngx_dart", &["9.0.0-dev.2"])],
        // Sass is the one from `dartforge-sass`: the port of dart-sass 1.102.0
        // and the compatibility modes for 1.101.0–1.101.3 (new_sali and
        // limitless_ui) and for 1.66.0 (the one of ngcomponents 3.0.0-dev.1).
        // Another `sass` version in the lock changes the output.
        "sass_builder:sass_builder" => &[
            ("sass_builder", &["2.2.1"]),
            (
                "sass",
                &[
                    "1.102.0", "1.101.0", "1.101.1", "1.101.2", "1.101.3", "1.66.0",
                ],
            ),
        ],
        "i18n:yamlBasedBuilder" => &[("i18n", &["4.2.1"])],
        _ => &[],
    }
}

fn extensions_map(v: &Value, list_or_string: bool) -> Result<Vec<(String, Vec<String>)>, String> {
    let Value::Map(m) = v else {
        return Err("build_extensions should be a map from inputs to outputs".into());
    };
    let mut r = Vec::new();
    for (k, s) in &m.0 {
        let Some(k) = k.as_text() else {
            return Err("build_extensions key is not a string".into());
        };
        let outputs: Vec<String> = match s {
            Value::Text(t) => vec![t.clone()],
            Value::List(l) if list_or_string => l
                .iter()
                .map(|x| x.as_text().map(str::to_string))
                .collect::<Option<_>>()
                .ok_or("output is not a string")?,
            _ => return Err(format!("invalid value in build_extensions for `{k}`")),
        };
        r.push((k.to_string(), outputs));
    }
    if r.is_empty() {
        return Err("Configured build_extensions must not be empty.".into());
    }
    Ok(r)
}

/// Run-time extensions of the factory `factory` of the builder `key` with the
/// effective options.
pub fn runtime_extensions(
    key: &str,
    factory: &str,
    factories: &[String],
    options: &Map,
    declared: &[(String, Vec<String>)],
) -> Result<Vec<(String, Vec<String>)>, String> {
    let e = |pairs: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.iter().map(|s| s.to_string()).collect()))
            .collect()
    };
    Ok(match (key, factory) {
        // `ngdart-8.0.0-dev.4/lib/src/build.dart:33-76`,
        // `ngcompiler-3.0.0-dev.3` (Placeholder, Compiler.asBuilder,
        // StylesheetCompiler).
        ("ngdart:ngdart" | "ngx_dart:ngx_dart", "templatePlaceholder") => {
            e(&[(".dart", &[".ng_placeholder"])])
        }
        ("ngdart:ngdart" | "ngx_dart:ngx_dart", "templateCompiler") => {
            if options.get("outline-only").is_some() {
                e(&[(".dart", &[".outline.template.dart"])])
            } else {
                e(&[(".dart", &[".template.dart"])])
            }
        }
        ("ngdart:ngdart" | "ngx_dart:ngx_dart", "stylesheetCompiler") => {
            e(&[(".css", &[".css.shim.dart", ".css.dart"])])
        }
        // `sass_builder-2.2.1/lib/sass_builder.dart:130-132`.
        ("sass_builder:sass_builder", _) => e(&[
            (".scss", &[".css", ".css.map"]),
            (".sass", &[".css", ".css.map"]),
        ]),
        // `source_gen-2.0.0/lib/builder.dart` (`validatedBuildExtensionsFrom`).
        ("source_gen:combining_builder", _) => match options.get("build_extensions") {
            Some(v) => extensions_map(v, true)?,
            None => e(&[(".dart", &[".g.dart"])]),
        },
        // `mockito-5.4.4/lib/src/builder.dart:2281-2303`.
        ("mockito:mockBuilder", _) => match options.get("build_extensions") {
            Some(v) => extensions_map(v, false)?,
            None => e(&[(".dart", &[".mocks.dart"])]),
        },
        // `SharedPartBuilder(…, 'json_serializable')` writes
        // `.json_serializable.g.part`; the 6.9.5 `build.yaml` declares it
        // without the dot.
        ("json_serializable:json_serializable", _) => {
            e(&[(".dart", &[".json_serializable.g.part"])])
        }
        // `drift_dev-2.28.0/lib/src/backends/build/*.dart`.
        ("drift_dev:preparing_builder", _) => e(&[
            (".moor", &[".expr.temp.dart", ".drift_prep.json"]),
            (".drift", &[".expr.temp.dart", ".drift_prep.json"]),
        ]),
        ("drift_dev:drift_dev" | "drift_dev:analyzer", "discover") => e(&[
            (".drift", &[".drift.drift_elements.json"]),
            (".dart", &[".dart.drift_elements.json"]),
        ]),
        ("drift_dev:drift_dev" | "drift_dev:analyzer", "analyzer") => e(&[
            (
                ".drift",
                &[".drift.drift_module.json", ".drift.types.temp.dart"],
            ),
            (
                ".dart",
                &[".dart.drift_module.json", ".dart.types.temp.dart"],
            ),
        ]),
        ("drift_dev:drift_dev", "driftBuilder") => e(&[(".dart", &[".drift.g.part"])]),
        ("drift_dev:not_shared", _) => e(&[(".dart", &[".drift.dart"])]),
        ("drift_dev:modular", _) => e(&[(".dart", &[".drift.dart"]), (".drift", &[".drift.dart"])]),
        // No descriptor and several factories: each factory is a `Builder`
        // with its own extensions, which only the Dart executor reveals. The
        // declared outputs go to the first factory (declared limitation).
        _ if factories.len() > 1 && factories.first().map(String::as_str) != Some(factory) => {
            Vec::new()
        }
        _ => declared.to_vec(),
    })
}
