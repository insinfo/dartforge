//! Emission of strings (P1, docs/NATIVO-ESPACO-UNIFICADO.md §2.11, §3.5 and
//! §4.3): the literals (`Const(String/StringWtf8)`), interpolation
//! (`ConcatStrings`) and the `@df.string_*` helpers of the prelude.
//!
//! A string is a `RAW` block of cid 6 (`_OneByteString`) or 7
//! (`_TwoByteString`): with the handle `h = block + 2`, the cid at `h+2` (`i32`), the
//! hash at `h+6` (`i32`, 0 = not computed), the length at `h+14` (`i64`) and the
//! code units starting at `h+22` (`layout::off`).
//!
//! **Literal in AOT** (`static_objects`): a static object of the module,
//! `@df.s.<key>`, `linkonce_odr` in a `comdat` keyed by the content (the linker
//! keeps a single copy across the program and the SDK modules: `identical` of
//! equal literals holds across libraries), state `PERMANENT`, in the image
//! section, with the hash already computed (`layout::string_hash`). The value is the
//! constant `ptrtoint (gep @df.s.<key>, 2)`: no call nor cache at the use
//! site. **In the JIT** the memory of a generation is freed (J02), so the literal
//! stays interned in the heap by the runtime (`dartforge_string_new`) with the
//! use-site cache in the isolate area.
//!
//! The helpers are `define internal … alwaysinline`; their numbers (cids,
//! offsets, header word) are those of `layout`, checked by the
//! `string_helpers_follow_layout` test.

use super::LlvmEmitter;
use crate::hir::*;
use dartforge_runtime::layout::{self, cid, flags, offsets, state};
use std::fmt::Write;

/// The `@df.string_*` helpers (§3.5), printed in the prelude of each module by
/// `emit_runtime_decls`.
///
/// * `@df.string_len(s)`: the length (`s` is a string);
/// * `@df.string_code_unit(s, i)`: code unit `i`, without checking bounds;
/// * `@df.string_allocate(len, two)`: a string of `len` zeroed code units, inline
///   through `@df.allocate` up to 16 words, otherwise `@dartforge_string_allocate`;
/// * `@df.string_write(s, i, u)`: the `writeInto*String` (writes without a barrier: the
///   string is `RAW` and has not been published yet);
/// * `@df.string_hash(s)`: the hash in the header, or `@dartforge_string_hash` on the
///   first query;
/// * `@df.string_equals(a, b)`: `==` between two strings (identity; different lengths and
///   different computed hashes give false without a call);
/// * `@df.string_equals_object(a, b)`: `==` of a string with an `Object?` (the
///   overlay's `_StringBase.==`): what is not a string is different.
pub const HELPERS: &str = "\
define internal i64 @df.string_len(i64 %s) alwaysinline nounwind {
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  %n = load i64, ptr %q, align 8
  ret i64 %n
}
define internal i64 @df.string_code_unit(i64 %s, i64 %i) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %b1 = load i8, ptr %a1, align 1
  %r1 = zext i8 %b1 to i64
  ret i64 %r1
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %b2 = load i16, ptr %a2, align 2
  %r2 = zext i16 %b2 to i64
  ret i64 %r2
}
define internal i64 @df.string_allocate(i64 %len, i1 %dois) alwaysinline {
entrada:
  %d = zext i1 %dois to i64
  %bytes = shl i64 %len, %d
  %b7 = add i64 %bytes, 7
  %pw = lshr i64 %b7, 3
  %w = add i64 %pw, 1
  %grande = icmp ugt i64 %w, 16
  br i1 %grande, label %lento, label %rapido
rapido:
  %c = add i64 %d, 6
  %cs = shl i64 %c, 32
  %ws = shl i64 %w, 16
  %c0 = or i64 %cs, %ws
  %cab = or i64 %c0, 513
  %h = call i64 @df.allocate(i64 %cab, i64 %w)
  %p = inttoptr i64 %h to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 14
  store i64 %len, ptr %q, align 8
  ret i64 %h
lento:
  %hl = call i64 @dartforge_string_allocate(i64 %len, i64 %d)
  ret i64 %hl
}
define internal void @df.string_write(i64 %s, i64 %i, i64 %x) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %pc = getelementptr inbounds i8, ptr %p, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %u = getelementptr inbounds i8, ptr %p, i64 22
  %um = icmp eq i32 %c, 6
  br i1 %um, label %um1, label %dois
um1:
  %a1 = getelementptr inbounds i8, ptr %u, i64 %i
  %t1 = trunc i64 %x to i8
  store i8 %t1, ptr %a1, align 1
  ret void
