use super::*;

#[test]
fn cross_inventory_word_and_uniform_contracts_for_the_same_arity_conflict() {
    let mut uniform = inventory(
        "uniform",
        "df.arc.ports.uniform",
        &[("shared", "uniform", 0, "declare i64 @shared(i64, ptr, ptr)")],
    );
    define(&mut uniform, "main", &["shared"]);
    define(&mut uniform, "shared", &[]);
    let word = inventory(
        "word",
        "df.arc.ports.word",
        &[("shared", "word", 3, "declare i64 @shared(i64, ptr, ptr)")],
    );
    for summaries in [[&uniform, &word], [&word, &uniform]] {
        let reachable = reach(&summaries, true);
        let error = assemble(
            &mut String::new(),
            &summaries,
            &reachable,
            true,
            &mut HashSet::new(),
        )
        .unwrap_err();
        assert!(error.contains("conflicting ARC word/uniform contracts"));
    }
}

#[test]
fn numeric_unquoted_llvm_target_names_are_rejected() {
    let summary = inventory(
        "sdk",
        "df.arc.ports.sdk",
        &[("123target", "word", 0, "declare i64 @123target()")],
    );
    assert!(validate(&summary).is_err());
    assert!(Summary::from_string(&summary.to_text()).is_err());
}

#[test]
fn typed_inventory_accepts_twelve_arguments_and_rejects_thirteen() {
    for count in [12, 13] {
        let abi = (0..count).fold(1_i64, |abi, _| abi * 5 + 2) * 5 + 1;
        let params = std::iter::repeat_n("i64", count + 1)
            .collect::<Vec<_>>()
            .join(", ");
        let declaration = format!("declare i64 @target({params})");
        let summary = inventory(
            "sdk",
            "df.arc.ports.sdk",
            &[("target", "typed", abi, &declaration)],
        );
        assert_eq!(validate(&summary).is_ok(), count == 12);
        assert_eq!(
            Summary::from_string(&summary.to_text()).is_ok(),
            count == 12
        );
    }
}

fn inventory(module: &str, registration: &str, ports: &[(&str, &str, i64, &str)]) -> Summary {
    let mut summary = Summary::new(module);
    let registration = summary.id(registration);
    let ports = ports
        .iter()
        .map(|&(target, kind, contract, declaration)| Port {
            target: summary.id(target),
            kind: kind.into(),
            contract,
            declaration: declaration.into(),
        })
        .collect();
    summary.arc_ports.push(Inventory {
        registration,
        ports,
    });
    summary
}

fn define(summary: &mut Summary, name: &str, refs: &[&str]) -> u32 {
    let name = summary.id(name);
    let refs = refs.iter().map(|s| summary.id(s)).collect();
    summary.definitions.push(Definition {
        name,
        refs,
        calls: vec![],
    });
    name
}

fn linked(summaries: &[&Summary], prune: bool) -> String {
    let reachable = reach(summaries, prune);
    let mut output = String::new();
    assemble(
        &mut output,
        summaries,
        &reachable,
        prune,
        &mut HashSet::new(),
    )
    .unwrap();
    output
}

#[test]
fn empty_inventory_round_trips_and_emits_an_empty_registration() {
    let summary = inventory("sdk", "df.arc.ports.sdk", &[]);
    assert_eq!(Summary::from_string(&summary.to_text()).unwrap(), summary);
    let output = linked(&[&summary], true);
    assert!(output.contains("define void @df.arc.ports.sdk()"));
    assert!(!output.contains("ptrtoint"));
}

