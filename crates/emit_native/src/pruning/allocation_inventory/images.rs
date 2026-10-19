//! Validate individual permanent string blocks, not whole linked image ranges.
//! A known block layout does not authorize registration or expose its handle.
use dartforge_runtime::layout::{self, cid, flags, state};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct StaticString {
    pub cid: u32,
}

/// Recognize the emitter's canonical pointer-free, permanent string definition.
/// All arithmetic is bounded by the actual initializer before inspecting units.
pub(super) fn static_string(line: &str) -> Option<StaticString> {
    let rest = line.strip_prefix("@\"df.s.")?;
    let (key, rest) = rest.split_once("\" = linkonce_odr constant { i64, i64, i64, [")?;
    if key.len() != 32
        || !key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let (size, rest) = rest.split_once(" x i8] } { i64 ")?;
    let size = size.parse::<usize>().ok()?;
    let (header, rest) = rest.split_once(", i64 ")?;
    let header = header.parse::<i64>().ok()? as u64;
    let (hash, rest) = rest.split_once(", i64 ")?;
    let hash = hash.parse::<u32>().ok()?;
    let (length, rest) = rest.split_once(", [")?;
    let length = length.parse::<usize>().ok()?;
    let (body_size, rest) = rest.split_once(" x i8] ")?;
    if body_size.parse::<usize>().ok()? != size || size % 8 != 0 {
        return None;
    }
    let (body, suffix) = rest.rsplit_once(" },")?;
    if !matches!(
        suffix,
        " section \"__DATA_CONST,__dfimg\", align 8"
            | " comdat, section \"dfimg\", align 8"
            | " comdat, section \".dfimg$m\", align 8"
    ) {
        return None;
    }
    let content = if size == 0 && body == "zeroinitializer" {
        Vec::new()
    } else {
        super::closure::native_byte_content(&format!(
            "@body = private constant [{size} x i8] {body}"
        ))?
    };
    let class = (header >> 32) as i32;
    let width = match class {
        cid::ONE_BYTE_STRING => 1,
        cid::TWO_BYTE_STRING => 2,
        _ => return None,
    };
    let used = length.checked_mul(width)?;
    if used.checked_add(7)? / 8 * 8 != size
        || used > content.len()
        || content[used..].iter().any(|b| *b != 0)
    {
        return None;
    }
    let words = 1usize.checked_add(size / 8)?;
    if header != layout::header_word(state::PERMANENT, flags::RAW, words, class) {
        return None;
    }
    let actual_hash = if width == 1 {
        layout::string_hash(content[..used].iter().map(|b| u16::from(*b)))
    } else {
        let units = content[..used].as_chunks::<2>().0;
        if units.iter().all(|b| b[1] == 0) {
            return None;
        }
        layout::string_hash(units.iter().map(|b| u16::from_le_bytes(*b)))
    };
    if actual_hash != hash {
        return None;
    }
    let mut digest = blake3::Hasher::new();
    digest.update(&class.to_le_bytes());
    digest.update(&content[..used]);
    if &digest.finalize().to_hex()[..32] != key {
        return None;
    }
    Some(StaticString { cid: class as u32 })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::hir::*;

    pub(crate) fn emitted_literal(bytes: &[u8]) -> String {
        let mut module = Module::new();
        module.functions.push(Function {
            symbol: "df.image.literal".into(),
            name: "literal".into(),
            params: vec![],
            return_ty: Type::Ref,
            debug_info: None,
            blocks: vec![BasicBlock {
                id: BlockId(0),
                instructions: vec![(
                    ValueId(0),
                    Instruction::Const(Constant::StringWtf8(bytes.to_vec())),
                    Type::Ref,
                )],
                terminator: Terminator::Return(Some(Operand::Val(ValueId(0)))),
            }],
        });
        let ir = crate::llvm::LlvmEmitter::new(&module)
            .with_static_objects(true)
            .emit_all();
        ir.lines()
            .find(|line| line.starts_with("@\"df.s."))
            .unwrap()
            .into()
    }

    #[test]
    fn real_emitter_literals_cover_empty_latin1_utf16_surrogates_and_padding() {
        for (bytes, class) in [
            (&b""[..], cid::ONE_BYTE_STRING),
            (&b"a;@x\\\""[..], cid::ONE_BYTE_STRING),
            (&b"123456789"[..], cid::ONE_BYTE_STRING),
            ("é".as_bytes(), cid::ONE_BYTE_STRING),
            ("λ😀".as_bytes(), cid::TWO_BYTE_STRING),
            (&[0xed, 0xa0, 0xbd][..], cid::TWO_BYTE_STRING),
        ] {
            let line = emitted_literal(bytes);
            assert_eq!(
                static_string(&line),
                Some(StaticString { cid: class as u32 }),
                "{line}"
            );
        }
    }

    #[test]
    fn altered_identity_header_hash_size_padding_and_section_are_not_certified() {
        let line = emitted_literal(b"abc");
        let prefix = " x i8] } { i64 ";
        let (_, rest) = line.split_once(prefix).unwrap();
        let header = rest.split_once(", i64 ").unwrap().0;
        let hash = rest
            .split_once(", i64 ")
            .unwrap()
            .1
            .split_once(", i64 ")
            .unwrap()
            .0;
        let variants = [
            line.replacen("df.s.", "df.x.", 1),
            line.replacen("linkonce_odr constant", "linkonce_odr global", 1),
            line.replacen("[8 x i8]", "[16 x i8]", 1),
            line.replacen(&format!("i64 {header},"), "i64 0,", 1),
            line.replacen(&format!(", i64 {hash},"), ", i64 0,", 1),
            line.replacen(", i64 3, [", ", i64 18446744073709551615, [", 1),
            line.replacen("abc", "abd", 1),
            line.replacen("\\00", "\\01", 1),
            line.replace("align 8", "align 1"),
            format!("{line} garbage"),
        ];
        for altered in variants {
            assert!(static_string(&altered).is_none(), "{altered}");
        }
        let key_start = line.find("df.s.").unwrap() + 5;
        let mut wrong_key = line.clone();
        wrong_key.replace_range(
            key_start..key_start + 1,
            if &line[key_start..key_start + 1] == "0" {
                "1"
            } else {
                "0"
            },
        );
        assert!(static_string(&wrong_key).is_none());
        let altered = line
            .replace("__DATA_CONST,__dfimg", "__DATA_CONST,__other")
            .replace("section \"dfimg\"", "section \"other\"")
            .replace(".dfimg$m", ".other$m");
        assert!(static_string(&altered).is_none());
    }

    #[test]
    fn noncanonical_utf16_latin1_storage_is_not_an_emitter_literal() {
        let line = emitted_literal("λ".as_bytes());
        let key_start = line.find("df.s.").unwrap() + 5;
        let old_key = &line[key_start..key_start + 32];
        let old_hash = layout::string_hash([0x03bb]);
        let hash = layout::string_hash([u16::from(b'a')]);
        let mut digest = blake3::Hasher::new();
        digest.update(&cid::TWO_BYTE_STRING.to_le_bytes());
        digest.update(&[b'a', 0]);
        let altered = line
            .replace(old_key, &digest.finalize().to_hex()[..32])
            .replace(&format!(", i64 {old_hash},"), &format!(", i64 {hash},"))
            .replace("\\BB\\03", "a\\00");
        assert_ne!(line, altered);
        assert!(static_string(&altered).is_none());
    }

    #[test]
    fn all_emitted_section_spellings_are_observable_not_link_certificates() {
        let line = emitted_literal(b"same layout");
        let (object, _) = line.rsplit_once(" },").unwrap();
        for suffix in [
            " section \"__DATA_CONST,__dfimg\", align 8",
            " comdat, section \"dfimg\", align 8",
            " comdat, section \".dfimg$m\", align 8",
        ] {
            assert!(static_string(&format!("{object} }},{suffix}")).is_some());
        }
    }
}
