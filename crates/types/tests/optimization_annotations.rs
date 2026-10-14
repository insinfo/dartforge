//! Metadata recognition is deliberately tested separately from backend proofs.
use dartforge_elements::{load::load, model::LibraryId, sdk::SdkLayout};
use dartforge_intern::Interner;
use dartforge_types::optimization_annotations::*;
use dartforge_types::{CoreTypes, TypeTable, infer_library_bodies_with_locals, resolve_outline};
use std::{fs, path::Path};

const ANNOTATIONS: &str = r#"
const int dartForgeAnnotationsSchema = 1;
final class PreferRegion { const PreferRegion(); }
final class PreferAcyclic { const PreferAcyclic(); }
final class NoEscape { const NoEscape(); }
final class Realtime { const Realtime(); }
"#;

fn mock_sdk(root: &Path) -> SdkLayout {
    let lib = root.join("sdk");
    fs::create_dir_all(lib.join("core")).unwrap();
    fs::create_dir_all(lib.join("async")).unwrap();
    fs::write(lib.join("libraries.json"), r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#).unwrap();
    fs::write(
        lib.join("core/core.dart"),
        r#"
class Object { const Object(); }
class num extends Object {} class int extends num {} class double extends num {}
class String extends Object {} class bool extends Object {} class Null extends Object {}
class Function extends Object {} class Record extends Object {}
class Iterable<E> extends Object {} class List<E> extends Object implements Iterable<E> {}
class Map<K,V> extends Object {} class Set<E> extends Object {}
"#,
    )
    .unwrap();
    fs::write(
        lib.join("async/async.dart"),
        "class Future<T> {} class Stream<T> {}",
    )
    .unwrap();
    SdkLayout::load(&lib, "dartdevc").unwrap()
}

fn normalize_source(source: &str, annotations: &str) -> Normalization {
    let temp = tempfile::tempdir().unwrap();
    let sdk = mock_sdk(temp.path());
    let package = temp.path().join("dartforge/lib/src");
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("annotations.dart"), annotations).unwrap();
    fs::write(
        package.parent().unwrap().join("dartforge.dart"),
        "export 'src/annotations.dart';",
    )
    .unwrap();
    fs::write(
        temp.path().join("reexport.dart"),
        "export 'package:dartforge/dartforge.dart';",
    )
    .unwrap();
    fs::write(temp.path().join("fake.dart"), "library dartforge.src.annotations; class Realtime { const Realtime(); } class PreferRegion { const PreferRegion(); }").unwrap();
    let config = temp.path().join("package_config.json");
    fs::write(&config, r#"{"configVersion":2,"packages":[{"name":"dartforge","rootUri":"dartforge/","packageUri":"lib/","languageVersion":"3.6"}]}"#).unwrap();
    let entry = temp.path().join("main.dart");
    fs::write(&entry, source).unwrap();
    let mut interner = Interner::new();
    let program = load(&entry, &sdk, Some(&config), &mut interner).unwrap();
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &interner);
    let (mut outline, _) = resolve_outline(&program, &interner, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.is_sdk)
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    let (bodies, _) = infer_library_bodies_with_locals(
        &program,
        &interner,
        &mut table,
        &core,
        &mut outline,
        &libs,
    );
    normalize(
        &program,
        &interner,
        &mut table,
        &core,
        &outline,
        &bodies,
        &libs.iter().copied().collect(),
        &libs,
    )
}

