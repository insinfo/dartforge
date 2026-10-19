use super::*;
use dartforge_runtime::layout::{self, flags, state};

fn literal(class: i32, units: &[u16]) -> Vec<u8> {
    let content: Vec<u8> = if class == cid::ONE_BYTE_STRING {
        units
            .iter()
            .map(|unit| u8::try_from(*unit).unwrap())
            .collect()
    } else {
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
    };
    let size = content.len().div_ceil(8) * 8;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        &layout::header_word(state::PERMANENT, flags::RAW, 1 + size / 8, class).to_le_bytes(),
    );
    bytes.extend_from_slice(&u64::from(layout::string_hash(units.iter().copied())).to_le_bytes());
    bytes.extend_from_slice(&(units.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&content);
    bytes.resize(24 + size, 0);
    bytes
}

#[test]
fn complete_bytes_cover_zero_words_empty_latin1_utf16_and_surrogates() {
    let empty = literal(cid::ONE_BYTE_STRING, &[]);
    let latin = literal(cid::ONE_BYTE_STRING, &[0xe9, 0, b'a' as u16]);
    let utf16 = literal(cid::TWO_BYTE_STRING, &[0x3bb, 0xd83d, 0xde00, 0xd800]);
    let data = [vec![0; 16], empty, latin, utf16, vec![0; 16]].concat();
    let (strings, zeros) = blocks(&data).unwrap();
    assert_eq!(zeros, 4);
    assert_eq!(strings.iter().map(|s| s.cid).collect::<Vec<_>>(), [6, 6, 7]);
    assert_eq!(
        strings.iter().map(|s| s.length).collect::<Vec<_>>(),
        [0, 3, 4]
    );
    assert_eq!(
        strings.iter().map(|s| s.offset).collect::<Vec<_>>(),
        [16, 40, 72]
    );
    assert!(strings.iter().all(|s| s.key.len() == 32));
}

#[test]
fn altered_headers_hash_metadata_lengths_and_padding_are_refused() {
    let good = literal(cid::ONE_BYTE_STRING, &[b'a' as u16]);
    for index in [0, 1, 2, 4, 8, 12, 16, 25] {
        let mut altered = good.clone();
        altered[index] ^= 1;
        assert!(blocks(&altered).is_err(), "accepted mutation at {index}");
    }
    for count in 1..good.len() {
        assert!(blocks(&good[..count]).is_err());
    }
    let mut huge = good.clone();
    huge[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(blocks(&huge).is_err());
    assert!(blocks(&literal(cid::TWO_BYTE_STRING, &[0xe9])).is_err());
    assert!(blocks(&[0; 9]).is_err());
    let (strings, zeroes) = blocks(&[0; 16]).unwrap();
    assert!(strings.is_empty());
    assert_eq!(zeroes, 2);
}

#[test]
fn saturated_header_word_count_still_uses_bounded_actual_length() {
    let units = vec![b'x' as u16; 8 * u16::MAX as usize];
    let data = literal(cid::ONE_BYTE_STRING, &units);
    let (strings, zeros) = blocks(&data).unwrap();
    assert_eq!(zeros, 0);
    assert_eq!(strings.len(), 1);
    assert_eq!(strings[0].bytes, data.len());
    assert_eq!(strings[0].length, units.len() as u64);
    assert!(blocks(&data[..data.len() - 8]).is_err());
}

#[test]
fn relocatable_objects_and_unrecognized_data_do_not_become_image_records() {
    for data in [b"not an image".as_slice(), b"!<arch>\n", b"BC\xc0\xde"] {
        assert!(inspect(data).is_err());
    }
}

#[test]
#[ignore = "requires the previously recorded three-system desktop executables"]
fn recorded_desktop_images_are_read_back_without_authorizing_relocations() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for relative in [
        "target/desktop-matrix/macos-arm64/explorer",
        "target/desktop-matrix/cross-linux-arm64/explorer",
        "target/desktop-matrix/cross-windows-x64/explorer.exe",
    ] {
        let path = root.join(relative);
        let bytes = std::fs::read(&path).unwrap();
        let image = inspect(&bytes).unwrap_or_else(|error| panic!("{relative}: {error}"));
        assert!(!image.section.strings.is_empty());
        let mut cids = std::collections::BTreeMap::new();
        for string in &image.section.strings {
            *cids.entry(string.cid).or_insert(0usize) += 1;
        }
        println!(
            "{}",
            serde_json::json!({"path":relative,"digest":image.digest,"format":image.format,"architecture":image.architecture,"section_bytes":image.section.size,"section_digest":image.section.content_digest,"strings":image.section.strings.len(),"cids":cids,"zero_words":image.section.zero_words,"relocations_verified":false,"executed":false})
        );
    }
}

