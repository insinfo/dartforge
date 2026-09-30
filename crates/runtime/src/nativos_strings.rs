// Runtime nativo: natives de `String` e `StringBuffer` do SDK da fonte (P5b;
// espaço unificado, P1).
//
// Os natives de `runtime/lib/string.cc`, os `Internal_*` de string
// (`internal_patch.dart`) e os `DartForge_string_*`/`DartForge_sb_*` da
// sobreposição (`sdk_nativo/core/string_patch.dart`,
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
    com_texto(this, |t| t.len() as i64)
}

/// `_StringBase.codeUnitAt(i)` (intrínseco da VM), com a conferência de
/// índice: fora da faixa, `RangeError.range(i, 0, length - 1, "length")`. O
/// caminho lento de `@df.texto_unidade` (`lower/textos.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_codeUnitAt(this: i64, indice: i64) -> i64 {
    let r = com_texto(this, |t| {
        usize::try_from(indice).ok().filter(|&i| i < t.len()).map(|i| i64::from(t.unidade(i))).ok_or(t.len())
    });
    match r {
        Ok(u) => u,
        Err(n) => {
            lancar_range(indice, 0, n as i64 - 1, "length");
            0
        }
    }
}

thread_local! {
    /// As strings de um caractere Latin-1 já devolvidas por `String_charAt`
    /// neste isolado: os literais canônicos (`Heap::string_literal`,
    /// permanentes), 0 = ainda não pedida.
    static UM_CARACTERE: std::cell::RefCell<[i64; 256]> = const { std::cell::RefCell::new([0; 256]) };
}

/// `String_charAt` (`s[i]`): a string de uma unidade; fora dos limites,
/// `RangeError.range(i, 0, length - 1, "index")` (`StringValueAt`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_charAt(this: i64, indice: i64) -> i64 {
    let r = com_texto(this, |t| usize::try_from(indice).ok().filter(|&i| i < t.len()).map(|i| t.unidade(i)).ok_or(t.len()));
    let u = match r {
        Ok(u) => u,
        Err(n) => {
            lancar_range(indice, 0, n as i64 - 1, "index");
            return 0;
        }
    };
    if u < 256 {
        // A unidade Latin-1 volta como a string canônica de um caractere, sem
        // alocar: a VM devolve o símbolo predefinido (`Symbols::FromCharCode`,
        // `String_charAt`), então `identical(s[0], 'a')` também vale aqui.
        let cache = UM_CARACTERE.with(|c| c.borrow()[u as usize]);
        if cache != 0 {
            return cache;
        }
        let h = HEAP.with(|heap| heap.borrow_mut().string_literal(crate::textos::TextoRef::Um(&[u as u8])));
        UM_CARACTERE.with(|c| c.borrow_mut()[u as usize] = h);
        return h;
    }
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Dois(&[u])))
}

/// `String_concat` (`+`): uma alocação, as unidades copiadas dos dois blocos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concat(this: i64, outro: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().juntar_textos(&[this, outro]))
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
    transformar(this, true)
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_toLowerCase(this: i64) -> i64 {
    transformar(this, false)
}

/// `StringBase_substringUnchecked` e `OneByteString_substringUnchecked`:
/// `[start, end)` sem conferência (o Dart já conferiu), na forma canônica.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_substringUnchecked(this: i64, inicio: i64, fim: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().fatia_de_texto(this, inicio as usize, fim as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_substringUnchecked(this: i64, inicio: i64, fim: i64) -> i64 {
    dartforge_nativo_StringBase_substringUnchecked(this, inicio, fim)
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

/// `Internal_writeIntoOneByteString`: grava a unidade `codigo & 0xFF` no
/// índice (a string foi alocada agora; o Dart conferiu índice e faixa).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoOneByteString(string: i64, indice: i64, codigo: i64) {
    HEAP.with(|heap| heap.borrow_mut().unidades_um_mut(string)[indice as usize] = codigo as u8);
}

/// `Internal_writeIntoTwoByteString`: grava a unidade `codigo & 0xFFFF`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoTwoByteString(string: i64, indice: i64, codigo: i64) {
    HEAP.with(|heap| heap.borrow_mut().unidades_dois_mut(string)[indice as usize] = codigo as u16);
}

