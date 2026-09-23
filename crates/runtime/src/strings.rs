// Runtime nativo: membros de `String`, `StringBuffer`, `RegExp` e `parse`.
//
// As strings são `Texto` (`heap.rs`): unidades UTF-16 na forma da VM,
// `_OneByteString` (Latin-1) ou `_TwoByteString` (decisão 5,
// docs/NATIVO-PLANO.md §7.1). Todo índice, comprimento e busca daqui é em
// unidades de código, como no `dart:core`; nada passa por `String` do Rust
// (que perderia os surrogates soltos). Estes externs são os do mecanismo
// "por nome" (congelado): morrem em P5d, quando os membros vêm da fonte do
// SDK e só os natives (`String_charAt`, `String_concat`, ...) ficam aqui.

/// Aloca uma string gerenciada. O chamador enraíza o resultado.
fn alocar_texto(t: Texto) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::String(t)))
}

/// Aloca uma string a partir de texto que o runtime formatou (UTF-8 válido).
fn alocar_str(s: &str) -> i64 {
    alocar_texto(Texto::de_str(s))
}

/// O texto de uma string (cópia; o empréstimo do heap não sobrevive à
/// próxima alocação). Handle 0 é bug do compilador (N4).
fn texto_de(handle: i64) -> Texto {
    HEAP.with(|heap| heap.borrow().texto(handle).clone())
}

/// Texto de um valor que pode ser string, `StringBuffer` ou `Match`
/// (o lowering por nome chama `length` e `codeUnitAt` sobre os três).
fn texto_de_qualquer(handle: i64) -> Option<Texto> {
    HEAP.with(|heap| match heap.borrow().try_get(handle) {
        Some(Value::String(t)) | Some(Value::Match(t)) => Some(t.clone()),
        Some(Value::StringBuffer(u)) => Some(Texto::de_fatia(u)),
        _ => None,
    })
}

/// Lança `RangeError.range(valor, min, max, nome)`, como o
/// `RangeError.checkValidRange`/`checkValueInInterval` do `dart:core`.
fn lancar_range(valor: i64, min: i64, max: i64, nome: &str) {
    let n = alocar_str(nome);
    let err = com_raizes(&[n], || dartforge_range_error_range(valor, min, max, n, 0));
    dartforge_exception_throw(err, 3);
}

/// `RangeError.checkValidRange(start, end, length)` do `dart:core`
/// (`errors.dart`): devolve o fim efetivo, ou lança e devolve `None`.
/// `end < 0` é o "ausente" do lowering por nome.
fn faixa_valida(start: i64, end: i64, len: usize) -> Option<(usize, usize)> {
    let len = len as i64;
    if start < 0 || start > len {
        lancar_range(start, 0, len, "start");
        return None;
    }
    let end = if end < 0 { len } else { end };
    if start > end || end > len {
        lancar_range(end, start, len, "end");
        return None;
    }
    Some((start as usize, end as usize))
}

/// Copia WTF-8 (UTF-8 válido é WTF-8) de uma constante LLVM para uma
/// string gerenciada.
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
    alocar_texto(Texto::de_wtf8(bytes))
}
/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().string_concat(a, b))
}
/// Compara conteúdo por unidades; dois handles null são iguais.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_equal(a: i64, b: i64) -> u8 {
    HEAP.with(|heap| u8::from(heap.borrow().string_equal(a, b)))
}

/// Comprimento em unidades UTF-16 (`String_getLength`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_len(handle: i64) -> i64 {
    if handle == 0 {
        return 0;
    }
    HEAP.with(|heap| match heap.borrow().get(handle) {
        Value::String(t) | Value::Match(t) => t.len() as i64,
        Value::StringBuffer(u) => u.len() as i64,
        _ => 0,
    })
}

