// Runtime nativo: membros de `String`, `StringBuffer`, `RegExp` e `parse`.

/// Copia UTF-8 de uma constante LLVM para uma string gerenciada.
/// SAFETY: ptr deve apontar para len bytes legíveis; o emissor garante essa região.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_new(ptr: *const u8, len: i64) -> i64 {
    let len = usize::try_from(len).expect("comprimento inválido");
    let bytes = if len == 0 {
        &[]
    } else {
        // SAFETY: única leitura de ponteiro estrangeiro; contrato da constante LLVM.
        unsafe { std::slice::from_raw_parts(ptr, len) }
    };
    let text = std::str::from_utf8(bytes)
        .expect("UTF-8 inválido")
        .to_owned();
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}
/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_concat(a, b))
}
/// Compara conteúdo UTF-8; dois handles null são iguais.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_equal(a: i64, b: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().string_equal(a, b)))
}

/// Comprimento em UTF-16 code units de uma string gerenciada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_len(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().count() as i64,
            Value::RawString(r) => r.len() as i64,
            Value::StringBuffer(b) => b.encode_utf16().count() as i64,
            _ => 0,
        }
    })
}

/// Retorna o code unit UTF-16 no índice especificado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_unit_at(handle: i64, index: i64) -> i64 {
    if handle == 0 || index < 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().nth(index as usize).unwrap_or(0) as i64,
            Value::RawString(r) => r.get(index as usize).copied().unwrap_or(0) as i64,
            Value::StringBuffer(b) => b.encode_utf16().nth(index as usize).unwrap_or(0) as i64,
            _ => 0,
        }
    })
}

/// Retorna uma lista de code units UTF-16 como inteiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_units(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let code_units: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().map(|u| TaggedValue { bits: u as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            Value::RawString(r) => r.iter().map(|&u| TaggedValue { bits: u as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            Value::StringBuffer(b) => b.encode_utf16().map(|u| TaggedValue { bits: u as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            _ => Vec::new(),
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(code_units)))
}

/// Retorna uma lista de runes (Unicode scalar values / code points) como inteiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_runes(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let runes: Vec<TaggedValue> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.chars().map(|c| TaggedValue { bits: c as u32 as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            Value::RawString(r) => char::decode_utf16(r.iter().copied()).map(|res| TaggedValue { bits: res.unwrap_or('\u{FFFD}') as u32 as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            Value::StringBuffer(b) => b.chars().map(|c| TaggedValue { bits: c as u32 as i64, tag: crate::heap::ValueTag::Int, is_ref: false }).collect(),
            _ => Vec::new(),
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(runes)))
}

/// Converte um inteiro para string na base indicada (ex.: base 16 para hex minúsculo).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_to_radix_string(value: i64, radix: i64) -> i64 {
    let radix = radix.clamp(2, 36) as u32;
    let s = if value == 0 {
        "0".to_string()
    } else {
        let neg = value < 0;
        let mut uval = if neg { (-(value as i128)) as u64 } else { value as u64 };
        let mut digits = Vec::new();
        let uradix = radix as u64;
        while uval > 0 {
            let rem = (uval % uradix) as u32;
            let c = if rem < 10 { (b'0' + rem as u8) as char } else { (b'a' + (rem - 10) as u8) as char };
            digits.push(c);
            uval /= uradix;
        }
        if neg { digits.push('-'); }
        digits.into_iter().rev().collect()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Retorna substring usando índices UTF-16.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_substring(handle: i64, start: i64, end: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let units: Vec<u16> = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => Vec::new(),
        }
    });
    let len = units.len() as i64;
    let actual_end = if end < 0 { len } else { end };
    if start < 0 || start > len || actual_end < start || actual_end > len {
        let err = dartforge_range_error_value(start, 0, 0);
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let start_idx = start as usize;
    let end_idx = actual_end as usize;
    let slice = &units[start_idx..end_idx];
    match String::from_utf16(slice) {
        Ok(s) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s))),
        Err(_) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(slice.to_vec()))),
    }
}

