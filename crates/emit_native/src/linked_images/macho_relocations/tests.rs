use super::*;

fn table(kind: &'static str) -> Table {
    Table {
        kind,
        offset: 0,
        bytes: 0,
        digest: String::new(),
        opcodes: 0,
        potential_writes: 0,
        overlapping_writes: 0,
        types: BTreeMap::new(),
        overlap_examples: vec![],
    }
}
fn segment() -> Segment {
    Segment {
        name: b"__DATA_CONST".to_vec(),
        address: 0x2000,
        memory: 0x1000,
        offset: 0,
        stored: 0x1000,
        maximum: 3,
        initial: 3,
        flags: 0,
        header_bytes: 0,
    }
}
fn put32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn leb_bytes(mut n: u64) -> Vec<u8> {
    let mut result = vec![];
    loop {
        let byte = (n & 127) as u8;
        n >>= 7;
        result.push(byte | if n == 0 { 0 } else { 128 });
        if n == 0 {
            return result;
        }
    }
}

/// Small linked image with a real permanent string and independently addressable
/// dyld opcode tables. No executable code or loader is run for mutations.
fn image(streams: [&[u8]; 4]) -> Vec<u8> {
    let mut bytes = vec![0u8; 0x500];
    put32(&mut bytes, 0, m::MH_MAGIC_64);
    put32(&mut bytes, 4, m::CPU_TYPE_ARM64);
    put32(&mut bytes, 8, 0);
    put32(&mut bytes, 12, m::MH_EXECUTE);
    put32(&mut bytes, 16, 4);
    put32(&mut bytes, 20, 344);
    put32(&mut bytes, 24, m::MH_DYLDLINK | m::MH_PIE);
    for (command, name, address, offset, stored, sections, size) in [
        (32, b"__TEXT".as_slice(), 0x1000, 0, 0x200, 0, 72),
        (
            104,
            b"__DATA_CONST".as_slice(),
            0x2000,
            0x200,
            0x100,
            1,
            152,
        ),
        (256, b"__LINKEDIT".as_slice(), 0x3000, 0x300, 0x200, 0, 72),
    ] {
        put32(&mut bytes, command, m::LC_SEGMENT_64);
        put32(&mut bytes, command + 4, size);
        bytes[command + 8..command + 8 + name.len()].copy_from_slice(name);
        put64(&mut bytes, command + 24, address);
        put64(&mut bytes, command + 32, 0x1000);
        put64(&mut bytes, command + 40, offset);
        put64(&mut bytes, command + 48, stored);
        put32(&mut bytes, command + 56, 3);
        put32(&mut bytes, command + 60, 3);
        put32(&mut bytes, command + 64, sections);
    }
    let section = 176;
    bytes[section..section + 7].copy_from_slice(b"__dfimg");
    bytes[section + 16..section + 28].copy_from_slice(b"__DATA_CONST");
    put64(&mut bytes, section + 32, 0x2040);
    put64(&mut bytes, section + 40, 32);
    put32(&mut bytes, section + 48, 0x240);
    put32(&mut bytes, section + 52, 3);
    let units = [b'a' as u16];
    let header = dartforge_runtime::layout::header_word(
        dartforge_runtime::layout::state::PERMANENT,
        dartforge_runtime::layout::flags::RAW,
        2,
        dartforge_runtime::layout::cid::ONE_BYTE_STRING,
    );
    put64(&mut bytes, 0x240, header);
    put64(
        &mut bytes,
        0x248,
        u64::from(dartforge_runtime::layout::string_hash(units)),
    );
    put64(&mut bytes, 0x250, 1);
    bytes[0x258] = b'a';
    put32(&mut bytes, 328, m::LC_DYLD_INFO_ONLY);
    put32(&mut bytes, 332, 48);
    let mut position = 0x300;
    for (index, stream) in streams.iter().enumerate() {
        put32(&mut bytes, 336 + index * 8, position as u32);
        put32(&mut bytes, 340 + index * 8, stream.len() as u32);
        bytes[position..position + stream.len()].copy_from_slice(stream);
        position += stream.len();
    }
    bytes
}

#[test]
fn valid_file_bytes_do_not_prove_absence_of_loader_writes() {
    let bytes = image([&[0x11, 0x21, 0x40, 0x51, 0], &[], &[], &[]]);
    assert_eq!(
        super::super::inspect(&bytes).unwrap().section.strings.len(),
        1
    );
    let audit = inspect(&bytes).unwrap();
    assert_eq!(audit.managed_address, 0x2040);
    assert_eq!(audit.tables[0].potential_writes, 1);
    assert_eq!(audit.tables[0].overlapping_writes, 1);
    assert_eq!(audit.tables[0].overlap_examples[0].address, 0x2040);
    let clear = image([&[0x11, 0x21, 0x20, 0x51, 0], &[], &[], &[]]);
    assert_eq!(inspect(&clear).unwrap().tables[0].overlapping_writes, 0);
}

