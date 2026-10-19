use dartforge_llvm::image_sections::{Bytes, Initializer};
use dartforge_llvm::{Format, Options, generate, inspect_image_sections};

const IR: &str = r#"
target triple = "aarch64-apple-darwin"
@anchor = linkonce_odr hidden constant [2 x i64] zeroinitializer, section "__DATA_CONST,__dfimg", align 8
@literal = linkonce_odr constant {i64, i64, i64, [8 x i8]} {i64 6, i64 7, i64 3, [8 x i8] c"abc\00\00\00\00\00"}, section "__DATA_CONST,__dfimg", align 8
@bounds = external hidden global i8
@ordinary = private constant [1 x i8] c"x"
@alias = alias [2 x i64], ptr @anchor
define void @code() section "__DATA_CONST,__dfimg" { ret void }
define i64 @tls() {
 %t = call i64 asm "mrs $0, TPIDRRO_EL0", "=r"()
 %byte = load volatile i8, ptr @bounds
 %address = ptrtoint ptr @literal to i64
 %result = xor i64 %t, %address
 ret i64 %result
}
module asm ".no_dead_strip _anchor"
"#;

#[test]
fn inspection_copies_structural_data_sections_declarations_and_assembly() {
    let inventory = inspect_image_sections("image", IR.as_bytes()).unwrap();
    assert_eq!(inventory.triple.as_deref(), Some("aarch64-apple-darwin"));
    assert_eq!(
        inventory.globals.len(),
        3,
        "ordinary data has no explicit section"
    );
    let anchor = inventory
        .globals
        .iter()
        .find(|g| g.name == b"anchor")
        .unwrap();
    assert!(anchor.hidden && anchor.constant && anchor.link_once_odr);
    assert_eq!(anchor.alignment, 8);
    assert_eq!(anchor.initializer, Some(Initializer::ZeroAnchor));
    let literal = inventory
        .globals
        .iter()
        .find(|g| g.name == b"literal")
        .unwrap();
    assert_eq!(
        literal.initializer,
        Some(Initializer::HeaderAndBytes {
            words: [6, 7, 3],
            bytes: Bytes::Data(b"abc\0\0\0\0\0".to_vec())
        })
    );
    assert!(
        inventory
            .globals
            .iter()
            .any(|g| g.name == b"bounds" && g.declaration)
    );
    assert_eq!(inventory.functions.len(), 1);
    assert_eq!(inventory.functions[0].name, b"code");
    assert_eq!(inventory.inline_asm.len(), 1);
    let asm = &inventory.inline_asm[0];
    assert_eq!(asm.function, b"tls");
    assert_eq!(asm.signature, "i64 ()");
    assert_eq!(asm.template, b"mrs $0, TPIDRRO_EL0");
    assert_eq!(asm.constraints, b"=r");
    assert!(!asm.side_effects && !asm.aligned_stack && !asm.can_unwind && !asm.intel_dialect);
    assert_eq!(inventory.aliases, [b"alias".to_vec()]);
    assert!(inventory.module_asm.starts_with(b".no_dead_strip"));
}

#[test]
fn bitcode_and_text_preserve_the_same_explicit_section_values() {
    let options = Options {
        optimize: false,
        format: Format::Bitcode,
        cpu: None,
    };
    let code = generate("image", IR, &options).unwrap();
    let before = inspect_image_sections("text", IR.as_bytes()).unwrap();
    let after = inspect_image_sections("bitcode", &code).unwrap();
    assert_eq!(before.globals, after.globals);
    assert_eq!(before.functions, after.functions);
    assert_eq!(before.inline_asm, after.inline_asm);
    assert_eq!(before.aliases, after.aliases);
}

#[test]
fn section_inspection_reads_bitcode_for_each_supported_object_format() {
    for (triple, section) in [
        ("aarch64-apple-darwin", "__DATA_CONST,__dfimg"),
        ("aarch64-unknown-linux-gnu", "dfimg"),
        ("x86_64-pc-windows-msvc", ".dfimg$m"),
    ] {
        let ir = format!(
            "target triple = \"{triple}\"\n@anchor = constant [2 x i64] zeroinitializer, section \"{section}\", align 8\ndefine ptr @address() {{ ret ptr @anchor }}\n"
        );
        let options = Options {
            optimize: false,
            format: Format::Bitcode,
            cpu: None,
        };
        let bitcode = generate("target-image", &ir, &options).unwrap();
        let inventory = inspect_image_sections("target-image", &bitcode).unwrap();
        assert_eq!(inventory.triple.as_deref(), Some(triple));
        assert_eq!(inventory.globals.len(), 1);
        assert_eq!(inventory.globals[0].section, section.as_bytes());
        assert_eq!(
            inventory.globals[0].initializer,
            Some(Initializer::ZeroAnchor)
        );
    }
}