/// Cria string a partir de um code point ou code unit UTF-16.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_code(code: i64) -> i64 {
    if (0xD800..=0xDFFF).contains(&code) {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(vec![code as u16])));
    }
    let s = if let Some(c) = char::from_u32(code as u32) {
        c.to_string()
    } else {
        String::new()
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s)))
}

/// Cria string a partir de lista/iterável de char codes.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_codes(list_handle: i64) -> i64 {
    if list_handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let units: Vec<u16> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::List(items) = heap.get(list_handle) else { return Vec::new(); };
        let mut out = Vec::new();
        for item in items {
            let code = item.bits;
            if code <= 0xFFFF {
                out.push(code as u16);
            } else if let Some(c) = char::from_u32(code as u32) {
                let mut buf = [0u16; 2];
                out.extend_from_slice(c.encode_utf16(&mut buf));
            }
        }
        out
    });
    match String::from_utf16(&units) {
        Ok(s) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(s))),
        Err(_) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::RawString(units))),
    }
}

/// Localiza substring a partir do offset `start` (em UTF-16). Devolve -1 se não encontrar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    if handle == 0 || pat_handle == 0 { return -1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return -1,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return -1,
        };
        let start_pos = (start.max(0) as usize).min(target_units.len());
        if pat_units.is_empty() { return start_pos as i64; }
        for i in start_pos..=target_units.len().saturating_sub(pat_units.len()) {
            if target_units[i..i + pat_units.len()] == pat_units[..] {
                return i as i64;
            }
        }
        -1
    })
}

/// Localiza última ocorrência de substring a partir de `start`. Devolve -1 se não encontrar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_last_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    if handle == 0 || pat_handle == 0 { return -1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return -1,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return -1,
        };
        let max_pos = if start < 0 {
            target_units.len()
        } else {
            (start as usize).min(target_units.len())
        };
        if pat_units.is_empty() { return max_pos as i64; }
        if target_units.len() < pat_units.len() { return -1; }
        let search_start = max_pos.min(target_units.len() - pat_units.len());
        for i in (0..=search_start).rev() {
            if target_units[i..i + pat_units.len()] == pat_units[..] {
                return i as i64;
            }
        }
        -1
    })
}

/// Divide a string pelo separador (split).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split(handle: i64, pat_handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let res_items: Vec<Value> = HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        let pat_str = if pat_handle != 0 {
            if let Value::String(s) = heap_ref.get(pat_handle) { s.as_str() } else { "" }
        } else { "" };
        let mut pieces = Vec::new();
        match heap_ref.get(handle) {
            Value::String(s) => {
                if pat_str.is_empty() {
                    for u in s.encode_utf16() {
                        let sub = match String::from_utf16(&[u]) {
                            Ok(v) => Value::String(v),
                            Err(_) => Value::RawString(vec![u]),
                        };
                        pieces.push(sub);
                    }
                } else {
                    for part in s.split(pat_str) {
                        pieces.push(Value::String(part.to_string()));
                    }
                }
            }
            Value::RawString(r) => {
                for &u in r {
                    pieces.push(Value::RawString(vec![u]));
                }
            }
            _ => {}
        }
        drop(heap_ref);
        pieces
    });
    lista_de_pedacos(res_items.into_iter().map(|v| (None, v)).collect())
}

/// Lista com os valores dados, na ordem, alocando cada um já com a lista
/// enraizada (G6): antes os pedaços iam para um `Vec` do Rust e a lista era
/// alocada no fim — uma coleta no meio liberava os primeiros.
fn lista_de_pedacos(itens: Vec<(Option<TaggedValue>, Value)>) -> i64 {
    let lista = HEAP.with(|h| h.borrow_mut().create_list(Vec::new()));
    com_raizes(&[lista], || {
        for (antes, val) in itens {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                if let Some(a) = antes {
                    h.list_push(lista, a);
                }
                let x = h.allocate(val);
                h.list_push(lista, TaggedValue::reference(x));
            });
        }
    });
    lista
}

/// Verifica se a string contém a substring a partir de `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_contains(handle: i64, pat_handle: i64, start: i64) -> u8 {
    (dartforge_string_index_of(handle, pat_handle, start) >= 0) as u8
}

