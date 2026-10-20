//! Bounded walks of user-space 64-bit Mach-O chained fixups.
use super::{Segment, add, disjoint, field, product, range, word};
use serde::Serialize;
use std::collections::BTreeSet;

/// One bounded example; every supported format writes an eight-byte pointer.
#[derive(Debug, Clone, Serialize)]
pub struct ChainWrite {
    pub address: u64,
    pub pointer_format: u16,
    pub binding: bool,
    pub authenticated: bool,
}

/// Counts for every declared chain in a segment, without exposing symbol targets.
#[derive(Debug, Clone, Serialize)]
pub struct SegmentChains {
    pub segment_index: u32,
    pub pointer_format: u16,
    pub page_size: u16,
    pub page_count: u16,
    pub chains: u64,
    pub rebases: u64,
    pub binds: u64,
    pub authenticated_writes: u64,
    pub overlapping_writes: u64,
    pub overlap_examples: Vec<ChainWrite>,
}

/// Declared chained pointer locations only; binding targets, authentication and
/// arbitrary loader effects remain unproved, even when no overlaps are reported.
#[derive(Debug, Clone, Serialize)]
pub struct ChainsAudit {
    pub offset: u64,
    pub bytes: u64,
    pub digest: String,
    pub imports: u32,
    pub imports_format: u32,
    pub symbols_bytes: u64,
    pub segments: Vec<SegmentChains>,
}

fn half(bytes: &[u8], offset: u64) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        range(bytes, offset, 2)?
            .try_into()
            .map_err(|_| "invalid chained half word")?,
    ))
}

struct Pointer {
    next: u64,
    stride: u64,
    binding: bool,
    authenticated: bool,
}

/// Decode location-control bits without treating target bits as managed values.
fn pointer(bits: u64, format: u16, imports: u32) -> Result<Pointer, String> {
    let (next, stride, binding, authenticated, ordinal) = match format {
        2 | 6 => {
            let binding = bits >> 63 != 0;
            let reserved = if binding {
                (bits >> 32) & 0x7ffff
            } else {
                (bits >> 44) & 0x7f
            };
            if reserved != 0 {
                return Err("nonzero reserved chained pointer bits".into());
            }
            ((bits >> 51) & 0xfff, 4, binding, false, bits & 0xffffff)
        }
        1 | 9 | 12 => {
            let binding = bits & (1 << 62) != 0;
            let ordinal_bits = if format == 12 { 24 } else { 16 };
            let ordinal_mask = (1u64 << ordinal_bits) - 1;
            if binding && (bits >> ordinal_bits) & (0xffffffff >> ordinal_bits) != 0 {
                return Err("nonzero reserved arm64e bind bits".into());
            }
            (
                (bits >> 51) & 0x7ff,
                8,
                binding,
                bits >> 63 != 0,
                bits & ordinal_mask,
            )
        }
        _ => return Err("unsupported chained pointer format".into()),
    };
    if binding && ordinal >= u64::from(imports) {
        return Err("chained bind ordinal exceeds imports".into());
    }
    Ok(Pointer {
        next,
        stride,
        binding,
        authenticated,
    })
}

struct Walker<'a> {
    image: &'a [u8],
    segments: &'a [Segment],
    metadata: (u64, u64),
    header_end: u64,
    imports: u32,
    managed: (u64, u64),
    visited: BTreeSet<u64>,
}
impl Walker<'_> {
    /// A visited address has one immutable source word. Repeated/overlapping
    /// targets could change a later control word during loading and are refused.
    fn chain(&mut self, stats: &mut SegmentChains, page: u64, start: u16) -> Result<(), String> {
        let segment = &self.segments[stats.segment_index as usize];
        if segment.name == b"__LINKEDIT" {
            return Err("chained fixup targets LINKEDIT metadata".into());
        }
        let page_offset = product(page, u64::from(stats.page_size))?;
        let mut offset = u64::from(start);
        stats.chains = add(stats.chains, 1)?;
        loop {
            if add(offset, 8)? > u64::from(stats.page_size) {
                return Err("chained pointer crosses its page".into());
            }
            let relative = add(page_offset, offset)?;
            if add(relative, 8)? > segment.stored || add(relative, 8)? > segment.memory {
                return Err("chained pointer is not completely file backed".into());
            }
            let address = add(segment.address, relative)?;
            let end = add(address, 8)?;
            if self
                .visited
                .range(..address)
                .next_back()
                .is_some_and(|previous| *previous + 8 > address)
                || self
                    .visited
                    .range(address..)
                    .next()
                    .is_some_and(|next| *next < end)
            {
                return Err("duplicate or overlapping chained fixup targets".into());
            }
            self.visited.insert(address);
            let source = add(segment.offset, relative)?;
            let source_end = add(source, 8)?;
            if source < self.header_end || source < self.metadata.1 && self.metadata.0 < source_end
            {
                return Err("chained fixup targets its header or control metadata".into());
            }
            let decoded = pointer(
                word(self.image, source)?,
                stats.pointer_format,
                self.imports,
            )?;
            if offset % decoded.stride != 0 {
                return Err("misaligned chained pointer start".into());
            }
            if decoded.binding {
                stats.binds = add(stats.binds, 1)?;
            } else {
                stats.rebases = add(stats.rebases, 1)?;
            }
            if decoded.authenticated {
                stats.authenticated_writes = add(stats.authenticated_writes, 1)?;
            }
            if address < self.managed.1 && self.managed.0 < end {
                stats.overlapping_writes = add(stats.overlapping_writes, 1)?;
                if stats.overlap_examples.len() < 16 {
                    stats.overlap_examples.push(ChainWrite {
                        address,
                        pointer_format: stats.pointer_format,
                        binding: decoded.binding,
                        authenticated: decoded.authenticated,
                    });
                }
            }
            if decoded.next == 0 {
                return Ok(());
            }
            offset = add(offset, product(decoded.next, decoded.stride)?)?;
        }
    }
}