/// A unidade UTF-16 no índice (`codeUnitAt`). Fora dos limites lança o
/// `RangeError` do `CheckBound` da VM (`DRT_RangeError`,
/// `runtime_entry.cc`): `RangeError.range(i, 0, length - 1, "length")`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_unit_at(handle: i64, index: i64) -> i64 {
    let unidade = HEAP.with(|heap| {
        let heap = heap.borrow();
        let n = match heap.get(handle) {
            Value::String(t) | Value::Match(t) => t.len(),
            Value::StringBuffer(u) => u.len(),
            _ => 0,
        };
        if index < 0 || index as usize >= n {
            return Err(n as i64);
        }
        let i = index as usize;
        Ok(match heap.get(handle) {
            Value::String(t) | Value::Match(t) => i64::from(t.unidade(i)),
            Value::StringBuffer(u) => i64::from(u[i]),
            _ => 0,
        })
    });
    match unidade {
        Ok(u) => u,
        Err(n) => {
            lancar_range(index, 0, n - 1, "length");
            0
        }
    }
}

/// Lista de inteiros (os pedaços já são escalares: sem alocação no meio).
fn lista_de_inteiros(valores: impl Iterator<Item = i64>) -> i64 {
    let itens: Vec<TaggedValue> = valores.map(TaggedValue::scalar).collect();
    HEAP.with(|heap| heap.borrow_mut().create_list(itens))
}

/// `codeUnits`: as unidades UTF-16 como inteiros.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_code_units(handle: i64) -> i64 {
    let t = texto_de_qualquer(handle).unwrap_or_else(Texto::vazio);
    lista_de_inteiros(t.unidades().map(i64::from))
}

/// `runes`: os pontos de código; surrogate solto sai como ele mesmo (o
/// `RuneIterator` do `dart:core`), não como U+FFFD.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_runes(handle: i64) -> i64 {
    let t = texto_de_qualquer(handle).unwrap_or_else(Texto::vazio);
    lista_de_inteiros(t.pontos().into_iter().map(i64::from))
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
    alocar_str(&s)
}

/// `substring(start, [end])` em unidades UTF-16, com o
/// `RangeError.checkValidRange` do `dart:core`. `end < 0` = ausente.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_substring(handle: i64, start: i64, end: i64) -> i64 {
    let t = texto_de(handle);
    match faixa_valida(start, end, t.len()) {
        Some((a, b)) => alocar_texto(t.fatia(a, b)),
        None => 0,
    }
}

/// `String.fromCharCode`: ponto ≤ 0xFFFF é uma unidade (inclusive
/// surrogate solto); acima, um par. Fora de 0..0x10FFFF lança
/// `RangeError.range(charCode, 0, 0x10FFFF)`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_code(code: i64) -> i64 {
    if !(0..=0x10FFFF).contains(&code) {
        lancar_range(code, 0, 0x10FFFF, "charCode");
        return 0;
    }
    let mut u = Vec::with_capacity(2);
    crate::heap::empurrar_ponto(&mut u, code as u32);
    alocar_texto(Texto::de_unidades(u))
}

/// `String.fromCharCodes(lista)`: cada elemento é um ponto de código.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_from_char_codes(list_handle: i64) -> i64 {
    let codigos: Vec<i64> = HEAP.with(|heap| match heap.borrow().get(list_handle) {
        Value::List(items) => items.iter().map(|v| v.bits).collect(),
        _ => Vec::new(),
    });
    let mut u = Vec::with_capacity(codigos.len());
    for c in codigos {
        if !(0..=0x10FFFF).contains(&c) {
            lancar_range(c, 0, 0x10FFFF, "charCode");
            return 0;
        }
        crate::heap::empurrar_ponto(&mut u, c as u32);
    }
    alocar_texto(Texto::de_unidades(u))
}

/// O padrão de busca: string ou `RegExp` (este pelo casador rudimentar).
enum Padrao {
    Texto(Texto),
    RegExp(Texto),
}

fn padrao_de(handle: i64) -> Option<Padrao> {
    HEAP.with(|heap| match heap.borrow().try_get(handle) {
        Some(Value::String(t)) => Some(Padrao::Texto(t.clone())),
        Some(Value::RegExp(p)) => Some(Padrao::RegExp(p.clone())),
        _ => None,
    })
}

/// Próxima ocorrência do padrão a partir de `desde`: (início, comprimento).
fn proxima(alvo: &Texto, padrao: &Padrao, desde: usize) -> Option<(usize, usize)> {
    match padrao {
        Padrao::Texto(p) => alvo.procurar(p, desde).map(|i| (i, p.len())),
        Padrao::RegExp(p) => regexp_proxima(alvo, p, desde),
    }
}

