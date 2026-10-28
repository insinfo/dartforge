//! The module's internal helpers that read the thread context, threaded with the
//! context a function already holds ([`super::LlvmEmitter::with_context_parameter`]).
//!
//! The body `f$cx(ptr %ctx, …)` receives the context, but the `alwaysinline`
//! helpers it calls (`@df.class`, `@df.subclass`, `@df.allocate`, `@df.get_area`,
//! the type-check memo…) each read it again from thread-local storage. Before,
//! those reads were the same `memory(none)` lookup as the function's own and LLVM
//! merged them; next to a parameter they would stay, one per inlined helper. So
//! every internal `@df.*` helper that reads the context, or calls one that does,
//! gets a twin `@df.h$ctx(ptr %df.ctx.in, …)` with the read replaced by the
//! parameter, and the calls of the `$cx` bodies (and of the twins) go to the
//! twins. The originals stay for every other caller; LLVM drops what is unused.

use std::collections::HashSet;

/// The read the emitter writes for the context.
const READ: &str = " = call ptr @dartforge_context()";
/// The parameter of a twin.
const PARAMETER: &str = "%df.ctx.in";
/// The suffix of a twin.
const SUFFIX: &str = "$ctx";

/// A `define … { … }` of the text: the line range, header included and the
/// closing `}` excluded.
struct Definition<'a> {
    name: &'a str,
    header: usize,
    end: usize,
}

/// The name of the function a `define` line defines (unquoted names only).
fn defined_name(line: &str) -> Option<&str> {
    if !line.starts_with("define ") {
        return None;
    }
    let at = line.find('@')? + 1;
    let rest = &line[at..];
    if rest.starts_with('"') {
        return None;
    }
    let end = rest.find('(')?;
    Some(&rest[..end])
}

fn definitions<'a>(lines: &[&'a str]) -> Vec<Definition<'a>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if let Some(name) = defined_name(lines[i]) {
            let mut end = i + 1;
            while end < lines.len() && lines[end] != "}" {
                end += 1;
            }
            out.push(Definition {
                name,
                header: i,
                end,
            });
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// The helpers called on `line` (`@<name>(` of a call), in order.
fn called<'a>(line: &'a str, helpers: &HashSet<&str>) -> Vec<&'a str> {
    let mut out = Vec::new();
    if !line.contains("call ") {
        return out;
    }
    let mut rest = line;
    while let Some(at) = rest.find("@df.") {
        let tail = &rest[at + 1..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
            .unwrap_or(tail.len());
        let name = &tail[..end];
        if tail[end..].starts_with('(') && helpers.contains(name) {
            out.push(name);
        }
        rest = &tail[end..];
    }
    out
}

