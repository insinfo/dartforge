use super::*;

fn minimal_macho() -> Vec<u8> {
    [0xfeed_facfu32, 0x0100_000c, 0, 1, 0, 0, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn archive(entries: &[(&[u8], &[u8])], bsd: bool) -> Vec<u8> {
    let mut bytes = b"!<arch>\n".to_vec();
    for (name, data) in entries {
        let (header_name, payload) = if bsd {
            (format!("#1/{}", name.len()), [*name, *data].concat())
        } else {
            (
                format!("{}/", std::str::from_utf8(name).unwrap()),
                data.to_vec(),
            )
        };
        bytes.extend_from_slice(
            format!(
                "{header_name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                0,
                0,
                0,
                "100644",
                payload.len()
            )
            .as_bytes(),
        );
        bytes.extend_from_slice(&payload);
        if payload.len() % 2 != 0 {
            bytes.push(b'\n');
        }
    }
    bytes
}

#[test]
fn standalone_and_duplicate_archive_members_keep_identity_and_digest() {
    let object = minimal_macho();
    let single = inspect(&object).unwrap();
    assert!(!single.archive);
    assert_eq!(single.members.len(), 1);
    assert_eq!(single.members[0].architecture, "Aarch64");
    for bsd in [false, true] {
        let data = archive(&[(b"same.o", &object), (b"same.o", &object)], bsd);
        let result = inspect(&data).unwrap();
        assert!(result.archive);
        assert_eq!(result.members.len(), 2);
        assert_eq!(result.members[0].digest, result.members[1].digest);
        assert_ne!(result.members[0].input_range, result.members[1].input_range);
        for member in &result.members {
            let (start, len) = member.input_range;
            assert_eq!(
                blake3::hash(&data[start as usize..(start + len) as usize])
                    .to_hex()
                    .as_str(),
                member.digest
            );
        }
        assert_eq!(result.members[0].name, b"same.o");
        assert_eq!(result.members[0].digest, single.members[0].digest);
        assert_ne!(result.digest, single.digest);
    }
}

/// A COFF short import member (`IMPORT_OBJECT_HEADER`) of `symbol` from `dll`, x86-64.
fn short_import(symbol: &[u8], dll: &[u8]) -> Vec<u8> {
    let data = [symbol, b"\0", dll, b"\0"].concat();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0u16.to_le_bytes()); // Sig1
    bytes.extend_from_slice(&0xffffu16.to_le_bytes()); // Sig2
    bytes.extend_from_slice(&0u16.to_le_bytes()); // Version
    bytes.extend_from_slice(&0x8664u16.to_le_bytes()); // Machine: AMD64
    bytes.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes()); // SizeOfData
    bytes.extend_from_slice(&0u16.to_le_bytes()); // Ordinal/Hint
    bytes.extend_from_slice(&(1u16 << 2).to_le_bytes()); // IMPORT_OBJECT_CODE, by name
    bytes.extend_from_slice(&data);
    bytes
}

#[test]
fn coff_short_imports_are_members_without_sections() {
    let object = minimal_macho();
    let import = short_import(b"GetLastError", b"kernel32.dll");
    let data = archive(&[(b"kernel32.dll", &import), (b"valid.o", &object)], false);
    let result = inspect(&data).unwrap();
    assert_eq!(result.members.len(), 2);
    let member = &result.members[0];
    assert_eq!(member.format, "CoffImport");
    assert_eq!(member.architecture, "X86_64");
    assert!(member.sections.is_empty() && member.markers.is_empty());
    let marker = short_import(b"df.img.anchor", b"kernel32.dll");
    let data = archive(&[(b"kernel32.dll", &marker)], false);
    assert!(inspect(&data).is_err());
}

#[test]
fn bsd_member_names_are_bytes_and_do_not_need_utf8() {
    let object = minimal_macho();
    let data = archive(&[(b"nonutf8\xff.o", &object)], true);
    assert_eq!(inspect(&data).unwrap().members[0].name, b"nonutf8\xff.o");
}

#[test]
fn thin_bitcode_nested_and_invalid_late_members_return_no_inventory() {
    let object = minimal_macho();
    let nested = archive(&[(b"valid.o", &object)], false);
    for invalid in [
        b"".as_slice(),
        b"!<thin>\n",
        b"BC\xc0\xde",
        b"\xde\xc0\x17\x0b",
        b"garbage",
    ] {
        assert!(inspect(invalid).is_err());
    }
    for invalid in [b"garbage".as_slice(), nested.as_slice(), b"BC\xc0\xde"] {
        let data = archive(&[(b"valid.o", &object), (b"bad.o", invalid)], false);
        assert!(inspect(&data).is_err());
    }
    let mut truncated = archive(&[(b"last.o", &object)], false);
    truncated.pop();
    assert!(inspect(&truncated).is_err());
}

#[test]
fn failed_macho_load_command_iteration_is_not_an_empty_object() {
    let mut object = minimal_macho();
    object[16..20].copy_from_slice(&1u32.to_le_bytes());
    // The generic parser tolerates the missing command; our explicit pass must
    // propagate its error rather than publish a zero-section observation.
    assert!(object::File::parse(object.as_slice()).is_ok());
    assert!(inspect(&object).is_err());
}

