// Runtime nativo: as listas tipadas do `typed_data_patch.dart` da VM.
//
// A representação é a do espaço unificado (`tipadas.rs`,
// docs/NATIVO-ESPACO-UNIFICADO.md §2.5): a lista interna (`_Uint8List`,
// `_Float64List`, …; cids 22–35) é um bloco `BRUTO` com o comprimento, o
// endereço dos bytes e os bytes (ou só o endereço da memória de fora, com
// `EXTERNO`); a visão (`_Uint8ArrayView`, `_ByteDataView`, …; cids 36–65) é
// um objeto de quatro campos com o comprimento e o endereço nos mesmos
// lugares, a lista interna e o deslocamento. Os bytes estão no endian do
// hospedeiro. Os membros `vm:recognized` e os natives `TypedData*` da VM que
// o Dart do patch usa são as funções daqui (tabela em
// `crates/emit_native/src/nativos.rs`): fábricas, `length`, `offsetInBytes`,
// `_typedData`, `_getX`/`_setX`, `[]`, `_memMoveN` e `_setClampedRange`. O
// código gerado lê o comprimento e os dados em linha (`llvm/tipados_ir.rs`),
// e estas funções ficam como caminho lento (com os erros da VM).

/// Os tipos de elemento (o `tipo` das formas do heap; `tipadas::tipo`).
const TIPO_INT8: u8 = crate::tipadas::tipo::INT8;
const TIPO_UINT8: u8 = crate::tipadas::tipo::UINT8;
const TIPO_UINT8_CLAMPED: u8 = crate::tipadas::tipo::UINT8_CLAMPED;
const TIPO_INT16: u8 = crate::tipadas::tipo::INT16;
const TIPO_UINT16: u8 = crate::tipadas::tipo::UINT16;
const TIPO_INT32: u8 = crate::tipadas::tipo::INT32;
const TIPO_UINT32: u8 = crate::tipadas::tipo::UINT32;
const TIPO_INT64: u8 = crate::tipadas::tipo::INT64;
const TIPO_UINT64: u8 = crate::tipadas::tipo::UINT64;
const TIPO_FLOAT32: u8 = crate::tipadas::tipo::FLOAT32;
const TIPO_FLOAT64: u8 = crate::tipadas::tipo::FLOAT64;
const TIPO_FLOAT32X4: u8 = crate::tipadas::tipo::FLOAT32X4;
const TIPO_INT32X4: u8 = crate::tipadas::tipo::INT32X4;
const TIPO_FLOAT64X2: u8 = crate::tipadas::tipo::FLOAT64X2;
const TIPO_BYTE_DATA: u8 = crate::tipadas::tipo::BYTE_DATA;

/// O bit do `tipo` de [`dartforge_view_nova`] que marca a visão não
/// modificável (`_UnmodifiableXArrayView`).
const VISAO_IMUTAVEL: i64 = 0x100;

/// Bytes por elemento de um tipo.
fn tamanho_do_elemento(tipo: u8) -> usize {
    crate::tipadas::tamanho_do_elemento(tipo)
}

/// A vista de `h` (lista interna, externa ou visão), ou `None`.
fn resolver(heap: &Heap, h: i64) -> Option<crate::tipadas::TipadaRef> {
    heap.tipada(h)
}

/// A vista de `h` sem segurar o empréstimo do heap (o endereço e o tamanho
/// valem enquanto a lista vive: o coletor não move).
fn vista_tipada(h: i64) -> Option<crate::tipadas::TipadaRef> {
    HEAP.with(|heap| heap.borrow().tipada(h))
}

/// Lança `RangeError.range(valor, 0, max, "length")` — o que a VM lança
/// para um comprimento inválido na criação de uma lista tipada.
fn lancar_comprimento(valor: i64, max: i64) {
    lancar_range(valor, 0, max, "length");
}

/// O maior comprimento que a VM aceita na criação (o máximo de um `Smi`);
/// além do que o heap comporta, o teto do heap encerra com a mensagem de
/// memória, como a VM com `OutOfMemoryError`.
fn maximo_de_elementos(_tipo: u8) -> i64 {
    (1i64 << 62) - 1
}