dois:
  %a2 = getelementptr inbounds i16, ptr %u, i64 %i
  %t2 = trunc i64 %x to i16
  store i16 %t2, ptr %a2, align 2
  ret void
}
define internal i64 @df.string_hash(i64 %s) alwaysinline nounwind {
entrada:
  %p = inttoptr i64 %s to ptr
  %q = getelementptr inbounds i8, ptr %p, i64 6
  %x = load i32, ptr %q, align 4
  %z = icmp eq i32 %x, 0
  %ze = call i1 @llvm.expect.i1(i1 %z, i1 false)
  br i1 %ze, label %lento, label %rapido
rapido:
  %r = zext i32 %x to i64
  ret i64 %r
lento:
  %l = call i64 @dartforge_string_hash(i64 %s)
  ret i64 %l
}
define internal i1 @df.string_equals(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %compr
compr:
  %pa = inttoptr i64 %a to ptr
  %pb = inttoptr i64 %b to ptr
  %qa = getelementptr inbounds i8, ptr %pa, i64 14
  %qb = getelementptr inbounds i8, ptr %pb, i64 14
  %la = load i64, ptr %qa, align 8
  %lb = load i64, ptr %qb, align 8
  %ml = icmp eq i64 %la, %lb
  br i1 %ml, label %hashes, label %nao
hashes:
  %ra = getelementptr inbounds i8, ptr %pa, i64 6
  %rb = getelementptr inbounds i8, ptr %pb, i64 6
  %ha = load i32, ptr %ra, align 4
  %hb = load i32, ptr %rb, align 4
  %az = icmp eq i32 %ha, 0
  %bz = icmp eq i32 %hb, 0
  %mh = icmp eq i32 %ha, %hb
  %algum = or i1 %az, %bz
  %talvez = or i1 %algum, %mh
  br i1 %talvez, label %lento, label %nao
lento:
  %r = call i8 @dartforge_string_equals(i64 %a, i64 %b)
  %rb1 = icmp ne i8 %r, 0
  ret i1 %rb1
sim:
  ret i1 true
nao:
  ret i1 false
}
define internal i1 @df.string_equals_object(i64 %a, i64 %b) alwaysinline nounwind {
entrada:
  %id = icmp eq i64 %a, %b
  br i1 %id, label %sim, label %objeto
objeto:
  %t = and i64 %b, -9223372036854775801
  %e = icmp eq i64 %t, 2
  br i1 %e, label %classe, label %nao
classe:
  %pb = inttoptr i64 %b to ptr
  %pc = getelementptr inbounds i8, ptr %pb, i64 2
  %c = load i32, ptr %pc, align 4, !invariant.load !{}
  %c6 = sub i32 %c, 6
  %texto = icmp ult i32 %c6, 2
  br i1 %texto, label %comparar, label %nao
comparar:
  %r = call i1 @df.string_equals(i64 %a, i64 %b)
  ret i1 %r
sim:
  ret i1 true
nao:
  ret i1 false
}
";

/// The effects of each `@df.*` helper of this file called by
/// `CallRuntime`: `(name, allocates, throws)`. `externs::effects_of` consults them
/// before the extern table (a helper outside here is conservative).
pub(super) const HELPER_EFFECTS: &[(&str, bool, bool)] = &[
    ("df.string_len", false, false),
    ("df.string_code_unit", false, false),
    ("df.string_allocate", true, false),
    ("df.string_write", false, false),
    ("df.string_hash", false, false),
    ("df.string_equals", false, false),
    ("df.string_equals_object", false, false),
];

/// The per-module emission state of strings.
#[derive(Default)]
pub(super) struct StringState {
    /// The static literals used by the module: the key (`@df.s.<key>`) and the
    /// definition of the global, written at the end of the module in key order.
    literals: std::collections::BTreeMap<String, String>,
}

/// The image section (§2.11) in the object format of the host.
fn image_section() -> &'static str {
    match crate::target::target_os() {
        crate::target::Os::Windows => ".dfimg$m",
        crate::target::Os::MacOs => "__DATA_CONST,__dfimg",
        crate::target::Os::Linux => "dfimg",
    }
}