/// Scan named portions of the symbol pool once. Shared suffixes reuse the
/// previous terminator; unused symbol padding requires no per-byte index.
fn imports(data: &[u8], offset: u64, count: u32, format: u32, symbols: u64) -> Result<(), String> {
    let width = match format {
        1 => 4,
        2 => 8,
        3 => 16,
        _ => return Err("unsupported chained import format".into()),
    };
    let size = product(u64::from(count), width)?;
    range(data, offset, size)?;
    if add(offset, size)? > symbols {
        return Err("chained imports overlap symbol pool".into());
    }
    let pool = range(data, symbols, data.len() as u64 - symbols)?;
    let mut names = Vec::new();
    for index in 0..u64::from(count) {
        let at = add(offset, product(index, width)?)?;
        let name = if format == 3 {
            let bits = word(data, at)?;
            if (bits >> 17) & 0x7fff != 0 {
                return Err("reserved chained import bits are nonzero".into());
            }
            bits >> 32
        } else {
            u64::from(field(data, at)? >> 9)
        };
        names.push(usize::try_from(name).map_err(|_| "chained symbol offset too large")?);
    }
    names.sort_unstable();
    names.dedup();
    let mut end = 0;
    for name in names {
        if pool.get(name).is_none_or(|b| *b == 0) {
            return Err("chained import name is out of bounds or empty".into());
        }
        if name >= end {
            end = name
                + pool[name..]
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or("unterminated chained import name")?;
        }
    }
    Ok(())
}

struct Record<'a> {
    data: &'a [u8],
    stats: SegmentChains,
}