// Os laços de `string_patch.dart` que a VM faz com `codeUnitAt` intrínseco: a
// comparação de trecho (`startsWith`, `endsWith`), a busca (`indexOf`,
// `lastIndexOf`, `contains`) e a divisão por um caractere (`split`). O
// resultado é o mesmo laço, feito sobre as vistas dos blocos.

/// `_StringBase._substringMatches(start, other)`: `other` vazio casa; fora
/// dos limites, não.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_regiao_igual(this: i64, inicio: i64, outro: i64) -> u8 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.texto(this).expect("bug do compilador: string esperada");
        let o = heap.texto(outro).expect("bug do compilador: string esperada");
        if o.is_empty() {
            return 1;
        }
        let Ok(i) = usize::try_from(inicio) else { return 0 };
        u8::from(t.coincide_em(o, i))
    })
}

/// `indexOf(String other, start)` depois da conferência de `start` (feita
/// no Dart): o primeiro índice `>= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_indice_de(this: i64, outro: i64, inicio: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.texto(this).expect("bug do compilador: string esperada");
        let o = heap.texto(outro).expect("bug do compilador: string esperada");
        if o.len() > t.len() {
            return -1;
        }
        let inicio = usize::try_from(inicio).unwrap_or(0);
        t.procurar(o, inicio).map_or(-1, |i| i as i64)
    })
}

/// `lastIndexOf(String other, start)` depois da conferência de `start`: o
/// último índice `<= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_ultimo_indice_de(this: i64, outro: i64, inicio: i64) -> i64 {
    if inicio < 0 {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.texto(this).expect("bug do compilador: string esperada");
        let o = heap.texto(outro).expect("bug do compilador: string esperada");
        t.procurar_ultimo(o, inicio as usize).map_or(-1, |i| i as i64)
    })
}

/// `_OneByteString._splitWithCharCode(charCode)`: acrescenta a `lista` (a
/// `<String>[]` nova do Dart) os pedaços entre as ocorrências da unidade
/// `codigo`, na ordem — o mesmo laço, com cada pedaço alocado aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_dividir_por_codigo(this: i64, lista: i64, codigo: i64) {
    // As fronteiras primeiro (sem alocar); depois cada pedaço copiado do
    // bloco. `this` e `lista` têm raiz no chamador; cada pedaço, assim que
    // alocado, entra na lista (a coleta de uma alocação seguinte o vê).
    let faixas: Vec<(usize, usize)> = com_texto(this, |t| {
        let mut v = Vec::new();
        let mut inicio = 0;
        for (i, u) in t.unidades().enumerate() {
            if i64::from(u) == codigo {
                v.push((inicio, i));
                inicio = i + 1;
            }
        }
        v.push((inicio, t.len()));
        v
    });
    for (a, b) in faixas {
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let h = heap.fatia_de_texto(this, a, b);
            heap.lista_push(lista, crate::heap::Valor::Ref(h));
        });
    }
}

/// `_StringBase._iguais(outro)` da sobreposição (`string_patch.dart`): as
/// mesmas unidades UTF-16 (o laço do `==` da VM, feito aqui).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_iguais(this: i64, outro: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().textos_iguais(this, outro)))
}

/// `_StringBase.==` inteiro (o intrínseco `String_equality` da VM):
/// `identical`, o teste `other is String` e as unidades. O que não é objeto
/// (`null`, `Smi`) é diferente sem consultar o heap. O caminho lento de
/// `@df.texto_igual_a` (`lower/textos.rs`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_igual_a(this: i64, outro: i64) -> u8 {
    if this == outro {
        return 1;
    }
    if !crate::layout::e_objeto(outro) {
        return 0;
    }
    HEAP.with(|heap| u8::from(heap.borrow().textos_iguais(this, outro)))
}

/// `printToConsole(line)` da sobreposição (`DartForge_imprimir`): a linha e
/// o fim de linha, como o `print` da VM (o substituto solto vira U+FFFD).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_imprimir(linha: i64) {
    use std::io::Write;
    let mut bytes = if linha == 0 {
        b"null".to_vec()
    } else {
        com_texto(linha, |t| t.para_utf8_da_vm())
    };
    bytes.push(b'\n');
    let mut saida = std::io::stdout().lock();
    let _ = saida.write_all(&bytes);
}

