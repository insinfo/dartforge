//! Audit declared ELF dynamic relocation records, not arbitrary loader writes.
use object::elf as e;
use serde::Serialize;
use std::collections::BTreeMap;

/// One bounded diagnostic example, with the complete potentially written span.
#[derive(Debug, Clone, Serialize)]
pub struct Write {
    pub table: &'static str,
    pub address: u64,
    pub relocation_type: u32,
    pub width: Option<u64>,
}

/// Counts cover the entire table; example lists are deliberately bounded.
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub kind: &'static str,
    pub address: u64,
    pub bytes: u64,
    pub digest: String,
    pub encoded_entries: u64,
    pub potential_writes: u64,
    pub overlapping_writes: u64,
    pub unclassified_writes: u64,
    pub types: BTreeMap<u32, u64>,
    pub overlap_examples: Vec<Write>,
    pub unknown_examples: Vec<Write>,
}

/// On-disk declarations only. Implicit loader writes, code invoked by symbol
/// resolution, initializers, permissions and actual registered bounds are unproved.
#[derive(Debug, Clone, Serialize)]
pub struct Audit {
    pub image_digest: String,
    pub managed_address: u64,
    pub managed_bytes: u64,
    pub managed_load_flags: u32,
    pub tables: Vec<Table>,
    pub unreviewed_dynamic_tags: Vec<i64>,
    pub initializer_tags_present: bool,
}

#[derive(Clone, Copy)]
struct Load {
    offset: u64,
    address: u64,
    stored: u64,
    memory: u64,
    flags: u32,
}

fn bytes_at(bytes: &[u8], start: u64, size: u64) -> Result<&[u8], String> {
    let end = start.checked_add(size).ok_or("ELF file range overflow")?;
    let start = usize::try_from(start).map_err(|_| "ELF offset too large")?;
    let end = usize::try_from(end).map_err(|_| "ELF end offset too large")?;
    bytes.get(start..end).ok_or("ELF data outside file".into())
}

fn u64_at(bytes: &[u8], offset: u64) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        bytes_at(bytes, offset, 8)?
            .try_into()
            .map_err(|_| "invalid ELF word")?,
    ))
}
fn u32_at(bytes: &[u8], offset: u64) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        bytes_at(bytes, offset, 4)?
            .try_into()
            .map_err(|_| "invalid ELF field")?,
    ))
}
fn u16_at(bytes: &[u8], offset: u64) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        bytes_at(bytes, offset, 2)?
            .try_into()
            .map_err(|_| "invalid ELF field")?,
    ))
}

fn overlaps(start: u64, size: u64, other: (u64, u64)) -> Result<bool, String> {
    let end = start.checked_add(size).ok_or("relocation range overflow")?;
    Ok(size != 0 && start < other.1 && other.0 < end)
}

fn mapped<'a>(
    bytes: &'a [u8],
    loads: &[Load],
    address: u64,
    size: u64,
) -> Result<(&'a [u8], u32), String> {
    let end = address
        .checked_add(size)
        .ok_or("ELF virtual range overflow")?;
    let mut matching = loads
        .iter()
        .filter(|load| address < load.address + load.memory && load.address < end);
    let load = matching.next().ok_or("ELF virtual data is not mapped")?;
    if matching.next().is_some() || address < load.address || end > load.address + load.stored {
        return Err("ELF virtual data has ambiguous or incomplete file backing".into());
    }
    Ok((
        bytes_at(bytes, load.offset + (address - load.address), size)?,
        load.flags,
    ))
}

fn width(arm64: bool, kind: u32) -> Option<u64> {
    if arm64 {
        match kind {
            e::R_AARCH64_NONE => Some(0),
            e::R_AARCH64_ABS64
            | e::R_AARCH64_GLOB_DAT
            | e::R_AARCH64_JUMP_SLOT
            | e::R_AARCH64_RELATIVE
            | e::R_AARCH64_IRELATIVE => Some(8),
            e::R_AARCH64_TLSDESC => Some(16),
            _ => None,
        }
    } else {
        match kind {
            e::R_X86_64_NONE => Some(0),
            e::R_X86_64_64
            | e::R_X86_64_GLOB_DAT
            | e::R_X86_64_JUMP_SLOT
            | e::R_X86_64_RELATIVE
            | e::R_X86_64_IRELATIVE
            | e::R_X86_64_DTPMOD64
            | e::R_X86_64_DTPOFF64
            | e::R_X86_64_TPOFF64 => Some(8),
            e::R_X86_64_32 | e::R_X86_64_32S | e::R_X86_64_PC32 => Some(4),
            e::R_X86_64_16 | e::R_X86_64_PC16 => Some(2),
            e::R_X86_64_8 | e::R_X86_64_PC8 => Some(1),
            e::R_X86_64_TLSDESC => Some(16),
            _ => None,
        }
    }
}

