use super::*;

fn linkage(output: &Path) -> Linkage<'_> {
    Linkage {
        product: Product::Executable,
        entries: vec![],
        origin_rpath: false,
        lto: false,
        cpu: None,
        prune: false,
        keep_debug_info: false,
        exit: output,
    }
}

#[test]
fn explicit_policy_keeps_default_commands_and_rejects_invalid_values() {
    assert!(!parse_relr(None).unwrap());
    assert!(!parse_relr(Some("0".as_ref())).unwrap());
    assert!(parse_relr(Some("1".as_ref())).unwrap());
    for value in ["", "yes", "2", "true"] {
        assert!(parse_relr(Some(value.as_ref())).is_err());
    }
    let sysroot = SysrootLinux {
        files: sysroot_files()
            .iter()
            .map(|name| Path::new("sysroot").join(name))
            .collect(),
    };
    let output = Path::new("app");
    let standard =
        statement_with_relr(Path::new("ld.lld"), &sysroot, &linkage(output), false).unwrap();
    let packed =
        statement_with_relr(Path::new("ld.lld"), &sysroot, &linkage(output), true).unwrap();
    let plain: Vec<_> = standard.get_args().collect();
    let enabled: Vec<_> = packed.get_args().collect();
    assert_eq!(enabled[0], "-z");
    assert_eq!(enabled[1], "pack-relative-relocs");
    assert_eq!(&enabled[2..], plain.as_slice());
    assert!(!enabled.contains(&std::ffi::OsStr::new("--pack-dyn-relocs=relr")));
}

#[test]
#[ignore = "requires a matching Linux link sysroot and Clang/LLD"]
fn glibc_fixture_links_both_modes_with_versioned_relr_requirement() {
    let Some(root) = std::env::var_os("DARTFORGE_TEST_LINUX_SYSROOT") else {
        eprintln!("DARTFORGE_TEST_LINUX_SYSROOT is not set; skipped");
        return;
    };
    let root = PathBuf::from(root);
    let sysroot = SysrootLinux::from_dir(&root).unwrap();
    let artifacts = std::env::var_os("DARTFORGE_RELR_TEST_OUTPUT").map(PathBuf::from);
    let temp = tempfile::tempdir().unwrap();
    let directory = artifacts.as_deref().unwrap_or(temp.path());
    std::fs::create_dir_all(directory).unwrap();
    let source = directory.join("relr.c");
    let object = directory.join("relr.o");
    let pointers = ["&value"; 4096].join(",");
    let header = dartforge_runtime::layout::header_word(
        dartforge_runtime::layout::state::PERMANENT,
        dartforge_runtime::layout::flags::RAW,
        2,
        dartforge_runtime::layout::cid::ONE_BYTE_STRING,
    );
    let hash = dartforge_runtime::layout::string_hash([111, 107]);
    std::fs::write(&source,format!("extern int puts(const char *); int value=42; int *roots[4096]={{{pointers}}}; __attribute__((section(\"dfimg\"),aligned(8))) const unsigned long long strings[]={{{header},{hash},2,27503}}; int main(void){{volatile const unsigned long long *s=strings;if(s[2]!=2)return 2;for(int i=0;i<4096;i++)if(roots[i]!=&value||*roots[i]!=42)return 1;puts(\"relr-ok\");return 0;}}\n")).unwrap();
    let triple = if crate::target::is_aarch64() {
        "aarch64-unknown-linux-gnu"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    let clang = crate::driver::NativeDriverOptions::default().clang;
    let result = Command::new(&clang)
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
    let mut sizes = Vec::new();
    let mut string_digest = None;
    for packed in [false, true] {
        let output = directory.join(if packed { "packed" } else { "standard" });
        let mut plan = linkage(&output);
        plan.entries.push(object.clone());
        let mut command = statement_with_relr(&ld_lld(&clang), &sysroot, &plan, packed).unwrap();
        let arguments: Vec<_> = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|arg| arg.to_str().expect("fixture paths are UTF-8").to_string())
            .collect();
        std::fs::write(
            output.with_extension("command.json"),
            serde_json::to_vec_pretty(&arguments).unwrap(),
        )
        .unwrap();
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let bytes = std::fs::read(&output).unwrap();
        let image = crate::linked_images::inspect(&bytes).unwrap();
        assert_eq!(image.section.strings.len(), 1);
        if let Some(previous) = &string_digest {
            assert_eq!(previous, &image.section.content_digest);
        } else {
            string_digest = Some(image.section.content_digest.clone());
        }
        let audit = crate::linked_images::elf_relocations::inspect(&bytes).unwrap();
        assert!(audit.unreviewed_dynamic_tags.is_empty());
        assert!(
            audit
                .tables
                .iter()
                .all(|table| table.overlapping_writes == 0 && table.unclassified_writes == 0)
        );
        assert_eq!(
            audit.tables.iter().any(|table| table.kind == "relr"),
            packed
        );
        let requirement = b"GLIBC_ABI_DT_RELR\0";
        let found = bytes
            .windows(requirement.len())
            .any(|word| word == requirement);
        assert_eq!(found, packed);
        sizes.push(bytes.len());
        println!(
            "{}",
            serde_json::json!({"path":output,"packed":packed,"bytes":bytes.len(),"glibc_relr_requirement":found,"digest":blake3::hash(&bytes).to_hex().to_string(),"string_section_digest":image.section.content_digest,"relocation_bytes":audit.tables.iter().map(|table|table.bytes).sum::<u64>(),"executed":false})
        );
    }
    assert!(sizes[1] < sizes[0]);
}