/// As strings `Ref` (não nulas) de `lista[inicio..fim)`, na ordem.
fn partes_de_concat_range(lista: i64, inicio: i64, fim: i64) -> Vec<i64> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.lista_len(lista) as i64;
        (inicio.max(0)..fim.min(n))
            .filter_map(|i| match heap.lista_get(lista, i as usize) {
                crate::heap::Valor::Ref(r) if r != 0 => Some(r),
                _ => None,
            })
            .collect()
    })
}

/// `_StringBase._concatRangeNative(strings, start, end)`: a concatenação
/// de `strings[start..end]` (`String_concatRange`), numa alocação.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concatRange(lista: i64, inicio: i64, fim: i64) -> i64 {
    // As strings estão vivas pela lista (enraizada pelo chamador).
    let partes = partes_de_concat_range(lista, inicio, fim);
    HEAP.with(|heap| heap.borrow_mut().juntar_textos(&partes))
}

/// Os elementos `[inicio, fim)` de uma lista de inteiros (códigos): lista do
/// núcleo ou lista tipada (o `Uint16List` do `String.fromCharCodes`), como os
/// natives da VM leem `TypedData`. Elemento que não é `int` vira -1.
fn codigos_da_lista_de_texto(lista: i64, inicio: i64, fim: i64) -> Vec<i64> {
    use crate::tipadas::tipo;
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.tipada(lista) {
            // SAFETY: a lista vive (o chamador a tem) e nada aloca aqui.
            let b = unsafe { t.fatia() };
            let s = crate::tipadas::tamanho_do_elemento(t.tipo);
            let (i, f) = (inicio.max(0) as usize, (fim.max(0) as usize).min(t.len));
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
        let n = heap.lista_len(lista) as i64;
        (inicio.max(0)..fim.min(n)).map(|i| inteiro_do_valor(&heap, heap.lista_get(lista, i as usize)).unwrap_or(-1)).collect()
    })
}

/// `_OneByteString._allocateFromOneByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_allocateFromOneByteList(lista: i64, inicio: i64, fim: i64) -> i64 {
    let u: Vec<u8> = codigos_da_lista_de_texto(lista, inicio, fim).into_iter().map(|c| (c & 0xFF) as u8).collect();
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Um(&u)))
}

/// `_TwoByteString._allocateFromTwoByteList(list, start, end)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TwoByteString_allocateFromTwoByteList(lista: i64, inicio: i64, fim: i64) -> i64 {
    let u: Vec<u16> = codigos_da_lista_de_texto(lista, inicio, fim).into_iter().map(|c| (c & 0xFFFF) as u16).collect();
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Dois(&u)))
}

/// `_StringBase._createFromCodePoints(list, start, end)`: pontos de código
/// (acima de U+FFFF viram o par substituto).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_createFromCodePoints(lista: i64, inicio: i64, fim: i64) -> i64 {
    let mut u = Vec::new();
    for c in codigos_da_lista_de_texto(lista, inicio, fim) {
        // Fora de `0..=0x10FFFF`: `ArgumentError` sem mensagem, como o
        // `StringBase_createFromCodePoints` da VM.
        if !(0..=0x10FFFF).contains(&c) {
            let e = dartforge_argument_error_new(0, 0);
            dartforge_exception_throw(e, 3);
            return 0;
        }
        crate::textos::empurrar_ponto(&mut u, c as u32);
    }
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Dois(&u)))
}

