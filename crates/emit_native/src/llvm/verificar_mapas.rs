//! O verificador do IR do modo mapas, antes do `rewrite-statepoints-for-gc`
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.4, no espírito do
//! `GCInvariantVerifier` do Julia), sobre a forma implementada do §14.8: o
//! valor `Ref` continua `i64`, e o ponteiro `addrspace(1)` só existe como a
//! raiz `%raiz<v>` de um valor `%v<v>`, mantida viva por `llvm.fake.use`.
//!
//! Confere, no texto do módulo:
//!
//! * nenhuma conversão para `ptr addrspace(1)` fora da forma
//!   `%raiz<v> = inttoptr i64 %v<v> to ptr addrspace(1)` (e a da sabotagem
//!   `bruto_no_mapa`, quando ligada);
//! * nenhum `undef`/`poison` de tipo `ptr addrspace(1)`;
//! * nenhum `ptr addrspace(1)` gravado em memória;
//! * toda extern declarada `"gc-leaf-function"` está na tabela de efeitos
//!   sem coletar nem chamar Dart (salvo a sabotagem `folha:<nome>`).
//!
//! Uma violação é erro de compilação: o mapa sairia com uma raiz a menos ou
//! com um valor que não é referência.
//! Escrito sem compilar nem executar (2026-10-05).

/// A forma de uma raiz: `%raiz<v> = inttoptr i64 %v<v> to ptr addrspace(1)`.
fn e_raiz(linha: &str) -> bool {
    let Some(resto) = linha.strip_prefix("%raiz") else { return false };
    let n = resto.bytes().take_while(u8::is_ascii_digit).count();
    if n == 0 {
        return false;
    }
    let (v, resto) = resto.split_at(n);
    resto == format!(" = inttoptr i64 %v{v} to ptr addrspace(1)")
}

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
    let sabotagem_bruta = crate::alvo::sabotagem("bruto_no_mapa");
    for (n, linha) in ir.lines().enumerate() {
        let l = linha.trim();
        let erro = |motivo: &str| Err(format!("verificador do modo mapas, linha {}: {motivo}: `{l}`", n + 1));
        if l.contains("to ptr addrspace(1)") && !e_raiz(l) && !(sabotagem_bruta && l == "%raizsab = inttoptr i64 %dfsabv to ptr addrspace(1)") {
            return erro("conversão para `ptr addrspace(1)` fora das raízes do emissor");
        }
        if l.contains("ptr addrspace(1) undef") || l.contains("ptr addrspace(1) poison") {
            return erro("`undef`/`poison` de tipo `ptr addrspace(1)`");
        }
        if l.starts_with("store ptr addrspace(1)") || l.starts_with("store volatile ptr addrspace(1)") {
            return erro("`ptr addrspace(1)` gravado em memória");
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
                  \x20 %raiz1 = inttoptr i64 %v1 to ptr addrspace(1)\n\
                  \x20 %v2 = call i64 @dartforge_string_concat(i64 %v1, i64 %v1)\n\
                  \x20 call void (...) @llvm.fake.use(ptr addrspace(1) %raiz1)\n\
                  \x20 ret i64 %v2\n\
                  }\n";
        assert_eq!(verificar(ir), Ok(()));
        assert!(e_raiz("%raiz12 = inttoptr i64 %v12 to ptr addrspace(1)"));
        assert!(!e_raiz("%raiz12 = inttoptr i64 %v13 to ptr addrspace(1)"));
    }

    /// As violações plantadas: cada uma tem de ser achada.
    #[test]
    fn acha_as_violacoes_plantadas() {
        let plantadas = [
            "  %x = inttoptr i64 2 to ptr addrspace(1)",
            "  %raiz3 = inttoptr i64 %v4 to ptr addrspace(1)",
            "  call void (...) @llvm.fake.use(ptr addrspace(1) undef)",
            "  %p = phi ptr addrspace(1) [ ptr addrspace(1) poison, %b0 ]",
            "  store ptr addrspace(1) %raiz1, ptr %slot",
            "declare i64 @dartforge_string_concat(i64, i64) \"gc-leaf-function\"",
        ];
        for p in plantadas {
            let ir = format!("define void @f() {{\n{p}\n  ret void\n}}\n");
            assert!(verificar(&ir).is_err(), "não achou: {p}");
        }
    }
}
