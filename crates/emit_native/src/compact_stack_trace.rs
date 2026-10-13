//! A forma compacta da tabela do rastro no formato da VM
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14, item 1): o conversor da
//! seção `dfpcl` de um objeto COFF ou ELF, que troca as entradas de 12 bytes
//! (duas relocações cada) pelo **DFPC v1**, reescrevendo o objeto no lugar,
//! como o conversor do mapa de pilha (`gcmap.rs`).
//!
//! ```text
//! cabeçalho (20 bytes)
//!   "DFPC"; u8 versão = 1; u8 forma (1 = ELF, 2 = COFF); u16 0
//!   u32 tamanho total do blob (múltiplo de 4); u32 n_funcoes; u32 n_registros
//! índice: n_funcoes × { i32 endereço da função; u32 início no fluxo }
//! registros: n_registros × i32 endereço do registro
//!   cada endereço é relocado relativo ao próprio campo: no ELF, `field + v`
//!   (`R_X86_64_PC32`, `R_AARCH64_PREL32`); no COFF, `field + 4 + v`
//!   (`IMAGE_REL_AMD64_REL32`, relativo ao byte seguinte)
//! fluxo (varints LEB128 sem sinal), por função:
//!   n; n × { delta do rótulo desde o anterior (o primeiro, desde o começo
//!            da função); registro << 2 | espécie; palavra }
//! zeros até múltiplo de 4
//! ```
//!
//! As espécies e as palavras são as da forma de 12 bytes (`llvm/stack_trace.rs`);
//! as entradas do mesmo rótulo ficam na ordem da seção (a cadeia de quadros
//! embutidos). Uma entrada de espécie 3 (o elo) não tem registro (0). O
//! runtime lê as duas formas na mesma seção, pela assinatura `DFPC`
//! (`RT/stack_trace.rs`). O Mach-O não é convertido (como o mapa).
//!
//! Escrito sem compilar nem executar (2026-10-05).

use crate::gcmap::{read_varint, u16_at, u32_at, u64_at, varint};

const MACHINE_X86_64: u16 = 0x8664;
const COFF_REL32: u16 = 0x0004;
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;
/// O tipo de símbolo de função do COFF (`DTYPE_FUNCTION << 4`).
const COFF_FUNCTION: u16 = 0x20;
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
const R_X86_64_PC32: u32 = 2;
const R_AARCH64_PREL32: u32 = 261;
const STT_FUNC: u8 = 2;

const SHAPE_ELF: u8 = 1;
const SHAPE_COFF: u8 = 2;

/// Uma entrada lida: a função do rótulo (o símbolo) e o deslocamento nela,
/// o registro (símbolo, adendo), a espécie e a palavra.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    function: u32,
    offset: u64,
    record: Option<(u32, i64)>,
    kind: u32,
    word: u32,
}

/// Uma relocação a escrever: a posição no blob, o símbolo e o adendo.
type Relocation = (u32, u32, i64);

