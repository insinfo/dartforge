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


// --- P1: convenção uniforme das closures (docs/NATIVO-PLANO.md §7.4) --------

/// Lê uma captura mutável como referência: um escalar guardado sai
/// encaixotado (R5), nunca como bits lidos por handle.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_cell_get_ref(handle: i64) -> i64 {
    let v = HEAP.with(|heap| heap.borrow().cell_get(handle));
    valor_como_ref(v)
}

/// Lê a posição `index` do ambiente como referência (escalar encaixotado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_env_get_ref(handle: i64, index: i64) -> i64 {
    let v = HEAP.with(|heap| {
        heap.borrow()
            .environment_get(handle, usize::try_from(index).expect("índice inválido"))
    });
    valor_como_ref(v)
}

/// O índice da entrada uniforme de uma closure na `@df_code_table`. Um
/// valor que não é closure (null, ou outro objeto chamado como função) deixa
/// `NoSuchMethodError` pendente e devolve 0, a entrada que só retorna.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_closure_entry(handle: i64) -> i64 {
    let codigo = HEAP.with(|heap| match heap.borrow().try_get(handle) {
        Some(Value::Closure { code_id, .. }) => Some(*code_id),
        _ => None,
    });
    codigo.unwrap_or_else(|| {
        dartforge_nsm_chamada();
        0
    })
}

/// Lança o `NoSuchMethodError` de uma chamada de valor função que não casa
/// (valor que não é função, ou aridade/nomes errados).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nsm_chamada() {
    let nome = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str("call"))));
    let erro = com_raizes(&[nome], || dartforge_no_such_method_error_new(nome));
    com_raizes(&[erro], || dartforge_exception_throw(erro, 3));
}

/// Confere um descritor de chamada `[n_pos, n_nom, hash…]` contra a
/// assinatura `[n_obrig, n_pos, n_nom, hash…, obrigatório…]` da função
/// chamada: posicionais entre os obrigatórios e o total, todo nomeado
/// passado existe, todo nomeado `required` foi passado.
///
/// # Safety
/// Os dois ponteiros são vetores constantes emitidos pelo compilador, com o
/// tamanho que os seus próprios cabeçalhos dizem.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_args_casam(desc: *const i64, sig: *const i64) -> u8 {
    // SAFETY: cabeçalhos de vetores constantes do emissor.
    let (npos, nnom) = unsafe { (*desc, *desc.add(1)) };
    let (obrig, total, snom) = unsafe { (*sig, *sig.add(1), *sig.add(2)) };
    if npos < obrig || npos > total {
        return 0;
    }
    let nnom = usize::try_from(nnom).unwrap_or(0);
    let snom = usize::try_from(snom).unwrap_or(0);
    // SAFETY: os tamanhos vêm dos cabeçalhos.
    let passados = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    let nomes = unsafe { std::slice::from_raw_parts(sig.add(3), snom) };
    let exigidos = unsafe { std::slice::from_raw_parts(sig.add(3 + snom), snom) };
    if passados.iter().any(|h| !nomes.contains(h)) {
        return 0;
    }
    for (h, req) in nomes.iter().zip(exigidos) {
        if *req != 0 && !passados.contains(h) {
            return 0;
        }
    }
    1
}

/// A posição (entre os nomeados) do argumento de nome `hash` no descritor,
/// ou -1 se ele não foi passado.
///
/// # Safety
/// `desc` é um descritor constante emitido pelo compilador.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arg_indice(desc: *const i64, hash: i64) -> i64 {
    // SAFETY: cabeçalho e nomes do descritor constante.
    let nnom = usize::try_from(unsafe { *desc.add(1) }).unwrap_or(0);
    let passados = unsafe { std::slice::from_raw_parts(desc.add(2), nnom) };
    passados
        .iter()
        .position(|h| *h == hash)
        .map_or(-1, |p| p as i64)
}

/// `Function._apply` da VM recebe `[função, posicionais…, nomeados…]` e os
/// nomes em uma segunda lista, já produzidas pelo patch Dart de `Function.apply`.
/// Recompõe a ABI uniforme das closures, inclusive o descritor de nomes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Function_apply(arguments: i64, names: i64) -> i64 {
    com_raizes(&[arguments, names], || {
        let count = lista_len(arguments).max(0) as usize;
        let named = lista_len(names).max(0) as usize;
        if count == 0 || named >= count {
            dartforge_nsm_chamada();
            return 0;
        }
        let function = dartforge_nativo_DartForge_lista_get(arguments, 0);
        let args: Vec<i64> = (1..count)
            .map(|i| dartforge_nativo_DartForge_lista_get(arguments, i as i64))
            .collect();
        let mut desc = vec![(count - 1 - named) as i64, named as i64];
        for i in 0..named {
            let value = dartforge_nativo_DartForge_lista_get(names, i as i64);
            let Some(name) = HEAP.with(|heap| match heap.borrow().try_get(value) {
                Some(Value::String(text)) => Some(text.para_string()),
                _ => None,
            }) else {
                dartforge_nsm_chamada();
                return 0;
            };
            // Mesmo FNV-1a 64 do descritor emitido em `lower/closures.rs`.
            let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            desc.push(hash as i64);
        }
        let code = dartforge_closure_entry(function);
        if code == 0 { return 0; }
        // SAFETY: `dartforge_closure_entry` devolve o endereço de uma entrada
        // uniforme gerada com assinatura (closure, argumentos, descritor).
        let entry: extern "C" fn(i64, *const i64, *const i64) -> i64 = unsafe { std::mem::transmute(code as usize) };
        entry(function, args.as_ptr(), desc.as_ptr())
    })
}

#[cfg(test)]
mod function_apply_tests {
    use super::*;

    extern "C" fn entry(_closure: i64, args: *const i64, desc: *const i64) -> i64 {
        // O patch Dart fornece 1 posicional e dois nomeados na ordem c,b.
        dartforge_gc_collect();
        let matches = unsafe {
            *desc == 1 && *desc.add(1) == 2
                && *desc.add(2) == hash("c") && *desc.add(3) == hash("b")
                && *args.add(1) == 0 && *args.add(2) == 0
        };
        let value_alive = HEAP.with(|heap| matches!(heap.borrow().try_get(unsafe { *args }), Some(Value::String(t)) if t.para_string() == "valor"));
        i64::from(matches && value_alive)
    }

    fn hash(name: &str) -> i64 {
        name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        }) as i64
    }

    #[test]
    fn function_apply_encaminha_descritor_nomeado_a_entrada_uniforme() {
        let function = dartforge_tearoff(entry as usize as i64);
        com_raizes(&[function], || {
            let (arguments, names) = HEAP.with(|heap| {
                let mut heap = heap.borrow_mut();
                let c = heap.allocate(Value::String(Texto::de_str("c")));
                let b = heap.allocate(Value::String(Texto::de_str("b")));
                let value = heap.allocate(Value::String(Texto::de_str("valor")));
                let arguments = heap.allocate(Value::List(vec![
                    TaggedValue::reference(function),
                    TaggedValue::reference(value),
                    TaggedValue::reference(0),
                    TaggedValue::reference(0),
                ]));
                let names = heap.allocate(Value::List(vec![TaggedValue::reference(c), TaggedValue::reference(b)]));
                (arguments, names)
            });
            assert_eq!(dartforge_nativo_Function_apply(arguments, names), 1);
        });
    }
}
