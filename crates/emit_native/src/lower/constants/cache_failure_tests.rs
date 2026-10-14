use super::*;
use std::collections::HashSet;

fn ordinary_nested_constants(action: impl FnOnce(Module) + Send + 'static) {
    std::thread::Builder::new().stack_size(64 << 20).spawn(move || {
        let temporary = tempfile::tempdir().unwrap();
        let entry = temporary.path().join("main.dart");
        std::fs::write(&entry, "class Leaf { final int value; const Leaf(this.value); }\nclass Parent { final Leaf child = const Leaf(7); const Parent(); }\nvoid main() { print(const Parent().child.value); }\n").unwrap();
        let sdk = crate::sdk_module::load_native_sdk(crate::sdk_tests()).unwrap();
        let mut interner = dartforge_intern::Interner::new();
        let (program, diagnostics) = dartforge_elements::load::load_lenient(&entry, &sdk, None, &mut interner);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let mut table = dartforge_types::TypeTable::new();
        let core = dartforge_types::CoreTypes::init(&mut table, &program, &interner);
        let (mut outline, _) = dartforge_types::resolve_outline(&program, &interner, &mut table, &core);
        let libraries: Vec<_> = program.libraries.iter().enumerate().filter(|(_, l)| !l.is_sdk)
            .map(|(i, _)| dartforge_elements::model::LibraryId(i as u32)).collect();
        let (bodies, diagnostics) = dartforge_types::infer_library_bodies_with_locals(&program, &interner, &mut table, &core, &mut outline, &libraries);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let context = crate::context::Context::new(&program, &interner, &table, &core, &outline, &bodies).with_source_sdk();
        let module = crate::lower::lower_program(&context);
        assert!(module.errors.is_empty(), "{:?}", module.errors);
        action(module);
    }).unwrap().join().unwrap();
}

#[test]
fn nested_constant_failure_cannot_reach_parent_publication_after_optimization() {
    ordinary_nested_constants(|mut module| {
        crate::optimize::optimize(&mut module);
        let parent = module.functions.iter().find(|f| f.symbol.starts_with("dfc.") && f.blocks.iter()
            .flat_map(|b| &b.instructions).any(|(_, i, _)| matches!(i, Instruction::CallStatic { symbol, .. } if symbol.starts_with("dfc.")))).expect("actual nested parent cache getter");
        let (block, position) = parent.blocks.iter().find_map(|b| b.instructions.iter().position(|(_, i, _)|
            matches!(i, Instruction::CallStatic { symbol, .. } if symbol.starts_with("dfc."))).map(|p| (b,p))).unwrap();
        assert!(
            matches!(&block.instructions[position + 1].1, Instruction::CallRuntime { name, .. } if name == "dartforge_exception_pending"),
            "child stack-guard failure must still be checked before parent stores: {parent:#?}"
        );
        let Terminator::CondBranch {
            then_block: failure,
            else_block: success,
            ..
        } = block.terminator
        else {
            panic!("checked child must branch");
        };
        let mut pending = vec![failure];
        let mut visited = HashSet::new();
        let mut known = std::collections::HashMap::new();
        while let Some(id) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            assert_ne!(
                id, success,
                "exception path cannot reach the successful initializer"
            );
            let b = parent.blocks.iter().find(|b| b.id == id).unwrap();
            assert!(
                !b.instructions
                    .iter()
                    .any(|(_, i, _)| matches!(i, Instruction::StoreGlobal { .. })),
                "failed child must not publish value or ready flag: {parent:#?}"
            );
            // Inlined constructor failures can join the constructor's exit;
            // its caller must recheck the still-pending exception before publish.
            let bit = |op: &Operand, known: &std::collections::HashMap<ValueId, bool>| match op {
                Operand::Val(v) => known.get(v).copied(),
                Operand::Constant(Constant::Int(i)) => Some(*i != 0),
                _ => None,
            };
            for (v, instruction, _) in &b.instructions {
                match instruction {
                    Instruction::CallRuntime { name, .. }
                        if name == "dartforge_exception_pending" =>
                    {
                        known.insert(*v, true);
                    }
                    Instruction::CallRuntime { name, .. }
                        if name == "dartforge_exception_clear" =>
                    {
                        panic!("initializer failure cannot clear the exception")
                    }
                    Instruction::ICmp(ICmpOp::Ne, a, c) => {
                        if let (Some(a), Some(c)) = (bit(a, &known), bit(c, &known)) {
                            known.insert(*v, a != c);
                        }
                    }
                    _ => {}
                }
            }
            match &b.terminator {
                Terminator::Return(value) => {
                    assert_eq!(*value, Some(Operand::Constant(Constant::Null)))
                }
                Terminator::Branch(next) => pending.push(*next),
                Terminator::CondBranch {
                    cond,
                    then_block,
                    else_block,
                } => match bit(cond, &known) {
                    Some(true) => pending.push(*then_block),
                    Some(false) => pending.push(*else_block),
                    None => pending.extend([*then_block, *else_block]),
                },
                other => panic!("unexpected failure terminator {other:?}"),
            }
        }
        assert!(!visited.is_empty());
        let ir = crate::llvm::LlvmEmitter::new(&module).emit_all();
        assert!(
            ir.contains("call void @dartforge_stack_overflow()"),
            "actual emitted getter prologues are fallible"
        );
        #[cfg(target_os = "macos")]
        native_failure_and_retry(&module, parent, &ir);
    });
}

