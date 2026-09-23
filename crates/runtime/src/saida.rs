// Runtime nativo: `print` e `toString` dos valores do runtime.

/// Imprime um inteiro assinado recebido do módulo LLVM.
// SAFETY: o nome é reservado pelo compilador e há exatamente uma definição.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_i64(value: i64) {
    println!("{value}");
}

/// Imprime o booleano representado pelo byte 0 ou 1 no contrato LLVM.
// SAFETY: símbolo reservado; u8 evita passar um bool Rust inválido pela ABI.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_bool(value: u8) {
    println!("{}", value != 0);
}

/// Imprime o valor null sem expor uma representação de objeto pela ABI.
// SAFETY: símbolo reservado, definido exatamente uma vez no runtime.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_null() {
    println!("null");
}

/// Imprime ponto flutuante de 64 bits (double).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_f64(value: f64) {
    if value.fract() == 0.0 && !value.is_infinite() && !value.is_nan() {
        println!("{value:.1}");
    } else {
        println!("{value}");
    }
}

/// Escreve uma linha de texto Dart na saída padrão como o
/// `Builtin_PrintString` da VM: UTF-8 com o surrogate solto trocado por
/// U+FFFD (`Dart_CopyUTF8EncodingOfString` → `Utf8::Encode`), escrito por
/// bytes (NUL inclusive).
fn imprimir_texto(t: &Texto) {
    use std::io::Write;
    let mut bytes = t.para_utf8_da_vm();
    bytes.push(b'\n');
    let mut saida = std::io::stdout().lock();
    let _ = saida.write_all(&bytes);
}

/// Imprime qualquer objeto gerenciado pelo handle.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_handle(handle: i64) {
    if handle == 0 {
        println!("null");
        return;
    }
    let t = HEAP.with(|heap| describe_texto(&heap.borrow(), handle));
    imprimir_texto(&t);
}

/// `double` como a VM imprime (`1.0`, `0.5`, `NaN`, `Infinity`).
fn formatar_double(d: f64) -> String {
    if d.is_nan() {
        "NaN".to_string()
    } else if d.is_infinite() {
        if d > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() }
    } else if d.fract() == 0.0 {
        format!("{d:.1}")
    } else {
        d.to_string()
    }
}

/// Imprime conteúdo da string gerenciada ou null para handle zero.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_string(handle: i64) {
    if handle == 0 {
        println!("null");
        return;
    }
    let t = HEAP.with(|heap| heap.borrow().texto(handle).clone());
    imprimir_texto(&t);
}

/// `Error.safeToString` do `dart:core`.
///
/// Referência: `sdk/lib/core/errors.dart`. Ela existe justamente para NÃO
/// chamar o `toString` do objeto — um `toString` que lança dentro da
/// construção de uma mensagem de erro esconderia o erro original. Portanto:
/// números, `bool` e `null` pelo `toString` deles; `String` entre aspas com
/// escapes; qualquer outra coisa pela forma de identidade do objeto.
///
/// As formas de identidade abaixo são as que a VM 3.6.2 imprime para as
/// coleções embutidas (`Instance(length:N) of '_GrowableList'`, `_Map len:N`,
/// `_Set len:N`), verificadas no oráculo; o corpus compara byte a byte.
fn safe_to_string(heap: &Heap, handle: i64, output: &mut TextoMut) {
    if handle == 0 {
        output.push_str("null");
        return;
    }
    match heap.get(handle) {
        Value::String(text) => {
            output.push('"');
            for u in text.unidades() {
                match u {
                    0x0A => output.push_str("\\n"),
                    0x0D => output.push_str("\\r"),
                    0x09 => output.push_str("\\t"),
                    0x22 => output.push_str("\\\""),
                    0x5C => output.push_str("\\\\"),
                    u => output.0.push(u),
                }
            }
            output.push('"');
        }
        Value::List(items) => {
            output.push_str(&format!("Instance(length:{}) of '_GrowableList'", items.len()))
        }
        Value::Map(pares) => output.push_str(&format!("_Map len:{}", pares.len())),
        Value::BoxedInt(i) => output.push_str(&i.to_string()),
        Value::BoxedDouble(d) => output.push_str(&formatar_double(*d)),
        Value::BoxedBool(b) => output.push_str(if *b { "true" } else { "false" }),
        Value::Set(items) => output.push_str(&format!("_Set len:{}", items.len())),
        Value::Object { class_id, .. } => {
            let nome = CLASS_NAMES
                .with(|map| map.borrow().get(class_id).cloned())
                .unwrap_or_else(|| "Object".to_string());
            output.push_str(&format!("Instance of '{nome}'"));
        }
        _ => output.push_texto(&describe_texto(heap, handle)),
    }
}

