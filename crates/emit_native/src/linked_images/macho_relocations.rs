//! Audit classic Mach-O dyld opcode write spans, without certifying loader behavior.
use object::{Object, ObjectSection, macho as m};
use serde::Serialize;
use std::collections::BTreeMap;

/// One bounded example of a declared write overlapping managed string storage.
#[derive(Debug, Clone, Serialize)]
pub struct Write {
    pub address: u64,
    pub width: u64,
    pub relocation_type: u8,
}

/// Counts cover every decoded write; examples are bounded independently.
#[derive(Debug, Clone, Serialize)]
pub struct Table {
    pub kind: &'static str,
    pub offset: u64,
    pub bytes: u64,
    pub digest: String,
    pub opcodes: u64,
    pub potential_writes: u64,
    pub overlapping_writes: u64,
    pub types: BTreeMap<u8, u64>,
    pub overlap_examples: Vec<Write>,
}

/// Declared opcode effects only. Other load commands, symbol resolvers, native
/// initializers, marker binding and actual registered ranges remain unproved.
#[derive(Debug, Clone, Serialize)]
pub struct Audit {
    pub image_digest: String,
    pub managed_address: u64,
    pub managed_bytes: u64,
    pub managed_initial_protection: u32,
    pub managed_maximum_protection: u32,
    pub tables: Vec<Table>,
    pub other_load_commands: Vec<u32>,
    pub initializer_sections_present: bool,
}

#[derive(Clone)]
struct Segment {
    name: Vec<u8>,
    address: u64,
    memory: u64,
    offset: u64,
    stored: u64,
    maximum: u32,
    initial: u32,
}

fn range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], String> {
    let end = offset
        .checked_add(size)
        .ok_or("Mach-O file range overflow")?;
    let offset = usize::try_from(offset).map_err(|_| "Mach-O offset too large")?;
    let end = usize::try_from(end).map_err(|_| "Mach-O end too large")?;
    bytes
        .get(offset..end)
        .ok_or("Mach-O data outside file".into())
}
fn word(bytes: &[u8], offset: u64) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        range(bytes, offset, 8)?
            .try_into()
            .map_err(|_| "invalid Mach-O word")?,
    ))
}
fn field(bytes: &[u8], offset: u64) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        range(bytes, offset, 4)?
            .try_into()
            .map_err(|_| "invalid Mach-O field")?,
    ))
}
fn name(bytes: &[u8]) -> Vec<u8> {
    bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())].to_vec()
}
fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or("Mach-O address arithmetic overflow".into())
}
fn product(a: u64, b: u64) -> Result<u64, String> {
    a.checked_mul(b).ok_or("Mach-O run size overflow".into())
}

/// LEB values are bounded by their encoded 64-bit domain, not by host shifts.
fn leb(bytes: &[u8], position: &mut usize, signed: bool) -> Result<u64, String> {
    let mut value = 0i128;
    for shift in (0..70).step_by(7) {
        let byte = *bytes.get(*position).ok_or("truncated Mach-O LEB operand")?;
        *position += 1;
        value |= i128::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            if signed && byte & 0x40 != 0 {
                value |= -1i128 << (shift + 7);
            }
            return if signed {
                i64::try_from(value)
                    .map(|v| v as u64)
                    .map_err(|_| "Mach-O SLEB overflow".into())
            } else {
                u64::try_from(value).map_err(|_| "Mach-O ULEB overflow".into())
            };
        }
    }
    Err("overlong Mach-O LEB operand".into())
}

#[derive(Default)]
struct State {
    segment: Option<usize>,
    offset: u64,
    kind: u8,
    symbol: bool,
    ordinal: bool,
}