#[test]
fn optimization_annotations_resolve_prefix_reexport_static_and_local_aliases() {
    let normalized = normalize_source(
        r#"
import 'reexport.dart' as api;
import 'package:dartforge/dartforge.dart';
const alias = api.PreferRegion();
class Metadata { static const rt = Realtime(); }
@PreferAcyclic() class Sample { @api.PreferAcyclic() const Sample(); }
@alias int first(@api.NoEscape() Object value) => 1;
class Processor { @Metadata.rt int run() => 2; }
void main() {
  const local = PreferRegion();
  @local int nested(@NoEscape() Object value) => 3;
  @PreferAcyclic() final sample = Sample();
  @PreferAcyclic() final (left, right) = (1, 2);
}
"#,
        ANNOTATIONS,
    );
    assert!(
        normalized.diagnostics.is_empty(),
        "{:?}",
        normalized.diagnostics
    );
    assert_eq!(normalized.records.len(), 9, "{:?}", normalized.records);
    assert_eq!(
        normalized
            .records
            .iter()
            .filter(|r| r.kind == AnnotationKind::Obligation(Obligation::NoEscape))
            .count(),
        2
    );
    assert!(
        normalized
            .records
            .iter()
            .any(|r| matches!(r.target, AnnotationTarget::LocalVariables { .. }))
    );
    assert!(normalized.records.iter().all(|r| r.span.end > r.span.start));
    let package = normalized.package.unwrap();
    assert_eq!(package.schema, Some(1));
    assert_eq!(package.defining_uri, DEFINING_URI);
    assert_eq!(package.sources.len(), 1);
    assert!(package.sources[0].source.contains("class NoEscape"));
}

#[test]
fn optimization_annotations_ignore_homonyms_and_shadowed_metadata() {
    let normalized = normalize_source(
        r#"
import 'package:dartforge/dartforge.dart';
import 'fake.dart' as fake;
@fake.Realtime() int irrelevant() => 1;
@fake.PreferRegion() class Another {}
void main() {
  const Realtime = fake.Realtime();
  @Realtime int local() => 1;
  { const PreferRegion = fake.PreferRegion(); @PreferRegion int nested() => 2; }
}
"#,
        ANNOTATIONS,
    );
    assert!(normalized.records.is_empty(), "{:?}", normalized.records);
    assert!(
        normalized.diagnostics.is_empty(),
        "{:?}",
        normalized.diagnostics
    );
}

#[test]
fn optimization_annotations_validate_targets_and_keep_external_noescape_pending() {
    let normalized = normalize_source(
        r#"
import 'package:dartforge/dartforge.dart';
@PreferRegion() class InvalidClass {}
@PreferAcyclic() final invalidGlobal = 1;
@NoEscape() void invalidFunction() {}
@Realtime() Future<int> asyncFunction() async => 1;
@Realtime() Iterable<int> syncGenerator() sync* { yield 1; }
@Realtime() Stream<int> asyncGenerator() async* { yield 1; }
@Realtime() external void externalFunction();
class Other { @Realtime() Other(@NoEscape() Object invalidParameter); @PreferAcyclic() int field = 1; }
external void unknownBody(@NoEscape() Object pending);
void main() {}
"#,
        ANNOTATIONS,
    );
    assert_eq!(
        normalized.diagnostics.len(),
        10,
        "{:?}",
        normalized.diagnostics
    );
    assert!(
        normalized
            .diagnostics
            .iter()
            .all(|d| d.kind == AnnotationDiagnosticKind::InvalidTarget)
    );
    assert_eq!(normalized.records.len(), 1);
    assert_eq!(
        normalized.records[0].kind,
        AnnotationKind::Obligation(Obligation::NoEscape)
    );
    assert_eq!(normalized.records[0].body, BodyAvailability::Unavailable);
}

#[test]
fn optimization_annotations_reject_unsupported_or_malformed_designated_schema() {
    let source =
        "import 'package:dartforge/dartforge.dart'; @Realtime() int run() => 1; void main() {}";
    for declarations in [
        ANNOTATIONS.replace("Schema = 1", "Schema = 2"),
        ANNOTATIONS.replace(
            "const int dartForgeAnnotationsSchema = 1;",
            "const dartForgeAnnotationsSchema = '1';",
        ),
        ANNOTATIONS.replace("const int dartForgeAnnotationsSchema = 1;", ""),
    ] {
        let normalized = normalize_source(source, &declarations);
        assert!(normalized.records.is_empty());
        assert_eq!(normalized.diagnostics.len(), 1);
        assert_eq!(
            normalized.diagnostics[0].kind,
            AnnotationDiagnosticKind::UnsupportedSchema
        );
    }
}

