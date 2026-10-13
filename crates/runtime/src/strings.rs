// Runtime nativo: auxiliares de string dos fragmentos, o literal do JIT, a
// interpolação e os externs de `String` do lowering "por nome".
//
// As strings são blocos `_OneByteString`/`_TwoByteString` do espaço de objetos
// (`textos.rs`, docs/NATIVO-ESPACO-UNIFICADO.md §2.5): lidas pela vista
// (`Heap::text`, uma `TextoRef` emprestada do bloco, sem cópia) e criadas em
// uma alocação na forma canônica. Todo índice, comprimento e busca daqui é em
// unidades de código, como no `dart:core`; nada passa por `String` do Rust (que
// perderia os surrogates soltos).
//
// Os externs do mecanismo "por nome" (`dartforge_string_*` fora os de
// interpolação e literal) são do caminho sem SDK da fonte e saem com ele (P5,
// §3.7); até lá seguem valendo sobre a representação nova.

/// Aloca uma string com as unidades de `t` (forma canônica). O chamador
/// enraíza o resultado.
fn allocate_string(t: Texto) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_string(t.vista()))
}

/// Aloca uma string a partir de texto que o runtime formatou (UTF-8 válido).
fn allocate_str(s: &str) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().allocate_str(s))
}

/// Empresta a vista da string `handle`, sem copiar. `f` não pode alocar no
/// heap (o empréstimo está aberto); quem aloca o resultado faz isso depois.
/// Um valor que não é string é bug do compilador (N4).
fn with_string<R>(handle: i64, f: impl FnOnce(crate::textos::TextoRef<'_>) -> R) -> R {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.text(handle) {
            Some(t) => f(t),
            None => panic!("bug do compilador: string esperada\nhandle {handle}"),
        }
    })
}

/// A cópia de construção das unidades de uma string (para os algoritmos que
/// alocam no meio). Não string: `None`.
fn string_copy(handle: i64) -> Option<Texto> {
    HEAP.with(|heap| heap.borrow().text(handle).map(crate::textos::TextoRef::to_string_fn))
}

/// A cópia de uma string; um valor que não é string é bug do compilador (N4).
fn copied_string(handle: i64) -> Texto {
    with_string(handle, |t| t.to_string_fn())
}

/// Lança `RangeError.range(value, min, max, name)`, como o
/// `RangeError.checkValidRange`/`checkValueInInterval` do `dart:core`.
fn throw_range_err(value: i64, min: i64, max: i64, name: &str) {
    let n = allocate_str(name);
    let err = with_roots(&[n], || dartforge_range_error_range(value, min, max, n, 0));
    dartforge_exception_throw(err, 3);
}

/// `RangeError.checkValidRange(start, end, length)` do `dart:core`
/// (`errors.dart`): devolve o fim efetivo, ou lança e devolve `None`.
/// `end < 0` é o "ausente" do lowering por nome.
fn valid_range(start: i64, end: i64, len: usize) -> Option<(usize, usize)> {
    let len = len as i64;
    if start < 0 || start > len {
        throw_range_err(start, 0, len, "start");
        return None;
    }
    let end = if end < 0 { len } else { end };
    if start > end || end > len {
        throw_range_err(end, start, len, "end");
        return None;
    }
    Some((start as usize, end as usize))
}

/// O `int` de um elemento de lista (`Value::Int` da forma compacta, `Smi` ou
/// `_Mint`); `None` se não é `int`.
fn value_int(heap: &Heap, v: crate::heap::Value) -> Option<i64> {
    match v {
        crate::heap::Value::Int(x) => Some(x),
        crate::heap::Value::Ref(r) if crate::layout::smi::is_smi(r) => Some(crate::layout::smi::value(r)),
        crate::heap::Value::Ref(r) if crate::layout::is_object(r) => heap.int_of(r),
        _ => None,
    }
}

