// Runtime nativo: natives de `String` e `StringBuffer` do SDK da fonte (P5b;
// espaço unificado, P1).
//
// Os natives de `runtime/lib/string.cc`, os `Internal_*` de string
// (`internal_patch.dart`) e os `DartForge_string_*`/`DartForge_sb_*` da
// sobreposição (`native_sdk/core/string_patch.dart`,
// `string_buffer_patch.dart`) sobre as strings do espaço de objetos
// (`textos.rs`): leitura pela vista emprestada do bloco, criação numa alocação
// na forma canônica. O receptor é o `Ref` da string; `int` chega como `i64`.
// A tabela que o lowering consulta é `crates/emit_native/src/nativos.rs`; os
// mais quentes (`length`, `codeUnitAt`, `hashCode`, `allocate*`/`writeInto*`,
// `==`) o código gerado faz em linha (`lower/textos.rs`) e só cai aqui no
// caminho lento.

/// `String_getLength`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_getLength(this: i64) -> i64 {
    with_string(this, |t| t.len() as i64)
}

/// `_StringBase.codeUnitAt(i)` (intrínseco da VM), com a conferência de
/// índice: fora da faixa, `RangeError.range(i, 0, length - 1, "length")`. O
/// caminho lento de `@df.texto_unidade` (`lower/textos.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_codeUnitAt(this: i64, ix: i64) -> i64 {
    let r = with_string(this, |t| {
        usize::try_from(ix).ok().filter(|&i| i < t.len()).map(|i| i64::from(t.unit(i))).ok_or(t.len())
    });
    match r {
        Ok(u) => u,
        Err(n) => {
            throw_range_err(ix, 0, n as i64 - 1, "length");
            0
        }
    }
}

thread_local! {
    /// As strings de um caractere Latin-1 já devolvidas por `String_charAt`
    /// neste isolado: os literais canônicos (`Heap::string_literal`,
    /// permanentes), 0 = ainda não pedida.
    static ONE_CHAR: std::cell::RefCell<[i64; 256]> = const { std::cell::RefCell::new([0; 256]) };
}

/// `String_charAt` (`s[i]`): a string de uma unidade; fora dos limites,
/// `RangeError.range(i, 0, length - 1, "index")` (`StringValueAt`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_charAt(this: i64, ix: i64) -> i64 {
    let r = with_string(this, |t| usize::try_from(ix).ok().filter(|&i| i < t.len()).map(|i| t.unit(i)).ok_or(t.len()));
    let u = match r {
        Ok(u) => u,
        Err(n) => {
            throw_range_err(ix, 0, n as i64 - 1, "index");
            return 0;
        }
    };
    if u < 256 {
        // A unidade Latin-1 volta como a string canônica de um caractere, sem
        // alocar: a VM devolve o símbolo predefinido (`Symbols::FromCharCode`,
        // `String_charAt`), então `identical(s[0], 'a')` também vale aqui.
        let cache = ONE_CHAR.with(|c| c.borrow()[u as usize]);
        if cache != 0 {
            return cache;
        }
        let h = HEAP.with(|heap| heap.borrow_mut().string_literal(crate::textos::TextoRef::Um(&[u as u8])));
        ONE_CHAR.with(|c| c.borrow_mut()[u as usize] = h);
        return h;
    }
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Dois(&[u])))
}

/// `String_concat` (`+`): uma alocação, as unidades copiadas dos dois blocos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concat(this: i64, other_one: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().join_strings(&[this, other_one]))
}

/// `String_getHashCode`: o `StringHasher` da VM, calculado uma vez por string e
/// guardado no cabeçalho (`Heap::hash_de_texto`). O caminho lento de
/// `@df.texto_hash`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_getHashCode(this: i64) -> i64 {
    dartforge_texto_hash(this)
}

/// `String_toUpperCase` e `String_toLowerCase` (`String::Transform`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_toUpperCase(this: i64) -> i64 {
    transform(this, true)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_toLowerCase(this: i64) -> i64 {
    transform(this, false)
}

