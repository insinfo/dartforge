use super::*;
use dartforge_runtime::layout::{self, cid, flags, state};
use object::Object;

const BASE: u64 = 0x40_0000;
const IMAGE: u64 = BASE + 0x400;

fn put16(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], at: usize, value: u64) {
    bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

fn fixture(tags: &[(i64, u64)], table: &[u8], arm64: bool) -> Vec<u8> {
    let mut bytes = vec![0; 0x900];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    put16(&mut bytes, 16, 3);
    put16(&mut bytes, 18, if arm64 { 183 } else { 62 });
    put32(&mut bytes, 20, 1);
    put64(&mut bytes, 32, 64);
    put64(&mut bytes, 40, 0x800);
    put16(&mut bytes, 52, 64);
    put16(&mut bytes, 54, 56);
    put16(&mut bytes, 56, 2);
    put16(&mut bytes, 58, 64);
    put16(&mut bytes, 60, 3);
    put16(&mut bytes, 62, 2);
    put32(&mut bytes, 64, e::PT_LOAD);
    put32(&mut bytes, 68, 6);
    put64(&mut bytes, 80, BASE);
    put64(&mut bytes, 96, 0x900);
    put64(&mut bytes, 104, 0x900);
    put64(&mut bytes, 112, 0x1000);
    put32(&mut bytes, 120, e::PT_DYNAMIC);
    put32(&mut bytes, 124, 6);
    put64(&mut bytes, 128, 0x200);
    put64(&mut bytes, 136, BASE + 0x200);
    put64(&mut bytes, 152, (tags.len() as u64 + 1) * 16);
    put64(&mut bytes, 160, (tags.len() as u64 + 1) * 16);
    put64(&mut bytes, 168, 8);
    for (index, (tag, value)) in tags.iter().enumerate() {
        put64(&mut bytes, 0x200 + index * 16, *tag as u64);
        put64(&mut bytes, 0x208 + index * 16, *value);
    }
    bytes[0x300..0x300 + table.len()].copy_from_slice(table);
    for (index, value) in [
        layout::header_word(state::PERMANENT, flags::RAW, 2, cid::ONE_BYTE_STRING),
        u64::from(layout::string_hash([97])),
        1,
        97,
    ]
    .into_iter()
    .enumerate()
    {
        put64(&mut bytes, 0x400 + index * 8, value);
    }
    let names = b"\0dfimg\0.shstrtab\0";
    bytes[0x600..0x600 + names.len()].copy_from_slice(names);
    put32(&mut bytes, 0x840, 1);
    put32(&mut bytes, 0x844, e::SHT_PROGBITS);
    put64(&mut bytes, 0x848, 2);
    put64(&mut bytes, 0x850, IMAGE);
    put64(&mut bytes, 0x858, 0x400);
    put64(&mut bytes, 0x860, 32);
    put64(&mut bytes, 0x870, 8);
    put32(&mut bytes, 0x880, 7);
    put32(&mut bytes, 0x884, e::SHT_STRTAB);
    put64(&mut bytes, 0x898, 0x600);
    put64(&mut bytes, 0x8a0, names.len() as u64);
    put64(&mut bytes, 0x8b0, 1);
    bytes
}

fn rela(target: u64, kind: u32) -> Vec<u8> {
    [target, u64::from(kind), 0]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
fn rela_tags(size: u64) -> Vec<(i64, u64)> {
    vec![
        (e::DT_RELA, BASE + 0x300),
        (e::DT_RELASZ, size),
        (e::DT_RELAENT, 24),
    ]
}

#[test]
fn dynamic_relocations_cannot_hide_behind_missing_relocation_sections() {
    let bytes = fixture(&rela_tags(24), &rela(IMAGE, e::R_X86_64_RELATIVE), false);
    let generic = object::File::parse(bytes.as_slice()).unwrap();
    assert_eq!(generic.dynamic_relocations().map_or(0, Iterator::count), 0);
    let audit = inspect(&bytes).unwrap();
    assert_eq!(audit.tables[0].potential_writes, 1);
    assert_eq!(audit.tables[0].overlapping_writes, 1);
    assert_eq!(audit.tables[0].overlap_examples[0].address, IMAGE);
}

#[test]
fn complete_write_width_detects_straddling_and_respects_half_open_edges() {
    for (target, overlap) in [
        (IMAGE - 8, false),
        (IMAGE - 4, true),
        (IMAGE + 28, true),
        (IMAGE + 32, false),
    ] {
        let audit = inspect(&fixture(
            &rela_tags(24),
            &rela(target, e::R_X86_64_RELATIVE),
            false,
        ))
        .unwrap();
        assert_eq!(audit.tables[0].overlapping_writes, u64::from(overlap));
    }
    let audit = inspect(&fixture(
        &rela_tags(24),
        &rela(IMAGE - 8, e::R_X86_64_TLSDESC),
        false,
    ))
    .unwrap();
    assert_eq!(audit.tables[0].overlapping_writes, 1);
    assert_eq!(audit.tables[0].overlap_examples[0].width, Some(16));
}

#[test]
fn unknown_copy_and_extension_records_do_not_become_disjoint_classifications() {
    let mut tags = rela_tags(24);
    tags.push((0x6000_0011, BASE + 0x400));
    for kind in [e::R_X86_64_COPY, 0xffff_fffe] {
        let audit = inspect(&fixture(&tags, &rela(IMAGE, kind), false)).unwrap();
        assert_eq!(audit.tables[0].unclassified_writes, 1);
        assert_eq!(audit.tables[0].unknown_examples[0].width, None);
        assert_eq!(audit.unreviewed_dynamic_tags, [0x6000_0011]);
    }
}

#[test]
fn relr_bitmaps_even_unaligned_addresses_and_overflow_are_checked() {
    let tags = [
        (e::DT_RELR, BASE + 0x300),
        (e::DT_RELRSZ, 16),
        (e::DT_RELRENT, 8),
    ];
    let data = [IMAGE - 8, 3]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let audit = inspect(&fixture(&tags, &data, true)).unwrap();
    assert_eq!(audit.tables[0].potential_writes, 2);
    assert_eq!(audit.tables[0].overlapping_writes, 1);
    let mut extension_tags = tags.to_vec();
    extension_tags.push((-1, u64::MAX));
    assert_eq!(
        inspect(&fixture(&extension_tags, &data, true))
            .unwrap()
            .unreviewed_dynamic_tags,
        [-1]
    );
    assert!(relr(&[0; 9], |_| Ok(())).is_err());
    let mut decoded = Vec::new();
    relr(
        &[2u64.to_le_bytes(), 3u64.to_le_bytes()].concat(),
        |address| {
            decoded.push(address);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(decoded, [2, 10]);
    for data in [
        1u64.to_le_bytes().to_vec(),
        (u64::MAX - 1).to_le_bytes().to_vec(),
    ] {
        assert!(relr(&data, |_| Ok(())).is_err());
    }
}

#[test]
fn fast_relative_prefix_cannot_turn_none_into_a_hidden_write() {
    let mut tags = rela_tags(24);
    tags.push((e::DT_RELACOUNT, 1));
    assert!(inspect(&fixture(&tags, &rela(IMAGE, e::R_X86_64_NONE), false)).is_err());
    assert!(inspect(&fixture(&tags, &rela(IMAGE, e::R_X86_64_RELATIVE), false)).is_ok());
    tags[3].1 = 2;
    assert!(inspect(&fixture(&tags, &rela(IMAGE, e::R_X86_64_RELATIVE), false)).is_err());
}

#[test]
fn malformed_metadata_mappings_and_unterminated_dynamic_tables_are_refused() {
    let mut version = fixture(&[], &[], false);
    version[6] = 2;
    assert!(inspect(&version).is_err());
    let mut flags = fixture(&[], &[], false);
    put32(&mut flags, 48, 1);
    assert!(inspect(&flags).is_err());
    for tags in [
        vec![(e::DT_RELA, BASE + 0x300)],
        vec![
            (e::DT_RELA, BASE + 0x300),
            (e::DT_RELASZ, 24),
            (e::DT_RELAENT, 16),
        ],
        vec![
            (e::DT_RELA, BASE + 0x300),
            (e::DT_RELASZ, 25),
            (e::DT_RELAENT, 24),
        ],
        vec![(e::DT_RELA, BASE + 0x300), (e::DT_RELA, BASE + 0x300)],
    ] {
        assert!(inspect(&fixture(&tags, &rela(IMAGE, 8), false)).is_err());
    }
    let mut bytes = fixture(&rela_tags(24), &rela(IMAGE, 8), false);
    put64(&mut bytes, 0x230, e::DT_NEEDED as u64);
    assert!(inspect(&bytes).is_err());
    let mut bytes = fixture(&rela_tags(24), &rela(IMAGE, 8), false);
    put64(&mut bytes, 72, 8);
    put64(&mut bytes, 112, 1);
    put64(&mut bytes, 96, 0x8f8);
    assert!(inspect(&bytes).is_err());
    let mut bytes = fixture(&[], &[], false);
    let duplicate = bytes[64..120].to_vec();
    bytes[120..176].copy_from_slice(&duplicate);
    assert!(inspect(&bytes).is_err());
    let mut bytes = fixture(&rela_tags(24), &rela(IMAGE, 8), false);
    put64(&mut bytes, 0x208, BASE + 0x8f8);
    assert!(inspect(&bytes).is_err());
}

#[test]
fn rel_and_jmprel_use_dynamic_encoding_and_diagnostic_examples_are_bounded() {
    let data = [IMAGE, u64::from(e::R_X86_64_64)]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    let tags = [
        (e::DT_REL, BASE + 0x300),
        (e::DT_RELSZ, 16),
        (e::DT_RELENT, 16),
        (e::DT_JMPREL, BASE + 0x300),
        (e::DT_PLTRELSZ, 16),
        (e::DT_PLTREL, e::DT_REL as u64),
    ];
    let audit = inspect(&fixture(&tags, &data, false)).unwrap();
    assert_eq!(audit.tables.len(), 2);
    assert!(audit.tables.iter().all(|t| t.overlapping_writes == 1));
    let plt_only = [
        (e::DT_RELENT, 16),
        (e::DT_JMPREL, BASE + 0x300),
        (e::DT_PLTRELSZ, 16),
        (e::DT_PLTREL, e::DT_REL as u64),
    ];
    let audit = inspect(&fixture(&plt_only, &data, false)).unwrap();
    assert_eq!(audit.tables.len(), 1);
    assert_eq!(audit.tables[0].kind, "jmprel");
    assert_eq!(audit.tables[0].overlapping_writes, 1);
    let data = vec![rela(IMAGE, e::R_X86_64_RELATIVE); 30].concat();
    // A larger table is placed after the string region, still in the load span.
    let mut bytes = fixture(&rela_tags(0), &[], false);
    put64(&mut bytes, 0x208, BASE + 0x900);
    put64(&mut bytes, 0x218, data.len() as u64);
    bytes.extend_from_slice(&data);
    let size = bytes.len() as u64;
    put64(&mut bytes, 96, size);
    put64(&mut bytes, 104, size);
    let audit = inspect(&bytes).unwrap();
    assert_eq!(audit.tables[0].overlapping_writes, 30);
    assert_eq!(audit.tables[0].overlap_examples.len(), 16);
}

#[test]
#[ignore = "requires the previously recorded Linux desktop executable"]
fn recorded_linux_image_has_no_known_dynamic_relocation_writes_into_strings() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/desktop-matrix/cross-linux-arm64/explorer");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("{}: not recorded on this machine; skipped", path.display());
        return;
    };
    let audit = inspect(&bytes).unwrap();
    assert!(!audit.tables.is_empty());
    assert!(
        audit
            .tables
            .iter()
            .all(|t| t.overlapping_writes == 0 && t.unclassified_writes == 0)
    );
    println!("{}", serde_json::to_string(&audit).unwrap());
}

#[test]
#[ignore = "requires Clang and LLD for ARM64/x64 Linux PIE fixtures"]
fn fresh_pie_rela_and_packed_relr_tables_match_targets_and_detect_corruption() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("relocations.c");
    let object = directory.path().join("relocations.o");
    let output = directory.path().join("relocations");
    let header = layout::header_word(state::PERMANENT, flags::RAW, 2, cid::ONE_BYTE_STRING);
    let hash = layout::string_hash([97]);
    let pointers = ["&target"; 70].join(",");
    std::fs::write(&source,format!("__attribute__((section(\"dfimg\"),aligned(8))) const unsigned long long strings[] = {{{header},{hash},1,97}};\nint target; void *roots[70] = {{{pointers}}};\nconst void *entry(void) {{ return target ? (const void *)roots : (const void *)strings; }}\n")).unwrap();
    for (triple, architecture) in [
        ("aarch64-unknown-linux-gnu", "Aarch64"),
        ("x86_64-unknown-linux-gnu", "X86_64"),
    ] {
        let clang = crate::driver::NativeDriverOptions::default().clang;
        let result = std::process::Command::new(clang)
            .args(["-target", triple, "-fPIC", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        for packed in [false, true] {
            let mut command = std::process::Command::new(crate::linked_images::lld_tool("ld.lld"));
            command
                .args(["-pie", "--no-dynamic-linker", "-e", "entry"])
                .arg(&object)
                .arg("-o")
                .arg(&output);
            if packed {
                command.arg("--pack-dyn-relocs=relr");
            }
            let result = command.output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let bytes = std::fs::read(&output).unwrap();
            let image = super::super::inspect(&bytes).unwrap();
            assert_eq!(image.architecture, architecture);
            let audit = inspect(&bytes).unwrap();
            assert!(audit.unreviewed_dynamic_tags.is_empty());
            assert!(
                audit
                    .tables
                    .iter()
                    .all(|t| t.overlapping_writes == 0 && t.unclassified_writes == 0)
            );
            let table = audit
                .tables
                .iter()
                .find(|t| t.kind == if packed { "relr" } else { "rela" })
                .unwrap();
            assert!(table.potential_writes >= 70);
            if packed {
                assert!(table.encoded_entries < table.potential_writes);
            }
            // Locate the table through program headers independently of names.
            let mut at = None;
            for index in 0..u64::from(u16_at(&bytes, 56).unwrap()) {
                let p = u64_at(&bytes, 32).unwrap() + index * 56;
                if u32_at(&bytes, p).unwrap() == e::PT_LOAD {
                    let address = u64_at(&bytes, p + 16).unwrap();
                    let size = u64_at(&bytes, p + 32).unwrap();
                    if address <= table.address && table.address < address + size {
                        at = Some(u64_at(&bytes, p + 8).unwrap() + table.address - address);
                    }
                }
            }
            let mut altered = bytes.clone();
            put64(&mut altered, at.unwrap() as usize, image.section.address);
            let broken = inspect(&altered).unwrap();
            assert!(broken.tables.iter().any(|t| t.overlapping_writes > 0));
            println!(
                "{}",
                serde_json::json!({"fresh_target":triple,"packed_relr":packed,"image_digest":audit.image_digest,"encoded_entries":table.encoded_entries,"potential_writes":table.potential_writes,"mutated_target_detected":true,"executed":false})
            );
        }
    }
}
