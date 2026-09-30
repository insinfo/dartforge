// Runtime nativo: auxiliares de string dos fragmentos, o literal do JIT, a
// interpolação e os externs de `String` do lowering "por nome".
//
// As strings são blocos `_OneByteString`/`_TwoByteString` do espaço de objetos
// (`textos.rs`, docs/NATIVO-ESPACO-UNIFICADO.md §2.5): lidas pela vista
// (`Heap::texto`, uma `TextoRef` emprestada do bloco, sem cópia) e criadas em
// uma alocação na forma canônica. Todo índice, comprimento e busca daqui é em
// unidades de código, como no `dart:core`; nada passa por `String` do Rust (que
// perderia os surrogates soltos).
//
// Os externs do mecanismo "por nome" (`dartforge_string_*` fora os de
// interpolação e literal) são do caminho sem SDK da fonte e saem com ele (P5,
// §3.7); até lá seguem valendo sobre a representação nova.

/// Aloca uma string com as unidades de `t` (forma canônica). O chamador
/// enraíza o resultado.
fn alocar_texto(t: Texto) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().alocar_texto(t.vista()))
}

/// Aloca uma string a partir de texto que o runtime formatou (UTF-8 válido).
fn alocar_str(s: &str) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().alocar_str(s))
}

/// Empresta a vista da string `handle`, sem copiar. `f` não pode alocar no
/// heap (o empréstimo está aberto); quem aloca o resultado faz isso depois.
/// Um valor que não é string é bug do compilador (N4).
fn com_texto<R>(handle: i64, f: impl FnOnce(crate::textos::TextoRef<'_>) -> R) -> R {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        match heap.texto(handle) {
            Some(t) => f(t),
            None => panic!("bug do compilador: string esperada\nhandle {handle}"),
        }
    })
}

/// A cópia de construção das unidades de uma string (para os algoritmos que
/// alocam no meio). Não string: `None`.
fn copia_de_texto(handle: i64) -> Option<Texto> {
    HEAP.with(|heap| heap.borrow().texto(handle).map(crate::textos::TextoRef::para_texto))
}

/// A cópia de uma string; um valor que não é string é bug do compilador (N4).
fn texto_copiado(handle: i64) -> Texto {
    com_texto(handle, |t| t.para_texto())
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

/// O `int` de um elemento de lista (`Valor::Int` da forma compacta, `Smi` ou
/// `_Mint`); `None` se não é `int`.
fn inteiro_do_valor(heap: &Heap, v: crate::heap::Valor) -> Option<i64> {
    match v {
        crate::heap::Valor::Int(x) => Some(x),
        crate::heap::Valor::Ref(r) if crate::layout::smi::e_smi(r) => Some(crate::layout::smi::valor(r)),
        crate::heap::Valor::Ref(r) if crate::layout::e_objeto(r) => heap.int_de(r),
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
    let chave = (ptr as usize, bytes.len());
    let achado = LITERAIS_POR_ENDERECO.with(|m| m.borrow().get(&chave).and_then(|(b, h)| (**b == *bytes).then_some(*h)));
    if let Some(h) = achado {
        return h;
    }
    let t = Texto::de_wtf8(bytes);
    let h = HEAP.with(|heap| heap.borrow_mut().string_literal(t.vista()));
    LITERAIS_POR_ENDERECO.with(|m| m.borrow_mut().insert(chave, (bytes.into(), h)));
    h
}

thread_local! {
    /// `dartforge_string_new`: (endereço, comprimento) da constante → os
    /// bytes e o handle do literal (permanente no heap desta thread).
    static LITERAIS_POR_ENDERECO: RefCell<crate::hash::HashMap<(usize, usize), (Box<[u8]>, i64)>> =
        RefCell::new(crate::hash::HashMap::default());
}

/// Os pares de dígitos de 00 a 99, para escrever dois dígitos por divisão.
const PARES_DECIMAIS: &[u8; 200] = b"\
0001020304050607080910111213141516171819\
2021222324252627282930313233343536373839\
4041424344454647484950515253545556575859\
6061626364656667686970717273747576777879\
8081828384858687888990919293949596979899";

/// Os dígitos decimais de `v` (com o sinal) no fim de `buf`: o `int.toString()`
/// da VM, sem a string intermediária. Devolve o índice do primeiro byte.
fn escrever_decimal(buf: &mut [u8; 20], v: i64) -> usize {
    let mut i = buf.len();
    let mut n = v.unsigned_abs();
    while n >= 100 {
        let par = (n % 100) as usize * 2;
        n /= 100;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&PARES_DECIMAIS[par..par + 2]);
    }
    if n >= 10 {
        let par = n as usize * 2;
        i -= 2;
        buf[i..i + 2].copy_from_slice(&PARES_DECIMAIS[par..par + 2]);
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
/// `partes` aponta para `2·n` palavras legíveis (a temporária do emissor); as
/// strings estão enraizadas pelo emissor.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dartforge_string_juntar_tipado(partes: *const i64, n: i64) -> i64 {
    let n = usize::try_from(n).expect("número de partes inválido");
    // SAFETY: garantido por quem chama.
    let partes = unsafe { std::slice::from_raw_parts(partes, 2 * n) };
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let mut buf = [0u8; 20];
        let (mut total, mut um) = (0usize, true);
        for p in partes.chunks_exact(2) {
            if p[0] == 1 {
                total += buf.len() - escrever_decimal(&mut buf, p[1]);
            } else {
                let t = heap.texto(p[1]).expect("bug do compilador: string esperada na interpolação");
                total += t.len();
                um &= t.cabe_em_um_byte();
            }
        }
        let r = heap.novo_texto(total, !um);
        let mut pos = 0;
        for p in partes.chunks_exact(2) {
            if p[0] == 1 {
                let i = escrever_decimal(&mut buf, p[1]);
                heap.escrever_texto(r, pos, crate::textos::TextoRef::Um(&buf[i..]));
                pos += buf.len() - i;
            } else {
                let k = heap.texto(p[1]).map_or(0, crate::textos::TextoRef::len);
                heap.copiar_texto(r, pos, p[1], 0, k);
                pos += k;
            }
        }
        r
    })
}