#[test]
fn inventory_addresses_do_not_root_cold_code_and_no_pruning_preserves_it() {
    let mut summary = inventory(
        "sdk",
        "df.arc.ports.sdk",
        &[("cold", "uniform", 0, "declare i64 @cold(i64, ptr, ptr)")],
    );
    define(&mut summary, "main", &["df.arc.ports.sdk"]);
    define(&mut summary, "cold", &[]);
    let proof = reach(&[&summary], true);
    assert!(!proof.live(summary.index["cold"]));
    assert!(!linked(&[&summary], true).contains("@cold"));
    let unpruned = linked(&[&summary], false);
    assert!(unpruned.contains("ptrtoint ptr @cold"));
    assert!(unpruned.contains("@dartforge_arc_register_uniform_owned_v1"));
}

#[test]
fn live_selector_closure_callback_and_typed_targets_keep_their_contracts() {
    let mut summary = inventory(
        "sdk",
        "df.arc.ports.sdk",
        &[
            (
                "selector",
                "uniform",
                0,
                "declare i64 @selector(i64, ptr, ptr)",
            ),
            (
                "closure",
                "uniform",
                0,
                "declare i64 @closure(i64, ptr, ptr)",
            ),
            ("callback", "word", 1, "declare i64 @callback(i64)"),
            ("typed", "typed", 32, "declare i64 @typed(i64, i64)"),
            ("cold", "uniform", 0, "declare i64 @cold(i64, ptr, ptr)"),
        ],
    );
    let main = define(
        &mut summary,
        "main",
        &["table", "closure", "callback", "typed"],
    );
    for name in ["selector", "closure", "callback", "typed", "cold"] {
        define(&mut summary, name, &[]);
    }
    let table = summary.id("table");
    summary.tables.push(SummarizedTable {
        node: table,
        cid: 32001,
        pairs: vec![(selector_hash("c:invoke"), summary.index["selector"])],
    });
    summary
        .exact_receiver_calls
        .push((main, selector_hash("c:invoke"), 32001));
    let output = linked(&[&summary], true);
    for target in ["selector", "closure", "callback", "typed"] {
        assert!(output.contains(&format!("ptrtoint ptr @{target}")));
    }
    assert!(!output.contains("@cold"));
    assert!(output.contains("@dartforge_arc_register_return_owned_v1"));
    assert!(output.contains("@dartforge_arc_register_typed_v1"));
}

#[test]
fn multiple_typed_contracts_and_distinct_sdk_registration_symbols_survive() {
    let mut first = inventory(
        "first",
        "df.arc.ports.first",
        &[
            ("shared", "typed", 6, "declare i64 @shared(i64)"),
            ("shared", "typed", 7, "declare i64 @shared(i64)"),
        ],
    );
    define(&mut first, "main", &["shared", "second"]);
    define(&mut first, "shared", &[]);
    let mut second = inventory(
        "second",
        "df.arc.ports.second",
        &[("second", "word", 0, "declare i64 @second()")],
    );
    define(&mut second, "second", &[]);
    let output = linked(&[&first, &second], true);
    assert!(output.contains("define void @df.arc.ports.first()"));
    assert!(output.contains("define void @df.arc.ports.second()"));
    assert_eq!(output.matches("ptrtoint ptr @shared").count(), 2);
    assert_eq!(output.matches("declare i64 @shared(i64)").count(), 1);
    assert!(output.contains(", i64 6)"));
    assert!(output.contains(", i64 7)"));
}

#[test]
fn duplicate_registrations_and_cross_module_signatures_are_rejected() {
    let mut first = inventory(
        "first",
        "df.arc.ports.first",
        &[("shared", "word", 0, "declare i64 @shared()")],
    );
    define(&mut first, "main", &["shared"]);
    define(&mut first, "shared", &[]);
    let second = inventory(
        "second",
        "df.arc.ports.second",
        &[("shared", "word", 1, "declare i64 @shared(i64)")],
    );
    let reachable = reach(&[&first, &second], true);
    assert!(
        assemble(
            &mut String::new(),
            &[&first, &second],
            &reachable,
            true,
            &mut HashSet::new()
        )
        .is_err()
    );
    let duplicate = inventory("duplicate", "df.arc.ports.first", &[]);
    let reachable = reach(&[&first, &duplicate], true);
    assert!(
        assemble(
            &mut String::new(),
            &[&first, &duplicate],
            &reachable,
            true,
            &mut HashSet::new()
        )
        .is_err()
    );
}

