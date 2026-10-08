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
//! * toda chamada que pode coletar ([`super::chamada_que_coleta`]) dentro de
//!   uma função definida leva o operando `"deopt"` — fora os intermediários
//!   `@df.vararg.<k>` (`noinline`, a chamada variádica não vira
//!   statepoint). Sem ele, a chamada não teria raízes no mapa, nem as de
//!   quem a chama depois do inlining;
//! * toda extern declarada `"gc-leaf-function"` está na tabela de efeitos
//!   sem coletar nem chamar Dart (salvo a sabotagem `folha:<nome>`).
//!
//! Uma violação é erro de compilação: o mapa sairia com uma raiz a menos ou
//! com um valor que não é referência.

/// O nome da função de uma linha `declare … @nome(…)`.
fn nome_declarado(linha: &str) -> Option<&str> {
    let depois = &linha[linha.find('@')? + 1..];
    Some(&depois[..depois.find('(')?])
}

/// Confere o IR de um módulo em modo mapas.
///
/// # Errors
/// A primeira violação, com a linha.
pub fn verificar(ir: &str) -> Result<(), String> {
    let mut dentro = false;
    let mut intermediario = false;
    for (n, linha) in ir.lines().enumerate() {
        let l = linha.trim();
        let erro = |motivo: &str| Err(format!("verificador do modo mapas, linha {}: {motivo}: `{l}`", n + 1));
        let definicao = linha.starts_with("define ");
        if definicao {
            dentro = true;
            intermediario = linha.contains("@df.vararg.");
        } else if linha.starts_with('}') {
            dentro = false;
        }
        if l.contains("addrspace(1)") {
            return erro("ponteiro `addrspace(1)`, que a forma das raízes por `\"deopt\"` não tem");
        }
        // O corpo dos ajudantes sai sem indentação: toda linha entre o
        // `define` e o `}` conta.
        if dentro && !definicao && !intermediario && super::chamada_que_coleta(linha) && !linha.contains("\"deopt\"(") {
            return erro("chamada que pode coletar sem o operando `\"deopt\"`");
        }
        if l.starts_with("declare ")
            && l.ends_with("\"gc-leaf-function\"")
            && let Some(nome) = nome_declarado(l)
            && !nome.starts_with("llvm.")
        {
            let ef = crate::llvm::externs::efeitos_de(nome);
            if (ef.aloca || ef.chama_dart) && !crate::alvo::folha_sabotada(nome) {
                return erro("folha (`gc-leaf-function`) que coleta pela tabela de efeitos");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn aceita_a_forma_do_emissor() {
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
        assert_eq!(verificar(ir), Ok(()));
    }

    /// As violações plantadas: cada uma tem de ser achada.
    #[test]
    fn acha_as_violacoes_plantadas() {
        let plantadas = [
            "  %x = inttoptr i64 2 to ptr addrspace(1)",
            "  %p = phi ptr addrspace(1) [ ptr addrspace(1) poison, %b0 ]",
            "  %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1)",
            "  %v2 = call i64 %f(i64 %v1)",
            "%f = call ptr @dartforge_seletor(ptr %c, i64 %r, i64 %h, ptr %n, i64 %l)",
            "declare i64 @dartforge_string_concat(i64, i64) \"gc-leaf-function\"",
        ];
        for p in plantadas {
            let ir = format!("define void @f() {{\n{p}\n  ret void\n}}\n");
            assert!(verificar(&ir).is_err(), "não achou: {p}");
        }
    }
}