/// `indexOf(padrão, [start])` em unidades; -1 se não houver.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    let t = texto_de(handle);
    let Some(p) = padrao_de(pat_handle) else { return -1 };
    if start < 0 || start as usize > t.len() {
        lancar_range(start, 0, t.len() as i64, "start");
        return 0;
    }
    proxima(&t, &p, start as usize).map_or(-1, |(i, _)| i as i64)
}

/// `lastIndexOf(padrão, [start])`; `start < 0` = ausente (o fim).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_last_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    let t = texto_de(handle);
    let Some(Padrao::Texto(p)) = padrao_de(pat_handle) else { return -1 };
    let ate = if start < 0 {
        t.len()
    } else if start as usize > t.len() {
        lancar_range(start, 0, t.len() as i64, "start");
        return 0;
    } else {
        start as usize
    };
    t.procurar_ultimo(&p, ate).map_or(-1, |i| i as i64)
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

/// `pattern.allMatches(alvo)`: (início, comprimento) de cada casamento, sem
/// sobreposição; depois de um casamento vazio a busca recomeça uma unidade
/// adiante (`_StringAllMatchesIterator.moveNext`, "Empty match, don't start
/// at same location again"). Padrão string vazio casa em `0..=length`.
fn ocorrencias(alvo: &Texto, padrao: &Padrao) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i <= alvo.len() {
        let Some((a, n)) = proxima(alvo, padrao, i) else { break };
        v.push((a, n));
        i = if n == 0 { a + 1 } else { a + n };
    }
    v
}

fn padrao_vazio(p: &Padrao) -> bool {
    matches!(p, Padrao::Texto(t) if t.is_empty())
}

/// `split(padrão)`, o algoritmo do `_StringBase.split` (`string_patch.dart`):
/// padrão string vazio separa em unidades (cada metade de um par de
/// surrogates vira um pedaço); receptor vazio com casamento dá `[]`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split(handle: i64, pat_handle: i64) -> i64 {
    let t = texto_de(handle);
    let p = padrao_de(pat_handle).unwrap_or(Padrao::Texto(Texto::vazio()));
    let n = t.len();
    let mut pedacos: Vec<Texto> = Vec::new();
    if padrao_vazio(&p) {
        pedacos.extend((0..n).map(|i| t.fatia(i, i + 1)));
    } else {
        let casamentos = ocorrencias(&t, &p);
        if !(n == 0 && !casamentos.is_empty()) {
            let mut it = casamentos.into_iter();
            let (mut inicio, mut anterior) = (0, 0);
            loop {
                if inicio == n {
                    pedacos.push(t.fatia(anterior, n));
                    break;
                }
                let Some((a, k)) = it.next() else {
                    pedacos.push(t.fatia(anterior, n));
                    break;
                };
                if a == n {
                    pedacos.push(t.fatia(anterior, n));
                    break;
                }
                let fim = a + k;
                if inicio == fim && fim == anterior {
                    inicio += 1;
                    continue;
                }
                pedacos.push(t.fatia(anterior, a));
                inicio = fim;
                anterior = fim;
            }
        }
    }
    lista_de_pedacos(pedacos.into_iter().map(|x| (None, Value::String(x))).collect())
}

/// `contains(padrão, [start])`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_contains(handle: i64, pat_handle: i64, start: i64) -> u8 {
    (dartforge_string_index_of(handle, pat_handle, start) >= 0) as u8
}

/// Casador rudimentar de `RegExp` (herdado; a decisão 3 troca pelo motor
/// `regress` atrás de uma interface). Reconhece `\d+`, uma classe
/// `[...]` de caracteres literais e texto literal. Devolve o comprimento do
/// casamento em `alvo[i..]`.
fn regexp_casa_em(alvo: &Texto, padrao: &Texto, i: usize) -> Option<usize> {
    let p = padrao.para_string();
    if p == r"\d+" {
        let n = (i..alvo.len()).take_while(|&k| (b'0' as u16..=b'9' as u16).contains(&alvo.unidade(k))).count();
        return (n > 0).then_some(n);
    }
    if p.len() >= 2 && p.starts_with('[') && p.ends_with(']') {
        if i >= alvo.len() {
            return None;
        }
        let conjunto: Vec<u16> = p[1..p.len() - 1].encode_utf16().collect();
        return conjunto.contains(&alvo.unidade(i)).then_some(1);
    }
    alvo.coincide_em(padrao, i).then_some(padrao.len())
}