/// Aloca uma lista tipada interna zerada de `n` elementos. `class_id` é o
/// cid fixo da lista do `tipo` (`layout::cid::tipada`), que o emissor passa
/// para registrar a tabela de métodos (`_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_novo(class_id: i64, tipo: i64, n: i64) -> i64 {
    let tipo = tipo as u8;
    debug_assert_eq!(class_id, i64::from(crate::layout::cid::tipada(tipo)), "cid da lista tipada fora do contrato");
    let max = maximo_de_elementos(tipo);
    if !(0..=max).contains(&n) {
        lancar_comprimento(n, max);
        return 0;
    }
    HEAP.with(|h| h.borrow_mut().nova_tipada(tipo, n as usize))
}

/// [`dartforge_typed_novo`] que registra a tabela de métodos da classe na
/// primeira alocação (como `dartforge_object_new_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_novo_t(class_id: i64, tipo: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(class_id, f);
    dartforge_typed_novo(class_id, tipo, n)
}

/// Aloca uma visão sobre `base` (lista interna; uma visão passada aqui é
/// resolvida para a lista interna dela). O bit [`VISAO_IMUTAVEL`] de `tipo`
/// marca a visão não modificável; o tipo [`TIPO_BYTE_DATA`] é o
/// `_ByteDataView`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_view_nova(class_id: i64, tipo: i64, base: i64, deslocamento: i64, comprimento: i64) -> i64 {
    let cid = crate::tipadas::cid_da_visao((tipo & 0xFF) as u8, tipo & VISAO_IMUTAVEL != 0);
    debug_assert_eq!(class_id, i64::from(cid), "cid da visão fora do contrato");
    // `nova_visao` enraíza a base durante a alocação.
    HEAP.with(|h| h.borrow_mut().nova_visao(cid, base, deslocamento.max(0) as usize, comprimento.max(0) as usize))
}

/// [`dartforge_view_nova`] que registra a tabela de métodos da classe.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_view_nova_t(
    class_id: i64,
    tipo: i64,
    base: i64,
    deslocamento: i64,
    comprimento: i64,
    f: extern "C" fn() -> *const i64,
) -> i64 {
    dartforge_registrar_tabela(class_id, f);
    dartforge_view_nova(class_id, tipo, base, deslocamento, comprimento)
}

/// `TypedDataBase_length`: o comprimento em elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataBase_length(this: i64) -> i64 {
    vista_tipada(this).map_or(0, |t| t.len as i64)
}

/// O comprimento em elementos de `h` se ela é uma lista tipada do `tipo`
/// dado (interna, externa ou visão) e, para `escrita != 0`, modificável;
/// senão 0. Caminho lento: o código gerado lê o comprimento em linha
/// (`@df.tipada_len`, `llvm/tipados_ir.rs`); fica para o preenchimento
/// ([`dartforge_typed_fill_int`]) e até o corte (§3.7).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_len(h: i64, tipo: i64, escrita: i64) -> i64 {
    heap_sem_emprestimo(|heap| match resolver(heap, h) {
        Some(t) if i64::from(t.tipo) == tipo && (escrita == 0 || !t.imutavel) => t.len as i64,
        _ => 0,
    })
}

/// O endereço do primeiro elemento de `h` se ela é lista tipada, senão 0
/// (sem efeito nem erro). Os bytes não se movem enquanto a lista vive (o
/// coletor não move), nem a memória externa de `asTypedList`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_ptr(h: i64) -> i64 {
    heap_sem_emprestimo(|heap| resolver(heap, h).map_or(0, |t| t.dados as i64))
}