/// `line` with each call to a helper going to its twin, passing `context`.
fn redirect(line: &str, helpers: &HashSet<&str>, context: &str) -> String {
    let mut out = String::with_capacity(line.len() + 32);
    let mut rest = line;
    while let Some(at) = rest.find("@df.") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let end = tail
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$' | '-')))
            .unwrap_or(tail.len());
        let name = &tail[..end];
        let after = &tail[end..];
        if after.starts_with('(') && helpers.contains(name) {
            out.push('@');
            out.push_str(name);
            out.push_str(SUFFIX);
            out.push_str("(ptr ");
            out.push_str(context);
            if !after.starts_with("()") {
                out.push_str(", ");
            }
            rest = &after[1..];
        } else {
            out.push('@');
            out.push_str(name);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// The header of the twin of `header` (`define internal … @name(…)…`).
fn twin_header(header: &str, name: &str) -> Option<String> {
    let at = header.find(&format!("@{name}("))?;
    let open = at + name.len() + 1;
    let inside = &header[open + 1..];
    let separator = if inside.starts_with(')') { "" } else { ", " };
    Some(format!(
        "{}@{name}{SUFFIX}(ptr {PARAMETER}{separator}{}",
        &header[..at],
        inside
    ))
}

/// The text with the twins ([`self`]). `ir` is a whole module.
pub(super) fn thread(ir: &str) -> String {
    let lines: Vec<&str> = ir.lines().collect();
    let all = definitions(&lines);
    // The internal `@df.*` helpers that read the context, then those that call
    // one of them (a fixpoint: the dispatch helper calls the class helper).
    let candidates: Vec<&Definition> = all
        .iter()
        .filter(|d| {
            d.name.starts_with("df.")
                && lines[d.header].starts_with("define internal ")
                && !d.name.ends_with(SUFFIX)
        })
        .collect();
    let mut helpers: HashSet<&str> = candidates
        .iter()
        .filter(|d| {
            lines[d.header + 1..d.end]
                .iter()
                .any(|l| l.trim_start().starts_with('%') && l.ends_with(READ))
        })
        .map(|d| d.name)
        .collect();
    if helpers.is_empty() {
        return ir.to_string();
    }
    loop {
        let before = helpers.len();
        for d in &candidates {
            if !helpers.contains(d.name)
                && lines[d.header + 1..d.end]
                    .iter()
                    .any(|l| !called(l, &helpers).is_empty())
            {
                helpers.insert(d.name);
            }
        }
        if helpers.len() == before {
            break;
        }
    }
    // Only the helpers with a header the twin can be derived from.
    helpers.retain(|h| {
        candidates
            .iter()
            .find(|d| d.name == *h)
            .is_some_and(|d| twin_header(lines[d.header], d.name).is_some())
    });
    let mut out = String::with_capacity(ir.len() + ir.len() / 16);
    let mut twins = String::new();
    let mut next = 0;
    for d in &all {
        for l in &lines[next..d.header] {
            out.push_str(l);
            out.push('\n');
        }
        let header = lines[d.header];
        let body = &lines[d.header + 1..d.end];
        // A body with the context parameter: its helper calls go to the twins.
        let holds_context = header.contains("$cx(ptr %ctx");
        out.push_str(header);
        out.push('\n');
        for l in body {
            if holds_context && !called(l, &helpers).is_empty() {
                out.push_str(&redirect(l, &helpers, "%ctx"));
            } else {
                out.push_str(l);
            }
            out.push('\n');
        }
        if d.end < lines.len() {
            out.push_str(lines[d.end]);
            out.push('\n');
        }
        if helpers.contains(d.name)
            && let Some(h) = twin_header(header, d.name)
        {
            twins.push_str(&h);
            twins.push('\n');
            for l in body {
                let t = l.trim_start();
                if t.starts_with('%') && l.ends_with(READ) {
                    let name = &t[..t.len() - READ.len()];
                    twins.push_str(&format!(
                        "  {name} = getelementptr inbounds i8, ptr {PARAMETER}, i64 0\n"
                    ));
                } else if !called(l, &helpers).is_empty() {
                    twins.push_str(&redirect(l, &helpers, PARAMETER));
                    twins.push('\n');
                } else {
                    twins.push_str(l);
                    twins.push('\n');
                }
            }
            twins.push_str("}\n");
        }
        next = d.end + 1;
    }
    for l in lines.iter().skip(next) {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&twins);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const IR: &str = r#"define internal i64 @df.class(i64 %h) alwaysinline {
  %ctx = call ptr @dartforge_context()
  %p = getelementptr inbounds i8, ptr %ctx, i64 336
  %v = load i64, ptr %p, align 8
  ret i64 %v
}
define internal ptr @df.selector(ptr %c, i64 %r) alwaysinline {
  %cid = call i64 @df.class(i64 %r)
  ret ptr %c
}
define internal i64 @df.pure(i64 %x) alwaysinline {
  ret i64 %x
}
define i64 @df.lib.f$cx(ptr %ctx, i64 %v0) {
b0:
  %v1 = call i64 @df.class(i64 %v0)
  %v2 = call ptr @df.selector(ptr null, i64 %v1)
  %v3 = call i64 @df.pure(i64 %v1)
  ret i64 %v3
}
define i64 @df.lib.f(i64 %v0) {
  %ctx = call ptr @dartforge_context()
  %r = call i64 @df.lib.f$cx(ptr %ctx, i64 %v0)
  ret i64 %r
}
define i64 @df.lib.g(i64 %v0) {
b0:
  %v1 = call i64 @df.class(i64 %v0)
  ret i64 %v1
}
"#;

    /// The helper that reads the context and the one that calls it get twins;
    /// the `$cx` body calls the twins with its parameter; everyone else keeps
    /// the originals.
    #[test]
    fn threads_the_parameter_into_the_helpers() {
        let out = thread(IR);
        assert!(out.contains(
            "define internal i64 @df.class$ctx(ptr %df.ctx.in, i64 %h) alwaysinline {\n  %ctx = getelementptr inbounds i8, ptr %df.ctx.in, i64 0\n"
        ), "{out}");
        assert!(out.contains(
            "define internal ptr @df.selector$ctx(ptr %df.ctx.in, ptr %c, i64 %r) alwaysinline {\n  %cid = call i64 @df.class$ctx(ptr %df.ctx.in, i64 %r)\n"
        ), "{out}");
        assert!(!out.contains("@df.pure$ctx"), "{out}");
        assert!(out.contains("  %v1 = call i64 @df.class$ctx(ptr %ctx, i64 %v0)\n"));
        assert!(out.contains("  %v2 = call ptr @df.selector$ctx(ptr %ctx, ptr null, i64 %v1)\n"));
        assert!(out.contains("  %v3 = call i64 @df.pure(i64 %v1)\n"));
        // The function without the parameter keeps the original helper.
        assert!(out.contains(
            "define i64 @df.lib.g(i64 %v0) {\nb0:\n  %v1 = call i64 @df.class(i64 %v0)\n"
        ));
    }

    /// A helper without arguments gets the parameter alone.
    #[test]
    fn helper_without_arguments() {
        let ir = "define internal ptr @df.get_area() alwaysinline {\n  %ctx = call ptr @dartforge_context()\n  ret ptr %ctx\n}\n\
define void @df.lib.f$cx(ptr %ctx) {\nb0:\n  %area = call ptr @df.get_area()\n  ret void\n}\n";
        let out = thread(ir);
        assert!(
            out.contains("define internal ptr @df.get_area$ctx(ptr %df.ctx.in) alwaysinline {")
        );
        assert!(out.contains("  %area = call ptr @df.get_area$ctx(ptr %ctx)\n"));
    }

    /// Without helpers that read the context, the text does not change.
    #[test]
    fn unchanged_without_readers() {
        let ir = "define i64 @df.lib.f$cx(ptr %ctx, i64 %v0) {\nb0:\n  ret i64 %v0\n}\n";
        assert_eq!(thread(ir), ir);
    }
}