impl Table {
    fn write(
        &mut self,
        address: u64,
        kind: u32,
        arm64: bool,
        managed: (u64, u64),
    ) -> Result<(), String> {
        let count = self.types.entry(kind).or_default();
        *count = count.checked_add(1).ok_or("relocation count overflow")?;
        let width = width(arm64, kind);
        if width == Some(0) {
            return Ok(());
        }
        self.potential_writes = self
            .potential_writes
            .checked_add(1)
            .ok_or("relocation count overflow")?;
        let example = Write {
            table: self.kind,
            address,
            relocation_type: kind,
            width,
        };
        match width {
            None => {
                self.unclassified_writes = self
                    .unclassified_writes
                    .checked_add(1)
                    .ok_or("relocation count overflow")?;
                if self.unknown_examples.len() < 16 {
                    self.unknown_examples.push(example);
                }
            }
            Some(size) if overlaps(address, size, managed)? => {
                self.overlapping_writes = self
                    .overlapping_writes
                    .checked_add(1)
                    .ok_or("relocation count overflow")?;
                if self.overlap_examples.len() < 16 {
                    self.overlap_examples.push(example);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn relr(data: &[u8], mut emit: impl FnMut(u64) -> Result<(), String>) -> Result<(), String> {
    if !data.len().is_multiple_of(8) {
        return Err("truncated RELR entry".into());
    }
    let mut cursor = None;
    for raw in data.as_chunks::<8>().0 {
        let raw = u64::from_le_bytes(*raw);
        if raw & 1 == 0 {
            emit(raw)?;
            cursor = Some(raw.checked_add(8).ok_or("RELR cursor overflow")?);
        } else {
            let base = cursor.ok_or("RELR bitmap before address")?;
            for bit in 1..64 {
                if raw & (1 << bit) != 0 {
                    emit(
                        base.checked_add((bit - 1) * 8)
                            .ok_or("RELR bitmap overflow")?,
                    )?;
                }
            }
            cursor = Some(base.checked_add(63 * 8).ok_or("RELR cursor overflow")?);
        }
    }
    Ok(())
}

fn reviewed_tag(tag: i64) -> bool {
    matches!(
        tag,
        e::DT_NEEDED
            | e::DT_PLTRELSZ
            | e::DT_PLTGOT
            | e::DT_HASH
            | e::DT_STRTAB
            | e::DT_SYMTAB
            | e::DT_RELA
            | e::DT_RELASZ
            | e::DT_RELAENT
            | e::DT_STRSZ
            | e::DT_SYMENT
            | e::DT_INIT
            | e::DT_FINI
            | e::DT_SONAME
            | e::DT_RPATH
            | e::DT_SYMBOLIC
            | e::DT_REL
            | e::DT_RELSZ
            | e::DT_RELENT
            | e::DT_PLTREL
            | e::DT_DEBUG
            | e::DT_TEXTREL
            | e::DT_JMPREL
            | e::DT_BIND_NOW
            | e::DT_INIT_ARRAY
            | e::DT_FINI_ARRAY
            | e::DT_INIT_ARRAYSZ
            | e::DT_FINI_ARRAYSZ
            | e::DT_RUNPATH
            | e::DT_FLAGS
            | e::DT_PREINIT_ARRAY
            | e::DT_PREINIT_ARRAYSZ
            | e::DT_RELR
            | e::DT_RELRSZ
            | e::DT_RELRENT
            | e::DT_GNU_HASH
            | e::DT_VERSYM
            | e::DT_RELACOUNT
            | e::DT_RELCOUNT
            | e::DT_FLAGS_1
            | e::DT_VERDEF
            | e::DT_VERDEFNUM
            | e::DT_VERNEED
            | e::DT_VERNEEDNUM
    )
}

struct Decoder<'a> {
    bytes: &'a [u8],
    loads: Vec<Load>,
    managed: (u64, u64),
    arm64: bool,
}

impl Decoder<'_> {
    fn table(
        &self,
        kind: &'static str,
        address: u64,
        size: u64,
        entry: u64,
        relative_prefix: u64,
    ) -> Result<Table, String> {
        if !size.is_multiple_of(entry) || relative_prefix > size / entry {
            return Err("invalid ELF relocation table size/count".into());
        }
        let data = if size == 0 {
            &[]
        } else {
            mapped(self.bytes, &self.loads, address, size)?.0
        };
        let mut table = Table {
            kind,
            address,
            bytes: size,
            digest: blake3::hash(data).to_hex().to_string(),
            encoded_entries: size / entry,
            potential_writes: 0,
            overlapping_writes: 0,
            unclassified_writes: 0,
            types: BTreeMap::new(),
            overlap_examples: Vec::new(),
            unknown_examples: Vec::new(),
        };
        let relative = if self.arm64 {
            e::R_AARCH64_RELATIVE
        } else {
            e::R_X86_64_RELATIVE
        };
        if kind == "relr" {
            relr(data, |address| {
                table.write(address, relative, self.arm64, self.managed)
            })?;
        } else {
            for index in 0..size / entry {
                let target = u64_at(data, index * entry)?;
                let info = u64_at(data, index * entry + 8)?;
                let relocation = info as u32;
                if index < relative_prefix && (relocation != relative || info >> 32 != 0) {
                    return Err(
                        "ELF fast relative prefix contains a non-relative relocation".into(),
                    );
                }
                table.write(target, relocation, self.arm64, self.managed)?;
            }
        }
        Ok(table)
    }
}

/// Read REL/RELA/RELR/JMPREL from PT_DYNAMIC and bound potential write spans.
/// Uses declared load segments, not relocation section names. This never proves
/// loader memory immutability or authorizes an image/class pruning decision.
///
/// ```
/// assert!(dartforge_emit_native::linked_images::elf_relocations::inspect(b"invalid").is_err());
/// ```
///
/// # Errors
/// Rejects invalid linked string images, unsupported ELF headers, ambiguous load
/// mappings, malformed/duplicate dynamic metadata, truncated tables and overflow.
/// Unknown relocation types/tags remain explicit, rather than becoming safe writes.
pub fn inspect(bytes: &[u8]) -> Result<Audit, String> {
    if bytes.get(..7) != Some(b"\x7fELF\x02\x01\x01")
        || u16_at(bytes, 52)? != 64
        || u32_at(bytes, 20)? != 1
        || u32_at(bytes, 48)? != 0
    {
        return Err("expected supported ELF64 LE headers without processor flags".into());
    }
    let image = super::inspect(bytes)?;
    let count = u16_at(bytes, 56)?;
    let offset = u64_at(bytes, 32)?;
    if count == 0xffff || u16_at(bytes, 54)? != 56 || offset < 64 {
        return Err("unsupported ELF program header table".into());
    }
    bytes_at(bytes, offset, u64::from(count) * 56)?;
    let mut loads = Vec::new();
    let mut dynamic = None;
    for index in 0..u64::from(count) {
        let at = offset + index * 56;
        let kind = u32_at(bytes, at)?;
        if !matches!(kind, e::PT_LOAD | e::PT_DYNAMIC) {
            continue;
        }
        let load = Load {
            offset: u64_at(bytes, at + 8)?,
            address: u64_at(bytes, at + 16)?,
            stored: u64_at(bytes, at + 32)?,
            memory: u64_at(bytes, at + 40)?,
            flags: u32_at(bytes, at + 4)?,
        };
        bytes_at(bytes, load.offset, load.stored)?;
        if load.stored > load.memory || load.address.checked_add(load.memory).is_none() {
            return Err("invalid ELF load span".into());
        }
        if kind == e::PT_LOAD {
            let align = u64_at(bytes, at + 48)?;
            if align > 1
                && (!align.is_power_of_two() || load.address % align != load.offset % align)
            {
                return Err("invalid ELF load alignment".into());
            }
            loads.push(load);
        } else if dynamic.replace(load).is_some() {
            return Err("multiple PT_DYNAMIC segments".into());
        }
    }
    let managed = (
        image.section.address,
        image.section.address + image.section.size,
    );
    let (data, flags) = mapped(bytes, &loads, managed.0, image.section.size)?;
    if blake3::hash(data).to_hex().as_str() != image.section.content_digest {
        return Err("managed section differs from declared load mapping".into());
    }
    let mut tags = BTreeMap::new();
    if let Some(dynamic) = dynamic {
        if !dynamic.stored.is_multiple_of(16) {
            return Err("invalid PT_DYNAMIC size".into());
        }
        let mapped_data = mapped(bytes, &loads, dynamic.address, dynamic.stored)?.0;
        if mapped_data != bytes_at(bytes, dynamic.offset, dynamic.stored)? {
            return Err("PT_DYNAMIC mapping mismatch".into());
        }
        let mut terminated = false;
        for entry in mapped_data.as_chunks::<16>().0 {
            let tag = u64_at(entry, 0)? as i64;
            if tag == e::DT_NULL {
                terminated = true;
                break;
            }
            let value = u64_at(entry, 8)?;
            if tag != e::DT_NEEDED && tags.insert(tag, value).is_some() {
                return Err(format!("duplicate ELF dynamic tag {tag}"));
            }
        }
        if !terminated {
            return Err("unterminated PT_DYNAMIC".into());
        }
    }
    let decoder = Decoder {
        bytes,
        loads,
        managed,
        arm64: image.architecture == "Aarch64",
    };
    let mut tables = Vec::new();
    for (kind, address_tag, size_tag, entry_tag, expected, prefix_tag) in [
        (
            "rela",
            e::DT_RELA,
            e::DT_RELASZ,
            e::DT_RELAENT,
            24,
            Some(e::DT_RELACOUNT),
        ),
        (
            "rel",
            e::DT_REL,
            e::DT_RELSZ,
            e::DT_RELENT,
            16,
            Some(e::DT_RELCOUNT),
        ),
        ("relr", e::DT_RELR, e::DT_RELRSZ, e::DT_RELRENT, 8, None),
    ] {
        let prefix = prefix_tag
            .and_then(|tag| tags.get(&tag))
            .copied()
            .unwrap_or(0);
        match (
            tags.get(&address_tag),
            tags.get(&size_tag),
            tags.get(&entry_tag),
        ) {
            (None, None, None) if prefix == 0 => {}
            // Entry-size metadata can accompany PLT-only relocation tables.
            (None, None, Some(&entry)) if entry == expected && prefix == 0 => {}
            (Some(&address), Some(&size), Some(&entry)) if entry == expected => {
                tables.push(decoder.table(kind, address, size, entry, prefix)?)
            }
            _ => return Err("incomplete or unsupported ELF relocation metadata".into()),
        }
    }
    match (
        tags.get(&e::DT_JMPREL),
        tags.get(&e::DT_PLTRELSZ),
        tags.get(&e::DT_PLTREL),
    ) {
        (None, None, None) => {}
        (Some(&address), Some(&size), Some(&encoding))
            if encoding == e::DT_RELA as u64 || encoding == e::DT_REL as u64 =>
        {
            tables.push(decoder.table(
                "jmprel",
                address,
                size,
                if encoding == e::DT_RELA as u64 {
                    24
                } else {
                    16
                },
                0,
            )?);
        }
        _ => return Err("incomplete or unsupported JMPREL metadata".into()),
    }
    Ok(Audit {
        image_digest: image.digest,
        managed_address: managed.0,
        managed_bytes: image.section.size,
        managed_load_flags: flags,
        initializer_tags_present: [
            e::DT_INIT,
            e::DT_FINI,
            e::DT_INIT_ARRAY,
            e::DT_FINI_ARRAY,
            e::DT_PREINIT_ARRAY,
        ]
        .iter()
        .any(|tag| tags.get(tag).is_some_and(|value| *value != 0)),
        unreviewed_dynamic_tags: tags
            .keys()
            .copied()
            .filter(|tag| !reviewed_tag(*tag))
            .collect(),
        tables,
    })
}

#[cfg(test)]
mod tests;