/// `StringBase_substringUnchecked` e `OneByteString_substringUnchecked`:
/// `[start, end)` sem conferência (o Dart já conferiu), na forma canônica.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_substringUnchecked(this: i64, start: i64, end: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_slice(this, start as usize, end as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_substringUnchecked(this: i64, start: i64, end: i64) -> i64 {
    dartforge_nativo_StringBase_substringUnchecked(this, start, end)
}

/// `Internal_allocateOneByteString`: `_OneByteString` de `n` unidades 0,
/// para ser preenchida por `writeIntoOneByteString` (o `dart:convert` e o
/// `_StringBase` montam strings assim). O código gerado faz em linha
/// (`@df.texto_alocar`); este é o caminho de quem chama pelo native.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_allocateOneByteString(n: i64) -> i64 {
    dartforge_texto_novo(n, 0)
}

/// `Internal_allocateTwoByteString`: `_TwoByteString` de `n` unidades 0.
/// Fica `Dois` mesmo que só receba unidades Latin-1, como na VM; a
/// igualdade é por unidades, então isso só custa espaço.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_allocateTwoByteString(n: i64) -> i64 {
    dartforge_texto_novo(n, 1)
}

/// `Internal_writeIntoOneByteString`: grava a unidade `code & 0xFF` no
/// índice (a string foi alocada agora; o Dart conferiu índice e faixa).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoOneByteString(string: i64, ix: i64, code: i64) {
    HEAP.with(|heap| heap.borrow_mut().one_units_mut(string)[ix as usize] = code as u8);
}

/// `Internal_writeIntoTwoByteString`: grava a unidade `code & 0xFFFF`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoTwoByteString(string: i64, ix: i64, code: i64) {
    HEAP.with(|heap| heap.borrow_mut().two_units_mut(string)[ix as usize] = code as u16);
}

// Os laços de `string_patch.dart` que a VM faz com `codeUnitAt` intrínseco: a
// comparação de trecho (`startsWith`, `endsWith`), a busca (`indexOf`,
// `lastIndexOf`, `contains`) e a divisão por um caractere (`split`). O
// resultado é o mesmo laço, feito sobre as vistas dos blocos.

/// `_StringBase._substringMatches(start, other)`: `other` vazio casa; fora
/// dos limites, não.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_regiao_igual(this: i64, start: i64, other_one: i64) -> u8 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("bug do compilador: string esperada");
        let o = heap.text(other_one).expect("bug do compilador: string esperada");
        if o.is_empty() {
            return 1;
        }
        let Ok(i) = usize::try_from(start) else { return 0 };
        u8::from(t.matches_at(o, i))
    })
}

/// `indexOf(String other, start)` depois da conferência de `start` (feita
/// no Dart): o primeiro índice `>= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_indice_de(this: i64, other_one: i64, start: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("bug do compilador: string esperada");
        let o = heap.text(other_one).expect("bug do compilador: string esperada");
        if o.len() > t.len() {
            return -1;
        }
        let start = usize::try_from(start).unwrap_or(0);
        t.search_for(o, start).map_or(-1, |i| i as i64)
    })
}

/// `lastIndexOf(String other, start)` depois da conferência de `start`: o
/// último índice `<= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_ultimo_indice_de(this: i64, other_one: i64, start: i64) -> i64 {
    if start < 0 {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.text(this).expect("bug do compilador: string esperada");
        let o = heap.text(other_one).expect("bug do compilador: string esperada");
        t.find_last(o, start as usize).map_or(-1, |i| i as i64)
    })
}

/// `_OneByteString._splitWithCharCode(charCode)`: acrescenta a `list` (a
/// `<String>[]` nova do Dart) os pedaços entre as ocorrências da unidade
/// `code`, na ordem — o mesmo laço, com cada pedaço alocado aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_dividir_por_codigo(this: i64, list: i64, code: i64) {
    // As fronteiras primeiro (sem alocar); depois cada pedaço copiado do
    // bloco. `this` e `list` têm raiz no chamador; cada pedaço, assim que
    // alocado, entra na lista (a coleta de uma alocação seguinte o vê).
    let ranges: Vec<(usize, usize)> = with_string(this, |t| {
        let mut v = Vec::new();
        let mut start = 0;
        for (i, u) in t.unidades().enumerate() {
            if i64::from(u) == code {
                v.push((start, i));
                start = i + 1;
            }
        }
        v.push((start, t.len()));
        v
    });
    for (a, b) in ranges {
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let h = heap.string_slice(this, a, b);
            heap.list_push(list, crate::heap::Value::Ref(h));
        });
    }
}