/// `lista.fillRange(inicio, fim, valor)` de uma lista tipada de inteiros
/// (lista interna ou visão modificável do tipo `tipo`, menos a
/// `Uint8ClampedList`), gravando os bits baixos de `valor` como o `[]=` da
/// VM. Devolve 1 se preencheu; 0 se não é o caso simples (lista vazia ou
/// não apta, faixa inválida) — o código gerado então chama o `fillRange`
/// do SDK, que lança o erro da VM. Antes, o `fillRange` do SDK gravava
/// elemento a elemento pelo despacho (~30 ns cada; o `clear` de um quadro
/// de 512×512 custava 26 ms).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_fill_int(h: i64, tipo: i64, inicio: i64, fim: i64, valor: i64) -> i64 {
    let Some((p, a, b)) = faixa_para_preencher(h, tipo, inicio, fim) else { return 0 };
    // SAFETY: `p` é o primeiro elemento de uma lista apta de `n ≥ fim`
    // elementos do tipo `tipo`, sem alinhamento suposto.
    unsafe {
        match tipo as u8 {
            TIPO_INT8 | TIPO_UINT8 => std::ptr::write_bytes(p.add(a), valor as u8, b - a),
            TIPO_INT16 | TIPO_UINT16 => (a..b).for_each(|i| (p as *mut u16).add(i).write_unaligned(valor as u16)),
            TIPO_INT32 | TIPO_UINT32 => (a..b).for_each(|i| (p as *mut u32).add(i).write_unaligned(valor as u32)),
            TIPO_INT64 | TIPO_UINT64 => (a..b).for_each(|i| (p as *mut u64).add(i).write_unaligned(valor as u64)),
            _ => return 0,
        }
    }
    1
}

/// [`dartforge_typed_fill_int`] de `Float32List`/`Float64List`: o `double`
/// arredondado a `float` no `Float32List`, como o `[]=`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_fill_double(h: i64, tipo: i64, inicio: i64, fim: i64, valor: f64) -> i64 {
    let Some((p, a, b)) = faixa_para_preencher(h, tipo, inicio, fim) else { return 0 };
    // SAFETY: como em `dartforge_typed_fill_int`.
    unsafe {
        match tipo as u8 {
            TIPO_FLOAT32 => (a..b).for_each(|i| (p as *mut f32).add(i).write_unaligned(valor as f32)),
            TIPO_FLOAT64 => (a..b).for_each(|i| (p as *mut f64).add(i).write_unaligned(valor)),
            _ => return 0,
        }
    }
    1
}

/// O endereço do primeiro elemento e a faixa `inicio..fim` de uma lista apta
/// à gravação do tipo `tipo`, se a faixa é válida e não vazia.
fn faixa_para_preencher(h: i64, tipo: i64, inicio: i64, fim: i64) -> Option<(*mut u8, usize, usize)> {
    if tipo == i64::from(TIPO_UINT8_CLAMPED) {
        return None;
    }
    let n = dartforge_typed_len(h, tipo, 1);
    if n == 0 || inicio < 0 || fim <= inicio || fim > n {
        return None;
    }
    let p = dartforge_typed_ptr(h) as *mut u8;
    (!p.is_null()).then_some((p, inicio as usize, fim as usize))
}

/// `TypedDataView_typedData`: a lista interna de uma visão.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataView_typedData(this: i64) -> i64 {
    vista_tipada(this).map_or(0, |t| t.base.unwrap_or(this))
}

/// `TypedDataView_offsetInBytes`: o deslocamento de uma visão.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataView_offsetInBytes(this: i64) -> i64 {
    vista_tipada(this).map_or(0, |t| t.deslocamento as i64)
}

/// O endereço dos `N` bytes de `this` em `off`, se cabem; senão lança o
/// `RangeError` da VM (`IndexError` sobre os bytes) e devolve `None`.
fn endereco_de_bytes<const N: usize>(this: i64, off: i64) -> Option<*mut u8> {
    let t = vista_tipada(this);
    let dentro = t.and_then(|t| {
        let off = usize::try_from(off).ok()?;
        let fim = off.checked_add(N)?;
        (fim <= t.bytes).then(|| t.dados.wrapping_add(off))
    });
    if dentro.is_none() {
        lancar_indice(off, this, t.map_or(0, |t| t.bytes as i64));
    }
    dentro
}

/// Lê `N` bytes da lista `this` em `off`; fora dos limites lança
/// `RangeError` e devolve `None`.
fn ler<const N: usize>(this: i64, off: i64) -> Option<[u8; N]> {
    let p = endereco_de_bytes::<N>(this, off)?;
    let mut b = [0u8; N];
    // SAFETY: `p..p+N` está dentro dos bytes da lista viva (conferido).
    unsafe { std::ptr::copy_nonoverlapping(p, b.as_mut_ptr(), N) };
    Some(b)
}

