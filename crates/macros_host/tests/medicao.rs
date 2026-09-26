//! Medição do hospedeiro de macros (docs/MACROS-PROTOCOLO.md §8): o tempo de
//! execução das macros, das consultas, da montagem e da recarga, separados,
//! no `corpus/macros/410_json_codable` e num caso sintético com `N` classes
//! `@JsonCodable` numa biblioteca só (onde a remontagem acumulada da fase de
//! declarações aparece).
//!
//! Mede desempenho, não correção (CONTRIBUTING.md): o teste só imprime os
//! números e confere que a sessão terminou. Exige a VM Dart 3.6.2
//! (`DARTFORGE_DART_SDK`) e o `pub get` de `corpus/macros/410_json_codable`:
//!
//! ```text
//! cargo test --release -p dartforge-macros-host --test medicao -- --ignored --nocapture
//! ```
use dartforge_elements::load::load_lenient_gerados;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use dartforge_macros_host::modelo::Vista;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sdk() -> SdkLayout {
    let lib = SdkLayout::discover().expect("SDK (DARTFORGE_SDK_LIB)");
    let mut sdk = SdkLayout::load(&lib, "dartdevc").unwrap();
    sdk.versao_corrente = LanguageVersion::new(3, 6);
    sdk.experimentos = vec![Feature::Macros];
    sdk
}

fn dart() -> PathBuf {
    std::env::var_os("DARTFORGE_DART_SDK")
        .map(PathBuf::from)
        .or_else(|| SdkLayout::discover().and_then(|l| l.parent().map(Path::to_path_buf)))
        .map(|s| s.join("bin").join(if cfg!(windows) { "dart.exe" } else { "dart" }))
        .expect("DARTFORGE_DART_SDK")
}

/// Uma compilação inteira com a VM como executor; imprime a medição.
fn medir(nome: &str, entrada: &Path, config: &Path, sdk: &SdkLayout) {
    let mut nomes = Interner::new();
    let (p, d) = load_lenient_gerados(entrada, sdk, Some(config), &mut nomes, None, None, None);
    assert!(d.is_empty(), "{d:?}");
    let apps = dartforge_macros_host::aplicacoes::detectar(&Vista { program: &p, interner: &nomes });
    let trabalho = tempfile::tempdir().unwrap();
    let cfg = dartforge_macros_host::vm::ConfigDaVm {
        dart: dart(),
        api: raiz().join("pacotes/macros"),
        trabalho: trabalho.path().to_path_buf(),
    };
    let t = Instant::now();
    let mut vm = dartforge_macros_host::vm::iniciar(&cfg, &apps, Some(config)).unwrap();
    let mut carregar = |i: &mut Interner, g| load_lenient_gerados(entrada, sdk, Some(config), i, None, None, g);
    let saida = match dartforge_macros_host::aplicar(p, &mut nomes, None, &mut carregar, &mut vm) {
        Ok(s) => s,
        Err(ds) => panic!("{}", ds.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n")),
    };
    let total = t.elapsed();
    let linhas: usize = saida.textos.iter().map(|t| t.texto.lines().count()).sum();
    eprintln!(
        "[medição] {nome}: {} aplicações, {linhas} linhas geradas, total {:.1} ms\n[medição] {nome}: {}",
        apps.len(),
        total.as_secs_f64() * 1000.0,
        saida.medicao.resumo()
    );
}

/// `n` classes `@JsonCodable` com três campos cada, numa biblioteca.
fn sintetico(dir: &Path, n: usize) -> PathBuf {
    let mut s = String::from("import 'package:json/json.dart';\n\n");
    for i in 0..n {
        s.push_str(&format!(
            "@JsonCodable()\nclass C{i} {{\n  final String nome;\n  final int? valor;\n  final List<int> notas;\n}}\n\n"
        ));
    }
    s.push_str("void main() {\n  print(C0.fromJson({'nome': 'a', 'notas': <int>[]}).toJson());\n}\n");
    let entrada = dir.join("main.dart");
    std::fs::write(&entrada, s).unwrap();
    entrada
}

#[test]
#[ignore = "medição: exige a VM Dart 3.6.2 (DARTFORGE_DART_SDK) e o pub get de corpus/macros/410_json_codable"]
fn medicao_do_hospedeiro() {
    let sdk = sdk();
    let caso = raiz().join("corpus/macros/410_json_codable");
    let config = caso.join(".dart_tool/package_config.json");
    assert!(config.is_file(), "falta o pub get de {}", caso.display());
    medir("410_json_codable", &caso.join("main.dart"), &config, &sdk);
    let n: Vec<usize> = std::env::var("DARTFORGE_MEDICAO_N")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_else(|| vec![8, 32]);
    for n in n {
        let dir = tempfile::tempdir().unwrap();
        let entrada = sintetico(dir.path(), n);
        medir(&format!("sintético N={n}"), &entrada, &config, &sdk);
    }
}