fn regexp_proxima(alvo: &Texto, padrao: &Texto, desde: usize) -> Option<(usize, usize)> {
    (desde..=alvo.len()).find_map(|i| regexp_casa_em(alvo, padrao, i).map(|n| (i, n)))
}

/// Cria um novo RegExp.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_regexp_new(pat_handle: i64) -> i64 {
    let pat = texto_de_qualquer(pat_handle).unwrap_or_else(Texto::vazio);
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::RegExp(pat)))
}

/// Divide a string em pedaços para splitMapJoin: lista de [is_match (bool), parte (Match ou String)].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_split_map_pieces(target_handle: i64, pat_handle: i64) -> i64 {
    let t = texto_de(target_handle);
    let p = padrao_de(pat_handle).unwrap_or(Padrao::Texto(Texto::vazio()));
    let n = t.len();
    let mut pedacos: Vec<(bool, Texto)> = Vec::new();
    if padrao_vazio(&p) {
        // `_splitMapJoinEmptyString`: um casamento vazio antes de cada
        // caractere, sem separar um par de surrogates.
        pedacos.push((false, Texto::vazio()));
        let mut i = 0;
        while i < n {
            pedacos.push((true, Texto::vazio()));
            let par = i + 1 < n
                && (t.unidade(i) & !0x3FF) == 0xD800
                && (t.unidade(i + 1) & !0x3FF) == 0xDC00;
            let k = if par { 2 } else { 1 };
            pedacos.push((false, t.fatia(i, i + k)));
            i += k;
        }
        pedacos.push((true, Texto::vazio()));
        pedacos.push((false, Texto::vazio()));
    } else {
        let mut inicio = 0;
        for (a, k) in ocorrencias(&t, &p) {
            pedacos.push((false, t.fatia(inicio, a)));
            pedacos.push((true, t.fatia(a, a + k)));
            inicio = a + k;
        }
        pedacos.push((false, t.fatia(inicio, n)));
    }
    lista_de_pedacos(
        pedacos
            .into_iter()
            .map(|(e_casamento, texto)| {
                let val = if e_casamento { Value::Match(texto) } else { Value::String(texto) };
                (Some(TaggedValue::boolean(e_casamento)), val)
            })
            .collect(),
    )
}

/// `replaceAll(de, para)`: padrão vazio insere em toda fronteira de
/// unidade (`'ab'.replaceAll('', '-')` é `-a-b-`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_all(handle: i64, from_handle: i64, to_handle: i64) -> i64 {
    let t = texto_de(handle);
    let para = texto_de_qualquer(to_handle).unwrap_or_else(Texto::vazio);
    let Some(p) = padrao_de(from_handle) else { return handle };
    let casamentos = ocorrencias(&t, &p);
    if casamentos.is_empty() {
        return handle;
    }
    let mut saida = TextoMut::new();
    let mut inicio = 0;
    for (a, k) in casamentos {
        saida.0.extend((inicio..a).map(|i| t.unidade(i)));
        saida.push_texto(&para);
        inicio = a + k;
    }
    saida.0.extend((inicio..t.len()).map(|i| t.unidade(i)));
    alocar_texto(saida.fim())
}

/// `padLeft`/`padRight` do `_StringBase`: `delta = width - length`; sem
/// nada a fazer devolve o próprio receptor; o enchimento é repetido
/// `delta` vezes inteiro (não é cortado).
fn preencher(handle: i64, width: i64, pad_handle: i64, esquerda: bool) -> i64 {
    let t = texto_de(handle);
    let pad = texto_de_qualquer(pad_handle).unwrap_or_else(|| Texto::de_str(" "));
    let delta = width - t.len() as i64;
    if delta <= 0 || pad.is_empty() {
        return handle;
    }
    let mut saida = TextoMut::new();
    if !esquerda {
        saida.push_texto(&t);
    }
    for _ in 0..delta {
        saida.push_texto(&pad);
    }
    if esquerda {
        saida.push_texto(&t);
    }
    alocar_texto(saida.fim())
}

