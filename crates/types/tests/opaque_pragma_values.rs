//! Core pragma field formals stay known without inferring SDK bodies.
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

fn inspect(assertion: bool) -> Vec<(String, bool, String, bool)> {
    let dir = tempfile::tempdir().unwrap();
    let sdk = dir.path().join("sdk");
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::create_dir_all(sdk.join("async")).unwrap();
    std::fs::write(sdk.join("libraries.json"),r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#).unwrap();
    let core = "class Object { const Object(); } class num extends Object {} class int extends num {} class double extends num {} class String extends Object {} class bool extends Object {} class Null extends Object {} class Function extends Object {} class Record extends Object {} class Iterable<E> extends Object {} class List<E> extends Object implements Iterable<E> {} class Map<K,V> extends Object {} class Set<E> extends Object {}";
    let suffix = if assertion { " : assert(false)" } else { "" };
    std::fs::write(sdk.join("core/core.dart"),format!("{core} final class pragma {{ final String name; final Object? options; const factory pragma(String name, [Object? options]) = pragma._; const pragma._(this.name, [this.options]){suffix}; }}")).unwrap();
    std::fs::write(
        sdk.join("async/async.dart"),
        "class Future<T> {} class Stream<T> {}",
    )
    .unwrap();
    let file = dir.path().join("main.dart");
    std::fs::write(&file,"import 'dart:core' as core; const tag = 'weak-' + 'tearoff-reference'; const marker = core.pragma(tag, const [1]); typedef Marker = core.pragma; class pragma { final core.String name; const pragma(this.name); } @core.pragma(tag) class Direct {} @marker class ConstantAlias {} @Marker(tag) class ConstructorAlias {} @pragma(tag) class Custom {} void main() {}").unwrap();
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
        .filter(|(_, l)| !l.is_sdk)
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    let (body, _) =
        infer_library_bodies_with_locals(&program, &names, &mut table, &core, &mut outline, &libs);
    let inferred = libs.iter().copied().collect();
    let mut engine = Engine::new(
        &program, &names, &mut table, &core, &outline, &body, &inferred,
    );
    let mut rows = Vec::new();
    for (i, unit) in program.units.iter().enumerate() {
        if program.library(unit.library).is_sdk {
            continue;
        }
        for decl in &unit.ast.decls {
            for metadata in &decl.metadata {
                let value = engine
                    .evaluate_annotation(UnitId(i as u32), metadata)
                    .unwrap();
                let Type::Interface { class, .. } = engine.table.get(value.ty) else {
                    panic!("non-instance metadata")
                };
                let is_core = program.library(program.class(*class).library).uri == "dart:core";
                let InstanceState::Generic { fields, unknown } = &value.state else {
                    panic!("unknown metadata shape")
                };
                let name = fields
                    .iter()
                    .find_map(|(key, value)| {
                        if *key != Field::MemberName(names.lookup("name").unwrap()) {
                            return None;
                        }
                        match &value.state {
                            InstanceState::Str(Some(t)) => Some(String::from_utf16_lossy(t)),
                            _ => None,
                        }
                    })
                    .unwrap_or_default();
                if metadata.name.len() == 1
                    && names.resolve(metadata.name[0].sym) == "marker"
                    && !assertion
                {
                    let options = fields
                        .iter()
                        .find_map(|(key, value)| {
                            (*key == Field::MemberName(names.lookup("options").unwrap()))
                                .then_some(value)
                        })
                        .unwrap();
                    assert!(
                        matches!(&options.state,InstanceState::List{elements,..} if elements.len()==1)
                    );
                }
                rows.push((
                    unit.source[metadata.span.start..metadata.span.end].into(),
                    is_core,
                    name,
                    *unknown,
                ));
            }
        }
    }
    rows
}

#[test]
fn opaque_core_pragmas_preserve_names_options_and_alias_identity() {
    let rows = inspect(false);
    assert_eq!(rows.len(), 4);
    for (_, _, name, unknown) in &rows {
        assert_eq!(name, "weak-tearoff-reference");
        assert!(!unknown);
    }
    assert_eq!(rows.iter().filter(|(_, core, _, _)| *core).count(), 3);
    assert!(
        !rows.last().unwrap().1,
        "a user pragma remains a different class"
    );
}

#[test]
fn opaque_sdk_initializers_are_still_unknown() {
    let rows = inspect(true);
    for (_, core, name, unknown) in rows {
        if core {
            assert!(name.is_empty());
            assert!(unknown);
        } else {
            assert_eq!(name, "weak-tearoff-reference");
            assert!(!unknown);
        }
    }
}
