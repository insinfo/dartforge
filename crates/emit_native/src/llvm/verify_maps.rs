//! The IR verifier for maps mode, before `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, in the spirit of
//! Julia's `GCInvariantVerifier`), over the shape of §14.12: a `Ref` value is an
//! `i64` throughout the code, and the live roots at a call that may collect
//! go in its `"deopt"` operand.
//!
//! Checks, in the module text:
//!
//! * no `ptr addrspace(1)`: the shape has no collector pointer, and one
//!   that appeared would enter the map as a relocatable root that the runtime does not
//!   read;
//! * every call that may collect (`super::collecting_call`) inside a
//!   defined function carries the `"deopt"` operand — except for the
//!   `@df.vararg.<k>` intermediates (`noinline`, the variadic call does not become a
//!   statepoint). Without it, the call would have no roots in the map, nor would
//!   its callers after inlining;
//! * every extern declared `"gc-leaf-function"` is in the effects table
//!   without collecting nor calling Dart (except for the `folha:<name>` sabotage).
//!
//! A violation is a compile error: the map would come out with one root too few or
//! with a value that is not a reference.

/// The function name of a `declare … @name(…)` line.
fn declared_name(line: &str) -> Option<&str> {
    let after = &line[line.find('@')? + 1..];
    Some(&after[..after.find('(')?])
}

/// Checks the IR of a module in maps mode.
///
/// # Errors
/// The first violation, with the line.
pub fn verify(ir: &str) -> Result<(), String> {
    let mut inside = false;
    let mut intermediate = false;
    for (n, line) in ir.lines().enumerate() {
        let l = line.trim();
        let error = |reason: &str| Err(format!("maps mode verifier, line {}: {reason}: `{l}`", n + 1));
        let definition = line.starts_with("define ");
        if definition {
            inside = true;
            intermediate = line.contains("@df.vararg.");
        } else if line.starts_with('}') {
            inside = false;
        }
        if l.contains("addrspace(1)") {
            return error("`addrspace(1)` pointer, which the shape of roots by `\"deopt\"` does not have");
        }
        // The body of the helpers comes out without indentation: every line between the
        // `define` and the `}` counts.
        if inside && !definition && !intermediate && super::collecting_call(line) && !line.contains("\"deopt\"(") {
            return error("call that may collect without the `\"deopt\"` operand");
        }
        if l.starts_with("declare ")
            && l.ends_with("\"gc-leaf-function\"")
            && let Some(name) = declared_name(l)
            && !name.starts_with("llvm.")
        {
            let ef = crate::llvm::externs::effects_of(name);
            if (ef.allocates || ef.calls_dart) && !crate::alvo::sabotaged_leaf(name) {
                return error("leaf (`gc-leaf-function`) that collects according to the effects table");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_emitter_shape() {
        let ir = "define i64 @f(i64 %v1) gc \"statepoint-example\" {\n\
                  b0:\n\
                  \x20 %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1) [ \"deopt\"(i64 %v1) ]\n\
                  \x20 %v3 = call ptr @llvm.stacksave.p0()\n\
                  \x20 ret i64 %v2\n\
                  }\n\
                  define internal i64 @df.vararg.0(ptr %f, i64 %p0) noinline {\n\
                  \x20 %r = call i64 (i64, ...) %f(i64 %p0)\n\
                  \x20 ret i64 %r\n\
                  }\n";
        assert_eq!(verify(ir), Ok(()));
    }

    /// The planted violations: each one must be found.
    #[test]
    fn finds_the_planted_violations() {
        let planted = [
            "  %x = inttoptr i64 2 to ptr addrspace(1)",
            "  %p = phi ptr addrspace(1) [ ptr addrspace(1) poison, %b0 ]",
            "  %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1)",
            "  %v2 = call i64 %f(i64 %v1)",
            "%f = call ptr @dartforge_seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)",
            "declare i64 @dartforge_string_concat(i64, i64) \"gc-leaf-function\"",
        ];
        for p in planted {
            let ir = format!("define void @f() {{\n{p}\n  ret void\n}}\n");
            assert!(verify(&ir).is_err(), "not found: {p}");
        }
    }
}
