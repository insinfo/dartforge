//! Known compiler declarations must never become analyzer defaults.
use dartforge_elements::{
    environment::{CompilationEnvironment, EnvironmentTarget},
    load::load,
    model::{LibraryId, VariableRef},
    sdk::SdkLayout,
};
use dartforge_intern::Interner;
use dartforge_types::const_eval::{
    self,
    evaluation::{Ctx, Engine},
    value::InstanceState,
};
use dartforge_types::{CoreTypes, TypeTable, infer_library_bodies_with_locals, resolve_outline};
use std::collections::BTreeMap;

fn inspect(flag: Option<&str>) -> (BTreeMap<String, String>, bool) {
    let dir = tempfile::tempdir().unwrap();
    let sdk = dir.path().join("sdk");
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::create_dir_all(sdk.join("async")).unwrap();
    std::fs::write(sdk.join("libraries.json"),r#"{"dartdevc":{"libraries":{"core":{"uri":"core/core.dart","patches":[]},"async":{"uri":"async/async.dart","patches":[]}}}}"#).unwrap();
    std::fs::write(sdk.join("core/core.dart"), r#"
class Object { const Object(); } class num extends Object {}
class int extends num { external const factory int.fromEnvironment(String name, {int defaultValue = 0}); }
class double extends num {}
class String extends Object { external const factory String.fromEnvironment(String name, {String defaultValue = ""}); }
class bool extends Object {
 external const factory bool.fromEnvironment(String name, {bool defaultValue = false});
 external const factory bool.hasEnvironment(String name);
}
class Null extends Object {} class Function extends Object {} class Record extends Object {}
class Iterable<E> extends Object {} class List<E> extends Object implements Iterable<E> {}
class Map<K,V> extends Object {} class Set<E> extends Object {}
"#).unwrap();
    std::fs::write(
        sdk.join("async/async.dart"),
        "class Future<T> {} class Stream<T> {}",
    )
    .unwrap();
    let file = dir.path().join("main.dart");
    std::fs::write(
        &file,
        r#"
const prefix = 'fea'; const key = '${prefix}ture';
const text = String.fromEnvironment(key, defaultValue: 'fallback');
const enabled = bool.fromEnvironment(key);
const present = bool.hasEnvironment(key);
const absent = bool.hasEnvironment('missing');
const empty = String.fromEnvironment('empty', defaultValue: 'fallback');
const number = int.fromEnvironment('n', defaultValue: 17);
const duplicate = {bool.fromEnvironment(key): 1, true: 2};
void main() {}
"#,
    )
    .unwrap();
    let mut names = Interner::new();
    let mut layout = SdkLayout::load(&sdk, "dartdevc").unwrap();
    // The analyzer's import context alone must not enable constant folding.
    layout.conditional_import_variables = Some([("feature".into(), "true".into())].into());
    if let Some(flag) = flag {
        CompilationEnvironment::new(
            EnvironmentTarget::JavaScript,
            &layout,
            false,
            &[format!("feature={flag}"), "empty=".into(), "n=0x10".into()],
        )
        .unwrap()
        .install(&mut layout);
    }
    let program = load(&file, &layout, None, &mut names).unwrap();
    assert_eq!(program.compilation_environment.is_some(), flag.is_some());
    let mut table = TypeTable::new();
    let core = CoreTypes::init(&mut table, &program, &names);
    let (mut outline, _) = resolve_outline(&program, &names, &mut table, &core);
    let libs: Vec<_> = program
        .libraries
        .iter()
        .enumerate()
        .filter(|(_, lib)| !lib.is_sdk)
        .map(|(i, _)| LibraryId(i as u32))
        .collect();
    let (body, _) =
        infer_library_bodies_with_locals(&program, &names, &mut table, &core, &mut outline, &libs);
    let inferred = libs.iter().copied().collect();
    let diagnostics = const_eval::verify(
        &program, &names, &mut table, &core, &outline, &body, &inferred, &libs,
    );
    let duplicate = diagnostics.iter().any(|(_, d)| {
        d.code == Some(dartforge_diagnostics::codes::compile_time_error::EQUAL_KEYS_IN_CONST_MAP)
    });
    let mut engine = Engine::new(
        &program, &names, &mut table, &core, &outline, &body, &inferred,
    );
    let mut values = BTreeMap::new();
    for variable in &program.variables {
        if !libs.contains(&variable.library) {
            continue;
        }
        let VariableRef::TopLevel { unit, decl, index } = variable.node else {
            continue;
        };
        let dartforge_elements::model::Unit { ast, .. } = program.unit(unit);
        let dartforge_frontend::ast::DeclKind::Variables(list) = &ast.decl(decl).kind else {
            continue;
        };
        let expression = list.variables[index].initializer.unwrap();
        let constant = engine.evaluate(&Ctx::simple(unit, variable.library), expression, true);
        let value = constant
            .value()
            .unwrap_or_else(|| panic!("{}: {constant:?}", names.resolve(variable.name)));
        let text = match &value.state {
            InstanceState::Bool(Some(value)) => value.to_string(),
            InstanceState::Int(Some(value)) => value.to_string(),
            InstanceState::Str(Some(value)) => String::from_utf16_lossy(value),
            _ => "unknown".into(),
        };
        values.insert(names.resolve(variable.name).to_string(), text);
    }
    (values, duplicate)
}

#[test]
fn compiler_values_and_duplicate_keys_follow_each_compilation() {
    for flag in ["true", "false", "true"] {
        let (values, duplicate) = inspect(Some(flag));
        assert_eq!(values["text"], flag);
        assert_eq!(values["enabled"], flag);
        assert_eq!(values["present"], "true");
        assert_eq!(values["absent"], "false");
        assert_eq!(values["empty"], "");
        assert_eq!(values["number"], "16");
        assert_eq!(duplicate, flag == "true");
    }
}

#[test]
fn analyzer_keeps_environment_values_unknown_with_explicit_import_variables() {
    let (values, duplicate) = inspect(None);
    for name in ["text", "enabled", "present", "absent", "empty", "number"] {
        assert_eq!(values[name], "unknown", "{name}");
    }
    assert!(!duplicate);
}