fn match_pattern_at(text: &str, pat: &str) -> Option<usize> {
    if pat == r"\d+" || pat == "\\d+" {
        let mut len = 0;
        for c in text.chars() {
            if c.is_ascii_digit() {
                len += c.len_utf8();
            } else {
                break;
            }
        }
        if len > 0 { Some(len) } else { None }
    } else if pat.starts_with('[') && pat.ends_with(']') && pat.len() >= 2 {
        let set = &pat[1..pat.len() - 1];
        if let Some(first_char) = text.chars().next() {
            if set.contains(first_char) {
                Some(first_char.len_utf8())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        if text.starts_with(pat) {
            Some(pat.len())
        } else {
            None
        }
    }
}

fn find_next_pattern(text: &str, start_byte: usize, pat: &str) -> Option<(usize, usize)> {
    let mut byte_idx = start_byte;
    while byte_idx < text.len() {
        if let Some(len) = match_pattern_at(&text[byte_idx..], pat) {
            return Some((byte_idx, len));
        }
        let ch = text[byte_idx..].chars().next().unwrap();
        byte_idx += ch.len_utf8();
    }
    None
}

/// Cria um novo RegExp.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_regexp_new(pat_handle: i64) -> i64 {
    let pat = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pat_handle != 0 {
            if let Value::String(s) = heap.get(pat_handle) { s.clone() } else { String::new() }
        } else {
            String::new()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::RegExp(pat)))
}

/// Divide a string em pedaços para splitMapJoin: retorna lista de [is_match (bool), part (Match ou String)].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split_map_pieces(target_handle: i64, pat_handle: i64) -> i64 {
    if target_handle == 0 {
        return HEAP.with(|h| h.borrow_mut().allocate(Value::List(Vec::new())));
    }
    let (target_str, pat_str) = HEAP.with(|heap| {
        let heap = heap.borrow();
        let target = match heap.get(target_handle) {
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        let pat = match heap.get(pat_handle) {
            Value::String(s) => s.clone(),
            Value::RegExp(p) => p.clone(),
            _ => String::new(),
        };
        (target, pat)
    });

    let mut pieces = Vec::new();
    let mut curr_byte = 0;
    while curr_byte <= target_str.len() {
        if let Some((match_start, match_len)) = find_next_pattern(&target_str, curr_byte, &pat_str) {
            let non_match = &target_str[curr_byte..match_start];
            pieces.push((false, non_match.to_string(), false));

            let matched = &target_str[match_start..match_start + match_len];
            pieces.push((true, matched.to_string(), true));

            if match_len > 0 {
                curr_byte = match_start + match_len;
            } else if let Some(ch) = target_str[match_start..].chars().next() {
                curr_byte = match_start + ch.len_utf8();
            } else {
                break;
            }
        } else {
            let non_match = &target_str[curr_byte..];
            pieces.push((false, non_match.to_string(), false));
            break;
        }
    }

    lista_de_pedacos(
        pieces
            .into_iter()
            .map(|(is_match, text, is_match_obj)| {
                let val = if is_match_obj { Value::Match(text) } else { Value::String(text) };
                (Some(TaggedValue::boolean(is_match)), val)
            })
            .collect(),
    )
}

/// Substitui todas as ocorrências de `from` por `to`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_all(handle: i64, from_handle: i64, to_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let res_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        let s = match heap.get(handle) {
            Value::String(s) => s.as_str(),
            _ => return String::new(),
        };
        let to = if to_handle != 0 {
            if let Value::String(t) = heap.get(to_handle) { t.as_str() } else { "" }
        } else { "" };

        if from_handle != 0 {
            match heap.get(from_handle) {
                Value::String(from) => s.replace(from, to),
                Value::RegExp(pat) => {
                    let mut out = String::new();
                    let mut curr = 0;
                    while curr < s.len() {
                        if let Some((start, len)) = find_next_pattern(s, curr, pat) {
                            out.push_str(&s[curr..start]);
                            out.push_str(to);
                            if len > 0 {
                                curr = start + len;
                            } else if let Some(ch) = s[start..].chars().next() {
                                out.push(ch);
                                curr = start + ch.len_utf8();
                            } else {
                                break;
                            }
                        } else {
                            out.push_str(&s[curr..]);
                            break;
                        }
                    }
                    out
                }
                _ => s.to_string(),
            }
        } else {
            s.to_string()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res_str)))
}

