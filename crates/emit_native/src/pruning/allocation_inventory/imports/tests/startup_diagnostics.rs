//! Startup diagnostics describe missing premises without granting capabilities.
use super::*;

fn startup(target: &str, body: &str) -> String {
    format!(
        "@name = private constant [1 x i8] c\"h\"\ndefine void @df.register.dart$3atest() {{\n call void @dartforge_register_helper(ptr @name, i64 1, {target})\n ret void\n}}\n{body}"
    )
}

fn detail(ir: &str) -> (tempfile::TempDir, Inventory) {
    let (directory, names, runtime) = fixtures(ir);
    let inventory = Inventory::load(directory.path(), "test-key", true, &names, &runtime).unwrap();
    (directory, inventory)
}

#[test]
fn helper_diagnostics_distinguish_definitions_and_allocation_coverage() {
    for (target, body, expected) in [
        ("ptr @target", "", "definition missing"),
        ("ptr null", "", "unresolved helper target operand"),
        (
            "ptr @target",
            "define i64 @target() {\n %r = call i64 @opaque()\n ret i64 %r\n}\n",
            "allocation closure unknown",
        ),
        (
            "ptr @target",
            "define i64 @target() { ret i64 0 }\n",
            "allocation footprint observed; invocation coverage unproved",
        ),
        (
            "ptr @target",
            "define i64 @target() { ret i64 0 }\ndefine i64 @target() { ret i64 0 }\n",
            "ambiguous definition",
        ),
        (
            "ptr @target",
            "define weak i64 @target() { ret i64 0 }\n",
            "unproved definition or ABI",
        ),
    ] {
        let (_directory, inventory) = detail(&startup(target, body));
        let symbol = "df.register.dart$3atest";
        let diagnostic = inventory.entry_obligations(symbol).unwrap();
        assert!(diagnostic.contains(expected), "{diagnostic}");
        assert!(!inventory.entry(symbol));
        assert!(!inventory.call(symbol, &[], Type::Void));
        assert!(inventory.entry_obligations("absent").is_none());
    }
}

#[test]
fn local_helper_resolution_does_not_use_an_unrelated_global_body() {
    let ir = startup(
        "ptr @target",
        "define internal i64 @target() {\n %r = call i64 @opaque()\n ret i64 %r\n}\n",
    );
    let other = "define i64 @target() {\n ret i64 0\n}\n";
    let bundle = crate::pruning::allocation_inventory::closure::audit(&[
        (MODULE, &ir),
        ("dart:other", other),
    ]);
    let raw: Payload = serde_json::from_value(bundle.to_json()).unwrap();
    let mut keys: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, function) in raw.functions.iter().enumerate() {
        keys.entry(function.key.clone()).or_default().push(index);
    }
    let diagnostics = super::super::diagnostics::startup_obligations(&raw.functions, &keys);
    assert!(diagnostics["df.register.dart$3atest"].contains("allocation closure unknown"));
}

#[test]
fn certified_metadata_and_pure_entries_have_no_rejection_diagnostic() {
    let pure = format!("define void @{LEAF}() {{\n ret void\n}}\n");
    let metadata = "@arg = private constant [1 x i8] c\"a\"\ndefine void @df.register.dart$3atest() {\n call void @dartforge_register_argument_name(ptr @arg, i64 1)\n ret void\n}\n";
    for (ir, symbol) in [(pure.as_str(), LEAF), (metadata, "df.register.dart$3atest")] {
        let (_directory, inventory) = detail(ir);
        assert!(inventory.entry(symbol));
        assert!(inventory.entry_obligations(symbol).is_none());
    }
}

#[test]
fn large_startup_diagnostics_are_deterministic_and_bound_the_target_list() {
    let mut ir = String::from(
        "@name = private constant [1 x i8] c\"h\"\ndefine void @df.register.dart$3atest() {\n",
    );
    for target in ["z", "c", "b", "a"] {
        ir.push_str(&format!(
            " call void @dartforge_register_helper(ptr @name, i64 1, ptr @{target})\n"
        ));
    }
    ir.push_str(" ret void\n}\n");
    let (_directory, inventory) = detail(&ir);
    let diagnostic = inventory
        .entry_obligations("df.register.dart$3atest")
        .unwrap();
    assert!(diagnostic.contains("helper targets: \"a\": definition missing, \"b\": definition missing, \"c\": definition missing, ... (4 total)"), "{diagnostic}");
    assert!(!diagnostic.contains("\"z\""));
}
