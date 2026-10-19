//! Object headers prove that the embedded generator selected the requested
//! destination rather than silently emitting an object for its own host.
use dartforge_llvm::{Format, Options, generate};

#[test]
fn emits_all_supported_object_formats_and_architectures() {
    let options = Options { optimize: false, format: Format::Object, cpu: None };
    for (triple, machine) in [
        ("x86_64-unknown-linux-gnu", 62u16),
        ("aarch64-unknown-linux-gnu", 183),
    ] {
        let bytes = generate("cross", &format!("target triple = \"{triple}\"\ndefine i64 @answer() {{ ret i64 42 }}\n"), &options).unwrap();
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert_eq!(u16::from_le_bytes([bytes[18], bytes[19]]), machine);
    }
    let bytes = generate("cross", "target triple = \"x86_64-pc-windows-msvc\"\ndefine i64 @answer() { ret i64 42 }\n", &options).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 0x8664);
    for (triple, cpu) in [("aarch64-apple-darwin", 0x100000cu32), ("x86_64-apple-darwin", 0x1000007)] {
        let bytes = generate("cross", &format!("target triple = \"{triple}\"\ndefine i64 @answer() {{ ret i64 42 }}\n"), &options).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[..4].try_into().unwrap()), 0xfeedfacf);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), cpu);
    }
}
