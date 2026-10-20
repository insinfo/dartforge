mod before {
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'$' | b'.' | b'_')
}

pub fn cited(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if b[i] != b'@' {
                i += 1;
                continue;
            }
            i += 1;
            if i < b.len() && b[i] == b'"' {
                let ini = i + 1;
                let end = text[ini..].find('"').map_or(b.len(), |k| ini + k);
                i = end + 1;
                return Some(&text[ini..end]);
            }
            let ini = i;
            while i < b.len() && name_char(b[i]) {
                i += 1;
            }
            if i > ini {
                return Some(&text[ini..i]);
            }
        }
        None
    })
}

}
mod after {
fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, b'-' | b'$' | b'.' | b'_')
}

pub fn cited(text: &str) -> impl Iterator<Item = &str> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut quoted_literal_end = 0;
    std::iter::from_fn(move || {
        while i < b.len() {
            if i >= quoted_literal_end {
                quoted_literal_end = 0;
            }
            if quoted_literal_end == 0 {
                if b[i] == b'c'
                    && b.get(i + 1) == Some(&b'"')
                    && (i == 0 || !name_char(b[i - 1]))
                    && let Some(end) = text[i + 2..].find('"')
                {
                    // LLVM encodes embedded quotes as \22, so the next raw
                    // quote terminates this byte string. An incomplete literal
                    // keeps the old conservative scan instead.
                    i += end + 3;
                    continue;
                }
                if b[i] == b'"' {
                    quoted_literal_end = text[i + 1..].find('"').map_or(b.len(), |end| i + end + 2);
                    i += 1;
                    continue;
                }
            }
            if b[i] != b'@' {
                i += 1;
                continue;
            }
            i += 1;
            if quoted_literal_end == 0 && i < b.len() && b[i] == b'"' {
                let ini = i + 1;
                let end = text[ini..].find('"').map_or(b.len(), |k| ini + k);
                i = end + 1;
                return Some(&text[ini..end]);
            }
            let ini = i;
            while i < b.len() && name_char(b[i]) {
                i += 1;
            }
            if i > ini {
                return Some(&text[ini..i]);
            }
        }
        None
    })
}

}

fn main() {
    for path in std::env::args().skip(1) {
        let source = std::fs::read_to_string(&path).unwrap();
        let mut old_count = 0usize;
        let mut new_count = 0usize;
        let mut changed = 0usize;
        let mut examples = 0usize;
        for (index, line) in source.lines().enumerate().filter(|(_, line)| line.contains('@')) {
            let old: Vec<_> = before::cited(line).collect();
            let new: Vec<_> = after::cited(line).collect();
            old_count += old.len(); new_count += new.len();
            if old != new {
                assert!(line.contains("c\""), "changed a non-byte-string line: {path}:{}", index + 1);
                let mut remaining = old.iter();
                for name in &new { assert!(remaining.any(|candidate| candidate == name), "lost reference ordering"); }
                changed += 1;
                if examples < 5 { eprintln!("{path}:{} before={old:?} after={new:?}", index + 1); examples += 1; }
            }
        }
        println!("{path}\t{old_count}\t{new_count}\t{changed}");
    }
}
