//! Diagnostics of the ARC preparation over complete modules lowered from source.
//! Publishes no bodies nor emits a program with partially changed conventions.

use dartforge_emit_native::{context::Context, driver, optimize, optimize::arc::*, sdk_module};
use dartforge_intern::Interner;
use dartforge_types::table::{CoreTypes, TypeTable};
use std::{collections::{BTreeMap, HashMap}, path::Path};

fn inventory_externs(modules: &[dartforge_emit_native::hir::Module]) {
    use dartforge_emit_native::hir::Instruction;
    let mut inventory = BTreeMap::<String, (usize, BTreeMap<String, usize>, String)>::new();
    for f in modules.iter().flat_map(|m| &m.functions) {
        for (v, inst, ty) in f.blocks.iter().flat_map(|b| &b.instructions) {
            let Instruction::CallRuntime { name, args, ret_ty } = inst else { continue };
            if contract(name).is_ok() { continue; }
            let types: Vec<_> = args.iter().map(|(_, ty)| ty).collect();
            let signature = format!("{types:?} -> {ret_ty:?} (SSA {ty:?})");
            let e = inventory.entry(name.clone()).or_insert_with(||
                (0, BTreeMap::new(), format!("{} v{}", f.symbol, v.0)));
            e.0 += 1;
            *e.1.entry(signature).or_default() += 1;
        }
    }
    println!("Inventory before the ARC producers: {} symbols without a direct contract in the catalog; does not certify ABI, reach or ownership", inventory.len());
    for (name_str, (n, signatures, example)) in inventory {
        println!("Extern without a direct contract: {name_str}: {n} calls; example {example}");
        for (signature, n) in signatures { println!("  {n} x {signature}"); }
    }
}

