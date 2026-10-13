// Runtime nativo: o caminho rápido do `_Map` e do `_Set` padrão
// (`native_sdk/collection/compact_hash.dart`) para chaves `int` e `String`.
//
// A tabela é a do SDK, sem mudança de forma: o `_index` (`Uint32List`, as
// sondas lineares com o padrão de hash nos bits altos) e o `_data` (`_List`
// com chave e valor lado a lado no mapa, só a chave no conjunto), com a
// ordem de inserção e os marcadores de remoção do `compact_hash.dart`. O
// que muda é quem percorre: na VM, o compilador especializa `_hashCode` e
// `_equals` do `_OperatorEqualsAndHashCode` para `int` e `String` e embute
// tudo; aqui cada passo seria uma chamada pelo seletor, com a conferência
// de covariância do `[]=` da lista. Estas funções fazem a sonda, a
// comparação e a gravação de uma vez, com as mesmas contas do Dart
// (`_hashPattern`, `_firstProbe`, `_nextProbe`), então a tabela que deixam
// é bit a bit a que o código Dart deixaria — e o código Dart continua
// valendo para ela (remoção, iteração, `_rehash`).
//
// Só a chave cujo `==` e `hashCode` o runtime conhece sem chamar Dart
// passa: `int` (`Smi` ou `_Mint`: o `hashCode` do `HashIntegerOp` da VM) e
// `String` (o `StringHasher` da VM, guardado no cabeçalho da string; a
// igualdade por unidades). Como `int` e `String` não têm subclasses,
// `chave == entrada` é conhecido para toda entrada, com uma exceção:
// `int == double` compara números (`1 == 1.0`); esse encontro devolve "não
// sei". "Não sei" (outra chave, tabela que precisa crescer, forma
// inesperada) não muda nada, e o Dart refaz a operação pelo caminho do SDK.
//
// Espaço unificado (docs/NATIVO-ESPACO-UNIFICADO.md §4.5, item 4): o `_data`
// é uma `_List` geral (`REFS`: cada entrada um `Ref` — `Smi`, `_Mint`,
// string…) e o `_index` uma `_Uint32List` interna (cid 28). Uma lista de dados
// compacta, imutável na gravação ou de outra classe devolve "não sei".
//
// Sem o `Heap` (docs/NATIVO-PLANO.md §13.2, item 1): os blocos são lidos pelos
// deslocamentos do contrato de `layout`, como o código gerado os lê
// (`@df.lista_*`, `@df.texto_*`), sem `HEAP.with`, `borrow` nem as conferências
// de `live_block`; o cabeçalho de cada objeto diz a classe e a forma antes de
// qualquer leitura do corpo. A gravação no `_data` aplica a barreira de
// elemento do código gerado (`@df.barreira_elemento`). Nada aqui aloca, lança
// ou chama Dart.

use crate::layout::{Header, DESLOCAMENTO_DO_HANDLE};

/// O cabeçalho do objeto `h` (`is_object(h)`; vale também para os estáticos).
///
/// # Safety
/// `h` é o handle de um objeto vivo que o código gerado passou.
#[inline(always)]
unsafe fn raw_header<'a>(h: i64) -> &'a Header {
    // SAFETY: o contrato da função (o bloco começa em `h - 2`).
    unsafe { &*((h - DESLOCAMENTO_DO_HANDLE) as *const Header) }
}

/// O endereço do deslocamento `d` (de `layout::desl`, relativo ao bloco) de `h`.
#[inline(always)]
fn in_block(h: i64, d: usize) -> *mut u8 {
    (h - DESLOCAMENTO_DO_HANDLE + d as i64) as *mut u8
}

/// A palavra de 8 bytes no deslocamento `d` do bloco de `h`.
///
/// # Safety
/// `d` está dentro do bloco de um objeto vivo.
#[inline(always)]
unsafe fn raw_word(h: i64, d: usize) -> i64 {
    // SAFETY: o contrato da função; os corpos são alinhados a 8.
    unsafe { *(in_block(h, d) as *const i64) }
}

/// O `_index` visto pelos bytes: as posições `u32`.
#[derive(Clone, Copy)]
struct Index {
    p: *mut u32,
    size: usize,
}