/// Copia WTF-8 (UTF-8 válido é WTF-8) de uma constante LLVM para o literal
/// canônico (`Heap::string_literal`): o literal do JIT, cujos módulos não têm
/// objetos estáticos (a memória de uma geração é liberada, §2.11).
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
    // O literal já visto por esta thread (isolado): pelo endereço da
    // constante, conferido pelos bytes guardados — outra constante pode
    // ocupar o mesmo endereço depois de uma recarga do JIT. Sem isto, cada
    // avaliação convertia os bytes para UTF-16 e procurava no mapa dos
    // literais do heap.
    let key = (ptr as usize, bytes.len());
    let found_item = LITERALS_BY_ADDRESS.with(|m| m.borrow().get(&key).and_then(|(b, h)| (**b == *bytes).then_some(*h)));
    if let Some(h) = found_item {
        return h;
    }
    let t = Texto::de_wtf8(bytes);
    let h = HEAP.with(|heap| heap.borrow_mut().string_literal(t.vista()));
    LITERALS_BY_ADDRESS.with(|m| m.borrow_mut().insert(key, (bytes.into(), h)));
    h
}

thread_local! {
    /// `dartforge_string_new`: (endereço, comprimento) da constante → os
    /// bytes e o handle do literal (permanente no heap desta thread).
    static LITERALS_BY_ADDRESS: RefCell<crate::hash::HashMap<(usize, usize), (Box<[u8]>, i64)>> =
        RefCell::new(crate::hash::HashMap::default());
}

/// Os pares de dígitos de 00 a 99, para escrever dois dígitos por divisão.
const DECIMAL_PAIRS: &[u8; 200] = b"\
0001020304050607080910111213141516171819\
2021222324252627282930313233343536373839\
4041424344454647484950515253545556575859\
6061626364656667686970717273747576777879\
8081828384858687888990919293949596979899";

/// Os dígitos decimais de `v` (com o sinal) no fim de `buf`: o `int.toString()`
/// da VM, sem a string intermediária. Devolve o índice do primeiro byte.
fn write_decimal(buf: &mut [u8; 20], v: i64) -> usize {
    let mut i = buf.len();
    let mut n = v.unsigned_abs();
    while n >= 100 {
        let pair = (n % 100) as usize * 2;
        n /= 100;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    }
    if n >= 10 {
        let pair = n as usize * 2;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    } else {
        i -= 1;
        buf[i] = b'0' + n as u8;
    }
    if v < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    i
}

/// A interpolação `'a$b c'` com partes `int` sem caixa (`JuntarTextos`,
/// `llvm/textos_ir.rs`): `n` pares (espécie, bits) — espécie 0, o `Ref` de uma
/// string; 1, um `int`, escrito em decimal direto no resultado. Uma medição
/// (comprimento e forma), uma alocação e as cópias das unidades dos blocos, sem
/// cópia intermediária.
///
/// # Safety
/// `part_list` aponta para `2·n` palavras legíveis (a temporária do emissor); as
/// strings estão enraizadas pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_juntar_tipado(part_list: *const i64, n: i64) -> i64 {
    // SAFETY: o contrato acima.
    unsafe { join_typed(part_list, n, false) }
}

/// A interpolação de `dartforge_string_juntar_tipado` sob ARC: entrega um
/// owner do código do resultado.
///
/// Mesmos pares (espécie, bits). Os textos ficam enraizados num quadro
/// proprietário durante a alocação, que pode coletar; o resultado copia as
/// unidades e não retém as partes. O chamador transfere o owner ou chama
/// release exatamente uma vez. Não reentra Dart nem deixa exceção pendente.
///
/// # Safety
/// `part_list` aponta para `2·n` palavras legíveis; cada parte de espécie 0 é
/// uma string viva.
///
/// # Panics
/// `n` negativo, espécie fora de 0/1 ou parte 0 que não é string; aborta na
/// fronteira C.
///
/// ```
/// use dartforge_runtime::abi::*;
/// let seven = dartforge_to_string_i64(7);
/// let part_list = [0, seven, 1, -42];
/// let text = unsafe { dartforge_arc_string_juntar_owned_v1(part_list.as_ptr(), 2) };
/// assert_eq!(dartforge_arc_observar_heap_v1(text), 3);
/// assert_eq!(dartforge_texto_iguais(text, dartforge_to_string_i64(-742)), 0);
/// dartforge_arc_release(text);
/// dartforge_arc_collect();
/// assert_eq!(dartforge_arc_observar_heap_v1(text), 0);
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_arc_string_juntar_owned_v1(part_list: *const i64, n: i64) -> i64 {
    // SAFETY: o contrato acima.
    unsafe { join_typed(part_list, n, true) }
}

