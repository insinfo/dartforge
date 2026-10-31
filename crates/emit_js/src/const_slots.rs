//! Per-site caches of constant expressions in the production profile.
//!
//! The DDC contract evaluates a constant where it is written:
//! `dart.const(new C.new(…))`, `dart.constList(rti, […])`, `dart.constMap`,
//! `dart.constSet` build the object and canonicalize it through the
//! runtime's tables (the own property names of the object, then one `Map`
//! per property) **on every evaluation**. A `const` in a loop, or behind a
//! getter of the SDK (`utf8.encode` reads `const Utf8Encoder()`, `jsonEncode`
//! `const JsonEncoder()`), paid that each time; `dart2js` emits each constant
//! once.
//!
//! In production a constant whose text is closed (every free name is a
//! library, a prelude alias of this module or another slot: no local, no
//! `this`, no type parameter) becomes `(K ??= <constant>)`, with `K` a
//! variable of the module's prelude. The first evaluation is the one the
//! contract made, so it returns the same canonical object; the next ones
//! return it again. A constant is never `null`, and its evaluation has no
//! effect besides creating the object, so skipping it changes nothing a
//! program observes. The name of `K` comes from the module and the text
//! alone, so the fragments of a library can be emitted in any order (and
//! cached) and still agree; two sites with the same text share it, which is
//! the canonical object anyway. `DARTFORGE_JSPROD_CONST_SLOTS=0` turns it off.

use crate::ctx::Ctx;
use crate::module::ModState;

/// Whether the per-site caches are on (production only).
pub(crate) fn enabled(ctx: &Ctx) -> bool {
    ctx.production() && std::env::var("DARTFORGE_JSPROD_CONST_SLOTS").map_or(true, |v| v != "0")
}

/// `code` (a constant expression) wrapped in its module slot, or unchanged
/// when the caches are off or the text is not closed.
pub(crate) fn wrap(ctx: &Ctx, m: &ModState, code: String) -> String {
    if !enabled(ctx) || !closed(&code, &|name| known_name(ctx, m, name)) {
        return code;
    }
    let slot = slot_name(m.lib.0, &code);
    m.note_const_slot(&slot);
    format!("({slot} ??= {code})")
}

/// The slot of `code` in module `module`: `K<module>$<hash of the text>`.
fn slot_name(module: u32, code: &str) -> String {
    // FNV-1a, 64 bits: deterministic across runs and platforms.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in code.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("K{module}${:012x}", h >> 16)
}

/// A free name the module scope gives the same meaning everywhere: a
/// library namespace, the runtime, `t$R`, a prelude alias (`dartx` member or
/// private name) or another slot of this module.
fn known_name(ctx: &Ctx, m: &ModState, name: &str) -> bool {
    matches!(name, "dart" | "dartx" | "t$R")
        || m.dartx_used.borrow().contains_key(name)
        || m.private_syms.borrow().contains_key(name)
        || m.const_slots.borrow().contains(name)
        || ctx.libs.iter().any(|l| l.js_var == name)
}

/// Whether every free identifier of the JavaScript expression `code` is a
/// keyword literal or satisfies `known`. Property names (after `.`) and the
/// keys of object literals (`{a: …}`, `, b: …`) are not free names; string
/// literals are skipped; a template literal with a substitution, or any
/// unexpected character, makes the text not closed.
fn closed(code: &str, known: &dyn Fn(&str) -> bool) -> bool {
    let b = code.as_bytes();
    let mut i = 0;
    // The last significant character before the current token.
    let mut prev: u8 = b'(';
    while i < b.len() {
        let c = b[i];
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
                continue;
            }
            b'"' | b'\'' | b'`' => {
                i += 1;
                while i < b.len() && b[i] != c {
                    if b[i] == b'\\' {
                        i += 1;
                    } else if c == b'`' && b[i] == b'$' && b.get(i + 1) == Some(&b'{') {
                        return false;
                    }
                    i += 1;
                }
                if i >= b.len() {
                    return false;
                }
                i += 1;
                prev = b'"';
                continue;
            }
            b'0'..=b'9' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.' || b[i] == b'_')
                {
                    i += 1;
                }
                prev = b'0';
                continue;
            }
            c if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => {
                let start = i;
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'$')
                {
                    i += 1;
                }
                let word = &code[start..i];
                let mut j = i;
                while j < b.len() && b[j].is_ascii_whitespace() {
                    j += 1;
                }
                let key = matches!(prev, b'{' | b',') && b.get(j) == Some(&b':');
                let free = prev != b'.' && !key;
                if free
                    && !matches!(word, "new" | "void" | "true" | "false" | "null")
                    && !known(word)
                {
                    return false;
                }
                prev = b'a';
                continue;
            }
            b'.' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b':' | b'?' | b'-' | b'+'
            | b'!' | b'=' | b'>' | b'<' | b'&' | b'|' | b'*' | b'/' | b'%' | b'^' | b'~' => {
                // `?.` reads a property too.
                prev = if c == b'?' && b.get(i + 1) == Some(&b'.') {
                    i += 1;
                    b'.'
                } else {
                    c
                };
                i += 1;
            }
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib(n: &str) -> bool {
        matches!(n, "dart" | "core" | "L$main" | "t$R" | "$length" | "K0$abc")
    }

    #[test]
    fn closed_constants() {
        assert!(closed("dart.const(new L$main.P.new(1, 2))", &lib));
        assert!(closed(
            "dart.constList(t$R(\"|p\"), [1, -2.5e3, true, null])",
            &lib
        ));
        assert!(closed(
            "dart.const(new core.X.new({$y: true, name: \"a\\\"b\"}))",
            &lib
        ));
        assert!(closed(
            "dart.constMap(t$R(\"|q\"), t$R(\"|p\"), [\"a\", (K0$abc ??= x0)])",
            &|n| { lib(n) || n == "x0" }
        ));
        assert!(closed(
            "dart.const(new L$main.P.new(void 0, `plain`))",
            &lib
        ));
    }

    #[test]
    fn open_constants() {
        // A local constant, a temporary, `this`, a type parameter.
        assert!(!closed("dart.constList(t$R(\"|p\"), [k, 2])", &lib));
        assert!(!closed("dart.constList(t$R(\"|p\"), [t$0])", &lib));
        assert!(!closed("dart.const(new L$main.P.new(this.x))", &lib));
        assert!(!closed("dart.constList(T, [])", &lib));
        // A ternary branch is not an object key.
        assert!(!closed("dart.constList(t$R(\"|p\"), [true ? k : 1])", &lib));
        assert!(!closed("dart.constList(t$R(\"|p\"), [`${k}`])", &lib));
        assert!(!closed("dart.constList(t$R(\"|p\"), [\"open", &lib));
    }

    #[test]
    fn slot_names_depend_on_module_and_text() {
        let a = slot_name(3, "dart.const(new L$main.P.new(1, 2))");
        assert_eq!(a, slot_name(3, "dart.const(new L$main.P.new(1, 2))"));
        assert_ne!(a, slot_name(4, "dart.const(new L$main.P.new(1, 2))"));
        assert_ne!(a, slot_name(3, "dart.const(new L$main.P.new(1, 3))"));
        assert!(a.starts_with("K3$") && a.len() == 3 + 12);
    }
}
