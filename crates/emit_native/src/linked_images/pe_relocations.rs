//! Audit declared PE32+ base relocation writes, not arbitrary Windows loader effects.
use serde::Serialize;
use std::collections::BTreeMap;

/// A declared data directory. The certificate directory uses file offsets,
/// whereas ordinary directories use RVAs; no other directory is certified here.
#[derive(Debug, Clone, Serialize)]
pub struct Directory {
    pub index: u32,
    pub address: u32,
    pub bytes: u32,
}

/// A bounded example of a complete relocation write overlapping managed storage.
#[derive(Debug, Clone, Serialize)]
pub struct Write {
    pub address: u64,
    pub width: u32,
    pub relocation_type: u16,
}

/// All declared base writes are counted, including repeated targets. Imports,
/// TLS callbacks, load configuration, native initializers and registered image
/// bounds remain unproved. This never authorizes class pruning or registration.
#[derive(Debug, Clone, Serialize)]
pub struct Audit {
    pub image_digest: String,
    pub preferred_base: u64,
    pub image_bytes: u32,
    pub managed_address: u64,
    pub managed_bytes: u64,
    pub managed_characteristics: u32,
    pub coff_characteristics: u16,
    pub dll_characteristics: u16,
    pub table_address: u32,
    pub table_bytes: u32,
    pub table_digest: String,
    pub blocks: u64,
    pub entries: u64,
    pub padding_entries: u64,
    pub potential_writes: u64,
    pub overlapping_writes: u64,
    pub types: BTreeMap<u16, u64>,
    pub overlap_examples: Vec<Write>,
    pub other_directories: Vec<Directory>,
}

struct Section {
    rva: u64,
    memory: u64,
    offset: u64,
    stored: u64,
    characteristics: u32,
}
fn range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], String> {
    let end = offset.checked_add(size).ok_or("PE file range overflow")?;
    bytes
        .get(
            usize::try_from(offset).map_err(|_| "PE offset too large")?
                ..usize::try_from(end).map_err(|_| "PE end too large")?,
        )
        .ok_or("PE range is not fully file backed".into())
}
fn field(bytes: &[u8], offset: u64) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        range(bytes, offset, 4)?.try_into().unwrap(),
    ))
}
fn half(bytes: &[u8], offset: u64) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        range(bytes, offset, 2)?.try_into().unwrap(),
    ))
}
fn overlaps(a: u64, size: u64, b: u64, length: u64) -> bool {
    size != 0 && length != 0 && a < b + length && b < a + size
}
fn contains(start: u64, size: u64, address: u64, width: u64) -> bool {
    address >= start
        && address
            .checked_add(width)
            .is_some_and(|end| end <= start + size)
}
fn stored<'a>(
    bytes: &'a [u8],
    sections: &[Section],
    rva: u64,
    size: u64,
) -> Result<&'a [u8], String> {
    let section = sections
        .iter()
        .find(|s| contains(s.rva, s.stored, rva, size))
        .ok_or("PE relocation directory is not fully section backed")?;
    range(bytes, section.offset + rva - section.rva, size)
}