/// O corpo das duas junções; `owned` enraíza as partes e entrega o owner.
unsafe fn join_typed(part_list: *const i64, n: i64, owned: bool) -> i64 {
    let n = usize::try_from(n).expect("número de partes inválido");
    // SAFETY: garantido por quem chama.
    let part_list: &[i64] = if n == 0 { &[] } else { unsafe { std::slice::from_raw_parts(part_list, 2 * n) } };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let frame = owned.then(|| {
            assert!(part_list.chunks_exact(2).all(|p| matches!(p[0], 0 | 1)), "espécie de parte inválida");
            let textos: Vec<i64> = part_list.chunks_exact(2).filter(|p| p[0] == 0).map(|p| p[1]).collect();
            let frame = heap.push_owning_frame(textos.len());
            for (k, &t) in textos.iter().enumerate() { heap.set_root(frame, k, t); }
            frame
        });
        let mut buf = [0u8; 20];
        let (mut total, mut one) = (0usize, true);
        for p in part_list.chunks_exact(2) {
            if p[0] == 1 {
                total += buf.len() - write_decimal(&mut buf, p[1]);
            } else {
                let t = heap.text(p[1]).expect("bug do compilador: string esperada na interpolação");
                total += t.len();
                one &= t.fits_in_one_byte();
            }
        }
        let r = heap.new_string(total, !one);
        let mut pos = 0;
        for p in part_list.chunks_exact(2) {
            if p[0] == 1 {
                let i = write_decimal(&mut buf, p[1]);
                heap.write_string(r, pos, crate::textos::TextoRef::Um(&buf[i..]));
                pos += buf.len() - i;
            } else {
                let k = heap.text(p[1]).map_or(0, crate::textos::TextoRef::len);
                heap.copy_string(r, pos, p[1], 0, k);
                pos += k;
            }
        }
        if let Some(frame) = frame {
            heap.retain_code_owner(r);
            heap.pop_frame(frame);
        }
        r
    })
}

/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().join_strings(&[a, b]))
}

/// Lista expansível de inteiros (sem alocação no meio: a forma compacta).
fn int_list(value_list: Vec<i64>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let n = value_list.len();
        let l = heap.new_growable(n, n, crate::listas::Element::Int);
        for (i, v) in value_list.into_iter().enumerate() {
            heap.list_set(l, i, crate::heap::Value::Int(v));
        }
        l
    })
}

/// O padrão de busca: string ou `RegExp` (o programa compilado, cid 16).
enum Pattern {
    Texto(Texto),
    RegExp(i64),
}

fn pattern_of(handle: i64) -> Option<Pattern> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.text(handle) {
            return Some(Pattern::Texto(t.to_string_fn()));
        }
        (crate::layout::is_object(handle) && heap.class(handle) == crate::layout::cid::REGEXP_PROGRAM)
            .then_some(Pattern::RegExp(handle))
    })
}

/// Próxima ocorrência do padrão a partir de `since`: (início, comprimento).
fn next_item(target: &Texto, pattern: &Pattern, since: usize) -> Option<(usize, usize)> {
    match pattern {
        Pattern::Texto(p) => target.search_for(p, since).map(|i| (i, p.len())),
        Pattern::RegExp(p) => {
            let caps = regexp_match_at(*p, &target.para_vec(), since, false)?;
            Some((caps[0] as usize, (caps[1] - caps[0]) as usize))
        }
    }
}

