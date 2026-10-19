//! Read back final managed string sections, without certifying loader fixups.
use crate::pruning::allocation_inventory::images::validate_storage;
use dartforge_runtime::layout::cid;
use object::{Object, ObjectSection};
use serde::Serialize;

/// A validated pointer-free permanent string in the file's section bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StringBlock {
    pub offset: usize,
    pub bytes: usize,
    pub cid: u32,
    pub length: u64,
    pub key: String,
}

/// Complete file-backed bytes of one recognized managed string section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StringSection {
    pub name: Vec<u8>,
    pub segment: Option<Vec<u8>>,
    /// Preferred virtual address in the file, not an address in a live process.
    pub address: u64,
    pub size: u64,
    pub content_digest: String,
    /// Zero words are observed as padding, not certified marker identities.
    pub zero_words: usize,
    pub strings: Vec<StringBlock>,
}

/// File observations; relocations and the actual registered range remain unproved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub digest: String,
    pub format: String,
    pub architecture: String,
    /// Digests only; link-option effects remain unverified.
    pub macho_linker_options: Vec<String>,
    pub section: StringSection,
}

fn word(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let data = bytes
        .get(offset..offset.checked_add(8).ok_or("offset overflow")?)
        .ok_or("truncated string header")?;
    Ok(u64::from_le_bytes(
        data.try_into().map_err(|_| "invalid word")?,
    ))
}

fn blocks(bytes: &[u8]) -> Result<(Vec<StringBlock>, usize), String> {
    if !bytes.len().is_multiple_of(8) {
        return Err("managed string section is not word sized".into());
    }
    let mut result = Vec::new();
    let mut offset = 0;
    let mut zero_words = 0;
    while offset < bytes.len() {
        let header = word(bytes, offset)?;
        if header == 0 {
            zero_words += 1;
            offset += 8;
            continue;
        }
        let block = &bytes[offset..];
        let hash = word(block, 8)?;
        let length = word(block, 16)?;
        let class = (header >> 32) as i32;
        let width = match class {
            cid::ONE_BYTE_STRING => 1,
            cid::TWO_BYTE_STRING => 2,
            _ => {
                return Err(format!(
                    "unreviewed managed class {class} at section offset {offset}"
                ));
            }
        };
        let used = usize::try_from(length)
            .ok()
            .and_then(|n| n.checked_mul(width))
            .ok_or("string length overflow")?;
        let size = used
            .checked_add(7)
            .map(|n| n / 8 * 8)
            .ok_or("string padding overflow")?;
        let total = size.checked_add(24).ok_or("string block size overflow")?;
        let content = block
            .get(24..total)
            .ok_or("string block exceeds its section")?;
        let storage = validate_storage(header, hash, length, content)
            .ok_or_else(|| format!("invalid permanent string at section offset {offset}"))?;
        result.push(StringBlock {
            offset,
            bytes: total,
            cid: storage.cid,
            length,
            key: storage.digest.to_hex()[..32].to_string(),
        });
        offset = offset.checked_add(total).ok_or("section offset overflow")?;
    }
    Ok((result, zero_words))
}

/// Inspect complete on-disk string-section bytes in a linked 64-bit LE image.
/// Checks permanent headers, hashes, lengths, canonical encoding and padding.
/// Loader rebases/binds, relocations, permissions and marker resolution are not
/// certified; this result never authorizes image registration or class pruning.
///
/// ```
/// assert!(dartforge_emit_native::linked_images::inspect(b"not an image").is_err());
/// ```
///
/// # Errors
/// Rejects non-image/unsupported formats, absent or ambiguous managed sections,
/// unaligned or incompletely stored data, unknown classes and invalid strings.
pub fn inspect(bytes: &[u8]) -> Result<Inventory, String> {
    let file = object::File::parse(bytes).map_err(|e| e.to_string())?;
    if !matches!(
        file.kind(),
        object::ObjectKind::Executable | object::ObjectKind::Dynamic
    ) || !file.is_64()
        || !file.is_little_endian()
        || !matches!(
            file.architecture(),
            object::Architecture::Aarch64 | object::Architecture::X86_64
        )
    {
        return Err("expected a linked 64-bit little-endian ARM64/x64 image".into());
    }
    let macho_linker_options = crate::native_inputs::macho_commands(&file)?;
    let mut result = None;
    for section in file.sections() {
        let name = section.name_bytes().map_err(|e| e.to_string())?;
        let segment = section.segment_name_bytes().map_err(|e| e.to_string())?;
        let known = match file.format() {
            object::BinaryFormat::MachO => name == b"__dfimg" && segment == Some(b"__DATA_CONST"),
            object::BinaryFormat::Elf => name == b"dfimg",
            object::BinaryFormat::Pe => name == b".dfimg",
            _ => false,
        };
        if !known {
            continue;
        }
        if result.is_some() {
            return Err("ambiguous managed string sections".into());
        }
        let size = section.size();
        if section.align() < 8
            || section.address() % 8 != 0
            || section.address().checked_add(size).is_none()
        {
            return Err("invalid managed section alignment or range".into());
        }
        let data = section.data().map_err(|e| e.to_string())?;
        let length = usize::try_from(size).map_err(|_| "managed section is too large")?;
        let data = data
            .get(..length)
            .ok_or("managed section is not fully file backed")?;
        let (strings, zero_words) = blocks(data)?;
        result = Some(StringSection {
            name: name.to_vec(),
            segment: segment.map(<[u8]>::to_vec),
            address: section.address(),
            size,
            content_digest: blake3::hash(data).to_hex().to_string(),
            zero_words,
            strings,
        });
    }
    Ok(Inventory {
        digest: blake3::hash(bytes).to_hex().to_string(),
        format: format!("{:?}", file.format()),
        architecture: format!("{:?}", file.architecture()),
        macho_linker_options,
        section: result.ok_or("no managed string section")?,
    })
}

#[cfg(test)]
mod tests;
