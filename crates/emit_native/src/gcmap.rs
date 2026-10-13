//! The compact stack map (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.4):
//! the converter of the version 3 `.llvm_stackmaps` of a COFF object into
//! **DFGM v2**, rewriting the object in place.
//!
//! The LLVM format is made for JIT patching: ~98 bytes per
//! record. The collector only needs, per call, the set of frame slots
//! that hold roots; DFGM keeps that in ~8 bytes per record
//! (varint deltas, and a bit for "the same set as the previous record").
//! It is the port of `gcmap.py` from the `x12` experiment, which runs.
//!
//! ```text
//! header (16 bytes)
//!   "DFGM"; u8 version = 2; u8 target (1 = x86-64, 2 = aarch64); u16 flags
//!   u32 total size of the blob (multiple of 4); u32 n_functions
//! index: n_functions × { u32 address of the function; u32 start in the stream }
//!   flags 0 (COFF): rva, `ADDR32NB` relocation
//!   flags 1 (ELF): i32 of the address of the function minus that of the field itself,
//!     `R_X86_64_PC32` / `R_AARCH64_PREL32` relocation — no dynamic
//!     relocation in the PIE executable, and the section can be read-only
//! stream (unsigned LEB128 varints), per function:
//!   n_records; frame size / 8 (0 = dynamic)
//!   n_records × { delta of the return offset; header }
//!     odd header: the root set of the previous record
//!     even header: (header >> 1) roots in slots, each one a varint `s`:
//!       bit 0 of s: base (0 = SP, 1 = FP)
//!       s >> 1: zigzag of the slot delta, in 8-byte words
//!     and a varint with the mask of the registers (bit `r` is
//!     DWARF register `r`) that hold roots at the call (v2)
//! zeros up to a multiple of 4
//! ```
//!
//! v1 (without the mask) is from when every `gc-live` value went to a slot;
//! the runtime reads both. With the roots in the `"deopt"` operand and
//! `use-registers-for-deopt-values` (`crates/llvm`), the root stays in the
//! preserved register it is already in across the call — LLVM's `Register`
//! location —, without a write into a slot at each collection point
//! (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §14.12).
//!
//! The converter replaces the content of the section with the blob (always smaller), renames it
//! and replaces each 64-bit relocation (one per function, in the `StkSizeRecord`) with
//! a 32-bit one in the index field. In COFF the section becomes `.dfgcm$m` and the
//! runtime finds it by name in the image; in ELF it becomes `dfgcm` (a C
//! identifier: the linker defines `__start_dfgcm`/`__stop_dfgcm`, which the module passes to the
//! runtime at startup) with `SHF_GNU_RETAIN`, because nothing references it and
//! `--gc-sections` would discard it (Perry measured that). Mach-O is not
//! converted: `ld64.lld` does not split the ThinLTO indices, so
//! production uses the linker's LTO, which emits the map in the LLVM format, and the
//! runtime reads that format (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md, Step 4).
//!
//! Rules (those of §14.4 and §14.12): the roots are the locations of the `"deopt"` and,
//! of each (base, derived) pair, only the base, without repetition; constants are
//! discarded; a location that is neither an 8-byte
//! `Indirect [SP|FP + d]` with `d` a multiple of 8, nor an 8-byte `Register`
//! in a register preserved by the target convention, is an **error**
//! (a volatile register does not survive the call); two
//! records at the same return offset are an error; after encoding,
//! the blob is decoded and compared with the origin.