/// Codifica as entradas. Devolve o blob (com os adendos já no lugar quando
/// `addendum_in_place`, a forma do COFF) e as relocações: uma por função, na
/// ordem do índice, e uma por registro.
fn encode(entries: &[Entry], shape: u8, addendum_in_place: bool) -> (Vec<u8>, Vec<Relocation>) {
    // As funções na ordem da primeira aparição; as entradas de cada uma pelo
    // deslocamento, estável (a ordem da seção nos empates).
    let mut functions: Vec<u32> = Vec::new();
    let mut by_function: std::collections::HashMap<u32, Vec<&Entry>> = std::collections::HashMap::new();
    for e in entries {
        if !by_function.contains_key(&e.function) {
            functions.push(e.function);
        }
        by_function.entry(e.function).or_default().push(e);
    }
    let mut records: Vec<(u32, i64)> = Vec::new();
    let mut record_index: std::collections::HashMap<(u32, i64), usize> = std::collections::HashMap::new();
    for e in entries {
        if let Some(r) = e.record
            && !record_index.contains_key(&r)
        {
            record_index.insert(r, records.len());
            records.push(r);
        }
    }
    let nf = functions.len();
    let nr = records.len();
    let index_start = 20;
    let registrations_start = index_start + 8 * nf;
    let flow_start = registrations_start + 4 * nr;
    let mut flow: Vec<u8> = Vec::new();
    let mut starts = Vec::with_capacity(nf);
    for f in &functions {
        starts.push(flow_start + flow.len());
        let mut list = by_function[f].clone();
        list.sort_by_key(|e| e.offset);
        varint(&mut flow, list.len() as u64);
        let mut previous = 0u64;
        for e in list {
            varint(&mut flow, e.offset - previous);
            previous = e.offset;
            let r = e.record.map_or(0, |r| record_index[&r]) as u64;
            varint(&mut flow, r << 2 | u64::from(e.kind & 3));
            varint(&mut flow, u64::from(e.word));
        }
    }
    let mut blob = Vec::with_capacity(flow_start + flow.len() + 4);
    blob.extend_from_slice(b"DFPC");
    blob.push(1);
    blob.push(shape);
    blob.extend_from_slice(&0u16.to_le_bytes());
    blob.extend_from_slice(&0u32.to_le_bytes());
    blob.extend_from_slice(&(nf as u32).to_le_bytes());
    blob.extend_from_slice(&(nr as u32).to_le_bytes());
    let mut relocations = Vec::with_capacity(nf + nr);
    for (k, f) in functions.iter().enumerate() {
        relocations.push(((index_start + 8 * k) as u32, *f, 0));
        blob.extend_from_slice(&0i32.to_le_bytes());
        blob.extend_from_slice(&(starts[k] as u32).to_le_bytes());
    }
    for (k, (s, a)) in records.iter().enumerate() {
        relocations.push(((registrations_start + 4 * k) as u32, *s, *a));
        let in_place = if addendum_in_place { *a as i32 } else { 0 };
        blob.extend_from_slice(&in_place.to_le_bytes());
    }
    blob.extend_from_slice(&flow);
    while blob.len() % 4 != 0 {
        blob.push(0);
    }
    let total = blob.len() as u32;
    blob[8..12].copy_from_slice(&total.to_le_bytes());
    (blob, relocations)
}

/// Decodifica o fluxo de um blob (a ida e volta): por função do índice, as
/// entradas `(offset, record, espécie, word)`.
fn decode(blob: &[u8]) -> Result<Vec<Vec<(u64, usize, u32, u32)>>, String> {
    if blob.get(..4) != Some(b"DFPC") || blob.get(4) != Some(&1) {
        return Err("blob DFPC sem a assinatura".to_string());
    }
    let nf = u32_at(blob, 12)? as usize;
    let mut exit = Vec::with_capacity(nf);
    for k in 0..nf {
        let mut p = u32_at(blob, 20 + 8 * k + 4)? as usize;
        let n = read_varint(blob, &mut p)?;
        let mut list = Vec::new();
        let mut offset = 0u64;
        for _ in 0..n {
            offset += read_varint(blob, &mut p)?;
            let rk = read_varint(blob, &mut p)?;
            let word = read_varint(blob, &mut p)? as u32;
            list.push((offset, (rk >> 2) as usize, (rk & 3) as u32, word));
        }
        exit.push(list);
    }
    Ok(exit)
}

/// A ida e volta: o que o runtime vai ler é o que a seção dizia.
fn check_round_trip(blob: &[u8], entries: &[Entry], relocations: &[Relocation]) -> Result<(), String> {
    let back = decode(blob)?;
    let nf = u32_at(blob, 12)? as usize;
    let mut counted = 0;
    for (k, list) in back.iter().enumerate() {
        let f = relocations[k].1;
        let mut expected_set: Vec<&Entry> = entries.iter().filter(|e| e.function == f).collect();
        expected_set.sort_by_key(|e| e.offset);
        if expected_set.len() != list.len() {
            return Err("a tabela do rastro compacta não reproduz a original (ida e volta)".to_string());
        }
        for (e, &(d, r, kind, word)) in expected_set.iter().zip(list) {
            let record = e.record.map(|_| relocations[nf + r].1);
            let same_record = match (e.record, record) {
                (Some((s, a)), Some(_)) => relocations.get(nf + r).is_some_and(|x| x.1 == s && x.2 == a),
                (None, _) => kind == 3,
                _ => false,
            };
            if e.offset != d || e.kind != kind || e.word != word || !same_record {
                return Err("a tabela do rastro compacta não reproduz a original (ida e volta)".to_string());
            }
        }
        counted += list.len();
    }
    if counted != entries.len() {
        return Err("a tabela do rastro compacta perdeu entradas".to_string());
    }
    Ok(())
}