impl Index {
    /// O `_index`, se `h` é uma `_Uint32List` interna (cid 28, sem `EXTERNAL`).
    #[inline(always)]
    fn de(h: i64) -> Option<Index> {
        use crate::layout::{cid, desl, is_object, flags};
        if !is_object(h) {
            return None;
        }
        // SAFETY: objeto vivo (o código gerado passou o campo `_index`).
        let c = unsafe { raw_header(h) };
        if c.class_id != cid::tipada(crate::tipadas::tipo::UINT32) || c.flags & flags::EXTERNAL != 0 {
            return None;
        }
        // SAFETY: toda lista tipada tem o comprimento em `b+16` e o ponteiro
        // dos dados em `b+24` (§2.5 da especificação).
        let (size, p) = unsafe { (raw_word(h, desl::COMPRIMENTO) as usize, raw_word(h, desl::DADOS) as *mut u32) };
        Some(Index { p, size })
    }

    #[inline(always)]
    fn do_read(self, i: usize) -> u32 {
        debug_assert!(i < self.size);
        // SAFETY: `i < tamanho` (a sonda mascara pelo tamanho).
        unsafe { *self.p.add(i) }
    }

    #[inline(always)]
    fn write_value(self, i: usize, pair: u32) {
        debug_assert!(i < self.size);
        // SAFETY: como em `do_read`; bytes sem referência, sem barreira.
        unsafe { *self.p.add(i) = pair };
    }
}

/// Os elementos de uma `_List`/`_ImmutableList` geral (`REFS`).
#[derive(Clone, Copy)]
struct Data {
    h: i64,
    p: *mut i64,
    len: usize,
}