#[test]
#[ignore = "requires Clang and LLD tools for five native destinations"]
fn fresh_linked_sections_match_storage_and_reject_mutated_file_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let literal = literal(cid::ONE_BYTE_STRING, &[b'a' as u16, 0xe9]);
    let words: Vec<_> = literal
        .as_chunks::<8>()
        .0
        .iter()
        .map(|word| u64::from_le_bytes(*word).to_string())
        .collect();
    for (target, section, linker, options) in [
        (
            "aarch64-apple-darwin",
            "__DATA_CONST,__dfimg",
            "ld64.lld",
            vec![
                "-arch",
                "arm64",
                "-platform_version",
                "macos",
                "11.0",
                "11.0",
                "-e",
                "_entry",
            ],
        ),
        (
            "x86_64-apple-darwin",
            "__DATA_CONST,__dfimg",
            "ld64.lld",
            vec![
                "-arch",
                "x86_64",
                "-platform_version",
                "macos",
                "10.12",
                "10.12",
                "-e",
                "_entry",
            ],
        ),
        (
            "aarch64-unknown-linux-gnu",
            "dfimg",
            "ld.lld",
            vec!["-e", "entry"],
        ),
        (
            "x86_64-unknown-linux-gnu",
            "dfimg",
            "ld.lld",
            vec!["-e", "entry"],
        ),
        (
            "x86_64-pc-windows-msvc",
            ".dfimg$m",
            "lld-link",
            vec!["/entry:entry", "/subsystem:console", "/nodefaultlib"],
        ),
    ] {
        let source = directory.path().join("literal.c");
        let object = directory.path().join("literal.o");
        let output = directory.path().join("image");
        std::fs::write(&source, format!("__attribute__((section(\"{section}\"), aligned(8))) const unsigned long long strings[] = {{{}}};\nconst void *entry(void) {{ return strings; }}\n",words.join(","))).unwrap();
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
        assert!(inspect(&std::fs::read(&object).unwrap()).is_err());
        let mut command = std::process::Command::new(linker);
        command.args(options).arg(&object);
        if linker == "lld-link" {
            command.arg(format!("/out:{}", output.display()));
        } else {
            command.arg("-o").arg(&output);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(&output).unwrap();
        let observed = inspect(&bytes).unwrap();
        assert_eq!(observed.section.strings.len(), 1);
        assert_eq!(observed.section.strings[0].length, 2);
        assert_eq!(
            observed.section.content_digest,
            blake3::hash(&literal).to_hex().as_str()
        );
        let file = object::File::parse(bytes.as_slice()).unwrap();
        let native = file
            .sections()
            .find(|s| s.name_bytes().unwrap() == observed.section.name)
            .unwrap();
        let (start, _) = native.file_range().unwrap();
        let mut altered = bytes.clone();
        altered[start as usize + 8] ^= 1;
        assert!(inspect(&altered).is_err());
        println!(
            "{}",
            serde_json::json!({"fresh_target":target,"image_digest":observed.digest,"strings":1,"mutated_hash_refused":true,"executed":false})
        );
    }
}