/// Preenche à esquerda até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_left(handle: i64, width: i64, pad_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let pad_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pad_handle != 0 {
            if let Value::String(p) = heap.get(pad_handle) { p.clone() } else { " ".to_string() }
        } else { " ".to_string() }
    });
    let s_len = dartforge_generic_len(handle);
    if width <= s_len { return handle; }
    let delta = (width - s_len) as usize;
    if pad_str.is_empty() { return handle; }
    let pad_repeated = pad_str.repeat(delta);
    let pad_h = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(pad_repeated)));
    HEAP.with(|heap| heap.borrow_mut().string_concat(pad_h, handle))
}

/// Preenche à direita até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_right(handle: i64, width: i64, pad_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let pad_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        if pad_handle != 0 {
            if let Value::String(p) = heap.get(pad_handle) { p.clone() } else { " ".to_string() }
        } else { " ".to_string() }
    });
    let s_len = dartforge_generic_len(handle);
    if width <= s_len { return handle; }
    let delta = (width - s_len) as usize;
    if pad_str.is_empty() { return handle; }
    let pad_repeated = pad_str.repeat(delta);
    let pad_h = HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(pad_repeated)));
    HEAP.with(|heap| heap.borrow_mut().string_concat(handle, pad_h))
}

/// Aloca um novo StringBuffer vazio.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_new() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::StringBuffer(String::new())))
}

/// Escreve no StringBuffer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_write(buf_handle: i64, str_handle: i64) {
    if buf_handle == 0 || str_handle == 0 { return; }
    HEAP.with(|heap| {
        let mut heap_ref = heap.borrow_mut();
        let text_to_append = match heap_ref.get(str_handle) {
            Value::String(s) => s.clone(),
            Value::RawString(r) => String::from_utf16_lossy(r),
            _ => describe_handle(&heap_ref, str_handle),
        };
        if let Value::StringBuffer(buf) = heap_ref.get_mut(buf_handle) {
            buf.push_str(&text_to_append);
        }
    });
}



/// Retorna uma nova string convertida para maiúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_upper(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.to_uppercase()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

/// Repete uma string `times` vezes (`'a' * 3`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    if handle == 0 || times <= 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(String::new())));
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.repeat(times as usize)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

fn is_dart_whitespace(c: char) -> bool {
    matches!(c,
        '\t' | '\n' | '\x0B' | '\x0C' | '\r' | ' ' |
        '\u{0085}' | '\u{00A0}' | '\u{1680}' |
        '\u{2000}'..='\u{200A}' |
        '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}'
    )
}

/// Remove espaços em branco do início e fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Remove espaços em branco do início.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_left(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_start_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Remove espaços em branco do fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_right(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let trimmed = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.trim_end_matches(is_dart_whitespace).to_string()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(trimmed)))
}

/// Verifica se a string começa com o prefixo dado a partir do índice `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_starts_with(handle: i64, pat_handle: i64, start: i64) -> u8 {
    if handle == 0 || pat_handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return 0,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        if start < 0 { return 0; }
        let start_pos = start as usize;
        if start_pos > target_units.len() { return 0; }
        if start_pos + pat_units.len() > target_units.len() { return 0; }
        (target_units[start_pos..start_pos + pat_units.len()] == pat_units[..]) as u8
    })
}

/// Verifica se a string termina com o sufixo dado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_ends_with(handle: i64, pat_handle: i64) -> u8 {
    if handle == 0 || pat_handle == 0 { return 0; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            Value::StringBuffer(b) => b.encode_utf16().collect(),
            _ => return 0,
        };
        let pat_units: Vec<u16> = match heap.get(pat_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        if pat_units.len() > target_units.len() { return 0; }
        target_units.ends_with(&pat_units) as u8
    })
}

