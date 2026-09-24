//! Versão de linguagem por biblioteca (`docs/VERSOES-LINGUAGEM.md` §2): o
//! marcador `// @dart = x.y` vale mais que o `languageVersion` do pacote, que
//! vale mais que a versão corrente; partes seguem a biblioteca; `dart:*` fica
//! no piso.
use dartforge_elements::load::load_lenient;
use dartforge_elements::model::Program;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn sdk_falso(dir: &Path) -> SdkLayout {
    let lib_dir = dir.join("lib");
    fs::create_dir_all(lib_dir.join("core")).unwrap();
    fs::write(
        lib_dir.join("libraries.json"),
        r#"{ "dartdevc": { "libraries": { "core": { "uri": "core/core.dart", "patches": [] } } } }"#,
    )
    .unwrap();
    fs::write(
        lib_dir.join("core/core.dart"),
        "library dart.core; class Object {} class int extends Object {} class String extends Object {} class bool extends Object {}",
    )
    .unwrap();
    SdkLayout::load(&lib_dir, "dartdevc").unwrap()
}

fn versao_de(p: &Program, trecho: &str) -> LanguageVersion {
    p.libraries
        .iter()
        .find(|l| l.uri.contains(trecho))
        .unwrap_or_else(|| panic!("{trecho}"))
        .features
        .versao()
}

fn v(a: u16, b: u16) -> LanguageVersion {
    LanguageVersion::new(a, b)
}

#[test]
fn sem_marcador_e_sem_pacote_vale_a_corrente() {
    let tmp = tempdir().unwrap();
    let mut sdk = sdk_falso(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(&main, "void main() {}").unwrap();
    let (p, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(versao_de(&p, "main.dart"), LanguageVersion::ATUAL);
    assert_eq!(versao_de(&p, "dart:core"), LanguageVersion::PISO);
    assert!(
        p.library(p.entry.unwrap())
            .features
            .tem(Feature::PrimaryConstructors)
    );

    sdk.versao_corrente = v(3, 6);
    let (p, _) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert_eq!(versao_de(&p, "main.dart"), v(3, 6));
    assert!(
        !p.library(p.entry.unwrap())
            .features
            .tem(Feature::WildcardVariables)
    );
}

#[test]
fn marcador_vale_mais_que_o_pacote() {
    let tmp = tempdir().unwrap();
    let sdk = sdk_falso(tmp.path());
    let proj = tmp.path().join("app");
    fs::create_dir_all(proj.join(".dart_tool")).unwrap();
    fs::create_dir_all(proj.join("bin")).unwrap();
    fs::create_dir_all(proj.join("lib")).unwrap();
    fs::write(
        proj.join(".dart_tool/package_config.json"),
        r#"{ "configVersion": 2, "packages": [
            { "name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.8" }
        ] }"#,
    )
    .unwrap();
    // `bin/` pertence ao pacote pela raiz; `lib/` pela URI `package:`.
    fs::write(
        proj.join("bin/main.dart"),
        "import 'package:app/a.dart';\nimport 'package:app/b.dart';\nvoid main() {}",
    )
    .unwrap();
    fs::write(proj.join("lib/a.dart"), "int a = 1;").unwrap();
    fs::write(
        proj.join("lib/b.dart"),
        "// licença\n// @dart = 3.6\nint b = 1;",
    )
    .unwrap();
    let (p, d) = load_lenient(
        &proj.join("bin/main.dart"),
        &sdk,
        None,
        &mut Interner::new(),
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(versao_de(&p, "main.dart"), v(3, 8));
    assert_eq!(versao_de(&p, "package:app/a.dart"), v(3, 8));
    assert_eq!(versao_de(&p, "package:app/b.dart"), v(3, 6));
}

#[test]
fn parte_com_versao_diferente_e_erro() {
    let tmp = tempdir().unwrap();
    let sdk = sdk_falso(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(
        &main,
        "// @dart=3.10\npart 'p.dart';\npart 'q.dart';\nvoid main() {}",
    )
    .unwrap();
    fs::write(
        tmp.path().join("p.dart"),
        "// @dart=3.10\npart of 'main.dart';\nint p = 1;",
    )
    .unwrap();
    fs::write(
        tmp.path().join("q.dart"),
        "part of 'main.dart';\nint q = 1;",
    )
    .unwrap();
    let (p, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert_eq!(versao_de(&p, "main.dart"), v(3, 10));
    // `q.dart` sem marcador fica na corrente (3.13), diferente da biblioteca.
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message.contains("q.dart")
            && d[0]
                .message
                .contains("a parte está na versão de linguagem 3.13"),
        "{}",
        d[0].message
    );
}

#[test]
fn marcador_fora_do_intervalo_e_erro() {
    let tmp = tempdir().unwrap();
    let sdk = sdk_falso(tmp.path());
    let alto = tmp.path().join("alto.dart");
    fs::write(&alto, "// @dart=3.99\nvoid main() {}").unwrap();
    let (p, d) = load_lenient(&alto, &sdk, None, &mut Interner::new());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message.contains("acima da suportada"),
        "{}",
        d[0].message
    );
    assert_eq!(versao_de(&p, "alto.dart"), LanguageVersion::ATUAL);
    let baixo = tmp.path().join("baixo.dart");
    fs::write(&baixo, "// @dart=2.9\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&baixo, &sdk, None, &mut Interner::new());
    assert!(
        d.len() == 1 && d[0].message.contains("abaixo da mínima"),
        "{d:?}"
    );
}

#[test]
fn recurso_desligado_pela_versao_e_diagnostico_do_parser() {
    let tmp = tempdir().unwrap();
    let sdk = sdk_falso(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(&main, "// @dart=3.7\nvar l = [?null];\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message
            .contains("'null-aware-elements' exige a versão de linguagem 3.8"),
        "{}",
        d[0].message
    );
    fs::write(&main, "// @dart=3.8\nvar l = [?null];\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert!(d.is_empty(), "{d:?}");
}