/// Preenche à esquerda até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_left(handle: i64, width: i64, pad_handle: i64) -> i64 {
    preencher(handle, width, pad_handle, true)
}

/// Preenche à direita até a largura indicada.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_pad_right(handle: i64, width: i64, pad_handle: i64) -> i64 {
    preencher(handle, width, pad_handle, false)
}

/// Aloca um novo StringBuffer vazio.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_new() -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate(Value::StringBuffer(Vec::new())))
}

/// `StringBuffer.write(obj)`: acrescenta `"$obj"` (null escreve `null`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_buffer_write(buf_handle: i64, str_handle: i64) {
    HEAP.with(|heap| {
        let mut heap_ref = heap.borrow_mut();
        let parte: Texto = if str_handle == 0 {
            Texto::de_str("null")
        } else {
            match heap_ref.get(str_handle) {
                Value::String(t) => t.clone(),
                _ => describe_texto(&heap_ref, str_handle),
            }
        };
        if let Value::StringBuffer(buf) = heap_ref.get_mut(buf_handle) {
            buf.extend(parte.unidades());
        }
    });
}

/// `toUpperCase`/`toLowerCase` da VM (`String::Transform`, `object.cc`):
/// mapeamento **simples**, um ponto de código para um ponto de código
/// (`CaseMapping`), então `'ß'.toUpperCase()` é `'ß'` (o JS dá `SS`).
/// O mapeamento completo do Rust só é usado quando dá um caractere só.
fn transformar(handle: i64, maiuscula: bool) -> i64 {
    let t = texto_de(handle);
    let mut u = Vec::with_capacity(t.len());
    let mut mudou = false;
    for p in t.pontos() {
        let novo = char::from_u32(p).map_or(p, |c| {
            let mut it: Box<dyn Iterator<Item = char>> =
                if maiuscula { Box::new(c.to_uppercase()) } else { Box::new(c.to_lowercase()) };
            match (it.next(), it.next()) {
                (Some(x), None) => x as u32,
                _ => p,
            }
        });
        mudou |= novo != p;
        crate::heap::empurrar_ponto(&mut u, novo);
    }
    if !mudou {
        return handle;
    }
    alocar_texto(Texto::de_unidades(u))
}

/// Retorna uma nova string convertida para maiúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_upper(handle: i64) -> i64 {
    transformar(handle, true)
}

/// Retorna uma nova string convertida para minúsculas.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_to_lower(handle: i64) -> i64 {
    transformar(handle, false)
}

/// Repete uma string `times` vezes (`'a' * 3`); zero ou negativo dá `''`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    if times <= 0 {
        return alocar_texto(Texto::vazio());
    }
    let t = texto_de(handle);
    let mut saida = TextoMut::new();
    for _ in 0..times {
        saida.push_texto(&t);
    }
    alocar_texto(saida.fim())
}

/// `_StringBase._isTwoByteWhitespace` (`string_patch.dart`), por unidade.
fn e_espaco_dart(u: u16) -> bool {
    if u <= 32 {
        return u == 32 || (9..=13).contains(&u);
    }
    if u < 0x85 {
        return false;
    }
    if u == 0x85 || u == 0xA0 {
        return true;
    }
    if u <= 0x200A {
        u == 0x1680 || 0x2000 <= u
    } else {
        matches!(u, 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF)
    }
}

/// `trim`/`trimLeft`/`trimRight`: nada a tirar devolve o próprio receptor.
fn aparar(handle: i64, esquerda: bool, direita: bool) -> i64 {
    let t = texto_de(handle);
    let n = t.len();
    let mut a = 0;
    if esquerda {
        while a < n && e_espaco_dart(t.unidade(a)) {
            a += 1;
        }
    }
    let mut b = n;
    if direita {
        while b > a && e_espaco_dart(t.unidade(b - 1)) {
            b -= 1;
        }
    }
    if a == 0 && b == n {
        return handle;
    }
    alocar_texto(t.fatia(a, b))
}

/// Remove espaços em branco do início e fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim(handle: i64) -> i64 {
    aparar(handle, true, true)
}

/// Remove espaços em branco do início.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_left(handle: i64) -> i64 {
    aparar(handle, true, false)
}

/// Remove espaços em branco do fim.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_trim_right(handle: i64) -> i64 {
    aparar(handle, false, true)
}

