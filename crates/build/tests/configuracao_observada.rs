//! DF-BUILD-018: o motor observa a configuração que muda o plano também
//! fora do `build.yaml` da raiz — o `build.yaml` de cada dependência (exista
//! ou não) e o diretório raiz (um override `<pacote>.build.yaml` que aparece
//! muda a marca dele) — e, quando ela muda, refaz o motor relendo o
//! `package_config.json` (DF-BUILD-010).
//!
//! Usa `corpus/builders/i18n` (pede `dart pub get` no caso); sem o
//! `package_config.json`, o teste diz e sai.
use dartforge_build::{Motor, OpcoesMotor};
use dartforge_elements::config::PackageConfig;
use std::path::Path;

#[test]
fn observa_build_yaml_das_dependencias_e_overrides() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/builders/i18n");
    let arquivo = raiz.join(".dart_tool/package_config.json");
    if !arquivo.is_file() {
        eprintln!("sem {}: rode `dart pub get` no caso", arquivo.display());
        return;
    }
    let cfg = PackageConfig::load(&arquivo).unwrap();
    let m = Motor::novo(&raiz, &cfg, OpcoesMotor::default()).expect("motor");
    let observados = m.observados();
    let tem = |f: &dyn Fn(&Path) -> bool| observados.iter().any(|p| f(p));
    // O `build.yaml` do pacote `i18n` (dependência, no pub cache).
    assert!(
        tem(&|p| p.ends_with("build.yaml") && p.to_string_lossy().contains("i18n-4.2.1")),
        "build.yaml do i18n fora dos observados: {observados:?}"
    );
    // O de uma dependência sem `build.yaml` também (criá-lo muda o plano).
    let sem: Vec<_> = observados
        .iter()
        .filter(|p| p.ends_with("build.yaml") && !p.is_file())
        .collect();
    assert!(!sem.is_empty(), "nenhum build.yaml ausente observado");
    // O diretório raiz (overrides `<pacote>.build.yaml`).
    let raiz_canonica = dartforge_elements::gerado::chave(&raiz);
    assert!(tem(&|p| *p == raiz_canonica), "raiz fora dos observados");
}
