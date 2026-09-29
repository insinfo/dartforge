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
    let r = com_texto(this, |t| {
        usize::try_from(indice).ok().filter(|&i| i < t.len()).map(|i| t.fatia(i, i + 1)).ok_or(t.len())
    });
    match r {
        Ok(f) => alocar_texto(f),
        Err(n) => {
            lancar_range(indice, 0, n as i64 - 1, "index");
            0
        }
    }
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
    let f = com_texto(this, |t| t.fatia(inicio as usize, fim as usize));
    alocar_texto(f)
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

// Os laços de `string_patch.dart` que a VM faz com `codeUnitAt` intrínseco
// (uma leitura em linha) e aqui custariam uma chamada ao runtime por
// unidade: a comparação de trecho (`startsWith`, `endsWith`), a busca
// (`indexOf`, `lastIndexOf`, `contains`) e a divisão por um caractere
// (`split`). O resultado é o mesmo laço, feito sobre o `Texto`.

/// As unidades de `outro` aparecem em `texto` a partir de `inicio`?
fn trecho_igual(texto: &Texto, inicio: usize, outro: &Texto) -> bool {
    match (texto, outro) {
        (Texto::Um(a), Texto::Um(b)) => a[inicio..inicio + b.len()] == b[..],
        (Texto::Dois(a), Texto::Dois(b)) => a[inicio..inicio + b.len()] == b[..],
        _ => (0..outro.len()).all(|i| texto.unidade(inicio + i) == outro.unidade(i)),
    }
}

/// `_StringBase._substringMatches(start, other)`: `other` vazio casa; fora
/// dos limites, não.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_regiao_igual(this: i64, inicio: i64, outro: i64) -> u8 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (t, o) = (heap.texto(this), heap.texto(outro));
        if o.is_empty() {
            return 1;
        }
        let Ok(i) = usize::try_from(inicio) else { return 0 };
        u8::from(i + o.len() <= t.len() && trecho_igual(t, i, o))
    })
}

/// `indexOf(String other, start)` depois da conferência de `start` (feita
/// no Dart): o primeiro índice `>= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_indice_de(this: i64, outro: i64, inicio: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (t, o) = (heap.texto(this), heap.texto(outro));
        let Some(maximo) = t.len().checked_sub(o.len()) else { return -1 };
        let inicio = usize::try_from(inicio).unwrap_or(0);
        if let (Texto::Um(a), Texto::Um(b)) = (t, o)
            && let Some(&primeiro) = b.first()
        {
            // O primeiro byte filtra as posições.
            let mut i = inicio;
            while i <= maximo {
                match a[i..=maximo].iter().position(|&x| x == primeiro) {
                    None => return -1,
                    Some(k) => i += k,
                }
                if a[i..i + b.len()] == b[..] {
                    return i as i64;
                }
                i += 1;
            }
            return -1;
        }
        (inicio..=maximo).find(|&i| trecho_igual(t, i, o)).map_or(-1, |i| i as i64)
    })
}

/// `lastIndexOf(String other, start)` depois da conferência de `start`: o
/// último índice `<= start` onde `other` casa, ou -1.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_ultimo_indice_de(this: i64, outro: i64, inicio: i64) -> i64 {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        let (t, o) = (heap.texto(this), heap.texto(outro));
        let Some(maximo) = t.len().checked_sub(o.len()) else { return -1 };
        if inicio < 0 {
            return -1;
        }
        let inicio = (inicio as usize).min(maximo);
        (0..=inicio).rev().find(|&i| trecho_igual(t, i, o)).map_or(-1, |i| i as i64)
    })
}

/// `_OneByteString._splitWithCharCode(charCode)`: acrescenta a `lista` (a
/// `<String>[]` nova do Dart) os pedaços entre as ocorrências da unidade
/// `codigo`, na ordem — o mesmo laço, com cada pedaço alocado aqui.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_string_dividir_por_codigo(this: i64, lista: i64, codigo: i64) {
    let pedacos: Vec<Texto> = HEAP.with(|heap| {
        let heap = heap.borrow();
        let t = heap.texto(this);
        let mut v = Vec::new();
        let mut inicio = 0;
        for i in 0..t.len() {
            if i64::from(t.unidade(i)) == codigo {
                v.push(t.fatia(inicio, i));
                inicio = i + 1;
            }
        }
        v.push(t.fatia(inicio, t.len()));
        v
    });
    for p in pedacos {
        // `this` e `lista` têm raiz no chamador; cada pedaço, assim que
        // alocado, entra na lista (a coleta de uma alocação seguinte o vê).
        HEAP.with(|heap| {
            let mut heap = heap.borrow_mut();
            let h = heap.allocate(Value::String(p));
            heap.list_push(lista, TaggedValue::reference(h));
        });
    }
}

/// O acumulador do `StringBuffer` (`string_buffer_patch.dart`): um
/// `Value::StringBuffer` com as unidades escritas, que só o `StringBuffer`
/// vê.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_novo() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::StringBuffer(Vec::new())))
}

/// Acrescenta as unidades da string `texto` ao acumulador.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_escrever(acumulador: i64, texto: i64) {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        // O texto é copiado antes (os dois são do heap e o acumulador é
        // emprestado para escrita); o caso comum, curto e Latin-1, passa
        // pela pilha.
        let mut curto = [0u8; 256];
        let (n, longo): (usize, Option<Vec<u16>>) = match heap.texto(texto) {
            Texto::Um(b) if b.len() <= curto.len() => {
                curto[..b.len()].copy_from_slice(b);
                (b.len(), None)
            }
            t => (0, Some(t.para_vec())),
        };
        if let Value::StringBuffer(u) = heap.get_mut(acumulador) {
            match longo {
                None => u.extend(curto[..n].iter().map(|&x| u16::from(x))),
                Some(v) => u.extend_from_slice(&v),
            }
        }
    });
}

/// A string com o conteúdo do acumulador (na forma canônica).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_sb_texto(acumulador: i64) -> i64 {
    let t = HEAP.with(|heap| match heap.borrow().get(acumulador) {
        Value::StringBuffer(u) => Texto::de_fatia(u),
        _ => Texto::vazio(),
    });
    alocar_texto(t)
}
