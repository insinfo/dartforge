//! Locate function attributes in LLVM headers without treating quoted data as syntax.

/// Keep the first application when the requested attribute is already present.
pub(super) fn apply(ir: &str, attribute: &str) -> String {
    let mut output = String::with_capacity(ir.len() + ir.len() / 64);
    let mut cursor = 0;
    let mut copied = 0;
    let mut depth = 0usize;
    let mut header = false;
    let mut named = false;
    let mut parameters = None;
    let mut suffix = false;
    let mut nesting = 0usize;
    let mut present = false;
    let mut previous_end = 0;
    while let Some((start, end)) = next_token(ir, &mut cursor) {
        let token = &ir[start..end];
        if token == "define" && depth == 0 {
            header = true;
            named = false;
            parameters = None;
            suffix = false;
            nesting = 0;
            present = false;
        } else if header && !suffix {
            if !named && token.starts_with('@') {
                named = true;
            } else if named && token == "(" {
                parameters = Some(parameters.unwrap_or(0usize) + 1);
            } else if let Some(level) = parameters
                && token == ")"
            {
                parameters = level.checked_sub(1).filter(|level| *level > 0);
                suffix = parameters.is_none();
            }
        } else if header {
            if nesting == 0
                && matches!(
                    token,
                    "{" | "section"
                        | "partition"
                        | "comdat"
                        | "align"
                        | "gc"
                        | "prefix"
                        | "prologue"
                        | "personality"
                        | "!"
                )
            {
                if !present {
                    output.push_str(&ir[copied..previous_end]);
                    output.push_str(attribute);
                    copied = previous_end;
                }
                header = false;
            } else if nesting == 0 && token == attribute.trim() {
                present = true;
            }
            if token == "(" {
                nesting += 1;
            }
            if token == ")" {
                nesting = nesting.saturating_sub(1);
            }
        }
        match token {
            "{" => depth += 1,
            "}" => depth = depth.saturating_sub(1),
            _ => {}
        }
        previous_end = end;
    }
    output.push_str(&ir[copied..]);
    output
}

/// Return token spans; LLVM quoted escapes do not contain literal quotes.
fn next_token(ir: &str, cursor: &mut usize) -> Option<(usize, usize)> {
    let bytes = ir.as_bytes();
    while *cursor < bytes.len() {
        if bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        } else if bytes[*cursor] == b';' {
            while *cursor < bytes.len() && bytes[*cursor] != b'\n' {
                *cursor += 1;
            }
        } else {
            break;
        }
    }
    let start = *cursor;
    let byte = *bytes.get(start)?;
    if matches!(byte, b'@' | b'%') {
        *cursor += 1;
    }
    if bytes.get(*cursor) == Some(&b'"') {
        *cursor += 1;
        while *cursor < bytes.len() && bytes[*cursor] != b'"' {
            *cursor += 1;
        }
        if *cursor < bytes.len() {
            *cursor += 1;
        }
    } else if bytes.get(*cursor).is_some_and(|byte| name_byte(*byte)) {
        while *cursor < bytes.len() && name_byte(bytes[*cursor]) {
            *cursor += 1;
        }
    } else if *cursor == start {
        *cursor += 1;
    }
    Some((start, *cursor))
}

fn name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'-')
}

#[cfg(test)]
mod tests {
    use crate::sdk_module::{with_nounwind, with_optsize, with_uwtable};

    #[test]
    fn function_attributes_preserve_quoted_names_and_attributes() {
        let ir = "define void @\" comdat personality gc  ! nounwind\"(ptr %\" nounwind\") \"note\"=\" nounwind\" {\n ret void\n}\n";
        let point = ir.find(" {\n").unwrap();
        for (attribute, transformed) in [
            (" nounwind", with_nounwind(ir)),
            (" optsize", with_optsize(ir)),
            (" uwtable", with_uwtable(ir)),
        ] {
            assert_eq!(
                transformed,
                format!("{}{attribute}{}", &ir[..point], &ir[point..])
            );
        }
    }

    #[test]
    fn function_attributes_leave_inline_bodies_and_comments_intact() {
        let ir = "define void @f() { call void asm sideeffect \" nounwind {\", \"\"() ret void } ; personality comdat\n";
        assert_eq!(with_nounwind(ir), ir.replacen("@f()", "@f() nounwind", 1));
        let ir = "define void @f() ; nounwind\n{\n ret void\n}\n";
        assert_eq!(with_nounwind(ir), ir.replacen("@f()", "@f() nounwind", 1));
    }

    #[test]
    fn function_attributes_handle_multiline_parameters_and_suffixes() {
        let ir = "define\nvoid @f(\n ptr byval({i64, i64}) %x\n)\n gc \"statepoint-example\" {\n ret void\n}\n";
        assert_eq!(
            with_nounwind(ir),
            ir.replacen(")\n gc", ") nounwind\n gc", 1)
        );
        for suffix in [
            "comdat",
            "section \"code\"",
            "partition \"part\"",
            "align 8",
            "gc \"statepoint-example\"",
            "prefix i32 0",
            "prologue i32 0",
            "personality ptr @handler",
            "!dbg !3",
        ] {
            let ir = format!("define void @f() {suffix} {{\n ret void\n}}\n");
            assert_eq!(with_nounwind(&ir), ir.replacen("@f()", "@f() nounwind", 1));
        }
    }

    #[test]
    fn function_attributes_are_idempotent_and_ignore_nondefinitions() {
        for transform in [with_nounwind, with_optsize, with_uwtable] {
            let ir = "declare void @g()\n; define void @fake() {\n@text = constant [7 x i8] c\"define\\00\"\ndefine void @f() {\n ret void\n}\n";
            let once = transform(ir);
            assert_eq!(transform(&once), once);
            assert!(once.starts_with("declare void @g()\n; define void @fake() {\n@text"));
        }
        let ir = "define void @f() uwtable(sync) {\n ret void\n}\n";
        assert_eq!(with_uwtable(ir), ir);
    }

    #[test]
    fn function_attributes_keep_real_llvm_names_and_bodies_valid() {
        let ir = "define i32 @\" comdat personality gc  ! nounwind\"(i32 %\" nounwind\") \"note\"=\" nounwind\" {\n ret i32 %\" nounwind\"\n}\ndefine i32 @main() {\n %r = call i32 @\" comdat personality gc  ! nounwind\"(i32 42)\n %z = sub i32 %r, 42\n ret i32 %z\n}\n";
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("attributes.ll");
        let object = dir.path().join("attributes.o");
        for text in [
            ir.to_string(),
            with_uwtable(&with_optsize(&with_nounwind(ir))),
        ] {
            std::fs::write(&source, text).unwrap();
            let output =
                std::process::Command::new(crate::driver::NativeDriverOptions::default().clang)
                    .args(["-c", "-x", "ir"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&object)
                    .output()
                    .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(object.metadata().unwrap().len() > 0);
        }
    }
}