#[test]
fn unknown_pointer_packed_and_poison_initializers_are_not_scalar_blocks() {
    let ir = r#"
@pointer = constant ptr @external, section "dfimg"
@external = external global i8
@packed = constant <{i64, i64, i64, [0 x i8]}> <{i64 0, i64 0, i64 0, [0 x i8] zeroinitializer}>, section "dfimg"
@poison = constant {i64, i64, i64, [0 x i8]} {i64 poison, i64 0, i64 0, [0 x i8] zeroinitializer}, section "dfimg"
@zeroes = constant {i64, i64, i64, [18446744073709551615 x i8]} zeroinitializer, section "dfimg"
"#;
    let inventory = inspect_image_sections("shapes", ir.as_bytes()).unwrap();
    for name in [b"pointer".as_slice(), b"packed", b"poison"] {
        assert_eq!(
            inventory
                .globals
                .iter()
                .find(|g| g.name == name)
                .unwrap()
                .initializer,
            Some(Initializer::Other)
        );
    }
    let zeroes = inventory
        .globals
        .iter()
        .find(|g| g.name == b"zeroes")
        .unwrap();
    assert_eq!(
        zeroes.initializer,
        Some(Initializer::HeaderAndBytes {
            words: [0; 3],
            bytes: Bytes::Zeroes(u64::MAX)
        })
    );
}

#[test]
fn embedded_nul_sections_are_not_truncated_to_a_known_prefix() {
    let inventory =
        inspect_image_sections("nul", b"@x = constant i64 0, section \"dfimg\\00other\"\n")
            .unwrap();
    assert_eq!(inventory.globals[0].section, b"dfimg\0other");
}

#[test]
fn quoted_payload_bytes_do_not_corrupt_section_recovery() {
    let ir = br#"@"name\5C\22" = constant [3 x i8] c"\22\5C\00", section "dfimg\5Csuffix\00tail"
@0 = constant i64 0, section "dfimg"
define void @1() section "code\5Csuffix" { ret void }
"#;
    let inventory = inspect_image_sections("escapes", ir).unwrap();
    assert_eq!(inventory.globals[0].name, b"name\\\"");
    assert_eq!(inventory.globals[0].section, b"dfimg\\suffix\0tail");
    assert_eq!(inventory.globals[1].section, b"dfimg");
    assert_eq!(inventory.functions[0].section, b"code\\suffix");
}

#[test]
fn aliases_and_resolvers_do_not_shift_section_header_identity() {
    let ir = br#"@anchor = constant i64 0, section "dfimg"
@"alias\5Cname" = alias i64, ptr @anchor
@indirect = ifunc void (), ptr @resolver
@later = constant i64 1, section "later"
declare void @external()
define ptr @resolver() { ret ptr @implementation }
define void @implementation() section "code" { ret void }
"#;
    let inventory = inspect_image_sections("indirect", ir).unwrap();
    assert_eq!(inventory.aliases, [b"alias\\name".to_vec()]);
    assert_eq!(inventory.indirect_functions, [b"indirect".to_vec()]);
    assert_eq!(inventory.globals[1].name, b"later");
    assert_eq!(inventory.globals[1].section, b"later");
    assert_eq!(inventory.functions[0].name, b"implementation");
    assert_eq!(inventory.functions[0].section, b"code");
}

#[test]
fn rejected_modules_publish_no_partial_observations() {
    for input in [
        b"not LLVM".as_slice(),
        b"BC\xc0\xde",
        b"define i64 @bad() { ret i32 0 }",
        b"define i64 @bad() { entry: br label %loop\nloop: %x = add i64 %x, 1\n ret i64 %x } ",
    ] {
        assert!(inspect_image_sections("invalid", input).is_err());
    }
    assert!(inspect_image_sections("nul\0name", b"").is_err());
}
