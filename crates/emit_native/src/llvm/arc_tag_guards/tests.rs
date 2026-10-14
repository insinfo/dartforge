use super::*;
use crate::hir::*;
use crate::llvm::LlvmEmitter;
use dartforge_runtime::{abi, heap::Heap, layout::smi};
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn emitted_guards_execute_exact_runtime_handle_predicate_at_both_optimization_levels() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "dartforge-arc-tags-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let ir = format!(
        r#"declare void @dartforge_arc_retain(i64) nounwind
declare void @dartforge_arc_release(i64) nounwind
{HELPERS}
define void @guarded_retain(i64 %value) {{ call void @{RETAIN}(i64 %value) ret void }}
define void @guarded_release(i64 %value) {{ call void @{RELEASE}(i64 %value) ret void }}
"#
    );
    let llvm = directory.join("guards.ll");
    std::fs::write(&llvm, ir).unwrap();
    let values = [
        0,
        smi::of(smi::MIN).unwrap(),
        smi::of(smi::MAX).unwrap(),
        smi::of(-1).unwrap(),
        smi::of(0).unwrap(),
        smi::of(1).unwrap(),
        i64::MIN,
        -2,
        2,
        8,
        i64::MAX,
    ];
    let literals = values
        .iter()
        .map(|&value| {
            if value == i64::MIN {
                "INT64_MIN".into()
            } else {
                format!("{value}LL")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let c = directory.join("record.c");
    std::fs::write(&c, format!(r#"#include <stdint.h>
#include <assert.h>
static int retains, releases;
static int64_t retained, released;
_Alignas(8) static uint64_t bool_true[3], bool_false[3], permanent_string[3];
void dartforge_arc_retain(int64_t value) {{ retains++; retained = value; }}
void dartforge_arc_release(int64_t value) {{ releases++; released = value; }}
extern void guarded_retain(int64_t);
extern void guarded_release(int64_t);
int main(void) {{
  int64_t values[] = {{ {literals}, (int64_t)(uintptr_t)bool_true + 2, (int64_t)(uintptr_t)bool_false + 2, (int64_t)(uintptr_t)permanent_string + 2 }};
  for (unsigned i = 0; i < sizeof(values)/sizeof(values[0]); i++) {{
    int64_t value = values[i];
    int handle = value != 0 && (value & 1) == 0;
    int before_retain = retains, before_release = releases;
    guarded_retain(value); guarded_release(value);
    assert(retains == before_retain + handle && releases == before_release + handle);
    if (handle) assert(retained == value && released == value);
  }}
  return 0;
}}
"#)).unwrap();
    let clang = std::env::var("DARTFORGE_CLANG").unwrap_or_else(|_| "clang".into());
    for level in ["-O0", "-O2"] {
        let binary = directory.join(if cfg!(windows) {
            "program.exe"
        } else {
            "program"
        });
        let result = std::process::Command::new(&clang)
            .args(["-Wno-override-module", level])
            .args(crate::target::link_arguments())
            .arg(&llvm)
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = std::process::Command::new(&binary).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty() && result.stderr.is_empty());
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn canonical_runtime_inventory_counts_boolean_statics_and_heap_mints_but_not_smi() {
    std::thread::spawn(|| {
        abi::dartforge_memory_arc_v1();
        for value in [Heap::box_bool(false), Heap::box_bool(true)] {
            assert!(smi::e_handle(value));
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 0);
            abi::dartforge_arc_retain(value);
            abi::dartforge_arc_retain(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 2);
            abi::dartforge_arc_release(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 2);
            abi::dartforge_arc_release(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 0);
        }
        for n in [smi::MIN, -1, 0, 1, smi::MAX] {
            let value = abi::dartforge_arc_box_int_owned_v1(n);
            assert!(!smi::e_handle(value));
            abi::dartforge_arc_retain(value);
            abi::dartforge_arc_release(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 0);
        }
        for n in [smi::MIN - 1, smi::MAX + 1] {
            let value = abi::dartforge_arc_box_int_owned_v1(n);
            assert!(smi::e_handle(value));
            abi::dartforge_arc_retain(value);
            abi::dartforge_arc_release(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 3);
            abi::dartforge_arc_release(value);
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 1);
            abi::dartforge_arc_collect();
            assert_eq!(abi::dartforge_arc_observe_heap_v1(value), 0);
        }
    })
    .join()
    .unwrap();
}

fn module() -> Module {
    let mut module = Module::new();
    module.arc_memory = true;
    module.functions.push(Function {
        symbol: "dynamic_ref".into(),
        name: "dynamic_ref".into(),
        debug_info: None,
        params: vec![(ValueId(0), "value".into(), Type::Ref)],
        return_ty: Type::Void,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            instructions: vec![
                (
                    ValueId(1),
                    Instruction::ArcCopy {
                        value: Operand::Val(ValueId(0)),
                    },
                    Type::Ref,
                ),
                (
                    ValueId(2),
                    Instruction::ArcDrop {
                        value: Operand::Val(ValueId(1)),
                    },
                    Type::Void,
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    });
    module
}

#[test]
fn ablation_changes_only_physical_dispatch_and_guards_are_exact_leaf_functions() {
    let module = module();
    let before = format!("{:?}", module.functions);
    let mut on = LlvmEmitter::new(&module);
    on.arc_tag_guards = true;
    let on = on.emit_all();
    let mut off = LlvmEmitter::new(&module);
    off.arc_tag_guards = false;
    let off = off.emit_all();
    assert!(on.contains("call void @df.arc_retain_ref(i64 %v0)"));
    assert!(on.contains("call void @df.arc_release_ref(i64 %v1)"));
    assert!(off.contains("call void @dartforge_arc_retain(i64 %v0)"));
    assert!(off.contains("call void @dartforge_arc_release(i64 %v1)"));
    assert!(!off.contains("define internal void @df.arc_retain_ref"));
    assert_eq!(format!("{:?}", module.functions), before);
    for name in [RETAIN, RELEASE] {
        assert!(!super::super::collecting_call(&format!(
            "  call void @{name}(i64 %v0)"
        )));
        assert!(super::super::MAP_LEAVES.contains(&name));
    }
    assert_eq!(
        HELPERS
            .matches("alwaysinline nounwind \"gc-leaf-function\"")
            .count(),
        2
    );
    for forbidden in ["readnone", "readonly", "memory(none)", "willreturn"] {
        assert!(!HELPERS.contains(forbidden));
    }
    for name in ["dartforge_arc_retain", "dartforge_arc_release"] {
        let effects = crate::llvm::externs::effects_of(name);
        assert!(!effects.allocates && !effects.throws && !effects.calls_dart);
    }
}