/// [`describe_texto`] como `String` do Rust, para mensagens (surrogate
/// solto vira U+FFFD).
fn describe_handle(heap: &Heap, handle: i64) -> String {
    describe_texto(heap, handle).para_string()
}

/// Descreve um handle (o `toString` dos valores do runtime), em unidades
/// UTF-16: a interpolação e o `print` de uma coleção com string que tem
/// surrogate solto não o perdem.
///
/// Profundidade limitada a 4 e 101 elementos, como a abreviação do SDK 3.6.2
/// para iteráveis; a forma exata é do subconjunto, não do SDK.
fn describe_texto(heap: &Heap, handle: i64) -> Texto {
    fn render(heap: &Heap, value: TaggedValue, depth: usize, output: &mut TextoMut) {
        if depth == 0 {
            output.push_str("...");
            return;
        }
        if value.is_ref {
            if value.bits == 0 {
                output.push_str("null");
                return;
            }
            match heap.get(value.bits) {
                Value::String(text) | Value::RegExp(text) | Value::Match(text) => {
                    output.push_texto(text);
                }
                Value::StringBuffer(v) => {
                    output.0.extend_from_slice(v);
                }
                Value::List(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('[');
                    for (index, item) in items.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    if items.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push(']');
                }
                Value::Map(entries) => {
                    let entries: Vec<(TaggedValue, TaggedValue)> = entries.clone();
                    output.push('{');
                    for (index, (key, item)) in entries.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *key, depth - 1, output);
                        output.push_str(": ");
                        render(heap, *item, depth - 1, output);
                    }
                    if entries.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push('}');
                }
                Value::Set(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('{');
                    for (index, item) in items.iter().take(101).enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    if items.len() > 101 {
                        output.push_str(", ...");
                    }
                    output.push('}');
                }
                Value::Closure { .. }
                | Value::Environment(_)
                | Value::Cell(_) => {
                    output.push_str("Instance");
                }
                // Caixas (R3): imprimem como o escalar que carregam.
                Value::BoxedInt(i) => output.push_str(&i.to_string()),
                Value::BoxedDouble(d) => output.push_str(&formatar_double(*d)),
                Value::BoxedBool(b) => output.push_str(if *b { "true" } else { "false" }),
                Value::Record(items) => {
                    let items: Vec<TaggedValue> = items.clone();
                    output.push('(');
                    for (index, item) in items.iter().enumerate() {
                        if index > 0 {
                            output.push_str(", ");
                        }
                        render(heap, *item, depth - 1, output);
                    }
                    output.push(')');
                }
                Value::Object { class_id, fields } => {
                    if *class_id == 1013 {
                        if let Some((b, _)) = fields.first() {
                            output.push_str(&format!("{b}"));
                        }
                        return;
                    }
                    let name = CLASS_NAMES.with(|map| map.borrow().get(class_id).cloned())
                        .unwrap_or_else(|| "Object".to_string());
                    if name == "Exception" || name == "_Exception" {
                        if !fields.is_empty() && fields[0].0 != 0 {
                            output.push_str("Exception: ");
                            let val = TaggedValue {
                                bits: fields[0].0,
                                is_ref: fields[0].1,
                                tag: if fields[0].1 { ValueTag::Ref } else { ValueTag::Int },
                            };
                            render(heap, val, depth - 1, output);
                        } else {
                            output.push_str("Exception");
                        }
                    } else if name == "FormatException" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let s_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let off_opt = fields.get(2).and_then(|(b, _)| if *b >= 0 { Some(*b) } else { None });
                        if let Some(s) = s_opt {
                            let m = m_opt.unwrap_or_default();
                            if let Some(off) = off_opt {
                                let char_idx = off + 1;
                                let spaces = " ".repeat(off as usize);
                                if m.is_empty() {
                                    output.push_str(&format!("FormatException (at character {char_idx})\n{s}\n{spaces}^\n"));
                                } else {
                                    output.push_str(&format!("FormatException: {m} (at character {char_idx})\n{s}\n{spaces}^\n"));
                                }
                            } else if m.is_empty() {
                                output.push_str(&format!("FormatException\n{s}"));
                            } else {
                                output.push_str(&format!("FormatException: {m}\n{s}"));
                            }
                        } else if let Some(m) = m_opt {
                            if m.is_empty() {
                                output.push_str("FormatException");
                            } else {
                                output.push_str(&format!("FormatException: {m}"));
                            }
                        } else {
                            output.push_str("FormatException");
                        }
                    } else if name == "StateError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => Texto::vazio() };
                                output.push_str(&format!("Bad state: {m}"));
                            } else {
                                output.push_str("Bad state");
                            }
                        } else {
                            output.push_str("Bad state");
                        }
                    } else if name == "ArgumentError" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let n_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let has_val = fields.get(3).map_or(false, |(b, _)| *b != 0);
                        if has_val {
                            let val_str = if let Some(&(v_bits, is_ref)) = fields.get(2) {
                                if is_ref {
                                    if v_bits == 0 {
                                        "null".to_string()
                                    } else {
                                        match heap.get(v_bits) {
                                            Value::String(s) => format!("\"{s}\""),
                                            _ => {
                                                let mut tmp = TextoMut::new();
                                                render(heap, TaggedValue::reference(v_bits), depth - 1, &mut tmp);
                                                tmp.fim().para_string()
                                            }
                                        }
                                    }
                                } else {
                                    v_bits.to_string()
                                }
                            } else {
                                "null".to_string()
                            };
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("Invalid argument ({n}): {m}: {val_str}")),
                                (Some(n), None) => output.push_str(&format!("Invalid argument ({n}): {val_str}")),
                                (None, Some(m)) => output.push_str(&format!("Invalid argument: {m}: {val_str}")),
                                (None, None) => output.push_str(&format!("Invalid argument: {val_str}")),
                            }
                        } else {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("Invalid argument(s) ({n}): {m}")),
                                (Some(n), None) => output.push_str(&format!("Invalid argument(s) ({n})")),
                                (None, Some(m)) => output.push_str(&format!("Invalid argument(s): {m}")),
                                (None, None) => output.push_str("Invalid argument(s)"),
                            }
                        }
                    } else if name == "RangeError" {
                        let m_opt = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let n_opt = fields.get(1).and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None });
                        let inv_val = fields.get(2).map_or(0, |(b, _)| *b);
                        let start_val = fields.get(3).map_or(0, |(b, _)| *b);
                        let end_val = fields.get(4).map_or(0, |(b, _)| *b);
                        let kind = fields.get(5).map_or(0, |(b, _)| *b);
                        let has_val = fields.get(6).map_or(false, |(b, _)| *b != 0);

                        if kind == 1 {
                            // `RangeError._errorExplanation` (dart:core/errors.dart).
                            let explicacao = if end_val > start_val {
                                format!("Not in inclusive range {start_val}..{end_val}")
                            } else if end_val < start_val {
                                "Valid value range is empty".to_string()
                            } else {
                                format!("Only valid value is {start_val}")
                            };
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: {explicacao}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Invalid value: {explicacao}: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: {explicacao}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Invalid value: {explicacao}: {inv_val}")),
                            }
                        } else if kind == 2 {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: index should be less than {end_val}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Index out of range: index should be less than {end_val}: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: index should be less than {end_val}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Index out of range: index should be less than {end_val}: {inv_val}")),
                            }
                        } else if has_val {
                            match (n_opt, m_opt) {
                                (Some(n), Some(m)) => output.push_str(&format!("RangeError ({n}): {m}: {inv_val}")),
                                (Some(n), None) => output.push_str(&format!("RangeError ({n}): Value not in range: {inv_val}")),
                                (None, Some(m)) => output.push_str(&format!("RangeError: {m}: {inv_val}")),
                                (None, None) => output.push_str(&format!("RangeError: Value not in range: {inv_val}")),
                            }
                        } else if let Some(m) = m_opt {
                            output.push_str(&format!("RangeError: {m}"));
                        } else {
                            output.push_str("RangeError");
                        }
                    } else if name == "UnsupportedError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => Texto::vazio() };
                                output.push_str(&format!("Unsupported operation: {m}"));
                            } else {
                                output.push_str("Unsupported operation");
                            }
                        } else {
                            output.push_str("Unsupported operation");
                        }
                    } else if name == "UnimplementedError" {
                        if let Some((b, _)) = fields.first() {
                            if *b != 0 {
                                let m = match heap.get(*b) { Value::String(s) => s.clone(), _ => Texto::vazio() };
                                if m.is_empty() {
                                    output.push_str("UnimplementedError");
                                } else {
                                    output.push_str(&format!("UnimplementedError: {m}"));
                                }
                            } else {
                                output.push_str("UnimplementedError");
                            }
                        } else {
                            output.push_str("UnimplementedError");
                        }
                    } else if name == "AssertionError" {
                        if let Some(&(b, is_ref)) = fields.first() {
                            if b != 0 {
                                if is_ref {
                                    match heap.get(b) {
                                        Value::String(s) => output.push_str(&format!("Assertion failed: \"{s}\"")),
                                        _ => {
                                            let mut s = TextoMut::new();
                                            render(heap, TaggedValue::reference(b), depth - 1, &mut s);
                                            output.push_str("Assertion failed: "); output.0.extend(s.0);
                                        }
                                    }
                                } else {
                                    output.push_str(&format!("Assertion failed: {b}"));
                                }
                            } else {
                                output.push_str("Assertion failed");
                            }
                        } else {
                            output.push_str("Assertion failed");
                        }
                    } else if name == "ConcurrentModificationError" {
                        // dart:core/errors.dart: sem `modifiedObject` o texto
                        // termina no ponto; com ele entra
                        // `Error.safeToString(modifiedObject)`, que NÃO chama o
                        // `toString` do objeto — usa a forma de identidade da
                        // VM. Por isso a VM imprime
                        // `Instance(length:4) of '_GrowableList'` e não `[1,2,3,1]`.
                        let alvo = fields.first().map_or(0, |(b, _)| *b);
                        if alvo == 0 {
                            output.push_str("Concurrent modification during iteration.");
                        } else {
                            let mut s = TextoMut::new();
                            safe_to_string(heap, alvo, &mut s);
                            output.push_str("Concurrent modification during iteration: "); output.0.extend(s.0); output.push('.');
                        }
                    } else if name == "TypeError" {
                        output.push_str("TypeError");
                    } else if name == "NoSuchMethodError" {
                        // PENDENTE: a VM imprime
                        // "NoSuchMethodError: Class 'X' has no instance getter
                        // 'y'." — para isso falta o nome da classe do receptor
                        // no ponto do lancamento. Ate la, o nome do membro ja
                        // e o que torna a falha diagnosticavel no placar.
                        let nome_membro = fields
                            .first()
                            .and_then(|(b, _)| if *b != 0 {
                                match heap.get(*b) {
                                    Value::String(s) => Some(s.clone()),
                                    _ => None,
                                }
                            } else {
                                None
                            });
                        match nome_membro {
                            Some(m) => output.push_str(&format!("NoSuchMethodError: {m}")),
                            None => output.push_str("NoSuchMethodError"),
                        }
                    } else if name == "StackTrace" || name == "_StackTrace" {
                        if let Some(m) = fields.first().and_then(|(b, _)| if *b != 0 { match heap.get(*b) { Value::String(s) => Some(s.clone()), _ => None } } else { None }) {
                            output.push_texto(&m);
                        } else {
                            output.push_str("#0      main (dart:native)\n");
                        }
                    } else {
                        output.push_str(&format!("Instance of '{name}'"));
                    }
                }
            }
            return;
        }
        use crate::heap::ValueTag;
        match value.tag {
            ValueTag::Bool => output.push_str(if value.bits != 0 { "true" } else { "false" }),
            ValueTag::Int => output.push_str(&value.bits.to_string()),
            ValueTag::Double => {
                let d = f64::from_bits(value.bits as u64);
                if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                    output.push_str(&format!("{d:.1}"));
                } else {
                    output.push_str(&d.to_string());
                }
            }
            ValueTag::Ref => {
                if value.bits != 0 {
                    render(heap, value, depth - 1, output);
                } else {
                    output.push_str("null");
                }
            }
        }
    }
    let mut output = TextoMut::new();
    render(
        heap,
        TaggedValue::reference(handle),
        4,
        &mut output,
    );
    output.fim()
}