/// Grava `v` na lista `this` em `off`; fora dos limites lança.
fn gravar<const N: usize>(this: i64, off: i64, v: [u8; N]) {
    if let Some(p) = endereco_de_bytes::<N>(this, off) {
        // SAFETY: como em `ler`.
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), p, N) };
    }
}

/// `_getInt8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getInt8(this: i64, off: i64) -> i64 {
    ler::<1>(this, off).map_or(0, |b| i8::from_ne_bytes(b) as i64)
}

/// `_setInt8`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setInt8(this: i64, off: i64, v: i64) {
    gravar::<1>(this, off, (v as i8).to_ne_bytes());
}

/// `_getUint8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getUint8(this: i64, off: i64) -> i64 {
    ler::<1>(this, off).map_or(0, |b| u8::from_ne_bytes(b) as i64)
}

/// `_setUint8`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setUint8(this: i64, off: i64, v: i64) {
    gravar::<1>(this, off, (v as u8).to_ne_bytes());
}

/// `_getInt16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getInt16(this: i64, off: i64) -> i64 {
    ler::<2>(this, off).map_or(0, |b| i16::from_ne_bytes(b) as i64)
}

/// `_setInt16`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setInt16(this: i64, off: i64, v: i64) {
    gravar::<2>(this, off, (v as i16).to_ne_bytes());
}

/// `_getUint16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getUint16(this: i64, off: i64) -> i64 {
    ler::<2>(this, off).map_or(0, |b| u16::from_ne_bytes(b) as i64)
}

/// `_setUint16`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setUint16(this: i64, off: i64, v: i64) {
    gravar::<2>(this, off, (v as u16).to_ne_bytes());
}

/// `_getInt32`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getInt32(this: i64, off: i64) -> i64 {
    ler::<4>(this, off).map_or(0, |b| i32::from_ne_bytes(b) as i64)
}

/// `_setInt32`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setInt32(this: i64, off: i64, v: i64) {
    gravar::<4>(this, off, (v as i32).to_ne_bytes());
}

/// `_getUint32`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getUint32(this: i64, off: i64) -> i64 {
    ler::<4>(this, off).map_or(0, |b| u32::from_ne_bytes(b) as i64)
}

/// `_setUint32`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setUint32(this: i64, off: i64, v: i64) {
    gravar::<4>(this, off, (v as u32).to_ne_bytes());
}

/// `_getInt64`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getInt64(this: i64, off: i64) -> i64 {
    ler::<8>(this, off).map_or(0, i64::from_ne_bytes)
}

/// `_setInt64`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setInt64(this: i64, off: i64, v: i64) {
    gravar::<8>(this, off, v.to_ne_bytes());
}

/// `_getUint64`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_getUint64(this: i64, off: i64) -> i64 {
    ler::<8>(this, off).map_or(0, |b| u64::from_ne_bytes(b) as i64)
}

/// `_setUint64`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setUint64(this: i64, off: i64, v: i64) {
    gravar::<8>(this, off, (v as u64).to_ne_bytes());
}

/// `TypedData_GetFloat32` (`_getFloat32`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_GetFloat32(this: i64, off: i64) -> f64 {
    ler::<4>(this, off).map_or(0.0, |b| f64::from(f32::from_ne_bytes(b)))
}

/// `TypedData_SetFloat32` (`_setFloat32`): o `double` arredondado para `float`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_SetFloat32(this: i64, off: i64, v: f64) {
    gravar::<4>(this, off, (v as f32).to_ne_bytes());
}

/// `TypedData_GetFloat64` (`_getFloat64`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_GetFloat64(this: i64, off: i64) -> f64 {
    ler::<8>(this, off).map_or(0.0, f64::from_ne_bytes)
}

/// `TypedData_SetFloat64` (`_setFloat64`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedData_SetFloat64(this: i64, off: i64, v: f64) {
    gravar::<8>(this, off, v.to_ne_bytes());
}

/// O elemento `i` de uma lista ou visão (`[]` da VM), conferido contra o
/// comprimento: `RangeError.range(i, 0, n - 1, "length")` fora dele. Devolve
/// a vista e o endereço do elemento.
fn elemento(this: i64, i: i64) -> Option<(crate::tipadas::TipadaRef, *mut u8)> {
    let t = vista_tipada(this)?;
    if i < 0 || i as usize >= t.len {
        lancar_range(i, 0, t.len as i64 - 1, "length");
        return None;
    }
    Some((t, t.dados.wrapping_add(i as usize * tamanho_do_elemento(t.tipo))))
}