/// `_StringBase._joinReplaceAllResult(base, matches, length, oneByte)`: as
/// fatias de `base` (um `Smi` negativo `-(início << 11 | tamanho)`, ou o par
/// início, fim) e as substituições, na ordem (`string_patch.dart`). Uma
/// medição, uma alocação e as cópias dos blocos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_StringBase_joinReplaceAllResult(base: i64, partes: i64, _tamanho: i64, _um_byte: u8) -> i64 {
    // (fonte, início, fim) de cada pedaço; as fontes vivem pela lista e por
    // `base` (enraizadas pelo chamador).
    let pedacos: Vec<(i64, usize, usize)> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = heap.lista_len(partes);
        let mut v = Vec::new();
        let mut i = 0;
        while i < n {
            let x = heap.lista_get(partes, i);
            match x {
                crate::heap::Valor::Ref(r) if crate::layout::e_objeto(r) && heap.e_texto(r) => {
                    let k = heap.texto(r).map_or(0, crate::textos::TextoRef::len);
                    v.push((r, 0, k));
                }
                _ => {
                    let a = inteiro_do_valor(&heap, x).unwrap_or(0);
                    let (ini, fim) = if a < 0 {
                        let bits = -a;
                        let ini = bits >> 11;
                        (ini, ini + (bits & ((1 << 11) - 1)))
                    } else {
                        i += 1;
                        (a, inteiro_do_valor(&heap, heap.lista_get(partes, i)).unwrap_or(a))
                    };
                    v.push((base, ini as usize, fim as usize));
                }
            }
            i += 1;
        }
        v
    });
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (mut total, mut um) = (0, true);
        for &(f, a, b) in &pedacos {
            let t = heap.texto(f).expect("bug do compilador: string esperada").fatia(a, b);
            total += t.len();
            um &= t.cabe_em_um_byte();
        }
        let r = heap.novo_texto(total, !um);
        let mut pos = 0;
        for &(f, a, b) in &pedacos {
            heap.copiar_texto(r, pos, f, a, b);
            pos += b - a;
        }
        r
    })
}

// ─── O acumulador do `StringBuffer` (`_AcumuladorDeTexto`, cid 15) ──────────
//
// Um bloco `BRUTO`+`ANEXO` de uma palavra: o ponteiro de um `Box<Vec<u16>>`
// com as unidades escritas, que o coletor solta (`soltar_acumulador`) quando o
// bloco morre (§2.8). Só o `StringBuffer` o vê
// (`sdk_nativo/core/string_buffer_patch.dart`).

/// Solta o `Vec<u16>` de um acumulador morto (chamado pelo coletor).
///
/// # Safety
/// `p` saiu de `Box::into_raw` de um `Box<Vec<u16>>` em
/// [`dartforge_nativo_DartForge_sb_novo`] e é solto uma vez só.
unsafe fn soltar_acumulador(p: *mut u8) {
    // SAFETY: contrato acima.
    drop(unsafe { Box::from_raw(p.cast::<Vec<u16>>()) });
}

/// Um acumulador novo com as unidades `unidades` (o `StringBuffer` vazio, ou a
/// cópia de um acumulador que chega por mensagem de outro isolado, §2.12).
fn novo_acumulador_com(unidades: Vec<u16>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let h = heap.alocar(
            crate::layout::cid::ACUMULADOR_DE_TEXTO,
            1,
            crate::layout::flags::BRUTO | crate::layout::flags::ANEXO,
        );
        let bytes = std::mem::size_of::<Vec<u16>>() + 2 * unidades.capacity();
        let p = Box::into_raw(Box::new(unidades)).cast::<u8>();
        // SAFETY: `soltar_acumulador` é a única liberação de `p`.
        unsafe { heap.anexar(h, soltar_acumulador, p, bytes) };
        h
    })
}

/// O `Vec<u16>` do acumulador `h`.
///
/// # Panics
/// Se `h` não é um acumulador (bug do compilador).
fn vetor_do_acumulador(heap: &Heap, h: i64) -> *mut Vec<u16> {
    assert!(
        crate::layout::e_objeto(h) && heap.classe(h) == crate::layout::cid::ACUMULADOR_DE_TEXTO,
        "bug do compilador: acumulador de StringBuffer esperado\nhandle {h}"
    );
    heap.anexo(h).cast::<Vec<u16>>()
}

/// Acrescenta as unidades de `t` (que não vem de um bloco que possa morrer
/// aqui) ao acumulador `acumulador`.
fn acrescentar_ao_acumulador(acumulador: i64, t: crate::textos::TextoRef<'_>) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let v = vetor_do_acumulador(&heap, acumulador);
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
    novo_acumulador_com(Vec::new())
}

/// Acrescenta as unidades da string `texto` ao acumulador.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever(acumulador: i64, texto: i64) {
    // O retorno é -1 só quando `texto` não é string, o que o Dart não deixa.
    let n = dartforge_nativo_DartForge_sb_escrever_se_texto(acumulador, texto);
    assert!(n >= 0, "bug do compilador: StringBuffer._escrever sem String");
}