/// The DWARF register of the SP and that of the FP on x86-64 and aarch64.
const REG_SP: u16 = 7;
const REG_FP: u16 = 6;
const REG_SP_AARCH64: u16 = 31;
const REG_FP_AARCH64: u16 = 29;
/// The DWARF registers preserved across a call, by
/// convention: on Windows x64, rbx, rsi, rdi, rbp and r12–r15; on SysV x86-64,
/// rbx, rbp and r12–r15; on aarch64, x19–x29.
const PRESERVED_WINDOWS: u32 = (1 << 3) | (1 << 4) | (1 << 5) | (1 << 6) | (0xF << 12);
const PRESERVED_SYSV: u32 = (1 << 3) | (1 << 6) | (0xF << 12);
const PRESERVED_AARCH64: u32 = ((1 << 11) - 1) << 19;
/// The value LLVM writes into the map for an `undef`/`poison` operand
/// (`StatepointLowering.cpp`): no value, not a root.
const UNDEFINED: i64 = 0xFEFE_FEFE;
/// The kind of a root: a slot over the SP, a slot over the FP, a register.
const NO_SP: u8 = 0;
const NO_FP: u8 = 1;
const IN_REGISTER: u8 = 2;
/// The blob version the converter writes.
const VERSION: u8 = 2;
/// The flags of the blob: the index with addresses relative to the field itself.
const RELATIVE_FLAG: u16 = 1;
/// `EM_X86_64` and `EM_AARCH64`.
const ELF_X86_64: u16 = 62;
const ELF_AARCH64: u16 = 183;
/// The ELF 64-bit absolute and 32-bit place-relative relocations.
const R_X86_64_64: u32 = 1;
const R_X86_64_PC32: u32 = 2;
const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_PREL32: u32 = 261;
/// `SHF_ALLOC` and `SHF_GNU_RETAIN`.
const SHF_ALLOC: u64 = 0x2;
const SHF_GNU_RETAIN: u64 = 0x20_0000;
/// `IMAGE_FILE_MACHINE_AMD64`.
const MACHINE_X86_64: u16 = 0x8664;
/// `IMAGE_REL_AMD64_ADDR64` and `IMAGE_REL_AMD64_ADDR32NB`.
const REL_ADDR64: u16 = 1;
const REL_ADDR32NB: u16 = 3;
/// `IMAGE_SCN_LNK_NRELOC_OVFL`: the number of relocations did not fit in 16 bits.
const SCN_NRELOC_OVFL: u32 = 0x0100_0000;

/// The roots of a record: (kind, byte offset of the slot or DWARF number
/// of the register), sorted ([`NO_SP`], [`NO_FP`], [`IN_REGISTER`]).
type Roots = Vec<(u8, i32)>;
/// A function of the map: (symbol of the relocation, frame size in bytes — 0
/// if dynamic —, records by return offset).
type Function = (u32, u64, Vec<(u32, Roots)>);

pub(crate) fn u16_at(d: &[u8], p: usize) -> Result<u16, String> {
    d.get(p..p + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u32_at(d: &[u8], p: usize) -> Result<u32, String> {
    d.get(p..p + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "objeto COFF truncado".to_string())
}

pub(crate) fn u64_at(d: &[u8], p: usize) -> Result<u64, String> {
    Ok(u64::from(u32_at(d, p)?) | (u64::from(u32_at(d, p + 4)?) << 32))
}

pub(crate) fn varint(exit: &mut Vec<u8>, mut n: u64) {
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            exit.push(b | 0x80);
        } else {
            exit.push(b);
            return;
        }
    }
}