#[test]
fn malformed_serialized_inventory_records_and_stale_headers_are_rejected() {
    let summary = inventory(
        "sdk",
        "df.arc.ports.sdk",
        &[("target", "uniform", 0, "declare i64 @target(i64, ptr, ptr)")],
    );
    let text = summary.to_text();
    for record in [
        "V\t99999\n",
        "V\t-1\n",
        "B\t0\t99999\tuniform:0:declare i64 @target(i64, ptr, ptr)\n",
        "B\t0\t1\tunknown:0:declare i64 @target(i64, ptr, ptr)\n",
        "B\t0\t1\tword:6:declare i64 @target(i64, i64, i64, i64, i64, i64)\n",
        "B\t0\t1\tword:-1:declare i64 @target()\n",
        "B\t0\t1\tuniform:1:declare i64 @target(i64, ptr, ptr)\n",
        "B\t0\t1\tuniform:0:declare i8 @target(i64, ptr, ptr)\n",
        "B\t0\t1\tuniform:0:declare i64 @different(i64, ptr, ptr)\n",
        "B\t0\t1\tuniform:0:declare i64 @target(i64, i64, ptr)\n",
        "B\t0\t1\ttyped:0:declare i64 @target(i64)\n",
        "B\t0\t1\ttyped:1:declare i64 @target(i64)\n",
        "B\t0\t1\ttyped:5:declare i64 @target(i64)\n",
        "B\t0\t1\ttyped:7:declare double @target(i64)\n",
        "B\t0\t1\ttyped:7:declare i64 @target(ptr)\n",
        "B\t0\t1\ttyped:7:declare i64 @target(i64, i64)\n",
        "B\t0\t1\ttyped:7:declare i64 @target(i64); injected\n",
    ] {
        assert!(
            Summary::from_string(&format!("{text}{record}")).is_err(),
            "accepted {record}"
        );
    }
    assert!(Summary::from_string(&text.replace("dartforge-poda\t6", "dartforge-poda\t5")).is_err());
    assert!(Summary::from_string("dartforge-poda\t6\tsdk\nN\tdf.arc.ports.sdk\nN\ttarget\nB\t0\t1\tword:0:declare i64 @target()\n").is_err());
}

#[test]
fn deferred_ir_inventory_is_nonretaining_but_eager_ir_keeps_addresses_live() {
    let deferred = "declare void @df.arc.ports.sdk()\n; df.arc-ports df.arc.ports.sdk\n; df.arc-port df.arc.ports.sdk\tuniform\t0\tdeclare i64 @cold(i64, ptr, ptr)\ndefine void @dartforge_entry() {\n  call void @df.arc.ports.sdk()\n  ret void\n}\ndefine i64 @cold(i64 %r, ptr %a, ptr %d) {\n  ret i64 0\n}\n";
    let summary = summarize(deferred, "sdk", &[]);
    assert!(!reach(&[&summary], true).live(summary.index["cold"]));
    assert!(!linked(&[&summary], true).contains("@cold"));
    let eager = "define private void @df.arc.register_ports() {\n  %code = ptrtoint ptr @cold to i64\n  %version = call i64 @dartforge_arc_register_uniform_owned_v1(i64 %code)\n  ret void\n}\ndefine void @dartforge_entry() {\n  call void @df.arc.register_ports()\n  ret void\n}\ndefine i64 @cold(i64 %r, ptr %a, ptr %d) {\n  ret i64 0\n}\n";
    let summary = summarize(eager, "sdk", &[]);
    assert!(summary.arc_ports.is_empty());
    assert!(reach(&[&summary], true).live(summary.index["cold"]));
}
