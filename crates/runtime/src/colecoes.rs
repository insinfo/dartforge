// Runtime nativo: listas, mapas, conjuntos e iterações ativas.

// As tabelas laterais por handle (coleções imutáveis, iterações ativas,
// origem das listas de chaves) moram no `Heap`, que as purga a cada coleta
// (G6, docs/NATIVO-PLANO.md §6.5).

fn register_key_iteration_origin(keys_list: i64, map_handle: i64) {
    HEAP.with(|h| h.borrow_mut().origens.insert(keys_list, map_handle));
}

fn is_active_iteration(handle: i64) -> bool {
    HEAP.with(|h| h.borrow().iteracoes_ativas.contains(&handle))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteration_begin(handle: i64) {
    if handle == 0 { return; }
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.iteracoes_ativas.insert(handle);
        if let Some(&origin) = h.origens.get(&handle) {
            h.iteracoes_ativas.insert(origin);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_iteration_end(handle: i64) {
    if handle == 0 { return; }
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        h.iteracoes_ativas.remove(&handle);
        if let Some(&origin) = h.origens.get(&handle) {
            h.iteracoes_ativas.remove(&origin);
        }
    });
}

/// `lista[i]` numa posição `Ref`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_ref(handle: i64, index: i64) -> i64 {
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::List(items) => usize::try_from(index).ok().and_then(|i| items.get(i).copied()),
            _ => None,
        }
    });
    match v {
        Some(v) => valor_como_ref(v),
        None => {
            // Fora dos limites: mesmo RangeError do acessor de bits.
            dartforge_list_get_bits(handle, index)
        }
    }
}

/// `first`/`last`/`single` numa posição `Ref`: o acessor de bits valida e
/// lança; aqui só se lê o elemento com a tag.
fn elemento_extremo_ref(handle: i64, qual: u8) -> i64 {
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::List(items) => match qual {
                0 => items.first().copied(),
                1 => items.last().copied(),
                _ => (items.len() == 1).then(|| items[0]),
            },
            _ => None,
        }
    });
    match (v, qual) {
        (Some(v), _) => valor_como_ref(v),
        (None, 0) => dartforge_list_first(handle),
        (None, 1) => dartforge_list_last(handle),
        (None, _) => dartforge_list_single(handle),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_first_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_last_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 1)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_single_ref(handle: i64) -> i64 {
    elemento_extremo_ref(handle, 2)
}

/// `mapa[chave]` (sempre anulável, logo `Ref`): ausente é null.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_ref(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    let v = HEAP.with(|heap| {
        let heap = heap.borrow();
        heap.map_contains(handle, key).then(|| heap.map_get(handle, key))
    });
    v.map_or(0, valor_como_ref)
}

/// Cria uma lista expansível com `len` pares (bits, tag) lidos de `pairs`.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis construídos pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_list_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let values = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetor temporário do emissor, legível pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_list(values))
}

/// Cria lista expansível vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_new_empty() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().create_list(Vec::new()))
}

/// Quantidade de elementos da lista.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().list_len(handle) as i64)
}

/// Lê o primeiro elemento da lista ou lança StateError se vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_first(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items[0].bits
        } else {
            0
        }
    })
}

/// Lê o último elemento da lista ou lança StateError se vazia.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_last(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items.last().unwrap().bits
        } else {
            0
        }
    })
}