/// `_StringBase._iguais(other_one)` da sobreposição (`string_patch.dart`): as
/// mesmas unidades UTF-16 (o laço do `==` da VM, feito aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_iguais(this: i64, other_one: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().strings_equal(this, other_one)))
}

/// `_StringBase.==` inteiro (o intrínseco `String_equality` da VM):
/// `identical`, o teste `other is String` e as unidades. O que não é objeto
/// (`null`, `Smi`) é diferente sem consultar o heap. O caminho lento de
/// `@df.texto_igual_a` (`lower/textos.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_igual_a(this: i64, other_one: i64) -> u8 {
    if this == other_one {
        return 1;
    }
    if !crate::layout::is_object(other_one) {
        return 0;
    }
    HEAP.with(|heap| u8::from(heap.borrow().strings_equal(this, other_one)))
}

/// `printToConsole(line)` da sobreposição (`DartForge_imprimir`): a linha e
/// o fim de linha, como o `print` da VM (o substituto solto vira U+FFFD).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_imprimir(line: i64) {
    use std::io::Write;
    let mut bytes = if line == 0 {
        b"null".to_vec()
    } else {
        with_string(line, |t| t.to_vm_utf8())
    };
    bytes.push(b'\n');
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(&bytes);
}

/// As strings `Ref` (não nulas) de `list[start..end)`, na ordem.
fn concat_range_parts(list: i64, start: i64, end: i64) -> Vec<i64> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.list_len(list) as i64;
        (start.max(0)..end.min(n))
            .filter_map(|i| match heap.list_get(list, i as usize) {
                crate::heap::Value::Ref(r) if r != 0 => Some(r),
                _ => None,
            })
            .collect()
    })
}

/// `_StringBase._concatRangeNative(strings, start, end)`: a concatenação
/// de `strings[start..end]` (`String_concatRange`), numa alocação.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concatRange(list: i64, start: i64, end: i64) -> i64 {
    // As strings estão vivas pela lista (enraizada pelo chamador).
    let part_list = concat_range_parts(list, start, end);
    HEAP.with(|heap| heap.borrow_mut().join_strings(&part_list))
}

/// Os elementos `[start, end)` de uma lista de inteiros (códigos): lista do
/// núcleo ou lista tipada (o `Uint16List` do `String.fromCharCodes`), como os
/// natives da VM leem `TypedData`. Elemento que não é `int` vira -1.
fn string_list_codes(list: i64, start: i64, end: i64) -> Vec<i64> {
    use crate::tipadas::tipo;
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.tipada(list) {
            // SAFETY: a lista vive (o chamador a tem) e nada aloca aqui.
            let b = unsafe { t.piece() };
            let s = crate::tipadas::tamanho_do_elemento(t.tipo);
            let (i, f) = (start.max(0) as usize, (end.max(0) as usize).min(t.len));
            return (i..f.max(i))
                .map(|k| {
                    let o = k * s;
                    match t.tipo {
                        tipo::INT8 => i64::from(b[o] as i8),
                        tipo::INT16 => i64::from(i16::from_ne_bytes([b[o], b[o + 1]])),
                        tipo::UINT16 => i64::from(u16::from_ne_bytes([b[o], b[o + 1]])),
                        tipo::INT32 => i64::from(i32::from_ne_bytes(b[o..o + 4].try_into().unwrap())),
                        tipo::UINT32 => i64::from(u32::from_ne_bytes(b[o..o + 4].try_into().unwrap())),
                        tipo::INT64 | tipo::UINT64 => i64::from_ne_bytes(b[o..o + 8].try_into().unwrap()),
                        _ => i64::from(b[o]),
                    }
                })
                .collect();
        }
        let n = heap.list_len(list) as i64;
        (start.max(0)..end.min(n)).map(|i| value_int(&heap, heap.list_get(list, i as usize)).unwrap_or(-1)).collect()
    })
}