fn audit(entry: &Path, sdk_in_group: bool, externs_inventory: bool) -> Result<(), String> {
    let lib = std::env::var_os("DARTFORGE_SDK_LIB").ok_or("defina DARTFORGE_SDK_LIB")?;
    let sdk_dir = Path::new(&lib);
    let sdk = sdk_module::load_native_sdk(sdk_dir)?;
    let mut interner = Interner::new();
    let (program, diags) =
        dartforge_elements::load::load_lenient(entry, &sdk, None, &mut interner);
    if !diags.is_empty() {
        return Err(format!("carga: {diags:?}"));
    }
    let mut types = TypeTable::new();
    let core = CoreTypes::init(&mut types, &program, &interner);
    let (mut outline, diags) =
        dartforge_types::resolve_outline(&program, &interner, &mut types, &core);
    let (bodies, body_diags) = dartforge_types::infer_program_bodies(
        &program,
        &interner,
        &mut types,
        &core,
        &mut outline,
    );
    for d in diags.iter().chain(&body_diags) {
        if dartforge_types::codes::is_compile_error(&d.message) {
            return Err(format!("typing: {d:?}"));
        }
    }
    let te = dartforge_emit_native::erasure::compute(&program, &outline, &mut types);
    let mut ctx = Context::new(&program, &interner, &types, &core, &outline, &bodies);
    ctx.te = te;
    ctx.arc_memory = true;
    ctx.world = dartforge_emit_native::native_world::compute(
        &program, &interner, &types, &outline, &bodies,
    );
    let ctx = ctx.with_source_sdk_and_ids(sdk_module::sdk_class_ids(sdk_dir)?);
    let mut m = dartforge_emit_native::lower::lower_program(&ctx);
    m.program_ids = ctx.program_ids();
    m.program_fields = ctx.program_fields();
    m.sdk_records = sdk_module::sdk_records();
    m.runtime_cids = sdk_module::runtime_cids(&ctx);
    if !m.errors.is_empty() {
        return Err(format!("lowering: {:?}", m.errors));
    }
    if !sdk_in_group {
        let clang = driver::NativeDriverOptions::default().clang;
        let compiled_sdk = sdk_module::compiled_sdk_in_profile(
            sdk_dir,
            &clang,
            sdk_module::SdkProfile::Production,
        )?;
        let summaries = dartforge_emit_native::pruning::read_summaries(&compiled_sdk.summaries)?;
        let refs = summaries.iter().map(|r| r.as_ref()).collect::<Vec<_>>();
        dartforge_emit_native::pruning::prune_hir(&mut m, &refs);
    }
    optimize::optimize(&mut m);
    let by_tables = dartforge_emit_native::target::table_exceptions()?;
    if by_tables {
        optimize::table_exceptions(&mut m);
    }
    if sdk_in_group {
        let mut modules = vec![m];
        modules.extend(lower_sdk_for_audit(sdk_dir, by_tables)?);
        if externs_inventory { inventory_externs(&modules); }
        let mut plans: Vec<HashMap<_, _>> = modules
            .iter()
            .map(|m| {
                m.functions
                    .iter()
                    .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
                    .collect()
            })
            .collect();
        let quantity: usize = modules.iter().map(|m| m.functions.len()).sum();
        let stopwatch = modules
            .iter()
            .flat_map(|m| &m.functions)
            .filter(|f| f.symbol == "df.dart$3acore.Stopwatch.new")
            .count();
        println!(
            "{}: {} modules, {quantity} functions, {stopwatch} Stopwatch.new body(ies)",
            entry.display(),
            modules.len()
        );
        for (index, module) in modules.iter().enumerate() {
            match audit_shared_ref_slots(module) {
                Ok(slots) => println!(
                    "Shared Ref slots in module {index}: {} functions, {} addresses",
                    slots.len(), slots.values().map(|s| s.len()).sum::<usize>()
                ),
                Err(error) => println!(
                    "Shared Ref slots in module {index}: {error}"
                ),
            }
            match lower_shared_ref_slots(module) {
                Ok(draft) => {
                    println!(
                        "Intermediate lowering of the slots in module {index}: {} local frames, {} new signatures",
                        draft.locals.len(), draft.symbols.len()
                    );
                    let mut slot_plans = draft.functions.iter().map(|f|
                        (f.symbol.clone(), DartFunctionPlan::default())).collect();
                    let roles = produce_cfg_ref_slot_plans(module, &draft, &mut slot_plans);
                    println!("Native roles of the slots in module {index}: {roles:?}");
                }
                Err(error) => println!("Intermediate lowering of the slots in module {index}: {error}"),
            }
        }
        let before = format!("{modules:?}/{plans:?}");
        let result = prepare_arc_dart_modules(&mut modules, &mut plans);
        println!("Joint preparation with the source SDK: {result:?}");
        if let Err(error) = &result {
            let size = modules.iter().flat_map(|m| &m.functions)
                .filter(|f| error.contains(&f.symbol)).map(|f| f.symbol.len()).max();
            for (index, m) in modules.iter().enumerate() {
                for f in m.functions.iter().filter(|f| Some(f.symbol.len()) == size && error.contains(&f.symbol)) {
                    println!("Associated body in module {index}: {f:#?}");
                    let mut calls = 0;
                    for caller in modules.iter().flat_map(|m| &m.functions) {
                        for (v, i, _) in caller.blocks.iter().flat_map(|b| &b.instructions) {
                            if let dartforge_emit_native::hir::Instruction::CallStatic { symbol, args, .. } = i
                                && symbol == &f.symbol
                            {
                                calls += 1;
                                println!("Capture caller {} v{}: {args:?}", caller.symbol, v.0);
                                for arg in args {
                                    if let dartforge_emit_native::hir::Operand::Val(id) = arg {
                                        let definition = caller.blocks.iter().flat_map(|b| &b.instructions)
                                            .find(|(v, _, _)| v == id);
                                        println!("Origin of argument v{}: {definition:?}", id.0);
                                    }
                                }
                            }
                        }
                    }
                    println!("Total direct callers of {}: {calls}", f.symbol);
                }
            }
        }

        if result.is_err() && format!("{modules:?}/{plans:?}") != before {
            return Err("rejected joint preparation changed modules/plans".into());
        }
        return Ok(());
    }
    if externs_inventory { inventory_externs(std::slice::from_ref(&m)); }
    let mut plans: HashMap<_, _> = m
        .functions
        .iter()
        .map(|f| (f.symbol.clone(), DartFunctionPlan::default()))
        .collect();
    println!(
        "{}: {} functions, {} layouts, {} sites",
        entry.display(),
        m.functions.len(),
        m.arc_field_layouts.len(),
        m.arc_sites.len()
    );
    let before = format!("{m:?}/{plans:?}");
    let result = if by_tables {
        prepare_arc_tabled_module(&mut m, &mut plans)
    } else {
        prepare_arc_dart_module(&mut m, &mut plans)
    };
    println!("Whole-module preparation: {result:?}");
    if let Err(error) = result {
        if format!("{m:?}/{plans:?}") != before {
            return Err("rejected preparation changed the module or the plans".into());
        }
        for f in &m.functions {
            if error.contains(&f.symbol)
                || f.blocks
                    .iter()
                    .flat_map(|b| &b.instructions)
                    .any(|(_, i, _)| match i {
                        dartforge_emit_native::hir::Instruction::CallRuntime { name, .. } => {
                            error.contains(name)
                        }
                        dartforge_emit_native::hir::Instruction::CallStatic { symbol, .. } => {
                            error.contains(symbol)
                        }
                        _ => false,
                    })
            {
                println!("Body associated with the diagnostic: {f:#?}");
            }
        }
    }
    Ok(())
}