/// Inspect every declared base-relocation block of a linked x64/ARM64 PE32+ image.
/// Uses the optional-header data directory, not a `.reloc` section-name heuristic.
/// Supports ABSOLUTE padding, HIGHLOW (four bytes) and DIR64 (eight bytes).
/// Complete write spans are checked against section mappings and managed storage;
/// writes into headers or the relocation directory itself are rejected.
///
/// ```
/// assert!(dartforge_emit_native::linked_images::pe_relocations::inspect(b"invalid").is_err());
/// ```
///
/// # Errors
/// Rejects malformed/unsupported images, ambiguous mappings, incomplete directories,
/// unknown relocation kinds, unmapped writes and writes that can mutate control data.
pub fn inspect(bytes: &[u8]) -> Result<Audit, String> {
    let image = super::inspect(bytes)?;
    if image.format != "Pe" {
        return Err("expected a linked PE32+ image".into());
    }
    let pe = u64::from(field(bytes, 0x3c)?);
    if range(bytes, pe, 4)? != b"PE\0\0" {
        return Err("invalid PE signature".into());
    }
    let coff = pe + 4;
    if !matches!(half(bytes, coff)?, 0x8664 | 0xaa64) {
        return Err("unsupported PE machine".into());
    }
    let sections_count = u64::from(half(bytes, coff + 2)?);
    let optional_size = u64::from(half(bytes, coff + 16)?);
    let optional = coff + 20;
    let header = range(bytes, optional, optional_size)?;
    if optional_size < 112 || half(header, 0)? != 0x20b {
        return Err("expected a complete PE32+ optional header".into());
    }
    let base = u64::from_le_bytes(range(header, 24, 8)?.try_into().unwrap());
    let alignment = field(header, 32)?;
    let file_alignment = field(header, 36)?;
    let image_bytes = field(header, 56)?;
    let headers = u64::from(field(header, 60)?);
    let directory_count = u64::from(field(header, 108)?);
    if directory_count > 16 || 112 + directory_count * 8 > optional_size {
        return Err("unsupported or truncated PE data directories".into());
    }
    if !base.is_multiple_of(65536)
        || !alignment.is_power_of_two()
        || !file_alignment.is_power_of_two()
        || alignment < file_alignment
        || (alignment < 4096 && alignment != file_alignment)
        || (alignment >= 4096 && !(512..=65536).contains(&file_alignment))
        || !image_bytes.is_multiple_of(alignment)
        || !headers.is_multiple_of(u64::from(file_alignment))
        || headers > u64::from(image_bytes)
        || base.checked_add(u64::from(image_bytes)).is_none()
    {
        return Err("invalid PE image alignment or extent".into());
    }
    range(bytes, 0, headers)?;
    let table = optional + optional_size;
    if table + sections_count * 40 > headers || !(1..=96).contains(&sections_count) {
        return Err("PE section table exceeds headers".into());
    }
    let mut sections: Vec<Section> = Vec::new();
    for index in 0..sections_count {
        let start = table + index * 40;
        let size = u64::from(field(bytes, start + 8)?);
        let rva = u64::from(field(bytes, start + 12)?);
        let stored = u64::from(field(bytes, start + 16)?);
        let offset = u64::from(field(bytes, start + 20)?);
        let memory = size.max(stored);
        if alignment < 4096 && stored != 0 && offset != rva
            || !rva.is_multiple_of(u64::from(alignment))
            || rva < headers
            || rva + memory > u64::from(image_bytes)
            || !stored.is_multiple_of(u64::from(file_alignment))
            || stored != 0
                && (offset < headers || !offset.is_multiple_of(u64::from(file_alignment)))
            || sections.iter().any(|s| {
                overlaps(rva, memory, s.rva, s.memory)
                    || overlaps(offset, stored, s.offset, s.stored)
            })
        {
            return Err("invalid or ambiguous PE section mapping".into());
        }
        if stored != 0 {
            range(bytes, offset, stored)?;
        }
        sections.push(Section {
            rva,
            memory,
            offset,
            stored,
            characteristics: field(bytes, start + 36)?,
        });
    }
    let managed = image
        .section
        .address
        .checked_sub(base)
        .ok_or("invalid PE managed RVA")?;
    let managed_section = sections
        .iter()
        .find(|s| contains(s.rva, s.memory, managed, image.section.size))
        .ok_or("PE managed section has no complete mapping")?;
    let mut directories = Vec::new();
    let mut table_rva = 0;
    let mut table_bytes = 0;
    for index in 0..directory_count {
        let address = field(header, 112 + index * 8)?;
        let size = field(header, 116 + index * 8)?;
        if index == 5 {
            table_rva = address;
            table_bytes = size;
        } else if address != 0 || size != 0 {
            directories.push(Directory {
                index: index as u32,
                address,
                bytes: size,
            });
        }
    }
    let characteristics = half(bytes, coff + 18)?;
    if (table_rva == 0) != (table_bytes == 0)
        || table_bytes != 0 && (characteristics & 1 != 0 || !table_rva.is_multiple_of(4))
    {
        return Err("inconsistent PE base relocation directory".into());
    }
    let relocation_data = if table_bytes == 0 {
        &[]
    } else {
        stored(
            bytes,
            &sections,
            u64::from(table_rva),
            u64::from(table_bytes),
        )?
    };
    let mut audit = Audit {
        image_digest: image.digest,
        preferred_base: base,
        image_bytes,
        managed_address: image.section.address,
        managed_bytes: image.section.size,
        managed_characteristics: managed_section.characteristics,
        coff_characteristics: characteristics,
        dll_characteristics: half(header, 70)?,
        table_address: table_rva,
        table_bytes,
        table_digest: blake3::hash(relocation_data).to_hex().to_string(),
        blocks: 0,
        entries: 0,
        padding_entries: 0,
        potential_writes: 0,
        overlapping_writes: 0,
        types: BTreeMap::new(),
        overlap_examples: Vec::new(),
        other_directories: directories,
    };
    let mut position = 0;
    while position < relocation_data.len() as u64 {
        let page = u64::from(field(relocation_data, position)?);
        let size = u64::from(field(relocation_data, position + 4)?);
        if !page.is_multiple_of(4096) || size < 8 || !size.is_multiple_of(4) {
            return Err("invalid PE base relocation block".into());
        }
        let block = range(relocation_data, position, size)?;
        audit.blocks += 1;
        for entry in block[8..].as_chunks::<2>().0 {
            let entry = u16::from_le_bytes(*entry);
            let kind = entry >> 12;
            audit.entries += 1;
            *audit.types.entry(kind).or_default() += 1;
            let width = match kind {
                0 => {
                    audit.padding_entries += 1;
                    continue;
                }
                3 => 4,
                10 => 8,
                _ => return Err(format!("unsupported PE base relocation type {kind}")),
            };
            let target = page + u64::from(entry & 0xfff);
            if !sections
                .iter()
                .any(|s| contains(s.rva, s.memory, target, width))
            {
                return Err("PE base relocation write is outside one section".into());
            }
            if overlaps(target, width, 0, headers)
                || overlaps(target, width, u64::from(table_rva), u64::from(table_bytes))
            {
                return Err("PE base relocation write can alter loader control data".into());
            }
            audit.potential_writes += 1;
            if overlaps(target, width, managed, image.section.size) {
                audit.overlapping_writes += 1;
                if audit.overlap_examples.len() < 16 {
                    audit.overlap_examples.push(Write {
                        address: base + target,
                        width: width as u32,
                        relocation_type: kind,
                    });
                }
            }
        }
        position += size;
    }
    Ok(audit)
}

#[cfg(test)]
mod tests;
