//! Compare verified source observations with the selected pre-LTO bitcode.
use super::*;
use dartforge_llvm::image_sections::{Bytes, Global, Initializer};

fn image_section(section: &[u8]) -> bool {
    let text = std::str::from_utf8(section).expect("SDK section names are UTF-8");
    assert!(
        !text.contains('\0'),
        "unreviewed embedded NUL in SDK section"
    );
    text == "dfimg"
        || text == ".dfimg"
        || text.starts_with(".dfimg$")
        || text.split(',').nth(1) == Some("__dfimg")
}

fn equivalent_bytes(left: &Bytes, right: &Bytes) -> bool {
    match (left, right) {
        (Bytes::Data(a), Bytes::Data(b)) => a == b,
        (Bytes::Zeroes(a), Bytes::Zeroes(b)) => a == b,
        (Bytes::Data(data), Bytes::Zeroes(size)) | (Bytes::Zeroes(size), Bytes::Data(data)) => {
            u64::try_from(data.len()).ok() == Some(*size) && data.iter().all(|byte| *byte == 0)
        }
    }
}

fn equivalent_global(actual: &Global, expected: &Global) -> bool {
    let data = match (&actual.initializer, &expected.initializer) {
        (
            Some(Initializer::HeaderAndBytes { words: a, bytes: x }),
            Some(Initializer::HeaderAndBytes { words: b, bytes: y }),
        ) => a == b && equivalent_bytes(x, y),
        (Some(Initializer::ZeroAnchor), Some(Initializer::ZeroAnchor)) => true,
        _ => false,
    };
    actual.name == expected.name
        && actual.section == expected.section
        && actual.constant == expected.constant
        && actual.link_once_odr == expected.link_once_odr
        && (!expected.hidden || actual.hidden)
        && !actual.declaration
        && !actual.externally_initialized
        && actual.alignment == expected.alignment
        && actual.address_space == expected.address_space
        && data
}

pub(super) fn compare_selected_objects(
    directory: &Path,
    key: &str,
    names: &[String],
    runtime: &Path,
    objects: &[std::path::PathBuf],
) {
    let mut publication =
        super::super::publication::read_verified(directory, key, true, names, runtime).unwrap();
    let raw: Payload = serde_json::from_value(publication["inventory"].take()).unwrap();
    for object in objects {
        let filename = object.file_name().unwrap().to_str().unwrap();
        let module = format!("dart:{}", object.file_stem().unwrap().to_str().unwrap());
        let source = raw.modules.iter().find(|m| m.module == module).unwrap();
        assert!(source.image_census.is_clean());
        let members: HashSet<_> = source
            .image_census
            .members
            .iter()
            .map(|m| m.symbol.as_str())
            .collect();
        let mut text = String::new();
        for global in raw
            .globals
            .iter()
            .filter(|g| g.module == module && members.contains(g.symbol.as_str()))
        {
            if global.initializer.contains(", comdat") && global.kind == "static_string" {
                text.push_str(&format!("$\"{}\" = comdat any\n", global.symbol));
            }
            text.push_str(&global.initializer);
            text.push('\n');
        }
        for line in &source.image_census.marker_lines {
            if line.starts_with('$')
                || crate::pruning::global_name(line)
                    .is_some_and(|(name, _, _)| !members.contains(name))
            {
                text.push_str(line);
                text.push('\n');
            }
        }
        let expected =
            dartforge_llvm::inspect_image_sections("source image definitions", text.as_bytes())
                .unwrap();
        let expected: HashMap<_, _> = expected
            .globals
            .into_iter()
            .filter(|g| image_section(&g.section))
            .map(|g| (g.name.clone(), g))
            .collect();
        assert_eq!(expected.len(), members.len());
        let bytes = std::fs::read(object).unwrap();
        let digest = publication["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["name"] == filename)
            .unwrap()["digest"]
            .as_str()
            .unwrap();
        assert_eq!(
            blake3::hash(&bytes).to_hex().as_str(),
            digest,
            "inspect exactly the selected immutable artifact bytes"
        );
        let actual = dartforge_llvm::inspect_image_sections(filename, &bytes).unwrap();
        let image: Vec<_> = actual
            .globals
            .iter()
            .filter(|g| image_section(&g.section))
            .collect();
        for global in &image {
            let source = expected
                .get(&global.name)
                .expect("no unobserved image definition in bitcode");
            assert!(
                equivalent_global(global, source),
                "{filename}: {global:?} != {source:?}"
            );
        }
        assert!(
            actual.functions.iter().all(|f| !image_section(&f.section)),
            "function in image section"
        );
        assert!(
            actual
                .aliases
                .iter()
                .chain(&actual.indirect_functions)
                .all(|name| !expected.contains_key(name)),
            "alias/ifunc cannot replace a checked image definition"
        );
        assert!(
            actual.module_asm.is_empty()
                || actual.module_asm == b".no_dead_strip __LLVM_StackMaps\n",
            "unreviewed module assembly: {:?}",
            actual.module_asm
        );
        for asm in &actual.inline_asm {
            assert_eq!(asm.template, b"mrs $0, TPIDRRO_EL0");
            assert_eq!(asm.constraints, b"=r");
            assert_eq!(asm.signature, "i64 ()");
            assert!(
                !asm.side_effects && !asm.aligned_stack && !asm.can_unwind && !asm.intel_dialect
            );
        }
        println!(
            "{}",
            serde_json::json!({"object":filename,"artifact_digest":digest,
            "source_image_definitions":expected.len(),"retained_image_definitions":image.len(),
            "removed_definitions":expected.len()-image.len(),"inline_assembly_sites":actual.inline_asm.len(),
            "object_triple":actual.triple,"native_link_image_verified":false})
        );
    }
}