pub(super) fn inspect(
    image: &[u8],
    offset: u64,
    size: u64,
    segments: &[Segment],
    base: u64,
    managed: (u64, u64),
    arm64: bool,
) -> Result<ChainsAudit, String> {
    let data = range(image, offset, size)?;
    if field(data, 0)? != 0 || field(data, 24)? != 0 {
        return Err("unsupported chained fixup header version or compressed symbols".into());
    }
    let starts = u64::from(field(data, 4)?);
    let import_offset = u64::from(field(data, 8)?);
    let symbols = u64::from(field(data, 12)?);
    let count = field(data, 16)?;
    let format = field(data, 20)?;
    if starts < 28 || starts >= import_offset || import_offset > symbols || symbols > size {
        return Err("invalid chained fixup metadata ordering".into());
    }
    imports(data, import_offset, count, format, symbols)?;
    let segment_count = field(data, starts)?;
    if segment_count as usize != segments.len() {
        return Err("chained segment count does not match load commands".into());
    }
    let starts_size = add(4, product(u64::from(segment_count), 4)?)?;
    range(data, starts, starts_size)?;
    let starts_end = add(starts, starts_size)?;
    if starts_end > import_offset {
        return Err("chained segment offsets exceed starts region".into());
    }
    let mut regions = vec![(starts, starts_end)];
    let mut records = Vec::new();
    let mut pointer_format = None;
    for index in 0..segment_count {
        let relative = u64::from(field(data, add(starts + 4, u64::from(index) * 4)?)?);
        if relative == 0 {
            continue;
        }
        let at = add(starts, relative)?;
        let length = u64::from(field(data, at)?);
        let record = range(data, at, length)?;
        let page_size = half(record, 4)?;
        let format = half(record, 6)?;
        let page_count = half(record, 20)?;
        if length < add(22, product(u64::from(page_count), 2)?)?
            || length % 2 != 0
            || at < starts_end
            || add(at, length)? > import_offset
        {
            return Err("chained segment record exceeds starts region".into());
        }
        if !matches!(page_size, 4096 | 16384)
            || !matches!(format, 1 | 2 | 6 | 9 | 12)
            || !arm64 && matches!(format, 1 | 9 | 12)
            || field(record, 16)? != 0
        {
            return Err("unsupported chained page size, pointer format or max pointer".into());
        }
        if pointer_format.is_some_and(|previous| previous != format) {
            return Err("mixed chained pointer formats".into());
        }
        pointer_format = Some(format);
        let segment = &segments[index as usize];
        if segment.address.checked_sub(base) != Some(word(record, 8)?)
            || u64::from(page_count) > segment.memory.div_ceil(u64::from(page_size))
        {
            return Err("chained segment offset or page count disagrees with mapping".into());
        }
        regions.push((at, add(at, length)?));
        records.push(Record {
            data: record,
            stats: SegmentChains {
                segment_index: index,
                pointer_format: format,
                page_size,
                page_count,
                chains: 0,
                rebases: 0,
                binds: 0,
                authenticated_writes: 0,
                overlapping_writes: 0,
                overlap_examples: Vec::new(),
            },
        });
    }
    if !disjoint(regions) {
        return Err("overlapping chained segment records".into());
    }
    let mut walker = Walker {
        image,
        segments,
        metadata: (offset, add(offset, size)?),
        header_end: add(32, u64::from(field(image, 20)?))?,
        imports: count,
        managed,
        visited: BTreeSet::new(),
    };
    for Record { data, stats } in &mut records {
        for page in 0..stats.page_count {
            let start = half(data, 22 + u64::from(page) * 2)?;
            if start == 0xffff {
                continue;
            }
            if start & 0x8000 == 0 {
                walker.chain(stats, u64::from(page), start)?;
                continue;
            }
            let mut index = u64::from(start & 0x7fff);
            if index < u64::from(stats.page_count) {
                return Err("chained multi-start index overlaps page entries".into());
            }
            loop {
                let entry = half(data, add(22, product(index, 2)?)?)?;
                walker.chain(stats, u64::from(page), entry & 0x7fff)?;
                if entry & 0x8000 != 0 {
                    break;
                }
                index = add(index, 1)?;
            }
        }
    }
    Ok(ChainsAudit {
        offset,
        bytes: size,
        digest: blake3::hash(data).to_hex().to_string(),
        imports: count,
        imports_format: format,
        symbols_bytes: size - symbols,
        segments: records.into_iter().map(|r| r.stats).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_formats_preserve_stride_authentication_and_ordinal_width() {
        for format in [2, 6] {
            let p = pointer((3 << 51) | (1 << 63), format, 1).unwrap();
            assert_eq!(
                (p.next, p.stride, p.binding, p.authenticated),
                (3, 4, true, false)
            );
            assert!(pointer(1 << 44, format, 1).is_err());
            assert!(pointer((1 << 63) | (1 << 32), format, 1).is_err());
        }
        for format in [1, 9, 12] {
            let p = pointer((3 << 51) | (1 << 63) | (1 << 62), format, 1).unwrap();
            assert_eq!(
                (p.next, p.stride, p.binding, p.authenticated),
                (3, 8, true, true)
            );
        }
        assert!(pointer((1 << 62) | 0x10000, 9, 0x10001).is_err());
        assert!(pointer((1 << 62) | 0x10000, 12, 0x10001).is_ok());
        assert!(pointer(1 << 63, 2, 0).is_err());
        for format in [0, 3, 4, 5, 7, 8, 10, 11, 13, 14, u16::MAX] {
            assert!(pointer(0, format, 1).is_err());
        }
    }
    #[test]
    fn shared_suffix_names_reuse_bounds_and_unused_padding_needs_no_index() {
        let mut data = vec![0u8; 16];
        for (i, offset) in [0u32, 1, 2, 0].into_iter().enumerate() {
            data[i * 4..i * 4 + 4].copy_from_slice(&((offset << 9) | 1).to_le_bytes());
        }
        data.extend_from_slice(b"abcdef\0");
        imports(&data, 0, 4, 1, 16).unwrap();
        assert!(imports(&data[..data.len() - 1], 0, 4, 1, 16).is_err());
        assert!(imports(&data, 0, 5, 1, 16).is_err());
        let padding = vec![0u8; 100_000];
        imports(&padding, 0, 0, 1, 0).unwrap();
    }
}
