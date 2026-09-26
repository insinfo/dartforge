// Runtime nativo: as listas tipadas do `typed_data_patch.dart` da VM.
//
// A lista interna (`_Uint8List`, `_Float64List`, …) é um `Value::TypedData`:
// os bytes, no endian do hospedeiro, com a classe e o tipo do elemento; uma
// visão (`_Uint8ArrayView`, `_ByteDataView`, …) é um `Value::TypedView` sobre
// uma lista interna, com deslocamento e comprimento. Os membros
// `vm:recognized` e os natives `TypedData*` da VM que o Dart do patch usa
// são as funções daqui (tabela em `crates/emit_native/src/nativos.rs`):
// fábricas, `length`, `offsetInBytes`, `_typedData`, `_getX`/`_setX`, `[]`,
// `_memMoveN` e `_setClampedRange`.

/// Os tipos de elemento (o `tipo` das formas do heap).
const TIPO_INT8: u8 = 0;
const TIPO_UINT8: u8 = 1;
const TIPO_UINT8_CLAMPED: u8 = 2;
const TIPO_INT16: u8 = 3;
const TIPO_UINT16: u8 = 4;
const TIPO_INT32: u8 = 5;
const TIPO_UINT32: u8 = 6;
const TIPO_INT64: u8 = 7;
const TIPO_UINT64: u8 = 8;
const TIPO_FLOAT32: u8 = 9;
const TIPO_FLOAT64: u8 = 10;
const TIPO_FLOAT32X4: u8 = 11;
const TIPO_INT32X4: u8 = 12;
const TIPO_FLOAT64X2: u8 = 13;
const TIPO_BYTE_DATA: u8 = 14;

/// Bytes por elemento de um tipo.
fn tamanho_do_elemento(tipo: u8) -> usize {
    match tipo {
        TIPO_INT8 | TIPO_UINT8 | TIPO_UINT8_CLAMPED | TIPO_BYTE_DATA => 1,
        TIPO_INT16 | TIPO_UINT16 => 2,
        TIPO_INT32 | TIPO_UINT32 | TIPO_FLOAT32 => 4,
        TIPO_INT64 | TIPO_UINT64 | TIPO_FLOAT64 => 8,
        TIPO_FLOAT32X4 | TIPO_INT32X4 | TIPO_FLOAT64X2 => 16,
        _ => 1,
    }
}

/// A lista interna e o deslocamento em bytes de `h` (lista interna ou
/// visão), e o tipo e o comprimento em elementos dele.
fn resolver(heap: &Heap, h: i64) -> Option<(i64, usize, u8, usize)> {
    match heap.try_get(h)? {
        Value::TypedData { tipo, bytes, .. } => Some((h, 0, *tipo, bytes.len() / tamanho_do_elemento(*tipo))),
        Value::TypedView { tipo, base, deslocamento, comprimento, .. } => Some((*base, *deslocamento, *tipo, *comprimento)),
        _ => None,
    }
}

/// Os bytes da lista interna `h`.
fn bytes_de(heap: &Heap, h: i64) -> &[u8] {
    match heap.get(h) {
        Value::TypedData { bytes, .. } => bytes,
        _ => panic!("bug do compilador: lista tipada interna esperada"),
    }
}

/// Os bytes mutáveis da lista interna `h`.
fn bytes_de_mut(heap: &mut Heap, h: i64) -> &mut Vec<u8> {
    match heap.get_mut(h) {
        Value::TypedData { bytes, .. } => bytes,
        _ => panic!("bug do compilador: lista tipada interna esperada"),
    }
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

/// Aloca uma lista tipada interna zerada de `n` elementos.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_novo(class_id: i64, tipo: i64, n: i64) -> i64 {
    let tipo = tipo as u8;
    let max = maximo_de_elementos(tipo);
    if !(0..=max).contains(&n) {
        lancar_comprimento(n, max);
        return 0;
    }
    let bytes = vec![0u8; n as usize * tamanho_do_elemento(tipo)];
    HEAP.with(|h| h.borrow_mut().allocate(Value::TypedData { class_id, tipo, bytes }))
}

/// [`dartforge_typed_novo`] que registra a tabela de métodos da classe na
/// primeira alocação (como `dartforge_object_new_t`).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_typed_novo_t(class_id: i64, tipo: i64, n: i64, f: extern "C" fn() -> *const i64) -> i64 {
    dartforge_registrar_tabela(class_id, f);
    dartforge_typed_novo(class_id, tipo, n)
}

/// Aloca uma visão sobre `base` (lista interna; uma visão passada aqui é
/// resolvida para a lista interna dela).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_view_nova(class_id: i64, tipo: i64, base: i64, deslocamento: i64, comprimento: i64) -> i64 {
    let resolvido = HEAP.with(|h| resolver(&h.borrow(), base));
    let Some((interna, desloc_base, _, _)) = resolvido else {
        panic!("bug do compilador: visão sobre algo que não é lista tipada");
    };
    let v = Value::TypedView {
        class_id,
        tipo: tipo as u8,
        base: interna,
        deslocamento: desloc_base + deslocamento.max(0) as usize,
        comprimento: comprimento.max(0) as usize,
    };
    com_raizes(&[interna], || HEAP.with(|h| h.borrow_mut().allocate(v)))
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
    HEAP.with(|h| resolver(&h.borrow(), this).map_or(0, |(_, _, _, n)| n as i64))
}

/// `TypedDataView_typedData`: a lista interna de uma visão.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataView_typedData(this: i64) -> i64 {
    HEAP.with(|h| resolver(&h.borrow(), this).map_or(0, |(b, _, _, _)| b))
}