/// `StringBuffer._escreverSeTexto(acumulador, obj)` da sobreposição: se
/// `obj` é uma string, acrescenta as unidades dela ao acumulador e devolve
/// quantas; senão -1, sem mudar nada (o Dart segue pelo `toString`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever_se_texto(acumulador: i64, obj: i64) -> i64 {
    if !crate::layout::e_objeto(obj) {
        return -1;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // As unidades lidas direto do bloco da string (viva: é o argumento); o
        // acumulador mora fora do heap, então crescer não as move.
        let Some(t) = heap.texto(obj) else { return -1 };
        let v = vetor_do_acumulador(&heap, acumulador);
        // SAFETY: o `Vec` do anexo mora fora do heap; nada mais o empresta.
        let v = unsafe { &mut *v };
        match t {
            crate::textos::TextoRef::Um(b) => v.extend(b.iter().map(|&x| u16::from(x))),
            crate::textos::TextoRef::Dois(u) => v.extend_from_slice(u),
        }
        t.len() as i64
    })
}

/// Acrescenta o ponto de código `codigo` ao acumulador (`writeCharCode`
/// da sobreposição, que já conferiu `0 <= codigo <= 0x10FFFF`): uma
/// unidade, ou o par de surrogates acima de 0xFFFF, como a VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever_codigo(acumulador: i64, codigo: i64) {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        // SAFETY: o `Vec` do anexo mora fora do heap; nada mais o empresta.
        let v = unsafe { &mut *vetor_do_acumulador(&heap, acumulador) };
        crate::textos::empurrar_ponto(v, codigo as u32);
    });
}

/// A string com o conteúdo do acumulador (na forma canônica); o acumulador
/// continua podendo crescer.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_texto(acumulador: i64) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let v = vetor_do_acumulador(&heap, acumulador);
        // SAFETY: o `Vec` mora fora do heap e só o solta a morte do
        // acumulador, que está enraizado pelo chamador durante a alocação.
        let u: &[u16] = unsafe { &*v };
        heap.alocar_texto(crate::textos::TextoRef::Dois(u))
    })
}

/// `_StringBase._deCodigos(lista, inicio, fim)` (DartForge, o
/// `createFromCharCodes` de `string_patch.dart`): as unidades `[inicio,
/// fim)` de uma lista do núcleo (`_List`, `_GrowableList`, `_ImmutableList`),
/// todas `int` em `0..=0xFF`, viram a `_OneByteString` numa cópia só — o
/// `_scanCodeUnits` e o `_setAt` por unidade da VM, que aqui eram uma chamada
/// ao runtime cada. Qualquer outra coisa (unidade acima de 0xFF, negativa ou
/// não `int`, faixa inválida, lista tipada): null, e o Dart segue pelo caminho
/// da VM, com os erros dela.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_de_codigos(lista: i64, inicio: i64, fim: i64) -> i64 {
    let bytes = HEAP.with(|heap| {
        let heap = heap.borrow();
        if !crate::layout::e_objeto(lista) || !heap.e_lista(lista) {
            return None;
        }
        let (i, f) = (usize::try_from(inicio).ok()?, usize::try_from(fim).ok()?);
        if i > f || f > heap.lista_len(lista) {
            return None;
        }
        let e = heap.lista_elementos(lista);
        let mut v = Vec::with_capacity(f - i);
        for k in i..f {
            let n = match e.get(k) {
                crate::heap::Valor::Int(x) => x,
                crate::heap::Valor::Ref(r) if crate::layout::smi::e_smi(r) => crate::layout::smi::valor(r),
                _ => return None,
            };
            v.push(u8::try_from(n).ok()?);
        }
        Some(v)
    });
    bytes.map_or(0, |b| HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Um(&b))))
}

// ─── Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §3.6) ──────────────────

/// Uma string nova de `len` unidades zeradas (`dois` ≠ 0: `_TwoByteString`): o
/// caminho lento de `@df.texto_alocar` (acima de 16 palavras, ou TLAB
/// esgotada). Não lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_texto_novo(len: i64, dois: i64) -> i64 {
    let len = usize::try_from(len).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().novo_texto(len, dois != 0))
}

/// O hash da string `s`, calculado e gravado no cabeçalho (`mapa`): o caminho
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
    HEAP.with(|heap| i8::from(heap.borrow().textos_iguais(a, b)))
}