/// Count an arithmetic progression directly; hostile repeat counts never cause
/// a proportional loop. Validate the entire run before publishing its counts.
fn run(
    table: &mut Table,
    segments: &[Segment],
    state: &mut State,
    count: u64,
    skip: u64,
    managed: (u64, u64),
) -> Result<(), String> {
    if count == 0 {
        return Ok(());
    }
    let width = match state.kind {
        1 => 8,
        2 | 3 => 4,
        _ => return Err("unknown or missing Mach-O relocation type".into()),
    };
    let segment = segments
        .get(state.segment.ok_or("Mach-O relocation has no segment")?)
        .ok_or("Mach-O relocation segment is out of range")?;
    let step = add(8, skip)?;
    let last = add(state.offset, product(count - 1, step)?)?;
    if add(last, width)? > segment.memory {
        return Err("Mach-O relocation write exceeds its segment".into());
    }
    let start = add(segment.address, state.offset)?;
    let next = add(state.offset, product(count, step)?)?;
    let first_overlap = managed
        .0
        .saturating_sub(width - 1)
        .saturating_sub(start)
        .div_ceil(step)
        .min(count);
    let after_overlap = managed.1.saturating_sub(start).div_ceil(step).min(count);
    let overlaps = after_overlap.saturating_sub(first_overlap);
    table.potential_writes = add(table.potential_writes, count)?;
    table.overlapping_writes = add(table.overlapping_writes, overlaps)?;
    let total = table.types.entry(state.kind).or_default();
    *total = add(*total, count)?;
    for index in
        first_overlap..first_overlap + overlaps.min((16 - table.overlap_examples.len()) as u64)
    {
        table.overlap_examples.push(Write {
            address: add(start, product(index, step)?)?,
            width,
            relocation_type: state.kind,
        });
    }
    state.offset = next;
    Ok(())
}

fn decode(
    data: &[u8],
    table: &mut Table,
    segments: &[Segment],
    managed: (u64, u64),
) -> Result<(), String> {
    let binding = table.kind != "rebase";
    let lazy = table.kind == "lazy_bind";
    let mut state = State {
        kind: if lazy { 1 } else { 0 },
        ordinal: table.kind == "weak_bind",
        ..State::default()
    };
    let mut position = 0;
    let mut terminated = data.is_empty();
    while position < data.len() {
        let byte = data[position];
        position += 1;
        table.opcodes += 1;
        let opcode = byte & 0xf0;
        let immediate = byte & 0xf;
        if opcode == 0 {
            if immediate != 0 {
                return Err("invalid Mach-O DONE immediate".into());
            }
            terminated = true;
            if lazy {
                continue;
            }
            if data[position..].iter().any(|b| *b != 0) {
                return Err("nonzero Mach-O opcodes after DONE".into());
            }
            break;
        }
        terminated = false;
        if !binding {
            match opcode {
                0x10 => state.kind = immediate,
                0x20 => {
                    state.segment = Some(usize::from(immediate));
                    state.offset = leb(data, &mut position, false)?;
                }
                0x30 if immediate == 0 => {
                    state.offset = add(state.offset, leb(data, &mut position, false)?)?
                }
                0x40 => state.offset = add(state.offset, u64::from(immediate) * 8)?,
                0x50 => run(
                    table,
                    segments,
                    &mut state,
                    u64::from(immediate),
                    0,
                    managed,
                )?,
                0x60 if immediate == 0 => {
                    let count = leb(data, &mut position, false)?;
                    run(table, segments, &mut state, count, 0, managed)?;
                }
                0x70 if immediate == 0 => {
                    let skip = leb(data, &mut position, false)?;
                    run(table, segments, &mut state, 1, skip, managed)?;
                }
                0x80 if immediate == 0 => {
                    let count = leb(data, &mut position, false)?;
                    let skip = leb(data, &mut position, false)?;
                    run(table, segments, &mut state, count, skip, managed)?;
                }
                _ => return Err("unsupported Mach-O rebase opcode".into()),
            }
            continue;
        }
        let mut write = None;
        match opcode {
            0x10 => state.ordinal = true,
            0x20 if immediate == 0 => {
                leb(data, &mut position, false)?;
                state.ordinal = true;
            }
            0x30 if matches!(immediate, 0 | 13..=15) => state.ordinal = true,
            0x40 if immediate & !9 == 0 => {
                let size = data[position..]
                    .iter()
                    .position(|b| *b == 0)
                    .ok_or("unterminated Mach-O bind symbol")?;
                if size == 0 {
                    return Err("empty Mach-O bind symbol".into());
                }
                position += size + 1;
                state.symbol = true;
            }
            0x50 => state.kind = immediate,
            0x60 if immediate == 0 => {
                leb(data, &mut position, true)?;
            }
            0x70 => {
                state.segment = Some(usize::from(immediate));
                state.offset = leb(data, &mut position, false)?;
            }
            0x80 if immediate == 0 => {
                state.offset = add(state.offset, leb(data, &mut position, false)?)?
            }
            0x90 if immediate == 0 => write = Some((1, 0)),
            0xa0 if immediate == 0 => write = Some((1, leb(data, &mut position, false)?)),
            0xb0 => write = Some((1, u64::from(immediate) * 8)),
            0xc0 if immediate == 0 => {
                let count = leb(data, &mut position, false)?;
                let skip = leb(data, &mut position, false)?;
                write = Some((count, skip));
            }
            _ => return Err("unsupported Mach-O bind opcode (including threaded binding)".into()),
        }
        if let Some((count, skip)) = write {
            if count != 0 && (!state.symbol || !state.ordinal) {
                return Err("Mach-O bind has no symbol or library ordinal".into());
            }
            run(table, segments, &mut state, count, skip, managed)?;
        }
    }
    if !terminated {
        return Err("unterminated Mach-O opcode stream".into());
    }
    Ok(())
}