#[test]
fn arithmetic_runs_match_an_independent_per_write_overlap_oracle() {
    for kind in [1, 2, 3] {
        for start in 0..80 {
            for count in 0..20 {
                for skip in [0, 1, 7, 9] {
                    let mut stats = table("rebase");
                    let mut state = State {
                        segment: Some(0),
                        offset: start,
                        kind,
                        ..State::default()
                    };
                    let width = if kind == 1 { 8 } else { 4 };
                    let expected = (0..count)
                        .filter(|i| {
                            let address = 0x2000 + start + i * (8 + skip);
                            address < 0x2050 && address + width > 0x2040
                        })
                        .count() as u64;
                    run(
                        &mut stats,
                        &[segment()],
                        &mut state,
                        count,
                        skip,
                        (0x2040, 0x2050),
                    )
                    .unwrap();
                    assert_eq!(stats.potential_writes, count);
                    assert_eq!(
                        stats.overlapping_writes, expected,
                        "kind={kind}, start={start}, count={count}, skip={skip}"
                    );
                    assert_eq!(stats.overlap_examples.len() as u64, expected.min(16));
                    assert_eq!(state.offset, start + count * (8 + skip));
                }
            }
        }
    }
}

#[test]
fn all_classic_rebase_run_and_address_opcodes_cover_the_same_writes() {
    for program in [
        vec![0x11, 0x20, 0x40, 0x52, 0],
        vec![0x11, 0x20, 0x40, 0x60, 2, 0],
        vec![0x11, 0x20, 0x38, 0x41, 0x51, 0x51, 0],
        vec![0x11, 0x20, 0x30, 0x30, 0x10, 0x51, 0x51, 0],
        vec![0x11, 0x20, 0x40, 0x70, 0, 0x70, 0, 0],
        vec![0x11, 0x20, 0x40, 0x80, 2, 0, 0],
    ] {
        let mut stats = table("rebase");
        decode(&program, &mut stats, &[segment()], (0x2040, 0x2050)).unwrap();
        assert_eq!(
            (stats.potential_writes, stats.overlapping_writes),
            (2, 2),
            "{program:?}"
        );
    }
}

#[test]
fn bind_weak_and_lazy_programs_include_cross_boundary_writes_and_later_records() {
    let prefix = [0x11, 0x40, b'x', 0, 0x51, 0x70, 0x3c];
    for operation in [&[0x90][..], &[0xa0, 0], &[0xb0], &[0xc0, 1, 0]] {
        let program = [&prefix[..], operation, &[0]].concat();
        let mut stats = table("bind");
        decode(&program, &mut stats, &[segment()], (0x2040, 0x2050)).unwrap();
        assert_eq!(stats.overlapping_writes, 1);
        assert_eq!(stats.overlap_examples[0].address, 0x203c);
    }
    let weak = [0x40, b'x', 0, 0x51, 0x70, 0x40, 0x90, 0];
    let mut stats = table("weak_bind");
    decode(&weak, &mut stats, &[segment()], (0x2040, 0x2050)).unwrap();
    assert_eq!(stats.potential_writes, 1);
    let lazy = [
        0x11, 0x40, b'x', 0, 0x70, 0x20, 0x90, 0, 0x70, 0x40, 0x90, 0,
    ];
    let mut stats = table("lazy_bind");
    decode(&lazy, &mut stats, &[segment()], (0x2040, 0x2050)).unwrap();
    assert_eq!((stats.potential_writes, stats.overlapping_writes), (2, 1));
}

#[test]
fn huge_runs_are_counted_without_expansion_and_examples_stay_bounded() {
    let mut huge = segment();
    huge.address = 0;
    huge.memory = 8_000_000_008;
    let mut stats = table("rebase");
    let program = [&[0x11, 0x20, 0, 0x60][..], &leb_bytes(1_000_000_000), &[0]].concat();
    decode(&program, &mut stats, &[huge], (0, 8_000_000_000)).unwrap();
    assert_eq!(stats.potential_writes, 1_000_000_000);
    assert_eq!(stats.overlapping_writes, 1_000_000_000);
    assert_eq!(stats.overlap_examples.len(), 16);
}