// Lowers the same bodies/libraries that the SDK compilation uses. The group
// stays private to the diagnostics: it does not reuse bitcode with the old ABI.
fn lower_sdk_for_audit(
    sdk_dir: &Path,
    by_tables: bool,
) -> Result<Vec<dartforge_emit_native::hir::Module>, String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let entry = tmp.path().join("main.dart");
    let mut source: String = sdk_module::SOURCE_LIBRARIES
        .iter()
        .map(|b| format!("import 'dart:{b}';\n"))
        .collect();
    source.push_str("void main() {}\n");
    std::fs::write(&entry, source).map_err(|e| e.to_string())?;
    let sdk = sdk_module::load_native_sdk(sdk_dir)?;
    let mut interner = Interner::new();
    let (program, diags) =
        dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
    if !diags.is_empty() {
        return Err(format!("carga SDK: {diags:?}"));
    }
    let mut types = TypeTable::new();
    let core = CoreTypes::init(&mut types, &program, &interner);
    let (mut outline, _) =
        dartforge_types::resolve_outline(&program, &interner, &mut types, &core);
    let libs: Vec<_> = sdk_module::SOURCE_LIBRARIES
        .iter()
        .filter_map(|b| {
            let uri = format!("dart:{b}");
            program
                .libraries
                .iter()
                .position(|l| l.uri == uri)
                .map(|i| dartforge_elements::model::LibraryId(i as u32))
        })
        .collect();
    if libs.len() != sdk_module::SOURCE_LIBRARIES.len() {
        return Err("incomplete SDK libraries".into());
    }
    let (bodies, _) = dartforge_types::infer_library_bodies(
        &program,
        &interner,
        &mut types,
        &core,
        &mut outline,
        &libs,
    );
    let ids = sdk_module::sdk_class_ids(sdk_dir)?;
    let mut modules = Vec::new();
    for lib in libs {
        let mut ctx = Context::new(&program, &interner, &types, &core, &outline, &bodies)
            .with_source_sdk_and_ids(ids.clone())
            .only_the_library(lib);
        ctx.arc_memory = true;
        let mut m = dartforge_emit_native::lower::lower_program(&ctx);
        m.sdk_library = true;
        m.runtime_cids = sdk_module::runtime_cids(&ctx);
        if !m.errors.is_empty() {
            return Err(format!(
                "lowering SDK {}: {:?}",
                program.library(lib).uri,
                m.errors
            ));
        }
        optimize::optimize(&mut m);
        if by_tables { optimize::table_exceptions(&mut m); }
        println!(
            "SDK {}: {} functions",
            program.library(lib).uri,
            m.functions.len()
        );
        // Checks compatibility with the set shared by the previous
        // emitter; these names produce no ownership contracts.
        let previous_ones: std::collections::HashSet<_> = m.functions.iter()
            .filter(|f| f.symbol.ends_with("$tear") || f.symbol.ends_with("$tearm") || f.symbol.starts_with("dfc."))
            .map(|f| f.symbol.clone()).collect();
        if previous_ones != m.arc_shared_functions {
            return Err(format!("incomplete SDK sharing declarations in {}", program.library(lib).uri));
        }
        modules.push(m);
    }
    Ok(modules)
}

fn main() -> Result<(), String> {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let mut entries = Vec::new();
            let mut selected_arc = false;
            let mut sdk_in_group = false;
            let mut externs_inventory = false;
            for argument in std::env::args_os().skip(1) {
                if argument == "--memoria=arc" {
                    selected_arc = true;
                } else if argument == "--sdk-no-grupo" {
                    sdk_in_group = true;
                } else if argument == "--inventario-externs" {
                    externs_inventory = true;
                } else {
                    entries.push(argument);
                }
            }
            // The explicit selection of the diagnostics must match the memory
            // key used by the cache/compilation of the SDK libraries.
            if !selected_arc || !dartforge_emit_native::target::arc_memory()? {
                return Err("the audit requires --memoria=arc and DARTFORGE_MEMORIA=arc".into());
            }
            if entries.is_empty() {
                return Err("informe os arquivos Dart a auditar".into());
            }
            for entry in entries {
                audit(Path::new(&entry), sdk_in_group, externs_inventory)?;
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "panic in the audit".to_string())?
}