fn disjoint(mut ranges: Vec<(u64, u64)>) -> bool {
    ranges.retain(|(start, end)| start != end);
    ranges.sort_unstable();
    ranges.windows(2).all(|pair| pair[0].1 <= pair[1].0)
}

/// Inspect classic rebase, bind, weak-bind and lazy-bind opcode write spans in a
/// linked 64-bit little-endian ARM64/x64 Mach-O image with valid static strings.
/// Checks complete stream bounds, segment mapping and every potentially written
/// span. Symbol resolution and implicit/native loader writes are not certified.
///
/// ```
/// assert!(dartforge_emit_native::linked_images::macho_relocations::inspect(b"invalid").is_err());
/// ```
///
/// # Errors
/// Rejects malformed/truncated images, ambiguous mappings, malformed LEB values,
/// unsupported opcode/relocation forms, arithmetic overflow and writes outside
/// segments. Chained fixups and legacy relocation tables require separate audits.
pub fn inspect(bytes: &[u8]) -> Result<Audit, String> {
    let image = super::inspect(bytes)?;
    if image.format != "MachO" || field(bytes, 0)? != m::MH_MAGIC_64 {
        return Err("expected a little-endian 64-bit Mach-O image".into());
    }
    let commands_end = add(32, u64::from(field(bytes, 20)?))?;
    range(bytes, 32, commands_end - 32)?;
    let mut position = 32;
    let mut segments = Vec::new();
    let mut info = None;
    let mut other_load_commands = Vec::new();
    let mut initializer_sections_present = false;
    for _ in 0..field(bytes, 16)? {
        let command = field(bytes, position)?;
        let size = u64::from(field(bytes, add(position, 4)?)?);
        if size < 8 || size % 8 != 0 || add(position, size)? > commands_end {
            return Err("invalid Mach-O load command bounds".into());
        }
        let data = range(bytes, position, size)?;
        match command {
            m::LC_SEGMENT_64 => {
                if size < 72 || add(72, product(u64::from(field(data, 64)?), 80)?)? != size {
                    return Err("invalid Mach-O segment command".into());
                }
                let segment = Segment {
                    name: name(range(data, 8, 16)?),
                    address: word(data, 24)?,
                    memory: word(data, 32)?,
                    offset: word(data, 40)?,
                    stored: word(data, 48)?,
                    maximum: field(data, 56)?,
                    initial: field(data, 60)?,
                };
                add(segment.address, segment.memory)?;
                range(bytes, segment.offset, segment.stored)?;
                if segment.stored > segment.memory || segment.initial & !segment.maximum != 0 {
                    return Err("invalid Mach-O segment storage or protections".into());
                }
                for section in 0..field(data, 64)? {
                    let offset = 72 + u64::from(section) * 80;
                    if field(data, offset + 60)? != 0 {
                        return Err(
                            "legacy Mach-O section relocations require a separate audit".into()
                        );
                    }
                    initializer_sections_present |=
                        matches!(field(data, offset + 64)? & 0xff, 9 | 10 | 0x16);
                }
                segments.push(segment);
            }
            m::LC_DYLD_INFO | m::LC_DYLD_INFO_ONLY => {
                if size != 48 || info.is_some() {
                    return Err("duplicate or malformed Mach-O dyld info".into());
                }
                info = Some(data.to_vec());
            }
            m::LC_DYLD_CHAINED_FIXUPS => {
                return Err("Mach-O chained fixups require a separate audit".into());
            }
            m::LC_DYSYMTAB => {
                if size != 80 || field(data, 68)? != 0 || field(data, 76)? != 0 {
                    return Err("legacy Mach-O dynamic relocations require a separate audit".into());
                }
                other_load_commands.push(command);
            }
            _ => other_load_commands.push(command),
        }
        position = add(position, size)?;
    }
    if position != commands_end {
        return Err("Mach-O load command count/size mismatch".into());
    }
    if !disjoint(
        segments
            .iter()
            .map(|s| Ok((s.address, add(s.address, s.memory)?)))
            .collect::<Result<_, String>>()?,
    ) || !disjoint(
        segments
            .iter()
            .map(|s| Ok((s.offset, add(s.offset, s.stored)?)))
            .collect::<Result<_, String>>()?,
    ) {
        return Err("overlapping Mach-O segment mappings".into());
    }
    let managed = (
        image.section.address,
        add(image.section.address, image.section.size)?,
    );
    let mut owners = segments
        .iter()
        .filter(|s| s.address <= managed.0 && managed.1 <= s.address + s.stored && s.memory != 0);
    let owner = owners
        .next()
        .ok_or("managed Mach-O section has no complete file mapping")?;
    if owners.next().is_some() || owner.name != b"__DATA_CONST" {
        return Err("managed Mach-O section has an ambiguous mapping".into());
    }
    let file = object::File::parse(bytes).map_err(|e| e.to_string())?;
    let section = file
        .sections()
        .find(|s| {
            s.name_bytes().ok() == Some(b"__dfimg")
                && s.segment_name_bytes().ok() == Some(Some(b"__DATA_CONST"))
        })
        .ok_or("managed Mach-O section is missing")?;
    if section.file_range()
        != Some((
            add(owner.offset, managed.0 - owner.address)?,
            image.section.size,
        ))
    {
        return Err("managed Mach-O section address/file mapping disagrees".into());
    }
    let mut tables = Vec::new();
    let mut ranges = Vec::new();
    if let Some(info) = info {
        for (index, kind) in ["rebase", "bind", "weak_bind", "lazy_bind"]
            .into_iter()
            .enumerate()
        {
            let offset = u64::from(field(&info, 8 + index as u64 * 8)?);
            let size = u64::from(field(&info, 12 + index as u64 * 8)?);
            let data = if size == 0 {
                &[]
            } else {
                let end = add(offset, size)?;
                let mut matching = segments
                    .iter()
                    .filter(|s| s.offset <= offset && end <= s.offset + s.stored);
                if matching.next().is_none_or(|s| s.name != b"__LINKEDIT")
                    || matching.next().is_some()
                {
                    return Err("Mach-O opcode stream is not uniquely mapped in LINKEDIT".into());
                }
                ranges.push((offset, end));
                range(bytes, offset, size)?
            };
            let mut table = Table {
                kind,
                offset,
                bytes: size,
                digest: blake3::hash(data).to_hex().to_string(),
                opcodes: 0,
                potential_writes: 0,
                overlapping_writes: 0,
                types: BTreeMap::new(),
                overlap_examples: Vec::new(),
            };
            decode(data, &mut table, &segments, managed)?;
            tables.push(table);
        }
    }
    if !disjoint(ranges) {
        return Err("overlapping Mach-O dyld opcode tables".into());
    }
    Ok(Audit {
        image_digest: image.digest,
        managed_address: managed.0,
        managed_bytes: image.section.size,
        managed_initial_protection: owner.initial,
        managed_maximum_protection: owner.maximum,
        tables,
        other_load_commands,
        initializer_sections_present,
    })
}

#[cfg(test)]
mod tests;
