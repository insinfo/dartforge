//! Constructor metadata through aliases retains its class, type and values.
use dartforge_elements::{
    load::load,
    model::{LibraryId, UnitId},
    sdk::SdkLayout,
};
use dartforge_intern::Interner;
use dartforge_types::const_eval::{
    evaluation::Engine,
    value::{Field, InstanceState},
};
use dartforge_types::{
    CoreTypes, Type, TypeTable, infer_library_bodies_with_locals, resolve_outline,
};
use std::collections::BTreeMap;
#[test]
fn annotation_alias_constructors_keep_target_identity_and_constant_fields() {
    let dir = tempfile::tempdir().unwrap();
    let sdk = dir.path().join("sdk");
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::create_dir_all(sdk.join("async")).unwrap();
    std::fs::write(sdk.join("libraries.json"),r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#).unwrap();
    std::fs::write(sdk.join("core/core.dart"), "class Object { const Object(); } class num extends Object {} class int extends num {} class double extends num {} class String extends Object {} class bool extends Object {} class Null extends Object {} class Function extends Object {} class Record extends Object {} class Iterable<E> extends Object {} class List<E> extends Object implements Iterable<E> {} class Map<K,V> extends Object {} class Set<E> extends Object {}").unwrap();
    std::fs::write(
        sdk.join("async/async.dart"),
        "class Future<T> {} class Stream<T> {}",
    )
    .unwrap();
    std::fs::write(dir.path().join("markers.dart"), "class Marker { final String value; const Marker(this.value); const Marker.named(this.value); } typedef Alias = Marker; typedef Chain = Alias; class Box<T> { final T value; const Box(this.value); } typedef Fixed = Box<int>; typedef Generic<T> = Box<T>;").unwrap();
    let file = dir.path().join("main.dart");
    std::fs::write(&file,"import 'markers.dart'; import 'markers.dart' as p; @p.Alias('first') class First {} @Chain.named('second') class Second {} @p.Alias.new('third') class Third {} @p.Fixed(42) class Fourth {} @p.Generic<String>('text') class Fifth {} void main() {}").unwrap();
    let mut names = Interner::new();
    let layout = SdkLayout::load(&sdk, "dartdevc").unwrap();
    let program = load(&file, &layout, None, &mut names).unwrap();
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &names);
    let (mut outline, _) = resolve_outline(&program, &names, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    let (body, _) =
        infer_library_bodies_with_locals(&program, &names, &mut table, &core, &mut outline, &libs);
    let inferred = libs.iter().copied().collect();
    let mut engine = Engine::new(
        &program, &names, &mut table, &core, &outline, &body, &inferred,
    );
    let mut found = BTreeMap::new();
    for (i, unit) in program.units.iter().enumerate() {
        for decl in &unit.ast.decls {
            for metadata in &decl.metadata {
                let value = engine
                    .evaluate_annotation(UnitId(i as u32), metadata)
                    .unwrap_or_else(|| {
                        panic!(
                            "unevaluated {}",
                            &unit.source[metadata.span.start..metadata.span.end]
                        )
                    });
                let Type::Interface { class, args, .. } = engine.table.get(value.ty) else {
                    panic!("non-instance metadata")
                };
                let class_name = names.resolve(program.class(*class).name).to_string();
                let type_args = args
                    .iter()
                    .map(|&t| engine.table.format(t, &names, &program))
                    .collect::<Vec<_>>();
                let InstanceState::Generic { fields, .. } = value.state else {
                    panic!("unknown metadata value")
                };
                let field = fields
                    .iter()
                    .find_map(|(key, value)| {
                        (*key == Field::MemberName(names.lookup("value").unwrap())).then_some(value)
                    })
                    .unwrap();
                let text = match &field.state {
                    InstanceState::Str(Some(s)) => String::from_utf16_lossy(s),
                    InstanceState::Int(Some(n)) => n.to_string(),
                    other => panic!("unknown field {other:?}"),
                };
                found.insert(text, (class_name, type_args));
            }
        }
    }
    for text in ["first", "second", "third"] {
        assert_eq!(found.get(text), Some(&("Marker".into(), vec![])));
    }
    assert_eq!(found.get("42"), Some(&("Box".into(), vec!["int".into()])));
    assert_eq!(
        found.get("text"),
        Some(&("Box".into(), vec!["String".into()]))
    );
    assert_eq!(found.len(), 5);
}