/// Lê os bits do elemento, sem verificar limites: o emissor verifica antes.
///
/// O índice fora dos limites lança `RangeError` capturável em vez de abortar;
/// o emissor desvia para o tratador ao observar a exceção pendente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_bits(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        match heap_ref.get(handle) {
            Value::List(_) => {
                if index < 0 || index >= heap_ref.list_len(handle) as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                heap_ref.list_get(handle, index as usize).bits
            }
            Value::String(s) => {
                let units: Vec<u16> = s.encode_utf16().collect();
                if index < 0 || index >= units.len() as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                let u = units[index as usize];
                drop(heap_ref);
                match String::from_utf16(&[u]) {
                    Ok(ch_str) => HEAP.with(|h| h.borrow_mut().allocate(Value::String(ch_str))),
                    Err(_) => HEAP.with(|h| h.borrow_mut().allocate(Value::RawString(vec![u]))),
                }
            }
            Value::RawString(r) => {
                if index < 0 || index >= r.len() as i64 {
                    drop(heap_ref);
                    let err = allocate_range_error("RangeError");
                    dartforge_exception_throw(err, 3);
                    return 0;
                }
                let u = r[index as usize];
                drop(heap_ref);
                HEAP.with(|h| h.borrow_mut().allocate(Value::RawString(vec![u])))
            }
            Value::Match(s) => {
                if index == 0 {
                    let s_clone = s.clone();
                    drop(heap_ref);
                    HEAP.with(|h| h.borrow_mut().allocate(Value::String(s_clone)))
                } else {
                    0
                }
            }
            _ => 0,
        }
    })
}

/// Lê a tag (1 = int, 2 = bool, 3 = referência) do elemento da lista ou string.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_get_tag(handle: i64, index: i64) -> u8 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        match heap_ref.get(handle) {
            Value::List(_) => {
                if index < 0 || index >= heap_ref.list_len(handle) as i64 {
                    return 0;
                }
                untag(heap_ref.list_get(handle, index as usize)).1
            }
            Value::String(_) | Value::RawString(_) | Value::Match(_) => 3,
            _ => 0,
        }
    })
}

/// Substitui o elemento existente; limites verificados como na leitura.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_set(handle: i64, index: i64, bits: i64, tag: u8) {
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
    let value = tagged(bits, tag);
    HEAP.with(|heap| {
        if index < 0 || index >= heap.borrow().list_len(handle) as i64 {
            let err = allocate_range_error("RangeError");
            dartforge_exception_throw(err, 3);
            return;
        }
        heap.borrow_mut().list_set(handle, index as usize, value);
    });
}

/// Acrescenta ao fim (`List.add`); devolve void pela ABI do emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_push(handle: i64, bits: i64, tag: u8) {
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
    let value = tagged(bits, tag);
    HEAP.with(|heap| heap.borrow_mut().list_push(handle, value));
}

/// Cria um mapa com `len` chaves e valores lidos de dois vetores de pares.
///
/// # Safety
/// `keys` e `values` apontam para `2 * len` i64 legíveis cada.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_map_new(keys: *const i64, values: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let read = |ptr: *const i64| -> Vec<TaggedValue> {
        if len == 0 {
            Vec::new()
        } else {
            // SAFETY: vetores temporários do emissor, legíveis pelos `2 * len` i64.
            let raw = unsafe { std::slice::from_raw_parts(ptr, len * 2) };
            raw.chunks_exact(2)
                .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
                .collect()
        }
    };
    let keys = read(keys);
    let values = read(values);
    HEAP.with(|heap| {
        heap.borrow_mut()
            .create_map(keys.into_iter().zip(values).collect())
    })
}

/// Quantidade de pares do mapa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().map_len(handle) as i64)
}

/// Pertinência de chave (`containsKey`); nunca lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_contains(handle: i64, bits: i64, tag: u8) -> u8 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow().map_contains(handle, key)))
}

/// Lê os bits do valor; chave ausente devolve null (bits 0, tag verificada à parte).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_bits(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.map_contains(handle, key) {
            heap.map_get(handle, key).bits
        } else {
            0
        }
    })
}

/// Lê a tag do valor; chave ausente devolve 0 (ausência, não null tipado).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_tag(handle: i64, bits: i64, tag: u8) -> u8 {
    let key = tagged(bits, tag);
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if heap.map_contains(handle, key) {
            untag(heap.map_get(handle, key)).1
        } else {
            0
        }
    })
}