/// O elemento `i` (bits de inteiro) de uma lista ou visão de inteiros, com
/// a checagem de índice da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_indexar_int(this: i64, i: i64) -> i64 {
    let Some((t, p)) = elemento(this, i) else { return 0 };
    // SAFETY: `p` é o elemento `i < len` da lista viva, sem alinhamento suposto.
    unsafe {
        match t.tipo {
            TIPO_INT8 => i64::from(p.cast::<i8>().read()),
            TIPO_UINT8 | TIPO_UINT8_CLAMPED | TIPO_BYTE_DATA => i64::from(p.read()),
            TIPO_INT16 => i64::from(p.cast::<i16>().read_unaligned()),
            TIPO_UINT16 => i64::from(p.cast::<u16>().read_unaligned()),
            TIPO_INT32 => i64::from(p.cast::<i32>().read_unaligned()),
            TIPO_UINT32 => i64::from(p.cast::<u32>().read_unaligned()),
            TIPO_INT64 | TIPO_UINT64 => p.cast::<i64>().read_unaligned(),
            _ => 0,
        }
    }
}

/// O elemento `i` de uma lista ou visão de `double`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_indexar_double(this: i64, i: i64) -> f64 {
    let Some((t, p)) = elemento(this, i) else { return 0.0 };
    // SAFETY: como em `dartforge_nativo_DartForge_typed_indexar_int`.
    unsafe {
        match t.tipo {
            TIPO_FLOAT32 => f64::from(p.cast::<f32>().read_unaligned()),
            _ => p.cast::<f64>().read_unaligned(),
        }
    }
}

/// `_memMoveN(start, count, from, skipCount)`: copia `count` elementos de
/// `N` bytes de `from` (a partir de `skipCount`) para `this` (a partir de
/// `start`), como `memmove` — as duas podem ser a mesma memória. O Dart do
/// patch confere as faixas antes; aqui só não se passa dos bytes de cada uma.
fn mover(this: i64, start: i64, count: i64, from: i64, skip: i64, n: usize) {
    let (Some(d), Some(s)) = (vista_tipada(this), vista_tipada(from)) else { return };
    let bytes = count.max(0) as usize * n;
    let de = skip.max(0) as usize * n;
    let para = start.max(0) as usize * n;
    if de + bytes > s.bytes || para + bytes > d.bytes || bytes == 0 {
        return;
    }
    // SAFETY: as duas faixas estão dentro das listas vivas; `copy` é o
    // `memmove` (as faixas podem se sobrepor).
    unsafe { std::ptr::copy(s.dados.add(de), d.dados.add(para), bytes) };
}

/// `_memMove1`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_memMove1(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    mover(this, start, count, from, skip, 1);
}

/// `_memMove2`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_memMove2(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    mover(this, start, count, from, skip, 2);
}

/// `_memMove4`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_memMove4(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    mover(this, start, count, from, skip, 4);
}

/// `_memMove8`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_memMove8(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    mover(this, start, count, from, skip, 8);
}

/// `_memMove16`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_memMove16(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    mover(this, start, count, from, skip, 16);
}

/// `TypedDataBase_setClampedRange(start, count, from, skipCount)`: copia
/// para uma lista de bytes com saturação em 0..255 os elementos inteiros de
/// `from` (lista ou visão de inteiros com sinal).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataBase_setClampedRange(this: i64, start: i64, count: i64, from: i64, skip: i64) {
    for k in 0..count.max(0) {
        let v = dartforge_nativo_DartForge_typed_indexar_int(from, skip + k);
        if dartforge_exception_pending() != 0 {
            return;
        }
        // O destino é uma lista de bytes: o elemento `start + k` é o byte.
        let Some(d) = vista_tipada(this) else { return };
        let i = start + k;
        if i < 0 || i as usize >= d.bytes {
            lancar_indice(i, this, d.bytes as i64);
            return;
        }
        // SAFETY: `i < bytes` da lista viva.
        unsafe { d.dados.add(i as usize).write(v.clamp(0, 255) as u8) };
    }
}
