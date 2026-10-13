//! O verificador do IR do modo mapas, antes do `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, no espírito do
//! `GCInvariantVerifier` do Julia), sobre a forma do §14.12: o valor `Ref` é
//! `i64` em todo o código, e as raízes vivas numa chamada que pode coletar
//! vão no operando `"deopt"` dela.
//!
//! Confere, no texto do módulo:
//!
//! * nenhum `ptr addrspace(1)`: a forma não tem ponteiro do coletor, e um
//!   que aparecesse entraria no mapa como raiz relocável que o runtime não
//!   lê;
//! * toda chamada que pode coletar (`super::collecting_call`) dentro de
//!   uma função definida leva o operando `"deopt"` — fora os intermediários
//!   `@df.vararg.<k>` (`noinline`, a chamada variádica não vira
//!   statepoint). Sem ele, a chamada não teria raízes no mapa, nem as de
//!   quem a chama depois do inlining;
//! * toda extern declarada `"gc-leaf-function"` está na tabela de efeitos
//!   sem coletar nem chamar Dart (salvo a sabotagem `leaf:<name>`).
//!
//! Uma violação é erro de compilação: o mapa sairia com uma raiz a menos ou
//! com um valor que não é referência.

/// O nome da função de uma linha `declare … @name(…)`.
fn declared_name(line: &str) -> Option<&str> {
    let after = &line[line.find('@')? + 1..];
    Some(&after[..after.find('(')?])
}

/// Confere o IR de um módulo em modo mapas.
///
/// # Errors
/// A primeira violação, com a linha.
pub fn verify(ir: &str) -> Result<(), String> {
    let mut inside = false;
    let mut intermediate = false;
    for (n, line) in ir.lines().enumerate() {
        let l = line.trim();
        let error = |reason: &str| Err(format!("verificador do modo mapas, linha {}: {reason}: `{l}`", n + 1));
        let definition = line.starts_with("define ");
        if definition {
            inside = true;
            intermediate = line.contains("@df.vararg.");
        } else if line.starts_with('}') {
            inside = false;
        }
        if l.contains("addrspace(1)") {
            return error("ponteiro `addrspace(1)`, que a forma das raízes por `\"deopt\"` não tem");
        }
        // O corpo dos ajudantes sai sem indentação: toda linha entre o
        // `define` e o `}` conta.
        if inside && !definition && !intermediate && super::collecting_call(line) && !line.contains("\"deopt\"(") {
            return error("chamada que pode coletar sem o operando `\"deopt\"`");
        }
        if l.starts_with("declare ")
            && l.ends_with("\"gc-leaf-function\"")
            && let Some(name) = declared_name(l)
            && !name.starts_with("llvm.")
        {
            let ef = crate::llvm::externs::effects_of(name);
            if (ef.allocates || ef.calls_dart) && !crate::alvo::sabotaged_leaf(name) {
                return error("folha (`gc-leaf-function`) que coleta pela tabela de efeitos");
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

    /// As violações plantadas: cada uma tem de ser achada.
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
            assert!(verify(&ir).is_err(), "não achou: {p}");
        }
    }
}
