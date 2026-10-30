use super::*;
use dartforge_runtime::layout::{self, cid, flags, state};

const OPTIONAL: usize = 0x98;
const SECTIONS: usize = 0x188;
const BASE: u64 = 0x140000000;
fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn fixture(page: u32, entries: &[u16], arm64: bool) -> Vec<u8> {
    assert!(entries.len().is_multiple_of(2));
    let mut bytes = vec![0; 0xa00];
    bytes[..2].copy_from_slice(b"MZ");
    put32(&mut bytes, 0x3c, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    put16(&mut bytes, 0x84, if arm64 { 0xaa64 } else { 0x8664 });
    put16(&mut bytes, 0x86, 3);
    put16(&mut bytes, 0x94, 240);
    put16(&mut bytes, 0x96, 0x22);
    put16(&mut bytes, OPTIONAL, 0x20b);
    bytes[OPTIONAL + 24..OPTIONAL + 32].copy_from_slice(&BASE.to_le_bytes());
    put32(&mut bytes, OPTIONAL + 32, 0x1000);
    put32(&mut bytes, OPTIONAL + 36, 0x200);
    put32(&mut bytes, OPTIONAL + 56, 0x5000);
    put32(&mut bytes, OPTIONAL + 60, 0x200);
    put16(&mut bytes, OPTIONAL + 68, 3);
    put16(&mut bytes, OPTIONAL + 70, 0x140);
    put32(&mut bytes, OPTIONAL + 108, 16);
    put32(&mut bytes, OPTIONAL + 152, 0x4000);
    put32(&mut bytes, OPTIONAL + 156, 8 + entries.len() as u32 * 2);
    for (index, name, rva, offset, size) in [
        (0, b".dfimg".as_slice(), 0x2000, 0x400, 32),
        (1, b".data".as_slice(), 0x3000, 0x600, 0x200),
        (2, b".reloc".as_slice(), 0x4000, 0x800, 0x200),
    ] {
        let section = SECTIONS + index * 40;
        bytes[section..section + name.len()].copy_from_slice(name);
        put32(&mut bytes, section + 8, size);
        put32(&mut bytes, section + 12, rva);
        put32(&mut bytes, section + 16, 0x200);
        put32(&mut bytes, section + 20, offset);
        put32(&mut bytes, section + 36, 0x40000040);
    }
    for (index, word) in [
        layout::header_word(state::PERMANENT, flags::RAW, 2, cid::ONE_BYTE_STRING),
        u64::from(layout::string_hash([97])),
        1,
        97,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[0x400 + index * 8..0x408 + index * 8].copy_from_slice(&word.to_le_bytes());
    }
    put32(&mut bytes, 0x800, page);
    put32(&mut bytes, 0x804, 8 + entries.len() as u32 * 2);
    for (index, entry) in entries.iter().enumerate() {
        put16(&mut bytes, 0x808 + index * 2, *entry);
    }
    bytes
}

#[test]
fn directory_rvas_not_section_names_control_base_relocation_decoding() {
    for arm64 in [false, true] {
        let mut bytes = fixture(0x2000, &[0xa000, 0], arm64);
        bytes[SECTIONS + 80..SECTIONS + 88].copy_from_slice(b"renamed\0");
        let audit = inspect(&bytes).unwrap();
        assert_eq!(
            (audit.blocks, audit.entries, audit.padding_entries),
            (1, 2, 1)
        );
        assert_eq!((audit.potential_writes, audit.overlapping_writes), (1, 1));
        assert_eq!(audit.overlap_examples[0].address, BASE + 0x2000);
        assert_eq!(audit.overlap_examples[0].width, 8);
        assert_eq!(audit.managed_characteristics, 0x40000040);
    }
}

#[test]
fn complete_write_spans_and_half_open_edges_determine_overlap() {
    for (offset, expected) in [(0, true), (28, true), (32, false), (500, false)] {
        for kind in [3, 10] {
            let audit = inspect(&fixture(0x2000, &[(kind << 12) | offset, 0], false)).unwrap();
            assert_eq!(audit.overlapping_writes, u64::from(expected));
        }
    }
    assert!(inspect(&fixture(0x2000, &[0xa1fc, 0], false)).is_err());
    // A four-byte write at the end of the mapped section is valid.
    assert!(inspect(&fixture(0x2000, &[0x31fc, 0], false)).is_ok());
}

#[test]
fn repeated_writes_are_counted_and_examples_are_bounded() {
    let audit = inspect(&fixture(0x2000, &[0xa000; 100], false)).unwrap();
    assert_eq!(audit.potential_writes, 100);
    assert_eq!(audit.overlapping_writes, 100);
    assert_eq!(audit.overlap_examples.len(), 16);
    assert_eq!(audit.types.get(&10), Some(&100));
    let audit = inspect(&fixture(!0xfff, &[0, 0], false)).unwrap();
    assert_eq!(audit.potential_writes, 0);
}

#[test]
fn malformed_blocks_unknown_types_and_partial_directories_fail_closed() {
    for kind in [1, 2, 4, 5, 7, 15] {
        assert!(inspect(&fixture(0x2000, &[kind << 12, 0], false)).is_err());
    }
    for (offset, value) in [
        (0x800, 0x2001),
        (0x804, 0),
        (0x804, 6),
        (0x804, 10),
        (0x804, 16),
        (OPTIONAL + 156, 13),
        (OPTIONAL + 152, 0),
        (OPTIONAL + 152, 0x41fc),
        (OPTIONAL + 108, 17),
    ] {
        let mut bytes = fixture(0x2000, &[0xa000, 0], false);
        put32(&mut bytes, offset, value);
        assert!(inspect(&bytes).is_err(), "accepted {offset:x}={value:x}");
    }
    for length in [0, 0x7ff, 0x809, 0x9ff] {
        assert!(inspect(&fixture(0x2000, &[0xa000, 0], false)[..length]).is_err());
    }
}

#[test]
fn self_modifying_control_unmapped_and_overflowing_targets_are_rejected() {
    for (page, entry) in [
        (0x4000, 0xa000),
        (0x3000, 0xaffe),
        (0, 0xa080),
        (0xfffff000, 0xafff),
    ] {
        assert!(inspect(&fixture(page, &[entry, 0], false)).is_err());
    }
    let mut bytes = fixture(0x2000, &[0xa000, 0], false);
    bytes[OPTIONAL + 24..OPTIONAL + 32].copy_from_slice(&(u64::MAX - 0x1000).to_le_bytes());
    assert!(inspect(&bytes).is_err());
}

#[test]
fn ambiguous_mappings_and_inconsistent_headers_do_not_hide_writes() {
    for (offset, value) in [
        (SECTIONS + 40 + 12, 0x2000),
        (SECTIONS + 40 + 20, 0x400),
        (SECTIONS + 40 + 20, 0x100),
        (SECTIONS + 40 + 16, 0x201),
        (SECTIONS + 40 + 8, 0x3000),
        (OPTIONAL + 56, 0x4000),
        (OPTIONAL + 24, 1),
        (OPTIONAL + 32, 3),
        (OPTIONAL + 36, 0),
        (OPTIONAL + 60, 0x100),
    ] {
        let mut bytes = fixture(0x2000, &[0xa000, 0], false);
        put32(&mut bytes, offset, value);
        assert!(inspect(&bytes).is_err(), "accepted {offset:x}={value:x}");
    }
    let mut bytes = fixture(0x2000, &[0xa000, 0], false);
    put16(&mut bytes, 0x96, 0x23);
    assert!(inspect(&bytes).is_err());
}

#[test]
fn absent_base_relocations_do_not_certify_other_loader_effects() {
    let mut bytes = fixture(0x2000, &[0xa000, 0], false);
    put32(&mut bytes, OPTIONAL + 152, 0);
    put32(&mut bytes, OPTIONAL + 156, 0);
    put32(&mut bytes, OPTIONAL + 112 + 9 * 8, 0x3000);
    put32(&mut bytes, OPTIONAL + 116 + 9 * 8, 40);
    let audit = inspect(&bytes).unwrap();
    assert_eq!((audit.blocks, audit.potential_writes), (0, 0));
    assert_eq!(audit.other_directories.len(), 1);
    assert_eq!(audit.other_directories[0].index, 9);
}

#[test]
#[ignore = "requires Clang and LLD for x64 and ARM64 Windows images"]
fn fresh_linked_pe_images_cover_multiple_blocks_and_preserve_string_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("literal.c");
    let header = layout::header_word(state::PERMANENT, flags::RAW, 2, cid::ONE_BYTE_STRING);
    let hash = layout::string_hash([97]);
    std::fs::write(&source, format!(
        "__attribute__((section(\".dfimg$m\"), aligned(8))) const unsigned long long strings[] = {{{header}ULL,{hash},1,97}};\nconst void *pointers[2100] = {{ {} }};\nconst void *entry(void) {{ return pointers[0]; }}\n",
        vec!["strings"; 2100].join(",")
    )).unwrap();
    for target in ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"] {
        let object = directory.path().join("literal.obj");
        let output = directory.path().join("image.exe");
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let result = std::process::Command::new(clang)
            .args(["-target", target, "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = std::process::Command::new("lld-link")
            .args([
                "/entry:entry",
                "/subsystem:console",
                "/nodefaultlib",
                "/dynamicbase",
            ])
            .arg(&object)
            .arg(format!("/out:{}", output.display()))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(&output).unwrap();
        let audit = inspect(&bytes).unwrap();
        assert_eq!(audit.potential_writes, 2100);
        assert_eq!(audit.overlapping_writes, 0);
        assert!(audit.blocks >= 5);
        assert_eq!(
            super::super::inspect(&bytes).unwrap().section.strings.len(),
            1
        );
        println!(
            "{}",
            serde_json::json!({"target":target,"audit":audit,"executed":false})
        );
    }
}

#[test]
#[ignore = "requires the previously recorded Windows desktop executable"]
fn recorded_explorer_declared_base_writes_do_not_overlap_managed_strings() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join("target/desktop-matrix/cross-windows-x64/explorer.exe");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("{}: not recorded on this machine; skipped", path.display());
        return;
    };
    let audit = inspect(&bytes).unwrap();
    assert!(audit.potential_writes > 0);
    assert_eq!(audit.overlapping_writes, 0);
    println!(
        "{}",
        serde_json::json!({"path":path,"audit":audit,"executed":false})
    );
}