/// `_OneByteString._allocateFromOneByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_allocateFromOneByteList(list: i64, start: i64, end: i64) -> i64 {
    let u: Vec<u8> = string_list_codes(list, start, end).into_iter().map(|c| (c & 0xFF) as u8).collect();
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Um(&u)))
}

/// `_TwoByteString._allocateFromTwoByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TwoByteString_allocateFromTwoByteList(list: i64, start: i64, end: i64) -> i64 {
    let u: Vec<u16> = string_list_codes(list, start, end).into_iter().map(|c| (c & 0xFFFF) as u16).collect();
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Dois(&u)))
}

/// `_StringBase._createFromCodePoints(list, start, end)`: pontos de código
/// (acima de U+FFFF viram o par substituto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_createFromCodePoints(list: i64, start: i64, end: i64) -> i64 {
    let mut u = Vec::new();
    for c in string_list_codes(list, start, end) {
        // Fora de `0..=0x10FFFF`: `ArgumentError` sem mensagem, como o
        // `StringBase_createFromCodePoints` da VM.
        if !(0..=0x10FFFF).contains(&c) {
            let e = dartforge_argument_error_new(0, 0);
            dartforge_exception_throw(e, 3);
            return 0;
        }
        crate::textos::push_point(&mut u, c as u32);
    }
    HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Dois(&u)))
}

/// `_StringBase._joinReplaceAllResult(base, matches, length, oneByte)`: as
/// fatias de `base` (um `Smi` negativo `-(início << 11 | tamanho)`, ou o par
/// início, fim) e as substituições, na ordem (`string_patch.dart`). Uma
/// medição, uma alocação e as cópias dos blocos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_joinReplaceAllResult(base: i64, part_list: i64, _size: i64, _one_byte: u8) -> i64 {
    // (fonte, início, fim) de cada pedaço; as fontes vivem pela lista e por
    // `base` (enraizadas pelo chamador).
    let chunks: Vec<(i64, usize, usize)> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.list_len(part_list);
        let mut v = Vec::new();
        let mut i = 0;
        while i < n {
            let x = heap.list_get(part_list, i);
            match x {
                crate::heap::Value::Ref(r) if crate::layout::is_object(r) && heap.is_string(r) => {
                    let k = heap.text(r).map_or(0, crate::textos::TextoRef::len);
                    v.push((r, 0, k));
                }
                _ => {
                    let a = value_int(&heap, x).unwrap_or(0);
                    let (begin, end) = if a < 0 {
                        let bits = -a;
                        let begin = bits >> 11;
                        (begin, begin + (bits & ((1 << 11) - 1)))
                    } else {
                        i += 1;
                        (a, value_int(&heap, heap.list_get(part_list, i)).unwrap_or(a))
                    };
                    v.push((base, begin as usize, end as usize));
                }
            }
            i += 1;
        }
        v
    });
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (mut total, mut one) = (0, true);
        for &(f, a, b) in &chunks {
            let t = heap.text(f).expect("bug do compilador: string esperada").piece(a, b);
            total += t.len();
            one &= t.fits_in_one_byte();
        }
        let r = heap.new_string(total, !one);
        let mut pos = 0;
        for &(f, a, b) in &chunks {
            heap.copy_string(r, pos, f, a, b);
            pos += b - a;
        }
        r
    })
}

// ─── O acumulador do `StringBuffer` (`_AcumuladorDeTexto`, cid 15) ──────────
//
// Um bloco `BRUTO`+`ATTACHMENT` de uma palavra: o ponteiro de um `Box<Vec<u16>>`
// com as unidades escritas, que o coletor solta (`release_accumulator`) quando o
// bloco morre (§2.8). Só o `StringBuffer` o vê
// (`native_sdk/core/string_buffer_patch.dart`).