/// Obtém valor do mapa já convertido para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_get_to_string(handle: i64, bits: i64, tag: u8) -> i64 {
    let key = tagged(bits, tag);
    let val = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if heap_ref.map_contains(handle, key) {
            heap_ref.map_get(handle, key)
        } else {
            tagged(0, 3) // null
        }
    });
    dartforge_tagged_to_string(val.bits, untag(val).1)
}

/// Insere ou substitui (`map[chave] = valor` e `[]=`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_set(
    handle: i64,
    key_bits: i64,
    key_tag: u8,
    value_bits: i64,
    value_tag: u8,
) {
    if dartforge_collection_is_unmodifiable(handle) != 0 {
        let err = dartforge_unsupported_error_new(0);
        dartforge_exception_throw(err, 3);
        return;
    }
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return;
    }
    let key = tagged(key_bits, key_tag);
    let value = tagged(value_bits, value_tag);
    HEAP.with(|heap| heap.borrow_mut().map_set(handle, key, value));
}

/// Remove par do mapa; lança ConcurrentModificationError se em iteração ativa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_remove(handle: i64, key_bits: i64, key_tag: u8) -> i64 {
    if handle == 0 { return 0; }
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let target_key = TaggedValue {
            bits: key_bits,
            is_ref: key_tag == 3,
            tag: match key_tag {
                1 => crate::heap::ValueTag::Int,
                2 => crate::heap::ValueTag::Bool,
                3 => crate::heap::ValueTag::Ref,
                _ => crate::heap::ValueTag::Int,
            },
        };
        let Value::Map(entries) = heap.get_mut(handle) else { return 0; };
        if let Some(pos) = entries.iter().position(|(k, _)| k.bits == target_key.bits) {
            entries.remove(pos).1.bits
        } else {
            0
        }
    })
}

/// Retorna lista com as chaves do mapa, associando a origem para rastreamento de iteração.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_map_keys(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let Value::Map(entries) = heap_ref.get(handle) else { return 0; };
        let keys: Vec<TaggedValue> = entries.iter().map(|(k, _)| *k).collect();
        drop(heap_ref);
        let list_handle = heap.borrow_mut().create_list(keys);
        register_key_iteration_origin(list_handle, handle);
        list_handle
    })
}

/// Cria um conjunto com `len` pares (bits, tag); duplicadas conservam a primeira.
///
/// # Safety
/// `pairs` deve apontar para `2 * len` i64 legíveis construídos pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_set_new(pairs: *const i64, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let values = if len == 0 {
        Vec::new()
    } else {
        // SAFETY: vetores temporários do emissor, legíveis pelos `2 * len` i64.
        let raw = unsafe { std::slice::from_raw_parts(pairs, len * 2) };
        raw.chunks_exact(2)
            .map(|pair| tagged(pair[0], u8::try_from(pair[1]).expect("tag inválida")))
            .collect()
    };
    HEAP.with(|heap| heap.borrow_mut().create_set(values))
}

/// Quantidade de elementos únicos do conjunto.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_len(handle: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().set_len(handle) as i64)
}

/// Pertinência (`contains`); nunca lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_contains(handle: i64, bits: i64, tag: u8) -> u8 {
    let value = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow().set_contains(handle, value)))
}

/// Insere quando ausente (`Set.add`); devolve se houve inserção.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_set_add(handle: i64, bits: i64, tag: u8) -> u8 {
    if is_active_iteration(handle) {
        let err = dartforge_concurrent_modification_error_new(handle);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let value = tagged(bits, tag);
    HEAP.with(|heap| u8::from(heap.borrow_mut().set_add(handle, value)))
}

/// Comprimento genérico para .length (String, List, Map, Set).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_generic_len(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().count() as i64,
            Value::RawString(r) => r.len() as i64,
            Value::StringBuffer(b) => b.encode_utf16().count() as i64,
            Value::List(l) => l.len() as i64,
            Value::Map(m) => m.len() as i64,
            Value::Set(s) => s.len() as i64,
            _ => 0,
        }
    })
}

