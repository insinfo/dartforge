//! Direct LLVM section contributions and opaque source constructs.
//! This is not a census of linker inputs or a registration authorization.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Member {
    pub symbol: String,
    pub owner: String,
    pub section: String,
    pub kind: String,
    pub source_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Census {
    pub source_digest: String,
    pub members: Vec<Member>,
    pub marker_lines: Vec<String>,
    pub obligations: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    String(Vec<u8>),
    Symbol(char, String),
    Punct(u8),
}

fn quoted(text: &str, index: usize) -> Option<(Vec<u8>, usize)> {
    let end = super::super::closure::quoted_end(text.as_bytes(), index)?;
    let mut bytes = Vec::new();
    let mut i = index + 1;
    while i < end - 1 {
        if text.as_bytes()[i] == b'\\' {
            bytes.push(u8::from_str_radix(text.get(i + 1..i + 3)?, 16).ok()?);
            i += 3;
        } else {
            bytes.push(text.as_bytes()[i]);
            i += 1;
        }
    }
    Some((bytes, end))
}

fn name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_.$-:".contains(&byte)
}

fn tokens(text: &str) -> Option<Vec<Token>> {
    let mut output = Vec::new();
    let mut i = 0;
    let bytes = text.as_bytes();
    while i < bytes.len() {
        match bytes[i] {
            b';' => break,
            byte if byte.is_ascii_whitespace() => i += 1,
            b'"' => {
                let (value, end) = quoted(text, i)?;
                output.push(Token::String(value));
                i = end;
            }
            prefix @ (b'@' | b'%' | b'$') => {
                i += 1;
                let name = if bytes.get(i) == Some(&b'"') {
                    let (value, end) = quoted(text, i)?;
                    i = end;
                    String::from_utf8(value).ok()?
                } else {
                    let start = i;
                    while i < bytes.len() && name_byte(bytes[i]) {
                        i += 1;
                    }
                    if start == i {
                        return None;
                    }
                    text[start..i].into()
                };
                output.push(Token::Symbol(prefix as char, name));
            }
            byte if name_byte(byte) => {
                let start = i;
                while i < bytes.len() && name_byte(bytes[i]) {
                    i += 1;
                }
                output.push(Token::Word(text[start..i].into()));
            }
            byte => {
                output.push(Token::Punct(byte));
                i += 1;
            }
        }
    }
    Some(output)
}

fn image_section(name: &str) -> bool {
    name == "dfimg"
        || name == ".dfimg"
        || name.starts_with(".dfimg$")
        || name.split(',').nth(1) == Some("__dfimg")
}

fn sections(items: &[Token]) -> Option<Vec<String>> {
    let mut output = Vec::new();
    for pair in items.windows(2) {
        if pair[0] == Token::Word("section".into()) {
            let Token::String(bytes) = &pair[1] else {
                return None;
            };
            let name = std::str::from_utf8(bytes).ok()?;
            if image_section(name) {
                output.push(name.into());
            }
        }
    }
    Some(output)
}

fn balanced(items: &[Token]) -> bool {
    let mut stack = Vec::new();
    for item in items {
        match item {
            Token::Punct(b'(') => stack.push(b')'),
            Token::Punct(b'[') => stack.push(b']'),
            Token::Punct(b'{') => stack.push(b'}'),
            Token::Punct(byte @ (b')' | b']' | b'}')) if stack.pop() != Some(*byte) => {
                return false;
            }
            _ => {}
        }
    }
    stack.is_empty()
}

fn protocols() -> Vec<(String, Vec<String>)> {
    use crate::target::Os;
    [Os::MacOs, Os::Linux, Os::Windows, Os::Wasm]
        .into_iter()
        .map(|os| {
            let name = match os {
                Os::MacOs => "macho",
                Os::Linux => "elf",
                Os::Windows => "coff",
                Os::Wasm => "wasm",
            };
            let (text, _, _) = crate::target::image_markers_for(os);
            (name.into(), text.lines().map(str::to_owned).collect())
        })
        .collect()
}