#[test]
fn optimization_annotations_reject_invalid_constructor_arguments_and_values() {
    let annotations = ANNOTATIONS.replace(
        "const PreferRegion();",
        "const PreferRegion([int ignored = 0]);",
    );
    let normalized = normalize_source(
        "import 'package:dartforge/dartforge.dart'; @PreferRegion(1) int run() => 1; void main() {}",
        &annotations,
    );
    assert!(normalized.records.is_empty());
    assert_eq!(normalized.diagnostics.len(), 1);
    assert_eq!(
        normalized.diagnostics[0].kind,
        AnnotationDiagnosticKind::InvalidMetadata
    );
}

#[test]
fn optimization_annotations_require_an_actual_constant_annotation_value() {
    let normalized = normalize_source(
        "import 'package:dartforge/dartforge.dart'; @Realtime int run() => 1; void main() {}",
        ANNOTATIONS,
    );
    assert!(normalized.records.is_empty());
    assert_eq!(normalized.diagnostics.len(), 1);
    assert_eq!(
        normalized.diagnostics[0].kind,
        AnnotationDiagnosticKind::InvalidMetadata
    );
}

#[test]
fn optimization_annotations_do_not_recognize_ambiguous_import_recovery() {
    for source in [
        "import 'package:dartforge/dartforge.dart'; import 'fake.dart'; @Realtime() int run() => 1; void main() {}",
        "import 'package:dartforge/dartforge.dart' as both; import 'fake.dart' as both; @both.Realtime() int run() => 1; void main() {}",
    ] {
        let normalized = normalize_source(source, ANNOTATIONS);
        assert!(normalized.records.is_empty(), "{:?}", normalized.records);
        assert!(
            normalized.diagnostics.is_empty(),
            "{:?}",
            normalized.diagnostics
        );
    }
}

#[test]
fn optimization_annotations_schema_policy_includes_future_class_use_but_not_imports() {
    let declarations = ANNOTATIONS.replace("Schema = 1", "Schema = 2")
        + "final class FutureContract { const FutureContract(); }";
    let used = normalize_source(
        "import 'package:dartforge/dartforge.dart'; @FutureContract() int run() => 1; void main() {}",
        &declarations,
    );
    assert!(used.records.is_empty());
    assert_eq!(used.diagnostics.len(), 1);
    assert_eq!(
        used.diagnostics[0].kind,
        AnnotationDiagnosticKind::UnsupportedSchema
    );
    let imported = normalize_source(
        "import 'package:dartforge/dartforge.dart'; void main() {}",
        &declarations,
    );
    assert!(imported.records.is_empty());
    assert!(imported.diagnostics.is_empty());
    let ordinary = normalize_source(
        "import 'package:dartforge/dartforge.dart'; @FutureContract() int run() => 1; void main() {}",
        &(ANNOTATIONS.to_string() + "final class FutureContract { const FutureContract(); }"),
    );
    assert!(ordinary.records.is_empty());
    assert!(ordinary.diagnostics.is_empty());
}

#[test]
fn optimization_annotations_local_binding_overrides_ambiguous_library_names() {
    let normalized = normalize_source(
        r#"
import 'package:dartforge/dartforge.dart' as api;
import 'package:dartforge/dartforge.dart';
import 'fake.dart';
void main() { const Realtime = api.Realtime(); @Realtime int local() => 1; }
"#,
        ANNOTATIONS,
    );
    assert!(
        normalized.diagnostics.is_empty(),
        "{:?}",
        normalized.diagnostics
    );
    assert_eq!(normalized.records.len(), 1);
    assert_eq!(
        normalized.records[0].kind,
        AnnotationKind::Obligation(Obligation::Realtime)
    );
}