/// `indexOf(padrão, [start])` em unidades; -1 se não houver.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    let t = copied_string(handle);
    let Some(p) = pattern_of(pat_handle) else { return -1 };
    if start < 0 || start as usize > t.len() {
        throw_range_err(start, 0, t.len() as i64, "start");
        return 0;
    }
    next_item(&t, &p, start as usize).map_or(-1, |(i, _)| i as i64)
}

/// Uma parte de uma lista de pedaços: o `bool` antes do texto (a marca de
/// casamento do `splitMapJoin`) e o texto.
type Chunk = (Option<bool>, Texto);

/// Lista expansível com os pedaços, na ordem, alocando cada um já com a lista
/// enraizada (G6: uma coleta no meio vê os pedaços anteriores pela lista).
fn chunk_list(items: Vec<Chunk>) -> i64 {
    let list = HEAP.with(|h| h.borrow_mut().new_growable(0, 0, crate::listas::Element::General));
    with_roots(&[list], || {
        for (before, text) in items {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                if let Some(b) = before {
                    h.list_push(list, crate::heap::Value::Bool(b));
                }
                let x = h.allocate_string(text.vista());
                h.list_push(list, crate::heap::Value::Ref(x));
            });
        }
    });
    list
}

/// `pattern.allMatches(target)`: (início, comprimento) de cada casamento, sem
/// sobreposição; depois de um casamento vazio a busca recomeça uma unidade
/// adiante (`_StringAllMatchesIterator.moveNext`, "Empty match, don't start
/// at same location again"). Padrão string vazio casa em `0..=length`.
fn occurrences(target: &Texto, pattern: &Pattern) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i <= target.len() {
        let Some((a, n)) = next_item(target, pattern, i) else { break };
        v.push((a, n));
        i = if n == 0 { a + 1 } else { a + n };
    }
    v
}

fn empty_pattern(p: &Pattern) -> bool {
    matches!(p, Pattern::Texto(t) if t.is_empty())
}

/// `padLeft`/`padRight` do `_StringBase`: `delta = width - length`; sem
/// nada a fazer devolve o próprio receptor; o enchimento é repetido
/// `delta` vezes inteiro (não é cortado).
fn fill(handle: i64, width: i64, pad_handle: i64, left: bool) -> i64 {
    let t = copied_string(handle);
    let pad = string_copy(pad_handle).unwrap_or_else(|| Texto::de_str(" "));
    let delta = width - t.len() as i64;
    if delta <= 0 || pad.is_empty() {
        return handle;
    }
    let mut out = StringMut::new();
    if !left {
        out.push_string(&t);
    }
    for _ in 0..delta {
        out.push_string(&pad);
    }
    if left {
        out.push_string(&t);
    }
    allocate_string(out.end())
}

/// `toUpperCase`/`toLowerCase` da VM (`String::Transform`, `object.cc`):
/// mapeamento **simples**, um ponto de código para um ponto de código
/// (`CaseMapping`), então `'ß'.toUpperCase()` é `'ß'` (o JS dá `SS`).
/// O mapeamento completo do Rust só é usado quando dá um caractere só. Nada a
/// mudar devolve o próprio receptor.
fn transform(handle: i64, uppercase: bool) -> i64 {
    let new = with_string(handle, |t| {
        // ASCII puro: o caminho de uma passada, sem a tabela Unicode.
        if let crate::textos::TextoRef::Um(b) = t
            && b.is_ascii()
        {
            let changes_flag = if uppercase { b.iter().any(u8::is_ascii_lowercase) } else { b.iter().any(u8::is_ascii_uppercase) };
            if !changes_flag {
                return None;
            }
            let v: Vec<u8> = b.iter().map(|c| if uppercase { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }).collect();
            return Some(Texto::Um(v));
        }
        let mut u = Vec::with_capacity(t.len());
        let mut changed = false;
        for p in t.points() {
            let new = char::from_u32(p).map_or(p, |c| {
                let mut it: Box<dyn Iterator<Item = char>> =
                    if uppercase { Box::new(c.to_uppercase()) } else { Box::new(c.to_lowercase()) };
                match (it.next(), it.next()) {
                    (Some(x), None) => x as u32,
                    _ => p,
                }
            });
            changed |= new != p;
            crate::textos::push_point(&mut u, new);
        }
        changed.then(|| Texto::from_units(u))
    });
    match new {
        Some(t) => allocate_string(t),
        None => handle,
    }
}