#[test]
fn missing_state_unknown_forms_and_truncated_operands_never_become_empty_tables() {
    for program in [
        vec![0x51, 0],
        vec![0x11, 0x21, 0, 0x51, 0],
        vec![0x14, 0x20, 0, 0x51, 0],
        vec![0x11, 0x20, 0x80],
        vec![0x11, 0x20, 0, 0x51],
        vec![0, 0x51],
        vec![0x90, 0],
        vec![0x01],
    ] {
        assert!(
            decode(
                &program,
                &mut table("rebase"),
                &[segment()],
                (0x2040, 0x2050)
            )
            .is_err(),
            "{program:?}"
        );
    }
    for program in [
        vec![0xd0, 0],
        vec![0x40, b'x'],
        vec![0x40, b'x', 0, 0x51, 0x70, 0, 0x90, 0],
        vec![0x11, 0x51, 0x70, 0, 0x90, 0],
        vec![0x32, 0],
        vec![0x42, b'x', 0],
        vec![0x60, 0x80],
    ] {
        assert!(
            decode(&program, &mut table("bind"), &[segment()], (0x2040, 0x2050)).is_err(),
            "{program:?}"
        );
    }
    let overflow = [&[0x11, 0x20, 0, 0x60][..], &leb_bytes(u64::MAX), &[0]].concat();
    assert!(decode(&overflow, &mut table("rebase"), &[segment()], (0, 16)).is_err());
}

#[test]
fn leb_domains_include_signed_extremes_and_reject_overflow_and_truncation() {
    for n in [0, 1, 127, 128, u32::MAX as u64, u64::MAX] {
        assert_eq!(leb(&leb_bytes(n), &mut 0, false).unwrap(), n);
    }
    assert_eq!(leb(&[0x7f], &mut 0, true).unwrap() as i64, -1);
    assert_eq!(
        leb(
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x7f],
            &mut 0,
            true
        )
        .unwrap() as i64,
        i64::MIN
    );
    for signed in [false, true] {
        for bytes in [
            vec![0x80],
            vec![0xff; 11],
            vec![0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02],
        ] {
            assert!(leb(&bytes, &mut 0, signed).is_err());
        }
    }
}

#[test]
fn complete_image_checks_refuse_ambiguous_mapping_and_unsupported_fixup_sources() {
    let original = image([&[0], &[], &[], &[]]);
    assert!(inspect(&original).is_ok());
    for (offset, value) in [
        (20, 352),
        (332, 40),
        (328, m::LC_DYLD_CHAINED_FIXUPS),
        (176 + 60, 1),
        (336, 0x100),
        (340, 0x400),
        (104 + 40, 0x100),
    ] {
        let mut bytes = original.clone();
        put32(&mut bytes, offset, value);
        assert!(inspect(&bytes).is_err(), "offset={offset}, value={value}");
    }
    let mut bytes = original.clone();
    put64(&mut bytes, 176 + 32, 0x2048);
    assert!(inspect(&bytes).is_err(), "section/file offset disagreement");
    let mut bytes = original.clone();
    put64(&mut bytes, 104 + 24, 0x1000);
    assert!(inspect(&bytes).is_err(), "overlapping VM mappings");
    for size in [0, 31, 80, 375, 0x250, 0x400] {
        assert!(inspect(&original[..size]).is_err(), "length={size}");
    }
}

#[test]
#[ignore = "requires the recorded macOS File Explorer executable"]
fn recorded_macos_explorer_declares_no_writes_into_managed_string_storage() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/desktop-matrix/macos-arm64/explorer");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("{}: not recorded on this machine; skipped", path.display());
        return;
    };
    let audit = inspect(&bytes).unwrap();
    assert!(audit.tables.iter().any(|t| t.potential_writes > 0));
    assert!(audit.tables.iter().all(|t| t.overlapping_writes == 0));
    println!("{}", serde_json::to_string(&audit).unwrap());
}