/// Runs actual emitted cache bodies against a bounded native test runtime. The
/// parent allocator moves the test stack limit above the child's frame exactly
/// once; the child fails its real emitted prologue, not an artificial Dart throw.
/// This does not validate the production allocator or error object construction.
#[cfg(target_os = "macos")]
fn native_failure_and_retry(module: &Module, parent: &Function, ir: &str) {
    let temporary = tempfile::tempdir().unwrap();
    let mut emitted = String::new();
    for line in ir
        .lines()
        .filter(|l| l.starts_with("declare ") || l.starts_with("@df.efn."))
    {
        emitted.push_str(line);
        emitted.push('\n');
    }
    for f in module
        .functions
        .iter()
        .filter(|f| f.symbol.starts_with("dfc.") && f.symbol.ends_with(".get"))
    {
        let definition = ir
            .lines()
            .find(|line| line.starts_with("define ") && line.contains(&format!("@{}(", f.symbol)))
            .unwrap();
        let start = ir.find(definition).unwrap();
        let end = start + ir[start..].find("\n}\n").unwrap() + 3;
        emitted.push_str(&ir[start..end]);
    }
    emitted.push_str("declare i64 @df.box_int(i64)\ndeclare void @df.barrier(i64, i64)\ndeclare ptr @df.get_area()\n");
    // Only table addresses are needed by the mock allocator, not registration.
    let mut table_stubs = String::new();
    for (i, (_, table, _)) in module.method_tables.iter().enumerate() {
        emitted.push_str(&format!("declare ptr @{table}()\n"));
        table_stubs.push_str(&format!(
            "void *table_{i}(void) __asm__(\"_{table}\"); void *table_{i}(void) {{ return 0; }}\n"
        ));
    }
    emitted.push_str(&format!(
        "define i64 @cache_test_parent() {{\n  %value = call i64 @{}()\n  ret i64 %value\n}}\n",
        parent.symbol
    ));
    let c = format!(
        r#"
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static _Alignas(8) unsigned char context[512];
static uint64_t area[2048];
static int allocations, marks;
static void *allocated[8];
void *dartforge_context(void) {{ return context; }}
void *context_alias(void) __asm__("_df.context");
void *context_alias(void) {{ return context; }}
void *area_alias(void) __asm__("_df.get_area");
void *area_alias(void) {{ return area; }}
int64_t box_int(int64_t value) __asm__("_df.box_int");
int64_t box_int(int64_t value) {{ assert(value == 7); return (value << 1) | 1; }}
void dartforge_stack_overflow(void) {{ context[0] = 1; }}
uint8_t dartforge_exception_pending(void) {{ return context[0]; }}
void dartforge_exception_clear(void) {{ context[0] = 0; }}
int64_t dartforge_object_new_t(int64_t cid, int64_t fields, void *table) {{
  (void)table;
  unsigned char *p = calloc(1, 16 + 8 * fields); assert(p);
  *(uint64_t *)p = (uint64_t)cid << 32;
  allocated[allocations++] = p;
  if (allocations == 1) *(uintptr_t *)(context + 376) = UINTPTR_MAX;
  return (intptr_t)p + 2;
}}
void barrier(int64_t a, int64_t b) __asm__("_df.barrier");
void barrier(int64_t a, int64_t b) {{ (void)a; (void)b; }}
int64_t dartforge_canonical_constant(int64_t value) {{ return value; }}
void dartforge_gc_global_root(int64_t root, int64_t value) {{ (void)root; (void)value; }}
void dartforge_mark_constant(int64_t value, int64_t getter) {{ assert(value); (void)getter; marks++; }}
void dartforge_effects_before(void *name, int64_t length, int64_t mask) {{ (void)name; (void)length; (void)mask; }}
void dartforge_effects_after(void) {{}}
{table_stubs}
int64_t parent(void) __asm__("_cache_test_parent");
int main(void) {{
  assert(parent() == 0); assert(context[0] == 1); assert(allocations == 1); assert(marks == 0);
  for (int i = 0; i < 2048; i++) assert(area[i] == 0);
  assert(*(uintptr_t *)(context + 8) == 0);
  context[0] = 0; *(uintptr_t *)(context + 376) = 0;
  int64_t value = parent(); assert(value != 0); assert(context[0] == 0);
  assert(allocations == 3); assert(marks == 2);
  int64_t child = *(int64_t *)((intptr_t)value - 2 + 16); assert(child != 0);
  assert(*(int64_t *)((intptr_t)child - 2 + 16) == 7);
  assert(parent() == value); assert(allocations == 3); assert(marks == 2);
  assert(*(uintptr_t *)(context + 8) == 0);
  puts("child-guard-failed; parent-unpublished; retry-valid; warm-identity-preserved");
  for (int i = 0; i < allocations; i++) free(allocated[i]);
  return 0;
}}
"#
    );
    let llvm = temporary.path().join("cache.ll");
    let helper = temporary.path().join("mock.c");
    let binary = temporary.path().join("probe");
    std::fs::write(&llvm, emitted).unwrap();
    std::fs::write(&helper, c).unwrap();
    let clang = std::env::var_os("DARTFORGE_CLANG").unwrap_or_else(|| "clang".into());
    let sdk = std::process::Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .unwrap();
    assert!(
        sdk.status.success(),
        "macOS SDK required for the native regression"
    );
    let sdk = String::from_utf8(sdk.stdout).unwrap();
    if let Some(output) = std::env::var_os("DARTFORGE_CONST_CACHE_PROBE_ARTIFACTS") {
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        std::fs::copy(&llvm, output.join("cache.ll")).unwrap();
        std::fs::copy(&helper, output.join("mock.c")).unwrap();
    }
    let compile = std::process::Command::new(clang)
        .args(["-O0", "-Wno-override-module", "-isysroot", sdk.trim()])
        .arg(&llvm)
        .arg(&helper)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let run = std::process::Command::new(binary).output().unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "child-guard-failed; parent-unpublished; retry-valid; warm-identity-preserved\n"
    );
}