/// Scan every source line; unreviewed syntax/assembly cannot imply completeness.
pub(crate) fn observe(ir: &str) -> Census {
    let protocols = protocols();
    let expected: BTreeSet<_> = protocols
        .iter()
        .flat_map(|(_, lines)| lines.iter().cloned())
        .collect();
    let mut marker_lines = Vec::new();
    let mut members = Vec::new();
    let mut obligations = BTreeSet::new();
    let mut in_function = false;
    for (index, raw) in ir.lines().enumerate() {
        let line = raw.trim();
        let Some(items) = tokens(line) else {
            obligations.insert(format!("malformed_source_line:{}", index + 1));
            continue;
        };
        if items.is_empty() {
            continue;
        }
        if items
            .iter()
            .any(|token| *token == Token::Word("asm".into()))
            && line != "module asm \".no_dead_strip __LLVM_StackMaps\""
            && line != crate::sdk_module::MACOS_ARM64_TSD_READ
        {
            obligations.insert(format!("unreviewed_assembly:{}", index + 1));
        }
        if in_function {
            if items == [Token::Punct(b'}')] {
                in_function = false;
            }
            continue;
        }
        if expected.contains(line) {
            marker_lines.push(line.into());
        }
        let global = match items.first() {
            Some(Token::Symbol('@', name)) => Some(name.clone()),
            _ => None,
        };
        let is_definition = items.first() == Some(&Token::Word("define".into()));
        let symbol = global.clone().or_else(|| {
            if is_definition {
                items.iter().find_map(|token| {
                    if let Token::Symbol('@', name) = token {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        });
        if let Some(Token::Symbol('$', name)) = items.first()
            && name.starts_with("df.img.")
            && !expected.contains(line)
        {
            obligations.insert(format!("unproved_marker_comdat:{name}"));
        }
        if let Some(name) = &symbol
            && (name.starts_with("df.img.")
                || matches!(name.as_str(), "__start_dfimg" | "__stop_dfimg")
                || name.contains("section$start$__DATA_CONST$__dfimg")
                || name.contains("section$end$__DATA_CONST$__dfimg"))
            && !expected.contains(line)
        {
            obligations.insert(format!("unproved_marker:{name}"));
        }
        let Some(section_names) = sections(&items) else {
            obligations.insert(format!("unparsed_section:{}", index + 1));
            continue;
        };
        for section in section_names {
            let kind = if global.is_some() && super::static_string(line).is_some() {
                "static_string"
            } else if global.is_some() && expected.contains(line) {
                "anchor"
            } else {
                "unknown"
            };
            if kind == "unknown" {
                obligations.insert(format!("unproved_section_member:{}", index + 1));
            }
            members.push(Member {
                symbol: symbol.clone().unwrap_or_default(),
                owner: if global.is_some() {
                    "global"
                } else if is_definition {
                    "function"
                } else {
                    "unknown"
                }
                .into(),
                section,
                kind: kind.into(),
                source_digest: blake3::hash(raw.as_bytes()).to_hex().to_string(),
            });
        }
        if is_definition {
            for (i, token) in items.iter().enumerate() {
                if !matches!(token, Token::String(_)) { continue }
                let section = i > 0 && items[i - 1] == Token::Word("section".into());
                let tail_key = *token == Token::String(b"disable-tail-calls".to_vec())
                    && items.get(i + 1) == Some(&Token::Punct(b'='))
                    && items.get(i + 2) == Some(&Token::String(b"true".to_vec()));
                let tail_value = *token == Token::String(b"true".to_vec()) && i >= 2
                    && items[i - 1] == Token::Punct(b'=')
                    && items[i - 2] == Token::String(b"disable-tail-calls".to_vec());
                // The emitter's helpers that never collect (`@df.rti_is_probe`,
                // `@df.string_units_equal`…): a valueless attribute that defines no
                // data and switches no section.
                let gc_leaf = *token == Token::String(b"gc-leaf-function".to_vec())
                    && items.get(i + 1) != Some(&Token::Punct(b'='));
                if !section && !tail_key && !tail_value && !gc_leaf {
                    obligations.insert(format!("unreviewed_function_string_attribute:{}", index + 1));
                }
            }
            let inline = symbol.is_some() && balanced(&items) && items.last() == Some(&Token::Punct(b'}'));
            if !inline && items.last() != Some(&Token::Punct(b'{')) { obligations.insert(format!("multiline_function_header:{}", index + 1)); }
            in_function = !inline;
        } else if global.is_some() {
            if !balanced(&items) { obligations.insert(format!("multiline_global:{}", index + 1)); }
        } else if items.first().is_some_and(|item| matches!(item, Token::Symbol('$' | '%', _) | Token::Punct(b'!'))) {
            if !balanced(&items) { obligations.insert(format!("multiline_top_level:{}", index + 1)); }
            if line.starts_with("!llvm.linker.options") { obligations.insert("linker_metadata".into()); }
        } else if items.first().is_some_and(|item| matches!(item, Token::Word(word) if matches!(word.as_str(), "declare" | "source_filename" | "target"))) {
            // Declarations and target selection do not themselves define data.
        } else if line == "module asm \".no_dead_strip __LLVM_StackMaps\"" {
            // The emitter's exact keep directive does not switch sections.
        } else { obligations.insert(format!("unreviewed_top_level:{}", index + 1)); }
    }
    if in_function {
        obligations.insert("unterminated_function".into());
    }
    // WebAssembly and ELF share the `dfimg` section name; only WebAssembly's weak
    // markers tell its module apart.
    let wasm_markers = protocols
        .iter()
        .any(|(format, lines)| format == "wasm" && lines.iter().any(|l| marker_lines.contains(l)));
    let used: BTreeSet<_> = protocols
        .iter()
        .filter(|(format, lines)| {
            lines.iter().any(|line| marker_lines.contains(line))
                || members.iter().any(|member| match format.as_str() {
                    "macho" => member.section.split(',').nth(1) == Some("__dfimg"),
                    "elf" => member.section == "dfimg" && !wasm_markers,
                    "wasm" => false,
                    _ => member.section == ".dfimg" || member.section.starts_with(".dfimg$"),
                })
        })
        .map(|(format, _)| format)
        .collect();
    if used.len() > 1 {
        obligations.insert("mixed_marker_formats".into());
    }
    let mut counts = HashMap::new();
    for line in &marker_lines {
        *counts.entry(line).or_insert(0usize) += 1;
    }
    if counts.values().any(|count| *count != 1) {
        obligations.insert("duplicate_marker_declarations".into());
    }
    for (format, lines) in &protocols {
        if used.contains(format) && lines.iter().any(|line| !marker_lines.contains(line)) {
            obligations.insert(format!("incomplete_marker_protocol:{format}"));
        }
    }
    if !members.is_empty() && used.is_empty() {
        obligations.insert("missing_marker_protocol".into());
    }
    Census {
        source_digest: blake3::hash(ir.as_bytes()).to_hex().to_string(),
        members,
        marker_lines,
        obligations: obligations.into_iter().collect(),
    }
}

impl Census {
    /// Internal source observations only; a clean record is not a link proof.
    pub(crate) fn is_clean(&self) -> bool {
        self.obligations.is_empty()
    }

    /// Reconcile the global portion with the independently stored initializers.
    pub(crate) fn matches_globals(&self, digest: &str, globals: &[(&str, &str)]) -> bool {
        if self.source_digest != digest || !self.obligations.windows(2).all(|w| w[0] < w[1]) {
            return false;
        }
        let source = globals
            .iter()
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n");
        let replay = observe(&source);
        let actual: Vec<_> = self
            .members
            .iter()
            .filter(|member| member.owner == "global")
            .collect();
        if actual != replay.members.iter().collect::<Vec<_>>() {
            return false;
        }
        let known: BTreeSet<_> = protocols()
            .into_iter()
            .flat_map(|(_, lines)| lines)
            .collect();
        if self.marker_lines.iter().any(|line| !known.contains(line)) {
            return false;
        }
        let actual_globals: Vec<_> = self
            .marker_lines
            .iter()
            .filter(|line| line.starts_with('@'))
            .collect();
        let expected_globals: Vec<_> = replay
            .marker_lines
            .iter()
            .filter(|line| line.starts_with('@'))
            .collect();
        if actual_globals != expected_globals {
            return false;
        }
        if self.members.iter().any(|m| {
            !matches!(m.owner.as_str(), "global" | "function" | "unknown")
                || !matches!(m.kind.as_str(), "static_string" | "anchor" | "unknown")
                || m.source_digest.len() != 64
        }) {
            return false;
        }
        if self.is_clean() {
            if self
                .members
                .iter()
                .any(|m| m.kind == "unknown" || m.owner != "global")
            {
                return false;
            }
            let comdats = self
                .marker_lines
                .iter()
                .filter(|line| line.starts_with('$'))
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            let protocol = observe(&format!("{source}\n{comdats}"));
            if !protocol.is_clean() {
                return false;
            }
            // A literal-only module without its markers must not claim closure.
            if !self.members.is_empty() && self.marker_lines.is_empty() {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::Os;

    #[test]
    fn canonical_marker_protocols_cover_all_object_formats() {
        for os in [Os::MacOs, Os::Linux, Os::Windows, Os::Wasm] {
            let (source, _, _) = crate::target::image_markers_for(os);
            let census = observe(&source);
            assert!(census.is_clean(), "{census:?}");
            assert!(census.members.iter().all(|member| member.kind == "anchor"));
            // WebAssembly's weak markers need no anchor.
            let anchors = match os {
                Os::Windows => 2,
                Os::Wasm => 0,
                Os::Linux | Os::MacOs => 1,
            };
            assert_eq!(census.members.len(), anchors);
            let globals: Vec<_> = source
                .lines()
                .filter(|line| line.starts_with('@'))
                .map(|line| ("unused", line))
                .collect();
            assert!(census.matches_globals(&census.source_digest, &globals));
        }
    }

    #[test]
    fn reviewed_tsd_read_does_not_authorize_variants_or_raw_memory_calls() {
        let instruction = crate::sdk_module::MACOS_ARM64_TSD_READ;
        let function = |body: &str| {
            format!(
                "define internal ptr @df.context() {{\n {body}\n %p = inttoptr i64 %t to ptr\n %c = load ptr, ptr %p\n ret ptr %c\n}}\n"
            )
        };
        let source = function(instruction);
        assert!(observe(&source).is_clean());
        let bundle = super::super::super::closure::audit(&[("sdk", &source)]);
        let payload = bundle.to_json();
        let body = &payload["functions"][0];
        assert!(body["transitive_classes"].is_null());
        assert!(!body["unknown_obligations"].as_array().unwrap().is_empty());
        for altered in [
            instruction.replace(
                "mrs $0, TPIDRRO_EL0",
                "mrs $0, TPIDRRO_EL0; .pushsection dfimg",
            ),
            instruction.replace("\"=r\"", "\"=r,~{memory}\""),
            instruction.replace("asm ", "asm sideeffect "),
        ] {
            assert!(!observe(&function(&altered)).is_clean());
        }
    }

    #[test]
    fn image_contributions_cannot_hide_in_functions_assembly_or_continuations() {
        let (markers, _, _) = crate::target::image_markers_for(Os::MacOs);
        for source in [
            "define void @in_image() section \"__DATA_CONST,__dfimg\" { ret void }",
            "define void @f() {\n call void asm sideeffect \".pushsection __DATA_CONST,__dfimg\", \"\"()\n ret void\n}",
            "module asm \".section __DATA_CONST,__dfimg\"",
            "@foreign = constant i64 7, section \"__DATA_CONST,__dfimg\"",
            "@foreign = constant [1 x i8]\n c\"x\", section \"__DATA_CONST,__dfimg\"",
            "define {i64, i64} @multiline()\n section \"__DATA_CONST,__dfimg\" {\n ret {i64, i64} zeroinitializer\n}",
            "define void @prefix() \"implicit-section-name\"=\"__dfimg\" { ret void }",
            "!llvm.linker.options = !{!0}",
            "attributes #0 = { \"unreviewed\"=\"attribute\" }",
        ] {
            let census = observe(&format!("{markers}\n{source}\n"));
            assert!(!census.is_clean(), "{source}");
        }
    }

    #[test]
    fn marker_overrides_missing_roles_and_mixed_formats_stay_open() {
        let (markers, _, _) = crate::target::image_markers_for(Os::Linux);
        for source in [
            markers.replace("external hidden global i8", "global i8 0"),
            markers.replace("$df.img.anchor = comdat any\n", ""),
            markers.replace("comdat any", "comdat largest"),
            format!("{markers}\n{markers}"),
            format!("{markers}\n@\"\\5F_start_dfimg\" = alias i8, ptr @other\n"),
            format!(
                "{markers}\n{}",
                crate::target::image_markers_for(Os::MacOs).0
            ),
            format!("{markers}\n@foreign = constant i64 0, section \".dfimg$x\"\n"),
        ] {
            assert!(!observe(&source).is_clean(), "{source}");
        }
        let line = super::super::tests::emitted_literal(b"literal");
        assert!(
            !observe(&line).is_clean(),
            "literal without marker protocol"
        );
    }

    #[test]
    fn quoted_names_strings_and_comments_do_not_invent_section_directives() {
        let source = "@text = private constant [1 x i8] c\";\" ; section \"dfimg\"\n\
            define void @\"section\"(i64 %\"asm\") \"disable-tail-calls\"=\"true\" {\n\
            section:\n ret void\n} ; ignored quote \"\n\
            !0 = !{!\"section\", !\"dfimg\"}\n";
        let census = observe(source);
        assert!(census.is_clean(), "{census:?}");
        assert!(census.members.is_empty());
    }

    #[test]
    fn the_gc_leaf_attribute_of_a_helper_is_reviewed_only_without_a_value() {
        let helper = "define internal i8 @df.rti_is_probe(ptr %c) noinline \"gc-leaf-function\" {\n\
            entry:\n ret i8 0\n}\n";
        let census = observe(helper);
        assert!(census.is_clean(), "{census:?}");
        let valued = "define internal i8 @f(ptr %c) \"gc-leaf-function\"=\"x\" {\n\
            entry:\n ret i8 0\n}\n";
        assert!(!observe(valued).is_clean());
    }
}
