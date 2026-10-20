//! Reserved display metadata is neither a nominal declaration nor an allocator.
use super::*;

fn startup() -> String {
    format!(
        "@frame = private constant [11 x i8] c\"_AsyncFrame\"\ndefine void @df.register.dart$3atest() {{\n call void @dartforge_register_class_name(i64 {}, ptr @frame, i64 11)\n ret void\n}}\n",
        crate::lower::async_sm::ASYNC_FRAME_ID
    )
}

#[test]
fn frame_name_is_entry_only_and_does_not_authorize_an_allocator() {
    let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
    let ir = format!(
        "{}define i64 @frame_allocator() {{\n %r = call i64 @dartforge_object_new(i64 {frame}, i64 3)\n ret i64 %r\n}}\n",
        startup()
    );
    let (directory, names, runtime) = fixtures(&ir);
    let inventory = Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
    let symbol = "df.register.dart$3atest";
    assert!(inventory.entry(symbol));
    assert!(!inventory.call(symbol, &[], Type::Void));
    assert!(!inventory.call("frame_allocator", &[], Type::Ref));
    assert!(!inventory.known_node("frame_allocator"));
    assert!(!inventory.known_node("dartforge_register_class_name"));
    assert!(
        !inventory.summaries[0]
            .nominal_classes
            .contains(&i64::from(frame))
    );
    assert!(
        inventory
            .table_declarations
            .values()
            .all(|cid| *cid != frame)
    );
    let refs = |summary: &Summary| {
        let definition = summary
            .definitions
            .iter()
            .find(|d| summary.names[d.name as usize] == symbol)
            .unwrap();
        definition
            .refs
            .iter()
            .map(|id| summary.names[*id as usize].clone())
            .collect::<Vec<_>>()
    };
    assert!(refs(&inventory.summaries[0]).contains(&"dartforge_register_class_name".into()));
    assert!(
        !refs(&inventory.class_proof_summaries()[0])
            .contains(&"dartforge_register_class_name".into())
    );
}

#[test]
fn different_names_ids_pointer_capabilities_and_allocating_startups_stay_open() {
    let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
    for variant in 0..10 {
        let ir = match variant {
            0 => startup().replace("_AsyncFrame", "_StackTrace"),
            1 => startup().replace("_AsyncFrame", "_AsyncFramX"),
            2 => startup().replace(&format!("i64 {frame},"), &format!("i64 {},", frame + 1)),
            3 => startup().replace("ptr @frame, i64 11", "ptr @frame, i64 10"),
            4 => startup().replace("private constant", "constant"),
            5 => startup().replace("ptr @frame,", "ptr null,"),
            6 => startup().replace(
                "ptr @frame,",
                "ptr getelementptr ([11 x i8], ptr @frame, i64 0, i64 1),",
            ),
            7 => startup().replace(
                " ret void",
                &format!(" %r = call i64 @dartforge_object_new(i64 {frame}, i64 0)\n ret void"),
            ),
            8 => startup().replace(
                " ret void",
                " call void @dartforge_register_helper(ptr @frame, i64 11, ptr null)\n ret void",
            ),
            _ => format!(
                "{}define void @dartforge_register_class_name(i64 %c, ptr %p, i64 %n) {{\n ret void\n}}\n",
                startup()
            ),
        };
        let (directory, names, runtime) = fixtures(&ir);
        let inventory =
            Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
        assert!(
            !inventory.entry("df.register.dart$3atest"),
            "variant {variant}"
        );
    }
}

fn certified(modules: &[(&str, &str)], mutate: impl FnOnce(&mut Vec<Summary>)) -> HashSet<String> {
    let bundle = crate::pruning::allocation_inventory::closure::audit(modules);
    let raw: super::super::Payload = serde_json::from_value(bundle.to_json()).unwrap();
    let mut summaries: Vec<_> = modules
        .iter()
        .map(|(module, ir)| crate::pruning::summarize(ir, module, &[]))
        .collect();
    mutate(&mut summaries);
    let mut keys: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, function) in raw.functions.iter().enumerate() {
        keys.entry(function.key.clone()).or_default().push(index);
    }
    let bytes = raw
        .globals
        .iter()
        .filter_map(|global| {
            crate::pruning::allocation_inventory::closure::native_byte_content(&global.initializer)
                .map(|bytes| (format!("{}#{}", global.module, global.symbol), bytes))
        })
        .collect();
    super::super::registry::metadata_startups(&raw.functions, &summaries, &keys, &bytes)
}

#[test]
fn frame_display_cannot_alias_nominal_tables_layouts_or_subtype_claims() {
    let ir = startup();
    assert!(certified(&[(MODULE, &ir)], |_| {}).contains("df.register.dart$3atest"));
    let frame = i64::from(crate::lower::async_sm::ASYNC_FRAME_ID);
    for variant in 0..12 {
        let accepted = certified(&[(MODULE, &ir), ("dart:other", "")], |summaries| {
            let summary = &mut summaries[usize::from(variant >= 6)];
            match variant % 6 {
                0 => summary.nominal_classes.push(frame),
                1 => summary.ambiguous_classes.push(frame),
                2 => summary.parent_edges.push((frame, 900)),
                3 => summary.parent_edges.push((900, frame)),
                4 => {
                    let node = summary.id("frame_table");
                    summary.tables.push(crate::pruning::SummarizedTable {
                        node,
                        cid: frame,
                        pairs: vec![],
                    });
                }
                _ => {
                    summary.declared_field_layouts.insert(frame as u32, vec![]);
                }
            }
        });
        assert!(accepted.is_empty(), "variant {variant}");
    }
}

#[test]
fn shared_display_is_idempotent_but_never_a_subtype_certificate() {
    let ir = startup();
    let other = ir.replace("dart$3atest", "dart$3aother");
    assert_eq!(
        certified(&[(MODULE, &ir), ("dart:other", &other)], |_| {}),
        HashSet::from([
            "df.register.dart$3atest".into(),
            "df.register.dart$3aother".into()
        ])
    );
    let frame = crate::lower::async_sm::ASYNC_FRAME_ID;
    for (child, parent) in [(frame, 900), (900, frame)] {
        let ir = format!(
            "; df.cone-class {frame}\n; df.cone-class 900\n; df.cone-parent {child} {parent}\ndefine void @df.register.dart$3atest() {{\n call void @dartforge_register_subclass(i64 {child}, i64 {parent})\n ret void\n}}\n"
        );
        assert!(certified(&[(MODULE, &ir)], |_| {}).is_empty());
    }
}
