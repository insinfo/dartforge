//! Independent encoded metadata fixtures; no mutated image is executed.
use super::*;

fn put16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn chained(format: u16, starts: &[u16]) -> Vec<u8> {
    let mut bytes = image([&[], &[], &[], &[]]);
    put32(&mut bytes, 20, 312);
    put32(&mut bytes, 328, m::LC_DYLD_CHAINED_FIXUPS);
    put32(&mut bytes, 332, 16);
    put32(&mut bytes, 336, 0x300);
    bytes[0x300..].fill(0);
    let record_size = 22 + starts.len() * 2;
    let imports = 48 + record_size;
    let symbols = imports + 4;
    let size = symbols + 7;
    put32(&mut bytes, 340, size as u32);
    for (at, value) in [
        (4, 32),
        (8, imports as u32),
        (12, symbols as u32),
        (16, 1),
        (20, 1),
        (32, 3),
        (40, 16),
        (48, record_size as u32),
    ] {
        put32(&mut bytes, 0x300 + at, value);
    }
    put16(&mut bytes, 0x300 + 52, 4096);
    put16(&mut bytes, 0x300 + 54, format);
    put64(&mut bytes, 0x300 + 56, 0x1000);
    put16(&mut bytes, 0x300 + 68, 1);
    for (index, start) in starts.iter().enumerate() {
        put16(&mut bytes, 0x300 + 70 + index * 2, *start);
    }
    put32(&mut bytes, 0x300 + imports, 1);
    bytes[0x300 + symbols..0x300 + size].copy_from_slice(b"symbol\0");
    bytes
}

#[test]
fn chains_detect_writes_into_valid_static_headers_and_across_the_section_end() {
    for start in [0x40, 0x5c] {
        let bytes = chained(2, &[start]);
        assert!(super::super::super::inspect(&bytes).is_ok());
        let audit = inspect(&bytes).unwrap();
        let segment = &audit.chained.unwrap().segments[0];
        assert_eq!(
            (segment.rebases, segment.binds, segment.overlapping_writes),
            (1, 0, 1)
        );
        assert_eq!(
            segment.overlap_examples[0].address,
            0x2000 + u64::from(start)
        );
    }
}

#[test]
fn normal_offset_authenticated_and_wide_ordinal_formats_follow_exact_chain_links() {
    for format in [1, 2, 6, 9, 12] {
        let mut bytes = chained(format, &[0x20]);
        let stride = if matches!(format, 2 | 6) { 4 } else { 8 };
        let binding = if matches!(format, 2 | 6) {
            1 << 63
        } else {
            (1 << 62) | (1 << 63)
        };
        put64(&mut bytes, 0x220, binding | ((8 / stride) << 51));
        let audit = inspect(&bytes).unwrap();
        let segment = &audit.chained.unwrap().segments[0];
        assert_eq!(
            (segment.rebases, segment.binds, segment.overlapping_writes),
            (1, 1, 0),
            "format={format}"
        );
        assert_eq!(
            segment.authenticated_writes,
            u64::from(!matches!(format, 2 | 6))
        );
    }
    let mut wrong_cpu = chained(9, &[0x20]);
    put32(&mut wrong_cpu, 4, m::CPU_TYPE_X86_64);
    assert!(inspect(&wrong_cpu).is_err());
}

#[test]
fn page_multi_starts_cover_every_chain_and_reject_aliases_and_missing_terminators() {
    let bytes = chained(2, &[0x8001, 0x20, 0x8040]);
    let audit = inspect(&bytes).unwrap();
    let segment = &audit.chained.unwrap().segments[0];
    assert_eq!(
        (segment.chains, segment.rebases, segment.overlapping_writes),
        (2, 2, 1)
    );
    for starts in [
        vec![0x8000, 0x8020],
        vec![0x8001, 0x20],
        vec![0x8001, 0x20, 0x8020],
        vec![0x8001, 0x20, 0x8024],
    ] {
        assert!(inspect(&chained(2, &starts)).is_err(), "{starts:?}");
    }
    let empty = inspect(&chained(2, &[0xffff])).unwrap();
    assert_eq!(empty.chained.unwrap().segments[0].chains, 0);
}