impl Data {
    /// As entradas de `h`, se é `_List` geral (`write_value`) ou `_List`/`_ImmutableList`
    /// geral (leitura).
    #[inline(always)]
    fn de(h: i64, write_value: bool) -> Option<Data> {
        use crate::layout::{cid, desl, is_object, estado, flags};
        if !is_object(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        let c = unsafe { raw_header(h) };
        let serve = if write_value { c.class_id == cid::LIST && c.estado != estado::PERMANENTE } else { cid::is_fixed_list(c.class_id) };
        if !serve || c.flags & (flags::FORMA | flags::ELEMENTO) != flags::REFS {
            return None;
        }
        // SAFETY: `_List` `REFS`: o comprimento na palavra 0 do corpo, os
        // elementos a partir da palavra 1.
        let len = unsafe { raw_word(h, desl::COMPRIMENTO) } as usize;
        Some(Data { h, p: in_block(h, desl::ELEMENTOS).cast(), len })
    }

    /// Os dados de uma `_GrowableList` geral (a pilha do listener do JSON):
    /// o armazenamento cortado no comprimento.
    #[inline(always)]
    fn from_growable(h: i64) -> Option<Data> {
        use crate::layout::{cid, desl, is_object};
        if !is_object(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        if unsafe { raw_header(h) }.class_id != cid::GROWABLE_LIST {
            return None;
        }
        // SAFETY: `_GrowableList`: comprimento no campo 0, armazenamento no 1.
        let (len, a) = unsafe { (raw_word(h, desl::COMPRIMENTO) as usize, raw_word(h, desl::EXPANSIVEL_DADOS)) };
        let mut d = Data::de(a, false)?;
        if len > d.len {
            return None;
        }
        d.len = len;
        Some(d)
    }

    #[inline(always)]
    fn do_read(self, i: usize) -> i64 {
        debug_assert!(i < self.len);
        // SAFETY: `i < len`.
        unsafe { *self.p.add(i) }
    }

    /// Grava o `Ref` `v` na entrada `i`, com a barreira de elemento do código
    /// gerado: um velho (ou lembrado) que recebe um filho jovem tem o cartão da
    /// entrada sujo, se tem cartões, e o velho é lembrado.
    #[inline(always)]
    fn write_value(self, i: usize, v: i64) {
        use crate::layout::{estado, flags};
        debug_assert!(i < self.len);
        // SAFETY: `i < len`; `_List` gravável (conferida em `de`).
        let old = unsafe { std::mem::replace(&mut *self.p.add(i), v) };
        if arc_enabled() {
            arc_raw_write(self.h, old, v);
        }
        // SAFETY: objeto vivo.
        let c = unsafe { raw_header(self.h) };
        let e = c.estado;
        if (e == estado::VELHO || e == estado::REMEMBERED) && young_child(v) {
            if c.flags & flags::CARTOES != 0 {
                // SAFETY: bloco `REFS` com `CARTOES` de `len` elementos, `i < len`.
                unsafe { crate::space::dirty_card((self.h - DESLOCAMENTO_DO_HANDLE) as *mut Header, self.len, i) };
            }
            if e == estado::VELHO {
                dartforge_lembrar(self.h);
            }
        }
    }
}

/// O `Ref` gravado pede a barreira (`@df.young_child`)? Só um objeto jovem.
#[inline(always)]
fn young_child(v: i64) -> bool {
    // SAFETY: objeto vivo.
    crate::layout::is_object(v) && unsafe { raw_header(v) }.estado == crate::layout::estado::JOVEM
}

/// Uma chave que o runtime compara e espalha sozinho.
#[derive(Clone, Copy)]
enum HashKey {
    Int(i64),
    /// O handle da string.
    Texto(i64),
}

/// O que o `==` da chave diz de uma entrada.
enum Comparison {
    Equal,
    Different,
    Unknown,
}

/// O resultado da sonda.
enum Probe {
    /// A entrada `d` de `_data` (a da chave).
    Found(usize),
    /// A posição do `_index` onde a chave entraria e o padrão de hash.
    Missing { point: usize, pattern: u32 },
}

/// O `int` de um `Smi` ou `_Mint`.
#[inline(always)]
fn integer(k: i64) -> Option<i64> {
    use crate::layout::{cid, desl, is_object, smi};
    if smi::is_smi(k) {
        return Some(smi::value(k));
    }
    // SAFETY: objeto vivo; a caixa `_Mint` tem o valor em `b+16`.
    if is_object(k) && unsafe { raw_header(k) }.class_id == cid::MINT {
        return Some(unsafe { raw_word(k, desl::VALOR) });
    }
    None
}

/// As unidades de uma string: de um byte (`_OneByteString`) ou de dois.
#[derive(Clone, Copy)]
enum Units {
    Um(*const u8, usize),
    Dois(*const u16, usize),
}

impl Units {
    /// As unidades de `h`, se é string.
    #[inline(always)]
    fn de(h: i64) -> Option<Units> {
        use crate::layout::{cid, desl, is_object};
        if !is_object(h) {
            return None;
        }
        // SAFETY: objeto vivo.
        let c = unsafe { raw_header(h) }.class_id;
        if !cid::is_string(c) {
            return None;
        }
        // SAFETY: string: o comprimento em `b+16`, as unidades em `b+24`.
        let len = unsafe { raw_word(h, desl::COMPRIMENTO) } as usize;
        let p = in_block(h, desl::UNIDADES);
        Some(if c == cid::ONE_BYTE_STRING { Units::Um(p, len) } else { Units::Dois(p.cast(), len) })
    }

    fn len(self) -> usize {
        match self {
            Units::Um(_, n) | Units::Dois(_, n) => n,
        }
    }

    /// A unidade `i` (`i < len`).
    #[inline(always)]
    fn at(self, i: usize) -> u16 {
        // SAFETY: `i < len` (quem chama confere).
        unsafe {
            match self {
                Units::Um(p, _) => u16::from(*p.add(i)),
                Units::Dois(p, _) => *p.add(i),
            }
        }
    }

    /// As mesmas unidades?
    fn equal(self, other: Units) -> bool {
        if self.len() != other.len() {
            return false;
        }
        // SAFETY: as duas fatias têm `len` unidades do tipo delas.
        unsafe {
            match (self, other) {
                (Units::Um(a, n), Units::Um(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                (Units::Dois(a, n), Units::Dois(b, _)) => std::slice::from_raw_parts(a, n) == std::slice::from_raw_parts(b, n),
                _ => (0..self.len()).all(|i| self.at(i) == other.at(i)),
            }
        }
    }
}

/// O `hashCode` da string `h` (o de `Heap::hash_de_texto`): o do cabeçalho, ou
/// calculado e gravado nele na primeira consulta (menos no estático, que já o
/// traz e mora em memória só de leitura).
#[inline(always)]
fn raw_string_hash(h: i64, u: Units) -> u32 {
    use crate::layout::{desl, estado};
    let p = in_block(h, desl::HASH_DO_TEXTO).cast::<u32>();
    // SAFETY: `map` do cabeçalho de uma string viva; o heap é desta thread.
    let current = unsafe { *p };
    if current != 0 {
        return current;
    }
    let x = match u {
        // SAFETY: as unidades da string.
        Units::Um(q, n) => crate::layout::hash_de_texto(unsafe { std::slice::from_raw_parts(q, n) }.iter().map(|&b| u16::from(b))),
        Units::Dois(q, n) => crate::layout::hash_de_texto(unsafe { std::slice::from_raw_parts(q, n) }.iter().copied()),
    };
    // SAFETY: como acima; o estado é o byte 0 do bloco.
    if unsafe { raw_header(h) }.estado != estado::PERMANENTE {
        unsafe { *p = x };
    }
    x
}

/// A chave e o `hashCode` dela, se o runtime os conhece: `int` (`Smi` ou
/// `_Mint`, o `hashCode` da VM, o mesmo de `_Smi.hashCode` no código gerado)
/// ou string (o hash do cabeçalho).
#[inline(always)]
fn hash_key(k: i64) -> Option<(HashKey, i64)> {
    if let Some(v) = integer(k) {
        return Some((HashKey::Int(v), dartforge_nativo_DartForge_int_hashCode(v)));
    }
    let u = Units::de(k)?;
    Some((HashKey::Texto(k), i64::from(raw_string_hash(k, u))))
}

/// `chave == entrada` (o `==` de `int` ou de `String`), com a entrada `Ref`.
#[inline(always)]
fn compare_key(key: HashKey, input: i64) -> Comparison {
    use crate::layout::{cid, is_object, smi};
    match key {
        HashKey::Int(v) => {
            if smi::is_smi(input) {
                return equal_if(smi::value(input) == v);
            }
            if !is_object(input) {
                return Comparison::Different;
            }
            // SAFETY: objeto vivo (uma entrada do `_data`).
            match unsafe { raw_header(input) }.class_id {
                cid::MINT => equal_if(integer(input) == Some(v)),
                // `1 == 1.0`: o `==` de `num`, que o Dart decide.
                cid::DOUBLE => Comparison::Unknown,
                _ => Comparison::Different,
            }
        }
        HashKey::Texto(h) => {
            if input == h {
                return Comparison::Equal;
            }
            let (Some(a), Some(b)) = (Units::de(h), Units::de(input)) else {
                return Comparison::Different;
            };
            // Os hashes já calculados diferem: unidades diferentes.
            // SAFETY: `map` de duas strings vivas, só lido.
            let (ha, hb) = unsafe {
                (
                    *in_block(h, crate::layout::desl::HASH_DO_TEXTO).cast::<u32>(),
                    *in_block(input, crate::layout::desl::HASH_DO_TEXTO).cast::<u32>(),
                )
            };
            if ha != 0 && hb != 0 && ha != hb {
                return Comparison::Different;
            }
            equal_if(a.equal(b))
        }
    }
}

#[inline(always)]
fn equal_if(b: bool) -> Comparison {
    if b { Comparison::Equal } else { Comparison::Different }
}

/// A sonda do `_findValueOrInsertPoint` (mapa, `step` 2) e do `_add`
/// (conjunto, `step` 1), com as contas de `_HashBase`. `None`: a
/// igualdade não é conhecida sem o Dart, ou a tabela tem forma inesperada.
#[inline(always)]
fn probe(ix: Index, data: Data, mask: i64, key: HashKey, hash: i64, step: usize) -> Option<Probe> {
    let size = ix.size;
    if size == 0 || !size.is_power_of_two() {
        return None;
    }
    let half = (size >> 1) as i64;
    let masked = hash & mask;
    // `_hashPattern`: cabe em 32 bits pela escolha de `_hashMask`.
    let pattern = if masked == 0 { half } else { masked.wrapping_mul(half) };
    let pattern = u32::try_from(pattern).ok()?;
    let size_mask = size - 1;
    let maximum = size >> 1;
    // `_firstProbe`.
    let i0 = (hash as usize) & size_mask;
    let mut i = ((i0 << 1) + i0) & size_mask;
    let mut first_removed: Option<usize> = None;
    loop {
        let pair = ix.do_read(i);
        if pair == 0 {
            break;
        }
        if pair == 1 {
            first_removed.get_or_insert(i);
        } else {
            let input = (pattern ^ pair) as usize;
            if input < maximum {
                let d = input * step;
                if d >= data.len {
                    return None;
                }
                match compare_key(key, data.do_read(d)) {
                    Comparison::Equal => return Some(Probe::Found(d)),
                    Comparison::Different => {}
                    Comparison::Unknown => return None,
                }
            }
        }
        i = (i + 1) & size_mask;
    }
    Some(Probe::Missing { point: first_removed.unwrap_or(i), pattern })
}

/// `mapa[chave] = valor` sobre a tabela já resolvida: o novo `_usedData` (o
/// mesmo, se a chave já estava), ou -1 quando o Dart tem de fazer a operação
/// (chave de outro tipo, `int` diante de `double`, tabela cheia: o `_rehash`
/// é do Dart).
#[inline(always)]
fn map_write(ix: Index, data: Data, mask: i64, used_count: i64, key: i64, value: i64) -> i64 {
    let Some((k, hash)) = hash_key(key) else { return -1 };
    match probe(ix, data, mask, k, hash, 2) {
        Some(Probe::Found(d)) => {
            if d + 1 >= data.len {
                return -1;
            }
            data.write_value(d + 1, value);
            used_count
        }
        Some(Probe::Missing { point, pattern }) => {
            if used_count < 0 || used_count as usize + 1 >= data.len {
                return -1;
            }
            let u = used_count as usize;
            ix.write_value(point, pattern | (u >> 1) as u32);
            data.write_value(u, key);
            data.write_value(u + 1, value);
            used_count + 2
        }
        None => -1,
    }
}

/// `_Set.add(key)` sobre a tabela já resolvida: o novo `_usedData` (o mesmo,
/// se a chave já estava), ou -1.
#[inline(always)]
fn set_add(ix: Index, data: Data, mask: i64, used_count: i64, key: i64) -> i64 {
    let Some((k, hash)) = hash_key(key) else { return -1 };
    match probe(ix, data, mask, k, hash, 1) {
        // O conjunto guarda a chave que já estava.
        Some(Probe::Found(_)) => used_count,
        Some(Probe::Missing { point, pattern }) => {
            if used_count < 0 || used_count as usize >= data.len {
                return -1;
            }
            let u = used_count as usize;
            ix.write_value(point, pattern | u as u32);
            data.write_value(u, key);
            used_count + 1
        }
        None => -1,
    }
}

/// `_Map[chave] = valor` para chave `int`/`String`: devolve o novo
/// `_usedData` (o mesmo, se a chave já estava), ou -1 quando o Dart tem de
/// fazer a operação (chave de outro tipo, `int` diante de `double`, tabela
/// cheia: o `_rehash` é do Dart).
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_gravar(ix: i64, data: i64, mask: i64, used_count: i64, key: i64, value: i64) -> i64 {
    match (Index::de(ix), Data::de(data, true)) {
        (Some(i), Some(d)) => map_write(i, d, mask, used_count, key, value),
        _ => -1,
    }
}

/// `_Map._getValueOrData(key)` para chave `int`/`String`: o valor, ou
/// `data` (o `_data`) quando a chave falta — o protocolo do SDK —, ou
/// `ix` (nunca um valor do mapa) quando o Dart tem de procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_buscar(ix: i64, data: i64, mask: i64, key: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Index::de(ix), Data::de(data, false), hash_key(key)) else {
        return ix;
    };
    match probe(ind, el, mask, k, hash, 2) {
        Some(Probe::Found(d)) if d + 1 < el.len => el.do_read(d + 1),
        Some(Probe::Missing { .. }) => data,
        _ => ix,
    }
}

/// `_Set.add(key)` para chave `int`/`String`: o novo `_usedData` (o
/// mesmo, se a chave já estava), ou -1 quando o Dart tem de fazer a
/// operação.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_adicionar(ix: i64, data: i64, mask: i64, used_count: i64, key: i64) -> i64 {
    match (Index::de(ix), Data::de(data, true)) {
        (Some(i), Some(d)) => set_add(i, d, mask, used_count, key),
        _ => -1,
    }
}

/// `_Set._getKeyOrData(key)` para chave `int`/`String`: a chave que está
/// no conjunto, ou `data` quando falta, ou `ix` quando o Dart tem de
/// procurar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_buscar(ix: i64, data: i64, mask: i64, key: i64) -> i64 {
    let (Some(ind), Some(el), Some((k, hash))) = (Index::de(ix), Data::de(data, false), hash_key(key)) else {
        return ix;
    };
    match probe(ind, el, mask, k, hash, 1) {
        Some(Probe::Found(d)) => el.do_read(d),
        Some(Probe::Missing { .. }) => data,
        None => ix,
    }
}

