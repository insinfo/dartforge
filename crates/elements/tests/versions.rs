//! Language version per library (`docs/VERSOES-LINGUAGEM.md` §2): the
//! `// @dart = x.y` marker wins over the package's `languageVersion`, which
//! wins over the current version; parts follow the library; `dart:*` stays
//! at the floor.
use dartforge_elements::load::load_lenient;
use dartforge_elements::model::Program;
use dartforge_elements::sdk::SdkLayout;
use dartforge_frontend::{Feature, LanguageVersion};
use dartforge_intern::Interner;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn fake_sdk(dir: &Path) -> SdkLayout {
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

fn version_of(p: &Program, excerpt: &str) -> LanguageVersion {
    p.libraries
        .iter()
        .find(|l| l.uri.contains(excerpt))
        .unwrap_or_else(|| panic!("{excerpt}"))
        .features
        .version()
}

fn v(a: u16, b: u16) -> LanguageVersion {
    LanguageVersion::new(a, b)
}

#[test]
fn no_marker_and_no_package_uses_current() {
    let tmp = tempdir().unwrap();
    let mut sdk = fake_sdk(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(&main, "void main() {}").unwrap();
    let (p, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(version_of(&p, "main.dart"), LanguageVersion::CURRENT);
    assert_eq!(version_of(&p, "dart:core"), LanguageVersion::FLOOR);
    assert!(
        p.library(p.entry.unwrap())
            .features
            .has(Feature::PrimaryConstructors)
    );

    sdk.current_version = v(3, 6);
    let (p, _) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert_eq!(version_of(&p, "main.dart"), v(3, 6));
    assert!(
        !p.library(p.entry.unwrap())
            .features
            .has(Feature::WildcardVariables)
    );
}

#[test]
fn marker_wins_over_package() {
    let tmp = tempdir().unwrap();
    let sdk = fake_sdk(tmp.path());
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
    // `bin/` belongs to the package through the root; `lib/` through the
    // `package:` URI.
    fs::write(
        proj.join("bin/main.dart"),
        "import 'package:app/a.dart';\nimport 'package:app/b.dart';\nvoid main() {}",
    )
    .unwrap();
    fs::write(proj.join("lib/a.dart"), "int a = 1;").unwrap();
    fs::write(
        proj.join("lib/b.dart"),
        "// license\n// @dart = 3.6\nint b = 1;",
    )
    .unwrap();
    let (p, d) = load_lenient(
        &proj.join("bin/main.dart"),
        &sdk,
        None,
        &mut Interner::new(),
    );
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(version_of(&p, "main.dart"), v(3, 8));
    assert_eq!(version_of(&p, "package:app/a.dart"), v(3, 8));
    assert_eq!(version_of(&p, "package:app/b.dart"), v(3, 6));
}

#[test]
fn part_with_different_version_is_error() {
    let tmp = tempdir().unwrap();
    let sdk = fake_sdk(tmp.path());
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
    assert_eq!(version_of(&p, "main.dart"), v(3, 10));
    // `q.dart` without a marker stays at the current version (3.13), unlike
    // the library.
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message.contains("q.dart")
            && d[0]
                .message
                .contains("the part is at language version 3.13"),
        "{}",
        d[0].message
    );
}

#[test]
fn marker_out_of_range_is_error() {
    let tmp = tempdir().unwrap();
    let sdk = fake_sdk(tmp.path());
    let high = tmp.path().join("alto.dart");
    fs::write(&high, "// @dart=3.99\nvoid main() {}").unwrap();
    let (p, d) = load_lenient(&high, &sdk, None, &mut Interner::new());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message
            .contains("greater than the latest known language version"),
        "{}",
        d[0].message
    );
    assert_eq!(version_of(&p, "alto.dart"), LanguageVersion::CURRENT);
    let low = tmp.path().join("baixo.dart");
    fs::write(&low, "// @dart=2.9\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&low, &sdk, None, &mut Interner::new());
    assert!(
        d.len() == 1
            && d[0]
                .message
                .contains("The language version must be >=2.12.0."),
        "{d:?}"
    );
}

#[test]
fn feature_disabled_by_version_is_parser_diagnostic() {
    let tmp = tempdir().unwrap();
    let sdk = fake_sdk(tmp.path());
    let main = tmp.path().join("main.dart");
    fs::write(&main, "// @dart=3.7\nvar l = [?null];\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message
            .contains("This requires the 'null-aware-elements' language feature to be enabled."),
        "{}",
        d[0].message
    );
    fs::write(&main, "// @dart=3.8\nvar l = [?null];\nvoid main() {}").unwrap();
    let (_, d) = load_lenient(&main, &sdk, None, &mut Interner::new());
    assert!(d.is_empty(), "{d:?}");
}