pub(crate) fn read_varint(d: &[u8], p: &mut usize) -> Result<u64, String> {
    let mut r = 0u64;
    let mut s = 0u32;
    loop {
        let b = *d.get(*p).ok_or_else(|| "truncated DFGM map".to_string())?;
        *p += 1;
        if s < 64 {
            r |= u64::from(b & 0x7f) << s;
        }
        s += 7;
        if b & 0x80 == 0 {
            return Ok(r);
        }
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn without_zigzag(u: u64) -> i64 {
    ((u >> 1) as i64) ^ -((u & 1) as i64)
}

/// Decodes the v3 `.llvm_stackmaps` blobs of `sec`. `symbol_in` gives, by the
/// offset of each `StkSizeRecord`, its relocation (the symbol in
/// COFF, the index of the relocation in ELF); `sp` and `fp` are the DWARF registers
/// of the target and `preserved`, the mask of those that survive the call.
fn decode_v3(sec: &[u8], symbol_in: &std::collections::HashMap<u32, u32>, sp: u16, fp: u16, preserved: u32) -> Result<Vec<Function>, String> {
    let mut functions = Vec::new();
    let mut p = 0usize;
    while p + 16 <= sec.len() {
        if sec[p] == 0 {
            p += 1;
            continue;
        }
        if sec[p] != 3 {
            return Err(format!("`.llvm_stackmaps` at version {} (the converter reads 3)", sec[p]));
        }
        let n_functions = u32_at(sec, p + 4)? as usize;
        let n_constants = u32_at(sec, p + 8)? as usize;
        let n_records = u32_at(sec, p + 12)? as usize;
        let table = p + 16;
        let constants = table + 24 * n_functions;
        let mut r = constants + 8 * n_constants;
        let mut total = 0usize;
        for i in 0..n_functions {
            let frame = u64_at(sec, table + 24 * i + 8)?;
            let function_records = u64_at(sec, table + 24 * i + 16)? as usize;
            let symbol = *symbol_in
                .get(&((table + 24 * i) as u32))
                .ok_or_else(|| "stack map function without a relocation".to_string())?;
            let mut records: Vec<(u32, Roots)> = Vec::with_capacity(function_records);
            for _ in 0..function_records {
                let offset = u32_at(sec, r + 8)?;
                let n_locals = u16_at(sec, r + 14)? as usize;
                let locals = r + 16;
                // (kind, size, register, offset or constant)
                let local = |j: usize| -> Result<(u8, u16, u16, i32), String> {
                    let l = locals + 12 * j;
                    let ty = *sec.get(l).ok_or_else(|| "truncated stack map".to_string())?;
                    Ok((ty, u16_at(sec, l + 2)?, u16_at(sec, l + 4)?, u32_at(sec, l + 8)? as i32))
                };
                if n_locals < 3 || local(0)?.0 != 4 || local(1)?.0 != 4 || local(2)?.0 != 4 {
                    return Err("statepoint record without the three initial constant locations".to_string());
                }
                let n_deopt = local(2)?.3.max(0) as usize;
                if (n_locals - 3).saturating_sub(n_deopt) % 2 != 0 {
                    return Err("statepoint record with incomplete (base, derived) pairs".to_string());
                }
                if 3 + n_deopt > n_locals {
                    return Err("statepoint record with more `deopt` locations than locations".to_string());
                }
                let mut roots: Roots = Vec::new();
                // The roots: each location of the `"deopt"` (the shape of §14.12) and the
                // base of each (base, derived) pair of the `"gc-live"`.
                for j in (3..3 + n_deopt).chain((3 + n_deopt..n_locals.saturating_sub(1)).step_by(2)) {
                    let (ty, size, register, d) = local(j)?;
                    // Constant: null or a `Smi` (odd), or the `undef` the
                    // optimizer left in an operand (0xFEFEFEFE); not a
                    // root. Any other even non-null constant would be a raw
                    // value treated as a reference: an emitter defect (§3.5).
                    if ty == 4 || ty == 5 {
                        let c = if ty == 4 {
                            i64::from(d)
                        } else {
                            u64_at(sec, constants + 8 * usize::try_from(d).map_err(|_| "negative `ConstIndex` in the stack map".to_string())?)? as i64
                        };
                        if c != 0 && c & 1 == 0 && c != UNDEFINED {
                            return Err(format!("non-null even constant ({c:#x}) as a root in the stack map: a raw value treated as a reference"));
                        }
                        continue;
                    }
                    let in_register = ty == 1 && size == 8 && register < 32 && preserved & (1 << register) != 0;
                    if !in_register && (ty != 3 || size != 8 || (register != sp && register != fp)) {
                        return Err(format!(
                            "location rejected in the stack map: type {ty}, size {size}, register {register} (only 8-byte `Indirect [SP|FP + d]` or a preserved `Register`)"
                        ));
                    }
                    if !in_register && d % 8 != 0 {
                        return Err(format!("root slot not a multiple of 8 in the stack map: {d}"));
                    }
                    let root = if in_register {
                        (IN_REGISTER, i32::from(register))
                    } else {
                        (if register == fp { NO_FP } else { NO_SP }, d)
                    };
                    if !roots.contains(&root) {
                        roots.push(root);
                    }
                }
                roots.sort_unstable();
                records.push((offset, roots));
                let mut l = (locals + 12 * n_locals + 7) & !7;
                let n_exits = u16_at(sec, l + 2)? as usize;
                l = (l + 4 + 4 * n_exits + 7) & !7;
                r = l;
            }
            records.sort_by_key(|g| g.0);
            if records.windows(2).any(|par| par[0].0 == par[1].0) {
                return Err("two stack map records at the same return address".to_string());
            }
            total += records.len();
            functions.push((symbol, if frame == u64::MAX { 0 } else { frame }, records));
        }
        if total != n_records {
            return Err("the `.llvm_stackmaps` header does not match the records".to_string());
        }
        p = r;
    }
    Ok(functions)
}

/// Encodes the DFGM v2 blob with the given `flags` and `target`; also returns
/// the position, in the blob, of the address field of each function (where
/// the relocation goes).
fn encode(functions: &[Function], flags: u16, alvo: u8) -> (Vec<u8>, Vec<u32>) {
    let mut flow: Vec<u8> = Vec::new();
    let mut starts: Vec<u32> = Vec::with_capacity(functions.len());
    for (_, frame, records) in functions {
        starts.push(flow.len() as u32);
        varint(&mut flow, records.len() as u64);
        varint(&mut flow, frame / 8);
        let mut previous_return = 0u32;
        let mut previous: Option<&Roots> = None;
        for (offset, roots) in records {
            varint(&mut flow, u64::from(offset - previous_return));
            previous_return = *offset;
            if previous == Some(roots) {
                varint(&mut flow, 1);
                continue;
            }
            let n_slots = roots.iter().filter(|r| r.0 != IN_REGISTER).count();
            varint(&mut flow, (n_slots as u64) << 1);
            let mut previous_slot = 0i64;
            let mut mask = 0u64;
            for &(ty, d) in roots {
                if ty == IN_REGISTER {
                    mask |= 1 << d;
                    continue;
                }
                let slot = i64::from(d) / 8;
                varint(&mut flow, (zigzag(slot - previous_slot) << 1) | u64::from(ty == NO_FP));
                previous_slot = slot;
            }
            varint(&mut flow, mask);
            previous = Some(roots);
        }
    }
    while flow.len() % 4 != 0 {
        flow.push(0);
    }
    let header = 16 + 8 * functions.len();
    let mut blob: Vec<u8> = Vec::with_capacity(header + flow.len());
    blob.extend_from_slice(b"DFGM");
    blob.extend_from_slice(&[VERSION, alvo]);
    blob.extend_from_slice(&flags.to_le_bytes());
    blob.extend_from_slice(&((header + flow.len()) as u32).to_le_bytes());
    blob.extend_from_slice(&(functions.len() as u32).to_le_bytes());
    let mut fields = Vec::with_capacity(functions.len());
    for start in &starts {
        fields.push(blob.len() as u32);
        blob.extend_from_slice(&0u32.to_le_bytes());
        blob.extend_from_slice(&start.to_le_bytes());
    }
    blob.extend_from_slice(&flow);
    (blob, fields)
}

/// Decodes a DFGM v2 blob: per function, (frame size in bytes,
/// records). It is the reference reader (the runtime's uses the same algorithm).
fn decode(blob: &[u8]) -> Result<Vec<(u64, Vec<(u32, Roots)>)>, String> {
    if blob.len() < 16 || &blob[..4] != b"DFGM" || blob[4] != VERSION {
        return Err("DFGM blob without the version 2 header".to_string());
    }
    if u32_at(blob, 8)? as usize != blob.len() {
        return Err("DFGM blob with the wrong size in the header".to_string());
    }
    if u16_at(blob, 6)? & !RELATIVE_FLAG != 0 {
        return Err("DFGM blob with unknown flags".to_string());
    }
    let n_functions = u32_at(blob, 12)? as usize;
    let base = 16 + 8 * n_functions;
    let mut exit = Vec::with_capacity(n_functions);
    for i in 0..n_functions {
        let mut p = base + u32_at(blob, 16 + 8 * i + 4)? as usize;
        let n = read_varint(blob, &mut p)?;
        let frame = read_varint(blob, &mut p)? * 8;
        let mut records = Vec::new();
        let mut ret = 0u32;
        let mut previous: Roots = Vec::new();
        for _ in 0..n {
            ret += read_varint(blob, &mut p)? as u32;
            let c = read_varint(blob, &mut p)?;
            if c & 1 == 0 {
                previous = Vec::new();
                let mut slot = 0i64;
                for _ in 0..(c >> 1) {
                    let s = read_varint(blob, &mut p)?;
                    slot += without_zigzag(s >> 1);
                    previous.push((if s & 1 == 1 { NO_FP } else { NO_SP }, (slot * 8) as i32));
                }
                let mask = read_varint(blob, &mut p)?;
                for r in 0..64 {
                    if mask & (1 << r) != 0 {
                        previous.push((IN_REGISTER, r));
                    }
                }
                previous.sort_unstable();
            }
            records.push((ret, previous.clone()));
        }
        exit.push((frame, records));
    }
    Ok(exit)
}

/// Converts, in place, the `.llvm_stackmaps` of the x86-64 COFF object `object` into
/// `.dfgcm$m`. Returns `false`, without touching the object, when there is nothing to
/// convert or the object has a shape the converter does not rewrite (big
/// COFF, relocations beyond 16 bits, another machine): the LLVM map stays, and
/// the runtime reads it the same way (`heap::ler_stackmaps`).
///
/// # Errors
/// A map the runtime could not read (a location that is not a frame slot), or
/// a malformed object.
pub fn convert_coff(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 20 {
        return Ok(false);
    }
    let machine = u16_at(object, 0)?;
    let n_sections = u16_at(object, 2)? as usize;
    if machine != MACHINE_X86_64 {
        return Ok(false);
    }
    let symbols = u32_at(object, 8)? as usize;
    let n_symbols = u32_at(object, 12)? as usize;
    let optional = u16_at(object, 16)? as usize;
    let strings = symbols + 18 * n_symbols;
    for s in 0..n_sections {
        let h = 20 + optional + 40 * s;
        let name = object.get(h..h + 8).ok_or_else(|| "objeto COFF truncado".to_string())?;
        // The name has more than 8 bytes: `/<offset in the string table>`.
        let is_the_map = match name.strip_prefix(b"/") {
            Some(rest) => {
                let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
                let shift = std::str::from_utf8(&rest[..end]).ok().and_then(|t| t.parse::<usize>().ok());
                shift.is_some_and(|d| object.get(strings + d..).is_some_and(|t| t.starts_with(b".llvm_stackmaps\0")))
            }
            None => false,
        };
        if !is_the_map {
            continue;
        }
        let size = u32_at(object, h + 16)? as usize;
        let data = u32_at(object, h + 20)? as usize;
        let relocations = u32_at(object, h + 24)? as usize;
        let n_relocations = u16_at(object, h + 32)? as usize;
        let features = u32_at(object, h + 36)?;
        if features & SCN_NRELOC_OVFL != 0 {
            return Ok(false);
        }
        let sec = object.get(data..data + size).ok_or_else(|| "objeto COFF truncado".to_string())?.to_vec();
        let mut symbol_in: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        for k in 0..n_relocations {
            let e = relocations + 10 * k;
            if u16_at(object, e + 8)? != REL_ADDR64 {
                return Err("unexpected relocation in `.llvm_stackmaps` (expected ADDR64)".to_string());
            }
            symbol_in.insert(u32_at(object, e)?, u32_at(object, e + 4)?);
        }
        let functions = decode_v3(&sec, &symbol_in, REG_SP, REG_FP, PRESERVED_WINDOWS)?;
        let (blob, fields) = encode(&functions, 0, 1);
        // Round trip: what the runtime will read is what LLVM wrote.
        let back = decode(&blob)?;
        let equal = back.len() == functions.len()
            && back.iter().zip(&functions).all(|((frame, records), (_, q, r))| *frame == (q / 8) * 8 && records == r);
        if !equal {
            return Err("the compact stack map does not reproduce LLVM's (round trip)".to_string());
        }
        if blob.len() > size || fields.len() != n_relocations {
            return Ok(false);
        }
        object[data..data + blob.len()].copy_from_slice(&blob);
        object[data + blob.len()..data + size].fill(0);
        object[h + 16..h + 20].copy_from_slice(&(blob.len() as u32).to_le_bytes());
        object[h..h + 8].copy_from_slice(b".dfgcm$m");
        // Alinhamento 4 (`IMAGE_SCN_ALIGN_4BYTES`).
        let new_ones = (features & !0x00F0_0000) | 0x0030_0000;
        object[h + 36..h + 40].copy_from_slice(&new_ones.to_le_bytes());
        for (k, (field, (symbol, _, _))) in fields.iter().zip(&functions).enumerate() {
            let e = relocations + 10 * k;
            object[e..e + 4].copy_from_slice(&field.to_le_bytes());
            object[e + 4..e + 8].copy_from_slice(&symbol.to_le_bytes());
            object[e + 8..e + 10].copy_from_slice(&REL_ADDR32NB.to_le_bytes());
        }
        return Ok(true);
    }
    Ok(false)
}

/// Converts, in place, the `.llvm_stackmaps` of the 64-bit ELF object
/// (x86-64 or aarch64) into `dfgcm`, with `SHF_GNU_RETAIN`, alignment 4 and the
/// index relative to the field itself (`R_X86_64_PC32`, `R_AARCH64_PREL32`).
/// Returns `false`, without touching anything, when there is no map. In ELF the conversion is
/// mandatory: the LLVM map has 64-bit absolute relocations in a
/// read-only section, which `ld.lld` rejects in a PIE executable (the
/// [#75074](https://github.com/llvm/llvm-project/issues/75074)) — a shape
/// the converter does not rewrite is an error, it does not fall back to the LLVM map.
///
/// # Errors
/// A map the runtime could not read, a malformed object or one in a shape the
/// converter does not rewrite.
pub fn convert_elf(object: &mut [u8]) -> Result<bool, String> {
    if object.len() < 64 || &object[..4] != b"\x7fELF" {
        return Ok(false);
    }
    if object[4] != 2 || object[5] != 1 {
        return Err("ELF object with roots by maps that is not 64-bit little-endian".to_string());
    }
    let (alvo, sp, fp, preserved, absolute, relative) = match u16_at(object, 0x12)? {
        ELF_X86_64 => (1u8, REG_SP, REG_FP, PRESERVED_SYSV, R_X86_64_64, R_X86_64_PC32),
        ELF_AARCH64 => (2u8, REG_SP_AARCH64, REG_FP_AARCH64, PRESERVED_AARCH64, R_AARCH64_ABS64, R_AARCH64_PREL32),
        another => return Err(format!("ELF object of machine {another}: roots by maps only exist on x86-64 and aarch64")),
    };
    let table = u64_at(object, 0x28)? as usize;
    let entry_size = u16_at(object, 0x3A)? as usize;
    let n_sections = u16_at(object, 0x3C)? as usize;
    let names_index = u16_at(object, 0x3E)? as usize;
    if entry_size != 64 || n_sections == 0 || names_index >= n_sections {
        return Err("ELF object with the section table in a form that the map converter does not read (extended numbering)".to_string());
    }
    let header = |i: usize| table + 64 * i;
    let names = u64_at(object, header(names_index) + 24)? as usize;
    let name_of = |o: &[u8], i: usize| -> Result<(usize, Vec<u8>), String> {
        let start = names + u32_at(o, header(i))? as usize;
        let rest = o.get(start..).ok_or_else(|| "objeto ELF truncado".to_string())?;
        let end = rest.iter().position(|&b| b == 0).unwrap_or(rest.len());
        Ok((start, rest[..end].to_vec()))
    };
    let mut map = None;
    for i in 0..n_sections {
        if name_of(object, i)?.1 == b".llvm_stackmaps" {
            map = Some(i);
        }
    }
    let Some(m) = map else { return Ok(false) };
    // The `SHT_RELA` section (4) that relocates the map.
    let mut relocations = None;
    for i in 0..n_sections {
        let ty = u32_at(object, header(i) + 4)?;
        if (ty == 4 || ty == 9) && u32_at(object, header(i) + 44)? as usize == m {
            if ty == 9 {
                return Err("`.llvm_stackmaps` with `SHT_REL` relocations (the converter reads `SHT_RELA`)".to_string());
            }
            relocations = Some(i);
        }
    }
    let Some(r) = relocations else {
        return Err("`.llvm_stackmaps` without the relocations section".to_string());
    };
    let data = u64_at(object, header(m) + 24)? as usize;
    let size = u64_at(object, header(m) + 32)? as usize;
    let sec = object.get(data..data + size).ok_or_else(|| "objeto ELF truncado".to_string())?.to_vec();
    let rel_start = u64_at(object, header(r) + 24)? as usize;
    let rel_size = u64_at(object, header(r) + 32)? as usize;
    if u64_at(object, header(r) + 56)? != 24 || rel_size % 24 != 0 {
        return Err("relocations of `.llvm_stackmaps` outside the `Elf64_Rela` format".to_string());
    }
    let n_relocations = rel_size / 24;
    // By the offset in the map, the index of the relocation; by index, the
    // symbol and the addend.
    let mut symbol_in: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
    let mut originals: Vec<(u64, u64)> = Vec::with_capacity(n_relocations);
    for k in 0..n_relocations {
        let e = rel_start + 24 * k;
        let place = u64_at(object, e)?;
        let info = u64_at(object, e + 8)?;
        if info as u32 != absolute {
            return Err(format!("relocation {} in `.llvm_stackmaps` (expected the 64-bit absolute one)", info as u32));
        }
        symbol_in.insert(u32::try_from(place).map_err(|_| "`.llvm_stackmaps` grande demais".to_string())?, k as u32);
        originals.push((info >> 32, u64_at(object, e + 16)?));
    }
    let functions = decode_v3(&sec, &symbol_in, sp, fp, preserved)?;
    let (blob, fields) = encode(&functions, RELATIVE_FLAG, alvo);
    let back = decode(&blob)?;
    let equal = back.len() == functions.len()
        && back.iter().zip(&functions).all(|((frame, records), (_, q, r))| *frame == (q / 8) * 8 && records == r);
    if !equal {
        return Err("the compact stack map does not reproduce LLVM's (round trip)".to_string());
    }
    if blob.len() > size || fields.len() != n_relocations {
        return Err("the compact stack map does not fit in the place of `.llvm_stackmaps`".to_string());
    }
    object[data..data + blob.len()].copy_from_slice(&blob);
    object[data + blob.len()..data + size].fill(0);
    let h = header(m);
    object[h + 8..h + 16].copy_from_slice(&(SHF_ALLOC | SHF_GNU_RETAIN).to_le_bytes());
    object[h + 32..h + 40].copy_from_slice(&(blob.len() as u64).to_le_bytes());
    object[h + 48..h + 56].copy_from_slice(&4u64.to_le_bytes());
    // The name: `dfgcm` over the `.llvm_stackmaps` text (shorter). If the
    // string table linker shared it with `.rela.llvm_stackmaps`, the name of the
    // relocation section changes too, with no effect: it is found by its type.
    let (location, _) = name_of(object, m)?;
    object[location..location + 6].copy_from_slice(b"dfgcm\0");
    for (k, (field, (ident, _, _))) in fields.iter().zip(&functions).enumerate() {
        let (symbol, addendum) = originals[*ident as usize];
        let e = rel_start + 24 * k;
        object[e..e + 8].copy_from_slice(&u64::from(*field).to_le_bytes());
        object[e + 8..e + 16].copy_from_slice(&((symbol << 32) | u64::from(relative)).to_le_bytes());
        object[e + 16..e + 24].copy_from_slice(&addendum.to_le_bytes());
    }
    Ok(true)
}

/// Whether the object generated on this host goes through the converter: in ELF always
/// (mandatory, [`convert_elf`]); in COFF except for the measurement
/// `DARTFORGE_SEM_MAPA_COMPACTO=1`; in Mach-O never.
pub fn convert_here() -> bool {
    match crate::alvo::sistema() {
        crate::alvo::Sistema::Linux => true,
        crate::alvo::Sistema::Windows => !std::env::var_os("DARTFORGE_SEM_MAPA_COMPACTO").is_some_and(|v| !v.is_empty() && v != "0"),
        crate::alvo::Sistema::MacOs => false,
    }
}

/// The converter for the object format: COFF or ELF. Returns `false` when it did not
/// convert (no map; or Mach-O, which stays in the LLVM format).
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

    #[test]
    fn varint_and_zigzag_round_trip() {
        for n in [0u64, 1, 127, 128, 300, 1 << 20, u64::MAX >> 1] {
            let mut b = Vec::new();
            varint(&mut b, n);
            let mut p = 0;
            assert_eq!(read_varint(&b, &mut p).unwrap(), n);
            assert_eq!(p, b.len());
        }
        for v in [0i64, 1, -1, 5, -5, 1 << 30, -(1 << 30)] {
            assert_eq!(without_zigzag(zigzag(v)), v);
        }
    }

    #[test]
    fn blob_round_trips() {
        let functions: Vec<Function> = vec![
            (0, 72, vec![(20, vec![(NO_SP, 32)]), (44, vec![(NO_SP, 32), (NO_SP, 40)]), (60, vec![(NO_SP, 32), (NO_SP, 40)])]),
            (1, 0, vec![(48, vec![(NO_SP, 40), (NO_FP, -16), (IN_REGISTER, 3), (IN_REGISTER, 14)]), (52, vec![(IN_REGISTER, 5)])]),
            (2, 40, Vec::new()),
        ];
        let (blob, fields) = encode(&functions, 0, 1);
        assert_eq!(blob.len() % 4, 0);
        assert_eq!(fields, vec![16, 24, 32]);
        let back = decode(&blob).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0], (72, functions[0].2.clone()));
        assert_eq!(back[1], (0, functions[1].2.clone()));
        assert_eq!(back[2], (40, Vec::new()));
    }

    #[test]
    fn object_without_map_stays_as_is() {
        let mut empty = vec![0u8; 8];
        assert_eq!(convert_coff(&mut empty), Ok(false));
        // An x86-64 header without sections.
        let mut o = vec![0u8; 20];
        o[0] = 0x64;
        o[1] = 0x86;
        let before = o.clone();
        assert_eq!(convert_coff(&mut o), Ok(false));
        assert_eq!(o, before);
    }
}