/// A static literal: the key and the definition of the global.
fn static_literal(bytes: &[u8]) -> (String, String) {
    let text = dartforge_runtime::heap::Text::from_wtf8(bytes);
    let seen = text.view();
    let two = !seen.is_one_byte();
    let c = if two {
        cid::TWO_BYTE_STRING
    } else {
        cid::ONE_BYTE_STRING
    };
    let len = seen.len();
    let w = layout::string_words(len, two);
    // The code units in bytes (little-endian), padded with zeros up to the word.
    let mut units: Vec<u8> = match seen {
        dartforge_runtime::text::TextRef::OneByte(b) => b.to_vec(),
        dartforge_runtime::text::TextRef::TwoByte(u) => {
            u.iter().flat_map(|x| x.to_le_bytes()).collect()
        }
    };
    units.resize(8 * (w - 1), 0);
    let mut h = blake3::Hasher::new();
    h.update(&c.to_le_bytes());
    h.update(&units[..if two { 2 * len } else { len }]);
    let key = h.finalize().to_hex()[..32].to_string();
    let header = layout::header_word(state::PERMANENT, flags::RAW, w, c);
    let hash = layout::string_hash(seen.units());
    let n = units.len();
    let body = if n == 0 {
        "zeroinitializer".to_string()
    } else {
        let mut s = String::from("c\"");
        for &b in &units {
            if b.is_ascii_graphic() && b != b'"' && b != b'\\' || b == b' ' {
                s.push(b as char);
            } else {
                write!(s, "\\{b:02X}").unwrap();
            }
        }
        s.push('"');
        s
    };
    let comdat = if crate::target::has_comdat() {
        ", comdat"
    } else {
        ""
    };
    let mut def = String::new();
    if crate::target::has_comdat() {
        writeln!(def, "$\"df.s.{key}\" = comdat any").unwrap();
    }
    writeln!(
        def,
        "@\"df.s.{key}\" = linkonce_odr constant {{ i64, i64, i64, [{n} x i8] }} {{ i64 {}, i64 {hash}, i64 {len}, [{n} x i8] {body} }}{comdat}, section \"{}\", align 8",
        header as i64,
        image_section()
    )
    .unwrap();
    (key, def)
}

impl LlvmEmitter<'_> {
    /// The string literal `bytes` (WTF-8) into `%v<v>`.
    pub(super) fn emit_const_string(&mut self, v: u32, bytes: &[u8]) {
        if self.static_objects {
            let (key, def) = static_literal(bytes);
            self.strings.literals.entry(key.clone()).or_insert(def);
            writeln!(
                self.out,
                "  %v{v} = ptrtoint ptr getelementptr inbounds (i8, ptr @\"df.s.{key}\", i64 {}) to i64",
                layout::HANDLE_OFFSET
            )
            .unwrap();
            return;
        }
        let idx = self.string_const_index(bytes).unwrap_or(0);
        let len = bytes.len();
        // JIT: the handle of the literal stays in a use-site cache, in the isolate
        // area (the literal is canonical and permanent; a JIT reload
        // empties the caches): the lookup in the runtime only on the first evaluation.
        let slot = self.cache_slot();
        let previous = self.current_label.clone();
        let o = &mut self.out;
        writeln!(o, "  %lsp{v} = getelementptr i64, ptr %area, i64 {slot}").unwrap();
        writeln!(o, "  %lsv{v} = load i64, ptr %lsp{v}, align 8").unwrap();
        writeln!(o, "  %lsz{v} = icmp eq i64 %lsv{v}, 0").unwrap();
        writeln!(
            o,
            "  %lsx{v} = call i1 @llvm.expect.i1(i1 %lsz{v}, i1 false)"
        )
        .unwrap();
        writeln!(o, "  br i1 %lsx{v}, label %ls{v}.nova, label %ls{v}.fim").unwrap();
        writeln!(o, "ls{v}.nova:").unwrap();
        writeln!(
            o,
            "  %lsn{v} = call i64 @dartforge_string_new(ptr @.str.{idx}, i64 {len})"
        )
        .unwrap();
        writeln!(o, "  store i64 %lsn{v}, ptr %lsp{v}, align 8").unwrap();
        writeln!(o, "  br label %ls{v}.fim").unwrap();
        writeln!(o, "ls{v}.fim:").unwrap();
        writeln!(
            o,
            "  %v{v} = phi i64 [ %lsv{v}, %{previous} ], [ %lsn{v}, %ls{v}.nova ]"
        )
        .unwrap();
        self.current_label = format!("ls{v}.fim");
    }

    /// The `alloca` of the entry block that `ConcatStrings` uses (if it does).
    pub(super) fn concat_strings_buffer(&mut self, v: u32, parts: &[Operand]) {
        writeln!(
            self.out,
            "  %jbuf{v} = alloca [{} x i64]",
            2 * parts.len().max(1)
        )
        .unwrap();
    }

    /// Interpolation: the parts (a `Ref` string or an unboxed `int`) into one string.
    /// The runtime (`dartforge_string_join_typed`) measures the parts through the views
    /// of the blocks, allocates once and copies the code units, without an intermediate copy.
    pub(super) fn emit_concat_strings(&mut self, v: u32, parts: &[Operand]) {
        // Pairs (kind, bits): 0 and the `Ref` of a string, 1 and an `int`.
        let n = parts.len();
        for (i, p) in parts.iter().enumerate() {
            let (kind, s) = if self.type_of(p) == Type::I64 {
                (1, self.coerce(p, Type::I64))
            } else {
                (0, self.coerce(p, Type::Ref))
            };
            writeln!(
                self.out,
                "  %jk{v}_{i} = getelementptr [{} x i64], ptr %jbuf{v}, i64 0, i64 {}",
                2 * n,
                2 * i
            )
            .unwrap();
            writeln!(self.out, "  store i64 {kind}, ptr %jk{v}_{i}").unwrap();
            writeln!(
                self.out,
                "  %jp{v}_{i} = getelementptr [{} x i64], ptr %jbuf{v}, i64 0, i64 {}",
                2 * n,
                2 * i + 1
            )
            .unwrap();
            writeln!(self.out, "  store i64 {s}, ptr %jp{v}_{i}").unwrap();
        }
        // Under ARC, the factory roots the parts and hands over the owner of the result.
        let name = if self.arc_owned_factories() {
            "dartforge_arc_string_join_owned_v1"
        } else {
            "dartforge_string_join_typed"
        };
        writeln!(
            self.out,
            "  %v{v} = call i64 @{name}(ptr %jbuf{v}, i64 {n})"
        )
        .unwrap();
    }

    /// The module-level globals of the strings: the static literals used
    /// by the module (with the `comdat` of each one), written at the end of the module.
    pub(super) fn emit_string_globals(&mut self) {
        let literals = std::mem::take(&mut self.strings.literals);
        if literals.is_empty() {
            return;
        }
        self.out
            .push_str("; Static string literals (docs/NATIVO-ESPACO-UNIFICADO.md §2.11)\n");
        for def in literals.values() {
            self.out.push_str(def);
        }
        self.out.push('\n');
    }
}