/// `TypedDataView_offsetInBytes`: o deslocamento de uma visão.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_TypedDataView_offsetInBytes(this: i64) -> i64 {
    HEAP.with(|h| resolver(&h.borrow(), this).map_or(0, |(_, d, _, _)| d as i64))
}

/// Lê `N` bytes da lista interna `this` em `off`; fora dos limites lança
/// `RangeError` e devolve `None`.
fn ler<const N: usize>(this: i64, off: i64) -> Option<[u8; N]> {
    let r = HEAP.with(|h| {
        let h = h.borrow();
        let b = bytes_de(&h, this);
        let off = usize::try_from(off).ok()?;
        let fim = off.checked_add(N)?;
        (fim <= b.len()).then(|| b[off..fim].try_into().expect("fatia de N bytes"))
    });
    if r.is_none() {
        let n = HEAP.with(|h| bytes_de(&h.borrow(), this).len() as i64);
        lancar_indice(off, this, n);
    }
    r
}

/// Grava `v` na lista interna `this` em `off`; fora dos limites lança.
fn gravar<const N: usize>(this: i64, off: i64, v: [u8; N]) {
    let ok = HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let b = bytes_de_mut(&mut h, this);
        let Ok(off) = usize::try_from(off) else { return false };
        match off.checked_add(N) {
            Some(fim) if fim <= b.len() => {
                b[off..fim].copy_from_slice(&v);
                true
            }
            _ => false,
        }
    });
    if !ok {
        let n = HEAP.with(|h| bytes_de(&h.borrow(), this).len() as i64);
        lancar_indice(off, this, n);
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
    ler::<8>(this, off).map_or(0, |b| i64::from_ne_bytes(b) as i64)
}

/// `_setInt64`: o valor truncado para o tipo do elemento.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_setInt64(this: i64, off: i64, v: i64) {
    gravar::<8>(this, off, (v as i64).to_ne_bytes());
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

/// O elemento `i` (bits de inteiro) de uma lista ou visão de inteiros, com
/// a checagem de índice da VM.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_indexar_int(this: i64, i: i64) -> i64 {
    let Some((base, desloc, tipo, n)) = HEAP.with(|h| resolver(&h.borrow(), this)) else { return 0 };
    if i < 0 || i as usize >= n {
        // A checagem do `[]` da VM: `RangeError.range(i, 0, n - 1, "length")`.
        lancar_range(i, 0, n as i64 - 1, "length");
        return 0;
    }
    let off = (desloc + i as usize * tamanho_do_elemento(tipo)) as i64;
    match tipo {
        TIPO_INT8 => dartforge_nativo_DartForge_typed_getInt8(base, off),
        TIPO_UINT8 | TIPO_UINT8_CLAMPED | TIPO_BYTE_DATA => dartforge_nativo_DartForge_typed_getUint8(base, off),
        TIPO_INT16 => dartforge_nativo_DartForge_typed_getInt16(base, off),
        TIPO_UINT16 => dartforge_nativo_DartForge_typed_getUint16(base, off),
        TIPO_INT32 => dartforge_nativo_DartForge_typed_getInt32(base, off),
        TIPO_UINT32 => dartforge_nativo_DartForge_typed_getUint32(base, off),
        TIPO_INT64 => dartforge_nativo_DartForge_typed_getInt64(base, off),
        TIPO_UINT64 => dartforge_nativo_DartForge_typed_getUint64(base, off),
        _ => 0,
    }
}

/// O elemento `i` de uma lista ou visão de `double`.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_typed_indexar_double(this: i64, i: i64) -> f64 {
    let Some((base, desloc, tipo, n)) = HEAP.with(|h| resolver(&h.borrow(), this)) else { return 0.0 };
    if i < 0 || i as usize >= n {
        lancar_range(i, 0, n as i64 - 1, "length");
        return 0.0;
    }
    let off = (desloc + i as usize * tamanho_do_elemento(tipo)) as i64;
    match tipo {
        TIPO_FLOAT32 => dartforge_nativo_TypedData_GetFloat32(base, off),
        _ => dartforge_nativo_TypedData_GetFloat64(base, off),
    }
}

/// `_memMoveN(start, count, from, skipCount)`: copia `count` elementos de
/// `N` bytes de `from` (a partir de `skipCount`) para `this` (a partir de
/// `start`), como `memmove` — as duas podem ser a mesma memória.
fn mover(this: i64, start: i64, count: i64, from: i64, skip: i64, n: usize) {
    HEAP.with(|h| {
        let mut h = h.borrow_mut();
        let (Some((db, dd, _, _)), Some((sb, sd, _, _))) = (resolver(&h, this), resolver(&h, from)) else {
            return;
        };
        let bytes = count.max(0) as usize * n;
        let de = sd + skip.max(0) as usize * n;
        let para = dd + start.max(0) as usize * n;
        if db == sb {
            bytes_de_mut(&mut h, db).copy_within(de..de + bytes, para);
        } else {
            let origem: Vec<u8> = bytes_de(&h, sb)[de..de + bytes].to_vec();
            bytes_de_mut(&mut h, db)[para..para + bytes].copy_from_slice(&origem);
        }
    });
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
        let Some((db, dd, _, _)) = HEAP.with(|h| resolver(&h.borrow(), this)) else { return };
        dartforge_nativo_DartForge_typed_setUint8(db, (dd as i64) + start + k, v.clamp(0, 255));
    }
}
