//! Removes direct local functions that lost every entry path.
//! The producer declares the candidates; names and Ptr parameters do not prove closure.

use crate::hir::*;
use std::collections::{HashMap, HashSet};

pub(super) fn prune(m: &mut Module) {
    if !m.arc_memory
        || m.arc_direct_local_functions.is_empty()
        || (!m.tables.is_empty() && m.tables.len() != m.functions.len())
    {
        return;
    }
    let mut live_set: HashSet<String> = m
        .functions
        .iter()
        .filter(|f| !m.arc_direct_local_functions.contains(&f.symbol))
        .map(|f| f.symbol.clone())
        .collect();
    live_set.extend(m.entry_symbol.iter().cloned());
    live_set.extend(m.call_dart.iter().cloned());
    live_set.extend(m.start_rti.iter().cloned());
    live_set.extend(m.record.iter().cloned());
    live_set.extend(m.sdk_records.iter().cloned());
    live_set.extend(m.source_roots.iter().cloned());
    live_set.extend(m.helpers.iter().map(|(_, s)| s.clone()));
    live_set.extend(m.ffi_callbacks.iter().map(|c| c.body.clone()));
    live_set.extend(m.ffi_trampolines.iter().map(|(_, s)| s.clone()));
    live_set.extend(m.table_functions.values().cloned());
    live_set.extend(m.arc_uniform_returns.iter().cloned());
    live_set.extend(m.arc_shared_functions.iter().cloned());
    live_set.extend(m.stack_trace_tearoffs.iter().map(|(s, _)| s.clone()));
    for c in &m.classes {
        live_set.extend(c.vtable.iter().map(|(_, s)| s.clone()));
        live_set.extend(c.to_string_symbol.iter().cloned());
    }
    for (_, table, entries) in &m.method_tables {
        live_set.insert(table.clone());
        live_set.extend(entries.iter().map(|(_, s)| s.clone()));
    }
    let by_name: HashMap<_, _> = m.functions.iter().map(|f| (f.symbol.as_str(), f)).collect();
    let mut queue: Vec<_> = live_set.iter().cloned().collect();
    while let Some(s) = queue.pop() {
        let Some(f) = by_name.get(s.as_str()) else {
            continue;
        };
        let mut cite = |s: &String| {
            if live_set.insert(s.clone()) {
                queue.push(s.clone());
            }
        };
        for b in &f.blocks {
            for (_, i, _) in &b.instructions {
                match i {
                    Instruction::CallStatic { symbol, .. } => cite(symbol),
                    Instruction::Const(Constant::Function(symbol)) => cite(symbol),
                    Instruction::TearOff { code_symbol }
                    | Instruction::AllocClosure { code_symbol, .. } => cite(code_symbol),
                    Instruction::AllocClosureTyped {
                        code_symbol, typed, ..
                    } => {
                        cite(code_symbol);
                        cite(typed);
                    }
                    Instruction::FunctionTable(ss) => {
                        for s in ss {
                            cite(s);
                        }
                    }
                    _ => {}
                }
                super::operands::operands(i, &mut |o| {
                    if let Operand::Constant(Constant::Function(s)) = o {
                        cite(s);
                    }
                });
            }
            super::operands::terminator_operands(&b.terminator, &mut |o| {
                if let Operand::Constant(Constant::Function(s)) = o {
                    cite(s);
                }
            });
        }
    }
    let dead_set: HashSet<_> = m
        .functions
        .iter()
        .filter(|f| {
            m.arc_direct_local_functions.contains(&f.symbol) && !live_set.contains(&f.symbol)
        })
        .map(|f| f.symbol.clone())
        .collect();
    if dead_set.is_empty() {
        return;
    }
    if !m.tables.is_empty() {
        m.tables = std::mem::take(&mut m.tables)
            .into_iter()
            .zip(&m.functions)
            .filter_map(|(t, f)| (!dead_set.contains(&f.symbol)).then_some(t))
            .collect();
    }
    m.functions.retain(|f| !dead_set.contains(&f.symbol));
    m.arc_direct_local_functions
        .retain(|s| !dead_set.contains(s));
    m.dart_scalar_parameters
        .retain(|s, _| !dead_set.contains(s));
    m.dart_rti_parameters.retain(|s, _| !dead_set.contains(s));
    m.dart_vector_parameters
        .retain(|s, _| !dead_set.contains(s));
    m.dart_ref_returns.retain(|s| !dead_set.contains(s));
    m.arc_sites.retain(|s, _| !dead_set.contains(s));
    m.source_errors.retain(|(s, _)| !dead_set.contains(s));
    m.rejected.retain(|(s, _)| !dead_set.contains(s));
    m.arc_aot_ports = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_registers_direct_capture_and_arc_pipeline_removes_callless_body() {
        std::thread::Builder::new().stack_size(64 << 20).spawn(|| {
            use crate::context::Context;
            use dartforge_intern::Interner;
            use dartforge_types::table::{CoreTypes, TypeTable};
            let dir = tempfile::tempdir().unwrap();
            let entry = dir.path().join("main.dart");
            std::fs::write(&entry,
                "void main() { var x = 1; void local() { x = x + 1; } if (false) { local(); } }\n").unwrap();
            let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
            let mut interner = Interner::new();
            let (program, ds) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
            assert!(ds.is_empty(), "{ds:?}");
            let mut types = TypeTable::new();
            let core = CoreTypes::init(&mut types, &program, &interner);
            let (mut outline, ds) = dartforge_types::resolve_outline(&program, &interner, &mut types, &core);
            let (bodies, ds_bodies) = dartforge_types::infer_program_bodies(
                &program, &interner, &mut types, &core, &mut outline);
            for d in ds.iter().chain(&ds_bodies) {
                assert!(!dartforge_types::codes::is_compile_error(&d.message), "{d:?}");
            }
            let mut ctx = Context::new(&program, &interner, &types, &core, &outline, &bodies);
            for arc in [false, true] {
                ctx.arc_memory = arc;
                let mut m = crate::lower::lower_program(&ctx);
                assert!(m.errors.is_empty(), "{:?}", m.errors);
                let locals: Vec<_> = m.functions.iter().filter(|f| f.name == "local")
                    .map(|f| f.symbol.clone()).collect();
                assert_eq!(locals.len(), 1);
                assert_eq!(m.arc_direct_local_functions.contains(&locals[0]), arc);
                super::super::optimize(&mut m);
                assert_eq!(m.functions.iter().any(|f| f.symbol == locals[0]), !arc);
                assert!(m.arc_direct_local_functions.is_empty());
            }
        }).unwrap().join().unwrap();
    }

    fn f(s: &str, target: Option<&str>) -> Function {
        Function {
            symbol: s.into(),
            name: s.into(),
            params: vec![],
            return_ty: Type::Void,
            debug_info: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: target
                    .into_iter()
                    .map(|a| {
                        (
                            ValueId(0),
                            Instruction::CallStatic {
                                symbol: a.into(),
                                args: vec![],
                                ret_ty: Type::Void,
                            },
                            Type::Void,
                        )
                    })
                    .collect(),
                terminator: Terminator::Return(None),
            }],
        }
    }

    #[test]
    fn cycle_without_entry_vanishes_but_called_chain_and_public_bodies_stay() {
        let mut m = Module::new();
        m.arc_memory = true;
        m.functions = vec![
            f("publica", Some("a")),
            f("a", Some("b")),
            f("b", None),
            f("morta", Some("recursiva")),
            f("recursiva", Some("morta")),
            f("externa", None),
        ];
        m.arc_direct_local_functions = ["a", "b", "morta", "recursiva"].map(String::from).into();
        m.dart_ref_returns.insert("morta".into());
        m.arc_sites.insert("morta".into(), Default::default());
        m.tables = vec![Default::default(); m.functions.len()];
        m.tables[2].checks_stack = true;
        prune(&mut m);
        assert_eq!(
            m.functions
                .iter()
                .map(|f| f.symbol.as_str())
                .collect::<Vec<_>>(),
            ["publica", "a", "b", "externa"]
        );
        assert_eq!(m.tables.len(), 4);
        assert!(m.tables[2].checks_stack);
        assert!(m.dart_ref_returns.is_empty());
        assert!(m.arc_sites.is_empty());
        let before = format!("{m:?}");
        prune(&mut m);
        assert_eq!(format!("{m:?}"), before);
    }

    #[test]
    fn addresses_in_operands_terminators_and_published_entries_are_roots() {
        for case in 0..7 {
            let mut m = Module::new();
            m.arc_memory = true;
            m.functions = vec![
                f("publica", None),
                f("local", Some("filha")),
                f("filha", None),
            ];
            m.arc_direct_local_functions = ["local", "filha"].map(String::from).into();
            match case {
                0 => m.functions[0].blocks[0].instructions.push((
                    ValueId(0),
                    Instruction::Const(Constant::Function("local".into())),
                    Type::I64,
                )),
                1 => {
                    m.functions[0].blocks[0].terminator = Terminator::Return(Some(
                        Operand::Constant(Constant::Function("local".into())),
                    ))
                }
                2 => m.helpers.push(("nativo".into(), "local".into())),
                3 => {
                    m.table_functions.insert(1, "local".into());
                }
                4 => m.source_roots.push("local".into()),
                5 => m.method_tables.push((
                    1,
                    "tabela".into(),
                    vec![("call".into(), "local".into())],
                )),
                _ => m.entry_symbol = Some("local".into()),
            }
            prune(&mut m);
            assert_eq!(m.functions.len(), 3, "caso {case}");
        }
    }

    #[test]
    fn tracing_and_incomplete_exceptional_inventory_are_not_changed() {
        for arc in [false, true] {
            let mut m = Module::new();
            m.arc_memory = arc;
            m.functions = vec![f("publica", None), f("morta", None)];
            m.arc_direct_local_functions.insert("morta".into());
            if arc {
                m.tables.push(Default::default());
            }
            let before = format!("{m:?}");
            prune(&mut m);
            assert_eq!(format!("{m:?}"), before);
        }
    }
}