/// Retorna uma nova string convertida para minúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_lower(handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        let Value::String(s) = heap.get(handle) else { return String::new(); };
        s.to_lowercase()
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(text)))
}

/// Compara duas strings lexicograficamente segundo unidades UTF-16 (-1, 0, 1).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_compare_to(handle: i64, other_handle: i64) -> i64 {
    if handle == 0 && other_handle == 0 { return 0; }
    if handle == 0 { return -1; }
    if other_handle == 0 { return 1; }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        let other_units: Vec<u16> = match heap.get(other_handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return 0,
        };
        match target_units.cmp(&other_units) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    })
}

/// Substitui a primeira ocorrência de `from` por `to` a partir de `start`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_first(handle: i64, from_handle: i64, to_handle: i64, start: i64) -> i64 {
    if handle == 0 { return 0; }
    let res_str = HEAP.with(|heap| {
        let heap = heap.borrow();
        let s = match heap.get(handle) {
            Value::String(s) => s.as_str(),
            _ => return String::new(),
        };
        let from = if from_handle != 0 {
            if let Value::String(f) = heap.get(from_handle) { f.as_str() } else { "" }
        } else { "" };
        let to = if to_handle != 0 {
            if let Value::String(t) = heap.get(to_handle) { t.as_str() } else { "" }
        } else { "" };
        if from.is_empty() {
            let start_idx = (start.max(0) as usize).min(s.len());
            return format!("{}{}{}", &s[..start_idx], to, &s[start_idx..]);
        }
        let search_start = (start.max(0) as usize).min(s.len());
        if let Some(pos) = s[search_start..].find(from) {
            let actual_pos = search_start + pos;
            format!("{}{}{}", &s[..actual_pos], to, &s[actual_pos + from.len()..])
        } else {
            s.to_string()
        }
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res_str)))
}

/// Substitui a faixa [start, end) por `replacement`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_range(handle: i64, start: i64, end: i64, rep_handle: i64) -> i64 {
    if handle == 0 { return 0; }
    let res = HEAP.with(|heap| {
        let heap = heap.borrow();
        let target_units: Vec<u16> = match heap.get(handle) {
            Value::String(s) => s.encode_utf16().collect(),
            Value::RawString(r) => r.clone(),
            _ => return String::new(),
        };
        let rep_units: Vec<u16> = if rep_handle != 0 {
            match heap.get(rep_handle) {
                Value::String(s) => s.encode_utf16().collect(),
                Value::RawString(r) => r.clone(),
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        let start_pos = (start.max(0) as usize).min(target_units.len());
        let end_pos = (end.max(0) as usize).min(target_units.len());
        let mut out = Vec::new();
        out.extend_from_slice(&target_units[..start_pos]);
        out.extend_from_slice(&rep_units);
        if end_pos < target_units.len() {
            out.extend_from_slice(&target_units[end_pos..]);
        }
        String::from_utf16_lossy(&out)
    });
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(res)))
}

/// Converte string para inteiro ou lança FormatException.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_parse(handle: i64) -> i64 {
    if handle == 0 {
        let err = allocate_format_exception("Invalid number: null");
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<i64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid radix-10 number: {text}"));
            dartforge_exception_throw(err, 3);
            0
        }
    }
}

/// Tenta converter string para inteiro; se falhar, devolve 0 (null); se passar, aloca _BoxedInt (1013).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_try_parse(handle: i64) -> i64 {
    if handle == 0 {
        return 0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<i64>() {
        Ok(val) => HEAP.with(|heap| heap.borrow_mut().allocate(Value::BoxedInt(val))),
        Err(_) => 0,
    }
}

/// Converte string para ponto flutuante ou lança FormatException.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_double_parse(handle: i64) -> f64 {
    if handle == 0 {
        let err = allocate_format_exception("Invalid double: null");
        dartforge_exception_throw(err, 3);
        return 0.0;
    }
    let text = HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.get(handle) {
            Value::String(s) => s.trim().to_string(),
            _ => String::new(),
        }
    });
    match text.parse::<f64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid double: {text}"));
            dartforge_exception_throw(err, 3);
            0.0
        }
    }
}

