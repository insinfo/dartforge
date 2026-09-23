// Runtime nativo: células, ambientes e closures.

/// Cria uma célula de captura mutável; o chamador a enraíza antes de coletar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_new(bits: i64, tag: u8) -> i64 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().create_cell(value))
}

/// Lê os bits de uma captura mutável, sem copiar o objeto de uma referência.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_bits(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().cell_get(handle).bits)
}

/// Lê a tag (1 = int, 2 = bool, 3 = referência) de uma captura mutável.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_tag(handle: i64) -> u8 {
    HEAP.with(|heap| untag(heap.borrow().cell_get(handle)).1)
}

/// Atualiza a captura observada por todos os ambientes que partilham a célula.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_set(handle: i64, bits: i64, tag: u8) {
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().cell_set(handle, value));
}

/// Cria um ambiente com `len` pares (bits, tag) lidos de `pairs`.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis; o emissor constrói o vetor
/// na pilha. Capturas mutáveis entram como handles de célula (tag 3).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_env_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let captures = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetor temporário do emissor, legível pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_environment(captures))
}

/// Obtém a captura (handle de célula) por índice do ambiente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_env_get(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        heap.borrow()
            .environment_get(handle, usize::try_from(index).expect("índice inválido"))
            .bits
    })
}

/// Cria uma closure com identidade própria sobre código simbólico e ambiente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_new(code_id: i64, env: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().create_closure(code_id, env))
}

/// Devolve o tear-off canônico de uma função top-level (mesmo handle sempre).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tearoff(code_id: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().tearoff(code_id))
}

/// Consulta o código simbólico de uma closure para despacho indireto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_code(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure_parts(handle).0)
}

/// Consulta o ambiente de uma closure para chamadas indiretas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_env(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().closure_parts(handle).1)
}