/// Concatena strings não nulas; argumentos devem estar enraizados pelo emissor.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_concat(a: i64, b: i64) -> i64 {
    HEAP.with(|heap| heap.borrow_mut().juntar_textos(&[a, b]))
}

/// Lista expansível de inteiros (sem alocação no meio: a forma compacta).
fn lista_de_inteiros(valores: Vec<i64>) -> i64 {
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let n = valores.len();
        let l = heap.nova_expansivel(n, n, crate::listas::Elemento::Int);
        for (i, v) in valores.into_iter().enumerate() {
            heap.lista_set(l, i, crate::heap::Valor::Int(v));
        }
        l
    })
}

/// O padrão de busca: string ou `RegExp` (o programa compilado, cid 16).
enum Padrao {
    Texto(Texto),
    RegExp(i64),
}

fn padrao_de(handle: i64) -> Option<Padrao> {
    HEAP.with(|heap| {
        let heap = heap.borrow();
        if let Some(t) = heap.texto(handle) {
            return Some(Padrao::Texto(t.para_texto()));
        }
        (crate::layout::e_objeto(handle) && heap.classe(handle) == crate::layout::cid::PROGRAMA_DE_REGEXP)
            .then_some(Padrao::RegExp(handle))
    })
}

/// Próxima ocorrência do padrão a partir de `desde`: (início, comprimento).
fn proxima(alvo: &Texto, padrao: &Padrao, desde: usize) -> Option<(usize, usize)> {
    match padrao {
        Padrao::Texto(p) => alvo.procurar(p, desde).map(|i| (i, p.len())),
        Padrao::RegExp(p) => {
            let caps = regexp_casar_em(*p, &alvo.para_vec(), desde, false)?;
            Some((caps[0] as usize, (caps[1] - caps[0]) as usize))
        }
    }
}

/// `indexOf(padrão, [start])` em unidades; -1 se não houver.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_index_of(handle: i64, pat_handle: i64, start: i64) -> i64 {
    let t = texto_copiado(handle);
    let Some(p) = padrao_de(pat_handle) else { return -1 };
    if start < 0 || start as usize > t.len() {
        lancar_range(start, 0, t.len() as i64, "start");
        return 0;
    }
    proxima(&t, &p, start as usize).map_or(-1, |(i, _)| i as i64)
}