#[test]
fn malformed_chain_metadata_never_returns_a_partial_clean_audit() {
    let original = chained(2, &[0x20]);
    for (offset, value) in [
        (0, 1),
        (4, 0),
        (8, 0),
        (12, 1000),
        (16, u32::MAX),
        (20, 99),
        (24, 1),
        (32, 4),
        (40, u32::MAX),
        (48, 23),
        (64, 1),
    ] {
        let mut bytes = original.clone();
        put32(&mut bytes, 0x300 + offset, value);
        assert!(inspect(&bytes).is_err(), "header/record offset={offset}");
    }
    for (offset, value) in [(52, 512), (54, 3), (68, 2), (70, 0xffc)] {
        let mut bytes = original.clone();
        put16(&mut bytes, 0x300 + offset, value);
        assert!(inspect(&bytes).is_err(), "half offset={offset}");
    }
    let mut bytes = original.clone();
    put64(&mut bytes, 0x338, 0x1008);
    assert!(inspect(&bytes).is_err(), "segment offset mapping");
    let mut bytes = original.clone();
    put64(&mut bytes, 0x220, 0xfff << 51);
    assert!(inspect(&bytes).is_err(), "next pointer leaves page");
    let mut bytes = original.clone();
    put64(&mut bytes, 0x220, (1 << 63) | 1);
    assert!(inspect(&bytes).is_err(), "bind ordinal exceeds one import");
    let mut bytes = original.clone();
    bytes[0x300 + 82] = b'x';
    assert!(inspect(&bytes).is_err(), "unterminated symbol");
    for size in 0..83 {
        let mut bytes = original.clone();
        put32(&mut bytes, 340, size);
        assert!(inspect(&bytes).is_err(), "payload size={size}");
    }
}

#[test]
fn all_import_layouts_validate_names_bounds_reserved_bits_and_terminators() {
    for (format, width) in [(1, 4), (2, 8), (3, 16)] {
        let mut bytes = chained(2, &[0x20]);
        let symbols = 72 + width;
        put32(&mut bytes, 0x300 + 12, symbols);
        put32(&mut bytes, 0x300 + 20, format);
        put32(&mut bytes, 340, symbols + 7);
        bytes[0x348..].fill(0);
        put32(&mut bytes, 0x348, 1);
        bytes[0x300 + symbols as usize..0x307 + symbols as usize].copy_from_slice(b"symbol\0");
        put64(&mut bytes, 0x220, 1 << 63);
        assert_eq!(
            inspect(&bytes).unwrap().chained.unwrap().segments[0].binds,
            1
        );
        let mut bad = bytes.clone();
        if format == 3 {
            put64(&mut bad, 0x348, 1 | (999 << 32));
        } else {
            put32(&mut bad, 0x348, 1 | (999 << 9));
        }
        assert!(
            inspect(&bad).is_err(),
            "name bounds for import format {format}"
        );
        if format == 3 {
            let mut bad = bytes.clone();
            put64(&mut bad, 0x348, 1 | (1 << 17));
            assert!(inspect(&bad).is_err());
        }
    }
}

#[test]
fn chains_cannot_rewrite_loader_headers_or_linkedit_and_commands_cannot_conflict() {
    for index in [0, 2] {
        let mut bytes = chained(2, &[0x20]);
        put32(&mut bytes, 0x328, 0);
        put32(&mut bytes, 0x324 + index * 4, 16);
        put64(&mut bytes, 0x338, index as u64 * 0x1000);
        let message = inspect(&bytes).unwrap_err();
        assert!(
            message.contains("control metadata") || message.contains("LINKEDIT"),
            "{message}"
        );
    }
    let mut duplicate = chained(2, &[0x20]);
    put32(&mut duplicate, 16, 5);
    put32(&mut duplicate, 20, 328);
    put32(&mut duplicate, 344, m::LC_DYLD_CHAINED_FIXUPS);
    put32(&mut duplicate, 348, 16);
    assert!(inspect(&duplicate).unwrap_err().contains("duplicate"));
    let mut mixed = chained(2, &[0x20]);
    mixed[344..392].fill(0);
    put32(&mut mixed, 16, 5);
    put32(&mut mixed, 20, 360);
    put32(&mut mixed, 344, m::LC_DYLD_INFO_ONLY);
    put32(&mut mixed, 348, 48);
    put32(&mut mixed, 352, 0x300);
    put32(&mut mixed, 356, 1);
    assert!(
        inspect(&mixed)
            .unwrap_err()
            .contains("mixed classic and chained")
    );
}