/// Repete uma string `times` vezes (`'a' * 3`); zero ou negativo dá `''`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    let times = usize::try_from(times).unwrap_or(0);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (n, one) = heap.text(handle).map_or((0, true), |t| (t.len(), t.fits_in_one_byte()));
        let total = n.checked_mul(times).expect("string grande demais");
        let r = heap.new_string(total, !one);
        for k in 0..times {
            heap.copy_string(r, k * n, handle, 0, n);
        }
        r
    })
}

/// `_StringBase._isTwoByteWhitespace` (`string_patch.dart`), por unidade.
fn is_dart_whitespace(u: u16) -> bool {
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

/// A faixa sem espaço Dart nas pontas pedidas.
fn trimmed_range(t: crate::textos::TextoRef<'_>, left: bool, right: bool) -> (usize, usize) {
    let n = t.len();
    let mut a = 0;
    if left {
        while a < n && is_dart_whitespace(t.unit(a)) {
            a += 1;
        }
    }
    let mut b = n;
    if right {
        while b > a && is_dart_whitespace(t.unit(b - 1)) {
            b -= 1;
        }
    }
    (a, b)
}

/// `trim`/`trimLeft`/`trimRight`: nada a tirar devolve o próprio receptor.
fn trim_free(handle: i64, left: bool, right: bool) -> i64 {
    let (n, (a, b)) = with_string(handle, |t| (t.len(), trimmed_range(t, left, right)));
    if a == 0 && b == n {
        return handle;
    }
    HEAP.with(|heap| heap.borrow_mut().string_slice(handle, a, b))
}

/// O texto aparado de uma string, para os `parse` (sem espaço Dart nas pontas).
fn string_to_parse(handle: i64) -> String {
    with_string(handle, |t| {
        let (a, b) = trimmed_range(t, true, true);
        t.piece(a, b).para_string()
    })
}

#[cfg(test)]
mod tests_owned_join {
    use super::*;

    #[test]
    fn owned_join_roots_parts_copies_units_and_releases_on_release() {
        for arc in [false, true] {
            let previous = HEAP.with(|h| h.replace(Heap::new(true)));
            if arc { HEAP.with(|h| h.borrow_mut().enable_arc()); }
            // As partes perdem a raiz do teste antes da chamada (sem coleta
            // entre as duas): durante a alocação, só a fábrica as protege.
            let frame = HEAP.with(|h| h.borrow_mut().push_owning_frame(2));
            let one = HEAP.with(|h| h.borrow_mut().allocate_str("parte sem owner"));
            HEAP.with(|h| h.borrow_mut().set_root(frame, 0, one));
            let dois = allocate_string(Texto::from_units(vec![0xD800, 0x41]));
            HEAP.with(|h| h.borrow_mut().pop_frame(frame));
            let part_list = [0, one, 1, i64::MIN, 0, dois, 1, 0];
            let r = unsafe { dartforge_arc_string_juntar_owned_v1(part_list.as_ptr(), 4) };
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(r), 3);
            let expected: Vec<u16> = "parte sem owner-9223372036854775808".encode_utf16()
                .chain([0xD800, 0x41, u16::from(b'0')]).collect();
            HEAP.with(|h| {
                let heap = h.borrow();
                let t = heap.text(r).unwrap();
                assert_eq!((0..t.len()).map(|i| t.unit(i)).collect::<Vec<_>>(), expected);
            });
            dartforge_arc_release(r);
            dartforge_arc_collect();
            assert_eq!(dartforge_arc_observar_heap_v1(r), 0);
            let empty = unsafe { dartforge_arc_string_juntar_owned_v1(std::ptr::null(), 0) };
            assert_eq!(dartforge_arc_observar_heap_v1(empty), 3);
            dartforge_arc_release(empty);
            HEAP.with(|h| { h.replace(previous); });
        }
    }
}