/// Um símbolo de função: `(seção, value, size)` (tamanho 0: até o
/// próximo).
type FunctionSymbol = (u32, u64, u64);

/// O símbolo de função que contém `(section, offset)`: o de maior valor
/// que não passa dele (e, com tamanho, que o cobre).
fn function_in(functions: &[(u32, FunctionSymbol)], section: u32, offset: u64) -> Option<(u32, u64)> {
    functions
        .iter()
        .filter(|(_, (s, v, t))| *s == section && *v <= offset && (*t == 0 || offset < v + t || offset == *v))
        .max_by_key(|(_, (_, v, _))| *v)
        .map(|(i, (_, v, _))| (*i, offset - v))
}

/// Converte, no lugar, a seção `.dfpcl$m` de um objeto COFF x86-64.
/// Devolve `false`, sem mexer, quando não há seção ou ela não tem entradas,
/// ou numa forma que o conversor não reescreve (a original fica, e o runtime
/// a lê do mesmo jeito).
///
/// # Errors
/// Objeto malformado, ou uma entrada que o conversor não entende.
pub fn convert_coff(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 20 || u16_at(object, 0)? != MACHINE_X86_64 {
        return Ok(false);
    }
    let n_sections = u16_at(object, 2)? as usize;
    let symbols = u32_at(object, 8)? as usize;
    let n_symbols = u32_at(object, 12)? as usize;
    let optional = u16_at(object, 16)? as usize;
    // Os símbolos: o valor e a seção de cada um; os de função à parte.
    let mut value_of: Vec<(u32, u64)> = vec![(0, 0); n_symbols];
    let mut functions: Vec<(u32, FunctionSymbol)> = Vec::new();
    let mut i = 0;
    while i < n_symbols {
        let s = symbols + 18 * i;
        let value = u64::from(u32_at(object, s + 8)?);
        let section = u16_at(object, s + 12)? as i16;
        let ty = u16_at(object, s + 14)?;
        let aux = *object.get(s + 17).ok_or_else(|| "objeto COFF truncado".to_string())? as usize;
        if section > 0 {
            value_of[i] = (section as u32, value);
            if ty == COFF_FUNCTION {
                functions.push((i as u32, (section as u32, value, 0)));
            }
        }
        i += 1 + aux;
    }
    for s in 0..n_sections {
        let h = 20 + optional + 40 * s;
        if object.get(h..h + 8) != Some(b".dfpcl$m") {
            continue;
        }
        let size = u32_at(object, h + 16)? as usize;
        let data = u32_at(object, h + 20)? as usize;
        let rel_start = u32_at(object, h + 24)? as usize;
        let n_rel = u16_at(object, h + 32)? as usize;
        let features = u32_at(object, h + 36)?;
        if features & SCN_NRELOC_OVFL != 0 || n_rel == 0 {
            return Ok(false);
        }
        let sec = object.get(data..data + size).ok_or_else(|| "objeto COFF truncado".to_string())?.to_vec();
        if sec.starts_with(b"DFPC") {
            return Ok(false);
        }
        // Pelo deslocamento na seção, o símbolo relocado.
        let mut rel: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_rel {
            let e = rel_start + 10 * k;
            if u16_at(object, e + 8)? != COFF_REL32 {
                return Err("relocação inesperada na seção do rastro (esperava REL32)".to_string());
            }
            rel.insert(u32_at(object, e)?, u32_at(object, e + 4)?);
        }
        let mut entries = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let field = |d: usize| i64::from(i32::from_le_bytes([sec[p + d], sec[p + d + 1], sec[p + d + 2], sec[p + d + 3]]));
            let word = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&s0) = rel.get(&(p as u32)) else {
                if field(0) != 0 || field(4) != 0 || word != 0 {
                    return Err("entrada do rastro sem relocação no rótulo".to_string());
                }
                p += 12;
                continue;
            };
            // `alvo − field` com o adendo no lugar: o alvo é `S + A − 4`.
            let (section, value) = *value_of.get(s0 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
            let label = (value as i64 + field(0) - 4) as u64;
            let Some((function, offset)) = function_in(&functions, section, label) else {
                return Ok(false);
            };
            let (record, kind) = match rel.get(&((p + 4) as u32)) {
                Some(&s1) => {
                    let (_, v1) = *value_of.get(s1 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
                    let alvo = v1 as i64 + field(4) - 4;
                    let kind = (alvo & 3) as u32;
                    (Some((s1, field(4) - 4 - i64::from(kind))), kind)
                }
                None if field(4) == 3 => (None, 3),
                None => return Err("entrada do rastro sem relocação no registro".to_string()),
            };
            entries.push(Entry { function, offset, record, kind, word });
            p += 12;
        }
        if entries.is_empty() {
            return Ok(false);
        }
        let (blob, relocations) = encode(&entries, SHAPE_COFF, true);
        check_round_trip(&blob, &entries, &relocations)?;
        if blob.len() > size || relocations.len() > n_rel {
            return Ok(false);
        }
        object[data..data + blob.len()].copy_from_slice(&blob);
        object[data + blob.len()..data + size].fill(0);
        object[h + 16..h + 20].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        object[h + 32..h + 34].copy_from_slice(&(relocations.len() as u16).to_le_bytes());
        for (k, (position, symbol, _)) in relocations.iter().enumerate() {
            let e = rel_start + 10 * k;
            object[e..e + 4].copy_from_slice(&position.to_le_bytes());
            object[e + 4..e + 8].copy_from_slice(&symbol.to_le_bytes());
            object[e + 8..e + 10].copy_from_slice(&COFF_REL32.to_le_bytes());
        }
        return Ok(true);
    }
    Ok(false)
}

