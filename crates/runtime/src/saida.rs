// Runtime nativo: `print` e `toString` dos valores do runtime, pelo cid do
// cabeçalho de cada bloco (docs/NATIVO-ESPACO-UNIFICADO.md §2.4).

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
fn imprimir_texto(t: crate::textos::TextoRef<'_>) {
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
    imprimir_texto(t.vista());
}

/// `double` como a VM imprime (`1.0`, `0.5`, `NaN`, `Infinity`).
fn formatar_double(d: f64) -> String {
    texto_de_double_da_vm(d)
}

/// Imprime conteúdo da string gerenciada ou null para handle zero.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_print_string(handle: i64) {
    if handle == 0 {
        println!("null");
        return;
    }
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.texto(handle) {
            Some(t) => imprimir_texto(t),
            None => imprimir_texto(describe_texto(&heap, handle).vista()),
        }
    });
}

/// O nome registrado da classe `cid` (vazio se não houver).
fn nome_registrado(cid: i32) -> Option<String> {
    CLASS_NAMES.with(|map| map.borrow().get(&i64::from(cid)).cloned())
}

/// A string de um campo `Ref` de um objeto (`None` para escalar, null ou
/// não string).
fn texto_do_campo(heap: &Heap, campo: Option<crate::heap::Campo>) -> Option<Texto> {
    match campo? {
        (b, true) if b != 0 => heap.texto(b).map(|t| t.para_texto()),
        _ => None,
    }
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
/// listas do núcleo (`Instance(length:N) of '_GrowableList'`), verificadas no
/// oráculo; o corpus compara byte a byte.
fn safe_to_string(heap: &Heap, handle: i64, output: &mut TextoMut) {
    use crate::layout::cid;
    if handle == 0 {
        output.push_str("null");
        return;
    }
    if crate::heap::smi::e_smi(handle) {
        output.push_str(&crate::heap::smi::valor(handle).to_string());
        return;
    }
    let classe = heap.classe(handle);
    match classe {
        cid::ONE_BYTE_STRING | cid::TWO_BYTE_STRING => {
            let text = heap.texto(handle).expect("string viva");
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
        cid::LIST | cid::IMMUTABLE_LIST | cid::GROWABLE_LIST => {
            let nome = match classe {
                cid::LIST => "_List",
                cid::IMMUTABLE_LIST => "_ImmutableList",
                _ => "_GrowableList",
            };
            output.push_str(&format!("Instance(length:{}) of '{nome}'", heap.lista_len(handle)));
        }
        cid::MINT => output.push_str(&heap.int_de(handle).unwrap_or(0).to_string()),
        cid::DOUBLE => output.push_str(&formatar_double(heap.double_de(handle).unwrap_or(0.0))),
        cid::BOOL => output.push_str(if heap.bool_de(handle) == Some(true) { "true" } else { "false" }),
        _ => {
            let nome = nome_registrado(classe).unwrap_or_else(|| "Object".to_string());
            output.push_str(&format!("Instance of '{nome}'"));
        }
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
    let mut output = TextoMut::new();
    descrever_valor(heap, crate::heap::Valor::Ref(handle), 4, &mut output);
    output.fim()
}

/// Um valor sem caixa, ou a descrição de um `Ref` ([`descrever_ref`]).
fn descrever_valor(heap: &Heap, value: crate::heap::Valor, depth: usize, output: &mut TextoMut) {
    use crate::heap::Valor;
    if depth == 0 {
        output.push_str("...");
        return;
    }
    match value {
        Valor::Bool(b) => output.push_str(if b { "true" } else { "false" }),
        Valor::Int(i) => output.push_str(&i.to_string()),
        Valor::Double(d) => {
            if d.fract() == 0.0 && !d.is_infinite() && !d.is_nan() {
                output.push_str(&format!("{d:.1}"));
            } else {
                output.push_str(&d.to_string());
            }
        }
        Valor::Ref(r) => descrever_ref(heap, r, depth, output),
    }
}

/// Os elementos `itens` entre `abre` e `fecha`, separados por vírgula, até
/// 101 (depois, `...`).
fn descrever_sequencia(heap: &Heap, itens: &[crate::heap::Valor], depth: usize, abre: char, fecha: char, output: &mut TextoMut) {
    output.push(abre);
    for (index, item) in itens.iter().take(101).enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        descrever_valor(heap, *item, depth - 1, output);
    }
    if itens.len() > 101 {
        output.push_str(", ...");
    }
    output.push(fecha);
}

/// A descrição de um `Ref`, pelo cid do bloco.
fn descrever_ref(heap: &Heap, r: i64, depth: usize, output: &mut TextoMut) {
    use crate::heap::Valor;
    use crate::layout::cid;
    if r == 0 {
        output.push_str("null");
        return;
    }
    // `Smi` (R10): o `int` que ele carrega.
    if crate::heap::smi::e_smi(r) {
        output.push_str(&crate::heap::smi::valor(r).to_string());
        return;
    }
    let classe = heap.classe(r);
    match classe {
        cid::ONE_BYTE_STRING | cid::TWO_BYTE_STRING => {
            output.0.extend(heap.texto(r).expect("string viva").unidades());
        }
        cid::LIST | cid::IMMUTABLE_LIST | cid::GROWABLE_LIST => {
            // Até o 102º: depois do 101º só importa que há mais (a reticência).
            let n = heap.lista_len(r);
            let itens: Vec<Valor> = (0..n.min(102)).map(|i| heap.lista_get(r, i)).collect();
            descrever_sequencia(heap, &itens, depth, '[', ']', output);
        }
        cid::RECORD => {
            let itens: Vec<Valor> = heap.record(r).unwrap_or(&[]).iter().map(|&x| Valor::Ref(x)).collect();
            output.push('(');
            for (index, item) in itens.iter().enumerate() {
                if index > 0 {
                    output.push_str(", ");
                }
                descrever_valor(heap, *item, depth - 1, output);
            }
            output.push(')');
        }
        // Caixas (R3): imprimem como o escalar que carregam.
        cid::MINT => output.push_str(&heap.int_de(r).unwrap_or(0).to_string()),
        cid::DOUBLE => output.push_str(&formatar_double(heap.double_de(r).unwrap_or(0.0))),
        cid::BOOL => output.push_str(if heap.bool_de(r) == Some(true) { "true" } else { "false" }),
        cid::CLOSURE | cid::CONTEXTO | cid::CELULA | cid::ACUMULADOR_DE_TEXTO | cid::PROGRAMA_DE_REGEXP => {
            output.push_str("Instance");
        }
        c if cid::e_tipada(c) || cid::e_simd(c) => {
            // As listas tipadas e os valores SIMD têm o texto do `toString`
            // Dart; este é o nome da classe.
            let name = nome_registrado(c).unwrap_or_default();
            output.push_str(&format!("Instance of '{name}'"));
        }
        _ => descrever_objeto(heap, r, depth, output),
    }
}

/// A descrição de um objeto `INSTANCIA` do programa ou do SDK: o nome da
/// classe, e o texto da VM para as classes de erro do runtime (ids
/// 1000–1013 e as do `dart:core` registradas pelo nome).
fn descrever_objeto(heap: &Heap, r: i64, depth: usize, output: &mut TextoMut) {
    let Some(objeto) = heap.objeto(r) else {
        let name = nome_registrado(heap.classe(r)).unwrap_or_else(|| "Object".to_string());
        output.push_str(&format!("Instance of '{name}'"));
        return;
    };
    let class_id = objeto.class_id;
    let fields: Vec<crate::heap::Campo> = objeto.to_vec();
    if class_id == 1013 {
        if let Some((b, _)) = fields.first() {
            output.push_str(&format!("{b}"));
        }
        return;
    }
    let name = CLASS_NAMES.with(|map| map.borrow().get(&class_id).cloned()).unwrap_or_else(|| "Object".to_string());
    // O campo `i` como `Valor` (um escalar guardado sem caixa é `int`).
    let valor_do_campo = |i: usize| -> crate::heap::Valor {
        match fields.get(i) {
            Some(&(b, true)) => crate::heap::Valor::Ref(b),
            Some(&(b, false)) => crate::heap::Valor::Int(b),
            None => crate::heap::Valor::Ref(0),
        }
    };
    let texto = |i: usize| texto_do_campo(heap, fields.get(i).copied());
    let bruto = |i: usize| fields.get(i).map_or(0, |(b, _)| *b);
    if name == "Exception" || name == "_Exception" {
        if !fields.is_empty() && fields[0].0 != 0 {
            output.push_str("Exception: ");
            descrever_valor(heap, valor_do_campo(0), depth - 1, output);
        } else {
            output.push_str("Exception");
        }
    } else if name == "FormatException" {
        let m_opt = texto(0);
        let s_opt = texto(1);
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
        match fields.first() {
            Some((b, _)) if *b != 0 => {
                let m = texto(0).unwrap_or_else(Texto::vazio);
                output.push_str(&format!("Bad state: {m}"));
            }
            _ => output.push_str("Bad state"),
        }
    } else if name == "ArgumentError" {
        let m_opt = texto(0);
        let n_opt = texto(1);
        let has_val = bruto(3) != 0;
        if has_val {
            let val_str = match fields.get(2) {
                Some(&(0, true)) | None => "null".to_string(),
                Some(&(v_bits, true)) => match heap.texto(v_bits) {
                    Some(s) => format!("\"{s}\""),
                    None => {
                        let mut tmp = TextoMut::new();
                        descrever_ref(heap, v_bits, depth - 1, &mut tmp);
                        tmp.fim().para_string()
                    }
                },
                Some(&(v_bits, false)) => v_bits.to_string(),
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
        let m_opt = texto(0);
        let n_opt = texto(1);
        let inv_val = bruto(2);
        let start_val = bruto(3);
        let end_val = bruto(4);
        let kind = bruto(5);
        let has_val = bruto(6) != 0;

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
        match fields.first() {
            Some((b, _)) if *b != 0 => {
                let m = texto(0).unwrap_or_else(Texto::vazio);
                output.push_str(&format!("Unsupported operation: {m}"));
            }
            _ => output.push_str("Unsupported operation"),
        }
    } else if name == "UnimplementedError" {
        match fields.first() {
            Some((b, _)) if *b != 0 => {
                let m = texto(0).unwrap_or_else(Texto::vazio);
                if m.is_empty() {
                    output.push_str("UnimplementedError");
                } else {
                    output.push_str(&format!("UnimplementedError: {m}"));
                }
            }
            _ => output.push_str("UnimplementedError"),
        }
    } else if name == "AssertionError" {
        match fields.first() {
            Some(&(b, is_ref)) if b != 0 => {
                if is_ref {
                    match heap.texto(b) {
                        Some(s) => output.push_str(&format!("Assertion failed: \"{s}\"")),
                        None => {
                            let mut s = TextoMut::new();
                            descrever_ref(heap, b, depth - 1, &mut s);
                            output.push_str("Assertion failed: ");
                            output.0.extend(s.0);
                        }
                    }
                } else {
                    output.push_str(&format!("Assertion failed: {b}"));
                }
            }
            _ => output.push_str("Assertion failed"),
        }
    } else if name == "ConcurrentModificationError" {
        // dart:core/errors.dart: sem `modifiedObject` o texto termina no
        // ponto; com ele entra `Error.safeToString(modifiedObject)`, que NÃO
        // chama o `toString` do objeto — usa a forma de identidade da VM.
        // Por isso a VM imprime `Instance(length:4) of '_GrowableList'` e não
        // `[1,2,3,1]`.
        let alvo = bruto(0);
        if alvo == 0 {
            output.push_str("Concurrent modification during iteration.");
        } else {
            let mut s = TextoMut::new();
            safe_to_string(heap, alvo, &mut s);
            output.push_str("Concurrent modification during iteration: ");
            output.0.extend(s.0);
            output.push('.');
        }
    } else if name == "TypeError" {
        // Com a mensagem (o `as` do RTI, `tipos.rs`), o `toString` da VM é a
        // mensagem.
        match texto(0) {
            Some(t) => output.push_texto(&t),
            None => output.push_str("TypeError"),
        }
    } else if name == "NoSuchMethodError" {
        // PENDENTE: a VM imprime "NoSuchMethodError: Class 'X' has no
        // instance getter 'y'." — para isso falta o nome da classe do
        // receptor no ponto do lançamento. Até lá, o nome do membro já é o
        // que torna a falha diagnosticável no placar.
        match texto(0) {
            Some(m) => output.push_str(&format!("NoSuchMethodError: {m}")),
            None => output.push_str("NoSuchMethodError"),
        }
    } else if name == "StackTrace" || name == "_StackTrace" {
        match texto(0) {
            Some(m) => output.push_texto(&m),
            // §13.14: os endereços guardados, simbolizados agora.
            None => {
                let retornos = retornos_do_objeto(heap, r);
                output.push_str(&if retornos.is_empty() { RASTRO_SEM_TABELA.to_string() } else { simbolizar_retornos(&retornos) });
            }
        }
    } else {
        output.push_str(&format!("Instance of '{name}'"));
    }
}

/// `double.toString` na forma que o runtime usa hoje (`1.0`, `0.5`).
fn texto_de_double(d: f64) -> String {
    texto_de_double_da_vm(d)
}

/// Uma string nova do heap com `s`.
fn texto_novo(s: &str) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().alocar_str(s))
}

/// Converte um valor (bits, tag) para uma string gerenciada no heap (a ABI
/// plana: 1 `int`, 2 `bool`, 3 `Ref`, 4 `double`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_tagged_to_string(bits: i64, tag: u8) -> i64 {
    match tag {
        1 => dartforge_to_string_i64(bits),
        2 => dartforge_to_string_bool(u8::from(bits != 0)),
        4 => dartforge_to_string_f64(f64::from_bits(bits as u64)),
        _ => dartforge_to_string_handle(bits),
    }
}

/// Converte um inteiro para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_i64(value: i64) -> i64 {
    // Os dígitos direto num buffer da pilha (sem a maquinaria do `fmt` e
    // sem a `String` intermediária).
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = value.unsigned_abs();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    let mut texto = [0u8; 21];
    let mut k = 0;
    if value < 0 {
        texto[0] = b'-';
        k = 1;
    }
    let d = buf.len() - i;
    texto[k..k + d].copy_from_slice(&buf[i..]);
    // Os dígitos já são a forma de um byte: sem passar por `Texto`.
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(crate::textos::TextoRef::Um(&texto[..k + d])))
}

/// Converte um double para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_f64(value: f64) -> i64 {
    texto_novo(&texto_de_double(value))
}

/// Converte um booleano para string gerenciada no heap.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_bool(value: u8) -> i64 {
    texto_novo(if value != 0 { "true" } else { "false" })
}

/// Converte qualquer handle (String, Object, List, Record, null) para
/// string. Uma string é devolvida ela mesma (`String.toString` é `this`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_to_string_handle(handle: i64) -> i64 {
    if handle == 0 {
        return texto_novo("null");
    }
    HEAP.with(|heap| {
        let heap_ref = heap.borrow();
        if crate::layout::e_objeto(handle) && heap_ref.e_texto(handle) {
            return handle;
        }
        let text = describe_texto(&heap_ref, handle);
        drop(heap_ref);
        heap.borrow_mut().alocar_texto(text.vista())
    })
}