#[test]
fn macho_linker_option_commands_are_recorded_as_uninterpreted_inputs() {
    let mut object = minimal_macho();
    object[16..20].copy_from_slice(&1u32.to_le_bytes());
    object[20..24].copy_from_slice(&16u32.to_le_bytes());
    let command: Vec<_> = [object::macho::LC_LINKER_OPTION, 16, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    object.extend_from_slice(&command);
    let result = inspect(&object).unwrap();
    assert_eq!(
        result.members[0].macho_linker_options,
        [blake3::hash(&command).to_hex().to_string()]
    );
}

#[test]
fn coff_directive_storage_is_observed_and_truncation_is_rejected() {
    // One AMD64 section, no optional header or symbols, four physical bytes.
    let mut bytes = vec![0u8; 64];
    bytes[..2].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[2..4].copy_from_slice(&1u16.to_le_bytes());
    bytes[20..28].copy_from_slice(b".drectve");
    bytes[36..40].copy_from_slice(&4u32.to_le_bytes());
    bytes[40..44].copy_from_slice(&60u32.to_le_bytes());
    bytes[56..60].copy_from_slice(&0xa00u32.to_le_bytes());
    bytes[60..].copy_from_slice(b"test");
    let inventory = inspect(&bytes).unwrap();
    let section = &inventory.members[0].sections[0];
    assert!(section.linker_directives);
    assert!(!section.managed_candidate);
    assert_eq!(section.file_range, Some((60, 4)));
    assert_eq!(
        section.content_digest,
        blake3::hash(b"test").to_hex().as_str()
    );
    bytes.pop();
    assert!(inspect(&bytes).is_err());
}

#[test]
fn managed_candidates_cover_macho_attributes_and_coff_subsections() {
    for name in [
        b"dfimg".as_slice(),
        b".dfimg",
        b".dfimg$m",
        b"__dfimg",
        b"__DATA_CONST,__dfimg,regular,no_dead_strip",
    ] {
        assert!(managed_section(name));
    }
    for name in [
        b"dfimg.other".as_slice(),
        b".dfimg_other",
        b"__DATA,__dfimg_other",
    ] {
        assert!(!managed_section(name));
    }
}

#[test]
fn common_marker_storage_is_not_mistaken_for_an_undefined_reference() {
    let name = b"__start_dfimg\0";
    let mut bytes = vec![0u8; 38];
    bytes[..2].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[8..12].copy_from_slice(&20u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&1u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&4u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&16u32.to_le_bytes());
    bytes[36] = 2; // External storage class, undefined section, nonzero size.
    bytes.extend_from_slice(&(4 + name.len() as u32).to_le_bytes());
    bytes.extend_from_slice(name);
    let inventory = inspect(&bytes).unwrap();
    let marker = &inventory.members[0].markers[0];
    assert!(marker.common);
    assert!(!marker.undefined && !marker.defined);
    assert_eq!(marker.section_kind, "Common");
}

#[test]
fn empty_archive_is_observed_without_claiming_a_link_image() {
    let result = inspect(b"!<arch>\n").unwrap();
    assert!(result.archive && result.members.is_empty());
}

#[test]
#[ignore = "requires Clang with Mach-O, ELF and COFF target support"]
fn real_objects_preserve_image_sections_and_marker_references_on_five_targets() {
    let directory = tempfile::tempdir().unwrap();
    for (target, section) in [
        ("aarch64-apple-darwin", "__DATA_CONST,__dfimg"),
        ("x86_64-apple-darwin", "__DATA_CONST,__dfimg"),
        ("aarch64-unknown-linux-gnu", "dfimg"),
        ("x86_64-unknown-linux-gnu", "dfimg"),
        ("x86_64-pc-windows-msvc", ".dfimg$m"),
    ] {
        let source = directory.path().join("fixture.c");
        let output = directory.path().join("fixture.o");
        std::fs::write(&source, format!(
            "__attribute__((section(\"{section}\"), aligned(8))) const long long image[2] = {{7, 9}};\nextern char boundary[] __asm__(\"__start_dfimg\");\nvoid *reference(void) {{ return boundary; }}\n"
        )).unwrap();
        let result =
            std::process::Command::new(crate::driver::NativeDriverOptions::default().clang)
                .args(["-target", target, "-c"])
                .arg(&source)
                .arg("-o")
                .arg(&output)
                .output()
                .unwrap();
        assert!(
            result.status.success(),
            "{target}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(&output).unwrap();
        let inventory = inspect(&bytes).unwrap();
        let member = &inventory.members[0];
        let image: Vec<_> = member
            .sections
            .iter()
            .filter(|s| s.managed_candidate)
            .collect();
        assert_eq!(image.len(), 1, "{target}: {member:?}");
        assert_eq!(image[0].size, 16);
        assert_eq!(image[0].stored_bytes, 16);
        assert_eq!(image[0].alignment, 8);
        assert!(
            member
                .markers
                .iter()
                .any(|m| !m.defined && m.name.ends_with(b"__start_dfimg"))
        );
        assert_eq!(
            image[0].content_digest,
            blake3::hash(&[7i64.to_le_bytes(), 9i64.to_le_bytes()].concat())
                .to_hex()
                .as_str()
        );
        assert!(inspect(&bytes[..bytes.len() / 2]).is_err());
        println!(
            "{}",
            serde_json::json!({"target":target,"digest":inventory.digest,"managed_sections":image.len(),"marker_references":member.markers.len(),"executed":false})
        );
    }
}

#[test]
#[ignore = "requires the three precompiled native runtime archives"]
fn selected_runtime_archives_are_enumerated_without_authorizing_images() {
    for (variant, runtime) in [
        (
            "main",
            crate::cache::RuntimeCache::get_or_compile().unwrap(),
        ),
        ("dll", crate::cache::RuntimeCache::for_dll().unwrap()),
        (
            "production",
            crate::cache::RuntimeCache::for_production().unwrap(),
        ),
    ] {
        let bytes = std::fs::read(&runtime.lib_path).unwrap();
        let inventory = inspect(&bytes).unwrap();
        assert!(inventory.archive && !inventory.members.is_empty());
        println!(
            "{}",
            serde_json::json!({"variant":variant,"path":runtime.lib_path,"digest":inventory.digest,
            "members":inventory.members.len(),"sections":inventory.members.iter().map(|m|m.sections.len()).sum::<usize>(),
            "managed_candidates":inventory.members.iter().flat_map(|m|&m.sections).filter(|s|s.managed_candidate).count(),
            "embedded_bitcode_candidates":inventory.members.iter().flat_map(|m|&m.sections).filter(|s|s.embedded_bitcode_candidate).count(),
            "embedded_payload_bytes":inventory.members.iter().flat_map(|m|&m.sections).filter(|s|s.embedded_bitcode_candidate).map(|s|s.stored_bytes).sum::<usize>(),
            "embedded_max_payload_bytes":inventory.members.iter().flat_map(|m|&m.sections).filter(|s|s.embedded_bitcode_candidate).map(|s|s.stored_bytes).max(),
            "directive_sections":inventory.members.iter().flat_map(|m|&m.sections).filter(|s|s.linker_directives).count(),
            "macho_linker_options":inventory.members.iter().map(|m|m.macho_linker_options.len()).sum::<usize>(),
            "marker_symbols":inventory.members.iter().map(|m|m.markers.len()).sum::<usize>(),"native_link_image_verified":false})
        );
    }
}

#[cfg(feature = "embedded-llvm")]
#[test]
#[ignore = "requires LLVM and the selected production runtime archive"]
fn selected_runtime_embedded_bitcode_is_inspected_from_bound_ranges() {
    let runtime = crate::cache::RuntimeCache::for_production().unwrap();
    let bytes = std::fs::read(&runtime.lib_path).unwrap();
    let inventory = inspect(&bytes).unwrap();
    let mut modules = 0;
    let mut managed_globals = 0;
    let mut managed_functions = 0;
    let mut assembly_sites = 0;
    let mut module_assembly = 0;
    for member in &inventory.members {
        let (offset, len) = member.input_range;
        let data = &bytes[offset as usize..(offset + len) as usize];
        assert_eq!(blake3::hash(data).to_hex().as_str(), member.digest);
        for section in member
            .sections
            .iter()
            .filter(|s| s.embedded_bitcode_candidate)
        {
            let (offset, len) = section
                .file_range
                .expect("embedded payload has physical storage");
            let bitcode = &data[offset as usize..(offset + len) as usize];
            assert_eq!(
                blake3::hash(bitcode).to_hex().as_str(),
                section.content_digest
            );
            let module =
                dartforge_llvm::inspect_image_sections("runtime embedded bitcode", bitcode)
                    .unwrap_or_else(|error| {
                        panic!(
                            "member {:?}, section {:?}: {error}",
                            member.name, section.name
                        )
                    });
            modules += 1;
            managed_globals += module
                .globals
                .iter()
                .filter(|g| managed_section(&g.section))
                .count();
            managed_functions += module
                .functions
                .iter()
                .filter(|g| managed_section(&g.section))
                .count();
            for asm in &module.inline_asm {
                println!(
                    "{}",
                    serde_json::json!({"runtime_member":member.name,"assembly":asm.template,"constraints":asm.constraints,"signature":asm.signature,"side_effects":asm.side_effects,"aligned_stack":asm.aligned_stack,"can_unwind":asm.can_unwind,"intel_dialect":asm.intel_dialect,"reviewed":false})
                );
            }
            assembly_sites += module.inline_asm.len();
            module_assembly += usize::from(!module.module_asm.is_empty());
        }
    }
    assert!(modules > 0);
    println!(
        "{}",
        serde_json::json!({"runtime_digest":inventory.digest,"embedded_modules":modules,"managed_globals":managed_globals,"managed_functions":managed_functions,"inline_assembly_sites":assembly_sites,"module_assembly_modules":module_assembly,"native_link_image_verified":false})
    );
}
