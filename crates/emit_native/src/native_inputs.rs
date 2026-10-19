//! Observations of explicit native link inputs, before archive selection/LTO.
//! This does not prove marker resolution, relocations or a complete linked image.

use object::{Object, ObjectSection, ObjectSymbol};
use serde::Serialize;

/// A physical section, without interpreting managed object layouts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    pub index: usize,
    pub name: Vec<u8>,
    pub segment: Option<Vec<u8>>,
    pub size: u64,
    pub alignment: u64,
    /// Offset and stored length relative to the containing member.
    pub file_range: Option<(u64, u64)>,
    pub stored_bytes: usize,
    pub content_digest: String,
    pub flags: String,
    pub managed_candidate: bool,
    pub embedded_bitcode_candidate: bool,
    pub linker_directives: bool,
}

/// A possible image marker; definitions and references are both retained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marker {
    pub name: Vec<u8>,
    pub defined: bool,
    pub undefined: bool,
    pub common: bool,
    pub section_kind: String,
    pub weak: bool,
    pub scope: String,
    pub section: Option<usize>,
}

/// One relocatable native object. Archive member order and duplicates survive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    pub name: Vec<u8>,
    /// Offset and length relative to the complete input byte slice.
    pub input_range: (u64, u64),
    pub digest: String,
    pub format: String,
    pub architecture: String,
    pub little_endian: bool,
    pub sections: Vec<Section>,
    pub markers: Vec<Marker>,
    pub macho_linker_options: Vec<String>,
}

/// Observed native input contents, not a closed-image certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inventory {
    pub digest: String,
    pub archive: bool,
    pub members: Vec<Member>,
}

fn managed_section(name: &[u8]) -> bool {
    matches!(name, b"dfimg" | b".dfimg" | b"__dfimg")
        || name.starts_with(b".dfimg$")
        || name.split(|byte| *byte == b',').nth(1) == Some(b"__dfimg")
}

fn marker_name(name: &[u8]) -> bool {
    fn matches(name: &[u8]) -> bool {
        name.starts_with(b"df.img.")
            || matches!(name, b"__start_dfimg" | b"__stop_dfimg")
            || name.starts_with(b"section$start$")
            || name.starts_with(b"section$end$")
    }
    matches(name) || name.strip_prefix(b"_").is_some_and(matches)
}

/// Walk commands fallibly even when the unified file parser is permissive.
pub(crate) fn macho_commands(file: &object::File<'_>) -> Result<Vec<String>, String> {
    let mut macho_linker_options = Vec::new();
    // The unified Mach-O parser tolerates failed load-command iteration. Walk
    // it explicitly: malformed commands must not look like absent sections.
    let mut commands = match file {
        object::File::MachO32(file) => Some(file.macho_load_commands()),
        object::File::MachO64(file) => Some(file.macho_load_commands()),
        _ => None,
    }
    .transpose()
    .map_err(|e| e.to_string())?;
    if let Some(commands) = commands.as_mut() {
        while let Some(command) = commands.next().map_err(|e| e.to_string())? {
            if command.cmd() == object::macho::LC_LINKER_OPTION {
                macho_linker_options.push(blake3::hash(command.raw_data()).to_hex().to_string());
            }
        }
    }
    Ok(macho_linker_options)
}

fn member(name: &[u8], bytes: &[u8], offset: u64) -> Result<Member, String> {
    let file = object::File::parse(bytes).map_err(|e| e.to_string())?;
    if file.kind() != object::ObjectKind::Relocatable {
        return Err("native input is not a relocatable object".into());
    }
    let macho_linker_options = macho_commands(&file)?;
    let mut sections = Vec::new();
    for section in file.sections() {
        let name = section.name_bytes().map_err(|e| e.to_string())?.to_vec();
        let segment = section
            .segment_name_bytes()
            .map_err(|e| e.to_string())?
            .map(<[u8]>::to_vec);
        let data = section.data().map_err(|e| e.to_string())?;
        sections.push(Section {
            index: section.index().0,
            managed_candidate: managed_section(&name),
            embedded_bitcode_candidate: matches!(
                name.as_slice(),
                b".llvmbc" | b"__bitcode" | b".llvm.lto"
            ),
            linker_directives: name == b".drectve",
            name,
            segment,
            size: section.size(),
            alignment: section.align(),
            file_range: section.file_range(),
            stored_bytes: data.len(),
            content_digest: blake3::hash(data).to_hex().to_string(),
            flags: format!("{:?}", section.flags()),
        });
    }
    let mut markers = Vec::new();
    for symbol in file.symbols() {
        let name = symbol.name_bytes().map_err(|e| e.to_string())?;
        if marker_name(name) {
            markers.push(Marker {
                name: name.to_vec(),
                defined: symbol.is_definition(),
                undefined: symbol.is_undefined(),
                common: symbol.is_common(),
                section_kind: format!("{:?}", symbol.section()),
                weak: symbol.is_weak(),
                scope: format!("{:?}", symbol.scope()),
                section: symbol.section_index().map(|index| index.0),
            });
        }
    }
    Ok(Member {
        name: name.to_vec(),
        input_range: (
            offset,
            u64::try_from(bytes.len()).map_err(|_| "native input too large")?,
        ),
        digest: blake3::hash(bytes).to_hex().to_string(),
        format: format!("{:?}", file.format()),
        architecture: format!("{:?}", file.architecture()),
        little_endian: file.is_little_endian(),
        sections,
        markers,
        macho_linker_options,
    })
}

/// Inspect a native relocatable object or a self-contained archive from bytes.
/// No paths are followed and no target code runs. Bitcode needs a separate LLVM
/// inspection; embedded bitcode and linker directives remain explicit observations.
/// Relocations, symbol resolution and archive member selection are not certified.
///
/// ```
/// let empty = dartforge_emit_native::native_inputs::inspect(b"!<arch>\n").unwrap();
/// assert!(empty.archive);
/// assert!(empty.members.is_empty());
/// ```
///
/// # Errors
/// Rejects unknown formats, raw bitcode, thin/nested archives, COFF short import members,
/// non-relocatable images, malformed member/section data and failed Mach-O load
/// commands. No partial inventory is returned when an inspected member fails.
pub fn inspect(bytes: &[u8]) -> Result<Inventory, String> {
    let archive = bytes.starts_with(b"!<arch>\n") || bytes.starts_with(b"!<thin>\n");
    let members = if archive {
        let archive =
            object::read::archive::ArchiveFile::parse(bytes).map_err(|e| e.to_string())?;
        if archive.is_thin() {
            return Err("thin archive requires external members".into());
        }
        archive
            .members()
            .map(|entry| {
                let entry = entry.map_err(|e| e.to_string())?;
                let data = entry.data(bytes).map_err(|e| e.to_string())?;
                member(entry.name(), data, entry.file_range().0)
                    .map_err(|error| format!("archive member {:?}: {error}", entry.name()))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![member(b"", bytes, 0)?]
    };
    Ok(Inventory {
        digest: blake3::hash(bytes).to_hex().to_string(),
        archive,
        members,
    })
}

#[cfg(test)]
mod tests;
