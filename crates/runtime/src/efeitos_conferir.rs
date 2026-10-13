// A conferência da tabela de efeitos das externs (`crates/runtime/efeitos.tsv`,
// docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8).
//
// Com `DARTFORGE_EFEITOS=conferir` na compilação, o código gerado chama
// `dartforge_efeitos_antes(name, marks)` antes e `dartforge_efeitos_depois()`
// depois de cada extern que a tabela marca `coleta = 0` (bit 0 das marcas) ou
// `lanca = 0` (bit 1). Aqui a marca é posta à prova:
//
// * `coleta = 0`: uma coleta enquanto a extern roda encerra o processo
//   (`heap::forbid_collection`), dizendo o nome dela;
// * `lanca = 0`: se a extern entrou sem exceção pendente e saiu com uma, o
//   processo é encerrado, dizendo o nome dela.
//
// A marca otimista errada é o defeito mais perigoso do emissor (uma raiz que
// falta, uma exceção que ninguém confere): só passa de 1 para 0 depois de o
// corpus rodar assim, de preferência com `--gc-stress`. Sem a variável na
// compilação, nada daqui é chamado.

thread_local! {
    /// As externs conferidas em execução nesta thread: (nome, marcas, a
    /// pendência na entrada).
    static EXTERNS_UNDER_CHECK: std::cell::RefCell<Vec<(String, i64, u8)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A entrada de uma extern conferida.
///
/// # Safety
/// `name` aponta para `len` bytes (o nome da extern, uma constante do módulo).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_efeitos_antes(name: *const u8, len: i64, marks: i64) {
    // SAFETY: garantido por quem chama.
    let bytes = unsafe { std::slice::from_raw_parts(name, usize::try_from(len).unwrap_or(0)) };
    let name = String::from_utf8_lossy(bytes).into_owned();
    let pending = dartforge_exception_pending();
    if marks & 1 != 0 {
        crate::heap::forbid_collection(name.clone());
    }
    EXTERNS_UNDER_CHECK.with(|p| p.borrow_mut().push((name, marks, pending)));
}

/// A saída da extern conferida mais recente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_depois() {
    let Some((name, marks, pending_before)) = EXTERNS_UNDER_CHECK.with(|p| p.borrow_mut().pop()) else {
        return;
    };
    if marks & 1 != 0 {
        crate::heap::allow_collection();
    }
    if marks & 2 != 0 && pending_before == 0 && dartforge_exception_pending() != 0 {
        eprintln!("dartforge: a extern {name} está marcada lanca = 0 em efeitos.tsv e deixou uma exceção pendente");
        std::process::abort();
    }
}

/// Retorna a profundidade das externs em conferência nesta thread.
///
/// ```
/// let level = dartforge_runtime::abi::dartforge_efeitos_nivel();
/// assert!(level >= 0);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_nivel() -> i64 {
    EXTERNS_UNDER_CHECK.with(|p| p.borrow().len() as i64)
}

/// Abandona conferências de externs atravessadas por unwind nativo.
///
/// Remove somente níveis acima do snapshot da entrada da função, liberando
/// suas proibições de coleta. Não confere uma saída normal nem altera pendência.
/// Não coleta nem executa Dart; o lowering restaura também as raízes no pouso.
///
/// # Panics
/// Nível negativo ou acima da profundidade corrente; aborta no limite C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let level = dartforge_efeitos_nivel();
/// dartforge_efeitos_restaurar(level);
/// assert_eq!(dartforge_efeitos_nivel(), level);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_efeitos_restaurar(level: i64) {
    let level = usize::try_from(level).expect("nível negativo de conferência de efeitos");
    EXTERNS_UNDER_CHECK.with(|p| {
        let mut stack = p.borrow_mut();
        assert!(level <= stack.len(), "nível de conferência de efeitos obsoleto");
        while stack.len() > level {
            let (_, marks, _) = stack.pop().unwrap();
            if marks & 1 != 0 { crate::heap::allow_collection(); }
        }
    });
}

#[cfg(test)]
mod tests_effects_restore {
    use super::*;
    #[test]
    fn unwind_preserves_outer_level_and_releases_abandoned_bans() {
        let initial = dartforge_efeitos_nivel();
        let name = b"fixture-unwind";
        // SAFETY: nome aponta para bytes válidos durante as duas chamadas.
        unsafe {
            dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 1);
            dartforge_efeitos_antes(name.as_ptr(), name.len() as i64, 1);
        }
        dartforge_efeitos_restaurar(initial + 1);
        assert_eq!(dartforge_efeitos_nivel(), initial + 1);
        dartforge_efeitos_restaurar(initial);
        assert_eq!(dartforge_efeitos_nivel(), initial);
        dartforge_arc_collect();
    }
}