// Checks that the numbers written by hand in `HELPERS` are those of `layout`.
const _: () = {
    assert!(offsets::CLASS as i64 - layout::HANDLE_OFFSET == 2);
    assert!(offsets::STRING_HASH as i64 - layout::HANDLE_OFFSET == 6);
    assert!(offsets::LENGTH as i64 - layout::HANDLE_OFFSET == 14);
    assert!(offsets::UNITS as i64 - layout::HANDLE_OFFSET == 22);
    assert!(cid::ONE_BYTE_STRING == 6 && cid::TWO_BYTE_STRING == 7);
    assert!(layout::TLAB_N == 16);
    assert!(layout::header_word(state::YOUNG, flags::RAW, 0, 0) == 513);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_helpers_follow_layout() {
        // The mask of `e_objeto` (`h & (7 | i64::MIN) == 2`).
        assert!(HELPERS.contains(&format!("and i64 %b, {}", 7 | i64::MIN)));
        // The header word built in `@df.string_allocate` is the one from `layout`.
        for (len, two) in [(0usize, false), (5, false), (9, true), (60, false)] {
            let w = layout::string_words(len, two);
            let c = if two {
                cid::TWO_BYTE_STRING
            } else {
                cid::ONE_BYTE_STRING
            };
            let assembled = (c as u64) << 32 | (w as u64) << 16 | 513;
            assert_eq!(
                assembled,
                layout::header_word(state::YOUNG, flags::RAW, w, c)
            );
            // The helper's computation of `w`: 1 + ((len << two) + 7) >> 3.
            assert_eq!(1 + (((len << u32::from(two)) + 7) >> 3), w);
        }
        for (name, _, _) in HELPER_EFFECTS {
            assert!(
                HELPERS.contains(&format!("@{name}(")),
                "{name} without a definition"
            );
        }
    }

    #[test]
    fn static_literal_in_block_shape() {
        let (key, def) = static_literal(b"ab");
        assert_eq!(key.len(), 32);
        let w0 = layout::header_word(state::PERMANENT, flags::RAW, 2, cid::ONE_BYTE_STRING) as i64;
        let hash = layout::string_hash("ab".encode_utf16());
        assert!(
            def.contains(&format!(
                "{{ i64 {w0}, i64 {hash}, i64 2, [8 x i8] c\"ab\\00\\00\\00\\00\\00\\00\" }}"
            )),
            "{def}"
        );
        assert!(def.contains("linkonce_odr constant"));
        // The same string gives the same key; the shape goes into the key.
        assert_eq!(static_literal(b"ab").0, key);
        assert_ne!(static_literal(b"ac").0, key);
        let (_, two) = static_literal("€".as_bytes());
        let w0 = layout::header_word(state::PERMANENT, flags::RAW, 2, cid::TWO_BYTE_STRING) as i64;
        assert!(two.contains(&format!("i64 {w0}")), "{two}");
        assert!(two.contains("c\"\\AC \\00\\00\\00\\00\\00\\00\""), "{two}");
        let (_, empty) = static_literal(b"");
        assert!(empty.contains("[0 x i8] zeroinitializer"), "{empty}");
    }
}