/// Uma parte de uma lista de pedaços: o `bool` antes do texto (a marca de
/// casamento do `splitMapJoin`) e o texto.
type Pedaco = (Option<bool>, Texto);

/// Lista expansível com os pedaços, na ordem, alocando cada um já com a lista
/// enraizada (G6: uma coleta no meio vê os pedaços anteriores pela lista).
fn lista_de_pedacos(itens: Vec<Pedaco>) -> i64 {
    let lista = HEAP.with(|h| h.borrow_mut().nova_expansivel(0, 0, crate::listas::Elemento::Geral));
    com_raizes(&[lista], || {
        for (antes, texto) in itens {
            HEAP.with(|h| {
                let mut h = h.borrow_mut();
                if let Some(b) = antes {
                    h.lista_push(lista, crate::heap::Valor::Bool(b));
                }
                let x = h.alocar_texto(texto.vista());
                h.lista_push(lista, crate::heap::Valor::Ref(x));
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

/// `padLeft`/`padRight` do `_StringBase`: `delta = width - length`; sem
/// nada a fazer devolve o próprio receptor; o enchimento é repetido
/// `delta` vezes inteiro (não é cortado).
fn preencher(handle: i64, width: i64, pad_handle: i64, esquerda: bool) -> i64 {
    let t = texto_copiado(handle);
    let pad = copia_de_texto(pad_handle).unwrap_or_else(|| Texto::de_str(" "));
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

/// `toUpperCase`/`toLowerCase` da VM (`String::Transform`, `object.cc`):
/// mapeamento **simples**, um ponto de código para um ponto de código
/// (`CaseMapping`), então `'ß'.toUpperCase()` é `'ß'` (o JS dá `SS`).
/// O mapeamento completo do Rust só é usado quando dá um caractere só. Nada a
/// mudar devolve o próprio receptor.
fn transformar(handle: i64, maiuscula: bool) -> i64 {
    let novo = com_texto(handle, |t| {
        // ASCII puro: o caminho de uma passada, sem a tabela Unicode.
        if let crate::textos::TextoRef::Um(b) = t
            && b.is_ascii()
        {
            let muda = if maiuscula { b.iter().any(u8::is_ascii_lowercase) } else { b.iter().any(u8::is_ascii_uppercase) };
            if !muda {
                return None;
            }
            let v: Vec<u8> = b.iter().map(|c| if maiuscula { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() }).collect();
            return Some(Texto::Um(v));
        }
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
            crate::textos::empurrar_ponto(&mut u, novo);
        }
        mudou.then(|| Texto::de_unidades(u))
    });
    match novo {
        Some(t) => alocar_texto(t),
        None => handle,
    }
}

/// Repete uma string `times` vezes (`'a' * 3`); zero ou negativo dá `''`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_string_repeat(handle: i64, times: i64) -> i64 {
    let times = usize::try_from(times).unwrap_or(0);
    HEAP.with(|heap| {
        let mut heap = heap.borrow_mut();
        let (n, um) = heap.texto(handle).map_or((0, true), |t| (t.len(), t.cabe_em_um_byte()));
        let total = n.checked_mul(times).expect("string grande demais");
        let r = heap.novo_texto(total, !um);
        for k in 0..times {
            heap.copiar_texto(r, k * n, handle, 0, n);
        }
        r
    })
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

/// A faixa sem espaço Dart nas pontas pedidas.
fn faixa_aparada(t: crate::textos::TextoRef<'_>, esquerda: bool, direita: bool) -> (usize, usize) {
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
    (a, b)
}

/// `trim`/`trimLeft`/`trimRight`: nada a tirar devolve o próprio receptor.
fn aparar(handle: i64, esquerda: bool, direita: bool) -> i64 {
    let (n, (a, b)) = com_texto(handle, |t| (t.len(), faixa_aparada(t, esquerda, direita)));
    if a == 0 && b == n {
        return handle;
    }
    HEAP.with(|heap| heap.borrow_mut().fatia_de_texto(handle, a, b))
}

/// O texto aparado de uma string, para os `parse` (sem espaço Dart nas pontas).
fn texto_para_parse(handle: i64) -> String {
    com_texto(handle, |t| {
        let (a, b) = faixa_aparada(t, true, true);
        t.fatia(a, b).para_string()
    })
}

