//! Allocation-only witnesses for the emitter's exact thread-context lookups.
//! Pointer validity, callback invocation and registry exposure remain unproved.
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(super) enum Lookup {
    Tsd,
    NativeTls(String),
}

// These reviewed shapes are deliberately independent of the emitter helpers.
// Changing generated code must invalidate this witness until its allocation
// behavior is reviewed again. Sharing an instruction template would grant new
// effects automatically when the generator changes.
const REVIEWED_TSD_READ: &str =
    "%t = call i64 asm \"mrs $0, TPIDRRO_EL0\", \"=r\"() nounwind memory(none)";

fn reviewed_tsd(attribute: &str, single_image: bool) -> String {
    let preemption = if single_image { "dso_local " } else { "" };
    format!(
        "\n@dartforge_context_slot = external {preemption}global i64, align 8\n\
         define internal ptr @df.context() nounwind willreturn memory(none){attribute} {{\n\
         \x20 %o = load i64, ptr @dartforge_context_slot, align 8, !invariant.load !{{}}\n\
         \x20 {REVIEWED_TSD_READ}\n\
         \x20 %b = and i64 %t, -8\n\
         \x20 %a = add i64 %b, %o\n\
         \x20 %p = inttoptr i64 %a to ptr\n\
         \x20 %c = load ptr, ptr %p, align 8, !invariant.load !{{}}\n\
         \x20 ret ptr %c\n\
         }}\n"
    )
}

fn reviewed_native_tls(variable: &str, attribute: &str) -> String {
    const CALL: &str = "call ptr @dartforge_context()";
    format!(
        "\n@{variable} = internal thread_local global ptr null, align 8\n\
         define internal ptr @df.context() nounwind{attribute} {{\n\
         \x20 %p = call ptr @llvm.threadlocal.address.p0(ptr @{variable})\n\
         \x20 %c = load ptr, ptr %p, align 8\n\
         \x20 %z = icmp eq ptr %c, null\n\
         \x20 br i1 %z, label %lento, label %pronto\n\
         lento:\n\
         \x20 %n = {CALL}\n\
         \x20 store ptr %n, ptr %p, align 8\n\
         \x20 ret ptr %n\n\
         pronto:\n\
         \x20 ret ptr %c\n\
         }}\n"
    )
}

fn binding(line: &str) -> Option<(bool, String)> {
    let line = line.trim_start();
    let declaration = line.split_whitespace().next()? == "declare";
    if !line.starts_with('@') && !matches!(line.split_whitespace().next()?, "define" | "declare") {
        return None;
    }
    let name = super::table_getters::canonical_symbol(crate::pruning::cited(line).next()?)?;
    Some((declaration, name))
}

fn opaque_assembly(line: &str) -> bool {
    let Some(line) = super::uncomment(line) else {
        return true;
    };
    if !line.contains("asm") || line.trim() == REVIEWED_TSD_READ {
        return false;
    }
    let mut unquoted = String::new();
    let mut start = 0;
    while let Some(offset) = line[start..].find('"') {
        let quote = start + offset;
        unquoted.push_str(&line[start..quote]);
        unquoted.push(' ');
        let Some(end) = super::quoted_end(line.as_bytes(), quote) else {
            return true;
        };
        start = end;
    }
    unquoted.push_str(&line[start..]);
    unquoted.split_whitespace().any(|word| word == "asm")
}

/// Reject ambiguous storage/symbols and opaque module assembly before matching
/// independently reviewed shapes. Names alone never establish an allocation effect.
pub(super) fn observe(modules: &[(&str, &str)]) -> HashMap<(String, String), Lookup> {
    let mut result = HashMap::new();
    if !modules
        .iter()
        .any(|(_, ir)| ir.contains("define internal ptr @df.context()"))
    {
        return result;
    }
    // A multiline/malformed quoted operand cannot hide a binding from this
    // line-oriented certificate. Module assembly can define invisible aliases.
    if modules.iter().any(|(_, ir)| {
        ir.lines().any(|line| {
            super::uncomment(line).is_none()
                || line.split_whitespace().next() == Some("module")
                || opaque_assembly(line)
                || (matches!(line.split_whitespace().next(), Some("define" | "declare"))
                    && binding(line).is_none())
        })
    }) {
        return result;
    }
    let shadowed = modules.iter().any(|(_, ir)| {
        ir.lines().any(|line| {
            binding(line).is_some_and(|(declaration, name)| {
                !declaration
                    && matches!(
                        name.as_str(),
                        "dartforge_context" | "llvm.threadlocal.address.p0"
                    )
            })
        })
    });
    for &(module, ir) in modules {
        let mut counts = HashMap::<String, usize>::new();
        let mut well_formed = true;
        for line in ir.lines() {
            if let Some((_, name)) = binding(line) {
                *counts.entry(name).or_default() += 1;
            } else if matches!(line.split_whitespace().next(), Some("define" | "declare")) {
                well_formed = false;
            }
        }
        if !well_formed || counts.get("df.context") != Some(&1) {
            continue;
        }
        let key = (module.to_string(), "df.context".to_string());
        for attribute in ["", " alwaysinline", " noinline"] {
            if counts.get("dartforge_context_slot") == Some(&1)
                && [false, true]
                    .into_iter()
                    .any(|single_image| ir.contains(&reviewed_tsd(attribute, single_image)))
            {
                result.insert(key.clone(), Lookup::Tsd);
                break;
            }
            if shadowed {
                continue;
            }
            let declarations: Vec<_> = ir
                .lines()
                .filter(|line| {
                    binding(line).is_some_and(|(declaration, name)| {
                        declaration && name == "llvm.threadlocal.address.p0"
                    })
                })
                .collect();
            if declarations.len() != 1
                || !matches!(
                    declarations[0],
                    "declare ptr @llvm.threadlocal.address.p0(ptr)"
                        | "declare nonnull ptr @llvm.threadlocal.address.p0(ptr nonnull)"
                )
            {
                continue;
            }
            for line in ir.lines() {
                let Some(variable) = line.strip_prefix('@').and_then(|line| {
                    line.strip_suffix(" = internal thread_local global ptr null, align 8")
                }) else {
                    continue;
                };
                if counts.get(variable) == Some(&1)
                    && variable
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"_$.-".contains(&byte))
                    && ir.contains(&reviewed_native_tls(variable, attribute))
                {
                    result.insert(key.clone(), Lookup::NativeTls(variable.to_string()));
                }
            }
        }
    }
    result
}

/// Preserve pointer/native exposure and every ordinary call. The exact TSD
/// instruction only reads a CPU register; TLS address computation touches no
/// Dart heap. The runtime context call still goes through normal SCC closure.
pub(super) fn analyze(definition: &mut super::Definition, lookup: &Lookup) {
    definition.exposures.extend([
        "thread_context_pointer".into(),
        "raw_memory_or_pointer_operation".into(),
        "global_or_function_address".into(),
    ]);
    if let Lookup::NativeTls(variable) = lookup {
        definition.native_tls_lookup = true;
        for call in [
            format!("call ptr @llvm.threadlocal.address.p0(ptr @{variable})"),
            "call ptr @dartforge_context()".into(),
        ] {
            let (symbol, abi, arguments) =
                super::signature(&call).expect("checked emitter call ABI");
            definition.calls.push(super::Call {
                symbol,
                abi,
                arguments,
            });
        }
    } else {
        definition.exposures.insert("inline_assembly".into());
    }
}