#[test]
fn all_four_tables_and_legacy_relocation_sources_remain_distinct() {
    let rebase = [0x11, 0x21, 0x40, 0x51, 0];
    let bind = [0x11, 0x40, b'x', 0, 0x51, 0x71, 0x40, 0x90, 0];
    let weak = [0x40, b'x', 0, 0x51, 0x71, 0x40, 0x90, 0];
    let lazy = [0x11, 0x40, b'x', 0, 0x71, 0x40, 0x90, 0];
    let bytes = image([&rebase, &bind, &weak, &lazy]);
    let audit = inspect(&bytes).unwrap();
    assert!(
        audit
            .tables
            .iter()
            .all(|table| table.overlapping_writes == 1)
    );
    assert_eq!(
        audit.tables.iter().map(|t| t.kind).collect::<Vec<_>>(),
        ["rebase", "bind", "weak_bind", "lazy_bind"]
    );
    let mut alias = bytes.clone();
    put32(&mut alias, 352, field(&bytes, 344).unwrap());
    put32(&mut alias, 356, field(&bytes, 348).unwrap());
    assert!(
        inspect(&alias)
            .unwrap_err()
            .contains("overlapping Mach-O dyld opcode tables")
    );
    let mut legacy = image([&[], &[], &[], &[]]);
    put32(&mut legacy, 16, 5);
    put32(&mut legacy, 20, 424);
    put32(&mut legacy, 376, m::LC_DYSYMTAB);
    put32(&mut legacy, 380, 80);
    assert!(inspect(&legacy).is_ok());
    for count in [376 + 68, 376 + 76] {
        let mut bad = legacy.clone();
        put32(&mut bad, count, 1);
        assert!(inspect(&bad).is_err(), "legacy count at {count}");
    }
}

#[test]
#[ignore = "requires Clang and ld64.lld for fresh ARM64/x64 linked images"]
fn fresh_classic_and_chained_images_cover_real_rebases_and_binds() {
    let directory = tempfile::tempdir().unwrap();
    let literal = image([&[], &[], &[], &[]]);
    let words: Vec<_> = literal[0x240..0x260]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|word| u64::from_le_bytes(*word).to_string())
        .collect();
    let source = directory.path().join("fixups.c");
    std::fs::write(&source,format!("__attribute__((section(\"__DATA_CONST,__dfimg\"), aligned(8))) const unsigned long long strings[] = {{{}}};\nconst void *global = strings;\nextern const void *external(void);\nconst void *entry(void) {{ return global ? external() : strings; }}\n",words.join(","))).unwrap();
    for (target, architecture) in [
        ("aarch64-apple-darwin", "arm64"),
        ("x86_64-apple-darwin", "x86_64"),
    ] {
        let object = directory.path().join(format!("{architecture}.o"));
        let result =
            std::process::Command::new(crate::driver::NativeDriverOptions::default().clang)
                .args(["-target", target, "-c"])
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
        for chained in [false, true] {
            let output = directory.path().join(format!("{architecture}-{chained}"));
            let result = std::process::Command::new("ld64.lld")
                .args([
                    "-arch",
                    architecture,
                    "-platform_version",
                    "macos",
                    "13.0",
                    "13.0",
                    "-e",
                    "_entry",
                    "-undefined",
                    "dynamic_lookup",
                    if chained {
                        "-fixup_chains"
                    } else {
                        "-no_fixup_chains"
                    },
                ])
                .arg(&object)
                .arg("-o")
                .arg(&output)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let bytes = std::fs::read(&output).unwrap();
            if chained {
                let audit = inspect(&bytes).unwrap();
                let chains = audit.chained.as_ref().unwrap();
                assert!(chains.segments.iter().any(|s| s.rebases > 0));
                assert!(chains.segments.iter().any(|s| s.binds > 0));
                assert!(chains.segments.iter().all(|s| s.overlapping_writes == 0));
                println!(
                    "{}",
                    serde_json::json!({"fresh_target":target,"chained":true,"executed":false,"audit":audit})
                );
            } else {
                let audit = inspect(&bytes).unwrap();
                assert!(
                    audit
                        .tables
                        .iter()
                        .any(|t| t.kind == "rebase" && t.potential_writes > 0)
                );
                assert!(
                    audit
                        .tables
                        .iter()
                        .any(|t| t.kind.contains("bind") && t.potential_writes > 0)
                );
                assert!(audit.tables.iter().all(|t| t.overlapping_writes == 0));
                println!(
                    "{}",
                    serde_json::json!({"fresh_target":target,"executed":false,"audit":audit})
                );
            }
        }
    }
}

#[test]
fn nonlinear_mappings_and_writes_to_loader_control_data_are_refused() {
    let mut accepted = Vec::new();
    for flag in [1, 2, 8, 0x80000000] {
        let mut bytes = image([&[0], &[], &[], &[]]);
        put32(&mut bytes, 104 + 68, flag);
        if inspect(&bytes).is_ok() {
            accepted.push(format!("segment flag {flag:#x}"));
        }
    }
    for (index, offset) in [(0, 0x20), (2, 0x10)] {
        let bytes = image([&[0x11, 0x20 | index, offset, 0x51, 0], &[], &[], &[]]);
        if inspect(&bytes).is_ok() {
            accepted.push(format!("control write in segment {index}"));
        }
    }
    assert!(
        accepted.is_empty(),
        "unproved mappings/control writes accepted: {accepted:?}"
    );
}

mod chained_tests;
