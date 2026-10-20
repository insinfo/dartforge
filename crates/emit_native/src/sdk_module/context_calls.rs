//! Rewrite context instructions without interpreting quoted LLVM bytes as code.

const CALL: &str = "call ptr @dartforge_context()";

/// Preserve a previously configured helper rather than rewriting its slow path.
/// `None` also means that no eligible instruction was found.
pub(super) fn rewrite(ir: &str) -> Option<String> {
    let bytes = ir.as_bytes();
    let mut cursor = 0;
    let mut copied = 0;
    let mut definition = false;
    let mut depth = 0usize;
    let mut output = String::new();
    while cursor < bytes.len() {
        match bytes[cursor] {
            b';' => {
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            b'"' => cursor = quoted_end(bytes, cursor),
            b'@' | b'%' => {
                let global = bytes[cursor] == b'@';
                cursor += 1;
                let start = cursor;
                if bytes.get(cursor) == Some(&b'"') {
                    cursor = quoted_end(bytes, cursor);
                } else {
                    while cursor < bytes.len() && name_byte(bytes[cursor]) {
                        cursor += 1;
                    }
                }
                if global && definition {
                    if is_helper(&ir[start..cursor]) {
                        return None;
                    }
                    definition = false;
                }
            }
            byte if name_byte(byte) => {
                let start = cursor;
                while cursor < bytes.len() && name_byte(bytes[cursor]) {
                    cursor += 1;
                }
                match &ir[start..cursor] {
                    "define" if depth == 0 => definition = true,
                    "call" if ir[start..].starts_with(CALL) => {
                        output.push_str(&ir[copied..start]);
                        output.push_str("call ptr @df.context()");
                        cursor = start + CALL.len();
                        copied = cursor;
                    }
                    _ => {}
                }
            }
            b'{' => {
                depth += 1;
                cursor += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                cursor += 1;
            }
            _ => cursor += 1,
        }
    }
    if copied == 0 {
        None
    } else {
        output.push_str(&ir[copied..]);
        Some(output)
    }
}

fn name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$' | b'-')
}

fn quoted_end(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            return cursor + 1;
        }
        // LLVM represents embedded quotes as hexadecimal escapes (\22).
        if bytes[cursor] == b'\\'
            && bytes.get(cursor + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(cursor + 2).is_some_and(u8::is_ascii_hexdigit)
        {
            cursor += 3;
        } else {
            cursor += 1;
        }
    }
    cursor
}

fn is_helper(name: &str) -> bool {
    let name = name
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(name);
    let mut bytes = name.bytes();
    for expected in b"df.context" {
        let Some(mut actual) = bytes.next() else {
            return false;
        };
        if actual == b'\\' {
            let (Some(high), Some(low)) = (bytes.next(), bytes.next()) else {
                return false;
            };
            let (Some(high), Some(low)) = ((high as char).to_digit(16), (low as char).to_digit(16))
            else {
                return false;
            };
            actual = (high * 16 + low) as u8;
        }
        if actual != *expected {
            return false;
        }
    }
    bytes.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk_module::{ContextInTls, with_context_in_tls};

    #[test]
    fn context_calls_preserve_quoted_bytes_and_comments() {
        let protected = concat!(
            "@text = private constant [30 x i8] c\"call ptr @dartforge_context()\\00\"\n",
            "; call ptr @dartforge_context()\n",
            "@\"call ptr @dartforge_context()\" = global i8 0\n",
            "define void @f() {\n",
            "  %\"call ptr @dartforge_context()\" = add i64 1, 2\n",
            "  call void asm sideeffect \"\\22call ptr @dartforge_context()\\22\", \"\"()\n",
            "  ; define ptr @df.context()\n",
        );
        let ir = format!("{protected}  %c = {CALL}\n  ret void\n}}\n");
        let expected = format!("{protected}  %c = call ptr @df.context()\n  ret void\n}}\n");
        assert_eq!(rewrite(&ir).as_deref(), Some(expected.as_str()));
        assert_eq!(rewrite(protected), None);
        assert_eq!(
            with_context_in_tls(protected, ContextInTls::Free, "tls", false),
            protected
        );
    }

    #[test]
    fn context_calls_preserve_existing_helpers_including_native_tls_slow_path() {
        for name in ["df.context", "\"df.context\"", "\"df\\2econtext\""] {
            let ir = format!("define internal ptr @{name}() {{\n %c = {CALL}\n ret ptr %c\n}}\n");
            assert_eq!(rewrite(&ir), None);
            assert_eq!(
                with_context_in_tls(&ir, ContextInTls::Inline, "tls", true),
                ir
            );
        }
    }

    #[test]
    fn context_calls_rewrite_inline_bodies_and_remain_idempotent() {
        let ir = format!("define ptr @f() {{ %c = {CALL} ret ptr %c }}\n");
        let once = with_context_in_tls(&ir, ContextInTls::Free, "tls", false);
        assert!(once.contains("%c = call ptr @df.context()"));
        assert_eq!(
            with_context_in_tls(&once, ContextInTls::PerCall, "other", true),
            once
        );
        assert_eq!(rewrite("%mycall ptr @dartforge_context()"), None);
    }

    #[test]
    fn context_calls_ignore_definition_words_inside_function_bodies() {
        let ir = format!(
            "define ptr @f() {{\ndefine:\n %c = {CALL}\n %d = call ptr @df.context()\n ret ptr %c\n}}\n"
        );
        assert_eq!(
            rewrite(&ir),
            Some(ir.replace(CALL, "call ptr @df.context()"))
        );
    }
}
