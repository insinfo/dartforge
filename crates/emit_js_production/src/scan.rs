//! Scanning of compiler-emitted JavaScript.
//!
//! This is not a JavaScript parser: it is a reader of **top-level
//! declarations**. DDC's `dart_sdk.js` and our modules are compiler output,
//! with a small and regular grammar — what we need is to know where each
//! top-level declaration starts and ends, and a character reader that
//! respects strings, comments and the depth of `{}`/`()`/`[]` does that
//! without building any tree.
//!
//! The property the test demands is **conservation**: the concatenation of
//! the returned slices must be, byte for byte, the input text. A scan that
//! loses a byte would silently corrupt the program, which is exactly the
//! failure mode one does not want in a minifier.

/// Walks `src` skipping the string that starts at `i` (which points at the
/// quote). Returns the index right after the closing quote.
fn skip_string(b: &[u8], mut i: usize) -> usize {
    let quote = b[i];
    i += 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            c if c == quote => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Like [`skip_string`], for comments. `i` points at the first `/`.
fn skip_comment(b: &[u8], mut i: usize) -> Option<usize> {
    if i + 1 >= b.len() || b[i] != b'/' {
        return None;
    }
    match b[i + 1] {
        b'/' => {
            i += 2;
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            Some(i)
        }
        b'*' => {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            Some((i + 2).min(b.len()))
        }
        _ => None,
    }
}

/// Slices `[start, end)` of the top-level declarations of `src`.
///
/// A declaration ends at the first `;` or `}` that is at depth zero and is
/// followed (only by spaces) by a line break — that is the shape DDC and our
/// emitter produce. The line break stays in the slice, so that the
/// concatenation rebuilds the original.
pub fn declarations(src: &str) -> Vec<(usize, usize)> {
    let b = src.as_bytes();
    let mut slices = Vec::new();
    let (mut i, mut start) = (0usize, 0usize);
    let (mut braces, mut parens, mut brackets) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = skip_string(b, i);
            continue;
        }
        if c == b'/'
            && let Some(j) = skip_comment(b, i)
        {
            i = j;
            continue;
        }
        match c {
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => brackets += 1,
            b']' => brackets -= 1,
            _ => {}
        }
        i += 1;
        if braces == 0 && parens == 0 && brackets == 0 && (c == b';' || c == b'}') {
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t' || b[j] == b'\r') {
                j += 1;
            }
            if j < b.len() && b[j] == b'\n' {
                slices.push((start, j + 1));
                start = j + 1;
                i = j + 1;
            }
        }
    }
    if start < b.len() {
        slices.push((start, b.len()));
    }
    slices
}

/// Slices of the top-level entries of an object literal body (the text
/// **between** the braces), separated by commas at depth zero. The comma
/// stays in the slice, for the same reason as in [`declarations`]:
/// conservation.
pub fn object_entries(body: &str) -> Vec<(usize, usize)> {
    let b = body.as_bytes();
    let mut slices = Vec::new();
    let (mut i, mut start) = (0usize, 0usize);
    let (mut braces, mut parens, mut brackets) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = skip_string(b, i);
            continue;
        }
        if c == b'/'
            && let Some(j) = skip_comment(b, i)
        {
            i = j;
            continue;
        }
        match c {
            b'{' => braces += 1,
            b'}' => braces -= 1,
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => brackets += 1,
            b']' => brackets -= 1,
            b',' if braces == 0 && parens == 0 && brackets == 0 => {
                slices.push((start, i + 1));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < b.len() {
        slices.push((start, b.len()));
    }
    slices
}

/// Slices of the members of a class body (the text **between** the braces).
///
/// A member of the DDC contract is always `header { body }` — method,
/// getter, setter, static or generator. It therefore ends at the brace that
/// closes its body at depth zero. A stray `;` at depth zero also closes one
/// (class fields, which DDC does not emit but ES allows).
pub fn class_entries(body: &str) -> Vec<(usize, usize)> {
    let b = body.as_bytes();
    let mut slices = Vec::new();
    let (mut i, mut start) = (0usize, 0usize);
    let (mut braces, mut parens, mut brackets) = (0i32, 0i32, 0i32);
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = skip_string(b, i);
            continue;
        }
        if c == b'/'
            && let Some(j) = skip_comment(b, i)
        {
            i = j;
            continue;
        }
        match c {
            b'{' => braces += 1,
            b'}' => {
                braces -= 1;
                if braces == 0 && parens == 0 && brackets == 0 {
                    slices.push((start, i + 1));
                    start = i + 1;
                }
            }
            b'(' => parens += 1,
            b')' => parens -= 1,
            b'[' => brackets += 1,
            b']' => brackets -= 1,
            b';' if braces == 0 && parens == 0 && brackets == 0 => {
                slices.push((start, i + 1));
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < b.len() {
        slices.push((start, b.len()));
    }
    slices
}

/// Index of the brace that closes the one starting at `open` (which points
/// at `{`).
pub fn closing_brace(src: &str, open: usize) -> Option<usize> {
    closing(src, open, b'{', b'}')
}

/// Index of the bracket that closes the one starting at `open` (which points
/// at `[`).
pub fn closing_bracket(src: &str, open: usize) -> Option<usize> {
    closing(src, open, b'[', b']')
}

fn closing(src: &str, open: usize, op: u8, cl: u8) -> Option<usize> {
    let b = src.as_bytes();
    let mut i = open;
    let mut d = 0i32;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' || c == b'`' {
            i = skip_string(b, i);
            continue;
        }
        if c == b'/'
            && let Some(j) = skip_comment(b, i)
        {
            i = j;
            continue;
        }
        if c == op {
            d += 1;
        } else if c == cl {
            d -= 1;
            if d == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn join(src: &str) -> String {
        declarations(src)
            .into_iter()
            .map(|(a, b)| &src[a..b])
            .collect()
    }

    #[test]
    fn conserves_bytes() {
        let src = "var a = 1;\nfn.x = function () {\n  return \";\\n}\";\n};\n// end }\nb.c = 2;\n";
        assert_eq!(join(src), src);
        assert_eq!(declarations(src).len(), 3, "three top-level declarations");
    }

    #[test]
    fn closing_brace_ignores_strings() {
        let src = "x = { a: \"}\" };";
        let open = src.find('{').unwrap();
        assert_eq!(&src[closing_brace(src, open).unwrap()..], "};");
    }

    #[test]
    fn class_members_conserve_bytes() {
        let body = "\n  f() {\n    return \"}\";\n  }\n  get x() { return 1; }\n";
        let v = class_entries(body);
        // two members plus the final line break, which gets its own slice
        assert_eq!(v.len(), 3);
        assert_eq!(
            v.iter().map(|&(a, b)| &body[a..b]).collect::<String>(),
            body
        );
    }

    #[test]
    fn object_entries_at_level_zero() {
        let body = " a: 1, b: {c: 2, d: 3}, e: f(1, 2) ";
        let v = object_entries(body);
        assert_eq!(v.len(), 3);
        assert_eq!(
            v.iter().map(|&(a, b)| &body[a..b]).collect::<String>(),
            body
        );
    }
}
