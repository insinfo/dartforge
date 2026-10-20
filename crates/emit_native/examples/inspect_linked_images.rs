//! Summarize linked string bytes and declared ELF/Mach-O/PE relocation records.
use dartforge_emit_native::linked_images;
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = std::env::args_os().skip(1).collect();
    if paths.is_empty() {
        return Err("usage: inspect_linked_images <native-image>...".into());
    }
    for path in paths {
        let bytes = std::fs::read(&path)?;
        let image = linked_images::inspect(&bytes)?;
        let mut classes = BTreeMap::new();
        for string in &image.section.strings {
            *classes.entry(string.cid).or_insert(0usize) += 1;
        }
        let relocations = if image.format == "Elf" {
            Some(linked_images::elf_relocations::inspect(&bytes)?)
        } else {
            None
        };
        let macho_relocations = if image.format == "MachO" {
            Some(linked_images::macho_relocations::inspect(&bytes)?)
        } else {
            None
        };
        let pe_relocations = if image.format == "Pe" {
            Some(linked_images::pe_relocations::inspect(&bytes)?)
        } else {
            None
        };
        println!(
            "{}",
            serde_json::json!({"path":path.to_string_lossy(),"bytes":bytes.len(),"digest":image.digest,
            "format":image.format,"architecture":image.architecture,
            "strings":image.section.strings.len(),"string_classes":classes,
            "string_section_bytes":image.section.size,"string_section_digest":image.section.content_digest,
            "relocations":relocations,"macho_relocations":macho_relocations,"pe_relocations":pe_relocations,"loader_memory_verified":false})
        );
    }
    Ok(())
}
