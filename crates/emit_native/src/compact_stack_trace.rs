//! The compact form of the stack trace table in the VM format
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.14, item 1): the converter of the
//! `dfpcl` section of a COFF or ELF object, which replaces the 12-byte entries
//! (two relocations each) with **DFPC v1**, rewriting the object in place,
//! like the stack map converter (`gcmap.rs`).
//!
//! ```text
//! header (20 bytes)
//!   "DFPC"; u8 version = 1; u8 format (1 = ELF, 2 = COFF); u16 0
//!   u32 total size of the blob (multiple of 4); u32 n_functions; u32 n_records
//! index: n_functions × { i32 address of the function; u32 start in the stream }
//! records: n_records × i32 address of the record
//!   each address is relocated relative to the field itself: in ELF, `field + v`
//!   (`R_X86_64_PC32`, `R_AARCH64_PREL32`); in COFF, `field + 4 + v`
//!   (`IMAGE_REL_AMD64_REL32`, relative to the next byte)
//! stream (unsigned LEB128 varints), per function:
//!   n; n × { delta of the label from the previous one (the first, from the start
//!            of the function); record << 2 | kind; word }
//! zeros up to a multiple of 4
//! ```
//!
//! The kinds and the words are those of the 12-byte form (`llvm/stack_trace.rs`);
//! the entries of the same label stay in section order (the chain of inlined
//! frames). An entry of kind 3 (the link) has no record (0). The
//! runtime reads both forms in the same section, by the `DFPC` signature
//! (`RT/stack_trace.rs`). Mach-O is not converted (like the map).
//!
//! Written without compiling or running (2026-10-05).

use crate::gcmap::{read_varint, u16_at, u32_at, u64_at, varint};

const MACHINE_X86_64: u16 = 0x8664;
const COFF_REL32: u16 = 0x0004;
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;
/// The COFF function symbol type (`DTYPE_FUNCTION << 4`).
const COFF_FUNCTION: u16 = 0x20;
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
const R_X86_64_PC32: u32 = 2;
const R_AARCH64_PREL32: u32 = 261;
const STT_FUNC: u8 = 2;

const SHAPE_ELF: u8 = 1;
const SHAPE_COFF: u8 = 2;

/// An entry read: the function of the label (the symbol) and the offset in it,
/// the record (symbol, addend), the kind and the word.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    function: u32,
    offset: u64,
    record: Option<(u32, i64)>,
    kind: u32,
    word: u32,
}

/// A relocation to write: the position in the blob, the symbol and the addend.
type Relocation = (u32, u32, i64);