/// Converte, no lugar, cada seção `dfpcl` de um objeto ELF de 64 bits
/// (x86-64 ou aarch64): uma por grupo `comdat`, cada uma com as relocações
/// dela. Devolve se converteu alguma.
///
/// # Errors
/// Objeto malformado, ou uma entrada que o conversor não entende.
pub fn convert_elf(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 64 || &object[..4] != b"\x7fELF" || object[4] != 2 || object[5] != 1 {
        return Ok(false);
    }
    let relative = match u16_at(object, 0x12)? {
        ELF_X86_64 => R_X86_64_PC32,
        ELF_AARCH64 => R_AARCH64_PREL32,
        _ => return Ok(false),
    };
    let table = u64_at(object, 0x28)? as usize;
    let n_sections = u16_at(object, 0x3C)? as usize;
    let names_index = u16_at(object, 0x3E)? as usize;
    if u16_at(object, 0x3A)? != 64 || n_sections == 0 || names_index >= n_sections {
        return Ok(false);
    }
    let header = |i: usize| table + 64 * i;
    let names = u64_at(object, header(names_index) + 24)? as usize;
    let name_of = |o: &[u8], i: usize| -> Result<Vec<u8>, String> {
        let start = names + u32_at(o, header(i))? as usize;
        let rest = o.get(start..).ok_or_else(|| "objeto ELF truncado".to_string())?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(rest[..end].to_vec())
    };
    let mut converted = false;
    for m in 0..n_sections {
        if name_of(object, m)? != b"dfpcl" {
            continue;
        }
        // A `SHT_RELA` (4) da seção, e a tabela de símbolos dela.
        let mut relocations = None;
        for i in 0..n_sections {
            if u32_at(object, header(i) + 4)? == 4 && u32_at(object, header(i) + 44)? as usize == m {
                relocations = Some(i);
            }
        }
        let Some(r) = relocations else { continue };
        let sec_symbols = u32_at(object, header(r) + 40)? as usize;
        let yes_start = u64_at(object, header(sec_symbols) + 24)? as usize;
        let yes_size = u64_at(object, header(sec_symbols) + 32)? as usize;
        let n_yes = yes_size / 24;
        let mut value_of: Vec<(u32, u64)> = Vec::with_capacity(n_yes);
        let mut functions: Vec<(u32, FunctionSymbol)> = Vec::new();
        for k in 0..n_yes {
            let s = yes_start + 24 * k;
            let info = *object.get(s + 4).ok_or_else(|| "objeto ELF truncado".to_string())?;
            let section = u32::from(u16_at(object, s + 6)?);
            let value = u64_at(object, s + 8)?;
            let size = u64_at(object, s + 16)?;
            value_of.push((section, value));
            if info & 0xf == STT_FUNC && section != 0 {
                functions.push((k as u32, (section, value, size)));
            }
        }
        let data = u64_at(object, header(m) + 24)? as usize;
        let size = u64_at(object, header(m) + 32)? as usize;
        let sec = object.get(data..data + size).ok_or_else(|| "objeto ELF truncado".to_string())?.to_vec();
        if sec.starts_with(b"DFPC") {
            continue;
        }
        let rel_start = u64_at(object, header(r) + 24)? as usize;
        let rel_size = u64_at(object, header(r) + 32)? as usize;
        if u64_at(object, header(r) + 56)? != 24 || rel_size % 24 != 0 {
            return Err("relocações da seção do rastro fora do formato `Elf64_Rela`".to_string());
        }
        let mut rel: std::collections::HashMap<u64, (u32, i64)> = std::collections::HashMap::new();
        for k in 0..rel_size / 24 {
            let e = rel_start + 24 * k;
            let info = u64_at(object, e + 8)?;
            if info as u32 != relative {
                return Err(format!("relocação {} na seção do rastro (esperava a relativa de 32 bits)", info as u32));
            }
            rel.insert(u64_at(object, e)?, ((info >> 32) as u32, u64_at(object, e + 16)? as i64));
        }
        if rel.is_empty() {
            continue;
        }
        let mut entries = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let raw_str = |d: usize| i64::from(i32::from_le_bytes([sec[p + d], sec[p + d + 1], sec[p + d + 2], sec[p + d + 3]]));
            let word = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&(s0, a0)) = rel.get(&(p as u64)) else {
                if raw_str(0) != 0 || raw_str(4) != 0 || word != 0 {
                    return Err("entrada do rastro sem relocação no rótulo".to_string());
                }
                p += 12;
                continue;
            };
            // `S + A − P`: o alvo é `S + A`.
            let (section, value) = *value_of.get(s0 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
            let label = (value as i64 + a0) as u64;
            let Some((function, offset)) = function_in(&functions, section, label) else {
                return Err("rótulo do rastro fora de uma função".to_string());
            };
            let (record, kind) = match rel.get(&((p + 4) as u64)) {
                Some(&(s1, a1)) => {
                    let (_, v1) = *value_of.get(s1 as usize).ok_or_else(|| "símbolo fora da tabela".to_string())?;
                    let kind = ((v1 as i64 + a1) & 3) as u32;
                    (Some((s1, a1 - i64::from(kind))), kind)
                }
                None if raw_str(4) == 3 => (None, 3),
                None => return Err("entrada do rastro sem relocação no registro".to_string()),
            };
            entries.push(Entry { function, offset, record, kind, word });
            p += 12;
        }
        if entries.is_empty() {
            continue;
        }
        let (blob, new_ones) = encode(&entries, SHAPE_ELF, false);
        check_round_trip(&blob, &entries, &new_ones)?;
        if blob.len() > size || 24 * new_ones.len() > rel_size {
            continue;
        }
        object[data..data + blob.len()].copy_from_slice(&blob);
        object[data + blob.len()..data + size].fill(0);
        let h = header(m);
        object[h + 32..h + 40].copy_from_slice(&(blob.len() as u64).to_le_bytes());
        for (k, (position, symbol, addendum)) in new_ones.iter().enumerate() {
            let e = rel_start + 24 * k;
            object[e..e + 8].copy_from_slice(&u64::from(*position).to_le_bytes());
            object[e + 8..e + 16].copy_from_slice(&((u64::from(*symbol) << 32) | u64::from(relative)).to_le_bytes());
            object[e + 16..e + 24].copy_from_slice(&addendum.to_le_bytes());
        }
        let hr = header(r);
        object[hr + 32..hr + 40].copy_from_slice(&((24 * new_ones.len()) as u64).to_le_bytes());
        converted = true;
    }
    Ok(converted)
}