/// `startsWith(padrão, [index])`; índice fora de `0..length` lança.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_starts_with(handle: i64, pat_handle: i64, start: i64) -> u8 {
    let t = texto_de(handle);
    if start < 0 || start as usize > t.len() {
        lancar_range(start, 0, t.len() as i64, "index");
        return 0;
    }
    match padrao_de(pat_handle) {
        Some(Padrao::Texto(p)) => u8::from(t.coincide_em(&p, start as usize)),
        Some(Padrao::RegExp(p)) => u8::from(regexp_casa_em(&t, &p, start as usize).is_some()),
        None => 0,
    }
}

/// Verifica se a string termina com o sufixo dado.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_ends_with(handle: i64, pat_handle: i64) -> u8 {
    let t = texto_de(handle);
    let p = texto_de(pat_handle);
    if p.len() > t.len() {
        return 0;
    }
    u8::from(t.coincide_em(&p, t.len() - p.len()))
}

/// `compareTo` por unidades UTF-16 (-1, 0, 1).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_compare_to(handle: i64, other_handle: i64) -> i64 {
    let a = texto_de(handle);
    let b = texto_de(other_handle);
    match a.comparar(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

/// `replaceFirst(de, para, [startIndex])`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_first(handle: i64, from_handle: i64, to_handle: i64, start: i64) -> i64 {
    let t = texto_de(handle);
    if start < 0 || start as usize > t.len() {
        lancar_range(start, 0, t.len() as i64, "startIndex");
        return 0;
    }
    let para = texto_de_qualquer(to_handle).unwrap_or_else(Texto::vazio);
    let Some(p) = padrao_de(from_handle) else { return handle };
    match proxima(&t, &p, start as usize) {
        Some((i, n)) => {
            let mut saida = TextoMut::new();
            saida.0.extend((0..i).map(|k| t.unidade(k)));
            saida.push_texto(&para);
            saida.0.extend((i + n..t.len()).map(|k| t.unidade(k)));
            alocar_texto(saida.fim())
        }
        None => handle,
    }
}

/// `replaceRange(start, end, replacement)` com o `checkValidRange`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_replace_range(handle: i64, start: i64, end: i64, rep_handle: i64) -> i64 {
    let t = texto_de(handle);
    let Some((a, b)) = faixa_valida(start, end, t.len()) else { return 0 };
    let rep = texto_de_qualquer(rep_handle).unwrap_or_else(Texto::vazio);
    let mut saida = TextoMut::new();
    saida.0.extend((0..a).map(|k| t.unidade(k)));
    saida.push_texto(&rep);
    saida.0.extend((b..t.len()).map(|k| t.unidade(k)));
    alocar_texto(saida.fim())
}

/// O texto aparado de uma string, para os `parse` (sem espaço Dart nas pontas).
fn texto_para_parse(handle: i64) -> String {
    let t = texto_de(handle);
    let n = t.len();
    let mut a = 0;
    while a < n && e_espaco_dart(t.unidade(a)) {
        a += 1;
    }
    let mut b = n;
    while b > a && e_espaco_dart(t.unidade(b - 1)) {
        b -= 1;
    }
    t.fatia(a, b).para_string()
}

/// Converte string para inteiro ou lança FormatException.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_parse(handle: i64) -> i64 {
    if handle == 0 {
        let err = allocate_format_exception("Invalid number: null");
        dartforge_exception_throw(err, 3);
        return 0;
    }
    let text = texto_para_parse(handle);
    match text.parse::<i64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid radix-10 number: {text}"));
            dartforge_exception_throw(err, 3);
            0
        }
    }
}

/// `int.tryParse`: null (0) se falhar; senão o `int` como referência (R3).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_int_try_parse(handle: i64) -> i64 {
    if handle == 0 {
        return 0;
    }
    match texto_para_parse(handle).parse::<i64>() {
        Ok(val) => valor_como_ref(TaggedValue::scalar(val)),
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
    let text = texto_para_parse(handle);
    match text.parse::<f64>() {
        Ok(val) => val,
        Err(_) => {
            let err = allocate_format_exception(&format!("Invalid double: {text}"));
            dartforge_exception_throw(err, 3);
            0.0
        }
    }
}