/// Encodes the entries. Returns the blob (with the addends already in place when
/// `addendum_in_place`, the COFF form) and the relocations: one per function, in
/// index order, and one per record.
fn encode(entries: &[Entry], shape: u8, addendum_in_place: bool) -> (Vec<u8>, Vec<Relocation>) {
    // The functions in order of first appearance; the entries of each by
    // offset, stable (section order on ties).
    let mut functions: Vec<u32> = Vec::new();
    let mut by_function: std::collections::HashMap<u32, Vec<&Entry>> =
        std::collections::HashMap::new();
    for e in entries {
        if !by_function.contains_key(&e.function) {
            functions.push(e.function);
        }
        by_function.entry(e.function).or_default().push(e);
    }
    let mut records: Vec<(u32, i64)> = Vec::new();
    let mut record_index: std::collections::HashMap<(u32, i64), usize> =
        std::collections::HashMap::new();
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

/// A decoded offset, record index, kind and source word.
type DecodedEntry = (u64, usize, u32, u32);

/// Decodes bounded function streams, checking the index and integer ranges.
/// This round-trip reader is separate from the runtime's reader.
fn decode(blob: &[u8]) -> Result<Vec<Vec<DecodedEntry>>, String> {
    if blob.len() < 20 || blob.get(..4) != Some(b"DFPC") || blob[4] != 1 {
        return Err("DFPC blob without the signature".to_string());
    }
    if u32_at(blob, 8)? as usize != blob.len()
        || u16_at(blob, 6)? != 0
        || !matches!(blob[5], SHAPE_ELF | SHAPE_COFF)
    {
        return Err("DFPC blob with invalid size, flags or format".to_string());
    }
    let nf = u32_at(blob, 12)? as usize;
    let nr = u32_at(blob, 16)? as usize;
    if nf > (blob.len() - 20) / 8 {
        return Err("truncated DFPC function index".to_string());
    }
    let index_end = 20 + 8 * nf;
    if nr > (blob.len() - index_end) / 4 {
        return Err("truncated DFPC record index".to_string());
    }
    let base = index_end + 4 * nr;
    let mut starts = Vec::with_capacity(nf);
    for k in 0..nf {
        let start = u32_at(blob, 24 + 8 * k)? as usize;
        if start < base
            || start >= blob.len()
            || starts.last().is_some_and(|&previous| previous >= start)
        {
            return Err("DFPC function stream outside the blob or out of order".to_string());
        }
        starts.push(start);
    }
    let mut output = Vec::with_capacity(nf);
    for (k, &start) in starts.iter().enumerate() {
        let end = starts.get(k + 1).copied().unwrap_or(blob.len());
        let stream = &blob[start..end];
        let mut p = 0;
        let n = read_varint(stream, &mut p)?;
        if n > ((stream.len() - p) / 3) as u64 {
            return Err("truncated DFPC entries".to_string());
        }
        let mut list = Vec::new();
        let mut offset = 0u64;
        for _ in 0..n {
            offset = offset
                .checked_add(read_varint(stream, &mut p)?)
                .ok_or("DFPC offset overflow")?;
            let rk = read_varint(stream, &mut p)?;
            let record = usize::try_from(rk >> 2).map_err(|_| "DFPC record index overflow")?;
            let kind = (rk & 3) as u32;
            if (kind != 3 && record >= nr) || (kind == 3 && record != 0) {
                return Err("DFPC record index outside the table".to_string());
            }
            let word = u32::try_from(read_varint(stream, &mut p)?)
                .map_err(|_| "DFPC source word exceeds u32")?;
            list.push((offset, record, kind, word));
        }
        output.push(list);
    }
    Ok(output)
}

/// The round trip: what the runtime will read is what the section said.
fn check_round_trip(
    blob: &[u8],
    entries: &[Entry],
    relocations: &[Relocation],
) -> Result<(), String> {
    let back = decode(blob)?;
    let nf = u32_at(blob, 12)? as usize;
    let mut counted = 0;
    for (k, list) in back.iter().enumerate() {
        let f = relocations[k].1;
        let mut expected_set: Vec<&Entry> = entries.iter().filter(|e| e.function == f).collect();
        expected_set.sort_by_key(|e| e.offset);
        if expected_set.len() != list.len() {
            return Err(
                "the compact stack trace table does not reproduce the original (round trip)"
                    .to_string(),
            );
        }
        for (e, &(d, r, kind, word)) in expected_set.iter().zip(list) {
            let record = e.record.map(|_| relocations[nf + r].1);
            let same_record = match (e.record, record) {
                (Some((s, a)), Some(_)) => relocations
                    .get(nf + r)
                    .is_some_and(|x| x.1 == s && x.2 == a),
                (None, _) => kind == 3,
                _ => false,
            };
            if e.offset != d || e.kind != kind || e.word != word || !same_record {
                return Err(
                    "the compact stack trace table does not reproduce the original (round trip)"
                        .to_string(),
                );
            }
        }
        counted += list.len();
    }
    if counted != entries.len() {
        return Err("the compact stack trace table lost entries".to_string());
    }
    Ok(())
}

/// A function symbol: `(section, value, size)` (size 0: up to the
/// next one).
type FunctionSymbol = (u32, u64, u64);

/// The function symbol that contains `(section, offset)`: the one with the largest value
/// that does not go past it (and, with a size, that covers it).
fn function_in(
    functions: &[(u32, FunctionSymbol)],
    section: u32,
    offset: u64,
) -> Option<(u32, u64)> {
    functions
        .iter()
        .filter(|(_, (s, v, t))| *s == section && *v <= offset && (*t == 0 || offset - v < *t))
        .max_by_key(|(_, (_, v, _))| *v)
        .map(|(i, (_, v, _))| (*i, offset - v))
}

/// Converts, in place, the `.dfpcl$m` section of an x86-64 COFF object.
/// Returns `false`, without touching anything, when there is no section or it has no entries,
/// or for a form the converter does not rewrite (the original stays, and the runtime
/// reads it the same way).
///
/// # Errors
/// Malformed object, or an entry the converter does not understand.
pub fn convert_coff(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 20 || u16_at(object, 0)? != MACHINE_X86_64 {
        return Ok(false);
    }
    let n_sections = u16_at(object, 2)? as usize;
    let symbols = u32_at(object, 8)? as usize;
    let n_symbols = u32_at(object, 12)? as usize;
    let optional = u16_at(object, 16)? as usize;
    // The symbols: the value and the section of each one; the function ones apart.
    let mut value_of: Vec<(u32, u64)> = vec![(0, 0); n_symbols];
    let mut functions: Vec<(u32, FunctionSymbol)> = Vec::new();
    let mut i = 0;
    while i < n_symbols {
        let s = symbols + 18 * i;
        let value = u64::from(u32_at(object, s + 8)?);
        let section = u16_at(object, s + 12)? as i16;
        let ty = u16_at(object, s + 14)?;
        let aux = *object
            .get(s + 17)
            .ok_or_else(|| "truncated COFF object".to_string())? as usize;
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
        let sec = object
            .get(data..data + size)
            .ok_or_else(|| "truncated COFF object".to_string())?
            .to_vec();
        if sec.starts_with(b"DFPC") {
            return Ok(false);
        }
        // By the offset in the section, the relocated symbol.
        let mut rel: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_rel {
            let e = rel_start + 10 * k;
            if u16_at(object, e + 8)? != COFF_REL32 {
                return Err(
                    "unexpected relocation in the stack trace section (expected REL32)".to_string(),
                );
            }
            rel.insert(u32_at(object, e)?, u32_at(object, e + 4)?);
        }
        let mut entries = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let field = |d: usize| {
                i64::from(i32::from_le_bytes([
                    sec[p + d],
                    sec[p + d + 1],
                    sec[p + d + 2],
                    sec[p + d + 3],
                ]))
            };
            let word = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&s0) = rel.get(&(p as u32)) else {
                if field(0) != 0 || field(4) != 0 || word != 0 {
                    return Err("stack trace entry without a relocation at the label".to_string());
                }
                p += 12;
                continue;
            };
            // `target − field` with the addend in place: the target is `S + A − 4`.
            let (section, value) = *value_of
                .get(s0 as usize)
                .ok_or_else(|| "symbol outside the table".to_string())?;
            let label = (value as i64 + field(0) - 4) as u64;
            let Some((function, offset)) = function_in(&functions, section, label) else {
                return Ok(false);
            };
            let (record, kind) = match rel.get(&((p + 4) as u32)) {
                Some(&s1) => {
                    let (_, v1) = *value_of
                        .get(s1 as usize)
                        .ok_or_else(|| "symbol outside the table".to_string())?;
                    let target = v1 as i64 + field(4) - 4;
                    let kind = (target & 3) as u32;
                    (Some((s1, field(4) - 4 - i64::from(kind))), kind)
                }
                None if field(4) == 3 => (None, 3),
                None => {
                    return Err("stack trace entry without a relocation at the record".to_string());
                }
            };
            entries.push(Entry {
                function,
                offset,
                record,
                kind,
                word,
            });
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

/// Converts, in place, each `dfpcl` section of a 64-bit ELF object
/// (x86-64 or aarch64): one per `comdat` group, each with its own
/// relocations. Returns whether it converted any.
///
/// # Errors
/// Malformed object, or an entry the converter does not understand.
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
        let rest = o
            .get(start..)
            .ok_or_else(|| "objeto ELF truncado".to_string())?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok(rest[..end].to_vec())
    };
    let mut converted = false;
    for m in 0..n_sections {
        if name_of(object, m)? != b"dfpcl" {
            continue;
        }
        // The `SHT_RELA` (4) of the section, and its symbol table.
        let mut relocations = None;
        for i in 0..n_sections {
            if u32_at(object, header(i) + 4)? == 4 && u32_at(object, header(i) + 44)? as usize == m
            {
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
            let info = *object
                .get(s + 4)
                .ok_or_else(|| "objeto ELF truncado".to_string())?;
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
        let sec = object
            .get(data..data + size)
            .ok_or_else(|| "objeto ELF truncado".to_string())?
            .to_vec();
        if sec.starts_with(b"DFPC") {
            continue;
        }
        let rel_start = u64_at(object, header(r) + 24)? as usize;
        let rel_size = u64_at(object, header(r) + 32)? as usize;
        if u64_at(object, header(r) + 56)? != 24 || !rel_size.is_multiple_of(24) {
            return Err(
                "relocations of the stack trace section outside the `Elf64_Rela` format"
                    .to_string(),
            );
        }
        let mut rel: std::collections::HashMap<u64, (u32, i64)> = std::collections::HashMap::new();
        for k in 0..rel_size / 24 {
            let e = rel_start + 24 * k;
            let info = u64_at(object, e + 8)?;
            if info as u32 != relative {
                return Err(format!(
                    "relocation {} in the stack trace section (expected the 32-bit relative one)",
                    info as u32
                ));
            }
            rel.insert(
                u64_at(object, e)?,
                ((info >> 32) as u32, u64_at(object, e + 16)? as i64),
            );
        }
        if rel.is_empty() {
            continue;
        }
        let mut entries = Vec::new();
        let mut p = 0usize;
        while p + 12 <= sec.len() {
            let raw_str = |d: usize| {
                i64::from(i32::from_le_bytes([
                    sec[p + d],
                    sec[p + d + 1],
                    sec[p + d + 2],
                    sec[p + d + 3],
                ]))
            };
            let word = u32::from_le_bytes([sec[p + 8], sec[p + 9], sec[p + 10], sec[p + 11]]);
            let Some(&(s0, a0)) = rel.get(&(p as u64)) else {
                if raw_str(0) != 0 || raw_str(4) != 0 || word != 0 {
                    return Err("stack trace entry without a relocation at the label".to_string());
                }
                p += 12;
                continue;
            };
            // `S + A − P`: the target is `S + A`.
            let (section, value) = *value_of
                .get(s0 as usize)
                .ok_or_else(|| "symbol outside the table".to_string())?;
            let label = (value as i64 + a0) as u64;
            let Some((function, offset)) = function_in(&functions, section, label) else {
                return Err("stack trace label outside a function".to_string());
            };
            let (record, kind) = match rel.get(&((p + 4) as u64)) {
                Some(&(s1, a1)) => {
                    let (_, v1) = *value_of
                        .get(s1 as usize)
                        .ok_or_else(|| "symbol outside the table".to_string())?;
                    let kind = ((v1 as i64 + a1) & 3) as u32;
                    (Some((s1, a1 - i64::from(kind))), kind)
                }
                None if raw_str(4) == 3 => (None, 3),
                None => {
                    return Err("stack trace entry without a relocation at the record".to_string());
                }
            };
            entries.push(Entry {
                function,
                offset,
                record,
                kind,
                word,
            });
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
            object[e + 8..e + 16]
                .copy_from_slice(&((u64::from(*symbol) << 32) | u64::from(relative)).to_le_bytes());
            object[e + 16..e + 24].copy_from_slice(&addendum.to_le_bytes());
        }
        let hr = header(r);
        object[hr + 32..hr + 40].copy_from_slice(&((24 * new_ones.len()) as u64).to_le_bytes());
        converted = true;
    }
    Ok(converted)
}

/// Whether the stack trace table of the object generated on this host goes through the
/// converter: in COFF and ELF, except for the `DARTFORGE_STACK_TRACE_RAW=1` measurement; in
/// Mach-O never.
pub fn convert_here() -> bool {
    crate::target::target_os() != crate::target::Os::MacOs
        && !std::env::var_os("DARTFORGE_STACK_TRACE_RAW").is_some_and(|v| !v.is_empty() && v != "0")
}

/// The converter for the object format. Returns whether it converted.
///
/// # Errors
/// Those of the converters.
pub fn convert(object: &mut [u8]) -> Result<bool, String> {
    if object.starts_with(b"\x7fELF") {
        convert_elf(object)
    } else {
        convert_coff(object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(function: u32, offset: u64, record: Option<(u32, i64)>, kind: u32, word: u32) -> Entry {
        Entry {
            function,
            offset,
            record,
            kind,
            word,
        }
    }

    #[test]
    fn blob_round_trips() {
        let entries = vec![
            e(7, 40, Some((3, 16)), 0, 3 << 12 | 5),
            e(7, 12, Some((3, 0)), 0, 2 << 12),
            // The chain of an inlined point: the same label, in order.
            e(7, 40, Some((3, 32)), 0, 9 << 12 | 1),
            e(9, 0, Some((4, 8)), 1, 1 << 12 | 6),
            e(9, 0, Some((4, 8)), 2, 5 << 12 | 9),
            e(9, 0, None, 3, 1 << 2 | 1),
        ];
        let (blob, relocations) = encode(&entries, SHAPE_ELF, false);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(&blob[..4], b"DFPC");
        // Two functions and four distinct records (the one of the inlined chain,
        // `(3, 32)`, is another).
        assert_eq!(relocations.len(), 6);
        assert_eq!(relocations[0].1, 7);
        assert_eq!(relocations[1].1, 9);
        assert!(check_round_trip(&blob, &entries, &relocations).is_ok());
        let back = decode(&blob).unwrap();
        // The order of ties is that of the section: 3:5 before 9:1.
        assert_eq!(
            back[0].iter().map(|x| x.3).collect::<Vec<_>>(),
            vec![2 << 12, 3 << 12 | 5, 9 << 12 | 1]
        );
        // COFF carries the addend in place.
        let (coff, _) = encode(&entries, SHAPE_COFF, true);
        let first_record = 20 + 8 * 2;
        assert_eq!(
            i32::from_le_bytes(coff[first_record..first_record + 4].try_into().unwrap()),
            16
        );
    }

    #[test]
    fn compact_trace_rejects_inconsistent_size_and_record_index() {
        let entries = vec![Entry {
            function: 1,
            offset: 0,
            record: Some((2, 0)),
            kind: 0,
            word: 1,
        }];
        let (mut blob, _) = encode(&entries, SHAPE_ELF, false);
        blob[8..12].copy_from_slice(&0u32.to_le_bytes());
        assert!(decode(&blob).is_err());
        let (mut blob, _) = encode(&entries, SHAPE_ELF, false);
        blob[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(decode(&blob).is_err());
    }

    #[test]
    fn compact_trace_bounds_function_streams_and_integer_fields() {
        let entries = vec![
            Entry {
                function: 1,
                offset: 0,
                record: Some((2, 0)),
                kind: 0,
                word: 1,
            },
            Entry {
                function: 3,
                offset: 0,
                record: None,
                kind: 3,
                word: 0,
            },
        ];
        let (blob, _) = encode(&entries, SHAPE_ELF, false);
        for end in 0..blob.len() {
            assert!(decode(&blob[..end]).is_err(), "prefix {end}");
        }
        let mut huge_count = blob.clone();
        huge_count[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode(&huge_count).is_err());
        let mut duplicate_stream = blob.clone();
        let first = u32_at(&blob, 24).unwrap();
        duplicate_stream[32..36].copy_from_slice(&first.to_le_bytes());
        assert!(decode(&duplicate_stream).is_err());
        let mut cross_stream = blob;
        cross_stream[first as usize] = 2;
        assert!(decode(&cross_stream).is_err());
        for values in [
            vec![2, u64::MAX, 0, 1, 1, 0, 1],
            vec![1, 0, 0, u32::MAX as u64 + 1],
        ] {
            let (mut blob, _) = encode(&entries[..1], SHAPE_ELF, false);
            let first = u32_at(&blob, 24).unwrap() as usize;
            blob.truncate(first);
            for value in values {
                varint(&mut blob, value);
            }
            let size = blob.len() as u32;
            blob[8..12].copy_from_slice(&size.to_le_bytes());
            assert!(decode(&blob).is_err());
        }
        let functions = [(1, (2, u64::MAX - 4, 10))];
        assert_eq!(function_in(&functions, 2, u64::MAX), Some((1, 4)));
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
        assert_eq!(convert_elf(&mut [0u8; 8]), Ok(false));
    }
}
