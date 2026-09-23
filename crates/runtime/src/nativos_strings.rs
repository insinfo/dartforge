// Runtime nativo: natives de `String` do SDK da fonte (P5b).
//
// Os natives de `runtime/lib/string.cc` e `Internal_*` de string
// (`internal_patch.dart`) sobre o `Texto` (`heap.rs`): `_OneByteString` e
// `_TwoByteString` com unidades UTF-16. O receptor é o `Ref` da string;
// `int` chega como `i64`. A tabela que o lowering consulta é
// `crates/emit_native/src/nativos.rs`.

/// `String_getLength`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_getLength(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().texto(this).len() as i64)
}

/// `String_charAt` (`s[i]`): a string de uma unidade; fora dos limites,
/// `RangeError.range(i, 0, length - 1, "index")` (`StringValueAt`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_charAt(this: i64, indice: i64) -> i64 {
    let t = texto_de(this);
    if indice < 0 || indice as usize >= t.len() {
        lancar_range(indice, 0, t.len() as i64 - 1, "index");
        return 0;
    }
    alocar_texto(t.fatia(indice as usize, indice as usize + 1))
}

/// `String_concat` (`+`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_concat(this: i64, outro: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_concat(this, outro))
}

/// `String_getHashCode`: o `StringHasher` da VM (`Texto::hash_vm`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_String_getHashCode(this: i64) -> i64 {
    HEAP.with(|heap| heap.borrow().texto(this).hash_vm())
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
    let t = texto_de(this);
    alocar_texto(t.fatia(inicio as usize, fim as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_OneByteString_substringUnchecked(this: i64, inicio: i64, fim: i64) -> i64 {
    dartforge_nativo_StringBase_substringUnchecked(this, inicio, fim)
}

/// `Internal_allocateOneByteString`: `_OneByteString` de `n` unidades 0,
/// para ser preenchida por `writeIntoOneByteString` (o `dart:convert` e o
/// `_StringBase` montam strings assim).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_allocateOneByteString(n: i64) -> i64 {
    alocar_texto(Texto::Um(vec![0; usize::try_from(n).unwrap_or(0)]))
}

/// `Internal_allocateTwoByteString`: `_TwoByteString` de `n` unidades 0.
/// Fica `Dois` mesmo que só receba unidades Latin-1, como na VM; a
/// igualdade é por unidades, então isso só custa espaço.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_allocateTwoByteString(n: i64) -> i64 {
    alocar_texto(Texto::Dois(vec![0; usize::try_from(n).unwrap_or(0)]))
}

/// `Internal_writeIntoOneByteString`: grava a unidade `codigo & 0xFF` no
/// índice (a string foi alocada agora; o Dart conferiu índice e faixa).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoOneByteString(string: i64, indice: i64, codigo: i64) {
    HEAP.with(|heap| {
        if let Value::String(Texto::Um(b)) = heap.borrow_mut().get_mut(string) {
            b[indice as usize] = codigo as u8;
        } else {
            panic!("bug do compilador: writeIntoOneByteString sobre string que não é _OneByteString");
        }
    });
}

/// `Internal_writeIntoTwoByteString`: grava a unidade `codigo & 0xFFFF`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_Internal_writeIntoTwoByteString(string: i64, indice: i64, codigo: i64) {
    HEAP.with(|heap| {
        if let Value::String(Texto::Dois(u)) = heap.borrow_mut().get_mut(string) {
            u[indice as usize] = codigo as u16;
        } else {
            panic!("bug do compilador: writeIntoTwoByteString sobre string que não é _TwoByteString");
        }
    });
}