/// Imprime uma lista gerenciada no formato `[1, 2]`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_list(handle: i64) {
    dartforge_print_handle(handle);
}

/// Imprime um mapa gerenciado no formato `{1: a}`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_map(handle: i64) {
    dartforge_print_handle(handle);
}

/// Imprime um conjunto gerenciado no formato `{1, 2}`, como `print` de Dart.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_set(handle: i64) {
    dartforge_print_handle(handle);
}

/// `double.toString` na forma que o runtime usa hoje (`1.0`, `0.5`).
fn texto_de_double(d: f64) -> String {
    if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
        format!("{d:.1}")
    } else {
        d.to_string()
    }
}

/// Converte um valor (bits, tag) para uma string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tagged_to_string(bits: i64, tag: u8) -> i64 {
    let value = tagged(bits, tag);
    let texto = match value.tag {
        crate::heap::ValueTag::Int => Texto::de_str(&value.bits.to_string()),
        crate::heap::ValueTag::Bool => Texto::de_str(if value.bits != 0 { "true" } else { "false" }),
        crate::heap::ValueTag::Double => Texto::de_str(&texto_de_double(f64::from_bits(value.bits as u64))),
        crate::heap::ValueTag::Ref => return dartforge_to_string_handle(value.bits),
    };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(texto)))
}

/// Converte um inteiro para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_i64(value: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str(&value.to_string()))))
}

/// Converte um double para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_f64(value: f64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str(&texto_de_double(value)))))
}

/// Converte um booleano para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_bool(value: u8) -> i64 {
    let s = if value != 0 { "true" } else { "false" };
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str(s))))
}

/// Converte qualquer handle (String, Object, List, Map, Set, Record, null)
/// para string. Uma string é devolvida ela mesma (`String.toString` é `this`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_handle(handle: i64) -> i64 {
    if handle == 0 {
        return HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(Texto::de_str("null"))));
    }
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if let Some(Value::String(_)) = heap_ref.try_get(handle) {
            return handle;
        }
        let text = describe_texto(&heap_ref, handle);
        drop(heap_ref);
        heap.borrow_mut().allocate(Value::String(text))
    })
}
