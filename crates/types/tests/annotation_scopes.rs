//! Shared annotation scope resolution must follow import prefixes.
use dartforge_elements::{load::load_lenient, model::Element, sdk::SdkLayout};
use dartforge_intern::Interner;
use dartforge_types::annotations as under_test;

fn program(
    source: &str,
) -> (
    tempfile::TempDir,
    dartforge_elements::model::Program,
    Interner,
) {
    let root = tempfile::tempdir().unwrap();
    let sdk = root.path().join("sdk");
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::write(
        sdk.join("libraries.json"),
        r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]}}}}"#,
    )
    .unwrap();
    std::fs::write(sdk.join("core/core.dart"), "class Object {} class String {} class Deprecated { const Deprecated(String text); } const deprecated = Deprecated('old');").unwrap();
    let file = root.path().join("main.dart");
    std::fs::write(&file, source).unwrap();
    let mut names = Interner::new();
    let layout = SdkLayout::load(&sdk, "dartdevc").unwrap();
    let (program, _) = load_lenient(&file, &layout, None, &mut names);
    (root, program, names)
}

#[test]
fn prefixed_annotation_element_denotes_the_exported_class() {
    let (_root, p, names) =
        program("import 'dart:core' as core; @core.Deprecated('old') class Old {}");
    let (i, unit) = p
        .units
        .iter()
        .enumerate()
        .find(|(_, u)| !p.library(u.library).is_sdk)
        .unwrap();
    let annotation = unit
        .ast
        .decls
        .iter()
        .find_map(|d| d.metadata.first())
        .unwrap();
    let target = under_test::annotation_element(
        &p,
        &names,
        dartforge_elements::model::UnitId(i as u32),
        annotation,
    );
    let Some(Element::Class(c)) = target else {
        panic!("expected the annotation class, received {target:?}");
    };
    assert_eq!(names.resolve(p.class(c).name), "Deprecated");
    assert_eq!(p.library(p.class(c).library).uri, "dart:core");
}

#[test]
fn a_same_named_user_annotation_keeps_its_own_identity() {
    let (_root, p, names) = program(
        "import 'dart:core' as core; class Deprecated { const Deprecated(core.String text); } @Deprecated('custom') class Fresh {}",
    );
    let (i, unit) = p
        .units
        .iter()
        .enumerate()
        .find(|(_, u)| !p.library(u.library).is_sdk)
        .unwrap();
    let annotation = unit
        .ast
        .decls
        .iter()
        .find_map(|d| d.metadata.first())
        .unwrap();
    let target = under_test::annotation_element(
        &p,
        &names,
        dartforge_elements::model::UnitId(i as u32),
        annotation,
    );
    let Some(Element::Class(c)) = target else {
        panic!("expected the user annotation class, received {target:?}");
    };
    assert_eq!(names.resolve(p.class(c).name), "Deprecated");
    assert_ne!(p.library(p.class(c).library).uri, "dart:core");
}

#[test]
fn prefixed_annotation_element_denotes_the_exported_constant() {
    let (_root, p, names) = program("import 'dart:core' as core; @core.deprecated class Old {}");
    let (i, unit) = p
        .units
        .iter()
        .enumerate()
        .find(|(_, u)| !p.library(u.library).is_sdk)
        .unwrap();
    let annotation = unit
        .ast
        .decls
        .iter()
        .find_map(|d| d.metadata.first())
        .unwrap();
    let target = under_test::annotation_element(
        &p,
        &names,
        dartforge_elements::model::UnitId(i as u32),
        annotation,
    );
    let Some(Element::Variable(v)) = target else {
        panic!("expected the annotation constant, received {target:?}");
    };
    assert_eq!(names.resolve(p.variable(v).name), "deprecated");
    assert_eq!(p.library(p.variable(v).library).uri, "dart:core");
}

#[test]
fn local_parameter_shadowing_keeps_the_import_prefix_hidden() {
    let (_root, p, names) = program(
        "import 'dart:core' as core; void outer(core.Object core) { @core.deprecated void local() {} }",
    );
    let (i, unit) = p
        .units
        .iter()
        .enumerate()
        .find(|(_, u)| !p.library(u.library).is_sdk)
        .unwrap();
    let annotation = unit
        .ast
        .local_metadata
        .iter()
        .flat_map(|(_, ms)| ms.iter())
        .next()
        .unwrap();
    let target = under_test::annotation_element(
        &p,
        &names,
        dartforge_elements::model::UnitId(i as u32),
        annotation,
    );
    assert_eq!(target, None);
}