/// Solta o `Vec<u16>` de um acumulador morto (chamado pelo coletor).
///
/// # Safety
/// `p` saiu de `Box::into_raw` de um `Box<Vec<u16>>` em
/// [`dartforge_nativo_DartForge_sb_novo`] e é solto uma vez só.
unsafe fn release_accumulator(p: *mut u8) {
    // SAFETY: contrato acima.
    drop(unsafe { Box::from_raw(p.cast::<Vec<u16>>()) });
}

/// Um acumulador novo com as unidades `unidades` (o `StringBuffer` vazio, ou a
/// cópia de um acumulador que chega por mensagem de outro isolado, §2.12).
fn new_accumulator_with(unidades: Vec<u16>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.allocate(
            crate::layout::cid::STRING_ACCUMULATOR,
            1,
            crate::layout::flags::BRUTO | crate::layout::flags::ATTACHMENT,
        );
        let bytes = std::mem::size_of::<Vec<u16>>() + 2 * unidades.capacity();
        let p = Box::into_raw(Box::new(unidades)).cast::<u8>();
        // SAFETY: `release_accumulator` é a única liberação de `p`.
        unsafe { heap.attach(h, release_accumulator, p, bytes) };
        h
    })
}

/// O `Vec<u16>` do acumulador `h`.
///
/// # Panics
/// Se `h` não é um acumulador (bug do compilador).
fn accumulator_vec(heap: &Heap, h: i64) -> *mut Vec<u16> {
    assert!(
        crate::layout::is_object(h) && heap.class(h) == crate::layout::cid::STRING_ACCUMULATOR,
        "bug do compilador: acumulador de StringBuffer esperado\nhandle {h}"
    );
    heap.attachment(h).cast::<Vec<u16>>()
}

/// Acrescenta as unidades de `t` (que não vem de um bloco que possa morrer
/// aqui) ao acumulador `accumulator`.
fn append_to_accumulator(accumulator: i64, t: crate::textos::TextoRef<'_>) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: o `Vec` do anexo mora fora do heap; nada mais o empresta.
        let v = unsafe { &mut *v };
        match t {
            crate::textos::TextoRef::Um(b) => v.extend(b.iter().map(|&x| u16::from(x))),
            crate::textos::TextoRef::Dois(u) => v.extend_from_slice(u),
        }
    });
}

/// `StringBuffer._novo()`: um acumulador vazio.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_novo() -> i64 {
    new_accumulator_with(Vec::new())
}

/// Acrescenta as unidades da string `text` ao acumulador.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever(accumulator: i64, text: i64) {
    // O retorno é -1 só quando `text` não é string, o que o Dart não deixa.
    let n = dartforge_nativo_DartForge_sb_escrever_se_texto(accumulator, text);
    assert!(n >= 0, "bug do compilador: StringBuffer._escrever sem String");
}

/// `StringBuffer._escreverSeTexto(accumulator, obj)` da sobreposição: se
/// `obj` é uma string, acrescenta as unidades dela ao acumulador e devolve
/// quantas; senão -1, sem mudar nada (o Dart segue pelo `toString`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever_se_texto(accumulator: i64, obj: i64) -> i64 {
    if !crate::layout::is_object(obj) {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // As unidades lidas direto do bloco da string (viva: é o argumento); o
        // acumulador mora fora do heap, então crescer não as move.
        let Some(t) = heap.text(obj) else { return -1 };
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: o `Vec` do anexo mora fora do heap; nada mais o empresta.
        let v = unsafe { &mut *v };
        match t {
            crate::textos::TextoRef::Um(b) => v.extend(b.iter().map(|&x| u16::from(x))),
            crate::textos::TextoRef::Dois(u) => v.extend_from_slice(u),
        }
        t.len() as i64
    })
}

/// Acrescenta o ponto de código `code` ao acumulador (`writeCharCode`
/// da sobreposição, que já conferiu `0 <= codigo <= 0x10FFFF`): uma
/// unidade, ou o par de surrogates acima de 0xFFFF, como a VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever_codigo(accumulator: i64, code: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // SAFETY: o `Vec` do anexo mora fora do heap; nada mais o empresta.
        let v = unsafe { &mut *accumulator_vec(&heap, accumulator) };
        crate::textos::push_point(v, code as u32);
    });
}