/// Se a tabela do rastro do objeto gerado neste hospedeiro passa pelo
/// conversor: no COFF e no ELF, salvo a medida `DARTFORGE_RASTRO_CRU=1`; no
/// Mach-O nunca.
pub fn convert_here() -> bool {
    crate::alvo::sistema() != crate::alvo::Sistema::MacOs && !std::env::var_os("DARTFORGE_RASTRO_CRU").is_some_and(|v| !v.is_empty() && v != "0")
}

/// O conversor do formato do objeto. Devolve se converteu.
///
/// # Errors
/// Os dos conversores.
pub fn convert(object: &mut [u8]) -> Result<bool, String> {
    if object.starts_with(b"\x7fELF") { convert_elf(object) } else { convert_coff(object) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(function: u32, offset: u64, record: Option<(u32, i64)>, kind: u32, word: u32) -> Entry {
        Entry { function, offset, record, kind, word }
    }

    #[test]
    fn blob_round_trips() {
        let entries = vec![
            e(7, 40, Some((3, 16)), 0, 3 << 12 | 5),
            e(7, 12, Some((3, 0)), 0, 2 << 12),
            // A cadeia de um ponto embutido: o mesmo rótulo, na ordem.
            e(7, 40, Some((3, 32)), 0, 9 << 12 | 1),
            e(9, 0, Some((4, 8)), 1, 1 << 12 | 6),
            e(9, 0, Some((4, 8)), 2, 5 << 12 | 9),
            e(9, 0, None, 3, 1 << 2 | 1),
        ];
        let (blob, relocations) = encode(&entries, SHAPE_ELF, false);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(&blob[..4], b"DFPC");
        // Duas funções e quatro registros distintos (o da cadeia embutida,
        // `(3, 32)`, é outro).
        assert_eq!(relocations.len(), 6);
        assert_eq!(relocations[0].1, 7);
        assert_eq!(relocations[1].1, 9);
        assert!(check_round_trip(&blob, &entries, &relocations).is_ok());
        let back = decode(&blob).unwrap();
        // A ordem dos empates é a da seção: 3:5 antes de 9:1.
        assert_eq!(back[0].iter().map(|x| x.3).collect::<Vec<_>>(), vec![2 << 12, 3 << 12 | 5, 9 << 12 | 1]);
        // O COFF leva o adendo no lugar.
        let (coff, _) = encode(&entries, SHAPE_COFF, true);
        let first_record = 20 + 8 * 2;
        assert_eq!(i32::from_le_bytes(coff[first_record..first_record + 4].try_into().unwrap()), 16);
    }

    #[test]
    fn label_function() {
        let functions = vec![(1, (2, 0, 100)), (5, (2, 100, 50)), (6, (3, 0, 10))];
        assert_eq!(function_in(&functions, 2, 120), Some((5, 20)));
        assert_eq!(function_in(&functions, 2, 99), Some((1, 99)));
        assert_eq!(function_in(&functions, 3, 40), None);
    }

    #[test]
    fn object_without_section_stays_as_is() {
        let mut o = vec![0u8; 20];
        o[0] = 0x64;
        o[1] = 0x86;
        let before = o.clone();
        assert_eq!(convert_coff(&mut o), Ok(false));
        assert_eq!(o, before);
        assert_eq!(convert_elf(&mut vec![0u8; 8]), Ok(false));
    }
}