/// O resultado de um lote: a posição seguinte em `pair_list` (onde o lote parou) nos
/// 32 bits altos e o novo `_usedData` nos baixos.
#[inline(always)]
fn batch(cur_position: usize, used_count: i64) -> i64 {
    ((cur_position as i64) << 32) | (used_count & 0xFFFF_FFFF)
}

/// As entradas de `pair_list`: uma `_List`/`_ImmutableList` geral (o `_data` velho
/// do `_rehash`) ou uma `_GrowableList` geral (a pilha do listener do JSON).
#[inline(always)]
fn entries(pair_list: i64) -> Option<Data> {
    Data::de(pair_list, false).or_else(|| Data::from_growable(pair_list))
}

/// O lote do `_Map` (docs/NATIVO-PLANO.md §13.2, itens 2 e 3): `mapa[k] = v`
/// para cada par `(pair_list[i], pair_list[i + 1])`, `i` de `de` até `until` de 2 em 2,
/// na ordem, como o `_set` faria. Com `skip_removed`, o par cuja chave é a
/// própria lista (`_HashBase._isDeleted` do `_data` velho) é pulado. Para no
/// primeiro par que o runtime não sabe gravar (chave de outro tipo, `int`
/// diante de `double`, tabela cheia) e devolve [`batch`]: o Dart grava esse par
/// e chama de novo a partir do seguinte.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_mapa_preencher(ix: i64, data: i64, mask: i64, used_count: i64, pair_list: i64, de: i64, until: i64, skip_removed: u8) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Index::de(ix), Data::de(data, true), entries(pair_list)) else {
        return batch(de.max(0) as usize, used_count);
    };
    let (mut i, until) = (de.max(0) as usize, (until.max(0) as usize).min(p.len));
    let mut u = used_count;
    while i + 1 < until {
        let k = p.do_read(i);
        if !(skip_removed != 0 && k == pair_list) {
            let r = map_write(ind, d, mask, u, k, p.do_read(i + 1));
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 2;
    }
    batch(i, u)
}