/// Retorna elementos da lista na ordem inversa.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_reversed(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let rev_items: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::List(items) = heap.get(handle) else { return Vec::new(); };
        items.iter().rev().copied().collect()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(rev_items)))
}

/// Concatena os elementos de uma lista usando um separador em string.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_join(handle: i64, sep_handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let joined = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let Value::List(items) = heap_ref.get(handle) else { return String::new(); };
        let sep = if sep_handle != 0 {
            if let Value::String(s) = heap_ref.get(sep_handle) { s.as_str() } else { "" }
        } else { "" };
        let mut parts = Vec::new();
        for item in items {
            let mut out = String::new();
            match item.tag {
                crate::heap::ValueTag::Int => out.push_str(&item.bits.to_string()),
                crate::heap::ValueTag::Bool => out.push_str(if item.bits != 0 { "true" } else { "false" }),
                crate::heap::ValueTag::Double => {
                    let d = f64::from_bits(item.bits as u64);
                    if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                        out.push_str(&format!("{d:.1}"));
                    } else {
                        out.push_str(&d.to_string());
                    }
                }
                crate::heap::ValueTag::Ref => {
                    if item.bits != 0 {
                        if let Value::String(s) = heap_ref.get(item.bits) {
                            out.push_str(s);
                        } else {
                            out.push_str(&describe_handle(&heap_ref, item.bits));
                        }
                    } else {
                        out.push_str("null");
                    }
                }
            }
            parts.push(out);
        }
        parts.join(sep)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(joined)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_mark_unmodifiable(handle: i64) -> i64 {
    HEAP.with(|h| h.borrow_mut().imutaveis.insert(handle));
    handle
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_collection_is_unmodifiable(handle: i64) -> u8 {
    HEAP.with(|h| u8::from(h.borrow().imutaveis.contains(&handle)))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_filled(len: i64, fill_bits: i64, fill_tag: u8) -> i64 {
    let count = usize::try_from(len.max(0)).expect("comprimento inválido");
    let val = tagged(fill_bits, fill_tag);
    let items = vec![val; count];
    HEAP.with(|heap| heap.borrow_mut().create_list(items))
}

// Funções de Lista com checagem de erros

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_single(handle: i64) -> i64 {
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if handle == 0 { return 0; }
        if let Value::List(items) = heap_ref.get(handle) {
            if items.is_empty() {
                drop(heap_ref);
                let err = allocate_state_error("No element");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            if items.len() > 1 {
                drop(heap_ref);
                let err = allocate_state_error("Too many elements");
                dartforge_exception_throw(err, 3);
                return 0;
            }
            items[0].bits
        } else {
            0
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_sublist(handle: i64, start: i64, end: i64) -> i64 {
    HEAP.with(|heap| {
        let len = {
            let h = heap.borrow();
            match h.get(handle) {
                Value::List(items) => items.len() as i64,
                _ => 0,
            }
        };
        let end_idx = if end < 0 { len } else { end };
        if start < 0 || start > len || end_idx < start || end_idx > len {
            let err = allocate_range_error("Index out of range");
            dartforge_exception_throw(err, 3);
            return 0;
        }
        let h = heap.borrow();
        let Value::List(items) = h.get(handle) else { return 0; };
        let slice: Vec<TaggedValue> = items[start as usize..end_idx as usize].to_vec();
        drop(h);
        heap.borrow_mut().allocate(Value::List(slice))
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_list_remove_at(handle: i64, index: i64) -> i64 {
    HEAP.with(|heap| {
        let len = {
            let h = heap.borrow();
            match h.get(handle) {
                Value::List(items) => items.len() as i64,
                _ => 0,
            }
        };
        if index < 0 || index >= len {
            let err = allocate_range_error("Index out of range");
            dartforge_exception_throw(err, 3);
            return 0;
        }
        let mut h = heap.borrow_mut();
        let Value::List(items) = h.get_mut(handle) else { return 0; };
        items.remove(index as usize).bits
    })
}