/// A string com o conteúdo do acumulador (na forma canônica); o acumulador
/// continua podendo crescer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_texto(accumulator: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = accumulator_vec(&heap, accumulator);
        // SAFETY: o `Vec` mora fora do heap e só o solta a morte do
        // acumulador, que está enraizado pelo chamador durante a alocação.
        let u: &[u16] = unsafe { &*v };
        heap.allocate_string(crate::textos::TextoRef::Dois(u))
    })
}

/// `_StringBase._deCodigos(list, start, end)` (DartForge, o
/// `createFromCharCodes` de `string_patch.dart`): as unidades `[inicio,
/// fim)` de uma list do núcleo (`_List`, `_GrowableList`, `_ImmutableList`),
/// todas `int` em `0..=0xFF`, viram a `_OneByteString` numa cópia só — o
/// `_scanCodeUnits` e o `_setAt` por unidade da VM, que aqui eram uma chamada
/// ao runtime cada. Qualquer outra coisa (unidade acima de 0xFF, negativa ou
/// não `int`, faixa inválida, lista tipada): null, e o Dart segue pelo caminho
/// da VM, com os erros dela.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_de_codigos(list: i64, start: i64, end: i64) -> i64 {
    let bytes = HEAP.with(|heap| {
        let heap = heap.borrow();
        if !crate::layout::is_object(list) || !heap.e_lista(list) {
            return None;
        }
        let (i, f) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
        if i > f || f > heap.list_len(list) {
            return None;
        }
        let e = heap.list_elements(list);
        let mut v = Vec::with_capacity(f - i);
        for k in i..f {
            let n = match e.get(k) {
                crate::heap::Value::Int(x) => x,
                crate::heap::Value::Ref(r) if crate::layout::smi::is_smi(r) => crate::layout::smi::value(r),
                _ => return None,
            };
            v.push(u8::try_from(n).ok()?);
        }
        Some(v)
    });
    bytes.map_or(0, |b| HEAP.with(|heap| heap.borrow_mut().allocate_string(crate::textos::TextoRef::Um(&b))))
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ──────────────────

/// Uma string nova de `len` unidades zeradas (`dois` ≠ 0: `_TwoByteString`): o
/// caminho lento de `@df.texto_alocar` (acima de 16 palavras, ou TLAB
/// esgotada). Não lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_texto_novo(len: i64, dois: i64) -> i64 {
    let len = usize::try_from(len).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().new_string(len, dois != 0))
}

/// [`dartforge_texto_novo`] sob ARC: a string nova (unidades zeradas, ainda
/// não publicada) com um owner do código, entregue antes de outra coleta.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let s = dartforge_arc_texto_novo_owned_v1(3, 0);
/// assert_eq!(dartforge_arc_observar_heap_v1(s), 3);
/// dartforge_arc_release(s);
/// ```
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_arc_texto_novo_owned_v1(len: i64, dois: i64) -> i64 {
    let len = usize::try_from(len).unwrap_or(0);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let s = heap.new_string(len, dois != 0);
        heap.retain_code_owner(s);
        s
    })
}

/// O hash da string `s`, calculado e gravado no cabeçalho (`map`): o caminho
/// lento de `@df.texto_hash`. Não aloca nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_texto_hash(s: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.hash_de_texto(s) {
            Some(x) => i64::from(x),
            None => panic!("bug do compilador: hashCode de string sobre valor que não é string\nhandle {s}"),
        }
    })
}

/// As duas strings têm as mesmas unidades? O caminho lento de `@df.texto_igual`
/// (os dois são strings; a identidade, o comprimento e os hashes já foram
/// conferidos em linha). Não aloca nem lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_texto_iguais(a: i64, b: i64) -> i8 {
    HEAP.with(|heap| i8::from(heap.borrow().strings_equal(a, b)))
}