/// O lote do `_Set` (o `_init` do `_rehash`): `add(olds_set[i])` para `i` de `de`
/// até `until`, pulando as removidas (a chave que é a própria lista). Para no
/// primeiro elemento que o runtime não sabe inserir e devolve [`batch`].
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_conjunto_preencher(ix: i64, data: i64, mask: i64, used_count: i64, olds_set: i64, de: i64, until: i64) -> i64 {
    let (Some(ind), Some(d), Some(p)) = (Index::de(ix), Data::de(data, true), entries(olds_set)) else {
        return batch(de.max(0) as usize, used_count);
    };
    let (mut i, until) = (de.max(0) as usize, (until.max(0) as usize).min(p.len));
    let mut u = used_count;
    while i < until {
        let k = p.do_read(i);
        if k != olds_set {
            let r = set_add(ind, d, mask, u, k);
            if r < 0 {
                break;
            }
            u = r;
        }
        i += 1;
    }
    batch(i, u)
}

/// `_dfNovosDados(n)` (`compact_hash.dart`): o `_data` novo de uma tabela, uma
/// `_List` geral de `n` nulls sem tipo reificado (o `new List.filled(n, null)`
/// do SDK, sem avaliar o `List<Object?>`: a lista nunca sai da tabela). Pode
/// coletar.
#[unsafe(no_mangle)]
pub extern "C" fn dartforge_nativo_DartForge_hash_novos_dados(n: i64) -> i64 {
    let n = usize::try_from(n).unwrap_or(0);
    HEAP.with(|heap| heap.borrow_mut().new_list(crate::layout::cid::LIST, n, crate::lists::Element::General))
}
